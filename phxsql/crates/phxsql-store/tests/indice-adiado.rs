//! Pedido 324: o indice ADIADO de uma carga, contra o sistema operacional.
//!
//! A reserva do `BULKINSERT` e memoria de processo, e processo morto nao diz
//! nada a quem reabre. O que faz uma queda no meio da carga adiada terminar
//! com o indice certo e a marca no DISCO -- bytes 52 e 53 do `.ndx`, levados
//! por `fdatasync` antes da primeira linha (parecer do papel C, R2) -- e o
//! arranque, que reconstroi todo `.ndx` marcado (pedido 522). Teste unitario
//! nao prova queda: aqui o filho e este mesmo binario, morto por
//! `Child::kill()` (SIGKILL, sem desenrolar nada) depois de gravar a carga.

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::Instancia;

mod comum;
use comum::DirTemp;

const AVISO_324: &str = "SONDA-324: carga adiada gravada, esperando o fim";
const N: i64 = 500;

fn esquema() -> Schema {
    Schema::new(
        "p",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]),
            IndexDef::new("porCidade", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    let cidades = ["Blumenau", "Joinville", "Lages"];
    vec![
        Value::Int(i),
        Value::Str(cidades[(i as usize) % cidades.len()].into()),
    ]
}

/// **A queda no meio da carga adiada, contra o sistema operacional.** O
/// filho cria a tabela, adia o indice, grava `N` linhas e para sem nunca
/// reconstruir; o pai o mata e reabre pelo motor do arranque.
///
/// **Defeito reposto** (o `NdxFile::suspender` sem gravar o cabecalho): o
/// disco fica com os bytes 52 e 53 em 0 sobre uma arvore VAZIA, o arranque
/// nao reconstroi nada, e a busca responderia «nao achei» em silencio. Medido
/// com o defeito reposto: cai o `assert_eq!(cab[53], 1)`; e o `D` (o `inserir`
/// que nao adia) cai na propria sonda, que recusa a linha pelo portao.
#[cfg(unix)]
#[test]
fn a_queda_no_meio_da_carga_adiada_reconstroi_no_arranque() {
    use std::io::BufRead as _;
    let area = DirTemp::novo("324-queda");
    let base = area.0.clone();
    let mut filho = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "sonda_324_carga_adiada_ate_a_morte",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("PHX_SONDA_324", &base)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // Lido numa thread: a sonda nao fecha o cano, e ler aqui penduraria o
    // teste se o aviso nunca viesse.
    let erro = filho.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for l in std::io::BufReader::new(erro).lines().map_while(|l| l.ok()) {
            let _ = tx.send(l);
        }
    });
    let prazo = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut visto = String::new();
    let parou = loop {
        let falta = prazo.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(falta) {
            Ok(l) if l.contains(AVISO_324) => break true,
            Ok(l) => {
                visto.push_str(&l);
                visto.push('\n');
            }
            Err(_) => break false,
        }
    };
    filho.kill().unwrap();
    let _ = filho.wait();
    assert!(parou, "a sonda nao chegou ao fim da carga:\n{visto}");

    // O disco depois da queda: a arvore se declara suspensa.
    let cab = std::fs::read(base.join("loja").join("p.ndx")).unwrap();
    assert_eq!(cab[53], 1, "o byte 53 nao estava no disco na queda");
    assert_eq!(cab[52], 1, "o byte 52 nao subiu junto");

    // A reabertura pelo MESMO motor do arranque do servidor.
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    let r = db.recuperar_marcas();
    assert_eq!(
        r.indices_reconstruidos, 1,
        "o arranque nao reconstruiu: {:?}",
        r.indices_pendentes
    );
    let mut t = db.abrir_tabela(None, "p").unwrap();
    assert!(!t.indice_suspenso());
    for i in 1..=N {
        let achadas = t.buscar("porId", &[Value::Int(i)]).unwrap();
        assert_eq!(achadas.len(), 1, "id {i}: {achadas:?}");
    }
    let rel = t.verificar().unwrap();
    assert_eq!(rel.registros, N as u64);
    drop(t);
}

/// A sonda da prova de cima. Nao volta -- o pai a mata.
#[test]
#[ignore = "sonda: roda so reexecutada por a_queda_no_meio_da_carga_adiada_reconstroi_no_arranque"]
fn sonda_324_carga_adiada_ate_a_morte() {
    let Some(base) = std::env::var_os("PHX_SONDA_324") else {
        return;
    };
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let mut t = db.criar_tabela(None, esquema()).unwrap();
    // Sincronizada ANTES de adiar: o byte 52 vai a 0 no disco, e e so a
    // suspensao que pode leva-lo de volta a 1. Sem isto a prova passaria
    // pelo 52 que a criacao deixou de pe, e nao pela marca da carga.
    t.sincronizar().unwrap();
    assert_eq!(
        std::fs::read(std::path::Path::new(&base).join("loja").join("p.ndx")).unwrap()[52],
        0
    );
    t.adiar_indice().unwrap();
    for i in 1..=N {
        t.inserir(&linha(i)).unwrap();
    }
    // O fecho da janela da reserva: o `.reg` vai ao disco, a arvore suspensa
    // nao -- `sincronizar` nao baixa a marca de um indice suspenso.
    t.sincronizar().unwrap();
    eprintln!("{AVISO_324}");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// O mesmo processo que abre de novo a tabela suspensa -- o servidor abre a
/// cada pedido -- continua vendo-a suspensa, e as recusas nao mandam
/// `reparar indice`. E o `reindexar` e o que tira a suspensao.
#[test]
fn a_reabertura_no_mesmo_processo_continua_suspensa() {
    let area = DirTemp::novo("324-mesmo");
    let base = area.0.clone();
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    {
        let mut t = db.criar_tabela(None, esquema()).unwrap();
        t.adiar_indice().unwrap();
        for i in 1..=20 {
            t.inserir(&linha(i)).unwrap();
        }
    }
    let mut t = db.abrir_tabela(None, "p").unwrap();
    assert!(t.indice_suspenso());
    let e = t.buscar("porId", &[Value::Int(3)]).unwrap_err();
    assert!(e.to_string().contains("suspenso"), "{e}");
    assert!(!e.to_string().contains("reparar"), "{e}");
    // A insercao continua adiada na reabertura, sem abrir janela.
    t.inserir(&linha(21)).unwrap();
    t.reindexar().unwrap();
    assert!(!t.indice_suspenso());
    for i in 1..=21 {
        assert_eq!(
            t.buscar("porId", &[Value::Int(i)]).unwrap().len(),
            1,
            "id {i}"
        );
    }
    drop(t);
}
