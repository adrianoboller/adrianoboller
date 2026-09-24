//! PhxZip web -- o 7z no navegador, numa porta de socket.
//!
//! Terceira peca do pedido 454, e casca fina como o `phxzipcmd`: nenhuma
//! decisao de formato, de nome seguro ou de senha de arquivo mora aqui -- tudo
//! vem da `phxzip`. O HTTP e as sessoes vem do `phxsql-server::http`, o mesmo
//! motor da interface do PhxSql.
//!
//! # Login: pedido ou nao
//!
//! Sem `usuario` na configuracao, a porta atende qualquer um que a alcance --
//! e por isso ela nasce presa a `127.0.0.1`. Com `usuario`, toda rota de
//! arquivo exige sessao aberta por `/api/entrar`. A senha do login nunca fica
//! em texto puro: o servidor guarda so o hash PBKDF2 (`senha::cifrar`), e a
//! conferencia e a mesma do PhxSql (`senha::conferir`).
//!
//! # Contra quem a porta se defende
//!
//! - **Outro site no mesmo navegador (CSRF):** toda rota que muda algo ou le
//!   arquivo exige o cabecalho `X-PhxZip: 1`, que um formulario de outra
//!   origem nao consegue mandar sem a pre-verificacao CORS que esta porta
//!   nunca aprova; e o cookie e `SameSite=Strict`.
//! - **Bomba e corpo gigante:** teto do corpo recusado ANTES de reservar
//!   memoria (413), e os `Limites` do motor na leitura do 7z.
//! - **Adivinhar a senha:** cada login errado custa meio segundo.
//!
//! # O envelope
//!
//! As rotas de arquivo recebem `JSON\n` seguido dos bytes: a linha JSON leva
//! as opcoes (senha do arquivo, nivel, nomes), e o resto e o arquivo. Senha no
//! corpo, e nao na URL nem em cabecalho: URL vai para historico e log.

use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use phxsql_core::json::Json;
use phxsql_core::semaforo::Semaforo;
use phxsql_server::http::{self, FalhaDoPedido, PedidoBinario, Sessoes};
pub mod textos;

use phxzip::{filetime_de_unix, unix_de_filetime, Arquivo7z, Erro, Escritor, Limites, Opcoes};

/// A porta padrao -- a constante UNICA de onde ela sai (pedido 454).
pub const PORTA_PADRAO: u16 = 4000;
/// O endereco padrao: so a propria maquina.
pub const ENDERECO_PADRAO: &str = "127.0.0.1";
const PAGINA: &str = include_str!("../ui/index.html");
/// O icone da aba: a letra Z em ambar (decisao do dono, 24/09/2026: o logo
/// do PhxZip e so a palavra; `marca/vetor/gerar.py`). O cabecalho escreve a
/// palavra em texto, na Exo 2 embutida.
const ICONE_SVG: &str = include_str!("../../../marca/vetor/phxzip-icone.svg");
const COOKIE: &str = "phxzip_sessao";
const SESSAO_MS: i64 = 30 * 60 * 1000;
const MAX_ENVELOPE_JSON: usize = 1 << 20;

/// Configuracao da porta.
#[derive(Clone)]
pub struct Config {
    /// Endereco de escuta.
    pub endereco: String,
    /// Porta.
    pub porta: u16,
    /// Usuario exigido; `None` = porta sem login.
    pub usuario: Option<String>,
    /// Hash PBKDF2 da senha do usuario (nunca a senha).
    pub senha_hash: Option<String>,
    /// Teto do corpo de um pedido, em bytes.
    pub max_corpo: usize,
    /// Conexoes atendidas ao mesmo tempo.
    pub max_conexoes: usize,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("endereco", &self.endereco)
            .field("porta", &self.porta)
            .field("usuario", &self.usuario)
            .field("senha_hash", &self.senha_hash.as_ref().map(|_| "<oculto>"))
            .finish()
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            endereco: ENDERECO_PADRAO.into(),
            porta: PORTA_PADRAO,
            usuario: None,
            senha_hash: None,
            max_corpo: 256 << 20,
            max_conexoes: 32,
        }
    }
}

