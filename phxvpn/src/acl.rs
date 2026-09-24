//! «Só o dono lê» -- o 0600/0700 do Linux, no Windows.
//!
//! No Linux o modo nasce na criacao do arquivo (`OpenOptionsExt::mode`). No
//! Windows o arquivo herda a ACL da pasta, e a de `%APPDATA%` ou de uma pasta
//! qualquer pode deixar Administradores, SYSTEM ou outros usuarios lerem a
//! chave P2P, a senha lembrada ou a chave do servidor. Aqui a DACL passa a
//! ter UMA entrada -- acesso total ao usuario do processo -- e fica
//! PROTEGIDA, isto e, sem herdar nada da pasta de cima.
//!
//! Aplicada ANTES de escrever o conteudo: entre criar o arquivo vazio e
//! gravar o segredo, a ACL ja e a certa. Em pasta, a entrada e herdavel
//! (arquivo e subpasta que nascerem dentro ja nascem so do dono).
//!
//! So APIs do sistema (`advapi32`), por FFI -- nenhuma biblioteca de fora.

use std::path::Path;

pub type R<T> = Result<T, String>;

#[cfg(not(windows))]
pub fn so_do_dono(_caminho: &Path) -> R<()> {
    // No Linux o modo ja nasce certo na criacao (0600 / 0700).
    Ok(())
}

#[cfg(windows)]
pub fn so_do_dono(caminho: &Path) -> R<()> {
    sistema::so_do_dono(caminho)
}

/// SYSTEM e Administradores, e mais ninguem (os segredos dos servicos).
#[cfg(windows)]
pub fn so_do_sistema(caminho: &Path) -> R<()> {
    sistema::so_do_sistema(caminho)
}

/// A DACL lida de volta: os SIDs com acesso (texto `S-1-...`), se ela e
/// protegida, e o SID do usuario do processo. So para provar.
#[cfg(windows)]
pub struct Lida {
    pub sids: Vec<String>,
    pub protegida: bool,
    pub eu: String,
}

#[cfg(windows)]
pub fn conferir(caminho: &Path) -> R<Lida> {
    sistema::conferir(caminho)
}

/// Roda sob o Wine? Ele guarda a ACL como modo Unix e a SINTETIZA na
/// leitura (SYSTEM + dono, sem a marca de protegida): a prova estrita so
/// vale no Windows de verdade.
#[cfg(windows)]
pub fn sob_wine() -> bool {
    sistema::sob_wine()
}

#[cfg(windows)]
mod sistema {
    use super::R;
    use crate::tap::largo;
    use std::ffi::c_void;
    use std::path::Path;

    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_USER: i32 = 1;
    const ACL_REVISION: u32 = 2;
    const FILE_ALL_ACCESS: u32 = 0x001F_01FF;
    const OBJECT_INHERIT_ACE: u32 = 0x1;
    const CONTAINER_INHERIT_ACE: u32 = 0x2;
    const SE_FILE_OBJECT: i32 = 1;
    const DACL_SECURITY_INFORMATION: u32 = 0x4;
    const SE_DACL_PROTECTED: u16 = 0x1000;

    #[repr(C)]
    struct AclCabecalho {
        revisao: u8,
        sbz1: u8,
        tamanho: u16,
        entradas: u16,
        sbz2: u16,
    }

