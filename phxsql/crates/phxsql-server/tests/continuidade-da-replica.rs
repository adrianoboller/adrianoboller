//! A continuidade do diario -- provada PELO SOQUETE, entre um source e uma
//! replica de verdade.
//!
//! Papel C, 17/09/2026, §2.6.1: `excluir_tabela` no source leva o `.log`
//! junto, o diario de la volta a zero, e a replica -- com N eventos de outra
//! vida -- nao fazia nada, para sempre, sem reclamar (`alcancar_tabela` tinha
//! `if posicao >= no.eventos { return Ok(0) }`). Pior: se o source recriado
//! passasse de N, a replica aplicava os eventos da vida nova em cima da
//! velha, porque os rowids coincidiam. O PITR ja fazia a pergunta certa
//! (`diario_vivo_continua`) e o irmao ficou.
//!
//! Teste unitario nao prova isto: o que se quer saber e se uma replica no ar,
//! puxando de um source no ar, ACUSA em vez de calar -- e isso so os dois
//! processos conversando mostram.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "continuidade";

fn pasta(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("continuidade-{nome}"))
}

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
    // O ESCAPE ESCRITO: desde 18/09/2026 a cifra do fio nasce exigida
    // (pedido 370, ordem do dono), e esta bateria conecta em claro porque o
    // que ela mede e a continuidade do diario entre source e replica. Sem esta
    // linha a recusa lida aqui seria a da cifra, e a prova passaria a medir
    // o portao errado.
    c.cifra_fio.exigir = false;
    c.web.ligado = false;
    c
}

/// Pede a porta 0 e devolve a REAL, lida do proprio servidor -- pedido 401.
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

fn subir_source(base: &std::path::Path) -> (Arc<Servidor>, u16) {
    let mut c = config_base(base);
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "source-do-teste".into();
    c.replicacao.imagem_da_linha = true;
    subir(c)
}

/// `porta_do_source` e' a porta REAL do source, ja no ar -- ele sobe primeiro
/// (`subir_source`), e so entao a replica e' configurada apontando para o
/// numero que ele realmente abriu. Nenhum numero e' escolhido por fora aqui.
fn subir_replica(base: &std::path::Path, porta_do_source: u16) -> (Arc<Servidor>, u16) {
    let mut c = config_base(base);
    c.replicacao.papel = Papel::Replica;
    c.replicacao.id_servidor = "replica-do-teste".into();
    c.replicacao.origens = vec![Origem {
        nome: "fonte".into(),
        host: "127.0.0.1".into(),
        porta: porta_do_source,
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
    subir(c)
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

fn criar_clientes(porta: u16) {
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
}

fn inserir(porta: u16, ids: impl Iterator<Item = i64>) {
    for id in ids {
        exigir(
            porta,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id}}}"#
            ),
        );
    }
}

