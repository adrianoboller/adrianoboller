//! O `.part` de um ZIP de backup que falhou nao fica na pasta -- pedido 555.
//!
//! Medido antes do conserto: o `.part` so vira `.zip` no `rename` de
//! `finalizar_zip`, e a rotacao so reconhece o nome final. Uma recusa no meio
//! deixava o `.part` para sempre -- um zip inteiro por falha, somando na
//! pasta de destino sem ninguem apagar.
//!
//! # Por que as falhas sao do sistema operacional, e nao forjadas
//!
//! Porque a pergunta e «o que sobra na pasta quando o DISCO recusa?», e a
//! resposta so vale com a recusa vindo de quem recusa de verdade. As duas
//! provas abaixo derrubam os dois passos que escrevem o `.part`:
//!
//! - a ESCRITA, num processo filho com `ulimit -f` (o nucleo devolve `EFBIG`
//!   no meio do `write`, com o `SIGXFSZ` ignorado para o filho nao morrer);
//! - o `rename` final, com um DIRETORIO no nome do zip (`EISDIR`).
#![cfg(unix)]

mod comum;
use comum::DirTemp;

use phxsql_store::backup;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const FILHO: &str = "PHX_555_FILHO_COM_ULIMIT";

/// Uma raiz com dado que nao comprime: o zip passa com folga de qualquer
/// `ulimit -f` pequeno, seja a unidade de 512 ou de 1024 bytes.
fn raiz_com_dado(d: &Path) -> PathBuf {
    let raiz = d.join("dados");
    std::fs::create_dir_all(raiz.join("loja")).unwrap();
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let bytes: Vec<u8> = (0..256 * 1024)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    std::fs::write(raiz.join("loja/clientes.reg"), &bytes).unwrap();
    raiz
}