    #[repr(C)]
    struct AceCabecalho {
        tipo: u8,
        bandeiras: u8,
        tamanho: u16,
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn OpenProcessToken(processo: *mut c_void, acesso: u32, token: *mut *mut c_void) -> i32;
        fn GetTokenInformation(
            token: *mut c_void,
            classe: i32,
            dados: *mut c_void,
            tam: u32,
            devolvido: *mut u32,
        ) -> i32;
        fn GetLengthSid(sid: *const c_void) -> u32;
        fn CreateWellKnownSid(
            tipo: i32,
            dominio: *const c_void,
            sid: *mut c_void,
            tam: *mut u32,
        ) -> i32;
        fn InitializeAcl(acl: *mut c_void, tam: u32, revisao: u32) -> i32;
        fn AddAccessAllowedAceEx(
            acl: *mut c_void,
            revisao: u32,
            bandeiras: u32,
            acesso: u32,
            sid: *const c_void,
        ) -> i32;
        fn InitializeSecurityDescriptor(sd: *mut c_void, revisao: u32) -> i32;
        fn SetSecurityDescriptorDacl(
            sd: *mut c_void,
            presente: i32,
            dacl: *const c_void,
            padrao: i32,
        ) -> i32;
        fn SetSecurityDescriptorControl(sd: *mut c_void, mascara: u16, valor: u16) -> i32;
        fn SetFileSecurityW(nome: *const u16, info: u32, sd: *const c_void) -> i32;
        fn GetNamedSecurityInfoW(
            nome: *const u16,
            tipo: i32,
            info: u32,
            dono: *mut *mut c_void,
            grupo: *mut *mut c_void,
            dacl: *mut *mut c_void,
            sacl: *mut *mut c_void,
            descritor: *mut *mut c_void,
        ) -> u32;
        fn GetSecurityDescriptorControl(
            descritor: *const c_void,
            controle: *mut u16,
            revisao: *mut u32,
        ) -> i32;
        fn GetAce(acl: *const c_void, indice: u32, ace: *mut *mut c_void) -> i32;
        fn ConvertSidToStringSidW(sid: *const c_void, texto: *mut *mut u16) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn CloseHandle(h: *mut c_void) -> i32;
        fn LocalFree(p: *mut c_void) -> *mut c_void;
        fn GetModuleHandleW(nome: *const u16) -> *mut c_void;
        fn GetProcAddress(m: *mut c_void, nome: *const u8) -> *mut c_void;
    }

    fn erro(onde: &str) -> String {
        format!("{onde}: {}", std::io::Error::last_os_error())
    }

