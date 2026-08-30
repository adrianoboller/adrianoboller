//! HKDF-SHA256, a derivacao de chave da RFC 5869, sobre o HMAC que ja existe.
//!
//! # Por que uma peca so para isto
//!
//! Um segredo Diffie-Hellman nao e uma chave. Ele e um ponto da curva escrito
//! em 32 bytes, com estrutura algebrica dentro e distribuicao que nao e
//! uniforme -- usa-lo direto como chave de cifra e o erro classico. O HKDF faz
//! duas coisas separadas, e a separacao E o desenho:
//!
//! * **extrair** concentra a entropia espalhada num PRK de 32 bytes
//!   uniformes. E um HMAC, com o sal na posicao da chave;
//! * **expandir** estica esse PRK em quantas chaves independentes se quiser,
//!   cada uma amarrada a um rotulo (`info`). Duas chaves derivadas do mesmo
//!   PRK com rotulos diferentes nao se deduzem uma da outra.
//!
//! O aperto do fio usa as duas: extrair a cada Diffie-Hellman da cadeia,
//! expandir no fim para tirar a chave de ida e a de volta.
//!
//! # Conferido contra o RFC
//!
//! Os tres casos SHA-256 do anexo A: A.1 (basico), A.2 (entradas longas) e
//! A.3 (sal e `info` vazios -- o caso que pega quem esquece que sal vazio vira
//! um bloco de zeros, e nao "nenhuma chave").

use crate::error::{PhxError, Result};
use crate::hash::{hmac_sha256, SHA256_LEN};

/// O maior tamanho que o `expandir` consegue produzir: 255 blocos.
pub const MAX_SAIDA: usize = 255 * SHA256_LEN;

/// `HKDF-Extract(salt, IKM)`: concentra a entropia da entrada num PRK.
///
/// O sal entra como CHAVE do HMAC, e nao como mensagem -- e essa inversao que
/// faz o sal poder ser publico e mesmo assim separar dois contextos. Sal vazio
/// vira um bloco de zeros do tamanho do hash, como manda a secao 2.2; e por
/// isso que ele nao e "nenhuma chave".
pub fn extrair(sal: &[u8], material: &[u8]) -> [u8; SHA256_LEN] {
    let zeros = [0u8; SHA256_LEN];
    let chave = if sal.is_empty() { &zeros[..] } else { sal };
    hmac_sha256(chave, material)
}

/// `HKDF-Expand(PRK, info, L)`: estica o PRK ate encher `saida`.
///
/// O contador de um byte no fim de cada bloco e o que limita a saida a 255
/// blocos; pedir mais que isso e erro, e nao um contador que da a volta.
pub fn expandir(prk: &[u8; SHA256_LEN], info: &[u8], saida: &mut [u8]) -> Result<()> {
    if saida.len() > MAX_SAIDA {
        return Err(PhxError::LimiteExcedido(format!(
            "HKDF-Expand nao produz mais que {MAX_SAIDA} bytes de uma vez, e \
             foram pedidos {}",
            saida.len()
        )));
    }
    let mut anterior: Vec<u8> = Vec::new();
    let mut posto = 0usize;
    let mut contador = 1u8;
    while posto < saida.len() {
        let mut entrada = Vec::with_capacity(anterior.len() + info.len() + 1);
        entrada.extend_from_slice(&anterior);
        entrada.extend_from_slice(info);
        entrada.push(contador);
        let bloco = hmac_sha256(prk, &entrada);
        let leva = bloco.len().min(saida.len() - posto);
        saida[posto..posto + leva].copy_from_slice(&bloco[..leva]);
        posto += leva;
        anterior = bloco.to_vec();
        contador = contador.wrapping_add(1);
    }
    Ok(())
}

/// Extrair e expandir de uma vez, que e como quase todo chamador usa.
pub fn derivar(sal: &[u8], material: &[u8], info: &[u8], saida: &mut [u8]) -> Result<()> {
    expandir(&extrair(sal, material), info, saida)
}

