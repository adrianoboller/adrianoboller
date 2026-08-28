//! O Profiler: o que esta CHEGANDO pela porta, antes de virar dado.
//!
//! # O que ele e
//!
//! O equivalente do Profiler do SQL Server(R): liga-se, escolhe-se o que
//! observar (banco, usuario, operacao), e ve-se o trafego passar. Cada pedido
//! aparece **quando chega**, com o texto que veio pelo soquete -- e nao depois,
//! reconstruido a partir do que o motor entendeu.
//!
//! O ponto de captura e uma linha depois do `read_line` e uma linha antes do
//! despacho. Nada foi gravado ainda. E por isso que ele serve para achar o
//! pedido que derruba o servidor: ele aparece mesmo que a operacao nunca
//! termine.
//!
//! # A senha NAO passa por aqui
//!
//! Esta e a regra que mais importa neste arquivo, porque um profiler e
//! exatamente o lugar onde uma senha vazaria sem ninguem notar: ele existe
//! para mostrar o texto cru do pedido, e o pedido de `login` traz a senha
//! dentro.
//!
//! Entao o texto NUNCA e guardado como veio. Ele e analisado, os campos
//! sensiveis viram `"***"`, e so o resultado disso e guardado ou escrito em
//! arquivo. Pedido que nao e JSON valido nao vira texto nenhum -- vira o
//! tamanho dele. Ha teste que falha se uma senha aparecer no anel ou no
//! arquivo.
//!
//! # Anel, e nao lista
//!
//! O que fica em memoria e um anel de tamanho fixo: um profiler esquecido
//! ligado num servidor movimentado nao pode comer a memoria da maquina. O
//! arquivo, esse, cresce -- mas quem o pediu escolheu o caminho e sabe onde
//! ele esta.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

/// Campos cujo valor nunca e guardado nem escrito.
///
/// A lista e por NOME e nao por heuristica: adivinhar o que e sensivel pelo
/// formato do valor erra nos dois sentidos, e errar para o lado de mostrar e
/// irreversivel -- o texto ja saiu.
const SEGREDOS: &[&str] = &[
    "senha",
    "senha_b64",
    "senha_hash",
    "nova_senha",
    "prova",
    "token",
    "chave",
    "chave_privada",
    "assinatura",
];

/// Operacoes que MUDAM dado. `so_escrita` filtra por esta lista.
const ESCRITAS: &[&str] = &[
    "inserir",
    "inserir_lote",
    "atualizar",
    "excluir",
    "restaurar",
    "esvaziar_lixeira",
    "aplicar",
    "criar_tabela",
    "excluir_tabela",
    "criar_database",
    "criar_schema",
    "duplicar_tabela",
    "copiar_tabela",
    "reindexar",
    "ajustar_sequencia",
];

/// O que observar. Campo vazio = nao filtra por ele.
#[derive(Debug, Clone, Default)]
pub struct Filtro {
    pub database: String,
    pub usuario: String,
    pub op: String,
    pub so_escrita: bool,
}

impl Filtro {
    fn aceita(&self, op: &str, usuario: &str, database: &str) -> bool {
        // A LEITURA do proprio profiler nunca entra. A tela pergunta uma vez
        // por segundo enquanto esta aberta, e sem esta linha o profiler
        // encheria de si mesmo -- em poucos minutos o anel seria so ele, e o
        // pedido que alguem estava procurando teria saido pela borda.
        // `profiler_ligar` e `profiler_desligar` entram: sao raros e dizem
        // quem mexeu na observacao.
        if op == "profiler" {
            return false;
        }
        if !self.database.is_empty() && !self.database.eq_ignore_ascii_case(database) {
            return false;
        }
        if !self.usuario.is_empty() && !self.usuario.eq_ignore_ascii_case(usuario) {
            return false;
        }
        if !self.op.is_empty() && !self.op.eq_ignore_ascii_case(op) {
            return false;
        }
        if self.so_escrita && !ESCRITAS.contains(&op) {
            return false;
        }
        true
    }
}

