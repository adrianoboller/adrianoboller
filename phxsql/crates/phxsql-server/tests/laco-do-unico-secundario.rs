//! O laco do bidirecional nao para numa chave duplicada -- provado PELO
//! SOQUETE, com dois servidores no ar (pedido 292, parte 2).
//!
//! O casamento entre servidores usa UMA chave (`bidirecional::chave_unica`), e
//! a unicidade dos OUTROS indices continua sendo conferida na gravacao -- e
//! esta certo que continue, porque violacao de indice unico nao se cura quando
//! o proximo lote chega, ao contrario da chave estrangeira. Com primaria
//! `porId` e um secundario `porEmail`, o evento do outro lado com um e-mail
//! que ja existe aqui e recusado -- e a recusa subia pelo `?` do laco: a
//! posicao consumida nunca andava, e o MESMO lote voltava para sempre. Nao e
//! uma linha perdida: e o par de servidores parado, sem ninguem saber.
//!
//! Teste unitario nao prova isto, e o motivo e o de sempre nesta casa: o que
//! se quer saber nao e se `aplicar_por_chave` devolve `Ok` ou `Err`, e se a
//! PROXIMA linha -- a que nada tem a ver com o conflito -- chega do outro
//! lado. Isso so os dois lacos rodando de verdade mostram. A unidade esta em
//! `servidor.rs`, no modulo `testes_da_recusa_por_unicidade`.
//!
//! **Por que so um lado puxa.** O defeito mora em quem PUXA, e os dois lados
//! de um multi puxam -- mas montar o conflito com os dois puxando e uma
//! corrida: o parceiro receberia a linha daqui antes de gravar a dele, e o
//! `inserir` do teste e que seria recusado. Entao o parceiro fica sem origem
//! configurada, e o cenario nasce igual toda vez.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "unico-secundario";
/// Quanto esperar por um laco que roda a cada segundo. O mesmo teto dos
/// outros testes de replicacao desta pasta.
const ESPERA: Duration = Duration::from_secs(20);

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn config_base(base: &std::path::Path, porta: u16, id: &str) -> Config {
    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    c.web.ligado = false;
    c.replicacao.papel = Papel::Multi;
    c.replicacao.id_servidor = id.into();
    // No multi a chave mora dentro da imagem, dos dois lados.
    c.replicacao.imagem_da_linha = true;
    c
}

fn subir(c: Config, porta: u16) -> Arc<Servidor> {
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar_porta(porta);
    s
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

/// Um pedido pela porta de dados; devolve o JSON da resposta inteira.
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

/// A tabela do cenario: primaria `porId` e um unico SECUNDARIO `porEmail` --
/// chave primaria + e-mail unico e a modelagem mais comum que existe.
fn criar_clientes(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"email","tipo":"Str(30)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                      {"nome":"porEmail","colunas":["email"],"unico":true}]"#,
    );
}

fn inserir(porta: u16, id: i64, email: &str) {
    exigir(
        porta,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{{"id":{id},"email":"{email}"}}"#
        ),
    );
}

fn linhas(porta: u16) -> Vec<(i64, String)> {
    exigir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    )
    .campo("linhas")
    .and_then(Json::lista)
    .unwrap_or(&[])
    .iter()
    .map(|l| (l.inteiro_ou("id", -1), l.texto_ou("email", "").to_string()))
    .collect()
}