impl Config {
    /// A porta exige login.
    pub fn exige_login(&self) -> bool {
        self.usuario.is_some()
    }
}

struct Estado {
    config: Config,
    sessoes: Mutex<Sessoes>,
    vagas: Semaforo,
}

fn agora_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Sobe a porta e atende para sempre (ou ate o processo morrer).
pub fn servir(config: Config) -> std::io::Result<()> {
    let ouvinte = TcpListener::bind((config.endereco.as_str(), config.porta))?;
    servir_em(ouvinte, config)
}

/// Atende num ouvinte ja aberto -- e o que os testes usam, na porta 0.
pub fn servir_em(ouvinte: TcpListener, config: Config) -> std::io::Result<()> {
    if config.exige_login() && config.senha_hash.is_none() {
        return Err(std::io::Error::other("usuario sem senha: recusado"));
    }
    let vagas = Semaforo::novo(config.max_conexoes);
    let estado = Arc::new(Estado {
        config,
        sessoes: Mutex::new(Sessoes::default()),
        vagas,
    });
    for fluxo in ouvinte.incoming() {
        let Ok(mut fluxo) = fluxo else { continue };
        // A permissao viaja para dentro da thread e morre com ela, inclusive
        // em panico -- um contador somado e subtraido a mao perderia a vaga
        // no primeiro panico, e a porta encolheria ate fechar.
        let Some(vaga) = estado.vagas.tentar() else {
            http::escoar(&fluxo);
            let _ = http::erro_json(&mut fluxo, 503, "porta cheia: tente de novo");
            continue;
        };
        let e = Arc::clone(&estado);
        std::thread::spawn(move || {
            let _vaga = vaga;
            atender(fluxo, &e);
        });
    }
    Ok(())
}

/// A pagina com as fontes da marca e o icone embutidos, montada uma vez.
/// As fontes vem do `phxsql_core::fontes` -- o mesmo `@font-face` do PhxSql.
fn pagina() -> &'static str {
    static P: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let icone = phxsql_core::base64::codificar(ICONE_SVG.as_bytes());
        PAGINA
            .replace("/*FONTES*/", phxsql_core::fontes::css_das_fontes())
            .replace("__ICONE_B64__", &icone)
    })
}

fn json_ok(pares: Vec<(&str, Json)>) -> Json {
    let mut v = vec![("ok", Json::Bool(true))];
    v.extend(pares);
    Json::objeto(v)
}

/// Erro de arquivo -> codigo HTTP e um `tipo` que a tela traduz pela chave.
fn erro_do_zip(e: &Erro) -> (u16, &'static str) {
    match e {
        Erro::SenhaNecessaria => (400, "senha_necessaria"),
        Erro::SenhaErradaOuCorrompido => (400, "senha_errada"),
        Erro::MetodoRecusado { .. } | Erro::NaoSuportado(_) => (400, "recusado"),
        Erro::Teto(_) | Erro::GrandeDemaisParaEsteAlvo => (413, "teto"),
        Erro::CaminhoInseguro => (400, "caminho"),
        Erro::Uso(_) => (400, "uso"),
        _ => (400, "corrompido"),
    }
}

fn responder_erro(fluxo: &mut TcpStream, codigo: u16, tipo: &str, msg: &str) {
    responder_erro_com(fluxo, codigo, tipo, msg, None);
}

fn responder_erro_com(
    fluxo: &mut TcpStream,
    codigo: u16,
    tipo: &str,
    msg: &str,
    metodo: Option<&str>,
) {
    let mut pares = vec![
        ("ok", Json::Bool(false)),
        ("tipo", Json::texto_de(tipo)),
        ("erro", Json::texto_de(msg)),
    ];
    // O nome do metodo recusado vai como DADO, para a tela montar a frase
    // no idioma dela em vez de repetir a frase portuguesa do motor.
    if let Some(m) = metodo {
        pares.push(("metodo", Json::texto_de(m)));
    }
    let corpo = Json::objeto(pares);
    let _ = http::responder_json(fluxo, codigo, &corpo);
}

