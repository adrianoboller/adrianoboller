//! `phxvpnw` -- o programa de mesa do phxvpn, para abrir com dois cliques.
//!
//! No Windows nasce no subsistema de janelas (sem o console preto atras) e
//! fica na BANDEJA: duplo clique abre a janela, o menu tem «Sair». Com
//! `--bandeja` (o que o «abrir com o sistema» registra) sobe so na bandeja.
//! Sem console, o erro vai para `phxvpnw.log` na pasta do programa.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn rodar() -> Result<(), String> {
    let so_bandeja = std::env::args().any(|a| a == "--bandeja");
    let pasta = phxvpn::mesa::pasta_padrao();
    let preparada = phxvpn::mesa::preparar(pasta, 0)?;
    let url = preparada.url();
    if !so_bandeja {
        phxvpn::mesa::abrir_janela(&url);
    }
    #[cfg(windows)]
    {
        std::thread::spawn(move || preparada.servir());
        phxvpn::bandeja::rodar(url)
    }
    #[cfg(not(windows))]
    {
        eprintln!("phxvpnw: janela em {url}");
        preparada.servir()
    }
}

fn main() {
    if let Err(e) = rodar() {
        let pasta = phxvpn::mesa::pasta_padrao();
        let _ = std::fs::create_dir_all(&pasta);
        let _ = std::fs::write(pasta.join("phxvpnw.log"), format!("{e}\n"));
        eprintln!("phxvpnw: {e}");
        std::process::exit(1);
    }
}