/// Um pedido, do jeito que chegou -- menos o que nao pode ser mostrado.
#[derive(Debug, Clone)]
pub struct Evento {
    pub serial: u64,
    pub quando_ms: i64,
    pub ip: String,
    pub usuario: String,
    pub op: String,
    pub database: String,
    pub tabela: String,
    /// Bytes que vieram pelo soquete, do pedido ORIGINAL.
    pub bytes: usize,
    /// O pedido, com os campos sensiveis substituidos.
    pub pedido: String,
    /// `None` enquanto a operacao esta em curso.
    pub duracao_ms: Option<u64>,
    pub ok: Option<bool>,
    pub erro: String,
}

impl Evento {
    /// A linha que vai para o arquivo de texto.
    ///
    /// Largura fixa nos primeiros campos para o arquivo poder ser lido com o
    /// olho, e o pedido no fim porque e o unico de tamanho livre.
    fn linha(&self) -> String {
        let estado = match (self.ok, self.duracao_ms) {
            (Some(true), Some(ms)) => format!("ok {ms:>5}ms"),
            (Some(false), Some(ms)) => format!("ERRO {ms:>4}ms"),
            _ => "em curso   ".to_string(),
        };
        format!(
            "{} {:<15} {:<12} {:<20} {:<24} {:<9} {:>7}B  {}{}",
            phxsql_core::datahora::instante_iso(self.quando_ms),
            self.ip,
            if self.usuario.is_empty() {
                "-"
            } else {
                &self.usuario
            },
            self.op,
            if self.database.is_empty() {
                "-".to_string()
            } else if self.tabela.is_empty() {
                self.database.clone()
            } else {
                format!("{}.{}", self.database, self.tabela)
            },
            estado,
            self.bytes,
            self.pedido,
            if self.erro.is_empty() {
                String::new()
            } else {
                format!("  <- {}", self.erro)
            }
        )
    }
}

pub struct Profiler {
    ligado: bool,
    filtro: Filtro,
    anel: VecDeque<Evento>,
    teto: usize,
    proximo_serial: u64,
    /// Quantos passaram pelo filtro desde que ligou.
    observados: u64,
    /// Quantos sairam do anel por falta de espaco.
    esquecidos: u64,
    caminho: PathBuf,
    arquivo: Option<File>,
    ligado_em_ms: i64,
}

impl Default for Profiler {
    fn default() -> Self {
        Profiler {
            ligado: false,
            filtro: Filtro::default(),
            anel: VecDeque::new(),
            teto: 500,
            proximo_serial: 1,
            observados: 0,
            esquecidos: 0,
            caminho: PathBuf::new(),
            arquivo: None,
            ligado_em_ms: 0,
        }
    }
}

impl Profiler {
    pub fn ligado(&self) -> bool {
        self.ligado
    }

    pub fn caminho(&self) -> &std::path::Path {
        &self.caminho
    }

    pub fn filtro(&self) -> &Filtro {
        &self.filtro
    }

    pub fn observados(&self) -> u64 {
        self.observados
    }

    pub fn esquecidos(&self) -> u64 {
        self.esquecidos
    }

    pub fn teto(&self) -> usize {
        self.teto
    }

    pub fn ligado_em_ms(&self) -> i64 {
        self.ligado_em_ms
    }

