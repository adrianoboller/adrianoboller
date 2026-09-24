//! Os soquetes de fora do no P2P e do repasse: um IPv4 e, ao lado, um IPv6.
//!
//! # Por que dois soquetes, e nao um `[::]` de pilha dupla
//!
//! As duas receitas existem la fora: o OpenVPN abre UM soquete `[::]` com
//! `IPV6_V6ONLY=0` (socket.c:3089) e o IPv4 chega como `::ffff:a.b.c.d`; o
//! WireGuard (no kernel e no wireguard-go) abre DOIS, cada um so da sua
//! familia. Ficamos com a segunda, por tres motivos medidos aqui:
//!
//! - **O caminho IPv4 fica byte a byte o de antes.** A descoberta manda
//!   broadcast `255.255.255.255` com `SO_BROADCAST`, a perfuracao e o farol
//!   guardam e comparam `SocketAddr::V4` no rol assinado e nas tabelas. Com
//!   pilha dupla, todo endereco que chega viria mapeado e cada comparacao com
//!   um V4 ja gravado teria de lembrar de desmapear -- a que esquecesse
//!   perderia o par em silencio.
//! - **Maquina sem IPv6 continua funcionando.** O conteiner desta casa
//!   arranca com `ipv6.disable=1` (`/proc/cmdline`): `socket(AF_INET6)` da
//!   `EAFNOSUPPORT` ate dentro de netns. Um `[::]` unico derrubaria o no
//!   inteiro ali; com dois, so o IPv6 fica de fora, e o no avisa.
//! - **O Windows nasce com `IPV6_V6ONLY=1`** (padrao do Winsock desde o
//!   Vista): la, pilha dupla pediria FFI antes do `bind`; dois soquetes saem
//!   da `std` pura. No Linux o padrao e o contrario (`bindv6only=0`), entao o
//!   IPv6 so e aberto por FFI com `IPV6_V6ONLY=1` ANTES do `bind` -- depois
//!   dele o kernel recusa a troca, e sem ela o `[::]:porta` brigaria com o
//!   `0.0.0.0:porta` do soquete IPv4.

use std::net::{IpAddr, SocketAddr, TcpListener, UdpSocket};

/// `::ffff:a.b.c.d` vira `a.b.c.d`. Endereco que chega escrito a mao (par
/// na linha de comando, convite) ou de outro sistema pode vir mapeado; o que
/// esta gravado (rol, farol, tabelas) e V4. Sem isto, o mesmo par teria dois
/// enderecos que nao se comparam iguais.
pub fn canonico(a: SocketAddr) -> SocketAddr {
    match a.ip() {
        IpAddr::V6(v) => match v.to_ipv4_mapped() {
            Some(v4) => SocketAddr::new(IpAddr::V4(v4), a.port()),
            None => a,
        },
        IpAddr::V4(_) => a,
    }
}

/// O soquete da familia do destino. Sem o IPv6 aberto, o IPv4 -- e o envio
/// falha como falhava antes, sem inventar caminho.
pub fn escolher<'a>(
    v4: &'a UdpSocket,
    v6: Option<&'a UdpSocket>,
    destino: &SocketAddr,
) -> &'a UdpSocket {
    match (destino.is_ipv6(), v6) {
        (true, Some(s)) => s,
        _ => v4,
    }
}

/// UDP so IPv6 em `[::]:porta`.
pub fn udp_so_v6(porta: u16) -> std::io::Result<UdpSocket> {
    so_v6::udp(porta)
}

/// TCP (escutando) so IPv6 em `[::]:porta`.
pub fn tcp_so_v6(porta: u16) -> std::io::Result<TcpListener> {
    so_v6::tcp(porta)
}

