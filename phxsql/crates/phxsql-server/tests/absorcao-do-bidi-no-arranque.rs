//! Pedido 330: a primeira rodada do bidirecional depois do arranque parava o
//! servidor -- provado PELO SOQUETE, com dois servidores de verdade e um
//! escritor concorrente medindo a propria espera.
//!
//! O mapa de toques e estado de processo. Processo novo, mapa vazio, e a
//! primeira rodada de cada tabela passava o diario local INTEIRO pelo mapa com
//! a trava exclusiva de dados na mao: nenhuma escrita e nenhuma leitura
//! andavam ate acabar. Medido em release (`--example custo-da-absorcao-do-bidi`,
//! 01/10/2026): 2,26-2,63 us por evento, 2,3-2,5 s para um diario de 1 M.
//!
//! O desenho do J (01/10/2026, `decisoes-onda-01-10-2026.md` §330): o grosso
//! sob a trava de LEITURA, em fatias de 10 ms; so a cauda sob a exclusiva. O
//! que se mede aqui nao e o veredito da replicacao -- e se o escritor de
//! cliente PEGOU a trava enquanto a absorcao acontecia.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::{Config, Origem, Papel, Servidor};
use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "absorcao-do-bidi";
/// O diario local que o processo novo encontra. Grande o bastante para a
/// absorcao inteira sob a exclusiva custar segundos num build de teste, e
/// pequeno o bastante para o arranjo nao dominar a corrida.
const EVENTOS: i64 = 300_000;
/// Quantos escritores tem de PEGAR a trava exclusiva entre duas fatias da
/// absorcao. Com o defeito, zero: a absorcao inteira acontece com a trava
/// exclusiva na mao, e nao ha «entre fatias».
///
/// # Por que contar ENTRADAS, e nao `inserir` terminados nem espera
///
/// A primeira versao punha teto na espera maxima de um `inserir`, e flocou.
/// A segunda contava os `inserir` que TERMINAVAM com o mapa pela metade, e
/// flocou tambem (pedido 623): caiu em 2 de 2 corridas dos portoes com zero
/// no meio e `sob_a_exclusiva` 22. O diagnostico escrito ali -- «o pico era o
/// disco» -- era plausivel e nao medido. Medido em 01/10/2026, com a maquina
/// parada: o escritor esperava 115-360 ms por `inserir` e ate 2,1 s, e as
/// fatias corriam coladas, 11 ms cada, sem buraco. Era o `RwLock`: o leitor
/// em laco retomava a leitura antes de o escritor acordado ser escalado
/// (ver `retrato.rs`, «O leitor que CEDE a vez»). Sob a suite, com os
/// nucleos tomados, o escritor perdia todas.
///
/// O conserto cede a vez, e a prova conta no SERVIDOR quem pegou a ficha
/// entre as fatias: a entrada so depende da trava, o fim do `inserir` depende
/// tambem do disco de quem escreve.
const ENTRARAM_ENTRE_FATIAS: i64 = 1;
/// Quantos escritores gravam ao mesmo tempo, cada um na sua conexao.
const ESCRITORES: i64 = 2;

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
    // O escape escrito da cifra exigida: o que se mede e a trava.
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

/// Um campo do mapa de toques de `loja/clientes` em B, pelo
/// `replicacao_estado`: `vistos`, `chaves`, `sob_a_exclusiva`,
/// `escritores_entre_fatias` ou `fatias_que_furaram_a_fila`.
fn do_mapa(porta: u16, campo: &str) -> i64 {
    tentar(porta, r#""op":"replicacao_estado""#)
        .and_then(|r| r.campo("resultado").cloned())
        .and_then(|r| r.campo("toques_no_mapa").cloned())
        .and_then(|m| m.campo("loja/clientes").cloned())
        .map(|t| t.inteiro_ou(campo, -1))
        .unwrap_or(-1)
}

fn vistos_em(porta: u16) -> i64 {
    do_mapa(porta, "vistos")
}

/// Um escritor: uma conexao so, um `inserir` atras do outro, com ids a partir
/// de `base`. Conta em `feitos` cada um que termina, e devolve a maior espera
/// -- os dois so para o relatorio: nenhum veredito sai do relogio.
fn escritor(porta: u16, base: i64, parar: Arc<AtomicBool>, feitos: Arc<AtomicU64>) -> Duration {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    let mut maior = Duration::ZERO;
    // Teto de voltas: o laco para pelo sinal, e o teto segura o caso em que
    // o sinal nunca vem (o teste caiu antes de dar).
    for i in 1..=200_000i64 {
        if parar.load(Ordering::SeqCst) {
            break;
        }
        let inicio = Instant::now();
        writeln!(
            escrita,
            r#"{{"token":"{TOKEN}","op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{}}}}}"#,
            base + i
        )
        .unwrap();
        let mut resposta = String::new();
        leitor.read_line(&mut resposta).unwrap();
        let d = inicio.elapsed();
        assert!(resposta.contains("\"ok\":true"), "{resposta}");
        // Os tres primeiros pagam abrir a tabela pela primeira vez.
        if i > 3 {
            maior = maior.max(d);
        }
        feitos.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(2));
    }
    maior
}

