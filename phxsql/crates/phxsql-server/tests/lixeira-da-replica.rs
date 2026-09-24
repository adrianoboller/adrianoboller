//! A lixeira da replica -- pedido 499, medido PELO SOQUETE entre um source e
//! uma replica de verdade (papel `replica`, `somente_leitura` ligado, que e
//! como uma replica roda por desenho).
//!
//! # O que se mede, e por que precisa dos dois processos
//!
//! A suspeita do papel C era de alcance: a replica aplica a exclusao do
//! source pelo MESMO `excluir_de_vez` de sempre (`Table::aplicar_evento`), e
//! ele guarda a linha inteira no `.trash` DELA -- mas o `esvaziar_lixeira` do
//! source nao escreve evento no diario, entao nao chega aqui; e o daqui estava
//! no `OPS_ESCRITA`, que a replica recusa. O dado apagado ficaria no `.trash`
//! da replica para sempre, sem porta nenhuma para tira-lo. Teste unitario nao
//! mostra isso: e o encontro do laco da replica com o portao de escrita, e so
//! os dois processos conversando o reproduzem.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "lixeira-da-replica";

/// O valor da linha apagada. E o que o `.trash` da replica guarda -- e o que
/// nao pode ficar la sem ninguem conseguir tirar.
const CPF: &str = "999.888.777-66";

fn config_base(base: &std::path::Path) -> Config {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // O mesmo escape escrito do `continuidade-da-replica.rs`: a cifra do fio
    // nasce exigida (pedido 370), e o que esta bateria mede e o portao de
    // escrita, nao a cifra.
    c.cifra_fio.exigir = false;
    c.web.ligado = false;
    c
}

fn subir(c: Config) -> (Arc<Servidor>, u16) {
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    esperar_porta(porta);
    (s, porta)
}

fn esperar_porta(porta: u16) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("a porta {porta} nao abriu em 5 s");
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("nao conectei em {porta}: {e}"));
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    Json::analisar(&resposta)
        .unwrap_or_else(|e| panic!("{corpo}: resposta ilegivel ({e}): {resposta}"))
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo);
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