    /// O TOKEN_USER do processo; o SID mora dentro do buffer devolvido.
    fn token_do_usuario() -> R<Vec<u64>> {
        let mut t = std::ptr::null_mut();
        // SAFETY: pseudo-handle do proprio processo; `t` recebe o token.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut t) } == 0 {
            return Err(erro("OpenProcessToken"));
        }
        // u64 para o alinhamento do ponteiro que vem dentro do TOKEN_USER.
        let mut buf = vec![0u64; 64];
        let mut n = 0u32;
        // SAFETY: `buf` tem 512 bytes; o Windows escreve ate `tam`.
        let ok = unsafe {
            GetTokenInformation(
                t,
                TOKEN_USER,
                buf.as_mut_ptr() as *mut c_void,
                (buf.len() * 8) as u32,
                &mut n,
            )
        };
        // SAFETY: token aberto acima.
        unsafe { CloseHandle(t) };
        if ok == 0 {
            return Err(erro("GetTokenInformation"));
        }
        Ok(buf)
    }

    /// O SID dentro do TOKEN_USER: o primeiro campo e o ponteiro para ele.
    fn sid_de(token: &[u64]) -> *const c_void {
        token[0] as usize as *const c_void
    }

    pub fn so_do_dono(caminho: &Path) -> R<()> {
        let token = token_do_usuario()?;
        aplicar(caminho, &[sid_de(&token)])
    }

    /// SYSTEM e Administradores: o arquivo que guarda um selo DPAPI de
    /// maquina (que qualquer processo desta maquina abriria, se lesse).
    pub fn so_do_sistema(caminho: &Path) -> R<()> {
        // WinLocalSystemSid = 22, WinBuiltinAdministratorsSid = 26.
        let mut sids = Vec::new();
        for tipo in [22, 26] {
            let mut b = vec![0u64; 12]; // SECURITY_MAX_SID_SIZE = 68
            let mut n = 96u32;
            // SAFETY: `b` tem 96 bytes; o Windows escreve ate `n`.
            if unsafe {
                CreateWellKnownSid(
                    tipo,
                    std::ptr::null(),
                    b.as_mut_ptr() as *mut c_void,
                    &mut n,
                )
            } == 0
            {
                return Err(erro("CreateWellKnownSid"));
            }
            sids.push(b);
        }
        let ptrs: Vec<*const c_void> = sids.iter().map(|b| b.as_ptr() as *const c_void).collect();
        aplicar(caminho, &ptrs)
    }

    fn aplicar(caminho: &Path, sids: &[*const c_void]) -> R<()> {
        let pasta = caminho.is_dir();
        // SAFETY: cada `sid` aponta memoria viva de quem chamou.
        let tam_sids: usize = sids
            .iter()
            .map(|s| unsafe { GetLengthSid(*s) } as usize)
            .sum();
        let tam = (std::mem::size_of::<AclCabecalho>() + sids.len() * 8 + tam_sids + 8) as u32;
        let mut acl = vec![0u32; (tam as usize).div_ceil(4)];
        let heranca = if pasta {
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
        } else {
            0
        };
        // SAFETY: `acl` tem `tam` bytes alinhados a 4.
        let mut ok =
            unsafe { InitializeAcl(acl.as_mut_ptr() as *mut c_void, tam, ACL_REVISION) != 0 };
        for sid in sids {
            // SAFETY: `acl` inicializada; `sid` valido.
            ok = ok
                && unsafe {
                    AddAccessAllowedAceEx(
                        acl.as_mut_ptr() as *mut c_void,
                        ACL_REVISION,
                        heranca,
                        FILE_ALL_ACCESS,
                        *sid,
                    ) != 0
                };
        }
        if !ok {
            return Err(erro("montar a ACL"));
        }
        // SetFileSecurityW, e nao SetNamedSecurityInfoW: a segunda, no Wine,
        // le a ACL e NUNCA a grava -- e devolve sucesso (medido pelo rastro
        // do servidor: nenhum `set_security_object`). A primeira vai direto
        // ao NtSetSecurityObject nos dois. A diferenca no Windows (nao
        // repassar heranca a filhos que ja existem) nao pesa: a ACL entra
        // na criacao.
        let mut sd = vec![0u64; 8]; // SECURITY_DESCRIPTOR: 40 bytes no x64
        let nome = largo(&caminho.to_string_lossy());
        // SAFETY: `sd` tem 64 bytes alinhados a 8; `acl` vive ate o fim.
        let ok = unsafe {
            let p = sd.as_mut_ptr() as *mut c_void;
            InitializeSecurityDescriptor(p, 1) != 0
                && SetSecurityDescriptorDacl(p, 1, acl.as_ptr() as *const c_void, 0) != 0
                && SetSecurityDescriptorControl(p, SE_DACL_PROTECTED, SE_DACL_PROTECTED) != 0
                && SetFileSecurityW(nome.as_ptr(), DACL_SECURITY_INFORMATION, p) != 0
        };
        if !ok {
            return Err(erro(&format!("restringir {} ao dono", caminho.display())));
        }
        Ok(())
    }

    fn texto_do_sid(sid: *const c_void) -> String {
        let mut p: *mut u16 = std::ptr::null_mut();
        // SAFETY: `sid` valido; `p` alocado pelo Windows, liberado abaixo.
        if unsafe { ConvertSidToStringSidW(sid, &mut p) } == 0 || p.is_null() {
            return "?".into();
        }
        let mut n = 0;
        // SAFETY: texto terminado em zero devolvido pelo Windows.
        while unsafe { *p.add(n) } != 0 {
            n += 1;
        }
        // SAFETY: `n` u16 validos a partir de `p`.
        let t = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) });
        // SAFETY: memoria do Windows, liberada uma vez.
        unsafe { LocalFree(p as *mut c_void) };
        t
    }

    pub fn sob_wine() -> bool {
        let ntdll = largo("ntdll.dll");
        // SAFETY: nome terminado em zero; so consulta.
        unsafe {
            let m = GetModuleHandleW(ntdll.as_ptr());
            !m.is_null() && !GetProcAddress(m, b"wine_get_version\0".as_ptr()).is_null()
        }
    }

    pub fn conferir(caminho: &Path) -> R<super::Lida> {
        let nome = largo(&caminho.to_string_lossy());
        let (mut dacl, mut desc) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: saidas recebem ponteiros para dentro de `desc`, liberado
        // com LocalFree no fim.
        let r = unsafe {
            GetNamedSecurityInfoW(
                nome.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut desc,
            )
        };
        if r != 0 {
            return Err(format!("ler a ACL de {}: erro {r}", caminho.display()));
        }
        let mut controle = 0u16;
        let mut rev = 0u32;
        // SAFETY: `desc` veio do Windows acima.
        unsafe { GetSecurityDescriptorControl(desc, &mut controle, &mut rev) };
        let mut sids = Vec::new();
        if !dacl.is_null() {
            // SAFETY: `dacl` aponta um ACL valido dentro de `desc`.
            let n = unsafe { (*(dacl as *const AclCabecalho)).entradas } as u32;
            for i in 0..n {
                let mut ace = std::ptr::null_mut();
                // SAFETY: indice < n; o SID de um ACCESS_ALLOWED_ACE comeca
                // depois do cabecalho (4) e da mascara (4).
                if unsafe { GetAce(dacl, i, &mut ace) } != 0 {
                    let sid =
                        unsafe { (ace as *const u8).add(std::mem::size_of::<AceCabecalho>() + 4) }
                            as *const c_void;
                    sids.push(texto_do_sid(sid));
                }
            }
        }
        // SAFETY: descritor alocado pelo Windows, liberado uma vez.
        unsafe { LocalFree(desc) };
        let token = token_do_usuario()?;
        Ok(super::Lida {
            sids,
            protegida: controle & SE_DACL_PROTECTED != 0,
            eu: texto_do_sid(sid_de(&token)),
        })
    }
}