fn estado(porta: u16) -> Json {
    exigir(porta, r#""op":"replicacao_estado""#)
}

/// Quantas recusas por unicidade este servidor ja contou em `loja/clientes`.
fn recusas(porta: u16) -> i64 {
    estado(porta)
        .campo("recusas_por_unicidade")
        .and_then(|r| r.campo("loja/clientes"))
        .and_then(Json::inteiro)
        .unwrap_or(0)
}

/// Ate onde o laco ja consumiu o diario do parceiro. E o numero que ficava
/// parado: `desde = lote.ate` so roda depois do lote inteiro aplicado.
fn posicao(porta: u16) -> i64 {
    estado(porta)
        .campo("origens")
        .and_then(|o| o.campo("parceiro"))
        .and_then(|p| p.campo("posicoes"))
        .and_then(|p| p.campo("loja/clientes"))
        .and_then(Json::inteiro)
        .unwrap_or(-1)
}

fn esperar<F: Fn() -> bool>(o_que: &str, f: F) {
    let ate = Instant::now() + ESPERA;
    while Instant::now() < ate {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{o_que} nao aconteceu em {} s", ESPERA.as_secs());
}

/// Sobe os dois: o que PUXA (com o parceiro como origem) e o parceiro, sem
/// origem nenhuma. O que puxa sobe primeiro, de proposito -- a porta do
/// parceiro ainda esta fechada, o laco tenta, falha e volta a tentar, e e
/// nessa janela que o dado local deste lado nasce sem corrida.
fn terreno(nome: &str) -> (Arc<Servidor>, Arc<Servidor>, u16, u16, DirTemp, DirTemp) {
    let dir_a = DirTemp::novo(&format!("unico-secundario-puxa-{nome}"));
    let dir_b = DirTemp::novo(&format!("unico-secundario-parceiro-{nome}"));
    let porta_a = porta_livre();
    let porta_b = porta_livre();

    let mut c = config_base(&dir_a, porta_a, "alfa");
    c.replicacao.origens = vec![Origem {
        nome: "parceiro".into(),
        host: "127.0.0.1".into(),
        porta: porta_b,
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
    let a = subir(c, porta_a);
    criar_clientes(porta_a);
    inserir(porta_a, 1, "a@x");

    let b = subir(config_base(&dir_b, porta_b, "beta"), porta_b);
    criar_clientes(porta_b);
    (a, b, porta_a, porta_b, dir_a, dir_b)
}

/// **Prova real.** O parceiro cria a linha 2 com o e-mail que a linha 1 daqui
/// ja ocupa: o `porEmail` recusa o evento, a recusa e CONTADA, a posicao anda,
/// e a linha seguinte -- que nada tem a ver com o conflito -- chega.
///
/// **Defeito reposto**: trocar o bloco da recusa por `escrita?;` em
/// `aplicar_por_chave`. A posicao fica em 0, a recusa nao aparece em
/// `replicacao_estado`, e a linha 3 nunca chega -- o par parado.
#[test]
fn chave_duplicada_no_unico_secundario_nao_prende_o_laco() {
    let (_a, _b, porta_a, porta_b, _da, _db) = terreno("segue");
    inserir(porta_b, 2, "a@x");

    esperar("a recusa por unicidade", || recusas(porta_a) >= 1);
    assert_eq!(
        posicao(porta_a),
        1,
        "a posicao nao andou: o mesmo lote vai voltar para sempre"
    );

    // A linha que prova que o laco seguiu: ela vem DEPOIS da recusada, no
    // mesmo diario, e com o `?` de antes ela nunca chegaria.
    inserir(porta_b, 3, "c@x");
    esperar("a linha 3 chegar", || {
        linhas(porta_a).iter().any(|(id, _)| *id == 3)
    });

    // O dado deste lado ficou como estava -- a recusa nao apaga nem sobrescreve
    // a linha que ocupa o e-mail --, e o contador nao inventou recusa a mais.
    assert_eq!(
        linhas(porta_a),
        vec![(1, "a@x".to_string()), (3, "c@x".to_string())]
    );
    assert_eq!(recusas(porta_a), 1, "uma recusa, e uma so");
    assert_eq!(posicao(porta_a), 2, "a posicao parou antes do fim");
}

/// O comportamento VELHO, que nao pode mudar: sem colisao nenhuma o laco
/// replica como sempre replicou, e `recusas_por_unicidade` nem aparece na
/// resposta -- campo que aparece vazio em toda instalacao sa e campo que
/// ninguem olha quando enche.
#[test]
fn sem_colisao_o_laco_replica_como_sempre_e_nada_e_contado() {
    let (_a, _b, porta_a, porta_b, _da, _db) = terreno("limpo");
    inserir(porta_b, 2, "b@x");
    inserir(porta_b, 3, "c@x");

    esperar("as duas linhas do parceiro", || linhas(porta_a).len() == 3);
    assert_eq!(
        linhas(porta_a),
        vec![
            (1, "a@x".to_string()),
            (2, "b@x".to_string()),
            (3, "c@x".to_string())
        ]
    );
    assert!(
        estado(porta_a)
            .campo("recusas_por_unicidade")
            .and_then(|r| r.campo("loja/clientes"))
            .is_none(),
        "tabela sem recusa apareceu no contador: {}",
        estado(porta_a).escrever()
    );
}
