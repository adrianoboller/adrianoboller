//! A placa de rede virtual (TUN) do Linux, por FFI direto ao `ioctl` -- sem
//! crate, como a petrea manda. O resto (abrir, ler, escrever) e `std::fs`.
//!
//! Cada `read` devolve UM pacote IP que o sistema mandou para a placa; cada
//! `write` entrega UM pacote IP ao sistema como se tivesse chegado por ela.
//! `IFF_NO_PI` tira o cabecalho de 4 bytes de protocolo: sai IP cru.
//!
//! Endereco, mascara, MTU e «ligar» vao por `ioctl` num soquete UDP qualquer
//! (o `UdpSocket` da std serve de descritor AF_INET) -- e o que o `ip addr`
//! faz por baixo, sem precisar do `iproute2` instalado.
//!
//! Precisa de `CAP_NET_ADMIN` (root, ou a capacidade dada ao binario).

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, UdpSocket};
use std::os::fd::AsRawFd;

extern "C" {
    fn ioctl(fd: i32, pedido: std::ffi::c_ulong, ...) -> i32;
    fn poll(fds: *mut PollFd, n: std::ffi::c_ulong, espera_ms: i32) -> i32;
}

#[repr(C)]
struct PollFd {
    fd: i32,
    eventos: i16,
    devolvidos: i16,
}
const POLLIN: i16 = 0x1;

const TUNSETIFF: std::ffi::c_ulong = 0x4004_54ca;
const IFF_TUN: i16 = 0x0001;
const IFF_NO_PI: i16 = 0x1000;
const SIOCSIFADDR: std::ffi::c_ulong = 0x8916;
const SIOCSIFNETMASK: std::ffi::c_ulong = 0x891c;
const SIOCGIFFLAGS: std::ffi::c_ulong = 0x8913;
const SIOCSIFFLAGS: std::ffi::c_ulong = 0x8914;
const SIOCSIFMTU: std::ffi::c_ulong = 0x8922;
const IFF_UP: i16 = 0x1;
const IFF_RUNNING: i16 = 0x40;

/// `struct ifreq` do Linux: 16 bytes de nome e 24 de uniao (40 no total em
/// 64 bits). So se usa o que cada pedido le.
#[repr(C)]
struct IfReq {
    nome: [u8; 16],
    uniao: [u8; 24],
}

impl IfReq {
    fn novo(nome: &str) -> Result<IfReq, String> {
        if nome.is_empty() || nome.len() > 15 || !nome.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(format!("nome de interface invalido: {nome:?}"));
        }
        let mut r = IfReq {
            nome: [0; 16],
            uniao: [0; 24],
        };
        r.nome[..nome.len()].copy_from_slice(nome.as_bytes());
        Ok(r)
    }

    fn por_endereco(nome: &str, ip: Ipv4Addr) -> Result<IfReq, String> {
        let mut r = IfReq::novo(nome)?;
        // sockaddr_in: familia (AF_INET = 2), porta 0, endereco.
        r.uniao[..2].copy_from_slice(&2u16.to_ne_bytes());
        r.uniao[4..8].copy_from_slice(&ip.octets());
        Ok(r)
    }
}

fn chamar(fd: i32, pedido: std::ffi::c_ulong, req: &mut IfReq, o_que: &str) -> Result<(), String> {
    // SAFETY: `req` e um `ifreq` valido do tamanho que o kernel le, vivo e
    // exclusivo durante a chamada; `fd` e um descritor aberto por nos.
    let r = unsafe { ioctl(fd, pedido, req as *mut IfReq) };
    if r < 0 {
        return Err(format!("{o_que}: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

pub struct Tun {
    arquivo: File,
    pub nome: String,
}

impl Tun {
    /// Cria (ou pega) a interface `nome`, da endereco e mascara, MTU e liga.
    pub fn abrir(nome: &str, ip: Ipv4Addr, prefixo: u8, mtu: u32) -> Result<Tun, String> {
        let arquivo = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/net/tun")
            .map_err(|e| format!("abrir /dev/net/tun (precisa de root/CAP_NET_ADMIN): {e}"))?;
        let mut req = IfReq::novo(nome)?;
        req.uniao[..2].copy_from_slice(&(IFF_TUN | IFF_NO_PI).to_ne_bytes());
        chamar(arquivo.as_raw_fd(), TUNSETIFF, &mut req, "TUNSETIFF")?;

        let soquete = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
        let fd = soquete.as_raw_fd();
        chamar(
            fd,
            SIOCSIFADDR,
            &mut IfReq::por_endereco(nome, ip)?,
            "endereco",
        )?;
        let mascara = if prefixo == 0 {
            0
        } else {
            u32::MAX << (32 - prefixo as u32)
        };
        chamar(
            fd,
            SIOCSIFNETMASK,
            &mut IfReq::por_endereco(nome, Ipv4Addr::from(mascara))?,
            "mascara",
        )?;
        let mut m = IfReq::novo(nome)?;
        m.uniao[..4].copy_from_slice(&(mtu as i32).to_ne_bytes());
        chamar(fd, SIOCSIFMTU, &mut m, "mtu")?;
        let mut f = IfReq::novo(nome)?;
        chamar(fd, SIOCGIFFLAGS, &mut f, "ler bandeiras")?;
        let atuais = i16::from_ne_bytes([f.uniao[0], f.uniao[1]]);
        f.uniao[..2].copy_from_slice(&(atuais | IFF_UP | IFF_RUNNING).to_ne_bytes());
        chamar(fd, SIOCSIFFLAGS, &mut f, "ligar")?;
        Ok(Tun {
            arquivo,
            nome: nome.to_string(),
        })
    }

    /// Le um pacote, ou devolve `None` quando `parar` liga. Espera em fatias
    /// de meio segundo (`poll`): um `read` bloqueado nao acordaria para
    /// desligar a rede, e a placa so some quando o descritor fecha.
    pub fn ler_ou_parar(
        &self,
        buf: &mut [u8],
        parar: &std::sync::atomic::AtomicBool,
    ) -> std::io::Result<Option<usize>> {
        use std::sync::atomic::Ordering;
        loop {
            if parar.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let mut p = PollFd {
                fd: self.arquivo.as_raw_fd(),
                eventos: POLLIN,
                devolvidos: 0,
            };
            // SAFETY: um PollFd valido, vivo durante a chamada.
            let r = unsafe { poll(&mut p, 1, 500) };
            if r < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if r > 0 {
                return self.ler(buf).map(Some);
            }
        }
    }

    /// Le um pacote IP que o sistema mandou para a placa.
    pub fn ler(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        (&self.arquivo).read(buf)
    }

    /// Entrega um pacote IP ao sistema.
    pub fn escrever(&self, pacote: &[u8]) -> std::io::Result<()> {
        (&self.arquivo).write_all(pacote)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_de_interface_torto_e_recusado() {
        assert!(IfReq::novo("").is_err());
        assert!(IfReq::novo("nome-grande-demais-aqui").is_err());
        assert!(IfReq::novo("../x").is_err());
        assert!(IfReq::novo("phx0").is_ok());
        assert_eq!(std::mem::size_of::<IfReq>(), 40);
    }
}
