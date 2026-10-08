//! Pedido 587: o laco da replica do CLUSTER recua como o laco comum, pelo
//! MESMO `replica::Ritmo`, e o recuo nao atrasa seguir o master eleito.
//!
//! Tudo pelo soquete: o "master" e um ouvinte que aceita e fecha na hora --
//! a conexao que cai no meio, `Falha::Rede` -- e anota QUANDO cada tentativa
//! chegou. E o laco de producao (`laco_da_replica_do_cluster`) que bate nele;
//! nada aqui chama o `Ritmo` direto.
use super::*;
use crate::usuarios::Cadastro;
use std::net::TcpListener;

type Vindas = Arc<Mutex<Vec<Instant>>>;

/// Um "master" que aceita, anota o instante e fecha. A thread nao termina:
/// o ouvinte vive o processo de teste, como os pares falsos do 585.
fn master_que_derruba() -> (u16, Vindas) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let vindas: Vindas = Arc::new(Mutex::new(Vec::new()));
    let anotar = Arc::clone(&vindas);
    std::thread::spawn(move || {
        for s in ouvinte.incoming().flatten() {
            anotar.lock().unwrap().push(Instant::now());
            drop(s);
        }
    });
    (porta, vindas)
}

/// Um no REPLICA de cluster com pulso de 1 s, cujos outros nos sao os
/// ouvintes dados (`no2`, `no3`). O master e declarado pelo proprio mapa
/// (`registrar`), como o pulso faria, e o laco sobe numa thread.
fn replica_do_cluster(
    nome: &str,
    porta2: u16,
    porta3: u16,
) -> (Arc<Servidor>, Arc<crate::cluster::EstadoCluster>, DirTemp) {
    let dir = DirTemp::novo(&format!("recuo-cluster-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
  "token": "t",
  "bind": "127.0.0.1:5397",
  "base": "{}",
  "replicacao": {{"papel": "replica", "id_servidor": "no1"}},
  "cluster": {{
    "id": "no1",
    "janela_inatividade_s": 30,
    "pulso_s": 1,
    "nos": [
      {{"id": "no1", "endereco": "127.0.0.1", "porta": 5397}},
      {{"id": "no2", "endereco": "127.0.0.1", "porta": {porta2}}},
      {{"id": "no3", "endereco": "127.0.0.1", "porta": {porta3}}}
    ]
  }}
}}
"#,
            dir.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    c.cadastro = Cadastro::default();
    let s = Servidor::novo(c).unwrap();
    let estado = s.cluster.clone().expect("cluster");
    assert_eq!(estado.papel(), crate::cluster::PapelVivo::Replica);
    let laco = Arc::clone(&s);
    std::thread::spawn(move || laco.laco_da_replica_do_cluster());
    (s, estado, dir)
}

fn declarar_master(estado: &crate::cluster::EstadoCluster, id: &str, epoca: u64) {
    estado
        .registrar(
            id,
            crate::cluster::PulsoDeNo {
                papel: crate::cluster::PapelVivo::Master,
                epoca,
                posicao: 0,
                incompleta: false,
                prioridade: 0,
                quando_ms: crate::agora_ms(),
                por_tabela: None,
            },
        )
        .unwrap();
}

fn intervalos(vindas: &Vindas) -> Vec<Duration> {
    let v = vindas.lock().unwrap().clone();
    v.windows(2).map(|p| p[1] - p[0]).collect()
}

