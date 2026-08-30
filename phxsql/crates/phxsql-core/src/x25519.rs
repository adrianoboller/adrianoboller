//! X25519: a troca de chaves da RFC 7748, sem dependencias externas.
//!
//! # Para que serve aqui
//!
//! Era a UNICA peca que faltava para cifrar o fio. O ChaCha20-Poly1305 ja
//! estava escrito e conferido (`cifra.rs`), o SHA-256 e o HMAC tambem
//! (`hash.rs`), e o desafio-resposta ja protegia a credencial. O que nao
//! existia era o jeito de dois lados que nunca se falaram chegarem a uma
//! chave comum sem manda-la pelo fio. Ver `docs/CIFRA-DO-FIO.md`.
//!
//! # Por que reaproveita o corpo do `ed25519.rs`
//!
//! Curve25519 e Ed25519 vivem no MESMO corpo finito, modulo 2^255 - 19. O
//! `ed25519.rs` ja traz essa aritmetica em cinco pedacos de 51 bits, conferida
//! contra a RFC 8032. Escrever um segundo `fe_mul` aqui do lado seria dobrar a
//! superficie de erro na parte mais dificil de auditar -- e a segunda copia
//! seria justamente a que ninguem revisa.
//!
//! # Tempo constante, e sem tabela
//!
//! A escada de Montgomery faz as MESMAS operacoes para todo escalar: 255
//! voltas, cada uma com a mesma conta. O bit do escalar so entra por uma troca
//! condicional feita com mascara e XOR -- nao ha `if` que dependa de segredo,
//! nao ha indice que dependa de segredo, e nao ha tabela pre-computada. Nada
//! aqui vaza a chave pelo tempo nem pelo cache.
//!
//! Isso e diferente do `ed25519.rs`, que assume o tempo NAO constante de
//! proposito: la o servidor so verifica assinatura, que e dado publico. Aqui
//! nao da: a privada estatica do servidor entra em toda multiplicacao, e o
//! aperto acontece dentro do servico exposto.
//!
//! # Conferido contra o RFC
//!
//! Todos os vetores da RFC 7748 que dao para exercitar em tempo de teste: os
//! dois de multiplicacao escalar da secao 5.2, o iterado da mesma secao (1 e
//! 1.000 vezes; o de 1.000.000 fica atras de `#[ignore]`, por custar minutos)
//! e o Diffie-Hellman da secao 6.1.

use crate::ed25519::Fe;
use crate::ed25519::{
    fe_de_bytes, fe_inverso, fe_mul, fe_para_bytes, fe_quadrado, fe_soma, fe_sub,
};
use crate::error::{PhxError, Result};
use crate::hash::iguais_em_tempo_constante;

/// Tamanho da chave e do ponto, em bytes.
pub const CHAVE_LEN: usize = 32;

/// A coordenada u do ponto base: 9.
pub const BASE: [u8; CHAVE_LEN] = {
    let mut b = [0u8; CHAVE_LEN];
    b[0] = 9;
    b
};

/// (A - 2) / 4, com A = 486662. E a unica constante da curva que a escada usa.
const A24: Fe = [121_665, 0, 0, 0, 0];

/// Troca `a` e `b` se `bit` for 1, sem desviar.
///
/// A mascara e 0 ou 0xFFFF... -- o processador executa as mesmas cinco
/// instrucoes nos dois casos, e o bit do escalar nao vira desvio nem endereco.
#[inline(always)]
fn troca_condicional(bit: u64, a: &mut Fe, b: &mut Fe) {
    let mascara = 0u64.wrapping_sub(bit);
    for i in 0..5 {
        let t = mascara & (a[i] ^ b[i]);
        a[i] ^= t;
        b[i] ^= t;
    }
}

/// `decodeScalar25519` da secao 5 da RFC 7748.
///
/// Os tres bits de baixo zerados poem o escalar num multiplo da cofator 8, o
/// que anula qualquer componente de ordem pequena que o outro lado tenha
/// mandado; o bit 254 fixo em 1 iguala o tamanho de toda escada, que e parte
/// do tempo constante.
fn limpar(k: &[u8; CHAVE_LEN]) -> [u8; CHAVE_LEN] {
    let mut e = *k;
    e[0] &= 248;
    e[31] &= 127;
    e[31] |= 64;
    e
}

