//! «Lembrar a senha» do programa de mesa: o segredo DERIVADO (a PSK da rede e
//! a credencial do intermediario), nunca a senha, guardado selado.
//!
//! # Onde e como
//!
//! * **Windows** -- DPAPI (`CryptProtectData`, do `crypt32` do sistema): so o
//!   mesmo usuario do Windows, na mesma maquina, abre. E o que o proprio
//!   Windows usa para senhas de Wi-Fi e do navegador.
//! * **Linux** -- XChaCha20-Poly1305 com uma chave local de 32 bytes em
//!   `lembrar.chave` (0600). Sem chaveiro do sistema (seria biblioteca de
//!   fora), o LIMITE fica dito: protege contra copiarem so o `.lembrada`, nao
//!   contra quem ja entra na conta do usuario -- esse le a chave tambem.
//!
//! O arquivo `<rede>.lembrada` (0600) carrega o selo; «Esquecer» o apaga.

use crate::comandos::gravar_secreto;
use phxsql_core::hash::{de_hex, para_hex};
use phxsql_core::json::Json;
use std::path::Path;

pub type R<T> = Result<T, String>;

/// O que se lembra de uma rede.
#[derive(Clone, Debug, PartialEq)]
pub struct Lembrado {
    pub psk: [u8; 32],
    /// (usuario, credencial) do servidor intermediario.
    pub repasse: Option<(String, [u8; 32])>,
}

fn arquivo(pasta: &Path, rede: &str) -> String {
    let base = crate::rede_p2p::Rede::caminho(rede);
    pasta
        .join(base.trim_end_matches(".p2p").to_string() + ".lembrada")
        .to_string_lossy()
        .into_owned()
}

fn para_bytes(l: &Lembrado) -> Vec<u8> {
    let mut campos = vec![("psk", Json::texto_de(para_hex(&l.psk)))];
    if let Some((u, c)) = &l.repasse {
        campos.push(("repasse_usuario", Json::texto_de(u.clone())));
        campos.push(("repasse_credencial", Json::texto_de(para_hex(c))));
    }
    Json::objeto(campos).escrever().into_bytes()
}

fn de_bytes(b: &[u8]) -> R<Lembrado> {
    let j = Json::analisar(std::str::from_utf8(b).map_err(|_| "segredo torto")?)
        .map_err(|e| e.to_string())?;
    let k32 = |t: &str| -> R<[u8; 32]> {
        de_hex(t)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| "segredo torto".to_string())
    };
    let repasse = match (
        j.campo("repasse_usuario").and_then(Json::texto),
        j.campo("repasse_credencial").and_then(Json::texto),
    ) {
        (Some(u), Some(c)) => Some((u.to_string(), k32(c)?)),
        _ => None,
    };
    Ok(Lembrado {
        psk: k32(j.texto_ou("psk", ""))?,
        repasse,
    })
}

pub fn guardar(pasta: &Path, rede: &str, l: &Lembrado) -> R<()> {
    let selo = selar(pasta, &para_bytes(l))?;
    let a = arquivo(pasta, rede);
    let _ = std::fs::remove_file(&a);
    gravar_secreto(&a, para_hex(&selo).as_bytes(), true)
}

pub fn ler(pasta: &Path, rede: &str) -> Option<Lembrado> {
    let t = std::fs::read_to_string(arquivo(pasta, rede)).ok()?;
    let selo = de_hex(t.trim())?;
    de_bytes(&abrir(pasta, &selo).ok()?).ok()
}

pub fn existe(pasta: &Path, rede: &str) -> bool {
    Path::new(&arquivo(pasta, rede)).exists()
}