/// Abre o IPv6 ao lado de um IPv4 ja aberto, na MESMA porta. Falhar nao e
/// erro do no: e maquina sem IPv6 (ou porta tomada so no IPv6), e o aviso
/// diz qual.
pub fn udp_v6_ao_lado(v4: &UdpSocket) -> (Option<UdpSocket>, Option<String>) {
    let porta = match v4.local_addr() {
        Ok(a) => a.port(),
        Err(e) => return (None, Some(format!("sem IPv6: {e}"))),
    };
    match udp_so_v6(porta) {
        Ok(s) => (Some(s), None),
        Err(e) => (None, Some(format!("sem IPv6 na porta UDP {porta}: {e}"))),
    }
}

#[cfg(target_os = "linux")]
mod so_v6 {
    use std::net::{TcpListener, UdpSocket};
    use std::os::fd::{FromRawFd, OwnedFd};

    #[repr(C)]
    struct SockAddrIn6 {
        familia: u16,
        porta: u16,
        fluxo: u32,
        ip: [u8; 16],
        escopo: u32,
    }
    extern "C" {
        fn socket(dominio: i32, tipo: i32, protocolo: i32) -> i32;
        fn setsockopt(fd: i32, nivel: i32, nome: i32, valor: *const u8, tam: u32) -> i32;
        fn bind(fd: i32, end: *const SockAddrIn6, tam: u32) -> i32;
        fn listen(fd: i32, fila: i32) -> i32;
    }
    const AF_INET6: i32 = 10;
    const SOCK_STREAM: i32 = 1;
    const SOCK_DGRAM: i32 = 2;
    const SOCK_CLOEXEC: i32 = 0o2_000_000;
    const SOL_SOCKET: i32 = 1;
    const SO_REUSEADDR: i32 = 2;
    const IPPROTO_IPV6: i32 = 41;
    const IPV6_V6ONLY: i32 = 26;

    fn checar(r: i32) -> std::io::Result<()> {
        if r < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn abrir(porta: u16, tipo: i32) -> std::io::Result<OwnedFd> {
        // SAFETY: socket(2) so devolve um fd novo ou -1; o fd vira OwnedFd
        // na hora, e todo retorno de erro abaixo o fecha pelo Drop.
        let fd = unsafe { socket(AF_INET6, tipo | SOCK_CLOEXEC, 0) };
        checar(fd)?;
        let dono = unsafe { OwnedFd::from_raw_fd(fd) };
        let valor: i32 = 1;
        // A mesma assinatura que o `usb.rs` declara (valor como bytes): duas
        // declaracoes diferentes do mesmo simbolo sao aviso do compilador.
        let um = (&valor as *const i32).cast::<u8>();
        // SAFETY: `valor` vive ate o fim da chamada e o tamanho e o de um i32.
        checar(unsafe { setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, um, 4) })?;
        if tipo == SOCK_STREAM {
            // O mesmo que a std faz no TcpListener: religar logo depois de
            // cair, sem esperar o TIME_WAIT.
            checar(unsafe { setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, um, 4) })?;
        }
        let end = SockAddrIn6 {
            familia: AF_INET6 as u16,
            porta: porta.to_be(),
            fluxo: 0,
            ip: [0; 16],
            escopo: 0,
        };
        // SAFETY: `end` e um sockaddr_in6 completo (28 bytes) que vive ate o
        // fim da chamada.
        checar(unsafe { bind(fd, &end, std::mem::size_of::<SockAddrIn6>() as u32) })?;
        if tipo == SOCK_STREAM {
            checar(unsafe { listen(fd, 128) })?;
        }
        Ok(dono)
    }

    pub fn udp(porta: u16) -> std::io::Result<UdpSocket> {
        abrir(porta, SOCK_DGRAM).map(UdpSocket::from)
    }

    pub fn tcp(porta: u16) -> std::io::Result<TcpListener> {
        abrir(porta, SOCK_STREAM).map(TcpListener::from)
    }
}