#[cfg(all(test, windows))]
mod testes {
    use super::*;

    /// SIDs de grupos largos que nunca podem ficar num arquivo com chave:
    /// Todos, Usuarios autenticados, Usuarios.
    const LARGOS: [&str; 3] = ["S-1-1-0", "S-1-5-11", "S-1-5-32-545"];
    /// SYSTEM: o Wine o sintetiza em toda ACL que le de volta.
    const SISTEMA: &str = "S-1-5-18";

    fn so_do_dono_de_fato(l: &Lida) -> bool {
        if sob_wine() {
            l.sids.contains(&l.eu) && l.sids.iter().all(|s| *s == l.eu || s == SISTEMA)
        } else {
            l.sids == [l.eu.clone()] && l.protegida
        }
    }

    /// Antes: o arquivo herda da pasta e um grupo largo le. Depois: so o
    /// dono (no Windows: uma entrada, protegida; no Wine, que sintetiza a
    /// ACL do modo Unix, so o dono e o SYSTEM). Arquivo que nasce dentro da
    /// pasta protegida ja nasce assim.
    #[test]
    fn arquivo_e_pasta_ficam_so_do_dono() {
        let d = std::env::temp_dir().join(format!("phxvpn-acl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let a = d.join("segredo.chave");
        std::fs::write(&a, b"x").unwrap();
        let antes = conferir(&a).unwrap();
        assert!(
            antes.sids.iter().any(|s| LARGOS.contains(&s.as_str())),
            "a prova precisa de um arquivo que comece aberto: {:?}",
            antes.sids
        );
        so_do_dono(&a).unwrap();
        let depois = conferir(&a).unwrap();
        assert!(
            so_do_dono_de_fato(&depois),
            "{:?} (eu {})",
            depois.sids,
            depois.eu
        );
        so_do_dono(&d).unwrap();
        assert!(so_do_dono_de_fato(&conferir(&d).unwrap()));
        let filho = d.join("novo.txt");
        std::fs::write(&filho, b"y").unwrap();
        let f = conferir(&filho).unwrap();
        // O Wine nao implementa heranca de ACL (arquivo novo sai do umask):
        // la a heranca nao se prova -- e e por isso que cada segredo recebe
        // a sua ACL, sem depender dela.
        if !sob_wine() {
            assert!(
                f.sids.iter().all(|s| !LARGOS.contains(&s.as_str())),
                "filho herdou grupo largo: {:?}",
                f.sids
            );
        }
        let c = d.join("servico.cred");
        std::fs::write(&c, b"z").unwrap();
        so_do_sistema(&c).unwrap();
        let l = conferir(&c).unwrap();
        assert!(
            l.sids
                .iter()
                .all(|s| s == SISTEMA || s == "S-1-5-32-544" || (sob_wine() && *s == l.eu)),
            "credencial de servico aberta: {:?}",
            l.sids
        );
        assert!(!l.sids.iter().any(|s| LARGOS.contains(&s.as_str())));
        eprintln!(
            "acl: {} -- antes {:?}, depois {:?}",
            if sob_wine() {
                "Wine (prova do essencial)"
            } else {
                "Windows (prova estrita)"
            },
            antes.sids,
            depois.sids
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
