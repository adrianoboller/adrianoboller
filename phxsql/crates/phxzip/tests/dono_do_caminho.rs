//! A corrida (TOCTOU) da extracao, fechada pelo DONO do caminho (revisao SEC
//! da Z9, 09/10/2026; o porque esta no topo do `disco.rs`).
//!
//! As provas de dono alheio pedem root (para dar a pasta a outro uid) e
//! dizem que pularam quando nao ha. A da corrida de verdade pede tambem o
//! `python3`, que faz a troca atomica `renameat2(RENAME_EXCHANGE)` como o
//! atacante faria, rodando como `nobody`.
#![cfg(unix)]

mod comum;

use std::os::unix::fs::{chown, MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use comum::DirTemp;

use phxzip::disco::Destino;
use phxzip::tar::{self, EscritorTar};
use phxzip::Erro;

const NINGUEM: u32 = 65534;

fn modo(p: &Path, m: u32) {
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).unwrap();
}

fn sou_root(d: &Path) -> bool {
    std::fs::metadata(d).unwrap().uid() == 0
}

fn inseguro<T>(r: Result<T, Erro>) -> bool {
    matches!(r, Err(Erro::DestinoInseguro(_)))
}

/// Raiz onde outros escrevem e recusada ANTES de qualquer byte -- mesmo com
/// o bit pegajoso, que protege o que esta na pasta mas nao a descida por ela.
///
/// Prova real: sem a conferencia do dono no `Destino::novo`, a raiz `0777`
/// abre e o teste cai.
#[test]
fn raiz_que_outros_escrevem_e_recusada() {
    let d = DirTemp::novo("dono-raiz");
    for m in [0o777, 0o1777, 0o757] {
        modo(&d, m);
        assert!(inseguro(Destino::novo(&d)), "modo {m:o} abriu");
    }
    // O irmao: a mesma pasta sem escrita alheia abre -- senao o portao
    // recusaria tudo.
    modo(&d, 0o755);
    assert!(Destino::novo(&d).is_ok());
}

/// Pasta que JA existia dentro da raiz, com escrita alheia, e recusada ao
/// descer por ela.
///
/// Prova real: sem a conferencia no `pastas`, `sub/x.txt` e gravado e o
/// teste cai.
#[test]
fn pasta_do_caminho_que_outros_escrevem_e_recusada() {
    let d = DirTemp::novo("dono-pasta");
    std::fs::create_dir(d.join("sub")).unwrap();
    modo(&d.join("sub"), 0o777);
    let destino = Destino::novo(&d).unwrap();
    assert!(inseguro(destino.arquivo("sub/x.txt", b"x", None)));
    assert!(!d.join("sub/x.txt").exists());
}

/// Ancestral pegajoso (`/tmp`) e aceito: ali ninguem renomeia o que nao e
/// seu. E o irmao que impede a regra de recusar o `/tmp/minha-pasta`.
#[test]
fn ancestral_pegajoso_e_aceito() {
    let d = DirTemp::novo("dono-pegajoso");
    let mae = d.join("compartilhada");
    std::fs::create_dir(&mae).unwrap();
    modo(&mae, 0o1777);
    let raiz = mae.join("minha");
    std::fs::create_dir(&raiz).unwrap();
    assert!(Destino::novo(&raiz).is_ok());
    modo(&mae, 0o777);
    assert!(
        inseguro(Destino::novo(&raiz)),
        "ancestral 0777 sem pegajoso abriu"
    );
}

/// Pasta de OUTRO usuario no caminho -- dentro da raiz ou acima dela --
/// recusa: o dono dela pode trocar o que esta dentro. Pede root.
#[test]
fn pasta_de_outro_usuario_no_caminho_e_recusada() {
    let d = DirTemp::novo("dono-alheio");
    if !sou_root(&d) {
        eprintln!("PULADO: dar a pasta a outro uid pede root");
        return;
    }
    std::fs::create_dir(d.join("sub")).unwrap();
    chown(d.join("sub"), Some(NINGUEM), Some(NINGUEM)).unwrap();
    let destino = Destino::novo(&d).unwrap();
    assert!(inseguro(destino.arquivo("sub/x.txt", b"x", None)));

    let alheia = d.join("alheia");
    std::fs::create_dir(&alheia).unwrap();
    let raiz = alheia.join("raiz");
    std::fs::create_dir(&raiz).unwrap();
    chown(&alheia, Some(NINGUEM), Some(NINGUEM)).unwrap();
    assert!(inseguro(Destino::novo(&raiz)), "ancestral alheio abriu");
}

/// O atacante de verdade: `nobody`, com escrita SO na pasta `a` do destino,
/// troca `a/b` por um link para fora com `renameat2(RENAME_EXCHANGE)`
/// enquanto o root extrai `a/b/xN.txt` -- o caso que a SEC mediu em 5 de 9.
/// O dano medido e o arquivo em `fora/`.
///
/// Prova real: sem a conferencia do dono em `pastas`, aparecem arquivos em
/// `fora/` (medido na rodada: ver o relatorio) e o teste cai.
#[test]
fn a_troca_atomica_por_outro_usuario_nao_escreve_fora() {
    let d = DirTemp::novo("dono-corrida");
    if !sou_root(&d) || Command::new("python3").arg("-V").output().is_err() {
        eprintln!("PULADO: a corrida pede root (para rodar o atacante como nobody) e python3");
        return;
    }
    modo(&d, 0o755);
    let raiz = d.join("raiz");
    let fora = d.join("fora");
    std::fs::create_dir_all(raiz.join("a/b")).unwrap();
    std::fs::create_dir(&fora).unwrap();
    modo(&fora, 0o777);
    modo(&raiz.join("a"), 0o777);
    modo(&raiz.join("a/b"), 0o777);
    let corredor = d.join("corredor.py");
    std::fs::write(
        &corredor,
        "import os, sys, time, ctypes\n\
         libc = ctypes.CDLL(None, use_errno=True)\n\
         a, fora, fim = sys.argv[1], sys.argv[2], time.time() + float(sys.argv[3])\n\
         b = os.path.join(a, 'b').encode(); l = os.path.join(a, '.l').encode()\n\
         os.symlink(fora, l)\n\
         while time.time() < fim:\n\
         \x20   libc.renameat2(-100, b, -100, l, 2); libc.renameat2(-100, b, -100, l, 2)\n",
    )
    .unwrap();
    modo(&corredor, 0o644);
    let mut atacante = Command::new("python3")
        .arg("-I")
        .arg(&corredor)
        .arg(raiz.join("a"))
        .arg(&fora)
        .arg("3")
        .uid(NINGUEM)
        .gid(NINGUEM)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let fim = Instant::now() + Duration::from_millis(2500);
    let mut tentativas = 0;
    while Instant::now() < fim {
        let mut w = EscritorTar::novo();
        w.arquivo(&format!("a/b/x{tentativas}.txt"), b"ESCAPOU", 0)
            .unwrap();
        let _ = tar::extrair_em(&w.terminar(), &raiz);
        tentativas += 1;
    }
    let _ = atacante.wait();
    let escaparam = std::fs::read_dir(&fora).unwrap().count();
    assert_eq!(
        escaparam, 0,
        "{escaparam} de {tentativas} extracoes escreveram FORA da raiz pelo link trocado"
    );
}
