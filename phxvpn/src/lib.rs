//! phxvpn como biblioteca: o motor (`painel`) e as pecas dele, para a
//! linha de comando e para os testes de integracao usarem o MESMO codigo.

pub mod acl;
#[cfg(windows)]
pub mod bandeja;
pub mod cofre;
pub mod comandos;
pub mod console;
pub mod guarda;
pub mod http;
pub mod iniciar;
pub mod lembrar;
pub mod mesa;
pub mod noise;
pub mod ovpn;
pub mod p2p;
pub mod painel;
pub mod pg;
pub mod pki;
pub mod rede_p2p;
pub mod repasse;
pub mod servico;
pub mod supervisor;
pub mod tap;
pub mod transporte;
#[cfg(target_os = "linux")]
pub mod tun;
#[cfg(windows)]
#[path = "tun_windows.rs"]
pub mod tun;
pub mod usb;
pub mod web;