pub fn esquecer(pasta: &Path, rede: &str) -> R<()> {
    match std::fs::remove_file(arquivo(pasta, rede)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// ------------------------------------------------------------- Windows ----

#[cfg(windows)]
mod sistema {
    use std::ffi::c_void;

    #[repr(C)]
    struct Blob {
        tamanho: u32,
        dados: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            entrada: *const Blob,
            descricao: *const u16,
            entropia: *const Blob,
            reservado: *mut c_void,
            aviso: *mut c_void,
            bandeiras: u32,
            saida: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            entrada: *const Blob,
            descricao: *mut *mut u16,
            entropia: *const Blob,
            reservado: *mut c_void,
            aviso: *mut c_void,
            bandeiras: u32,
            saida: *mut Blob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(p: *mut c_void) -> *mut c_void;
    }

    /// Sem interface: nunca abre janela de aviso do Windows.
    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    fn chamar(dados: &[u8], proteger: bool) -> Result<Vec<u8>, String> {
        let entrada = Blob {
            tamanho: dados.len() as u32,
            dados: dados.as_ptr() as *mut u8,
        };
        // Entropia fixa do phxvpn: outro programa do mesmo usuario, chamando
        // a DPAPI sem ela, nao abre o selo por engano.
        let e = b"phxvpn-lembrar-v1";
        let entropia = Blob {
            tamanho: e.len() as u32,
            dados: e.as_ptr() as *mut u8,
        };
        let mut saida = Blob {
            tamanho: 0,
            dados: std::ptr::null_mut(),
        };
        // SAFETY: blobs apontam para buffers vivos durante a chamada; a saida
        // e alocada pelo Windows e liberada com LocalFree logo abaixo.
        let ok = unsafe {
            if proteger {
                CryptProtectData(
                    &entrada,
                    std::ptr::null(),
                    &entropia,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut saida,
                )
            } else {
                CryptUnprotectData(
                    &entrada,
                    std::ptr::null_mut(),
                    &entropia,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut saida,
                )
            }
        };
        if ok == 0 {
            return Err(format!(
                "DPAPI recusou: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: `saida` foi preenchida pela DPAPI com `tamanho` bytes.
        let v = unsafe { std::slice::from_raw_parts(saida.dados, saida.tamanho as usize).to_vec() };
        // SAFETY: memoria alocada pela DPAPI, liberada uma vez.
        unsafe { LocalFree(saida.dados as *mut c_void) };
        Ok(v)
    }

    pub fn selar(_pasta: &std::path::Path, dados: &[u8]) -> Result<Vec<u8>, String> {
        chamar(dados, true)
    }

    pub fn abrir(_pasta: &std::path::Path, selo: &[u8]) -> Result<Vec<u8>, String> {
        chamar(selo, false)
    }
}

// --------------------------------------------------------------- Linux ----

#[cfg(not(windows))]
mod sistema {
    use phxsql_core::cifra::{xabrir, xselar};
    use phxsql_core::senha::bytes_aleatorios;
    use std::path::Path;

    fn chave(pasta: &Path) -> Result<[u8; 32], String> {
        let c = pasta.join("lembrar.chave");
        let caminho = c.to_string_lossy();
        if let Ok(b) = std::fs::read(&c) {
            return b.try_into().map_err(|_| "lembrar.chave torta".to_string());
        }
        let k: [u8; 32] = bytes_aleatorios(32).try_into().expect("32");
        crate::comandos::gravar_secreto(&caminho, &k, true)?;
        Ok(k)
    }

    pub fn selar(pasta: &Path, dados: &[u8]) -> Result<Vec<u8>, String> {
        let nonce: [u8; 24] = bytes_aleatorios(24).try_into().expect("24");
        let (mut c, tag) = xselar(&chave(pasta)?, &nonce, b"phxvpn-lembrar-v1", dados);
        c.extend_from_slice(&tag);
        let mut s = nonce.to_vec();
        s.extend_from_slice(&c);
        Ok(s)
    }

    pub fn abrir(pasta: &Path, selo: &[u8]) -> Result<Vec<u8>, String> {
        if selo.len() < 40 {
            return Err("selo curto".into());
        }
        let nonce: [u8; 24] = selo[..24].try_into().expect("24");
        let corte = selo.len() - 16;
        let tag: [u8; 16] = selo[corte..].try_into().expect("16");
        xabrir(
            &chave(pasta)?,
            &nonce,
            b"phxvpn-lembrar-v1",
            &selo[24..corte],
            &tag,
        )
        .map_err(|e| e.to_string())
    }
}

use sistema::{abrir, selar};

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn guarda_le_e_esquece_sem_senha_em_claro() {
        let d = std::env::temp_dir().join(format!("phxvpn-lembrar-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let l = Lembrado {
            psk: [7; 32],
            repasse: Some(("filial-a".into(), [9; 32])),
        };
        guardar(&d, "Matriz", &l).unwrap();
        assert!(existe(&d, "Matriz"));
        assert_eq!(ler(&d, "Matriz"), Some(l));
        let bruto = std::fs::read_to_string(arquivo(&d, "Matriz")).unwrap();
        assert!(
            !bruto.contains(&para_hex(&[7u8; 32])),
            "a PSK apareceu em claro"
        );
        // Trocar a chave local (outro computador/usuario): nao abre.
        #[cfg(not(windows))]
        {
            std::fs::remove_file(d.join("lembrar.chave")).unwrap();
            assert_eq!(ler(&d, "Matriz"), None);
        }
        esquecer(&d, "Matriz").unwrap();
        assert!(!existe(&d, "Matriz"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
