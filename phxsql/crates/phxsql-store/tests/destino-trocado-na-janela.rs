//! O destino do backup trocado NO MEIO da corrida -- pedido 611 (S6 e S7 da
//! revisao SEC independente de 01/10/2026).
//!
//! Os irmaos do 568/593 (`destino-do-backup-sem-atalho.rs`) plantam o link
//! ANTES ou DEPOIS da corrida. Aqui a troca acontece DURANTE: uma thread vira
//! um link do caminho do destino de um lado para o outro, sem parar, enquanto
//! a corrida roda. E a unica prova que alcanca a janela entre conferir pelo
//! nome e usar o descritor, porque o nucleo e quem decide quem ve o que -- e
//! isso nao se simula.
//!
//! # Por que um teto de voltas, e nao um laco ate acertar
//!
//! A corrida e sorte, e um teste sem teto trava a suite no dia em que a sorte
//! muda. Com o defeito reposto, o teto foi medido acertando a janela bem
//! antes do fim (numero na nota de cada teste); com o conserto, as voltas
//! todas passam. Um teto que o defeito reposto nao alcanca nao prova nada --
//! por isso o numero medido do RED fica escrito ao lado.
#![cfg(target_os = "linux")]

mod comum;
use comum::DirTemp;

use phxsql_store::backup;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Voltas no maximo, e tempo no maximo: o que vier primeiro.
const TETO_DE_VOLTAS: usize = 4000;
const TETO_DE_TEMPO: Duration = Duration::from_secs(20);

/// Uma raiz com dois databases, cada um com o seu `c.reg`.
fn raiz_com_dois(d: &Path) -> PathBuf {
    let raiz = d.join("dados");
    for db in ["loja", "rh"] {
        std::fs::create_dir_all(raiz.join(db)).unwrap();
        std::fs::write(raiz.join(db).join("c.reg"), db.as_bytes()).unwrap();
    }
    raiz
}

/// Vira `ponte` entre `a` e `b` sem parar, ate `parar`. Cada virada e um
/// `symlink` num nome temporario e um `rename` por cima de `ponte` -- atomico:
/// quem resolve `ponte` ve sempre um dos dois, nunca o nome faltando.
fn virador(
    d: &Path,
    a: PathBuf,
    b: PathBuf,
    parar: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    let ponte = d.join("ponte");
    let tmp = d.join("ponte.tmp");
    std::os::unix::fs::symlink(&a, &ponte).unwrap();
    std::thread::spawn(move || {
        let mut lado = false;
        while !parar.load(Ordering::Relaxed) {
            lado = !lado;
            let _ = std::fs::remove_file(&tmp);
            std::os::unix::fs::symlink(if lado { &b } else { &a }, &tmp).unwrap();
            std::fs::rename(&tmp, &ponte).unwrap();
        }
    })
}

/// **S7: o destino conferido pelo nome e aberto pelo descritor.**
///
/// `destino = d/ponte/loja`, e `ponte` vira entre `d/fora` (onde `loja` e uma
/// pasta comum) e a RAIZ de dados (onde `loja` e o database vivo). Quando o
/// `conferir_destino` le o nome com `ponte -> fora` e o `Pasta::abrir` o abre
/// com `ponte -> raiz`, as copias caiam em `dados/loja/rh/c.reg`: o schema
/// `rh` dentro do database `loja`, visivel a quem so tem direito em `loja`.
/// Conferido pelo descritor aberto (dev/inode dele e dos ancestrais da raiz),
/// a corrida recusa antes da primeira copia.
#[test]
fn copias_nunca_caem_dentro_da_raiz_pela_troca_na_janela() {
    let d = DirTemp::novo("611-s7-troca-na-janela");
    let raiz = raiz_com_dois(&d);
    let fora = d.join("fora");
    std::fs::create_dir_all(fora.join("loja")).unwrap();
    let destino = d.join("ponte").join("loja");
    let invadido = raiz.join("loja").join("rh");

    let parar = Arc::new(AtomicBool::new(false));
    let fio = virador(&d, fora.clone(), raiz.clone(), parar.clone());
    let comeco = Instant::now();
    let (mut voltas, mut abriu_fora, mut recusou) = (0usize, 0usize, 0usize);
    let mut caiu_dentro = None;
    while voltas < TETO_DE_VOLTAS && comeco.elapsed() < TETO_DE_TEMPO {
        voltas += 1;
        match backup::executar(&raiz, &destino, 1) {
            Ok(_) => abriu_fora += 1,
            Err(_) => recusou += 1,
        }
        if invadido.exists() {
            caiu_dentro = Some(voltas);
            break;
        }
    }
    parar.store(true, Ordering::Relaxed);
    fio.join().unwrap();
    eprintln!(
        "611/S7: {voltas} voltas, {abriu_fora} no destino de fora, {recusou} recusadas, \
         dentro da raiz: {caiu_dentro:?}"
    );
    assert_eq!(
        caiu_dentro, None,
        "a corrida conferiu o destino pelo NOME e abriu pelo descritor outra \
         pasta: as copias cairam dentro do database vivo (dados/loja/rh)"
    );
    // A premissa: a troca alcancou os dois lados -- senao o teste passaria
    // por nunca ter trocado nada.
    assert!(
        abriu_fora > 0 && recusou > 0,
        "a troca nao alternou os lados"
    );
}

