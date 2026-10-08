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
    subir_replica_com(base, porta_do_source, false)
}

/// [`subir_replica`] escolhendo o `somente_leitura` -- a guarda PEDIDA da
/// escrita local (pedido 300 (4)).
fn subir_replica_com(
    base: &std::path::Path,
    porta_do_source: u16,
    somente_leitura: bool,
) -> (Arc<Servidor>, u16) {
    subir(config_da_replica(base, porta_do_source, somente_leitura))
}

fn config_da_replica(
    base: &std::path::Path,
    porta_do_source: u16,
    somente_leitura: bool,
) -> Config {
    let mut c = config_base(base);
    c.somente_leitura = somente_leitura;
    c.replicacao.papel = Papel::Replica;
    c.replicacao.id_servidor = "replica-do-teste".into();
    c.replicacao.origens = vec![Origem {
        nome: "fonte".into(),
        host: "127.0.0.1".into(),
        porta: porta_do_source,
        token: TOKEN.into(),
        // Lista VAZIA, «o que vier»: desde o pedido 677 o database DECLARADO
        // recusa escrita local, e o que esta bateria prova -- a escrita
        // local que rompe a continuidade, pedido 300 (4) -- so continua
        // possivel na origem que nao declarou o que traz.
        databases: vec![],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
    }];
    c
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

/// Os eventos de `loja/clientes` enquanto se ESPERA: a replica cria a base e
/// a tabela sozinha, depois de subir, e ate la o `posicao` responde
/// `NAO_ENCONTRADO`. Isso quer dizer «ainda nao chegou», nao «deu errado» --
/// e o `exigir` de antes caia na primeira pergunta sempre que a replica
/// perdia a corrida para o teste (medido em 24/09/2026: 4 quedas em 13
/// corridas, e mais com a maquina carregada). Qualquer outro erro continua
/// derrubando o teste.
fn eventos_de_clientes_se_ja_chegou(porta: u16) -> i64 {
    let r = pedir(porta, r#""op":"posicao","database":"loja""#);
    if !r.booleano_ou("ok", false) {
        assert_eq!(
            r.campo("nome").and_then(Json::texto),
            Some("NAO_ENCONTRADO"),
            "posicao na replica falhou por outro motivo: {}",
            r.escrever()
        );
        return -1;
    }
    r.campo("resultado")
        .and_then(|res| res.campo("tabelas"))
        .and_then(|t| t.campo("clientes"))
        .map(|c| c.inteiro_ou("eventos", -1))
        .unwrap_or(-1)
}

fn esperar_eventos(porta: u16, quantos: i64) {
    let ate = Instant::now() + Duration::from_secs(20);
    while Instant::now() < ate {
        if eventos_de_clientes_se_ja_chegou(porta) == quantos {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "a replica nao chegou a {quantos} evento(s) em 20 s (esta em {})",
        eventos_de_clientes_se_ja_chegou(porta)
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

/// As escritas locais que a replica aceitou em `loja/clientes`.
fn escritas_locais(porta_replica: u16) -> i64 {
    exigir(porta_replica, r#""op":"replicacao_estado""#)
        .campo("escritas_locais")
        .and_then(|e| e.campo("loja/clientes"))
        .and_then(Json::inteiro)
        .unwrap_or(0)
}

/// **Pedido 300 (4), prova real.** A replica SEM `somente_leitura` aceita uma
/// escrita local: ela toma o lugar do evento seguinte do source no diario
/// daqui. A conferencia de continuidade para a tabela -- nenhum evento do
/// source e pulado calado, e os de la nao entram por cima --, e a recusa diz a
/// causa VERDADEIRA: a escrita local aceita aqui, quantas, e o conserto.
///
/// **Defeito reposto** (`anotar_escrita_local` sem contar -- o silencio de
/// antes): a recusa volta a culpar o source e o `replicacao_estado` nao diz
/// nada; o teste cai na asercao do numero.
#[test]
fn escrita_local_na_replica_rompe_dizendo_a_causa_e_nao_pula_o_source() {
    let base_s = pasta("source-escrita-local");
    let base_r = pasta("replica-escrita-local");
    let (_source, porta_s) = subir_source(&base_s);
    exigir(porta_s, r#""op":"criar_database","database":"loja""#);
    criar_clientes(porta_s);
    inserir(porta_s, 1..=3);
    let (_replica, porta_r) = subir_replica(&base_r, porta_s);
    esperar_eventos(porta_r, 3);

    // A aplicacao escreve na replica, e o source segue a vida dele.
    inserir(porta_r, 99..=99);
    assert_eq!(
        escritas_locais(porta_r),
        1,
        "a replica aceitou escrita local calada"
    );
    inserir(porta_s, 4..=5);

    let ate = Instant::now() + Duration::from_secs(20);
    let recusa = loop {
        if let Some(r) = recusa_de_clientes(porta_r) {
            break r;
        }
        assert!(
            Instant::now() < ate,
            "a replica nao acusou em 20 s (ids: {:?})",
            ids_na_replica(porta_r)
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(
        recusa.contains("escrita LOCAL") && recusa.contains("somente_leitura"),
        "a recusa tem de nomear a escrita local e o conserto: {recusa}"
    );
    assert!(
        !recusa.contains("apagada e recriada"),
        "a recusa culpou o source pelo que foi escrito aqui: {recusa}"
    );
    // Nada do source entrou por cima, e nada foi pulado calado: a tabela
    // ficou como estava.
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(ids_na_replica(porta_r), vec![1, 2, 3, 99]);
}

/// **Pedido 626, o ramo da CONTAGEM, sem depender da rodada.** A escrita
/// local na replica e o source PARADO: a replica passa a contar mais eventos
/// que o source, e so o ramo `no.eventos < posicao` de `alcancar_tabela` a
/// alcanca -- nenhuma escrita do source chega para o lote comparar evento a
/// evento. E o ramo que o teste de cima so pegava quando a rodada de 1 s
/// caia entre a escrita local e o `inserir` no source, e por isso ele flocava.
///
/// **Defeito reposto** (a frase fixa «apagada e recriada no source» no ramo
/// da contagem, sem `por_que_nao_continua`): a recusa culpa o source pelo que
/// foi escrito aqui e o teste cai na asercao da causa.
#[test]
fn escrita_local_sem_o_source_andar_rompe_pela_contagem_dizendo_a_causa() {
    let base_s = pasta("source-contagem");
    let base_r = pasta("replica-contagem");
    let (_source, porta_s) = subir_source(&base_s);
    exigir(porta_s, r#""op":"criar_database","database":"loja""#);
    criar_clientes(porta_s);
    inserir(porta_s, 1..=3);
    let (_replica, porta_r) = subir_replica(&base_r, porta_s);
    esperar_eventos(porta_r, 3);

    inserir(porta_r, 99..=99);
    assert_eq!(escritas_locais(porta_r), 1);

    let ate = Instant::now() + Duration::from_secs(20);
    let recusa = loop {
        if let Some(r) = recusa_de_clientes(porta_r) {
            break r;
        }
        assert!(
            Instant::now() < ate,
            "a replica nao acusou em 20 s (ids: {:?})",
            ids_na_replica(porta_r)
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(
        recusa.contains("tem 3 evento(s) e esta replica tem 4"),
        "a recusa nao veio do ramo da contagem: {recusa}"
    );
    assert!(
        recusa.contains("escrita LOCAL") && recusa.contains("somente_leitura"),
        "a recusa tem de nomear a escrita local e o conserto: {recusa}"
    );
    assert!(
        !recusa.contains("apagada e recriada"),
        "a recusa culpou o source pelo que foi escrito aqui: {recusa}"
    );
    assert_eq!(ids_na_replica(porta_r), vec![1, 2, 3, 99]);
}

/// O irmao que trava a convergencia: COM `somente_leitura`, a escrita local
/// e recusada no portao, nada se conta, e a replica segue o source.
#[test]
fn com_somente_leitura_a_escrita_local_e_recusada_e_a_replica_segue() {
    let base_s = pasta("source-so-leitura");
    let base_r = pasta("replica-so-leitura");
    let (_source, porta_s) = subir_source(&base_s);
    exigir(porta_s, r#""op":"criar_database","database":"loja""#);
    criar_clientes(porta_s);
    inserir(porta_s, 1..=3);
    let (_replica, porta_r) = subir_replica_com(&base_r, porta_s, true);
    esperar_eventos(porta_r, 3);
    let r = pedir(
        porta_r,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":99}"#,
    );
    assert!(!r.booleano_ou("ok", true), "{}", r.escrever());
    assert_eq!(escritas_locais(porta_r), 0);
    inserir(porta_s, 4..=5);
    esperar_eventos(porta_r, 5);
    assert_eq!(ids_na_replica(porta_r), vec![1, 2, 3, 4, 5]);
    assert_eq!(recusa_de_clientes(porta_r), None);
}

// ------------------------------------------- pedido 630: a conta no lugar

const SENHA: &str = "senha-da-bateria";

/// Uma conexao que FICA aberta e entra com login -- com cadastro, o pedido
/// anonimo nem chega aos portoes.
struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn entrar(porta: u16, login: &str) -> Ligacao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let f = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
        f.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        f.set_nodelay(true).unwrap();
        let mut c = Ligacao {
            escrita: f.try_clone().unwrap(),
            leitor: BufReader::new(f),
        };
        let r = c.pedir(&format!(
            r#""op":"login","usuario":"{login}","senha":"{SENHA}""#
        ));
        assert!(
            r.booleano_ou("ok", false),
            "login de {login}: {}",
            r.escrever()
        );
        c
    }

    fn pedir(&mut self, corpo: &str) -> Json {
        // Uma escrita so por pedido, e sem Nagle: o `writeln!` em pedacos
        // esperava o ACK atrasado do outro lado, ~40 ms por pedido.
        let linha = format!("{{\"token\":\"{TOKEN}\",{corpo}}}\n");
        self.escrita.write_all(linha.as_bytes()).unwrap();
        let mut resposta = String::new();
        self.leitor.read_line(&mut resposta).unwrap();
        Json::analisar(&resposta)
            .unwrap_or_else(|e| panic!("{corpo}: resposta ilegivel ({e}): {resposta}"))
    }

    /// O mapa `escritas_locais` inteiro do `replicacao_estado`.
    fn escritas_locais(&mut self) -> Json {
        let r = self.pedir(r#""op":"replicacao_estado""#);
        assert!(r.booleano_ou("ok", false), "{}", r.escrever());
        r.campo("resultado")
            .and_then(|e| e.campo("escritas_locais"))
            .cloned()
            .unwrap_or(Json::Nulo)
    }
}

/// **Pedido 630, prova real pelo soquete.** Numa replica fiel SEM
/// `somente_leitura`, quem so le manda 10.000 `inserir` -- metade em nomes
/// aleatorios, metade na tabela real --, e o root manda 100 em tabelas que
/// nao existem. Tudo recusado, e o `escritas_locais` continua VAZIO: nem
/// memoria por nome inventado, nem a tabela real marcada como escrita aqui.
///
/// E o comportamento velho na mesma replica, para o teste nao passar com um
/// contador que nunca conta: a escrita local ACEITA do root conta 1.
///
/// **Defeito reposto** (a conta de volta no portao 2b, antes do portao 3):
/// o mapa sai com 5.101 chaves e a asercao do vazio cai.
#[test]
fn escrita_recusada_na_replica_nao_conta_nem_ocupa_memoria() {
    let base_s = pasta("source-630");
    let base_r = pasta("replica-630");
    let (_source, porta_s) = subir_source(&base_s);
    exigir(porta_s, r#""op":"criar_database","database":"loja""#);
    criar_clientes(porta_s);
    inserir(porta_s, 1..=3);

    let h = |s: &str| phxsql_core::senha::cifrar_com(s, 1);
    let mut c = config_da_replica(&base_r, porta_s, false);
    c.cadastro = phxsql_server::Cadastro::de_json(
        &Json::analisar(&format!(
            r#"{{ "root": {{ "login": "root", "senha_hash": "{}" }},
                 "usuarios": [ {{ "login": "leitor", "senha_hash": "{}",
                                  "bases": {{ "loja": {{ "ler": true }} }} }} ] }}"#,
            h(SENHA),
            h(SENHA)
        ))
        .unwrap(),
    )
    .unwrap();
    let (_replica, porta_r) = subir(c);
    let mut root = Ligacao::entrar(porta_r, "root");
    // A tabela real tem de estar AQUI antes do laco: e ela que o pedido
    // recusado nao pode marcar.
    let ate = Instant::now() + Duration::from_secs(20);
    loop {
        let r = root.pedir(r#""op":"posicao","database":"loja""#);
        let n = r
            .campo("resultado")
            .and_then(|j| j.campo("tabelas"))
            .and_then(|t| t.campo("clientes"))
            .map_or(-1, |c| c.inteiro_ou("eventos", -1));
        if n == 3 {
            break;
        }
        assert!(
            Instant::now() < ate,
            "a replica nao alcancou: {}",
            r.escrever()
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    let mut leitor = Ligacao::entrar(porta_r, "leitor");
    for i in 0..10_000u32 {
        let tabela = if i % 2 == 0 {
            format!("t{:08x}", i.wrapping_mul(2_654_435_761))
        } else {
            "clientes".to_string()
        };
        let r = leitor.pedir(&format!(
            r#""op":"inserir","database":"loja","tabela":"{tabela}","linha":{{"id":{}}}"#,
            1_000 + i
        ));
        assert!(
            !r.booleano_ou("ok", true),
            "o leitor gravou: {}",
            r.escrever()
        );
    }
    for i in 0..100 {
        let r = root.pedir(&format!(
            r#""op":"inserir","database":"loja","tabela":"fantasma{i}","linha":{{"id":1}}"#
        ));
        assert!(!r.booleano_ou("ok", true), "{}", r.escrever());
    }
    let mapa = root.escritas_locais();
    let chaves = match &mapa {
        Json::Objeto(pares) => pares.len(),
        _ => 0,
    };
    assert_eq!(
        chaves, 0,
        "pedido recusado entrou na conta da escrita local: {} chave(s)",
        chaves
    );

    // O comportamento velho: a escrita ACEITA conta.
    let r = root.pedir(r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":99}"#);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    let mapa = root.escritas_locais();
    assert_eq!(
        mapa.campo("loja/clientes").and_then(Json::inteiro),
        Some(1),
        "a escrita local aceita nao contou: {}",
        mapa.escrever()
    );
    // O IRMAO do `despachar`: o `INSERT` pelo SQL chega como `inserir`
    // derivado, pelo `executar_derivado`, e conta pela MESMA funcao. Sem a
    // conta nele, a escrita local por SQL voltaria a ser calada.
    let r = root
        .pedir(r#""op":"sql","database":"loja","texto":"INSERT INTO clientes (id) VALUES (98)""#);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    assert_eq!(
        root.escritas_locais()
            .campo("loja/clientes")
            .and_then(Json::inteiro),
        Some(2),
        "o INSERT pelo SQL nao contou como escrita local"
    );
}
