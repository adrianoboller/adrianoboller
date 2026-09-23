//! Phoenix VPN como biblioteca: o motor (`painel`) e as pecas dele, para a
//! linha de comando e para os testes de integracao usarem o MESMO codigo.

pub mod cofre;
pub mod http;
pub mod ovpn;
pub mod painel;
pub mod pg;
pub mod pki;
pub mod supervisor;
