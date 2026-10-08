//! Pedido 330 (b): o mapa de toques do bidirecional ganhou teto -- provado
//! PELO SOQUETE, com dois servidores de verdade.
//!
//! O mapa guarda o ultimo toque local de cada chave distinta, e era o processo
//! inteiro sem teto: 86-118 bytes por chave, medido (`--example
//! custo-da-absorcao-do-bidi -- --teto N`, 01/10/2026). O teto
//! (`replicacao.teto_de_toques`) esquece os toques mais velhos e guarda o mais
//! novo esquecido como PISO. O evento remoto numa chave esquecida que nao
//! passa do piso nao tem resposta -- e o par PARA naquela tabela, dizendo, em
//! vez de escolher um lado calado.
//!
//! O cenario e o unico em que a resposta falta: uma escrita de A MAIS VELHA
//! que a escrita de B na mesma chave, chegando depois que o teto de B ja
//! esqueceu a chave. Com o mapa inteiro, a de B vence. Esquecida sem piso, a
//! de A sobrescrevia a de B calada -- que e o defeito que esta bateria trava.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::{Config, Origem, Papel, Servidor};
use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "teto-dos-toques";
const ESPERA: Duration = Duration::from_secs(30);
/// As chaves que B escreve DEPOIS da chave disputada -- bem mais que o teto
/// pequeno da prova, para a disputada sair do mapa.
const OUTRAS: i64 = 12;

struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let _ = tentar(self.porta, r#""op":"servico_parar""#);
    }
}

fn config(base: &std::path::Path, id: &str) -> Config {
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
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = Papel::Multi;
    c.replicacao.id_servidor = id.into();
    c.replicacao.imagem_da_linha = true;
    c
}

fn subir(c: Config) -> NoAr {
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr { _s: s, porta }
}

fn tentar(porta: u16, corpo: &str) -> Option<Json> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().ok()?;
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").ok()?;
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).ok()?;
    Json::analisar(&resposta).ok()
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = tentar(porta, corpo).unwrap_or_else(|| panic!("{corpo}: sem resposta de {porta}"));
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

fn clientes() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

