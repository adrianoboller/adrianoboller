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
//! que se mede aqui nao e o veredito da replicacao -- e quanto um `inserir`
//! de cliente ESPEROU enquanto a absorcao acontecia.

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
/// Quantos `inserir` do escritor tem de TERMINAR enquanto a absorcao esta pela
/// metade. Com o defeito, zero: a absorcao inteira acontece com a trava
/// exclusiva na mao, e nenhum escritor anda nesse intervalo.
///
/// # Por que contar, e nao cronometrar
///
/// A primeira versao desta prova punha teto na espera maxima de um `inserir`,
/// e flocou: o `inserir` paga o `fsync` dele, e o disco deste conteiner e
/// dividido com outras frentes compilando. Medido em 01/10/2026 (build de
/// teste, 300.000 eventos), com o conserto e `sob_a_exclusiva` = 0 nas
/// cinco: 225 ms, 447 ms, 460 ms e **2,01 s** de espera maxima -- o pico era
/// o disco, nao a trava (a absorcao parou junto, no mesmo instante). Um teto
/// que floca e pior que teto nenhum. Contar quem andou DURANTE a absorcao
/// separa as duas coisas: o disco lento atrasa o escritor, a trava presa o
/// zera. E por isso o minimo e UM, e nao uma taxa: com o escritor na mesma
/// tabela, as corridas de 01/10/2026 deram 38, 14 e 5 no meio (a de 5 com um
/// pico de 2,1 s de disco); com o defeito, o meio nem existe.
const ANDARAM_NO_MEIO: u64 = 1;

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
/// `replicacao_estado`: `vistos`, `chaves` ou `sob_a_exclusiva`.
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

/// O escritor: uma conexao so, um `inserir` atras do outro. Conta em `feitos`
/// cada um que termina, e devolve a maior espera (so para o relatorio).
fn escritor(porta: u16, parar: Arc<AtomicBool>, feitos: Arc<AtomicU64>) -> Duration {
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
            1_000_000 + i
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
/// absorve o diario local inteiro no mapa. Duas
/// provas, as duas pelo soquete: o escritor ANDA enquanto a absorcao esta pela
/// metade ([`ANDARAM_NO_MEIO`]), e o `replicacao_estado` de B diz que menos de
/// um lote passou com a trava exclusiva na mao.
///
/// O vermelho: sem a chamada a `pre_absorver_sob_leitura` em
/// `abrir_para_bidi`, a absorcao inteira roda sob a exclusiva -- o mapa nunca
/// e visto pela metade (o `replicacao_estado` espera o mapa junto) e
/// `sob_a_exclusiva` vira os 300.000. Medido em 01/10/2026.
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
    let (sinal, conta) = (Arc::clone(&parar), Arc::clone(&feitos));
    let fio = std::thread::spawn(move || escritor(porta_b, sinal, conta));
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
    // metade, e na primeira em que foi visto inteiro.
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
    let maior = fio.join().unwrap();
    let sob = do_mapa(b.porta, "sob_a_exclusiva");
    let andaram = no_meio.map(|m| no_fim - m);
    eprintln!(
        "absorcao-do-bidi: {andaram:?} inserir durante a absorcao, maior espera {maior:?}, \
         sob a exclusiva {sob}"
    );
    // Do diario inteiro, so a cauda -- menos de um lote -- passou com a trava
    // exclusiva na mao. Com o defeito reposto, os 300.000 passam.
    assert!(
        (0..500).contains(&sob),
        "{sob} evento(s) do diario local foram absorvidos com a trava EXCLUSIVA"
    );
    // O diario inteiro entrou: os 300.000 de antes e o que o escritor gravou.
    assert!(do_mapa(b.porta, "chaves") >= EVENTOS);
    // E o escritor ANDOU enquanto a absorcao estava pela metade.
    let andaram = andaram.expect(
        "o mapa nunca foi visto pela metade: a absorcao aconteceu inteira de uma vez, \
         com a trava exclusiva na mao",
    );
    assert!(
        andaram >= ANDARAM_NO_MEIO,
        "so {andaram} inserir terminaram durante a absorcao (minimo {ANDARAM_NO_MEIO}): \
         a absorcao do diario local segurou o escritor"
    );
}