fn sessao_do_cookie(p: &PedidoBinario) -> Option<String> {
    let c = p.cabecalho("cookie")?;
    c.split(';').find_map(|par| {
        let (k, v) = par.trim().split_once('=')?;
        (k == COOKIE).then(|| v.to_string())
    })
}

/// Separa a linha JSON do resto dos bytes.
fn envelope(corpo: &[u8]) -> Result<(Json, &[u8]), &'static str> {
    let fim = corpo
        .iter()
        .take(MAX_ENVELOPE_JSON)
        .position(|b| *b == b'\n')
        .ok_or("envelope sem a linha JSON")?;
    let texto = std::str::from_utf8(&corpo[..fim]).map_err(|_| "linha JSON nao e UTF-8")?;
    let j = Json::analisar(texto).map_err(|_| "linha JSON invalida")?;
    Ok((j, &corpo[fim + 1..]))
}

fn atender(mut fluxo: TcpStream, est: &Estado) {
    let _ = fluxo.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = fluxo.set_write_timeout(Some(Duration::from_secs(60)));
    let p = match http::ler_pedido_binario(&fluxo, est.config.max_corpo) {
        Ok(p) => p,
        Err(FalhaDoPedido::Grande) => {
            http::escoar(&fluxo);
            responder_erro(&mut fluxo, 413, "teto", "arquivo acima do teto desta porta");
            return;
        }
        Err(FalhaDoPedido::Ilegivel) => return,
    };
    let rota = (p.metodo.as_str(), p.caminho.as_str());
    match rota {
        ("GET", "/") => {
            let _ = http::responder_pagina(&mut fluxo, 200, pagina());
            return;
        }
        // Os textos da tela, resolvidos pela fabrica de idiomas do PhxSql.
        // Publico como a pagina: o formulario de login tambem tem rotulo.
        ("GET", "/api/textos") => {
            let idioma = http::parametro(&p.consulta, "idioma");
            let _ = http::responder_json(&mut fluxo, 200, &textos::para_a_pagina(&idioma));
            return;
        }
        ("GET", "/api/estado") => {
            let logado = logado(est, &p);
            let j = json_ok(vec![
                ("exige_login", Json::Bool(est.config.exige_login())),
                ("logado", Json::Bool(logado.is_some())),
                ("usuario", logado.map(Json::texto_de).unwrap_or(Json::Nulo)),
                ("versao", Json::texto_de(env!("CARGO_PKG_VERSION"))),
            ]);
            let _ = http::responder_json(&mut fluxo, 200, &j);
            return;
        }
        _ => {}
    }
    if p.metodo != "POST" || !p.caminho.starts_with("/api/") {
        responder_erro(&mut fluxo, 404, "rota", "rota inexistente");
        return;
    }
    // CSRF: o cabecalho que outra origem nao manda sem pre-verificacao.
    if p.cabecalho("x-phxzip") != Some("1") {
        responder_erro(&mut fluxo, 403, "origem", "pedido sem o cabecalho X-PhxZip");
        return;
    }
    match p.caminho.as_str() {
        "/api/entrar" => return entrar(&mut fluxo, est, &p),
        "/api/sair" => {
            if let Some(id) = sessao_do_cookie(&p) {
                if let Ok(mut s) = est.sessoes.lock() {
                    s.encerrar(&id);
                }
            }
            let extras =
                format!("Set-Cookie: {COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0\r\n");
            let _ = http::responder_com_extras(
                &mut fluxo,
                200,
                "application/json; charset=utf-8",
                &json_ok(vec![]).escrever(),
                &extras,
            );
            return;
        }
        _ => {}
    }
    if est.config.exige_login() && logado(est, &p).is_none() {
        responder_erro(&mut fluxo, 401, "login", "entre com usuario e senha");
        return;
    }
    let (meta, bytes) = match envelope(&p.corpo) {
        Ok(x) => x,
        Err(m) => {
            responder_erro(&mut fluxo, 400, "uso", m);
            return;
        }
    };
    let r = match p.caminho.as_str() {
        "/api/listar" => listar(&mut fluxo, &meta, bytes, false),
        "/api/testar" => listar(&mut fluxo, &meta, bytes, true),
        "/api/extrair" => extrair(&mut fluxo, &meta, bytes),
        "/api/compactar" => compactar(&mut fluxo, &meta, bytes),
        _ => {
            responder_erro(&mut fluxo, 404, "rota", "rota inexistente");
            Ok(())
        }
    };
    if let Err(e) = r {
        let (c, t) = erro_do_zip(&e);
        let metodo = match &e {
            Erro::MetodoRecusado { nome, .. } => Some(*nome),
            _ => None,
        };
        responder_erro_com(&mut fluxo, c, t, &e.to_string(), metodo);
    }
}

