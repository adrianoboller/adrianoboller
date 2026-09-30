//! A linha da porta lida no MEIO da escrita -- pedido 581.
//!
//! O `permissao-do-banco::a_base_antiga_por_link_simbolico_tambem_alerta`
//! caiu uma vez com `AddrParseError` no `comum::porta_do_phxsqld`. A causa,
//! medida por `strace -e write` no `phxsqld`: o `eprintln!` da porta sai em
//! uma syscall por pedaco (`"porta de dados escutando em "`, `"127"`, `"."`,
//! ...), e quem le o arquivo entre duas delas ve a linha pela metade. No
//! servidor de verdade a janela e de microssegundos, e o laco de 20 ms quase
//! nunca cai nela -- por isso o floco.
//!
//! Aqui a janela e aberta de proposito: um `sh` no lugar do `phxsqld`
//! escreve a linha em dois pedacos com 400 ms no meio. O leitor que casava o
//! prefixo sem esperar o `\n` cai TODA vez; o que so le linha inteira espera
//! o resto.
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::process::{Command, Stdio};

/// Um "servidor" que escreve `primeiro`, espera, escreve `resto` e fica vivo.
fn falso(d: &DirTemp, primeiro: &str, resto: &str) -> Result<u16, String> {
    let erro_padrao = d.join("stderr.txt");
    let mut filho = Filho(
        Command::new("sh")
            .arg("-c")
            .arg("printf '%s' \"$0\" >&2; sleep 0.4; printf '%s' \"$1\" >&2; exec sleep 30")
            .arg(primeiro)
            .arg(resto)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .unwrap(),
    );
    porta_do_phxsqld(&mut filho, &erro_padrao)
}

/// **O endereco cortado no meio nao derruba a leitura.** Era o floco: o
/// prefixo ja estava inteiro, o endereco nao, e o `parse` recusava.
#[test]
fn o_endereco_pela_metade_espera_o_resto() {
    let d = DirTemp::novo("581-endereco");
    assert_eq!(
        falso(
            &d,
            "arrancando\nporta de dados escutando em 127.0.",
            "0.1:4321\n"
        ),
        Ok(4321)
    );
}

/// **A porta cortada no meio nao vira OUTRA porta.** O pior dos dois: o
/// `parse` de `127.0.0.1:43` passa, e o teste ia bater na porta 43.
#[test]
fn a_porta_pela_metade_nao_vira_outra_porta() {
    let d = DirTemp::novo("581-porta");
    assert_eq!(
        falso(&d, "porta de dados escutando em 127.0.0.1:43", "21\n"),
        Ok(4321)
    );
}

/// **A linha INTEIRA que nao se le e erro, e na hora.** Esperar mais nao a
/// consertaria; sem isto o conserto trocaria o floco por 20 s de espera.
#[test]
fn a_linha_inteira_ilegivel_e_erro_na_hora() {
    let d = DirTemp::novo("581-ilegivel");
    let r = falso(&d, "porta de dados escutando em lugar-nenhum\n", "");
    assert!(
        r.as_ref().is_err_and(|e| e.contains("lugar-nenhum")),
        "{r:?}"
    );
}