fn eventos(porta: u16) -> i64 {
    let r = pedir(porta, r#""op":"posicao","database":"loja""#);
    r.campo("resultado")
        .and_then(|res| res.campo("tabelas"))
        .and_then(|t| t.campo("clientes"))
        .map(|c| c.inteiro_ou("eventos", -1))
        .unwrap_or(-1)
}

fn esperar_eventos(porta: u16, quantos: i64) {
    let ate = Instant::now() + Duration::from_secs(20);
    while Instant::now() < ate {
        if eventos(porta) == quantos {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "a replica nao chegou a {quantos} evento(s) em 20 s (esta em {})",
        eventos(porta)
    );
}

/// Quantas linhas o `.trash` deste servidor guarda, e o texto da resposta.
fn na_lixeira(porta: u16) -> (i64, String) {
    let r = exigir(
        porta,
        r#""op":"lixeira","database":"loja","tabela":"clientes""#,
    );
    (r.inteiro_ou("total", -1), r.escrever())
}

const ESVAZIAR: &str =
    r#""op":"esvaziar_lixeira","database":"loja","tabela":"clientes","motivo":"expurgo do teste""#;

/// **A premissa, medida, e o conserto, na mesma corrida.**
///
/// 1. o source apaga de vez uma linha, e a replica a aplica: o `.trash` DELA
///    passa a guardar a linha inteira, com o CPF dentro;
/// 2. o source esvazia a lixeira DELE e o diario nao ganha evento nenhum --
///    nada vai para a replica, e o `.trash` de la continua com a linha;
/// 3. o administrador da replica esvazia a lixeira da replica. Antes do
///    conserto a resposta era a recusa do somente-leitura, e a linha ficava
///    la para sempre; agora a op mexe so no no local, como o `expurgar_trilha`
///    do C2 do 368.
#[test]
fn a_replica_esvazia_a_propria_lixeira() {
    let base_s = DirTemp::novo("lixeira-source");
    let base_r = DirTemp::novo("lixeira-replica");

    let mut c = config_base(&base_s);
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "source-da-lixeira".into();
    c.replicacao.imagem_da_linha = true;
    let (_source, porta_s) = subir(c);
    exigir(porta_s, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta_s,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"cpf","tipo":"Str(14)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    for id in 1..=3 {
        exigir(
            porta_s,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id},"cpf":"{CPF}"}}"#
            ),
        );
    }

    let mut c = config_base(&base_r);
    c.replicacao.papel = Papel::Replica;
    c.replicacao.id_servidor = "replica-da-lixeira".into();
    c.somente_leitura = true;
    c.replicacao.origens = vec![Origem {
        nome: "fonte".into(),
        host: "127.0.0.1".into(),
        porta: porta_s,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
    }];
    let (_replica, porta_r) = subir(c);
    esperar_eventos(porta_r, 3);

    // (1) a exclusao de vez chega, e a replica guarda a linha no `.trash`.
    exigir(
        porta_s,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":2,
           "fisico":true,"motivo":"pedido do titular""#,
    );
    esperar_eventos(porta_r, 4);
    let (total, texto) = na_lixeira(porta_r);
    assert_eq!(total, 1, "a replica nao guardou a linha apagada: {texto}");
    assert!(texto.contains(CPF), "a premissa caiu: {texto}");

    // (2) o esvaziar do source nao vira evento, e nao chega a replica.
    let r = exigir(porta_s, ESVAZIAR);
    assert_eq!(r.inteiro_ou("apagadas", -1), 1, "{}", r.escrever());
    assert_eq!(na_lixeira(porta_s).0, 0);
    assert_eq!(eventos(porta_s), 4, "o esvaziar do source virou evento");
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        na_lixeira(porta_r).0,
        1,
        "o `.trash` da replica mudou sem evento nenhum -- a premissa caiu"
    );

    // (3) o administrador da replica esvazia a lixeira DA REPLICA.
    let r = pedir(porta_r, ESVAZIAR);
    assert!(
        r.booleano_ou("ok", false),
        "a replica recusou esvaziar o proprio `.trash`, e a linha apagada no \
         source fica aqui para sempre: {}",
        r.escrever()
    );
    let (total, texto) = na_lixeira(porta_r);
    assert_eq!(total, 0, "{texto}");
    assert!(!texto.contains(CPF), "{texto}");
    // O dado vivo da replica nao foi tocado: o esvaziar e do `.trash`.
    assert_eq!(eventos(porta_r), 4);
}

/// **O controle: o somente-leitura continua recusando o que grava DADO.**
///
/// Tirar o `esvaziar_lixeira` do `OPS_ESCRITA` nao pode abrir a porta das
/// outras: num servidor somente-leitura o `inserir` e o `restaurar` seguem
/// recusados. Sem este teste, um conserto que esvaziasse a lista inteira
/// passaria no de cima.
#[test]
fn o_somente_leitura_continua_recusando_o_que_grava_dado() {
    let base = DirTemp::novo("lixeira-controle");
    {
        let (_s, porta) = subir(config_base(&base));
        exigir(porta, r#""op":"criar_database","database":"loja""#);
        exigir(
            porta,
            r#""op":"criar_tabela","database":"loja","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
        );
    }
    let mut c = config_base(&base);
    c.somente_leitura = true;
    let (_s, porta) = subir(c);
    for corpo in [
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1}"#,
        r#""op":"restaurar","database":"loja","tabela":"clientes","rowid":1"#,
    ] {
        let r = pedir(porta, corpo);
        assert!(
            !r.booleano_ou("ok", true) && r.texto_ou("erro", "").contains("somente leitura"),
            "{corpo} -> {}",
            r.escrever()
        );
    }
}

/// Uma conexao que FICA aberta: a transacao mora na ligacao, e o `pedir` de
/// cima abre uma por pedido.
struct Conexao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Conexao {
    fn nova(porta: u16) -> Conexao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Conexao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn pedir(&mut self, corpo: &str) -> Json {
        writeln!(
            self.escrita,
            "{{\"token\":\"{TOKEN}\",{}}}",
            corpo.replace('\n', " ")
        )
        .unwrap();
        let mut resposta = String::new();
        self.leitor.read_line(&mut resposta).unwrap();
        Json::analisar(&resposta).unwrap()
    }
}

