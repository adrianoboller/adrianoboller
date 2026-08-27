//! Servidor TCP do PhxSql.
//!
//! Protocolo JSON Lines: uma linha JSON por pedido, uma linha JSON por
//! resposta, UTF-8, terminadas em `\n`. A conexao aceita varios pedidos
//! seguidos e cada um vira uma entrada no log de acessos.
//!
//! ```text
//! -> {"token":"...","op":"ping"}
//! <- {"ok":true,"op":"ping","resultado":{"phxsql":"0.1.0"},"ms":0}
//! ```
//!
//! # Concorrencia
//!
//! O motor de armazenamento ainda nao tem travas de arquivo nem de registro,
//! entao TODO acesso a dados passa por um mutex unico: as conexoes sao
//! aceitas em paralelo, mas as operacoes se enfileiram. E lento sob carga e e
//! correto -- o contrario seria rapido e corrompido. Travas finas entram junto
//! com as transacoes.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_store::catalogo::Instancia;
use phxsql_store::table::Table;

use crate::acesso::{Acesso, LogAcessos};
use crate::config::Config;
use crate::usuarios::{Atividade, Usuario};
use crate::valores::{json_para_chave, json_para_linha, linha_para_json};

pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Operacoes que alteram dados. Recusadas quando `somente_leitura` esta ligado.
const OPS_ESCRITA: &[&str] = &[
    "inserir",
    "atualizar",
    "excluir",
    "reindexar",
    "criar_database",
    "criar_schema",
];

/// Estado de uma conexao.
///
/// A senha e conferida com PBKDF2, que custa da ordem de 100 ms de proposito.
/// Fazer isso a cada pedido inviabilizaria o servidor, entao a autenticacao
/// acontece UMA VEZ por conexao e o resultado fica aqui.
#[derive(Default)]
struct Sessao {
    usuario: Option<Usuario>,
}

impl Sessao {
    fn login(&self) -> &str {
        self.usuario
            .as_ref()
            .map(|u| u.login.as_str())
            .unwrap_or("")
    }

    /// Id gravado no `.log` da tabela como autor da operacao.
    /// Zero quando a conexao veio pelo token de servico, sem login.
    fn id(&self) -> u32 {
        self.usuario.as_ref().map(|u| u.id).unwrap_or(0)
    }
}

pub struct Servidor {
    config: Config,
    /// Trava unica de dados. Ver a nota de concorrencia no topo do modulo.
    dados: Mutex<Instancia>,
    log: Mutex<LogAcessos>,
    conexoes: AtomicUsize,
}

