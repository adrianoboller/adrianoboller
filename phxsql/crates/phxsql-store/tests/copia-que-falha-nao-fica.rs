//! O backup em PASTA que falha nao deixa copias sem manifesto -- pedido 576.
//!
//! Medido antes do conserto: `executar` → `concluir` (`sincronizar_copias` e
//! `finalizar_manifesto`) subia o erro e deixava na pasta as copias ja
//! escritas, sem `backup.json`. Nem `op_backups` nem a rotacao as reconhecem,
//! entao ficavam para sempre -- uma copia do banco inteiro por falha.
//!
//! # Por que as falhas sao do sistema operacional, e nao forjadas
//!
//! Pelo mesmo motivo do irmao ZIP (`parcial-do-zip-nao-fica.rs`, pedido 555):
//! a pergunta e «o que sobra na pasta quando o DISCO recusa?». As duas provas
//! derrubam as duas fases:
//!
//! - a COPIA, num processo filho com `ulimit -f` (`EFBIG` no meio do `write`
//!   do arquivo grande, depois de o pequeno ja ter nascido);
//! - o MANIFESTO, com um DIRETORIO no nome `backup.json` -- as copias ja
//!   escritas e sincronizadas, a recusa no ultimo passo.
//!
//! E a regra dura das duas: sai SO o que a corrida criou, pela lista dela.
//! A pasta e escolhida pelo usuario e pode ter outra coisa dentro.
#![cfg(unix)]

mod comum;
use comum::DirTemp;

use phxsql_store::backup;
use std::path::{Path, PathBuf};

const FILHO: &str = "PHX_576_FILHO_COM_ULIMIT";

/// Uma raiz com um arquivo pequeno que ordena PRIMEIRO (nasce antes da
/// recusa) e um grande que nao comprime, acima de qualquer `ulimit -f 16`.
fn raiz_com_dado(d: &Path) -> PathBuf {
    let raiz = d.join("dados");
    std::fs::create_dir_all(raiz.join("loja")).unwrap();
    std::fs::create_dir_all(raiz.join("outro")).unwrap();
    std::fs::write(raiz.join("loja/a.phx"), b"esquema pequeno").unwrap();
    std::fs::write(raiz.join("outro/b.reg"), b"registro pequeno").unwrap();
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let bytes: Vec<u8> = (0..256 * 1024)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    std::fs::write(raiz.join("loja/z.reg"), &bytes).unwrap();
    raiz
}

/// **A copia recusa no meio: a pasta que a corrida criou sai inteira, e a
/// de fora com o alheio dentro fica.**
///
/// O corpo roda num filho com `ulimit -f 16`; o pai confere o que sobrou. O
/// filho confere a premissa: a recusa tem de ser o `EFBIG` do `write`, senao
/// nenhuma copia teria nascido e a prova passaria por engano.
#[test]
fn a_copia_que_recusa_no_meio_nao_deixa_a_pasta() {
    if let Some(destino) = std::env::var_os(FILHO) {
        let destino = PathBuf::from(destino);
        let d = destino.parent().unwrap().parent().unwrap();
        let e = backup::executar(&d.join("dados"), &destino, 1_787_000_000_000)
            .expect_err("o ulimit tinha de recusar a copia grande");
        assert!(
            e.to_string().contains("too large") || e.to_string().contains("27"),
            "a recusa nao foi o EFBIG do write -- a premissa nao vale: {e}"
        );
        return;
    }
    let d = DirTemp::novo("576-copia-recusa");
    raiz_com_dado(&d);
    let pasta = d.join("copias");
    std::fs::create_dir_all(&pasta).unwrap();
    std::fs::write(pasta.join("alheio.txt"), b"do usuario").unwrap();
    let destino = pasta.join("2026-09-30");
    let eu = std::env::current_exe().unwrap();
    let saida = std::process::Command::new("sh")
        .arg("-c")
        .arg(
            "trap '' XFSZ; ulimit -f 16 && exec \"$0\" --exact \"$1\" --nocapture --test-threads=1",
        )
        .arg(&eu)
        .arg("a_copia_que_recusa_no_meio_nao_deixa_a_pasta")
        .env(FILHO, &destino)
        .output()
        .expect("reexecutar o teste com ulimit -f");
    let texto = format!(
        "{}\n{}",
        String::from_utf8_lossy(&saida.stdout),
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(saida.status.success(), "o filho reprovou:\n{texto}");
    assert!(texto.contains("1 passed"), "o filho nao rodou:\n{texto}");
    assert!(
        std::fs::symlink_metadata(&destino).is_err(),
        "a copia recusada deixou a pasta da corrida: {:?}",
        std::fs::read_dir(&destino).map(|d| d.flatten().map(|e| e.path()).collect::<Vec<_>>())
    );
    assert_eq!(
        std::fs::read(pasta.join("alheio.txt")).unwrap(),
        b"do usuario",
        "a faxina mexeu no que ja estava na pasta do usuario"
    );
}

/// **O manifesto recusa no fim: saem as copias e as pastas que NASCERAM,
/// fica tudo o que ja estava.**
///
/// O destino ja existe e tem dono: um arquivo alheio, uma pasta `loja/` com
/// uma copia antiga de mesmo nome (sobrescrita, mas o nome nao e desta
/// corrida) e um diretorio no nome `backup.json`, que faz o manifesto recusar.
#[test]
fn o_manifesto_que_recusa_leva_so_o_que_a_corrida_criou() {
    let d = DirTemp::novo("576-manifesto-recusa");
    let raiz = raiz_com_dado(&d);
    let destino = d.join("copias");
    std::fs::create_dir_all(destino.join("loja")).unwrap();
    std::fs::write(destino.join("alheio.txt"), b"do usuario").unwrap();
    std::fs::write(destino.join("loja/a.phx"), b"copia antiga").unwrap();
    std::fs::create_dir_all(destino.join("backup.json")).unwrap();
    std::fs::write(destino.join("backup.json/dentro"), b"nao e nosso").unwrap();

    let (r, copias) = backup::executar(&raiz, &destino, 1_787_000_000_000).unwrap();
    assert!(
        destino.join("loja/z.reg").is_file() && destino.join("outro/b.reg").is_file(),
        "a premissa: as copias nasceram antes do manifesto"
    );
    let e = backup::concluir(&destino, 1_787_000_000_000, &r, &copias)
        .expect_err("o diretorio no nome do manifesto tinha de recusar");

    assert!(
        !destino.join("loja/z.reg").exists(),
        "a copia nascida ficou sem manifesto: {e}"
    );
    assert!(
        std::fs::symlink_metadata(destino.join("outro")).is_err(),
        "a pasta que a corrida criou ficou: {e}"
    );
    assert!(destino.join("loja").is_dir(), "a pasta que ja existia saiu");
    assert!(
        destino.join("loja/a.phx").is_file(),
        "a faxina apagou um nome que ja existia antes da corrida"
    );
    assert_eq!(
        std::fs::read(destino.join("alheio.txt")).unwrap(),
        b"do usuario"
    );
    assert_eq!(
        std::fs::read(destino.join("backup.json/dentro")).unwrap(),
        b"nao e nosso"
    );
}