/// Quantos eventos `clientes` tem num servidor, pela op `posicao`.
fn eventos_de_clientes(porta: u16) -> i64 {
    exigir(porta, r#""op":"posicao","database":"loja""#)
        .campo("tabelas")
        .and_then(|t| t.campo("clientes"))
        .map(|c| c.inteiro_ou("eventos", -1))
        .unwrap_or(-1)
}

fn esperar_eventos(porta: u16, quantos: i64) {
    let ate = Instant::now() + Duration::from_secs(20);
    while Instant::now() < ate {
        if eventos_de_clientes(porta) == quantos {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "a replica nao chegou a {quantos} evento(s) em 20 s (esta em {})",
        eventos_de_clientes(porta)
    );
}

/// O recado de recusa de `loja/clientes` na origem `fonte`, se houver.
fn recusa_de_clientes(porta_replica: u16) -> Option<String> {
    exigir(porta_replica, r#""op":"replicacao_estado""#)
        .campo("origens")
        .and_then(|o| o.campo("fonte"))
        .and_then(|f| f.campo("recusas"))
        .and_then(|r| r.campo("loja/clientes"))
        .and_then(Json::texto)
        .map(str::to_string)
}

fn ids_na_replica(porta: u16) -> Vec<i64> {
    exigir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    )
    .campo("linhas")
    .and_then(Json::lista)
    .unwrap_or(&[])
    .iter()
    .map(|l| l.inteiro_ou("id", -1))
    .collect()
}

/// **Prova real.** O source apaga e recria `clientes` com MAIS linhas do que
/// a replica tinha: a replica acusa em `replicacao_estado`, nomeando a
/// tabela e dizendo que ela foi apagada e recriada, e NAO aplica a vida nova
/// em cima da velha. Com o `>=` de antes e o lote sem conferencia, a replica
/// aplica a quarta linha calada -- e este teste cai.
#[test]
fn tabela_apagada_e_recriada_no_source_e_acusada_e_nao_aplicada() {
    let base_s = pasta("source-recria");
    let base_r = pasta("replica-recria");
    let (_source, porta_s) = subir_source(&base_s);
    exigir(porta_s, r#""op":"criar_database","database":"loja""#);
    criar_clientes(porta_s);
    inserir(porta_s, 1..=3);
    let (_replica, porta_r) = subir_replica(&base_r, porta_s);
    esperar_eventos(porta_r, 3);
    assert_eq!(ids_na_replica(porta_r), vec![1, 2, 3]);
    assert_eq!(
        recusa_de_clientes(porta_r),
        None,
        "com o source sao, recusa nenhuma"
    );

    // A outra vida: a mesma tabela, com quatro linhas que nao sao aquelas.
    exigir(
        porta_s,
        r#""op":"excluir_tabela","database":"loja","tabela":"clientes","confirmar":"clientes""#,
    );
    criar_clientes(porta_s);
    inserir(porta_s, 11..=14);
    assert_eq!(eventos_de_clientes(porta_s), 4);

    let ate = Instant::now() + Duration::from_secs(20);
    let recusa = loop {
        if let Some(r) = recusa_de_clientes(porta_r) {
            break r;
        }
        assert!(
            Instant::now() < ate,
            "a replica nao acusou em 20 s a tabela apagada e recriada no source \
             (eventos aqui: {}, ids: {:?})",
            eventos_de_clientes(porta_r),
            ids_na_replica(porta_r)
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(
        recusa.contains("apagada e recriada"),
        "a recusa tem de dizer o que aconteceu: {recusa}"
    );
    // E o dado desta replica ficou como estava: tres linhas da vida velha,
    // nenhuma da nova -- nem a quarta, cujo rowid coincidiria.
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(ids_na_replica(porta_r), vec![1, 2, 3]);
    assert_eq!(eventos_de_clientes(porta_r), 3);
}

/// O comportamento VELHO, que nao pode mudar: com o source continuando a
/// mesma historia, a replica segue aplicando e nao acusa nada -- inclusive
/// depois de ficar ociosa (a conferencia de uma tabela sem novidade e a que
/// vai pela rede, e ela tem de dizer «continua»).
#[test]
fn com_o_source_continuo_a_replica_segue_sem_recusa() {
    let base_s = pasta("source-segue");
    let base_r = pasta("replica-segue");
    let (_source, porta_s) = subir_source(&base_s);
    exigir(porta_s, r#""op":"criar_database","database":"loja""#);
    criar_clientes(porta_s);
    inserir(porta_s, 1..=3);
    let (_replica, porta_r) = subir_replica(&base_r, porta_s);
    esperar_eventos(porta_r, 3);
    // Ociosa por duas rodadas: a conferencia pela rede roda e confirma.
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(recusa_de_clientes(porta_r), None);

    inserir(porta_s, 4..=5);
    esperar_eventos(porta_r, 5);
    assert_eq!(ids_na_replica(porta_r), vec![1, 2, 3, 4, 5]);
    assert_eq!(
        recusa_de_clientes(porta_r),
        None,
        "o source continuou: nada a acusar"
    );

    // E o diario daqui e A MESMA HISTORIA: o carimbo de cada evento e o do
    // source, nao a hora em que esta replica sincronizou (papel C, §2.3).
    let de_la = exigir(
        porta_s,
        r#""op":"diario","database":"loja","tabela":"clientes","max":5"#,
    );
    let daqui = exigir(
        porta_r,
        r#""op":"diario","database":"loja","tabela":"clientes","max":5"#,
    );
    let carimbos = |j: &Json| -> Vec<i64> {
        j.campo("eventos")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .map(|e| e.inteiro_ou("carimbo_ms", 0))
            .collect()
    };
    assert_eq!(
        carimbos(&de_la),
        carimbos(&daqui),
        "a replica lavou o carimbo do source"
    );
}