/// **A prova real.** B arranca com 300.000 eventos locais em `loja/clientes`
/// e um escritor gravando na MESMA `loja/clientes` sem parar -- o caixa que
/// trabalha enquanto sincroniza, e o caso em que a tabela vive com o cabecalho
/// do `.log` atras do arquivo. A cria `clientes`, e a rodada seguinte de B
/// absorve o diario local inteiro no mapa. Tres
/// provas, todas pelo `replicacao_estado` de B: menos de um lote passou com a
/// trava exclusiva na mao, escritor PEGOU a trava entre as fatias
/// ([`ENTRARAM_ENTRE_FATIAS`]), e nenhuma fatia retomou a leitura passando na
/// frente de escritor na fila.
///
/// Os vermelhos, medidos em 01/10/2026: sem a chamada a
/// `pre_absorver_sob_leitura` em `abrir_para_bidi`, a absorcao inteira roda
/// sob a exclusiva -- `sob_a_exclusiva` vira os 300.000 e ninguem entra entre
/// fatias, porque nao ha fatia; sem o `ceder` entre as fatias, o leitor fura
/// a fila do escritor (pedido 623).
#[test]
fn a_primeira_rodada_nao_para_o_escritor() {
    let base_a = DirTemp::novo("absorcao330-a");
    let base_b = DirTemp::novo("absorcao330-b");
    // O diario local de B, gravado antes de o processo subir: e o que um
    // caixa encontra ao reiniciar.
    {
        let inst = Instancia::nova(&base_b.0).unwrap();
        let db = inst.criar_database("loja").unwrap();
        let mut t = db.criar_tabela(None, clientes()).unwrap();
        t.ligar_imagem_no_diario(true);
        for i in 1..=EVENTOS {
            t.inserir(&[Value::Int(i), Value::Str(format!("c{i}"))])
                .unwrap();
        }
        t.sincronizar().unwrap();
    }
    let a = subir(config(&base_a.0, "no-a"));
    let mut cb = config(&base_b.0, "no-b");
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
    }];
    let b = subir(cb);

    let parar = Arc::new(AtomicBool::new(false));
    let feitos = Arc::new(AtomicU64::new(0));
    let porta_b = b.porta;
    let fios: Vec<_> = (1..=ESCRITORES)
        .map(|n| {
            let (sinal, conta) = (Arc::clone(&parar), Arc::clone(&feitos));
            std::thread::spawn(move || escritor(porta_b, n * 1_000_000, sinal, conta))
        })
        .collect();
    // O escritor ja esta gravando quando A passa a ter a tabela: a absorcao
    // acontece com ele no meio, e nao antes nem depois.
    std::thread::sleep(Duration::from_millis(300));
    exigir(a.porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        a.porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},{"nome":"nome","tipo":"Str(20)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#
            .replace('\n', " ")
            .as_str(),
    );

    // Quantos `inserir` terminaram na primeira vez que o mapa foi visto pela
    // metade, e na primeira em que foi visto inteiro -- so para o relatorio.
    let ate = Instant::now() + Duration::from_secs(120);
    let mut no_meio: Option<u64> = None;
    let no_fim = loop {
        let v = vistos_em(b.porta);
        let agora = feitos.load(Ordering::SeqCst);
        if v >= EVENTOS {
            break agora;
        }
        if v > 0 && no_meio.is_none() {
            no_meio = Some(agora);
        }
        assert!(
            Instant::now() < ate,
            "B nao absorveu o diario local em 120 s (vistos: {v})"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    parar.store(true, Ordering::SeqCst);
    let maior = fios
        .into_iter()
        .map(|f| f.join().unwrap())
        .max()
        .unwrap_or_default();
    let sob = do_mapa(b.porta, "sob_a_exclusiva");
    let entraram = do_mapa(b.porta, "escritores_entre_fatias");
    let furaram = do_mapa(b.porta, "fatias_que_furaram_a_fila");
    let andaram = no_meio.map(|m| no_fim - m);
    eprintln!(
        "absorcao-do-bidi: {andaram:?} inserir terminados durante a absorcao, maior espera \
         {maior:?}, sob a exclusiva {sob}, entraram entre fatias {entraram}, \
         fatias que furaram a fila {furaram}"
    );
    // Do diario inteiro, so a cauda -- menos de um lote -- passou com a trava
    // exclusiva na mao. Com o defeito reposto, os 300.000 passam.
    assert!(
        (0..500).contains(&sob),
        "{sob} evento(s) do diario local foram absorvidos com a trava EXCLUSIVA"
    );
    // O diario inteiro entrou: os 300.000 de antes e o que o escritor gravou.
    assert!(do_mapa(b.porta, "chaves") >= EVENTOS);
    // O escritor PEGOU a trava enquanto a absorcao estava pela metade.
    assert!(
        entraram >= ENTRARAM_ENTRE_FATIAS,
        "so {entraram} escritor(es) pegaram a trava entre as fatias da absorcao \
         (minimo {ENTRARAM_ENTRE_FATIAS}): a absorcao do diario local segurou o escritor"
    );
    // E nenhuma fatia passou na frente de quem esperava.
    assert_eq!(
        furaram, 0,
        "{furaram} fatia(s) retomaram a leitura com escritor na fila e nenhum dentro: \
         a absorcao nao cedeu a vez"
    );
}