#[cfg(not(target_os = "linux"))]
mod so_v6 {
    //! Windows: o Winsock ja nasce com `IPV6_V6ONLY=1`, entao o `bind` da
    //! `std` basta. Se um dia nascer de pilha dupla, o `[::]:porta` brigaria
    //! com o `0.0.0.0:porta` ja aberto e o `bind` falharia -- e isso vira o
    //! aviso «sem IPv6», nunca um soquete tomando a porta do outro.
    use std::net::{Ipv6Addr, TcpListener, UdpSocket};

    pub fn udp(porta: u16) -> std::io::Result<UdpSocket> {
        UdpSocket::bind((Ipv6Addr::UNSPECIFIED, porta))
    }

    pub fn tcp(porta: u16) -> std::io::Result<TcpListener> {
        TcpListener::bind((Ipv6Addr::UNSPECIFIED, porta))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mapeado_compara_igual_ao_v4_gravado() {
        let gravado: SocketAddr = "203.0.113.7:51820".parse().unwrap();
        let chegou: SocketAddr = "[::ffff:203.0.113.7]:51820".parse().unwrap();
        assert_ne!(gravado, chegou, "a premissa: sem desmapear, diferem");
        assert_eq!(canonico(chegou), gravado);
        // IPv6 de verdade fica como veio; IPv4 tambem.
        let v6: SocketAddr = "[2001:db8::7]:51820".parse().unwrap();
        assert_eq!(canonico(v6), v6);
        assert_eq!(canonico(gravado), gravado);
    }

    #[test]
    fn destino_v6_sai_pelo_soquete_v6() {
        // Dois soquetes IPv4 no papel dos dois: o que se prova e a ESCOLHA,
        // que roda igual em maquina sem IPv6 (como este conteiner).
        let v4 = UdpSocket::bind("127.0.0.1:0").unwrap();
        let v6 = UdpSocket::bind("127.0.0.1:0").unwrap();
        let (a4, a6) = (v4.local_addr().unwrap(), v6.local_addr().unwrap());
        let d6: SocketAddr = "[2001:db8::1]:1".parse().unwrap();
        let d4: SocketAddr = "192.0.2.1:1".parse().unwrap();
        assert_eq!(escolher(&v4, Some(&v6), &d6).local_addr().unwrap(), a6);
        assert_eq!(escolher(&v4, Some(&v6), &d4).local_addr().unwrap(), a4);
        assert_eq!(escolher(&v4, None, &d6).local_addr().unwrap(), a4);
    }

    /// Sem IPv6 na maquina, abrir o IPv6 e um aviso, nunca um panico nem um
    /// erro que derrube o no. Com IPv6, abrir na MESMA porta do IPv4 ja e a
    /// prova do `IPV6_V6ONLY=1`: de pilha dupla, o `bind` daria EADDRINUSE.
    #[test]
    fn ipv6_ao_lado_do_ipv4_na_mesma_porta_ou_aviso() {
        let v4 = UdpSocket::bind("0.0.0.0:0").unwrap();
        let porta = v4.local_addr().unwrap().port();
        match udp_v6_ao_lado(&v4) {
            (Some(s), None) => {
                assert_eq!(s.local_addr().unwrap().port(), porta);
                assert!(s.local_addr().unwrap().is_ipv6());
            }
            (None, Some(aviso)) => assert!(aviso.contains("sem IPv6"), "{aviso}"),
            _ => panic!("ou soquete, ou aviso"),
        }
    }

    /// Ida e volta por `::1`. Maquina sem IPv6 nao tem `::1`: o teste diz
    /// que nao rodou em vez de passar calado.
    #[test]
    #[ignore = "pede IPv6 no kernel; este conteiner arranca com ipv6.disable=1"]
    fn ida_e_volta_por_ipv6_de_verdade() {
        let a = udp_so_v6(0).unwrap();
        let b = udp_so_v6(0).unwrap();
        let eb: SocketAddr = format!("[::1]:{}", b.local_addr().unwrap().port())
            .parse()
            .unwrap();
        a.send_to(b"ola", eb).unwrap();
        let mut buf = [0u8; 8];
        let (n, de) = b.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"ola");
        assert!(de.is_ipv6());
    }
}