/// Um servidor sozinho com `loja.clientes`, quatro linhas e a de rowid 2 ja
/// apagada de vez -- uma linha na lixeira.
fn com_uma_na_lixeira(base: &std::path::Path) -> (Arc<Servidor>, u16) {
    let (s, porta) = subir(config_base(base));
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    for id in 1..=4 {
        exigir(
            porta,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id}}}"#
            ),
        );
    }
    exigir(
        porta,
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":2,
           "fisico":true,"motivo":"pedido do titular""#,
    );
    assert_eq!(na_lixeira(porta).0, 1, "a premissa caiu");
    (s, porta)
}

/// As duas ops do no, com o que cada uma pede para rodar.
fn as_do_no() -> [(&'static str, String); 2] {
    [
        ("esvaziar_lixeira", ESVAZIAR.to_string()),
        (
            "expurgar_trilha",
            format!(
                r#""op":"expurgar_trilha","database":"loja","tabela":"clientes",
                   "motivo":"expurgo do teste","ate_ms":{},"fechar_ativo":true"#,
                phxsql_server::agora_ms() + 1
            ),
        ),
    ]
}

/// **B2 do parecer do DBA (pedido 499): o que so mexe no no CONTINUA sendo
/// escrita para a transacao.**
///
/// A primeira versao do 499 tirou o `esvaziar_lixeira` do `OPS_ESCRITA`, e
/// ele deixou de ser recusado dentro da transacao: `BEGIN; esvaziar_lixeira;
/// ROLLBACK` esvaziava, e a lixeira nao voltava -- medido pelo papel C. O
/// `expurgar_trilha` tinha o mesmo furo desde o 368. As duas agora recusam
/// dentro do BEGIN, como toda escrita que a transacao nao empilha, e a
/// lixeira continua com a linha depois do ROLLBACK.
#[test]
fn as_ops_do_no_nao_entram_em_transacao() {
    let base = DirTemp::novo("lixeira-transacao");
    let (_s, porta) = com_uma_na_lixeira(&base);
    let mut a = Conexao::nova(porta);
    for (op, corpo) in as_do_no() {
        let r = a.pedir(r#""op":"begin""#);
        assert!(r.booleano_ou("ok", false), "{}", r.escrever());
        let r = a.pedir(&corpo);
        assert!(
            !r.booleano_ou("ok", true) && r.texto_ou("erro", "").contains("nao entra em transacao"),
            "{op} rodou dentro da transacao, e o ROLLBACK nao o desfaz: {}",
            r.escrever()
        );
        let r = a.pedir(r#""op":"rollback""#);
        assert!(r.booleano_ou("ok", false), "{}", r.escrever());
        assert_eq!(na_lixeira(porta).0, 1, "{op}: a lixeira mudou");
    }
}

/// **B2, a segunda regressao: a trava de OUTRA transacao.** Com a B
/// segurando a tabela (um `inserir` empilhado), o esvaziar da A passava por
/// cima -- medido pelo papel C. Agora as duas ops do no esbarram na trava,
/// como toda escrita comum, e rodam depois que a B solta.
#[test]
fn as_ops_do_no_esbarram_na_trava_de_outra_transacao() {
    let base = DirTemp::novo("lixeira-trava");
    let (_s, porta) = com_uma_na_lixeira(&base);
    let mut b = Conexao::nova(porta);
    let r = b.pedir(r#""op":"begin""#);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    let r = b.pedir(r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":9}"#);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    for (op, corpo) in as_do_no() {
        let r = pedir(porta, &corpo);
        assert!(
            !r.booleano_ou("ok", true) && r.texto_ou("nome", "") == "EM_TRANSACAO",
            "{op} passou por cima da trava da outra transacao: {}",
            r.escrever()
        );
    }
    assert_eq!(na_lixeira(porta).0, 1);
    let r = b.pedir(r#""op":"rollback""#);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    // Solta a trava, o esvaziar passa: o portao barrou pela trava, e nao por
    // outro motivo.
    let r = exigir(porta, ESVAZIAR);
    assert_eq!(r.inteiro_ou("apagadas", -1), 1, "{}", r.escrever());
}
