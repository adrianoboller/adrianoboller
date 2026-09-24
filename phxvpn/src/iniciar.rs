//! «Abrir com o sistema»: o programa de mesa sobe sozinho quando o usuario
//! entra no computador.
//!
//! * **Windows** -- valor `phxvpn` em
//!   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, apontando para o
//!   `phxvpnw.exe --bandeja` (sobe direto na bandeja, sem abrir a janela).
//!   E o lugar que o proprio Windows lista em «Aplicativos de inicializacao».
//! * **Linux** -- `~/.config/autostart/phxvpn.desktop`, o padrao XDG que o
//!   GNOME, o KDE e o XFCE leem. O limite fica dito: ligar a rede exige
//!   `CAP_NET_ADMIN` (root, ou `setcap` no binario); sem isso, a janela abre e
//!   o «Ligar» explica por que nao liga.
//!
//! So HKCU / a pasta do usuario: nada de todos os usuarios, nada que peca
//! administrador.

pub type R<T> = Result<T, String>;

/// O comando que o sistema vai rodar: o `phxvpnw` ao lado deste executavel
/// (ou este mesmo, se ja for ele).
pub fn comando() -> R<String> {
    let este = std::env::current_exe().map_err(|e| e.to_string())?;
    let nome = if cfg!(windows) {
        "phxvpnw.exe"
    } else {
        "phxvpn"
    };
    let alvo = este
        .parent()
        .map(|d| d.join(nome))
        .filter(|c| c.is_file())
        .unwrap_or(este);
    let alvo = alvo.to_string_lossy();
    Ok(if cfg!(windows) {
        format!("\"{alvo}\" --bandeja")
    } else {
        format!("\"{alvo}\" mesa")
    })
}

#[cfg(windows)]
mod sistema {
    use crate::tap::largo;

    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    const KEY_ALL: u32 = 0xF003F;
    const REG_SZ: u32 = 1;
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALOR: &str = "phxvpn";

    #[link(name = "advapi32")]
    extern "system" {
        fn RegCreateKeyExW(
            chave: isize,
            sub: *const u16,
            reservado: u32,
            classe: *mut u16,
            opcoes: u32,
            acesso: u32,
            seguranca: *mut std::ffi::c_void,
            saida: *mut isize,
            disposicao: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            chave: isize,
            nome: *const u16,
            r: u32,
            tipo: u32,
            dados: *const u8,
            tam: u32,
        ) -> i32;
        fn RegQueryValueExW(
            chave: isize,
            nome: *const u16,
            r: *mut u32,
            tipo: *mut u32,
            dados: *mut u8,
            tam: *mut u32,
        ) -> i32;
        fn RegDeleteValueW(chave: isize, nome: *const u16) -> i32;
        fn RegCloseKey(chave: isize) -> i32;
    }

    fn abrir() -> Result<isize, String> {
        let mut h = 0isize;
        let sub = largo(RUN);
        // SAFETY: ponteiros para buffers vivos durante a chamada.
        let r = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                0,
                std::ptr::null_mut(),
                0,
                KEY_ALL,
                std::ptr::null_mut(),
                &mut h,
                std::ptr::null_mut(),
            )
        };
        if r == 0 {
            Ok(h)
        } else {
            Err(format!("registro (Run): erro {r}"))
        }
    }

    pub fn ligar(comando: &str) -> Result<(), String> {
        let h = abrir()?;
        let v = largo(comando);
        let nome = largo(VALOR);
        // SAFETY: `v` e UTF-16 terminado em zero, com o tamanho em bytes.
        let r = unsafe {
            RegSetValueExW(
                h,
                nome.as_ptr(),
                0,
                REG_SZ,
                v.as_ptr() as *const u8,
                (v.len() * 2) as u32,
            )
        };
        // SAFETY: chave aberta acima.
        unsafe { RegCloseKey(h) };
        if r == 0 {
            Ok(())
        } else {
            Err(format!("registro (Run): erro {r}"))
        }
    }

    pub fn desligar() -> Result<(), String> {
        let h = abrir()?;
        let nome = largo(VALOR);
        // SAFETY: chave aberta acima; nome terminado em zero.
        let r = unsafe { RegDeleteValueW(h, nome.as_ptr()) };
        unsafe { RegCloseKey(h) };
        // 2 = ERROR_FILE_NOT_FOUND: ja estava desligado.
        if r == 0 || r == 2 {
            Ok(())
        } else {
            Err(format!("registro (Run): erro {r}"))
        }
    }

    pub fn ligado() -> Option<String> {
        let h = abrir().ok()?;
        let nome = largo(VALOR);
        let mut buf = vec![0u16; 1024];
        let mut tam = (buf.len() * 2) as u32;
        let mut tipo = 0u32;
        // SAFETY: `buf` tem `tam` bytes.
        let r = unsafe {
            RegQueryValueExW(
                h,
                nome.as_ptr(),
                std::ptr::null_mut(),
                &mut tipo,
                buf.as_mut_ptr() as *mut u8,
                &mut tam,
            )
        };
        unsafe { RegCloseKey(h) };
        (r == 0 && tipo == REG_SZ).then(|| crate::tap::de_largo(&buf))
    }
}

#[cfg(not(windows))]
mod sistema {
    use std::path::PathBuf;

    fn arquivo() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
            });
        base.join("autostart").join("phxvpn.desktop")
    }

    pub fn conteudo(comando: &str) -> String {
        format!(
            "[Desktop Entry]\nType=Application\nName=phxvpn\nComment=Redes virtuais phxvpn\n\
             Exec={comando}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
        )
    }

    pub fn ligar(comando: &str) -> Result<(), String> {
        let a = arquivo();
        if let Some(d) = a.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&a, conteudo(comando)).map_err(|e| format!("{}: {e}", a.display()))
    }

    pub fn desligar() -> Result<(), String> {
        match std::fs::remove_file(arquivo()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn ligado() -> Option<String> {
        let t = std::fs::read_to_string(arquivo()).ok()?;
        t.lines()
            .find_map(|l| l.strip_prefix("Exec="))
            .map(str::to_string)
    }
}

pub fn ligar() -> R<()> {
    sistema::ligar(&comando()?)
}

pub fn desligar() -> R<()> {
    sistema::desligar()
}

/// O comando registrado, se «abrir com o sistema» esta ligado.
pub fn ligado() -> Option<String> {
    sistema::ligado()
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Liga, confere e desliga -- no registro do Windows (rodado sob o Wine)
    /// ou na pasta de autostart (com `XDG_CONFIG_HOME` temporario).
    #[test]
    fn liga_confere_e_desliga() {
        #[cfg(not(windows))]
        {
            let d = std::env::temp_dir().join(format!("phxvpn-xdg-{}", std::process::id()));
            std::env::set_var("XDG_CONFIG_HOME", &d);
        }
        desligar().unwrap();
        assert_eq!(ligado(), None);
        ligar().unwrap();
        let c = ligado().expect("tinha de estar ligado");
        assert!(c.contains("phxvpn"), "{c}");
        assert!(
            if cfg!(windows) {
                c.ends_with("--bandeja")
            } else {
                c.ends_with(" mesa")
            },
            "{c}"
        );
        desligar().unwrap();
        assert_eq!(ligado(), None);
    }
}
