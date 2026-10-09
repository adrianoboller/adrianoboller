//! Apoio dos testes do workspace. Quatro modulos, UMA casa (funcao nao se duplica):
//!
//! * [`pulado`] -- o pulo VISIVEL de um teste que depende de recurso da maquina;
//! * [`guarda`] -- o varredor que reprova o pulo CALADO novo (catraca em 0);
//! * [`git_http`] -- o servidor git HTTP falso em loopback das provas de push/pull de rede;
//! * [`pg`] -- um PostgreSQL efemero de verdade (a fila de execucoes se prova contra ele).
//!
//! Entra so como `[dev-dependencies]`; o produto nao depende daqui.

pub mod git_http;
pub mod guarda;
pub mod pg;
pub mod pulado;