    /// Liga a observacao. `arquivo` vazio deixa so o anel em memoria.
    ///
    /// Abre em modo APPEND: religar o profiler no mesmo arquivo continua o
    /// registro em vez de apagar o que ja estava la, que e o que alguem
    /// esperaria de um log.
    pub fn ligar(
        &mut self,
        filtro: Filtro,
        arquivo: &str,
        teto: usize,
        agora_ms: i64,
    ) -> Result<()> {
        self.arquivo = None;
        self.caminho = PathBuf::new();
        if !arquivo.trim().is_empty() {
            let caminho = PathBuf::from(arquivo.trim());
            if let Some(pai) = caminho.parent() {
                if !pai.as_os_str().is_empty() && !pai.exists() {
                    return Err(PhxError::NaoEncontrado(format!(
                        "o diretorio {} nao existe -- crie antes, ou escolha outro caminho",
                        pai.display()
                    )));
                }
            }
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&caminho)
                .map_err(|e| {
                    PhxError::Io(std::io::Error::other(format!(
                        "nao consegui abrir {}: {e}",
                        caminho.display()
                    )))
                })?;
            writeln!(
                f,
                "\n=== profiler ligado em {} === filtro: {}",
                phxsql_core::datahora::instante_iso(agora_ms),
                descrever(&filtro)
            )?;
            f.flush()?;
            self.arquivo = Some(f);
            self.caminho = caminho;
        }
        self.filtro = filtro;
        self.teto = teto.clamp(10, 20_000);
        self.anel.clear();
        self.observados = 0;
        self.esquecidos = 0;
        self.ligado = true;
        self.ligado_em_ms = agora_ms;
        Ok(())
    }

    pub fn desligar(&mut self, agora_ms: i64) {
        if let Some(f) = self.arquivo.as_mut() {
            let _ = writeln!(
                f,
                "=== profiler desligado em {} === {} evento(s)",
                phxsql_core::datahora::instante_iso(agora_ms),
                self.observados
            );
            let _ = f.flush();
        }
        self.ligado = false;
        self.arquivo = None;
    }

    pub fn limpar(&mut self) {
        self.anel.clear();
        self.esquecidos = 0;
    }

    pub fn eventos(&self, max: usize) -> Vec<Evento> {
        self.anel.iter().rev().take(max).cloned().collect()
    }

    /// Anota um pedido que ACABOU DE CHEGAR. Nada foi gravado ainda.
    ///
    /// Devolve o serial, para o resultado poder ser costurado depois. `None`
    /// quando o profiler esta desligado ou o filtro recusou.
    #[allow(clippy::too_many_arguments)]
    pub fn chegou(
        &mut self,
        linha_crua: &str,
        op: &str,
        usuario: &str,
        database: &str,
        tabela: &str,
        ip: &str,
        quando_ms: i64,
    ) -> Option<u64> {
        if !self.ligado || !self.filtro.aceita(op, usuario, database) {
            return None;
        }
        let serial = self.proximo_serial;
        self.proximo_serial += 1;
        self.observados += 1;

        let evento = Evento {
            serial,
            quando_ms,
            ip: ip.to_string(),
            usuario: usuario.to_string(),
            op: op.to_string(),
            database: database.to_string(),
            tabela: tabela.to_string(),
            bytes: linha_crua.len(),
            pedido: redigir(linha_crua),
            duracao_ms: None,
            ok: None,
            erro: String::new(),
        };
        if self.anel.len() >= self.teto {
            self.anel.pop_front();
            self.esquecidos += 1;
        }
        self.anel.push_back(evento);
        Some(serial)
    }

    /// Costura o resultado no evento, e so entao escreve a linha no arquivo.
    ///
    /// No arquivo escreve-se uma vez, no fim, com o tempo e o desfecho: duas
    /// linhas por pedido dobrariam o arquivo para repetir o que a primeira ja
    /// dizia. Na tela o evento aparece antes, com «em curso».
    pub fn terminou(&mut self, serial: u64, duracao_ms: u64, ok: bool, erro: &str) {
        let Some(e) = self.anel.iter_mut().find(|e| e.serial == serial) else {
            return;
        };
        e.duracao_ms = Some(duracao_ms);
        e.ok = Some(ok);
        e.erro = erro.to_string();
        let linha = e.linha();
        if let Some(f) = self.arquivo.as_mut() {
            let _ = writeln!(f, "{linha}");
            let _ = f.flush();
        }
    }
}

