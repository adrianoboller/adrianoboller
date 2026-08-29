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
//! ele esta, e a tela diz quanto ele ja tem.
//!
//! # Uma linha do arquivo e UMA linha
//!
//! O `pedido` sai seguro do `redigir`, porque JSON escapa a quebra de linha.
//! Os outros campos da linha nao passam por JSON nenhum -- `op`, `database` e
//! `tabela` vem do corpo do pedido, e o `erro` carrega texto que o cliente
//! influencia. Provado por soquete: um pedido com
//! `"op": "ping\n2000-01-01T00:00:00 9.9.9.9 forjado ..."` deixou no arquivo
//! uma segunda linha que se le como um evento de outro IP e de outro usuario.
//! Log de monitoracao que aceita linha forjada nao serve para investigar
//! nada, entao todo campo livre e reduzido a uma linha antes de entrar no
//! evento.
//!
//! # Gravacao que falha e gravacao que se conta
//!
//! Escrever no arquivo pode falhar -- disco cheio, cota, sistema de arquivos
//! que virou somente-leitura. Medido: num tmpfs de 64 KB, 400 pedidos
//! deixaram 223 linhas no arquivo e a resposta continuava dizendo «gravando
//! em ...», sem uma palavra sobre as 177 que se perderam. Log que promete
//! gravar e nao grava e pior que log nenhum, entao a falha e CONTADA e sai na
//! resposta.

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

/// Teto de um campo de identificacao na linha do arquivo.
///
/// `op`, `database`, `tabela` e `usuario` vem do pedido, e nada no protocolo
/// obriga que sejam curtos: um `"op"` de dez mil bytes vira uma linha de dez
/// mil bytes no arquivo de quem so queria ver o que estava chegando.
const TETO_DO_CAMPO: usize = 120;

