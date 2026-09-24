//! A placa virtual no Windows: o driver TAP-Windows6 do OpenVPN em modo TUN.
//!
//! Decisao do dono (23/09/2026): no Windows, usar o driver que o instalador do
//! OpenVPN ja poe na maquina. O papel J (pesquisa) escolheu o TAP-Windows6: e
//! o unico presente no OpenVPN 2.6 E no 2.7, em toda arquitetura; o Wintun
//! saiu na 2.7 e o ovpn-dco-win e acoplado ao protocolo do OpenVPN.
//!
//! # Como se fala com ele (so APIs do sistema, sem DLL nossa)
//!
//! 1. Achar o adaptador: no registro, a subchave de
//!    `...\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}` com
//!    `ComponentId = tap0901` da o GUID em `NetCfgInstanceId`; o nome que o
//!    usuario ve fica em `...\Control\Network\{4D36E972-...}\{GUID}\Connection`.
//! 2. Abrir `\\.\Global\{GUID}.tap` com `FILE_FLAG_OVERLAPPED` -- o acesso e
//!    EXCLUSIVO (o openvpn nao pode estar usando o mesmo adaptador), por isso
//!    o phxvpn usa um adaptador proprio, chamado `phxvpn`.
//! 3. `TAP_WIN_IOCTL_CONFIG_TUN` (0x220028) com `[ip_local, rede, mascara]`
//!    poe o driver em modo TUN: `ReadFile`/`WriteFile` passam IP cru (o driver
//!    poe e tira o Ethernet e responde o ARP sozinho).
//! 4. `TAP_WIN_IOCTL_SET_MEDIA_STATUS` (0x220018) com 1 «liga o cabo».
//! 5. Endereco e MTU da interface: `netsh`, que e do proprio Windows.
//!
//! # Por que overlapped
//!
//! O handle e um so para os dois sentidos. Num handle sincrono o Windows
//! serializa as chamadas: um `ReadFile` esperando pacote travaria o
//! `WriteFile` do outro sentido. Com overlapped, cada sentido tem o seu
//! `OVERLAPPED` e o seu evento.
//!
//! # O que NAO foi feito aqui
//!
//! Criar o adaptador: pede SetupAPI (1.372 linhas de C no `tapctl` de
//! referencia). O phxvpn chama o `tapctl.exe` que o OpenVPN ja instalou
//! (`phxvpn p2p placa`), uma vez, como administrador.
//!
//! **Nada disto rodou num Windows ainda**: compila para `x86_64-pc-windows-gnu`
//! e as pecas puras tem teste; o resto espera a prova na maquina.

use std::ffi::c_void;
use std::net::Ipv4Addr;
use std::sync::Mutex;

type Handle = *mut c_void;
const INVALIDO: Handle = -1isize as Handle;

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
const ERROR_IO_PENDING: i32 = 997;
const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
const KEY_READ: u32 = 0x20019;
const REG_SZ: u32 = 1;

pub use crate::tap::{
    codigo_tap, config_tun, de_largo, largo, IOCTL_CONFIG_TUN, IOCTL_SET_MEDIA_STATUS,
};

const CLASSE_REDE: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";
const CONEXOES_REDE: &str =
    r"SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}";

#[repr(C)]
struct Overlapped {
    interno: usize,
    interno_alto: usize,
    deslocamento: u32,
    deslocamento_alto: u32,
    evento: Handle,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        nome: *const u16,
        acesso: u32,
        compartilhar: u32,
        seguranca: *mut c_void,
        criacao: u32,
        bandeiras: u32,
        modelo: Handle,
    ) -> Handle;
    fn DeviceIoControl(
        h: Handle,
        codigo: u32,
        entrada: *const c_void,
        tam_entrada: u32,
        saida: *mut c_void,
        tam_saida: u32,
        devolvidos: *mut u32,
        ov: *mut Overlapped,
    ) -> i32;
    fn ReadFile(h: Handle, buf: *mut u8, tam: u32, lidos: *mut u32, ov: *mut Overlapped) -> i32;
    fn WriteFile(
        h: Handle,
        buf: *const u8,
        tam: u32,
        escritos: *mut u32,
        ov: *mut Overlapped,
    ) -> i32;
    fn GetOverlappedResult(h: Handle, ov: *mut Overlapped, feitos: *mut u32, esperar: i32) -> i32;
    fn CreateEventW(seg: *mut c_void, manual: i32, inicial: i32, nome: *const u16) -> Handle;
    fn CloseHandle(h: Handle) -> i32;
    fn WaitForSingleObject(h: Handle, ms: u32) -> u32;
    fn CancelIoEx(h: Handle, ov: *mut Overlapped) -> i32;
    fn GetLastError() -> u32;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        chave: isize,
        sub: *const u16,
        opcoes: u32,
        acesso: u32,
        saida: *mut isize,
    ) -> i32;
    fn RegEnumKeyExW(
        chave: isize,
        indice: u32,
        nome: *mut u16,
        tam: *mut u32,
        reservado: *mut u32,
        classe: *mut u16,
        tam_classe: *mut u32,
        hora: *mut c_void,
    ) -> i32;
    fn RegQueryValueExW(
        chave: isize,
        valor: *const u16,
        reservado: *mut u32,
        tipo: *mut u32,
        dados: *mut u8,
        tam: *mut u32,
    ) -> i32;
    fn RegCloseKey(chave: isize) -> i32;
}