fn logado(est: &Estado, p: &PedidoBinario) -> Option<String> {
    let id = sessao_do_cookie(p)?;
    est.sessoes.lock().ok()?.usar(&id, SESSAO_MS, agora_ms())
}

fn entrar(fluxo: &mut TcpStream, est: &Estado, p: &PedidoBinario) {
    let j = std::str::from_utf8(&p.corpo)
        .ok()
        .and_then(|t| Json::analisar(t).ok());
    let (usuario, senha) = match &j {
        Some(j) => (j.texto_ou("usuario", ""), j.texto_ou("senha", "")),
        None => ("", ""),
    };
    let (Some(u), Some(h)) = (&est.config.usuario, &est.config.senha_hash) else {
        // Porta sem login: entrar e sempre certo, e nao abre sessao.
        let _ = http::responder_json(fluxo, 200, &json_ok(vec![]));
        return;
    };
    let usuario_bate =
        phxsql_core::hash::iguais_em_tempo_constante(usuario.as_bytes(), u.as_bytes());
    // Confere a senha mesmo com usuario errado: o tempo nao diz qual errou.
    let senha_bate = phxsql_core::senha::conferir(senha, h);
    if !(usuario_bate && senha_bate) {
        std::thread::sleep(Duration::from_millis(500));
        responder_erro(fluxo, 401, "login_errado", "usuario ou senha errados");
        return;
    }
    let id = match est.sessoes.lock() {
        Ok(mut s) => s.nova(u, SESSAO_MS, agora_ms()),
        Err(_) => {
            responder_erro(fluxo, 500, "interno", "sessoes indisponiveis");
            return;
        }
    };
    let extras = format!(
        "Set-Cookie: {COOKIE}={id}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}\r\n",
        SESSAO_MS / 1000
    );
    let corpo = json_ok(vec![("usuario", Json::texto_de(u.as_str()))]).escrever();
    let _ = http::responder_com_extras(
        fluxo,
        200,
        "application/json; charset=utf-8",
        &corpo,
        &extras,
    );
}

fn senha_do(meta: &Json) -> Option<&str> {
    meta.campo("senha")
        .and_then(Json::texto)
        .filter(|s| !s.is_empty())
}