/// Grava o diario de um servidor ANTES de ele subir: e o que cada lado tem
/// quando o par se encontra.
fn diario(base: &std::path::Path, linhas: &[(i64, &str)]) {
    let inst = Instancia::nova(base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let mut t = db.criar_tabela(None, clientes()).unwrap();
    t.ligar_imagem_no_diario(true);
    for (id, nome) in linhas {
        t.inserir(&[Value::Int(*id), Value::Str((*nome).into())])
            .unwrap();
        // Um carimbo por milissegundo, no minimo: a prova depende de a
        // escrita disputada de B ser estritamente a mais velha DELE.
        std::thread::sleep(Duration::from_millis(2));
    }
    t.sincronizar().unwrap();
}

/// O nome da linha 1 em B, pelo soquete.
fn nome_da_um(porta: u16) -> String {
    exigir(
        porta,
        r#""op":"buscar","database":"loja","tabela":"clientes","indice":"porId","chave":[1]"#,
    )
    .campo("linhas")
    .and_then(Json::lista)
    .and_then(|l| l.first())
    .map(|l| l.texto_ou("nome", "").to_string())
    .unwrap_or_default()
}

fn estado(porta: u16) -> Json {
    exigir(porta, r#""op":"replicacao_estado""#)
}

fn parada(porta: u16) -> Option<Json> {
    estado(porta)
        .campo("origens")
        .and_then(|o| o.campo("a"))
        .and_then(|p| p.campo("paradas"))
        .and_then(|p| p.campo("loja/clientes"))
        .cloned()
}

fn do_mapa(porta: u16) -> Json {
    estado(porta)
        .campo("toques_no_mapa")
        .and_then(|m| m.campo("loja/clientes"))
        .cloned()
        .unwrap_or(Json::Nulo)
}

fn colisoes(porta: u16) -> i64 {
    estado(porta)
        .campo("colisoes_de_sequencia")
        .and_then(|c| c.campo("loja/clientes"))
        .and_then(Json::inteiro)
        .unwrap_or(0)
}

/// O par: A com a linha 1 escrita PRIMEIRO; B com a mesma linha 1 escrita
/// DEPOIS (mais nova) e mais [`OUTRAS`] chaves. B puxa de A com o `teto`.
fn terreno(nome: &str, teto: Option<u64>) -> (NoAr, NoAr, DirTemp, DirTemp) {
    let base_a = DirTemp::novo(&format!("teto-toques-a-{nome}"));
    let base_b = DirTemp::novo(&format!("teto-toques-b-{nome}"));
    diario(&base_a.0, &[(1, "de A, mais velha")]);
    std::thread::sleep(Duration::from_millis(20));
    let mut de_b = vec![(1, "de B, mais nova")];
    let outras: Vec<String> = (0..OUTRAS).map(|i| format!("outra {i}")).collect();
    for (i, n) in outras.iter().enumerate() {
        de_b.push((100 + i as i64, n.as_str()));
    }
    diario(&base_b.0, &de_b);

    let a = subir(config(&base_a.0, "no-a"));
    let mut cb = config(&base_b.0, "no-b");
    if let Some(t) = teto {
        cb.replicacao.teto_de_toques = t;
    }
    cb.replicacao.origens = vec![Origem {
        nome: "a".into(),
        host: "127.0.0.1".into(),
        porta: a.porta,
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
        pino_tls: String::new(),
        espelho: false,
    }];
    let b = subir(cb);
    (a, b, base_a, base_b)
}

/// **A prova real.** Teto 4 em B: a linha 1 de B (a escrita mais velha DELE)
/// sai do mapa na absorcao, e o piso fica acima do carimbo da escrita de A.
/// Quando o evento de A chega, o mapa nao sabe -- e o par PARA em
/// `loja/clientes` com `toque_esquecido_pelo_teto`, a linha de B intacta, e o
/// `replicacao_estado` dizendo o teto e quantas esqueceu.
///
/// **Defeito reposto** (chave ausente = `Vence` em `MapaDeToques::decidir`,
/// ignorando o piso -- o `None => true` de antes, so que agora com o mapa que
/// esquece): a escrita VELHA de A sobrescreve a nova de B, calada, e o teste
/// cai na primeira asercao do laco.
#[test]
fn chave_esquecida_para_o_par_em_vez_de_perder_a_escrita() {
    let (_a, b, _da, _db) = terreno("para", Some(4));
    let ate = Instant::now() + ESPERA;
    let p = loop {
        let nome = nome_da_um(b.porta);
        assert_ne!(
            nome, "de A, mais velha",
            "a escrita MAIS VELHA de A sobrescreveu a de B, calada: o teto esqueceu o \
             toque e a decisao saiu as cegas"
        );
        if let Some(p) = parada(b.porta) {
            break p;
        }
        assert!(
            Instant::now() < ate,
            "o par nao parou em {} s (mapa: {})",
            ESPERA.as_secs(),
            do_mapa(b.porta).escrever()
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(
        p.texto_ou("motivo", ""),
        "toque_esquecido_pelo_teto",
        "{}",
        p.escrever()
    );
    assert!(
        p.texto_ou("detalhe", "").contains("teto_de_toques"),
        "o grito nao diz o caminho de volta: {}",
        p.escrever()
    );
    assert_eq!(nome_da_um(b.porta), "de B, mais nova");
    let m = do_mapa(b.porta);
    assert_eq!(m.inteiro_ou("teto", -1), 4, "{}", m.escrever());
    assert!(
        m.inteiro_ou("chaves", -1) <= 4,
        "o mapa passou do teto: {}",
        m.escrever()
    );
    assert!(m.inteiro_ou("esquecidas", 0) > 0, "{}", m.escrever());
    assert!(
        m.campo("esquecido_ate").and_then(Json::inteiro).is_some(),
        "{}",
        m.escrever()
    );
}

/// **O comportamento velho.** Sem teto apertado (o padrao), o mesmo par
/// decide como sempre: a linha de B, mais nova, vence; a inclusao de A sobre
/// a chave viva de B e contada como colisao; nada para e nada se esquece.
#[test]
fn sem_passar_do_teto_o_par_decide_como_sempre() {
    let (_a, b, _da, _db) = terreno("velho", None);
    let ate = Instant::now() + ESPERA;
    while colisoes(b.porta) < 1 {
        assert!(
            Instant::now() < ate,
            "o evento de A nao chegou em {} s",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(nome_da_um(b.porta), "de B, mais nova");
    assert!(parada(b.porta).is_none());
    let m = do_mapa(b.porta);
    assert_eq!(m.inteiro_ou("esquecidas", -1), 0, "{}", m.escrever());
    assert_eq!(m.inteiro_ou("chaves", -1), 1 + OUTRAS, "{}", m.escrever());
}
