//! As pecas PURAS da placa do Windows (TAP-Windows6): codigos de IOCTL,
//! a entrada do `CONFIG_TUN` e o UTF-16. Ficam fora do `tun_windows.rs` de
//! proposito: aqui compilam e se testam em qualquer sistema -- dentro dele,
//! os testes so rodariam num Windows, e este repositorio e provado no Linux.

use std::net::Ipv4Addr;

/// `CTL_CODE(FILE_DEVICE_UNKNOWN, funcao, METHOD_BUFFERED, FILE_ANY_ACCESS)`,
/// como o `TAP_CONTROL_CODE` do `tap-windows.h`.
pub const fn codigo_tap(funcao: u32) -> u32 {
    (0x22 << 16) | (funcao << 2)
}
pub const IOCTL_SET_MEDIA_STATUS: u32 = codigo_tap(6);
pub const IOCTL_CONFIG_TUN: u32 = codigo_tap(10);

/// UTF-16 terminado em zero, como o Windows quer.
pub fn largo(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// De volta de UTF-16 (ate o primeiro zero).
pub fn de_largo(b: &[u16]) -> String {
    let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf16_lossy(&b[..fim])
}

/// A entrada do `CONFIG_TUN`: tres IPADDR em ordem de rede. O driver recusa
/// se `rede & mascara != rede` (device.c:425), por isso a rede sai do IP.
pub fn config_tun(ip: Ipv4Addr, prefixo: u8) -> [u8; 12] {
    let mascara = if prefixo == 0 {
        0
    } else {
        u32::MAX << (32 - prefixo as u32)
    };
    let rede = u32::from(ip) & mascara;
    let mut e = [0u8; 12];
    e[..4].copy_from_slice(&ip.octets());
    e[4..8].copy_from_slice(&rede.to_be_bytes());
    e[8..].copy_from_slice(&mascara.to_be_bytes());
    e
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn codigos_de_ioctl_batem_com_o_tap_windows_h() {
        assert_eq!(IOCTL_CONFIG_TUN, 0x0022_0028);
        assert_eq!(IOCTL_SET_MEDIA_STATUS, 0x0022_0018);
    }

    #[test]
    fn config_tun_em_ordem_de_rede_e_rede_mascarada() {
        let e = config_tun("10.78.0.5".parse().unwrap(), 24);
        assert_eq!(e, [10, 78, 0, 5, 10, 78, 0, 0, 255, 255, 255, 0]);
    }

    #[test]
    fn utf16_ida_e_volta() {
        let l = largo("phxvpn");
        assert_eq!(*l.last().unwrap(), 0);
        assert_eq!(de_largo(&l), "phxvpn");
        assert_eq!(de_largo(&largo("Conexão")), "Conexão");
    }
}
