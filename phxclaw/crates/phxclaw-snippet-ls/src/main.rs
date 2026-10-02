#![forbid(unsafe_code)]
//! `phxclaw-snippet-ls`: o servidor de snippets e Emmet do IDE, no stdio. Entra no
//! `languages.toml` do Helix como servidor adicional (ver tools/instalar_helix.sh).

fn main() {
    let entrada = std::io::stdin();
    let saida = std::io::stdout();
    let mut e = entrada.lock();
    let mut s = saida.lock();
    if let Err(err) = phxclaw_snippet_ls::servir(&mut e, &mut s) {
        eprintln!("phxclaw-snippet-ls: {err}");
        std::process::exit(1);
    }
}
