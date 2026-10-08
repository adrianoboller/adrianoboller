//! Verificacao de assinatura RSA -- PKCS#1 v1.5 e PSS (RFC 8017 §8.2.2 e
//! §8.1.2), pedido 572, T6c-1.
//!
//! So VERIFICA. O TLS com servidores de fora precisa conferir certificado e
//! `CertificateVerify` RSA (das 153 raizes do conteiner, 149 nao sao P-256 --
//! plano do 572), e verificar mexe so com dado publico: a aritmetica e a do
//! [`crate::bigint`], que nao e de tempo constante e nao precisa ser. Assinar
//! RSA aqui nao entra: as identidades desta casa sao P-256.
//!
//! O v1.5 confere por CODIFICAR E COMPARAR (a recomendacao da propria RFC 8017
//! §8.2.2, nota): monta o `EM` esperado e compara byte a byte, em vez de
//! analisar o `DigestInfo` que veio -- analisar e o caminho por onde passaram
//! as falsificacoes do tipo Bleichenbacher 2006 (lixo depois do hash, `NULL`
//! opcional, comprimento frouxo).

use crate::asn1;
use crate::bigint::{self, Modulo};
use crate::error::{PhxError, Result};

/// O resumo usado na assinatura.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resumo {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl Resumo {
    pub fn tamanho(self) -> usize {
        match self {
            Resumo::Sha1 => 20,
            Resumo::Sha256 => 32,
            Resumo::Sha384 => 48,
            Resumo::Sha512 => 64,
        }
    }

    pub fn calcular(self, dados: &[u8]) -> Vec<u8> {
        match self {
            Resumo::Sha1 => crate::sha1::sha1(dados).to_vec(),
            Resumo::Sha256 => crate::hash::sha256(dados).to_vec(),
            Resumo::Sha384 => crate::sha512::sha384(dados).to_vec(),
            Resumo::Sha512 => crate::sha512::sha512(dados).to_vec(),
        }
    }

    /// O `DigestInfo` sem o hash -- RFC 8017 §9.2, nota 1, copiado do texto.
    fn prefixo(self) -> &'static [u8] {
        match self {
            Resumo::Sha1 => &[
                0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04,
                0x14,
            ],
            Resumo::Sha256 => &[
                0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x01, 0x05, 0x00, 0x04, 0x20,
            ],
            Resumo::Sha384 => &[
                0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x02, 0x05, 0x00, 0x04, 0x30,
            ],
            Resumo::Sha512 => &[
                0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x03, 0x05, 0x00, 0x04, 0x40,
            ],
        }
    }
}

/// Uma chave publica RSA: o modulo `n` e o expoente `e`, em big-endian sem
/// zeros a esquerda.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChavePublica {
    n: Vec<u8>,
    e: Vec<u8>,
}

/// Menor modulo aceito, em bits. 1024 e o piso do que ainda circula (os
/// vetores do PKCS#1 v2.1 usam 1024); abaixo disso nao e assinatura, e
/// enfeite.
pub const BITS_MINIMOS: usize = 1024;
/// Maior modulo aceito: 16384 bits. O custo da verificacao cresce com o
/// quadrado do tamanho, e o certificado vem de quem ainda nao provou nada --
/// sem teto, um modulo gigante seria trabalho escolhido pelo outro lado.
pub const BITS_MAXIMOS: usize = 16384;

fn torto(m: &str) -> PhxError {
    PhxError::Corrompido(format!("RSA: {m}"))
}

impl ChavePublica {
    /// De `n` e `e` em big-endian. Recusa modulo par, fora da faixa de
    /// tamanho, e expoente par ou menor que 3.
    pub fn nova(n: &[u8], e: &[u8]) -> Result<ChavePublica> {
        let n = sem_zeros(n);
        let e = sem_zeros(e);
        let bits_n = bigint::bits(&bigint::de_be(n));
        if !(BITS_MINIMOS..=BITS_MAXIMOS).contains(&bits_n) {
            return Err(torto(&format!(
                "modulo de {bits_n} bits fora de {BITS_MINIMOS}..{BITS_MAXIMOS}"
            )));
        }
        if n.last().is_none_or(|b| b & 1 == 0) {
            return Err(torto("modulo par"));
        }
        let ee = bigint::de_be(e);
        if ee[0] & 1 == 0 || (bigint::bits(&ee) < 2) || e.len() > n.len() {
            return Err(torto("expoente publico invalido"));
        }
        Ok(ChavePublica {
            n: n.to_vec(),
            e: e.to_vec(),
        })
    }

    /// Do `RSAPublicKey` (RFC 8017 §A.1.1): `SEQUENCE { n INTEGER, e INTEGER }`,
    /// que e o que vai dentro do BIT STRING de um SPKI `rsaEncryption`.
    pub fn de_der(der: &[u8]) -> Result<ChavePublica> {
        let (seq, sobra) = asn1::esperar(der, asn1::TAG_SEQUENCE)?;
        if !sobra.is_empty() {
            return Err(torto("sobra depois do RSAPublicKey"));
        }
        let partes = asn1::filhos(seq.conteudo)?;
        let [n, e] = partes.as_slice() else {
            return Err(torto("RSAPublicKey sem os dois inteiros"));
        };
        if n.tag != asn1::TAG_INTEGER || e.tag != asn1::TAG_INTEGER {
            return Err(torto("RSAPublicKey com tipo errado"));
        }
        ChavePublica::nova(
            &asn1::decodificar_inteiro(n.conteudo)?,
            &asn1::decodificar_inteiro(e.conteudo)?,
        )
    }