fn descrever(f: &Filtro) -> String {
    let mut p = Vec::new();
    if !f.database.is_empty() {
        p.push(format!("database={}", f.database));
    }
    if !f.usuario.is_empty() {
        p.push(format!("usuario={}", f.usuario));
    }
    if !f.op.is_empty() {
        p.push(format!("op={}", f.op));
    }
    if f.so_escrita {
        p.push("so escrita".into());
    }
    if p.is_empty() {
        "tudo".into()
    } else {
        p.join(", ")
    }
}

/// Troca por `"***"` o valor de todo campo sensivel, em qualquer profundidade.
///
/// Analisa e reserializa em vez de recortar o texto: recortar depende de o
/// pedido estar escrito de um jeito, e um pedido pode chegar com espaco entre
/// os dois-pontos, com a chave escapada, ou em qualquer ordem. Quem nao e JSON
/// valido nao vira texto -- vira o tamanho.
pub fn redigir(linha: &str) -> String {
    match Json::analisar(linha) {
        Ok(j) => limpar(&j).escrever(),
        Err(_) => format!("<pedido invalido, {} bytes>", linha.trim().len()),
    }
}

fn limpar(j: &Json) -> Json {
    match j {
        Json::Objeto(pares) => Json::Objeto(
            pares
                .iter()
                .map(|(k, v)| {
                    if SEGREDOS.iter().any(|s| k.eq_ignore_ascii_case(s)) {
                        (k.clone(), Json::Texto("***".into()))
                    } else {
                        (k.clone(), limpar(v))
                    }
                })
                .collect(),
        ),
        Json::Lista(itens) => Json::Lista(itens.iter().map(limpar).collect()),
        outro => outro.clone(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A regra do projeto, aplicada ao lugar onde ela seria mais facil de
    /// quebrar: senha nao aparece, em nenhum campo, em nenhuma profundidade.
    #[test]
    fn a_senha_nunca_aparece() {
        let pedidos = [
            r#"{"op":"login","usuario":"adm","senha":"segredo1"}"#,
            r#"{"op":"login","usuario":"adm","senha_b64":"c2VncmVkbzE="}"#,
            r#"{"op":"login","usuario":"adm","prova":"deadbeef","token":"segredo1"}"#,
            r#"{"op":"criar_usuario","usuario":{"login":"x","senha":"segredo1"}}"#,
            r#"{"op":"lote","linhas":[{"senha":"segredo1"},{"nome":"ok"}]}"#,
            r#"{ "op" : "login" , "senha" : "segredo1" }"#,
        ];
        for p in pedidos {
            let saida = redigir(p);
            assert!(
                !saida.contains("segredo1") && !saida.contains("c2VncmVkbzE="),
                "vazou em {p}\n  -> {saida}"
            );
            assert!(saida.contains("***"), "nao redigiu nada em {p}");
        }
    }

    /// Pedido que nao e JSON nao vira texto: vira o tamanho dele.
    ///
    /// Porque um pedido malformado pode ter uma senha dentro, e nao ha como
    /// achar o campo para tapar se a estrutura nao se le.
    #[test]
    fn pedido_invalido_nao_vira_texto() {
        let s = redigir("{\"op\":\"login\",\"senha\":\"segredo1\"");
        assert!(!s.contains("segredo1"), "vazou no pedido invalido: {s}");
        assert!(s.contains("bytes"), "{s}");
    }

    /// O que NAO e segredo continua legivel -- senao o profiler nao serviria.
    #[test]
    fn o_resto_do_pedido_continua_visivel() {
        let s = redigir(
            r#"{"op":"inserir","database":"loja","tabela":"clientes","linha":{"nome":"Adriano","cidade":"Blumenau"}}"#,
        );
        assert!(
            s.contains("Adriano") && s.contains("Blumenau") && s.contains("clientes"),
            "{s}"
        );
    }

    /// O profiler nao observa a si mesmo. Sem isto, a tela aberta enche o
    /// anel com as proprias perguntas e empurra para fora o que se procurava.
    #[test]
    fn a_leitura_do_profiler_nao_entra_no_anel() {
        let mut p = Profiler::default();
        p.ligar(Filtro::default(), "", 100, 0).unwrap();
        assert!(p.chegou("{}", "profiler", "adm", "", "", "ip", 0).is_none());
        assert!(p
            .chegou("{}", "profiler_ligar", "adm", "", "", "ip", 0)
            .is_some());
        assert!(p
            .chegou("{}", "profiler_desligar", "adm", "", "", "ip", 0)
            .is_some());
        assert_eq!(p.observados(), 2);
    }

    #[test]
    fn o_filtro_separa_por_banco_usuario_e_operacao() {
        let mut p = Profiler::default();
        p.ligar(
            Filtro {
                database: "loja".into(),
                usuario: "adm".into(),
                ..Default::default()
            },
            "",
            100,
            0,
        )
        .unwrap();
        assert!(p
            .chegou("{}", "inserir", "adm", "loja", "c", "1.1.1.1", 0)
            .is_some());
        assert!(p
            .chegou("{}", "inserir", "op", "loja", "c", "1.1.1.1", 0)
            .is_none());
        assert!(p
            .chegou("{}", "inserir", "adm", "outra", "c", "1.1.1.1", 0)
            .is_none());
        assert_eq!(p.observados(), 1);
    }

    #[test]
    fn so_escrita_deixa_a_leitura_de_fora() {
        let mut p = Profiler::default();
        p.ligar(
            Filtro {
                so_escrita: true,
                ..Default::default()
            },
            "",
            100,
            0,
        )
        .unwrap();
        assert!(p.chegou("{}", "inserir", "a", "d", "t", "ip", 0).is_some());
        assert!(p.chegou("{}", "varrer", "a", "d", "t", "ip", 0).is_none());
        assert!(p
            .chegou("{}", "atualizar", "a", "d", "t", "ip", 0)
            .is_some());
    }

    /// O anel nao cresce: um profiler esquecido ligado nao come a memoria.
    #[test]
    fn o_anel_esquece_o_mais_antigo() {
        let mut p = Profiler::default();
        p.ligar(Filtro::default(), "", 10, 0).unwrap();
        for i in 0..25 {
            p.chegou("{}", "varrer", "a", "d", "t", "ip", i);
        }
        assert_eq!(p.eventos(100).len(), 10, "o anel passou do teto");
        assert_eq!(p.observados(), 25);
        assert_eq!(p.esquecidos(), 15);
        // O mais recente vem primeiro.
        assert_eq!(p.eventos(1)[0].quando_ms, 24);
    }

    #[test]
    fn o_desfecho_e_costurado_no_evento() {
        let mut p = Profiler::default();
        p.ligar(Filtro::default(), "", 10, 0).unwrap();
        let s = p.chegou("{}", "inserir", "a", "d", "t", "ip", 0).unwrap();
        assert_eq!(p.eventos(1)[0].duracao_ms, None, "nasceu concluido");
        p.terminou(s, 42, false, "chave duplicada");
        let e = &p.eventos(1)[0];
        assert_eq!(e.duracao_ms, Some(42));
        assert_eq!(e.ok, Some(false));
        assert_eq!(e.erro, "chave duplicada");
    }

    /// Ligar num diretorio que nao existe recusa com o caminho na mensagem,
    /// em vez de aceitar e nunca escrever nada.
    #[test]
    fn arquivo_em_diretorio_inexistente_e_recusado() {
        let mut p = Profiler::default();
        let erro = p
            .ligar(Filtro::default(), "/nao/existe/mesmo/x.txt", 10, 0)
            .unwrap_err();
        assert!(erro.to_string().contains("nao existe"), "{erro}");
        assert!(!p.ligado());
    }
}