/// **S6: o manifesto velho sai pela pasta que a corrida ABRIU.**
///
/// `destino = d/ponte/copias`, e `ponte` vira entre `d/nosso` e `d/outro` --
/// cada um com o seu `copias/backup.json`. Pelo nome, quando o `Pasta::abrir`
/// pegava `nosso` e o `invalidar_manifesto_velho` resolvia `ponte -> outro`,
/// o `remove_file` apagava o manifesto do OUTRO backup -- que a corrida nem
/// tocou -- e o `restaurar` passava a recusa-lo inteiro.
///
/// O sinal: o manifesto do outro sumiu E nenhuma copia foi para la. Quando a
/// corrida inteira abriu `outro`, apagar o manifesto de la e o certo (e a
/// pasta da corrida), e o teste repoe o lado e segue.
#[test]
fn o_manifesto_apagado_e_o_da_pasta_aberta() {
    let d = DirTemp::novo("611-s6-manifesto-pelo-descritor");
    let raiz = raiz_com_dois(&d);
    let (nosso, outro) = (d.join("nosso"), d.join("outro"));
    let manifesto_do_outro = outro.join("copias").join(backup::MANIFESTO);
    for lado in [&nosso, &outro] {
        std::fs::create_dir_all(lado.join("copias")).unwrap();
        std::fs::write(lado.join("copias").join(backup::MANIFESTO), b"{}").unwrap();
    }
    let destino = d.join("ponte").join("copias");

    let parar = Arc::new(AtomicBool::new(false));
    let fio = virador(&d, nosso.clone(), outro.clone(), parar.clone());
    let comeco = Instant::now();
    let (mut voltas, mut no_outro) = (0usize, 0usize);
    let mut apagou_alheio = None;
    while voltas < TETO_DE_VOLTAS && comeco.elapsed() < TETO_DE_TEMPO {
        voltas += 1;
        // Cada volta comeca com um manifesto dos dois lados.
        std::fs::write(nosso.join("copias").join(backup::MANIFESTO), b"{}").unwrap();
        let _ = backup::executar(&raiz, &destino, 1);
        let copiou_la = outro.join("copias").join("loja").exists();
        if copiou_la {
            no_outro += 1;
            let _ = std::fs::remove_dir_all(outro.join("copias").join("loja"));
            let _ = std::fs::remove_dir_all(outro.join("copias").join("rh"));
        } else if !manifesto_do_outro.exists() {
            apagou_alheio = Some(voltas);
            break;
        }
        std::fs::write(&manifesto_do_outro, b"{}").unwrap();
    }
    parar.store(true, Ordering::Relaxed);
    fio.join().unwrap();
    eprintln!(
        "611/S6: {voltas} voltas, {no_outro} inteiras no outro, manifesto alheio \
         apagado: {apagou_alheio:?}"
    );
    assert_eq!(
        apagou_alheio, None,
        "o manifesto velho saiu pelo NOME: o backup.json de OUTRO backup, que a \
         corrida nem tocou, foi apagado pela troca do link no meio"
    );
    assert!(no_outro > 0, "a troca nao alternou os lados");
}
