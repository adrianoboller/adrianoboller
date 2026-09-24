//! `phxvpnw` -- o programa de mesa do phxvpn, para abrir com dois cliques.
//!
//! No Windows nasce no subsistema de janelas: sem o console preto atras. Por
//! isso o erro nao tem onde aparecer -- vai para `phxvpnw.log`, na pasta do
//! programa (`%APPDATA%\\phxvpn`).
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    let pasta = phxvpn::mesa::pasta_padrao();
    if let Err(e) = phxvpn::mesa::principal(pasta.clone(), 0, true) {
        let _ = std::fs::create_dir_all(&pasta);
        let _ = std::fs::write(pasta.join("phxvpnw.log"), format!("{e}\n"));
        eprintln!("phxvpnw: {e}");
        std::process::exit(1);
    }
}
