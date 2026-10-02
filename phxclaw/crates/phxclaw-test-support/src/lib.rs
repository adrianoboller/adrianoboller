//! Apoio dos testes do workspace. Dois modulos, UMA casa (funcao nao se duplica):
//!
//! * [`pulado`] -- o pulo VISIVEL de um teste que depende de recurso da maquina;
//! * [`guarda`] -- o varredor que reprova o pulo CALADO novo (catraca em 0).
//!
//! Entra so como `[dev-dependencies]`; o produto nao depende daqui.

pub mod guarda;
pub mod pulado;