fn parciais(pasta: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(pasta)
        .map(|d| {
            d.flatten()
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|n| n.ends_with(".part"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// **A escrita recusa no meio: o `.part` sai junto com o erro.**
///
/// O corpo roda num filho com `ulimit -f 16` -- o limite so vale para ele, e
/// o pai confere o que sobrou na pasta. O filho confere a premissa antes:
/// a recusa tem de ser do `write` (`EFBIG`), nao do `open`, senao o `.part`
/// nunca teria nascido e a prova passaria por engano.
#[test]
fn a_escrita_que_recusa_no_meio_nao_deixa_o_part() {
    let rotulo = "555-escrita-recusa";
    if let Some(pasta) = std::env::var_os(FILHO) {
        let pasta = PathBuf::from(pasta);
        let raiz = pasta.parent().unwrap().join("dados");
        let e = backup::executar_zip(&raiz, &pasta, "loja", "ana", 1_787_000_000_000)
            .expect_err("o ulimit tinha de recusar a escrita do zip");
        assert!(
            e.to_string().contains("too large") || e.to_string().contains("27"),
            "a recusa nao foi o EFBIG do write -- a premissa nao vale: {e}"
        );
        return;
    }
    let d = DirTemp::novo(rotulo);
    raiz_com_dado(&d);
    let pasta = d.join("zips");
    std::fs::create_dir_all(&pasta).unwrap();
    let eu = std::env::current_exe().unwrap();
    let saida = std::process::Command::new("sh")
        .arg("-c")
        .arg(
            "trap '' XFSZ; ulimit -f 16 && exec \"$0\" --exact \"$1\" --nocapture --test-threads=1",
        )
        .arg(&eu)
        .arg("a_escrita_que_recusa_no_meio_nao_deixa_o_part")
        .env(FILHO, &pasta)
        .output()
        .expect("reexecutar o teste com ulimit -f");
    let texto = format!(
        "{}\n{}",
        String::from_utf8_lossy(&saida.stdout),
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(saida.status.success(), "o filho reprovou:\n{texto}");
    assert!(texto.contains("1 passed"), "o filho nao rodou:\n{texto}");
    assert_eq!(
        parciais(&pasta),
        Vec::<String>::new(),
        "a escrita recusada no meio deixou o .part na pasta"
    );
}

/// **O `rename` final recusa: o `.part` sai junto com o erro.**
///
/// Um diretorio com o nome do zip faz o `rename` do arquivo por cima dele
/// recusar (`EISDIR`) -- a falha do ultimo passo, com o zip inteiro ja
/// escrito e sincronizado no `.part`.
#[test]
fn o_rename_que_recusa_nao_deixa_o_part() {
    let d = DirTemp::novo("555-rename-recusa");
    let raiz = raiz_com_dado(&d);
    let pasta = d.join("zips");
    let (zip, _) = backup::executar_zip(&raiz, &pasta, "loja", "ana", 1_787_000_000_000).unwrap();
    assert_eq!(parciais(&pasta).len(), 1, "a premissa: o .part nasceu");
    std::fs::create_dir_all(zip.join("ocupado")).unwrap();

    let e =
        backup::finalizar_zip(&zip).expect_err("o rename por cima do diretorio tinha de recusar");
    assert!(zip.is_dir(), "o diretorio do nome final mudou: {e}");
    assert_eq!(
        parciais(&pasta),
        Vec::<String>::new(),
        "o rename recusado deixou o .part na pasta: {e}"
    );
}

fn envelhecer(p: &Path, idade: Duration) {
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(SystemTime::now() - idade)
        .unwrap();
}

/// **A faxina da proxima corrida so apaga o `.part` orfao que e NOSSO.**
///
/// O `.part` deixado por uma corrida que caiu no meio (o processo morreu,
/// entao nenhum caminho de erro rodou) sai no proximo backup que termina. E
/// fica tudo o que nao e comprovadamente nosso: nome de outro formato, link
/// simbolico com o nosso nome (nem ele nem o alvo mudam), FIFO com o nosso
/// nome (e o backup nao para abrindo-a), e o `.part` NOVO -- que pode ser de
/// um backup vizinho ainda no `fsync`, fora da trava.
#[test]
fn a_proxima_corrida_limpa_so_o_part_orfao_que_e_nosso() {
    let d = DirTemp::novo("555-faxina");
    let raiz = raiz_com_dado(&d);
    let pasta = d.join("zips");
    std::fs::create_dir_all(&pasta).unwrap();

    let velho = Duration::from_secs(2 * 3600);
    let orfao = pasta.join("loja_ana_2026-09-01_0300.zip.part");
    std::fs::write(&orfao, b"zip pela metade").unwrap();
    envelhecer(&orfao, velho);

    let novo = pasta.join("loja_bia_2026-09-01_0301.zip.part");
    std::fs::write(&novo, b"zip de um vizinho no fsync").unwrap();

    let alheio = pasta.join("download-do-navegador.part");
    std::fs::write(&alheio, b"nao e nosso").unwrap();
    envelhecer(&alheio, velho);

    let vitima = d.join("vitima.txt");
    std::fs::write(&vitima, b"fora da pasta").unwrap();
    envelhecer(&vitima, velho);
    let link = pasta.join("loja_eve_2026-09-01_0302.zip.part");
    std::os::unix::fs::symlink(&vitima, &link).unwrap();

    let fifo = pasta.join("loja_eve_2026-09-01_0303.zip.part");
    let tem_fifo = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    // O backup roda numa thread com prazo: com a FIFO aberta por engano, o
    // defeito reprova em 10 s em vez de pendurar a bateria.
    let (tx, rx) = std::sync::mpsc::channel();
    let (r2, p2) = (raiz.clone(), pasta.clone());
    std::thread::spawn(move || {
        let (zip, _) = backup::executar_zip(&r2, &p2, "loja", "ana", 1_787_000_000_000).unwrap();
        let _ = tx.send(backup::finalizar_zip(&zip).map(|()| zip));
    });
    let zip = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("o backup parou abrindo a FIFO com o nome de um .part")
        .unwrap();

    assert!(zip.is_file(), "o backup desta corrida tinha de terminar");
    assert!(!orfao.exists(), "o .part orfao nosso ficou na pasta");
    assert!(novo.is_file(), "a faxina apagou o .part NOVO de um vizinho");
    assert!(alheio.is_file(), "a faxina apagou arquivo de outro formato");
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "a faxina mexeu no link simbolico com o nosso nome"
    );
    assert_eq!(std::fs::read(&vitima).unwrap(), b"fora da pasta");
    if tem_fifo {
        assert!(
            std::fs::symlink_metadata(&fifo).is_ok(),
            "a faxina apagou a FIFO com o nosso nome"
        );
    }
}