/// Teto do texto de erro na linha do arquivo. Maior porque explicacao boa e
/// comprida -- a do `sql` sem indice passa de 200 caracteres.
const TETO_DO_ERRO: usize = 500;

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
    /// Bytes ja escritos no arquivo desde que ligou.
    ///
    /// Contados aqui e nao perguntados ao sistema de arquivos: o `profiler` e
    /// pedido uma vez por segundo pela tela, e um `metadata()` por volta seria
    /// uma ida ao disco para dizer o que a soma ja sabe. Serve para quem
    /// esqueceu o profiler ligado ver o arquivo crescendo -- 345 B por pedido,
    /// medido -- antes de ele comer a particao.
    gravados: u64,
    /// Linhas que o arquivo RECUSOU: disco cheio, cota, sistema de arquivos
    /// somente-leitura, arquivo removido debaixo do descritor.
    falhas_de_escrita: u64,
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
            gravados: 0,
            falhas_de_escrita: 0,
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

    pub fn gravados(&self) -> u64 {
        self.gravados
    }

    pub fn falhas_de_escrita(&self) -> u64 {
        self.falhas_de_escrita
    }

    /// Escreve uma linha no arquivo, e CONTA o que aconteceu.
    ///
    /// O `let _ = writeln!` de antes engolia a falha: com a particao cheia o
    /// anel seguia enchendo, a resposta seguia dizendo «gravando em ...», e as
    /// linhas iam para o chao sem ninguem saber. Contar nao conserta o disco,
    /// mas troca um log que mente por um log que avisa.
    fn escrever_linha(&mut self, linha: &str) {
        let resultado = match self.arquivo.as_mut() {
            None => return,
            Some(f) => writeln!(f, "{linha}").and_then(|_| f.flush()),
        };
        match resultado {
            Ok(()) => self.gravados += linha.len() as u64 + 1,
            Err(_) => self.falhas_de_escrita += 1,
        }
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
        self.gravados = 0;
        self.falhas_de_escrita = 0;
        self.ligado = true;
        self.ligado_em_ms = agora_ms;
        Ok(())
    }

    pub fn desligar(&mut self, agora_ms: i64) {
        let rodape = format!(
            "=== profiler desligado em {} === {} evento(s){}",
            phxsql_core::datahora::instante_iso(agora_ms),
            self.observados,
            match self.falhas_de_escrita {
                0 => String::new(),
                n => format!(", {n} linha(s) NAO gravada(s)"),
            }
        );
        self.escrever_linha(&rodape);
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

        // Todo campo livre entra JA reduzido a uma linha: assim o anel e o
        // arquivo contam a mesma historia, e a linha forjada morre no
        // nascimento em vez de morrer na hora de escrever -- que e onde
        // alguem esqueceria de matar.
        let evento = Evento {
            serial,
            quando_ms,
            ip: de_uma_linha(ip, TETO_DO_CAMPO),
            usuario: de_uma_linha(usuario, TETO_DO_CAMPO),
            op: de_uma_linha(op, TETO_DO_CAMPO),
            database: de_uma_linha(database, TETO_DO_CAMPO),
            tabela: de_uma_linha(tabela, TETO_DO_CAMPO),
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
        let linha = {
            // `.rev()`: o evento procurado e quase sempre o ULTIMO que
            // entrou -- `chegou` empurra atras e o desfecho chega logo
            // depois. Procurando do mais antigo, cada pedido varria o anel
            // inteiro: com `guardar: 20000` sao 20.000 comparacoes para achar
            // o que esta na ponta. Medido emparelhado, na carga uma a uma:
            // **1,17x** com o anel em 20.000, e 1,00x com o anel padrao de
            // 500 -- ou seja, a instrumentacao ficava mais cara justamente
            // para quem dava mais memoria a ela. `bancada/profiler/custo-anel.py`.
            let Some(e) = self.anel.iter_mut().rev().find(|e| e.serial == serial) else {
                return;
            };
            e.duracao_ms = Some(duracao_ms);
            e.ok = Some(ok);
            e.erro = de_uma_linha(erro, TETO_DO_ERRO);
            e.linha()
        };
        self.escrever_linha(&linha);
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

/// Reduz um campo livre a UMA linha, com teto de tamanho.
///
/// Controle vira a forma escapada -- `\n`, `\r`, `\t`, `\xNN` -- em vez de
/// sumir: quem investiga precisa saber que o pedido trazia aquilo, e apagar
/// em silencio esconderia justamente a tentativa. O corte respeita a fronteira
/// de caractere, porque o nome da tabela pode ter acento.
fn de_uma_linha(s: &str, teto: usize) -> String {
    let mut saida = String::with_capacity(s.len());
    for c in s.chars().take(teto) {
        match c {
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                saida.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => saida.push(c),
        }
    }
    // O aviso de corte so sai quando houve corte de verdade: um `chars().count()`
    // adiantado percorreria toda entrada, inclusive as curtas, que sao a
    // maioria absoluta.
    if s.chars().nth(teto).is_some() {
        saida.push_str("...");
    }
    saida
}

/// Troca por `"***"` o valor de todo campo sensivel, em qualquer profundidade.
///
/// Analisa e reserializa em vez de recortar o texto: recortar depende de o
/// pedido estar escrito de um jeito, e um pedido pode chegar com espaco entre
/// os dois-pontos, com a chave escapada, ou em qualquer ordem. Quem nao e JSON
/// valido nao vira texto -- vira o tamanho.
///
/// E o que e JSON valido mas NAO e objeto tambem vira o tamanho, pelo mesmo
/// motivo. A redacao e por NOME de campo; um topo que nao tem campo nenhum --
/// `["op","senha","..."]` -- nao tem nome para tapar, e mostrar o texto seria
/// mostrar o que estivesse la dentro. O protocolo so aceita objeto no topo,
/// entao nao se perde pedido legitimo nenhum: perde-se so o lixo, e o lixo e
/// exatamente onde a senha apareceria por engano.
pub fn redigir(linha: &str) -> String {
    let tamanho = linha.trim().len();
    match Json::analisar(linha) {
        Ok(j @ Json::Objeto(_)) => limpar(&j).escrever(),
        Ok(_) => format!("<pedido nao e objeto, {tamanho} bytes>"),
        Err(_) => format!("<pedido invalido, {tamanho} bytes>"),
    }
}

fn limpar(j: &Json) -> Json {
    match j {
        Json::Objeto(pares) => Json::Objeto(
            pares
                .iter()
                .map(|(k, v)| {
                    // `k.trim()`: a chave `"senha "` -- com espaco DENTRO das
                    // aspas -- nao e a chave que o servidor le, entao ela
                    // nunca autentica ninguem; mas um cliente desastrado que a
                    // mande poe uma senha de verdade no fio, e o profiler a
                    // mostraria inteira. Comparar aparado nao perde nada e
                    // fecha a porta.
                    if SEGREDOS.iter().any(|s| k.trim().eq_ignore_ascii_case(s)) {
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

    // ------------------------------------------------------- os torcidos
    //
    // Os seis casos abaixo sao os que separam ANALISAR de RECORTAR. Todos
    // passam pelo `redigir`, e todos falham se alguem trocar a analise por
    // um `find("\"senha\"")` e um corte -- que e exatamente a tentacao, porque
    // recortar parece mais barato.

    /// A chave escapada em `\u`: `senha` E `senha` depois do analisador,
    /// e nao e antes dele. Recorte nenhum acha isto.
    #[test]
    fn chave_escapada_em_unicode_tambem_e_senha() {
        // A chave chega escrita `\u0073enha`, e nao `senha`.
        let s = redigir(r#"{"op":"login","usuario":"adm","\u0073enha":"segredo1"}"#);
        assert!(!s.contains("segredo1"), "vazou: {s}");
        assert!(s.contains("***"), "{s}");
    }

    /// Chave com espaco DENTRO das aspas. O servidor nao le `"senha "`, entao
    /// ela nunca autentica ninguem -- mas um cliente desastrado que a mande
    /// poe uma senha de verdade no fio, e mostra-la seria mostrar a senha.
    #[test]
    fn chave_com_espaco_no_nome_ainda_e_senha() {
        for p in [
            r#"{"op":"ping","senha ":"segredo1"}"#,
            r#"{"op":"ping"," senha":"segredo1"}"#,
            "{\"op\":\"ping\",\"senha\\n\":\"segredo1\"}",
        ] {
            let s = redigir(p);
            assert!(!s.contains("segredo1"), "vazou em {p} -> {s}");
        }
    }

    /// Aspas escapadas DENTRO de um valor: o texto do campo `obs` contem
    /// `"senha":"..."` escrito por um humano. Isto e DADO, e tem de continuar
    /// visivel -- e o mesmo caso que faz o recorte errar para o outro lado,
    /// tapando o que nao era segredo.
    #[test]
    fn aspas_escapadas_dentro_de_um_valor_nao_confundem() {
        let p = r#"{"op":"inserir","linha":{"obs":"ele disse \"senha\":\"abc\" no chat"}}"#;
        let s = redigir(p);
        assert!(s.contains("ele disse"), "sumiu o dado: {s}");
        assert!(!s.contains("***"), "tapou o que nao era segredo: {s}");
    }

    /// Lote grande: a senha esta na linha 4.999 de 5.000, e nao na primeira.
    #[test]
    fn lote_grande_e_redigido_ate_a_ultima_linha() {
        let mut corpo = String::from(r#"{"op":"inserir_lote","tabela":"t","linhas":["#);
        for i in 0..5_000 {
            if i > 0 {
                corpo.push(',');
            }
            corpo.push_str(&format!(r#"{{"id":{i},"senha":"segredo{i}"}}"#));
        }
        corpo.push_str("]}");
        let s = redigir(&corpo);
        assert!(!s.contains("segredo4999"), "vazou a ultima linha do lote");
        assert!(!s.contains("segredo0"), "vazou a primeira linha do lote");
        assert_eq!(s.matches("***").count(), 5_000, "nao redigiu todas");
    }

    /// JSON valido que NAO e objeto nao tem campo para tapar -- entao nao
    /// vira texto. O protocolo so aceita objeto no topo, entao nao se perde
    /// pedido legitimo nenhum.
    #[test]
    fn topo_que_nao_e_objeto_nao_vira_texto() {
        for p in [
            r#"["op","senha","segredo1"]"#,
            r#""senha=segredo1""#,
            r#"[{"senha":"segredo1"}]"#,
        ] {
            let s = redigir(p);
            assert!(!s.contains("segredo1"), "vazou em {p} -> {s}");
            assert!(s.contains("bytes"), "{s}");
        }
    }

    /// Corpo que nao e JSON nenhum -- um `GET /` que caiu na porta de dados.
    #[test]
    fn corpo_que_nao_e_json_vira_o_tamanho() {
        let s = redigir("GET /?senha=segredo1 HTTP/1.1");
        assert!(!s.contains("segredo1"), "{s}");
        assert!(s.contains("bytes"), "{s}");
    }

    // ------------------------------------------------- o arquivo em texto

    /// Uma linha do arquivo e UMA linha.
    ///
    /// Provado por soquete antes de virar teste: um pedido com uma quebra de
    /// linha no nome da `op` deixou no .txt uma segunda linha que se le como
    /// um evento de outro IP e de outro usuario. Quem investiga um incidente
    /// no arquivo estaria lendo o que o suspeito escreveu.
    #[test]
    fn quebra_de_linha_no_pedido_nao_forja_linha_no_arquivo() {
        let mut p = Profiler::default();
        p.ligar(Filtro::default(), "", 10, 0).unwrap();
        let forjada = "ping\n2000-01-01T00:00:00 9.9.9.9 forjado ping - ok 0ms 0B {}";
        let s = p
            .chegou("{}", forjada, "adm", "d\nFORJADO", "t\rFORJADO", "ip", 0)
            .unwrap();
        p.terminou(s, 1, false, "erro\ncom quebra");
        let linha = p.eventos(1)[0].linha();
        assert_eq!(linha.lines().count(), 1, "a linha virou duas: {linha}");
        assert!(
            linha.contains("\\n"),
            "engoliu a quebra em vez de mostra-la"
        );
    }

    /// Campo gigante nao vira linha gigante no arquivo de quem so queria ver
    /// o que estava chegando.
    #[test]
    fn campo_gigante_e_cortado_na_linha() {
        let mut p = Profiler::default();
        p.ligar(Filtro::default(), "", 10, 0).unwrap();
        let enorme = "a".repeat(10_000);
        p.chegou("{}", &enorme, "adm", "", "", "ip", 0).unwrap();
        let e = &p.eventos(1)[0];
        assert!(e.op.chars().count() <= TETO_DO_CAMPO + 3, "{}", e.op.len());
        assert!(e.op.ends_with("..."), "cortou sem dizer que cortou");
    }

    /// Gravacao que falha e CONTADA. `/dev/full` aceita a abertura e recusa
    /// toda escrita com ENOSPC -- e o disco cheio de verdade, sem esperar o
    /// disco encher.
    #[test]
    #[cfg(unix)]
    fn linha_que_o_disco_recusa_e_contada() {
        let mut p = Profiler::default();
        // O cabecalho do `ligar` ja falha em /dev/full, e falhar cedo e o
        // certo: quem escolheu um destino que nao aceita escrita descobre na
        // hora. Entao o teste liga em memoria e troca o arquivo depois --
        // que e o caso de verdade: o disco enche DEPOIS.
        p.ligar(Filtro::default(), "", 10, 0).unwrap();
        p.arquivo = OpenOptions::new().append(true).open("/dev/full").ok();
        assert!(p.arquivo.is_some(), "sem /dev/full nao da para provar");
        let s = p.chegou("{}", "inserir", "a", "d", "t", "ip", 0).unwrap();
        p.terminou(s, 1, true, "");
        assert_eq!(p.falhas_de_escrita(), 1, "engoliu a falha de escrita");
        assert_eq!(p.gravados(), 0, "contou como gravado o que nao gravou");
    }

    /// E a gravacao que dá certo e contada em bytes, para a tela poder dizer
    /// o tamanho do arquivo sem ir ao disco perguntar.
    #[test]
    fn o_que_grava_conta_os_bytes() {
        let d = std::env::temp_dir().join(format!("phx-prof-bytes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let alvo = d.join("p.txt");
        let mut p = Profiler::default();
        p.ligar(Filtro::default(), alvo.to_str().unwrap(), 10, 0)
            .unwrap();
        let s = p.chegou("{}", "inserir", "a", "d", "t", "ip", 0).unwrap();
        p.terminou(s, 1, true, "");
        assert!(p.gravados() > 0, "nao contou nada");
        assert_eq!(p.falhas_de_escrita(), 0);
        p.desligar(1);
        let texto = std::fs::read_to_string(&alvo).unwrap();
        assert!(texto.contains("inserir"), "{texto}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