/// A escada de Montgomery: `u` multiplicado pelo escalar ja limpo.
fn escada(k: &[u8; CHAVE_LEN], u: Fe) -> Fe {
    let x1 = u;
    let mut x2: Fe = [1, 0, 0, 0, 0];
    let mut z2: Fe = [0, 0, 0, 0, 0];
    let mut x3 = u;
    let mut z3: Fe = [1, 0, 0, 0, 0];
    let mut trocado = 0u64;

    // Comeca no bit 254 porque o 255 e sempre zero depois do `limpar`, e o 254
    // e sempre um -- rodar os 255 daria o mesmo resultado e uma volta a toa.
    for t in (0..255).rev() {
        let bit = ((k[t >> 3] >> (t & 7)) & 1) as u64;
        trocado ^= bit;
        troca_condicional(trocado, &mut x2, &mut x3);
        troca_condicional(trocado, &mut z2, &mut z3);
        trocado = bit;

        let a = fe_soma(x2, z2);
        let aa = fe_quadrado(a);
        let b = fe_sub(x2, z2);
        let bb = fe_quadrado(b);
        let e = fe_sub(aa, bb);
        let c = fe_soma(x3, z3);
        let d = fe_sub(x3, z3);
        let da = fe_mul(d, a);
        let cb = fe_mul(c, b);
        x3 = fe_quadrado(fe_soma(da, cb));
        z3 = fe_mul(x1, fe_quadrado(fe_sub(da, cb)));
        x2 = fe_mul(aa, bb);
        z2 = fe_mul(e, fe_soma(aa, fe_mul(A24, e)));
    }

    troca_condicional(trocado, &mut x2, &mut x3);
    troca_condicional(trocado, &mut z2, &mut z3);
    fe_mul(x2, fe_inverso(z2))
}

/// A funcao `X25519(k, u)` da RFC 7748, com o escalar limpo como manda a norma.
///
/// O bit 255 do ponto e ignorado na leitura (`decodeUCoordinate`), e nao ha
/// recusa de ponto aqui: quem precisa da recusa e o [`segredo`], porque o que
/// se recusa e o SEGREDO todo-zeros, e nao a entrada.
pub fn multiplicar(escalar: &[u8; CHAVE_LEN], ponto: &[u8; CHAVE_LEN]) -> [u8; CHAVE_LEN] {
    fe_para_bytes(escada(&limpar(escalar), fe_de_bytes(ponto)))
}

/// A publica que corresponde a esta privada: `X25519(k, 9)`.
pub fn chave_publica(privada: &[u8; CHAVE_LEN]) -> [u8; CHAVE_LEN] {
    multiplicar(privada, &BASE)
}

/// O segredo compartilhado -- ou erro, quando ele sai todo-zeros.
///
/// # Por que o todo-zeros e ERRO, e nao um segredo qualquer
///
/// Existem pontos de ordem pequena na curva (e na torcao dela) cuja
/// multiplicacao por QUALQUER escalar da zero. Quem manda um deles como chave
/// publica faz os dois lados derivarem a mesma coisa sem saber segredo nenhum
/// -- e um aperto que fecha sem ninguem ter provado nada.
///
/// A secao 6.1 da RFC 7748 diz que a recusa e opcional para o Diffie-Hellman
/// puro (porque quem manda ordem pequena so engana a si mesmo). Aqui NAO e
/// opcional: as chaves de sessao saem deste segredo, e um segredo previsivel
/// vira um tunel que o atacante le. Recusar e uma linha; descobrir depois que
/// nao se recusou custa a conexao inteira.
pub fn segredo(privada: &[u8; CHAVE_LEN], publica: &[u8; CHAVE_LEN]) -> Result<[u8; CHAVE_LEN]> {
    let k = multiplicar(privada, publica);
    if iguais_em_tempo_constante(&k, &[0u8; CHAVE_LEN]) {
        return Err(PhxError::Autorizacao(
            "chave publica de ordem pequena: o segredo compartilhado sairia \
             todo-zeros, e um aperto assim fecha sem ninguem provar nada"
                .into(),
        ));
    }
    Ok(k)
}

