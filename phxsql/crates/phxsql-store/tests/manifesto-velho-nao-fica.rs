//! Backup que falha numa pasta REAPROVEITADA nao deixa o `backup.json` velho
//! mentindo sobre copias que ja mudaram -- pedido 577.
//!
//! Medido antes do conserto: a corrida que falha no meio ja sobrescreveu as
//! primeiras copias (o nome nao nasceu nela, entao a faxina do 576 nao as
//! tira, e esta certo nao tirar), e o manifesto da corrida ANTERIOR ficava no
//! lugar -- `op_backups` listava a pasta como backup pronto e o manifesto
//! trazia um SHA que nao batia mais com a copia.
//!
//! Junto, menor: a pasta que `executar_zip` cria ficava vazia quando ele
//! falhava.
//!
//! # Por que as falhas sao do sistema operacional, e nao forjadas
//!
//! O mesmo motivo dos irmaos (`parcial-do-zip-nao-fica.rs`, 555, e
//! `copia-que-falha-nao-fica.rs`, 576): a pergunta e «o que sobra quando o
//! DISCO recusa?». O corpo roda num filho com `ulimit -f 16`, e o `EFBIG` vem
//! do nucleo no meio do `write` do arquivo grande, depois de o pequeno ja ter
//! sido sobrescrito.
#![cfg(unix)]

mod comum;
use comum::DirTemp;

use phxsql_store::backup;
use std::path::{Path, PathBuf};

const FILHO: &str = "PHX_577_FILHO_COM_ULIMIT";
const QUANDO: i64 = 1_787_000_000_000;

/// Um arquivo pequeno que ordena PRIMEIRO (e sobrescrito antes da recusa) e
/// um grande que nao comprime, acima de qualquer `ulimit -f 16`.
fn raiz_com_dado(d: &Path) -> PathBuf {
    let raiz = d.join("dados");
    std::fs::create_dir_all(raiz.join("loja")).unwrap();
    std::fs::write(raiz.join("loja/a.phx"), b"esquema da primeira corrida").unwrap();
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

/// Reexecuta UM teste deste binario num filho com `ulimit -f 16`, com o
/// SIGXFSZ ignorado para o `write` devolver `EFBIG` em vez de matar o filho.
fn no_filho_com_ulimit(teste: &str, alvo: &Path) {
    let eu = std::env::current_exe().unwrap();
    let saida = std::process::Command::new("sh")
        .arg("-c")
        .arg(
            "trap '' XFSZ; ulimit -f 16 && exec \"$0\" --exact \"$1\" --nocapture --test-threads=1",
        )
        .arg(&eu)
        .arg(teste)
        .env(FILHO, alvo)
        .output()
        .expect("reexecutar o teste com ulimit -f");
    let texto = format!(
        "{}\n{}",
        String::from_utf8_lossy(&saida.stdout),
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(saida.status.success(), "o filho reprovou:\n{texto}");
    assert!(texto.contains("1 passed"), "o filho nao rodou:\n{texto}");
}

/// O filho confere a premissa: a recusa tem de ser o `EFBIG` do `write`,
/// senao nada teria sido escrito e a prova passaria por engano.
fn e_o_efbig(e: &phxsql_core::error::PhxError) {
    assert!(
        e.to_string().contains("too large") || e.to_string().contains("27"),
        "a recusa nao foi o EFBIG do write -- a premissa nao vale: {e}"
    );
}

/// **A corrida que falha numa pasta reaproveitada leva o manifesto velho.**
///
/// A primeira corrida termina inteira; a segunda, com o `a.phx` da raiz ja
/// mudado, sobrescreve a copia dele e recusa no `z.reg`. O que o pai confere:
/// nao ha `backup.json` descrevendo a copia nova com o SHA velho, e o
/// `restaurar` recusa a pasta dizendo por que, sem restaurar metade.
#[test]
fn a_corrida_que_falha_na_pasta_reaproveitada_leva_o_manifesto_velho() {
    if let Some(destino) = std::env::var_os(FILHO) {
        let destino = PathBuf::from(destino);
        let raiz = destino.parent().unwrap().join("dados");
        let e = backup::executar(&raiz, &destino, QUANDO)
            .expect_err("o ulimit tinha de recusar a copia grande");
        e_o_efbig(&e);
        return;
    }
    let d = DirTemp::novo("577-reaproveitada");
    let raiz = raiz_com_dado(&d);
    let destino = d.join("copia");
    let (r, copias) = backup::executar(&raiz, &destino, QUANDO).unwrap();
    backup::concluir(&destino, QUANDO, &r, &copias).unwrap();
    assert!(
        backup::conferir(&destino).unwrap().ok(),
        "a premissa: a primeira corrida deixou um backup integro"
    );

    std::fs::write(raiz.join("loja/a.phx"), b"esquema MUDADO depois").unwrap();
    no_filho_com_ulimit(
        "a_corrida_que_falha_na_pasta_reaproveitada_leva_o_manifesto_velho",
        &destino,
    );
    assert_eq!(
        std::fs::read(destino.join("loja/a.phx")).unwrap(),
        b"esquema MUDADO depois",
        "a premissa: a copia pequena foi sobrescrita antes da recusa"
    );

    assert!(
        std::fs::symlink_metadata(destino.join(backup::MANIFESTO)).is_err(),
        "o backup.json velho ficou descrevendo copias que ja mudaram: {:?}",
        backup::conferir(&destino).map(|r| r.divergencias)
    );
    let base = d.join("base");
    std::fs::create_dir_all(&base).unwrap();
    let e = phxsql_store::restaurar::Preparada::preparar(&destino, &base, "loja")
        .err()
        .expect("o restaurar tinha de recusar a pasta sem manifesto");
    assert!(
        e.to_string().contains(backup::MANIFESTO),
        "a recusa nao diz o que falta: {e}"
    );
    assert!(
        !base.join("loja").exists(),
        "o restaurar recusado deixou algo na base"
    );
}

/// **O zip que falha nao deixa a pasta que ele criou.**
///
/// As duas pastas (`zips/` e `zips/sub/`) nascem nesta chamada; a escrita do
/// `.part` recusa no meio. O `.part` ja saia (555); agora saem as pastas, e
/// a de cima -- que ja existia -- fica.
#[test]
fn o_zip_que_falha_nao_deixa_a_pasta_que_criou() {
    if let Some(pasta) = std::env::var_os(FILHO) {
        let pasta = PathBuf::from(pasta);
        let raiz = pasta.parent().unwrap().parent().unwrap().join("dados");
        let e = backup::executar_zip(&raiz, &pasta, "loja", "ana", QUANDO)
            .expect_err("o ulimit tinha de recusar a escrita do zip");
        e_o_efbig(&e);
        return;
    }
    let d = DirTemp::novo("577-zip-pasta");
    raiz_com_dado(&d);
    let pasta = d.join("zips/sub");
    no_filho_com_ulimit("o_zip_que_falha_nao_deixa_a_pasta_que_criou", &pasta);
    assert!(
        std::fs::symlink_metadata(d.join("zips")).is_err(),
        "o zip recusado deixou a pasta que criou: {:?}",
        std::fs::read_dir(d.join("zips"))
            .map(|l| l.flatten().map(|e| e.path()).collect::<Vec<_>>())
    );
    assert!(
        d.join("dados/loja/z.reg").is_file(),
        "a faxina mexeu na raiz"
    );
}
