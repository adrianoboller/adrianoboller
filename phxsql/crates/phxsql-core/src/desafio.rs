//! Desafio-resposta: autenticar sem a senha atravessar o fio.
//!
//! Base64 esconde a senha de quem olha por cima do ombro; **isto** esconde a
//! senha de quem captura o pacote. Sao problemas diferentes.
//!
//! # Como funciona
//!
//! ```text
//! cliente                                        servidor
//!    |  {"op":"desafio","usuario":"adriano"}         |
//!    | ------------------------------------------->  |
//!    |                                               | sorteia nonce
//!    |  {"sal":..., "iteracoes":..., "nonce":...}    |
//!    | <-------------------------------------------  |
//!    |                                               |
//!    | dk    = pbkdf2(senha, sal, iteracoes)         |
//!    | prova = hmac(dk, nonce+nonce_cliente+usuario) |
//!    |                                               |
//!    |  {"op":"login","prova":..., "nonce_cliente":..} |
//!    | ------------------------------------------->  |
//!    |                                               | refaz a conta com o
//!    |                                               | dk que ja tem guardado
//! ```
//!
//! A senha nunca sai da maquina do cliente. O nonce do servidor e sorteado a
//! cada desafio e vale uma vez so, entao gravar o dialogo e repeti-lo depois
//! nao autentica ninguem.
//!
//! # O que isto NAO resolve
//!
//! * **O resto do trafego continua em claro.** Protege a credencial, nao os
//!   dados. Para os dados, tunel (IPSec, WireGuard).
//! * **Quem le o `config.json` consegue autenticar**, porque o que esta
//!   guardado la e exatamente a chave usada na prova. E um problema menor do
//!   que parece: esse mesmo arquivo tem o token de servico e aponta para os
//!   dados. Quem o le ja ganhou.

use crate::error::{PhxError, Result};
use crate::hash::{de_hex, hmac_sha256, iguais_em_tempo_constante, para_hex};

/// Bytes do nonce sorteado a cada desafio.
pub const NONCE_LEN: usize = 16;

/// Quanto tempo um desafio vale, em milissegundos.
pub const VALIDADE_MS: i64 = 60_000;

/// A mensagem que os dois lados assinam.
///
/// Os dois nonces entram: o do servidor impede repetir um dialogo gravado, e o
/// do cliente impede o servidor de escolher sozinho o que sera assinado.
fn mensagem(nonce_servidor: &str, nonce_cliente: &str, usuario: &str) -> Vec<u8> {
    let mut m = Vec::with_capacity(nonce_servidor.len() + nonce_cliente.len() + usuario.len() + 2);
    m.extend_from_slice(nonce_servidor.as_bytes());
    m.push(b',');
    m.extend_from_slice(nonce_cliente.as_bytes());
    m.push(b',');
    m.extend_from_slice(usuario.as_bytes());
    m
}

/// Calcula a prova, em hexadecimal. E o que o cliente manda.
pub fn calcular_prova(
    derivado: &[u8],
    nonce_servidor: &str,
    nonce_cliente: &str,
    usuario: &str,
) -> String {
    para_hex(&hmac_sha256(
        derivado,
        &mensagem(nonce_servidor, nonce_cliente, usuario),
    ))
}

/// Confere a prova recebida. Comparacao em tempo constante.
pub fn conferir_prova(
    derivado: &[u8],
    nonce_servidor: &str,
    nonce_cliente: &str,
    usuario: &str,
    prova: &str,
) -> bool {
    let Some(recebida) = de_hex(prova) else {
        return false;
    };
    let esperada = hmac_sha256(derivado, &mensagem(nonce_servidor, nonce_cliente, usuario));
    iguais_em_tempo_constante(&recebida, &esperada)
}