struct Chave(isize);

impl Drop for Chave {
    fn drop(&mut self) {
        // SAFETY: chave aberta por RegOpenKeyExW e fechada uma vez so.
        unsafe { RegCloseKey(self.0) };
    }
}

fn abrir_chave(caminho: &str) -> Option<Chave> {
    let mut h = 0isize;
    let c = largo(caminho);
    // SAFETY: ponteiros para buffers vivos durante a chamada.
    (unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, c.as_ptr(), 0, KEY_READ, &mut h) } == 0)
        .then_some(Chave(h))
}

fn ler_texto(chave: &Chave, valor: &str) -> Option<String> {
    let v = largo(valor);
    let mut buf = vec![0u16; 512];
    let mut tam = (buf.len() * 2) as u32;
    let mut tipo = 0u32;
    // SAFETY: `buf` tem `tam` bytes e vive durante a chamada.
    let r = unsafe {
        RegQueryValueExW(
            chave.0,
            v.as_ptr(),
            std::ptr::null_mut(),
            &mut tipo,
            buf.as_mut_ptr() as *mut u8,
            &mut tam,
        )
    };
    (r == 0 && tipo == REG_SZ).then(|| de_largo(&buf[..(tam as usize / 2).min(buf.len())]))
}

/// GUIDs dos adaptadores TAP-Windows6 instalados, com o nome de cada um.
pub fn adaptadores_tap() -> Vec<(String, String)> {
    let Some(classe) = abrir_chave(CLASSE_REDE) else {
        return Vec::new();
    };
    let mut achados = Vec::new();
    for i in 0.. {
        let mut nome = vec![0u16; 256];
        let mut tam = nome.len() as u32;
        // SAFETY: `nome` tem `tam` posicoes; os demais ponteiros sao nulos.
        let r = unsafe {
            RegEnumKeyExW(
                classe.0,
                i,
                nome.as_mut_ptr(),
                &mut tam,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if r != 0 {
            break;
        }
        let sub = format!(r"{CLASSE_REDE}\{}", de_largo(&nome));
        let Some(k) = abrir_chave(&sub) else {
            continue;
        };
        if ler_texto(&k, "ComponentId").as_deref() != Some("tap0901") {
            continue;
        }
        let Some(guid) = ler_texto(&k, "NetCfgInstanceId") else {
            continue;
        };
        let apelido = abrir_chave(&format!(r"{CONEXOES_REDE}\{guid}\Connection"))
            .and_then(|c| ler_texto(&c, "Name"))
            .unwrap_or_default();
        achados.push((guid, apelido));
    }
    achados
}

struct Evento(Handle);
// SAFETY: um HANDLE de evento do Windows pode ser usado de qualquer thread.
unsafe impl Send for Evento {}

pub struct Tun {
    h: Handle,
    leitura: Mutex<Evento>,
    escrita: Mutex<Evento>,
    pub nome: String,
}

// SAFETY: o handle do dispositivo e usado por duas threads, cada uma com o
// proprio OVERLAPPED e evento (guardados atras de Mutex) -- e o uso que o
// Windows preve para um handle overlapped.
unsafe impl Send for Tun {}
unsafe impl Sync for Tun {}

fn erro(o_que: &str) -> String {
    // SAFETY: sem argumentos; le o erro da thread.
    format!("{o_que}: erro {} do Windows", unsafe { GetLastError() })
}

fn evento() -> Result<Evento, String> {
    // SAFETY: evento anonimo de reset manual, sem seguranca.
    let e = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
    if e.is_null() {
        return Err(erro("CreateEventW"));
    }
    Ok(Evento(e))
}

impl Tun {
    /// Abre o adaptador TAP chamado `nome` (o de `phxvpn p2p placa`), poe em
    /// modo TUN, liga o cabo e da endereco e MTU pelo `netsh`.
    pub fn abrir(nome: &str, ip: Ipv4Addr, prefixo: u8, mtu: u32) -> Result<Tun, String> {
        let lista = adaptadores_tap();
        let (guid, _) = lista
            .iter()
            .find(|(_, apelido)| apelido.eq_ignore_ascii_case(nome))
            .ok_or_else(|| {
                format!(
                    "adaptador TAP «{nome}» nao existe ({} TAP achado(s)). Crie uma vez, como \
                     administrador: phxvpn p2p placa --interface {nome}",
                    lista.len()
                )
            })?;
        let caminho = largo(&format!(r"\\.\Global\{guid}.tap"));
        // SAFETY: caminho terminado em zero; demais argumentos por valor.
        let h = unsafe {
            CreateFileW(
                caminho.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_SYSTEM | FILE_FLAG_OVERLAPPED,
                std::ptr::null_mut(),
            )
        };
        if h == INVALIDO {
            return Err(erro(&format!(
                "abrir o TAP «{nome}» (em uso pelo openvpn? o acesso e exclusivo)"
            )));
        }
        let tun = Tun {
            h,
            leitura: Mutex::new(evento()?),
            escrita: Mutex::new(evento()?),
            nome: nome.to_string(),
        };
        tun.ioctl(IOCTL_CONFIG_TUN, &config_tun(ip, prefixo), "CONFIG_TUN")?;
        tun.ioctl(
            IOCTL_SET_MEDIA_STATUS,
            &1u32.to_le_bytes(),
            "SET_MEDIA_STATUS",
        )?;
        let mascara = Ipv4Addr::from(if prefixo == 0 {
            0
        } else {
            u32::MAX << (32 - prefixo as u32)
        });
        netsh(&[
            "interface",
            "ip",
            "set",
            "address",
            &format!("name={nome}"),
            "static",
            &ip.to_string(),
            &mascara.to_string(),
        ])?;
        netsh(&[
            "interface",
            "ipv4",
            "set",
            "subinterface",
            nome,
            &format!("mtu={mtu}"),
            "store=active",
        ])?;
        Ok(tun)
    }

    fn ioctl(&self, codigo: u32, entrada: &[u8], o_que: &str) -> Result<(), String> {
        let ev = evento()?;
        let mut ov = Overlapped {
            interno: 0,
            interno_alto: 0,
            deslocamento: 0,
            deslocamento_alto: 0,
            evento: ev.0,
        };
        let mut saida = [0u8; 16];
        let mut n = 0u32;
        // SAFETY: buffers vivos durante a chamada e ate GetOverlappedResult.
        let ok = unsafe {
            DeviceIoControl(
                self.h,
                codigo,
                entrada.as_ptr() as *const c_void,
                entrada.len() as u32,
                saida.as_mut_ptr() as *mut c_void,
                saida.len() as u32,
                &mut n,
                &mut ov,
            ) != 0
                || (GetLastError() as i32 == ERROR_IO_PENDING
                    && GetOverlappedResult(self.h, &mut ov, &mut n, 1) != 0)
        };
        // SAFETY: evento criado acima, fechado uma vez.
        unsafe { CloseHandle(ev.0) };
        if ok {
            Ok(())
        } else {
            Err(erro(o_que))
        }
    }

    fn operar(&self, ler: bool, buf: *mut u8, tam: usize) -> std::io::Result<usize> {
        let trava = if ler { &self.leitura } else { &self.escrita };
        let ev = trava.lock().unwrap_or_else(|e| e.into_inner());
        let mut ov = Overlapped {
            interno: 0,
            interno_alto: 0,
            deslocamento: 0,
            deslocamento_alto: 0,
            evento: ev.0,
        };
        let mut n = 0u32;
        // SAFETY: `buf` tem `tam` bytes e vive ate GetOverlappedResult voltar
        // (esperando: o buffer nunca sai de cena com a operacao pendente).
        let ok = unsafe {
            let r = if ler {
                ReadFile(self.h, buf, tam as u32, &mut n, &mut ov)
            } else {
                WriteFile(self.h, buf, tam as u32, &mut n, &mut ov)
            };
            r != 0
                || (GetLastError() as i32 == ERROR_IO_PENDING
                    && GetOverlappedResult(self.h, &mut ov, &mut n, 1) != 0)
        };
        if ok {
            Ok(n as usize)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    /// Le um pacote, ou devolve `None` quando `parar` liga: espera o evento
    /// em fatias de meio segundo e, para desligar, cancela a leitura pendente
    /// (`CancelIoEx`) e ESPERA o cancelamento acabar -- o buffer nao pode sair
    /// de cena com o driver ainda escrevendo nele.
    pub fn ler_ou_parar(
        &self,
        buf: &mut [u8],
        parar: &std::sync::atomic::AtomicBool,
    ) -> std::io::Result<Option<usize>> {
        use std::sync::atomic::Ordering;
        const WAIT_OBJECT_0: u32 = 0;
        let ev = self.leitura.lock().unwrap_or_else(|e| e.into_inner());
        let mut ov = Overlapped {
            interno: 0,
            interno_alto: 0,
            deslocamento: 0,
            deslocamento_alto: 0,
            evento: ev.0,
        };
        let mut n = 0u32;
        // SAFETY: `buf` vive ate o GetOverlappedResult final (com espera).
        unsafe {
            if ReadFile(self.h, buf.as_mut_ptr(), buf.len() as u32, &mut n, &mut ov) != 0 {
                return Ok(Some(n as usize));
            }
            if GetLastError() as i32 != ERROR_IO_PENDING {
                return Err(std::io::Error::last_os_error());
            }
            loop {
                if WaitForSingleObject(ev.0, 500) == WAIT_OBJECT_0 {
                    break;
                }
                if parar.load(Ordering::Relaxed) {
                    CancelIoEx(self.h, &mut ov);
                    GetOverlappedResult(self.h, &mut ov, &mut n, 1);
                    return Ok(None);
                }
            }
            if GetOverlappedResult(self.h, &mut ov, &mut n, 1) != 0 {
                Ok(Some(n as usize))
            } else {
                Err(std::io::Error::last_os_error())
            }
        }
    }

    /// Le um pacote IP que o Windows mandou para a placa.
    pub fn ler(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.operar(true, buf.as_mut_ptr(), buf.len())
    }

    /// Entrega um pacote IP ao Windows.
    pub fn escrever(&self, pacote: &[u8]) -> std::io::Result<()> {
        self.operar(false, pacote.as_ptr() as *mut u8, pacote.len())
            .map(|_| ())
    }
}

impl Drop for Tun {
    fn drop(&mut self) {
        // SAFETY: handles abertos por este tipo, fechados uma vez so.
        unsafe {
            CloseHandle(
                self.leitura
                    .get_mut()
                    .map(|e| e.0)
                    .unwrap_or(std::ptr::null_mut()),
            );
            CloseHandle(
                self.escrita
                    .get_mut()
                    .map(|e| e.0)
                    .unwrap_or(std::ptr::null_mut()),
            );
            CloseHandle(self.h);
        }
    }
}

fn netsh(args: &[&str]) -> Result<(), String> {
    let s = std::process::Command::new("netsh")
        .args(args)
        .output()
        .map_err(|e| format!("netsh: {e}"))?;
    if s.status.success() {
        Ok(())
    } else {
        Err(format!(
            "netsh {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&s.stdout).trim()
        ))
    }
}

/// O `tapctl.exe` que o instalador do OpenVPN poe (padrao: Arquivos de
/// Programas\OpenVPN\bin).
pub fn achar_tapctl() -> Option<std::path::PathBuf> {
    let mut candidatos = Vec::new();
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Ok(base) = std::env::var(var) {
            candidatos.push(std::path::Path::new(&base).join(r"OpenVPN\bin\tapctl.exe"));
        }
    }
    candidatos
        .into_iter()
        .find(|c| c.is_file())
        .or_else(|| crate::supervisor::achar_no_path("tapctl"))
}

/// Cria o adaptador TAP do phxvpn pelo `tapctl.exe` do OpenVPN (uma vez,
/// como administrador). Devolve a saida dele (o GUID).
pub fn criar_placa(nome: &str) -> Result<String, String> {
    if adaptadores_tap()
        .iter()
        .any(|(_, a)| a.eq_ignore_ascii_case(nome))
    {
        return Ok(format!("o adaptador «{nome}» ja existe"));
    }
    let tapctl = achar_tapctl()
        .ok_or("tapctl.exe nao encontrado: instale o OpenVPN 2.6+ com o componente TAP-Windows6")?;
    let s = std::process::Command::new(&tapctl)
        .args(["create", "--hwid", r"root\tap0901", "--name", nome])
        .output()
        .map_err(|e| format!("{}: {e}", tapctl.display()))?;
    if !s.status.success() {
        return Err(format!(
            "tapctl falhou (rode como administrador): {}",
            String::from_utf8_lossy(&s.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&s.stdout).trim().to_string())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn overlapped_tem_o_tamanho_do_windows() {
        // Internal, InternalHigh (ULONG_PTR), Offset, OffsetHigh (DWORD), hEvent.
        let esperado = if cfg!(target_pointer_width = "64") {
            32
        } else {
            20
        };
        assert_eq!(std::mem::size_of::<Overlapped>(), esperado);
    }
}
