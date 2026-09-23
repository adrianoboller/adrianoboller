//! Mede a pilha que o analisador de JSON gasta por nivel de aninhamento.
//!
//! `pilha-do-json NIVEIS KIB` tenta analisar NIVEIS de `[` numa thread de KIB
//! de pilha e sai 0 se coube. Estouro de pilha e ABORT do processo, por isso
//! cada tentativa e um processo: quem procura o minimo e o laco de fora
//! (`docs/` do phxvpn, secao do teto de aninhamento).
use phxsql_core::json::Json;
fn main() {
    let a: Vec<usize> = std::env::args()
        .skip(1)
        .map(|x| x.parse().unwrap())
        .collect();
    let (niveis, kib) = (a[0], a[1]);
    let t = format!("{}{}", "[".repeat(niveis), "]".repeat(niveis));
    let ok = std::thread::Builder::new()
        .stack_size(kib * 1024)
        .spawn(move || Json::analisar(&t).is_ok())
        .unwrap()
        .join()
        .unwrap();
    std::process::exit(if ok { 0 } else { 1 });
}