fn esperar_vindas(vindas: &Vindas, quantas: usize, paciencia: Duration) {
    let limite = Instant::now() + paciencia;
    while vindas.lock().unwrap().len() < quantas {
        assert!(
            Instant::now() < limite,
            "{quantas} tentativa(s) nao chegaram em {paciencia:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Encerra o laco sem matar a thread: promovido, ele so dorme o pulso.
fn encerrar(estado: &crate::cluster::EstadoCluster) {
    let _ = estado.promover(estado.epoca() + 1);
}

/// **587: o master que derruba a conexao e retentado com recuo.** A
/// primeira espera e o pulso (1 s), a segunda o dobro (2 s): em 5,5 s
/// chegam tres tentativas (0, 1, 3 s; a quarta so aos 7 s).
///
/// # Prova real
///
/// Com o braco de volta ao `sleep(espera)` de antes, chegam seis no mesmo
/// tempo, uma por pulso, e o segundo intervalo sai ~1 s em vez de ~2 s.
#[test]
fn o_master_que_derruba_e_retentado_com_recuo() {
    let (porta2, vindas) = master_que_derruba();
    let (porta3, _) = master_que_derruba();
    let (s, estado, _dir) = replica_do_cluster("recua", porta2, porta3);
    declarar_master(&estado, "no2", 0);
    esperar_vindas(&vindas, 1, Duration::from_secs(3));
    std::thread::sleep(Duration::from_millis(5_500));
    let iv = intervalos(&vindas);
    encerrar(&estado);
    assert!(
        iv.len() == 2 || iv.len() == 3,
        "esperava 3 tentativas em 5,5 s (recuo 1, 2, 4 s), vieram {}: {iv:?}",
        iv.len() + 1
    );
    assert!(
        iv[1] >= Duration::from_millis(1_700),
        "o segundo intervalo tinha de dobrar o pulso, veio {iv:?}"
    );
    // E o recuo aparece no `replicacao_estado`, como no laco comum.
    let seguidas = s
        .estado_replicacao
        .lock()
        .unwrap()
        .get("cluster:no2")
        .map(|e| e.falhas_de_rede_seguidas)
        .unwrap_or(0);
    assert!(seguidas >= 2, "falhas_de_rede_seguidas = {seguidas}");
}

/// Espera, com paciencia longa (so desistencia, nunca afirmacao de
/// rapidez), o laco anotar `minimo` falhas seguidas contra a origem. Devolve
/// as seguidas, o `proxima_tentativa_ms` e o instante (ms) da leitura ANTERIOR
/// a que enxergou a anotacao: a anotacao grava os dois campos juntos, entao
/// `proxima - anterior` e a espera pedida mais, no maximo, o intervalo entre
/// duas leituras -- uma cota superior que nao depende de threads acordando em
/// serie (pedido 703).
fn esperar_seguidas(s: &Servidor, origem: &str, minimo: u32) -> (u32, i64, i64) {
    let limite = Instant::now() + Duration::from_secs(30);
    let mut anterior = crate::agora_ms();
    loop {
        let lido = s
            .estado_replicacao
            .lock()
            .unwrap()
            .get(origem)
            .map(|e| (e.falhas_de_rede_seguidas, e.proxima_tentativa_ms));
        if let Some((seguidas, proxima)) = lido {
            if seguidas >= minimo && proxima != 0 {
                return (seguidas, proxima, anterior);
            }
        }
        assert!(
            Instant::now() < limite,
            "{origem}: {minimo} falha(s) seguida(s) nao foram anotadas em 30 s"
        );
        anterior = crate::agora_ms();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A espera da `n`-esima falha seguida, pelo MESMO `Ritmo` do laco: o teste
/// nao carrega o numero digitado.
fn recuo_da_falha(base: Duration, n: u32) -> Duration {
    let mut r = crate::replica::Ritmo::novo(base);
    let mut espera = base;
    for _ in 0..n {
        if let crate::replica::Decisao::Dormir(e) = r.apos(crate::replica::Falha::Rede) {
            espera = e;
        }
    }
    espera
}

/// **O COMPORTAMENTO VELHO: a falha unica volta no pulso de sempre.** A
/// segunda tentativa chega ~1 s depois da primeira -- nem antes (giro em
/// vazio), nem com recuo (a primeira falha espera so a base).
///
/// Pedido 703: o teto de relogio (1,5 s) media o escalonador sob a suite
/// cheia. O piso (900 ms) e robusto a carga e fica; o «nao recuou» virou
/// contagem (uma falha seguida) mais a cota da espera anotada, que e menor
/// que o dobro da base (escrito aqui, nao tirado do `Ritmo`, senao o defeito
/// no proprio `Ritmo` moveria o limite junto) sem depender de tres threads acordando em serie.
#[test]
fn a_falha_unica_volta_no_pulso_de_sempre() {
    let (porta2, vindas) = master_que_derruba();
    let (porta3, _) = master_que_derruba();
    let (s, estado, _dir) = replica_do_cluster("velho", porta2, porta3);
    declarar_master(&estado, "no2", 0);
    let (seguidas, proxima, anterior) = esperar_seguidas(&s, "cluster:no2", 1);
    esperar_vindas(&vindas, 2, Duration::from_secs(30));
    let iv = intervalos(&vindas);
    encerrar(&estado);
    assert_eq!(seguidas, 1, "a primeira falha anota uma seguida");
    let base = Duration::from_secs(1);
    let cota = (proxima - anterior) as u128;
    assert!(
        cota < (base * 2).as_millis(),
        "a falha unica esperou mais que a base: {cota} ms (o recuo seria o dobro da base)"
    );
    assert!(
        iv[0] >= Duration::from_millis(900),
        "a falha unica tinha de voltar no pulso (1 s), veio {iv:?}"
    );
}

/// **O recuo nao atrasa seguir o eleito.** Com o recuo do no2 ja em 4 s,
/// a eleicao passa o master para o no3: a primeira tentativa no no3 chega
/// em ate ~1 s (o passo do sono vigiado), e nao no fim do recuo. Antes do
/// 587 o sono era o pulso inteiro as cegas; com `pulso_s` = 1 o teto e o
/// mesmo de antes, e com pulso maior fica menor que antes.
///
/// Pedido 703: o teto era «valor certo + 300 ms», e medido sob a suite cheia
/// deu 1,69 s -- 2 s abaixo do que o defeito daria. O limite agora e DERIVADO
/// DO DEFEITO: o sono cego terminaria no fim do recuo corrente (a espera da
/// terceira falha, pelo `Ritmo`), contado do mesmo ouvinte que carimbou a
/// terceira tentativa; chegar antes disso prova que o sono acordou no passo.
/// O segundo prazo (o recuo nao atravessa para o no3) virou contagem.
#[test]
fn o_recuo_nao_atrasa_seguir_o_master_eleito() {
    let (porta2, vindas2) = master_que_derruba();
    let (porta3, vindas3) = master_que_derruba();
    let (s, estado, _dir) = replica_do_cluster("eleito", porta2, porta3);
    declarar_master(&estado, "no2", 0);
    // Tres tentativas no no2 (0, 1, 3 s): o sono corrente e o de 4 s.
    esperar_vindas(&vindas2, 3, Duration::from_secs(30));
    let terceira = vindas2.lock().unwrap()[2];
    declarar_master(&estado, "no3", 1);
    esperar_vindas(&vindas3, 1, Duration::from_secs(30));
    let primeira_no3 = vindas3.lock().unwrap()[0];
    // E o recuo NAO atravessou para o eleito: a primeira falha nele conta uma
    // (ou duas, se a segunda ja caiu); o recuo herdado contaria quatro.
    let (seguidas, _, _) = esperar_seguidas(&s, "cluster:no3", 1);
    encerrar(&estado);
    let base = Duration::from_secs(1);
    let fim_do_recuo = terceira + recuo_da_falha(base, 3);
    eprintln!(
        "587: o eleito foi procurado {:?} antes do fim do recuo",
        fim_do_recuo.saturating_duration_since(primeira_no3)
    );
    assert!(
        primeira_no3 < fim_do_recuo,
        "o recuo do master velho atrasou seguir o eleito: so chegou depois do fim do recuo"
    );
    assert!(
        seguidas <= 2,
        "o recuo do no2 atravessou para o no3: falhas_de_rede_seguidas = {seguidas}"
    );
}