/// As duas saidas de 32 bytes que o aperto do Noise chama de `HKDF(ck, ikm, 2)`.
///
/// Existe como funcao propria porque e a forma que o `fio.rs` usa em toda
/// mistura de chave, e escrever o `info` vazio a cada chamada convidaria a
/// escrever um `info` errado numa delas.
pub fn duas(sal: &[u8], material: &[u8]) -> ([u8; SHA256_LEN], [u8; SHA256_LEN]) {
    let prk = extrair(sal, material);
    let mut saida = [0u8; SHA256_LEN * 2];
    // Nao pode falhar: 64 bytes estao muito abaixo do teto de 255 blocos.
    let _ = expandir(&prk, &[], &mut saida);
    let mut a = [0u8; SHA256_LEN];
    let mut b = [0u8; SHA256_LEN];
    a.copy_from_slice(&saida[..SHA256_LEN]);
    b.copy_from_slice(&saida[SHA256_LEN..]);
    (a, b)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hash::para_hex;

    /// RFC 5869, anexo A.1: o caso basico com SHA-256.
    #[test]
    fn caso_1_do_anexo_a() {
        let ikm = [0x0bu8; 22];
        let sal: Vec<u8> = (0u8..=0x0c).collect();
        let info: Vec<u8> = (0xf0u8..=0xf9).collect();
        let prk = extrair(&sal, &ikm);
        assert_eq!(
            para_hex(&prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
        let mut okm = [0u8; 42];
        expandir(&prk, &info, &mut okm).unwrap();
        assert_eq!(
            para_hex(&okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
             34007208d5b887185865"
        );
    }

    /// RFC 5869, anexo A.2: entradas longas -- 80 bytes de tudo, 82 de saida.
    ///
    /// E o caso que pega o contador de blocos: 82 bytes sao tres blocos, e o
    /// terceiro so sai certo se o bloco anterior entrar na conta do seguinte.
    #[test]
    fn caso_2_do_anexo_a() {
        let ikm: Vec<u8> = (0u8..0x50).collect();
        let sal: Vec<u8> = (0x60u8..0xb0).collect();
        let info: Vec<u8> = (0xb0u8..=0xff).collect();
        let prk = extrair(&sal, &ikm);
        assert_eq!(
            para_hex(&prk),
            "06a6b88c5853361a06104c9ceb35b45cef760014904671014a193f40c15fc244"
        );
        let mut okm = [0u8; 82];
        expandir(&prk, &info, &mut okm).unwrap();
        assert_eq!(
            para_hex(&okm),
            "b11e398dc80327a1c8e7f78c596a49344f012eda2d4efad8a050cc4c19afa97c\
             59045a99cac7827271cb41c65e590e09da3275600c2f09b8367793a9aca3db71\
             cc30c58179ec3e87c14c01d5c1f3434f1d87"
        );
    }

    /// RFC 5869, anexo A.3: sal e `info` vazios.
    ///
    /// O que ele guarda: sal vazio NAO e "sem chave" -- e um bloco de zeros do
    /// tamanho do hash. Quem passar `&[]` direto ao HMAC acha outro PRK e nao
    /// percebe, porque o resultado continua parecendo aleatorio.
    #[test]
    fn caso_3_do_anexo_a() {
        let ikm = [0x0bu8; 22];
        let prk = extrair(&[], &ikm);
        assert_eq!(
            para_hex(&prk),
            "19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04"
        );
        let mut okm = [0u8; 42];
        expandir(&prk, &[], &mut okm).unwrap();
        assert_eq!(
            para_hex(&okm),
            "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d\
             9d201395faa4b61a96c8"
        );
    }

    /// Pedir mais que 255 blocos e erro, e nao um contador que da a volta.
    #[test]
    fn passar_de_255_blocos_e_erro() {
        let prk = extrair(b"sal", b"material");
        let mut saida = vec![0u8; MAX_SAIDA + 1];
        assert!(expandir(&prk, b"", &mut saida).is_err());
        let mut no_teto = vec![0u8; MAX_SAIDA];
        assert!(expandir(&prk, b"", &mut no_teto).is_ok());
    }

    /// Rotulos diferentes sobre o MESMO PRK dao chaves diferentes -- e e disso
    /// que o `fio.rs` depende para a chave de ida nao ser a de volta.
    #[test]
    fn rotulos_diferentes_dao_chaves_diferentes() {
        let prk = extrair(b"sal", b"material");
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        expandir(&prk, b"ida", &mut a).unwrap();
        expandir(&prk, b"volta", &mut b).unwrap();
        assert_ne!(a, b);
    }

    /// As duas saidas de 32 bytes sao o prefixo e o sufixo dos 64.
    #[test]
    fn duas_sao_as_duas_metades_de_64() {
        let (a, b) = duas(b"cadeia", b"segredo");
        let prk = extrair(b"cadeia", b"segredo");
        let mut sessenta_e_quatro = [0u8; 64];
        expandir(&prk, b"", &mut sessenta_e_quatro).unwrap();
        assert_eq!(&a[..], &sessenta_e_quatro[..32]);
        assert_eq!(&b[..], &sessenta_e_quatro[32..]);
    }
}
