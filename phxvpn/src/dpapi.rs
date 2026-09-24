//! DPAPI do Windows (`CryptProtectData`, do `crypt32` do sistema): o selo
//! que o proprio Windows usa para senha de Wi-Fi e do navegador.
//!
//! * escopo do USUARIO (`maquina = false`) -- so o mesmo usuario, na mesma
//!   maquina, abre: a senha lembrada do programa de mesa;
//! * escopo da MAQUINA (`maquina = true`) -- qualquer processo desta maquina
//!   abre, entao o arquivo que guarda o selo e que tem de ser fechado
//!   (SYSTEM e Administradores): os segredos dos servicos, que rodam como
//!   SYSTEM sem ninguem logado.

#![cfg(windows)]

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
const CRYPTPROTECT_LOCAL_MACHINE: u32 = 0x4;

fn chamar(dados: &[u8], entropia: &[u8], maquina: bool, proteger: bool) -> Result<Vec<u8>, String> {
    let entrada = Blob {
        tamanho: dados.len() as u32,
        dados: dados.as_ptr() as *mut u8,
    };
    let e = Blob {
        tamanho: entropia.len() as u32,
        dados: entropia.as_ptr() as *mut u8,
    };
    let mut saida = Blob {
        tamanho: 0,
        dados: std::ptr::null_mut(),
    };
    let bandeiras = CRYPTPROTECT_UI_FORBIDDEN
        | if maquina {
            CRYPTPROTECT_LOCAL_MACHINE
        } else {
            0
        };
    // SAFETY: blobs apontam para buffers vivos durante a chamada; a saida
    // e alocada pelo Windows e liberada com LocalFree logo abaixo.
    let ok = unsafe {
        if proteger {
            CryptProtectData(
                &entrada,
                std::ptr::null(),
                &e,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                bandeiras,
                &mut saida,
            )
        } else {
            CryptUnprotectData(
                &entrada,
                std::ptr::null_mut(),
                &e,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                bandeiras,
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

pub fn selar(dados: &[u8], entropia: &[u8], maquina: bool) -> Result<Vec<u8>, String> {
    chamar(dados, entropia, maquina, true)
}

pub fn abrir(selo: &[u8], entropia: &[u8], maquina: bool) -> Result<Vec<u8>, String> {
    chamar(selo, entropia, maquina, false)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn selo_da_maquina_e_do_usuario_voltam_e_entropia_errada_nao_abre() {
        for maquina in [false, true] {
            let s = selar(b"segredo", b"phxvpn-teste", maquina).unwrap();
            assert!(!s.windows(7).any(|w| w == b"segredo"), "segredo em claro");
            assert_eq!(abrir(&s, b"phxvpn-teste", maquina).unwrap(), b"segredo");
            assert!(
                abrir(&s, b"outra", maquina).is_err(),
                "abriu com entropia errada"
            );
        }
    }
}