/// Uma privada nova, sorteada.
///
/// Ja sai limpa (`decodeScalar25519`) para que a privada guardada em arquivo e
/// a privada usada na conta sejam o MESMO valor: sem isso, quem comparasse os
/// dois bytes a bytes acharia diferenca onde nao ha.
pub fn gerar_privada() -> [u8; CHAVE_LEN] {
    let mut k = [0u8; CHAVE_LEN];
    crate::cifra::sortear(&mut k);
    limpar(&k)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hash::{de_hex, para_hex};

    fn bytes32(hex: &str) -> [u8; 32] {
        let v = de_hex(hex).expect("hexadecimal invalido no vetor");
        let mut b = [0u8; 32];
        b.copy_from_slice(&v);
        b
    }

    /// RFC 7748, secao 5.2, primeiro vetor.
    #[test]
    fn vetor_1_da_secao_5_2() {
        let k = bytes32("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4");
        let u = bytes32("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c");
        assert_eq!(
            para_hex(&multiplicar(&k, &u)),
            "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552"
        );
    }

    /// RFC 7748, secao 5.2, segundo vetor.
    #[test]
    fn vetor_2_da_secao_5_2() {
        let k = bytes32("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d");
        let u = bytes32("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493");
        assert_eq!(
            para_hex(&multiplicar(&k, &u)),
            "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957"
        );
    }

    /// RFC 7748, secao 5.2: o vetor iterado, 1 e 1.000 vezes.
    ///
    /// E o vetor que pega o que os dois de cima nao pegam: um erro que so
    /// aparece em UMA entrada especifica some no meio de mil composicoes, e um
    /// erro de propagacao de carrego que so morde em um caso em mil aparece.
    #[test]
    fn iteracoes_da_secao_5_2() {
        let mut k = BASE;
        let mut u = BASE;
        for i in 1..=1_000 {
            let saida = multiplicar(&k, &u);
            u = k;
            k = saida;
            if i == 1 {
                assert_eq!(
                    para_hex(&k),
                    "422c8e7a6227d7bca1350b3e2bb7279f7897b87bb6854b783c60e80311ae3079"
                );
            }
        }
        assert_eq!(
            para_hex(&k),
            "684cf59ba83309552800ef566f2f4d3c1c3887c49360e3875f2eb94d99532c51"
        );
    }

    /// RFC 7748, secao 5.2: 1.000.000 de iteracoes.
    ///
    /// Fora da bateria de proposito -- leva minutos. Roda com
    /// `cargo test -p phxsql-core --release -- --ignored um_milhao`.
    #[test]
    #[ignore = "leva minutos: e o vetor de 1.000.000 de iteracoes da RFC 7748"]
    fn um_milhao_de_iteracoes_da_secao_5_2() {
        let mut k = BASE;
        let mut u = BASE;
        for _ in 0..1_000_000 {
            let saida = multiplicar(&k, &u);
            u = k;
            k = saida;
        }
        assert_eq!(
            para_hex(&k),
            "7c3911e0ab2586fd864497297e575e6f3bc601c0883c30df5f4dd2d24f665424"
        );
    }

    /// RFC 7748, secao 6.1: o Diffie-Hellman inteiro, com as duas publicas e o
    /// segredo que os dois lados tem de achar.
    #[test]
    fn diffie_hellman_da_secao_6_1() {
        let a = bytes32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let b = bytes32("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
        let pub_a = chave_publica(&a);
        let pub_b = chave_publica(&b);
        assert_eq!(
            para_hex(&pub_a),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        assert_eq!(
            para_hex(&pub_b),
            "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"
        );
        let esperado = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742";
        assert_eq!(para_hex(&segredo(&a, &pub_b).unwrap()), esperado);
        assert_eq!(para_hex(&segredo(&b, &pub_a).unwrap()), esperado);
    }

    /// Ponto de ordem pequena: o segredo sai todo-zeros e tem de virar ERRO.
    ///
    /// Os cinco sao os pontos de ordem 1, 2 e 8 conhecidos da curva e da
    /// torcao. Sem a recusa, os dois lados fechariam o aperto com uma chave
    /// que o atacante escolheu.
    #[test]
    fn ponto_de_ordem_pequena_e_recusado() {
        let a = bytes32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        for hostil in [
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0100000000000000000000000000000000000000000000000000000000000000",
            "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b800",
            "5f9c95bca3508c24b1d0b1559c83ef5b04445cc4581c8e86d8224eddd09f1157",
            "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
        ] {
            let p = bytes32(hostil);
            assert_eq!(
                para_hex(&multiplicar(&a, &p)),
                "0000000000000000000000000000000000000000000000000000000000000000",
                "{hostil} devia dar segredo todo-zeros"
            );
            assert!(
                segredo(&a, &p).is_err(),
                "{hostil} passou: o segredo todo-zeros virou chave de sessao"
            );
        }
    }

    /// O bit 255 do ponto nao faz parte da coordenada (`decodeUCoordinate`).
    #[test]
    fn o_bit_255_do_ponto_e_ignorado() {
        let a = gerar_privada();
        let mut u = bytes32("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c");
        let sem = multiplicar(&a, &u);
        u[31] |= 0x80;
        assert_eq!(multiplicar(&a, &u), sem);
    }

    /// Duas privadas sorteadas fecham no mesmo segredo. E o caso que o servidor
    /// vive: nenhum dos dois lados escolheu o vetor.
    #[test]
    fn dois_sorteados_chegam_ao_mesmo_segredo() {
        let a = gerar_privada();
        let b = gerar_privada();
        assert_ne!(a, b);
        let sa = segredo(&a, &chave_publica(&b)).unwrap();
        let sb = segredo(&b, &chave_publica(&a)).unwrap();
        assert_eq!(sa, sb);
    }
}
