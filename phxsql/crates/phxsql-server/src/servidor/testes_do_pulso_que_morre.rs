//! A thread de pulso que morre em PANICO -- pedido 452, provado pelo soquete.
//!
//! O outro no e um `TcpListener` deste teste que conta as conexoes: cada
//! tentativa de pulso e uma conexao de verdade, e «o par nunca mais e pulsado»
//! e contar zero. O panico e o gancho `panicos_no_pulso_de_teste`, que so
//! existe com `cfg(test)`.
use super::*;
use crate::apoio_teste::DirTemp;
use std::sync::atomic::AtomicUsize;

/// Um servidor em cluster de dois nos, com o `no2` no ouvinte do teste, e
/// o contador de conexoes que chegam la.
fn cluster_com_ouvinte(nome: &str) -> (Arc<Servidor>, Arc<AtomicUsize>, DirTemp) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let chegaram = Arc::new(AtomicUsize::new(0));
    let conta = Arc::clone(&chegaram);
    std::thread::spawn(move || {
        for c in ouvinte.incoming() {
            if c.is_ok() {
                conta.fetch_add(1, Ordering::SeqCst);
            }
        }
    });
    let dir = DirTemp::novo(&format!("pulso-452-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
  "token": "t",
  "bind": "127.0.0.1:0",
  "base": "{}",
  "replicacao": {{"papel": "source", "id_servidor": "no1", "imagem_da_linha": true}},
  "cluster": {{
    "id": "no1",
    "pulso_s": 1,
    "janela_inatividade_s": 30,
    "nos": [
      {{"id": "no1", "endereco": "127.0.0.1", "porta": 1}},
      {{"id": "no2", "endereco": "127.0.0.1", "porta": {porta}}}
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
    (Servidor::novo(c).unwrap(), chegaram, dir)
}

/// Sobe SO o supervisor do pulso, pelo laco de producao.
fn supervisor(s: &Arc<Servidor>) {
    let s = Arc::clone(s);
    std::thread::spawn(move || s.laco_do_supervisor_do_pulso());
}

/// Tira o `no2` da lista viva no fim: a thread de pulso ve e sai, e o
/// supervisor passa a girar sobre lista vazia.
fn soltar(s: &Arc<Servidor>) {
    if let Some(e) = s.cluster.clone() {
        e.remover("no2");
    }
}

/// **Um panico, e o par volta a ser pulsado.**
///
/// Com o defeito (a desmarcacao so na saida normal): o id fica marcado, o
/// supervisor nunca mais sobe outra thread, e o ouvinte conta ZERO
/// conexoes em 5 s. Com o conserto: a guarda desmarca no desenrolar, e
/// depois do recuo de 1 s a thread nova conecta.
#[test]
fn pulso_que_morre_em_panico_volta_a_pulsar() {
    let (s, chegaram, _dir) = cluster_com_ouvinte("volta");
    s.panicos_no_pulso_de_teste.store(1, Ordering::SeqCst);
    supervisor(&s);
    let ate = Instant::now() + Duration::from_secs(5);
    while chegaram.load(Ordering::SeqCst) == 0 && Instant::now() < ate {
        std::thread::sleep(Duration::from_millis(20));
    }
    let n = chegaram.load(Ordering::SeqCst);
    soltar(&s);
    assert_eq!(
        s.panicos_no_pulso_dados.load(Ordering::SeqCst),
        1,
        "o gancho nao entrou em panico -- a prova nao exercitou nada"
    );
    assert!(
        n >= 1,
        "a thread de pulso morreu em panico e o no2 NUNCA MAIS foi pulsado: \
             {n} conexao(oes) em 5 s"
    );
}

/// **O panico que se repete recua, e nao vira laco.**
///
/// Com o recuo tirado: o supervisor sobe uma thread nova a cada meio
/// segundo, e cada uma entra em panico. Com o recuo de 1 s, 2 s e 4 s, no
/// maximo 4 em 4,5 s -- e pelo menos 2, porque o no continua sendo
/// tentado.
#[test]
fn panico_que_se_repete_no_pulso_recua_em_vez_de_virar_laco() {
    let (s, _chegaram, _dir) = cluster_com_ouvinte("laco");
    s.panicos_no_pulso_de_teste
        .store(u32::MAX, Ordering::SeqCst);
    supervisor(&s);
    std::thread::sleep(Duration::from_millis(4_500));
    let n = s.panicos_no_pulso_dados.load(Ordering::SeqCst);
    s.panicos_no_pulso_de_teste.store(0, Ordering::SeqCst);
    soltar(&s);
    assert!(
        n <= 4,
        "o pulso que entra em panico a cada volta virou LACO de panico: {n} \
             panicos em 4,5 s (o supervisor sobe outro a cada 0,5 s)"
    );
    assert!(
        n >= 2,
        "o recuo virou abandono: so {n} panico(s) em 4,5 s -- o no deixou de \
             ser tentado"
    );
}