impl Servidor {
    pub fn novo(config: Config) -> Result<Arc<Servidor>> {
        let instancia = Instancia::nova(&config.base)?;
        let log = LogAcessos::abrir(&config.log_acessos)?;
        Ok(Arc::new(Servidor {
            config,
            dados: Mutex::new(instancia),
            log: Mutex::new(log),
            conexoes: AtomicUsize::new(0),
        }))
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Sobe o servidor e atende ate o processo ser encerrado.
    pub fn escutar(self: &Arc<Self>) -> Result<()> {
        let endereco = self.config.endereco()?;
        let ouvinte = TcpListener::bind(endereco)
            .map_err(|e| PhxError::Esquema(format!("nao consegui escutar em {endereco}: {e}")))?;
        eprintln!(
            "PhxSql {VERSAO} escutando em {endereco} | base {} | papel {}",
            self.config.base.display(),
            self.config.replicacao.papel.nome()
        );
        eprintln!("log de acessos: {}", self.config.log_acessos.display());

        for conexao in ouvinte.incoming() {
            match conexao {
                Ok(fluxo) => {
                    let par = fluxo.peer_addr().ok();
                    if self.conexoes.load(Ordering::SeqCst) >= self.config.conexoes_max {
                        // Recusa sem derrubar o servico, e deixa registro.
                        if let Some(p) = par {
                            self.anotar(&Acesso {
                                quando_ms: crate::agora_ms(),
                                ip: p.ip().to_string(),
                                porta_origem: p.port(),
                                op: "conexao".into(),
                                usuario: String::new(),
                                autenticado: false,
                                ok: false,
                                duracao_ms: 0,
                                erro: Some("limite de conexoes atingido".into()),
                            });
                        }
                        continue;
                    }
                    let servidor = Arc::clone(self);
                    self.conexoes.fetch_add(1, Ordering::SeqCst);
                    std::thread::spawn(move || {
                        let endereco = par.unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
                        servidor.atender(fluxo, endereco);
                        servidor.conexoes.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) => eprintln!("conexao recusada pelo sistema: {e}"),
            }
        }
        Ok(())
    }

    fn anotar(&self, acesso: &Acesso) {
        if let Ok(mut log) = self.log.lock() {
            if let Err(e) = log.registrar(acesso) {
                eprintln!("falha ao gravar o log de acessos: {e}");
            }
        }
    }

    fn atender(&self, fluxo: TcpStream, par: SocketAddr) {
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));

        let permitido = self.config.ip_permitido(&ip);
        let escrita = fluxo.try_clone();
        let mut leitor = BufReader::new(fluxo);
        let mut saida = match escrita {
            Ok(f) => f,
            Err(_) => return,
        };

        if !permitido {
            self.anotar(&Acesso {
                quando_ms: crate::agora_ms(),
                ip,
                porta_origem: porta,
                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),
            });
            let _ = writeln!(
                saida,
                "{}",
                resposta_erro("conexao", "ip nao autorizado", 0).escrever()
            );
            return;
        }

        let mut sessao = Sessao::default();
        let mut linha = String::new();
        loop {
            linha.clear();
            match leitor.read_line(&mut linha) {
                Ok(0) => return, // conexao fechada
                Ok(_) => {}
                Err(_) => return,
            }
            if linha.trim().is_empty() {
                continue;
            }

            let inicio = Instant::now();
            let quando_ms = crate::agora_ms();
            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao);
            let duracao = inicio.elapsed().as_millis() as u64;

            let resposta = match &resultado {
                Ok(valor) => Json::objeto(vec![
                    ("ok", Json::Bool(true)),
                    ("op", Json::texto_de(&op)),
                    ("resultado", valor.clone()),
                    ("ms", Json::de_u64(duracao)),
                ]),
                Err(e) => resposta_erro(&op, &e.to_string(), duracao),
            };

            self.anotar(&Acesso {
                quando_ms,
                ip: ip.clone(),
                porta_origem: porta,
                op: op.clone(),
                usuario: sessao.login().to_string(),
                autenticado,
                ok: resultado.is_ok(),
                duracao_ms: duracao,
                erro: resultado.as_ref().err().map(|e| e.to_string()),
            });

            if writeln!(saida, "{}", resposta.escrever()).is_err() {
                return;
            }
            let _ = saida.flush();
        }
    }

    /// Le o pedido e o leva por tres portoes, nesta ordem: o token (a rede),
    /// o login (a identidade) e a permissao (o poder). Devolve (operacao,
    /// autenticado, resultado) para que o log registre mesmo o que falhou.
    fn despachar(&self, linha: &str, sessao: &mut Sessao) -> (String, bool, Result<Json>) {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            Err(e) => return ("?".into(), false, Err(e)),
        };
        let op = pedido.texto_ou("op", "").trim().to_string();
        let op = if op.is_empty() {
            "ping".to_string()
        } else {
            op
        };

        // Portao 1 -- o token. E a chave da porta da rede, nao a identidade.
        if !self.config.token_confere(pedido.texto_ou("token", "")) {
            return (
                op,
                false,
                Err(PhxError::Autorizacao("token invalido".into())),
            );
        }

        // Portao 2 -- o login. Havendo cadastro, o token sozinho nao basta.
        if op == "login" {
            let r = self.op_login(&pedido, sessao);
            return (op, r.is_ok(), r);
        }
        if !self.config.cadastro.vazio()
            && sessao.usuario.is_none()
            && Atividade::da_operacao(&op).is_some()
        {
            return (
                op,
                true,
                Err(PhxError::Autorizacao(
                    "faca login antes: {\"op\":\"login\",\"usuario\":...,\"senha\":...}".into(),
                )),
            );
        }

        if self.config.somente_leitura && OPS_ESCRITA.contains(&op.as_str()) {
            return (
                op,
                true,
                Err(PhxError::Autorizacao(
                    "servidor em modo somente leitura".into(),
                )),
            );
        }

        // Portao 3 -- o poder deste usuario sobre a base deste pedido.
        if let (Some(atividade), Some(usuario)) =
            (Atividade::da_operacao(&op), sessao.usuario.as_ref())
        {
            let base = pedido.texto_ou("database", "");
            if !usuario.pode(base, atividade) {
                return (
                    op,
                    true,
                    Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de {} em {}",
                        usuario.login,
                        atividade.nome(),
                        if base.is_empty() { "(sem base)" } else { base }
                    ))),
                );
            }
        }

        let r = self.executar(&op, &pedido, sessao);
        (op, true, r)
    }

    /// Confere login e senha e guarda a identidade na conexao.
    fn op_login(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = p
            .texto_ou("usuario", p.texto_ou("login", ""))
            .trim()
            .to_string();
        let clara = p.texto_ou("senha", "");
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\" e \"senha\"".into()));
        }
        match self.config.cadastro.autenticar(&login, clara) {
            Some(u) => {
                let ficha = u.ficha();
                sessao.usuario = Some(u.clone());
                Ok(ficha)
            }
            None => {
                sessao.usuario = None;
                // Mensagem unica de proposito: nao dizer se o que errou foi o
                // login ou a senha.
                Err(PhxError::Autorizacao("usuario ou senha invalidos".into()))
            }
        }
    }

    fn executar(&self, op: &str, p: &Json, sessao: &Sessao) -> Result<Json> {
        match op {
            "ping" => Ok(Json::objeto(vec![
                ("phxsql", Json::texto_de(VERSAO)),
                ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
                (
                    "conexoes",
                    Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
                ),
            ])),
            "config" => Ok(self.config.para_json()),
            "quem_sou" => Ok(match &sessao.usuario {
                Some(u) => u.ficha(),
                None => Json::objeto(vec![
                    ("usuario", Json::Nulo),
                    ("via", Json::texto_de("token de servico")),
                ]),
            }),
            "usuarios" => Ok(self.config.cadastro.fichas()),
            "acessos" => self.op_acessos(p),
            "ips" => self.op_ips(),
            "bancos" => self.op_bancos(),
            "tabelas" => self.op_tabelas(p),
            "esquema" => self.op_esquema(p, sessao),
            "criar_database" => self.op_criar_database(p),
            "ler" => self.op_ler(p, sessao),
            "varrer" => self.op_varrer(p, sessao),
            "buscar" => self.op_buscar(p, sessao),
            "inserir" => self.op_inserir(p, sessao),
            "atualizar" => self.op_atualizar(p, sessao),
            "excluir" => self.op_excluir(p, sessao),
            "diario" => self.op_diario(p, sessao),
            "verificar" => self.op_verificar(p, sessao),
            "reindexar" => self.op_reindexar(p, sessao),
            outro => Err(PhxError::NaoEncontrado(format!(
                "operacao desconhecida: {outro}"
            ))),
        }
    }

    // ------------------------------------------------------------ ajudantes

    fn abrir(&self, p: &Json, sessao: &Sessao) -> Result<Table> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        if database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\" e \"tabela\"".into(),
            ));
        }
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = dados.abrir_database(database)?.abrir_qualificada(tabela)?;
        // Quem alterar assina o evento no .log da tabela.
        t.definir_usuario(sessao.id());
        Ok(t)
    }

    fn rowid(&self, p: &Json) -> Result<u64> {
        p.campo("rowid")
            .and_then(Json::inteiro)
            .filter(|n| *n > 0)
            .map(|n| n as u64)
            .ok_or_else(|| PhxError::Esquema("informe \"rowid\" maior que zero".into()))
    }

    fn limite(&self, p: &Json) -> u64 {
        let pedido = p.inteiro_ou("max", self.config.max_linhas as i64).max(0) as u64;
        if pedido == 0 {
            self.config.max_linhas
        } else {
            pedido.min(self.config.max_linhas)
        }
    }

    // ------------------------------------------------------------ operacoes

    fn op_acessos(&self, p: &Json) -> Result<Json> {
        let max = self.limite(p) as usize;
        let todos = LogAcessos::ler(&self.config.log_acessos)?;
        let total = todos.len();
        let recentes: Vec<Json> = todos
            .iter()
            .rev()
            .take(max)
            .map(|a| a.para_json())
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("acessos", Json::Lista(recentes)),
        ]))
    }

    fn op_ips(&self) -> Result<Json> {
        let resumo = LogAcessos::resumo_por_ip(&self.config.log_acessos)?;
        Ok(Json::Lista(
            resumo
                .iter()
                .map(|r| {
                    Json::objeto(vec![
                        ("ip", Json::texto_de(&r.ip)),
                        ("acessos", Json::de_u64(r.acessos)),
                        ("recusados", Json::de_u64(r.recusados)),
                        ("primeiro", Json::texto_de(r.primeiro())),
                        ("ultimo", Json::texto_de(r.ultimo())),
                    ])
                })
                .collect(),
        ))
    }

    fn op_bancos(&self) -> Result<Json> {
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        Ok(Json::Lista(
            dados.databases()?.into_iter().map(Json::texto_de).collect(),
        ))
    }

    fn op_tabelas(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(nome)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(nome)),
            (
                "schemas",
                Json::Lista(db.schemas()?.into_iter().map(Json::texto_de).collect()),
            ),
            (
                "tabelas",
                Json::Lista(
                    db.todas_as_tabelas()?
                        .into_iter()
                        .map(Json::texto_de)
                        .collect(),
                ),
            ),
        ]))
    }

    fn op_criar_database(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.criar_database(nome)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(db.nome())),
            (
                "caminho",
                Json::texto_de(db.caminho().display().to_string()),
            ),
        ]))
    }

    fn op_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let t = self.abrir(p, sessao)?;
        let e = t.esquema();
        let colunas: Vec<Json> = e
            .colunas()
            .iter()
            .map(|c| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                ])
            })
            .collect();
        let indices: Vec<Json> = e
            .indices()
            .iter()
            .map(|i| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&i.nome)),
                    ("unico", Json::Bool(i.unico)),
                    (
                        "colunas",
                        Json::Lista(
                            i.colunas
                                .iter()
                                .map(|ic| {
                                    Json::objeto(vec![
                                        ("coluna", Json::texto_de(&e.colunas()[ic.coluna].nome)),
                                        ("desc", Json::Bool(ic.desc)),
                                        ("nocase", Json::Bool(ic.nocase)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        let fks: Vec<Json> = e
            .chaves_estrangeiras()
            .iter()
            .map(|fk| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&fk.nome)),
                    (
                        "colunas",
                        Json::Lista(
                            fk.colunas
                                .iter()
                                .map(|c| Json::texto_de(&e.colunas()[*c].nome))
                                .collect(),
                        ),
                    ),
                    ("tabela_ref", Json::texto_de(&fk.tabela_ref)),
                    (
                        "colunas_ref",
                        Json::Lista(fk.colunas_ref.iter().map(Json::texto_de).collect()),
                    ),
                    ("ao_excluir", Json::texto_de(format!("{:?}", fk.ao_excluir))),
                    ("ao_alterar", Json::texto_de(format!("{:?}", fk.ao_alterar))),
                ])
            })
            .collect();
        let pag = e.paginacao();
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(e.nome())),
            ("registros", Json::de_u64(t.registros())),
            ("slots", Json::de_u64(t.slots())),
            ("colunas", Json::Lista(colunas)),
            ("indices", Json::Lista(indices)),
            ("chaves_estrangeiras", Json::Lista(fks)),
            (
                "paginacao",
                if pag.ligada() {
                    Json::objeto(vec![
                        (
                            "registros_por_arquivo",
                            Json::de_u64(pag.registros_por_arquivo),
                        ),
                        ("max_arquivos", Json::de_u64(pag.max_arquivos as u64)),
                        ("capacidade", Json::de_u64(pag.capacidade())),
                    ])
                } else {
                    Json::Nulo
                },
            ),
        ]))
    }

    fn op_ler(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        match t.ler(rowid)? {
            None => Ok(Json::Nulo),
            Some(linha) => Ok(linha_para_json(&linha, t.esquema())),
        }
    }

    fn op_varrer(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let max = self.limite(p);
        let indice = p.texto_ou("indice", "").to_string();
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;

        let rowids: Vec<u64> = if indice.is_empty() {
            t.varrer()?.into_iter().map(|(r, _)| r).collect()
        } else {
            t.varrer_indice(&indice)?
        };
        let total = rowids.len();
        let mut linhas = Vec::new();
        for rowid in rowids.into_iter().take(max as usize) {
            if let Some(l) = t.ler(rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("devolvidas", Json::de_u64(linhas.len() as u64)),
            (
                "ordem",
                Json::texto_de(if indice.is_empty() {
                    "digitacao".to_string()
                } else {
                    format!("indice {indice}")
                }),
            ),
            ("linhas", Json::Lista(linhas)),
        ]))
    }

    fn op_buscar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let indice = p.texto_ou("indice", "").to_string();
        if indice.is_empty() {
            return Err(PhxError::Esquema("informe \"indice\"".into()));
        }
        let chave_json = p
            .campo("chave")
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"chave\"".into()))?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let pos = t
            .esquema()
            .indice_por_nome(&indice)
            .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {indice} nao existe")))?;
        let chave = json_para_chave(&chave_json, t.esquema(), pos)?;
        let rowids = t.buscar(&indice, &chave)?;

        let mut linhas = Vec::new();
        for rowid in rowids.iter().take(self.limite(p) as usize) {
            if let Some(l) = t.ler(*rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(*rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }
        Ok(Json::objeto(vec![
            ("encontrados", Json::de_u64(rowids.len() as u64)),
            ("linhas", Json::Lista(linhas)),
        ]))
    }

    fn op_inserir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let valores_json = p
            .campo("valores")
            .or_else(|| p.campo("linha"))
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let linha = json_para_linha(&valores_json, t.esquema())?;
        let rowid = t.inserir(&linha)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("registros", Json::de_u64(t.registros())),
        ]))
    }

    fn op_atualizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let valores_json = p
            .campo("valores")
            .or_else(|| p.campo("linha"))
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let linha = json_para_linha(&valores_json, t.esquema())?;
        t.atualizar(rowid, &linha)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![("rowid", Json::de_u64(rowid))]))
    }

    fn op_excluir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let removeu = t.excluir(rowid)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("excluido", Json::Bool(removeu)),
        ]))
    }

    fn op_diario(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let max = self.limite(p) as usize;
        let rowid = p.campo("rowid").and_then(Json::inteiro).map(|n| n as u64);
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let eventos = match rowid {
            Some(r) => t.historico(r)?,
            None => t.diario(0, 0)?,
        };
        let total = eventos.len();
        let recentes: Vec<Json> = eventos
            .iter()
            .rev()
            .take(max)
            .rev()
            .map(|e| {
                Json::objeto(vec![
                    ("quando", Json::texto_de(e.instante_iso())),
                    ("carimbo_ms", Json::Numero(e.carimbo as f64)),
                    ("operacao", Json::texto_de(e.operacao.nome())),
                    ("rowid", Json::de_u64(e.rowid)),
                    ("versao", Json::de_u64(e.versao)),
                    ("usuario", Json::de_u64(e.usuario as u64)),
                ])
            })
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("eventos", Json::Lista(recentes)),
        ]))
    }

    fn op_verificar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let r = t.verificar()?;
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&r.tabela)),
            ("registros", Json::de_u64(r.registros)),
            ("slots", Json::de_u64(r.slots)),
            ("eventos", Json::de_u64(r.eventos)),
            (
                "indices",
                Json::Objeto(
                    r.indices
                        .iter()
                        .map(|(n, q)| (n.clone(), Json::de_u64(*q)))
                        .collect(),
                ),
            ),
            (
                "volumes",
                Json::objeto(vec![
                    ("reg", Json::de_u64(r.volumes.0 as u64)),
                    ("bin", Json::de_u64(r.volumes.1 as u64)),
                    ("memo", Json::de_u64(r.volumes.2 as u64)),
                    ("log", Json::de_u64(r.volumes.3 as u64)),
                ]),
            ),
        ]))
    }

    fn op_reindexar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let indices = t.reindexar()?;
        t.sincronizar()?;
        Ok(Json::Objeto(
            indices
                .into_iter()
                .map(|(n, q)| (n, Json::de_u64(q)))
                .collect(),
        ))
    }
}

fn trava_envenenada() -> PhxError {
    PhxError::Corrompido("uma operacao anterior entrou em panico e deixou a trava suja".into())
}

fn resposta_erro(op: &str, mensagem: &str, ms: u64) -> Json {
    Json::objeto(vec![
        ("ok", Json::Bool(false)),
        ("op", Json::texto_de(op)),
        ("erro", Json::texto_de(mensagem)),
        ("ms", Json::de_u64(ms)),
    ])
}