/// A prova a partir da senha em claro -- o lado do cliente, inteiro.
///
/// Existe aqui para que qualquer cliente escrito em Rust use exatamente o
/// mesmo calculo do servidor, sem reimplementar nada.
pub fn prova_de_senha(
    senha: &str,
    sal_hex: &str,
    iteracoes: u32,
    nonce_servidor: &str,
    nonce_cliente: &str,
    usuario: &str,
) -> Result<String> {
    let sal =
        de_hex(sal_hex).ok_or_else(|| PhxError::Tipo("sal do desafio nao e hexadecimal".into()))?;
    let mut derivado = vec![0u8; 32];
    crate::hash::pbkdf2_sha256(senha.as_bytes(), &sal, iteracoes, &mut derivado);
    Ok(calcular_prova(
        &derivado,
        nonce_servidor,
        nonce_cliente,
        usuario,
    ))
}

/// Um nonce novo, em hexadecimal.
pub fn nonce() -> String {
    para_hex(&crate::senha::bytes_aleatorios(NONCE_LEN))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::senha;

    const RAPIDO: u32 = 64;

    #[test]
    fn o_cliente_prova_sem_mandar_a_senha() {
        let guardado = senha::cifrar_com("Senha Do Adriano", RAPIDO);
        let dk = senha::derivado_do_hash(&guardado).unwrap();
        let (sal, it) = senha::sal_e_iteracoes(&guardado).unwrap();

        let ns = nonce();
        let nc = nonce();
        // O cliente so tem a senha, o sal e as iteracoes.
        let prova =
            prova_de_senha("Senha Do Adriano", &para_hex(&sal), it, &ns, &nc, "adriano").unwrap();
        // O servidor so tem o derivado guardado.
        assert!(conferir_prova(&dk, &ns, &nc, "adriano", &prova));
    }

    #[test]
    fn senha_errada_nao_prova() {
        let guardado = senha::cifrar_com("certa", RAPIDO);
        let dk = senha::derivado_do_hash(&guardado).unwrap();
        let (sal, it) = senha::sal_e_iteracoes(&guardado).unwrap();
        let (ns, nc) = (nonce(), nonce());
        let prova = prova_de_senha("errada", &para_hex(&sal), it, &ns, &nc, "ana").unwrap();
        assert!(!conferir_prova(&dk, &ns, &nc, "ana", &prova));
    }

    #[test]
    fn dialogo_gravado_nao_serve_no_proximo_desafio() {
        let guardado = senha::cifrar_com("x", RAPIDO);
        let dk = senha::derivado_do_hash(&guardado).unwrap();
        let (sal, it) = senha::sal_e_iteracoes(&guardado).unwrap();

        let (ns1, nc) = (nonce(), nonce());
        let prova = prova_de_senha("x", &para_hex(&sal), it, &ns1, &nc, "ana").unwrap();
        assert!(conferir_prova(&dk, &ns1, &nc, "ana", &prova));

        // Mesmo dialogo, nonce novo do servidor: nao passa.
        let ns2 = nonce();
        assert_ne!(ns1, ns2);
        assert!(!conferir_prova(&dk, &ns2, &nc, "ana", &prova));
    }

    #[test]
    fn a_prova_amarra_o_usuario() {
        let guardado = senha::cifrar_com("x", RAPIDO);
        let dk = senha::derivado_do_hash(&guardado).unwrap();
        let (sal, it) = senha::sal_e_iteracoes(&guardado).unwrap();
        let (ns, nc) = (nonce(), nonce());
        let prova = prova_de_senha("x", &para_hex(&sal), it, &ns, &nc, "ana").unwrap();
        // A mesma prova apresentada como se fosse de outro login nao vale.
        assert!(!conferir_prova(&dk, &ns, &nc, "joao", &prova));
    }

    #[test]
    fn prova_malformada_nao_derruba_nem_deixa_entrar() {
        let dk = vec![0u8; 32];
        for ruim in ["", "nao-e-hex", "zz", "abc"] {
            assert!(!conferir_prova(&dk, "a", "b", "c", ruim));
        }
    }

    #[test]
    fn nonce_nao_repete() {
        let mut vistos = std::collections::HashSet::new();
        for _ in 0..200 {
            assert!(vistos.insert(nonce()), "nonce repetiu");
        }
        assert_eq!(nonce().len(), NONCE_LEN * 2);
    }
}
