//! `phxvpncmd` -- o console do phxvpn (modos Painel, P2P e Ferramentas).
//! O mesmo que `phxvpn cmd`: os dois chamam `phxvpn::console::principal`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = phxvpn::console::principal(&args) {
        eprintln!("phxvpncmd: {e}");
        std::process::exit(1);
    }
}