    /// `k`, o tamanho do modulo em bytes.
    pub fn tamanho(&self) -> usize {
        self.n.len()
    }

    /// RSAVP1 com o I2OSP: `s^e mod n` em `tam` bytes. `None` quando a
    /// assinatura nao tem `k` bytes ou quando `s >= n` (RFC 8017 §8.2.2,
    /// passo 1, e §5.2.2) -- aceitar `s + n` daria duas assinaturas para a
    /// mesma mensagem, o que maleabiliza quem guarda assinatura como prova.
    fn abrir(&self, assinatura: &[u8], tam: usize) -> Option<Vec<u8>> {
        if assinatura.len() != self.n.len() {
            return None;
        }
        let md = Modulo::novo(&self.n)?;
        let s = md.ajustar(&bigint::de_be(assinatura))?;
        let m = md.sair(&md.potencia(&md.entrar(&s), &self.e));
        bigint::para_be(&m, tam)
    }
}

fn sem_zeros(b: &[u8]) -> &[u8] {
    let i = b.iter().position(|&x| x != 0).unwrap_or(b.len());
    &b[i..]
}

/// RSASSA-PKCS1-v1_5-VERIFY (RFC 8017 §8.2.2), por codificar e comparar.
pub fn verificar_pkcs1v15(
    chave: &ChavePublica,
    resumo: Resumo,
    mensagem: &[u8],
    assinatura: &[u8],
) -> bool {
    let k = chave.tamanho();
    let Some(em) = chave.abrir(assinatura, k) else {
        return false;
    };
    let h = resumo.calcular(mensagem);
    let t_len = resumo.prefixo().len() + h.len();
    // §9.2 passo 3: `emLen < tLen + 11` e «mensagem longa demais».
    if k < t_len + 11 {
        return false;
    }
    let mut esperado = Vec::with_capacity(k);
    esperado.extend_from_slice(&[0x00, 0x01]);
    esperado.resize(k - t_len - 1, 0xff);
    esperado.push(0x00);
    esperado.extend_from_slice(resumo.prefixo());
    esperado.extend_from_slice(&h);
    em == esperado
}

/// MGF1 (RFC 8017 §B.2.1).
fn mgf1(resumo: Resumo, semente: &[u8], tam: usize) -> Vec<u8> {
    let mut saida = Vec::with_capacity(tam + resumo.tamanho());
    let mut contador = 0u32;
    while saida.len() < tam {
        let mut entrada = semente.to_vec();
        entrada.extend_from_slice(&contador.to_be_bytes());
        saida.extend_from_slice(&resumo.calcular(&entrada));
        contador += 1;
    }
    saida.truncate(tam);
    saida
}

/// RSASSA-PSS-VERIFY (RFC 8017 §8.1.2 e EMSA-PSS-VERIFY, §9.1.2), com o mesmo
/// resumo no hash e no MGF1 e o sal de `tam_sal` bytes. O TLS 1.3 fixa o sal
/// no tamanho do resumo (RFC 8446 §4.2.3); os vetores do NIST e do PKCS#1
/// usam outros, e por isso ele e parametro.
pub fn verificar_pss(
    chave: &ChavePublica,
    resumo: Resumo,
    mensagem: &[u8],
    assinatura: &[u8],
    tam_sal: usize,
) -> bool {
    let mod_bits = bigint::bits(&bigint::de_be(&chave.n));
    let em_bits = mod_bits - 1;
    let em_len = em_bits.div_ceil(8);
    // §8.1.2 passo 2c: o I2OSP em `emLen` bytes -- se o modulo tem um bit a
    // mais que um multiplo de 8, o byte de cima do `m` tem de ser zero, e o
    // `para_be` recusa quando nao e.
    let Some(em) = chave.abrir(assinatura, em_len) else {
        return false;
    };
    let h_len = resumo.tamanho();
    let m_hash = resumo.calcular(mensagem);
    // Passos 3 e 4.
    if em_len < h_len + tam_sal + 2 || em[em_len - 1] != 0xbc {
        return false;
    }
    let (masked_db, resto) = em.split_at(em_len - h_len - 1);
    let h = &resto[..h_len];
    // Passo 6: os `8*emLen - emBits` bits de cima tem de vir zerados.
    let sobra = 8 * em_len - em_bits;
    let mascara_alta = if sobra == 0 {
        0u8
    } else {
        0xffu8 << (8 - sobra)
    };
    if masked_db[0] & mascara_alta != 0 {
        return false;
    }
    let db_mask = mgf1(resumo, h, masked_db.len());
    let mut db: Vec<u8> = masked_db.iter().zip(&db_mask).map(|(a, b)| a ^ b).collect();
    db[0] &= !mascara_alta;
    // Passo 10: zeros e depois exatamente um 0x01 antes do sal.
    let zeros = em_len - h_len - tam_sal - 2;
    if db[..zeros].iter().any(|&b| b != 0) || db[zeros] != 0x01 {
        return false;
    }
    let sal = &db[db.len() - tam_sal..];
    let mut m_linha = vec![0u8; 8];
    m_linha.extend_from_slice(&m_hash);
    m_linha.extend_from_slice(sal);
    resumo.calcular(&m_linha) == h
}

#[cfg(test)]
mod testes;