fn listar(fluxo: &mut TcpStream, meta: &Json, bytes: &[u8], testar: bool) -> Result<(), Erro> {
    let a = Arquivo7z::abrir(bytes, senha_do(meta), Limites::default())?;
    let inicio = std::time::Instant::now();
    if testar {
        a.testar()?;
    }
    let ms = inicio.elapsed().as_millis() as u64;
    let entradas: Vec<Json> = a
        .entradas()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            Json::objeto(vec![
                ("indice", Json::de_u64(i as u64)),
                ("nome", Json::texto_de(e.nome.as_str())),
                ("tamanho", Json::de_u64(e.tamanho)),
                ("pasta", Json::Bool(e.e_pasta)),
                ("cifrada", Json::Bool(e.cifrada)),
                ("seguro", Json::Bool(e.caminho().is_ok() && !e.e_ligacao())),
                (
                    "mtime",
                    e.mtime
                        .map(|t| Json::de_i64(unix_de_filetime(t)))
                        .unwrap_or(Json::Nulo),
                ),
                ("metodos", Json::texto_de(a.metodos(i).join("+"))),
            ])
        })
        .collect();
    let j = json_ok(vec![
        ("cabecalho_cifrado", Json::Bool(a.cabecalho_cifrado())),
        ("testado", Json::Bool(testar)),
        ("ms", Json::de_u64(ms)),
        ("tamanho_arquivo", Json::de_u64(bytes.len() as u64)),
        ("entradas", Json::Lista(entradas)),
    ]);
    let _ = http::responder_json(fluxo, 200, &j);
    Ok(())
}

fn extrair(fluxo: &mut TcpStream, meta: &Json, bytes: &[u8]) -> Result<(), Erro> {
    let a = Arquivo7z::abrir(bytes, senha_do(meta), Limites::default())?;
    let i = meta.inteiro_ou("indice", -1);
    let i = usize::try_from(i).map_err(|_| Erro::Uso("indice ausente"))?;
    let e = a
        .entradas()
        .get(i)
        .ok_or(Erro::Uso("indice de entrada inexistente"))?;
    // A web entrega o arquivo para o navegador gravar: o nome seguro vai
    // junto, e entrada com nome inseguro nao sai -- a mesma recusa do disco.
    e.caminho()?;
    if e.e_ligacao() {
        return Err(Erro::NaoSuportado("ligacao simbolica dentro do arquivo"));
    }
    let d = a.extrair(i)?;
    let _ = http::responder_bytes(fluxo, 200, "application/octet-stream", &d, "");
    Ok(())
}

fn compactar(fluxo: &mut TcpStream, meta: &Json, bytes: &[u8]) -> Result<(), Erro> {
    let nivel = meta.inteiro_ou("nivel", 5).clamp(0, 9) as u8;
    let mut acaso = [0u8; 32];
    phxsql_core::cifra::sortear(&mut acaso);
    let op = Opcoes {
        nivel,
        senha: senha_do(meta).map(String::from),
        cifrar_nomes: !meta.booleano_ou("nomes_visiveis", false),
        acaso,
    };
    let mut esc = Escritor::novo(op);
    let lista = meta
        .campo("arquivos")
        .and_then(Json::lista)
        .ok_or(Erro::Uso("lista de arquivos ausente"))?;
    let agora = filetime_de_unix(agora_ms() / 1000);
    let mut pos = 0usize;
    for item in lista {
        let nome = item.texto_ou("nome", "");
        let tam = usize::try_from(item.inteiro_ou("tamanho", -1))
            .map_err(|_| Erro::Uso("tamanho ausente"))?;
        let fim = pos
            .checked_add(tam)
            .filter(|f| *f <= bytes.len())
            .ok_or(Erro::Uso("tamanhos nao fecham com o corpo"))?;
        let mtime = item
            .campo("mtime")
            .and_then(Json::inteiro)
            .map(filetime_de_unix)
            .unwrap_or(agora);
        esc.arquivo(nome, bytes[pos..fim].to_vec(), Some(mtime), None)?;
        pos = fim;
    }
    if pos != bytes.len() {
        return Err(Erro::Uso("sobram bytes depois do ultimo arquivo"));
    }
    let inicio = std::time::Instant::now();
    let saida = esc.gravar()?;
    let extras = format!("X-PhxZip-Ms: {}\r\n", inicio.elapsed().as_millis());
    let _ = http::responder_bytes(fluxo, 200, "application/x-7z-compressed", &saida, &extras);
    Ok(())
}
