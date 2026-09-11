//! Codec ASN.1 DER, escrito a mao e sem dependencia externa.
//!
//! # Para que serve aqui
//!
//! O correio nativo guarda a chave de cada par num arquivo PKCS#12 (`.p12`)
//! DE VERDADE -- para que `openssl`, um chaveiro de sistema ou um cliente
//! qualquer consigam ler. PKCS#12, X.509 e PKCS#8 sao todos ASN.1 codificado
//! em DER; sem um codec DER nao ha `.p12` interoperavel. Ver
//! `docs/CORREIO-P12.md`.
//!
//! # Por que DER, e nao "BER que parece DER"
//!
//! BER deixa escolher: comprimento indefinido, `0` codificado com bytes
//! sobrando, a mesma coisa de mais de um jeito. DER TIRA a escolha -- ha UMA
//! codificacao por valor, e so ela. Isso importa por dois motivos: quem assina
//! um certificado assina os BYTES da TBSCertificate, entao a codificacao tem
//! de ser reproduzivel bit a bit; e quem le da rede nao pode aceitar duas
//! formas do mesmo dado, porque a segunda forma e por onde entra o ataque.
//!
//! Por isso o leitor daqui RECUSA o que o DER proibe: comprimento na forma
//! longa quando cabia na curta, `INTEGER` com `0x00` sobrando na frente,
//! comprimento indefinido. "Aceitar por gentileza" abriria a porta que o DER
//! fecha de proposito.
//!
//! # O que este modulo NAO faz
//!
//! Nao interpreta o significado dos campos -- isso e do `x509.rs` e do futuro
//! `pkcs12.rs`. Aqui so mora a gramatica: tag, comprimento, conteudo, e o
//! encaixe recursivo. E nao trata tags de numero alto (>= 31, a forma de
//! multiplos bytes), porque nenhuma estrutura que este correio escreve ou le
//! precisa delas; se um dia precisar, o leitor recusa em vez de adivinhar.

use crate::error::{PhxError, Result};

// ------------------------------------------------------------------ as tags
//
// O byte de tag e classe (2 bits) | construido (1 bit) | numero (5 bits).
// Universais primitivas tem classe 00 e o bit de construido em 0; SEQUENCE e
// SET sao construidas (bit 5 ligado, 0x20). As de contexto explicitas ligam a
// classe 10 (0x80) e o bit de construido (0x20) -> 0xA0 | n.

pub const TAG_BOOLEAN: u8 = 0x01;
pub const TAG_INTEGER: u8 = 0x02;
pub const TAG_BIT_STRING: u8 = 0x03;
pub const TAG_OCTET_STRING: u8 = 0x04;
pub const TAG_NULL: u8 = 0x05;
pub const TAG_OID: u8 = 0x06;
pub const TAG_UTF8_STRING: u8 = 0x0c;
pub const TAG_PRINTABLE_STRING: u8 = 0x13;
pub const TAG_UTC_TIME: u8 = 0x17;
pub const TAG_GENERALIZED_TIME: u8 = 0x18;
pub const TAG_SEQUENCE: u8 = 0x30;
pub const TAG_SET: u8 = 0x31;

/// A tag de uma tag de contexto `[n]` explicita (construida).
pub const fn tag_contexto(n: u8) -> u8 {
    0xa0 | (n & 0x1f)
}

fn erro(motivo: &str) -> PhxError {
    // `Corrompido` e o erro de "estrutura interna inconsistente" -- e o que um
    // DER malformado e. A entrada vem de arquivo ou da rede, entao nada aqui
    // entra em panico.
    PhxError::Corrompido(format!("DER: {motivo}"))
}

// ============================================================ CODIFICACAO ==

/// Escreve o comprimento na forma DER: curta ate 127, longa acima -- e a longa
/// so quando a curta nao cabe, com o minimo de bytes.
fn escrever_comprimento(n: usize, saida: &mut Vec<u8>) {
    if n < 0x80 {
        saida.push(n as u8);
        return;
    }
    let bytes = n.to_be_bytes();
    let inicio = bytes
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(bytes.len() - 1);
    let significativos = &bytes[inicio..];
    saida.push(0x80 | significativos.len() as u8);
    saida.extend_from_slice(significativos);
}

/// Monta um TLV: tag, comprimento e conteudo.
fn tlv(tag: u8, conteudo: &[u8]) -> Vec<u8> {
    let mut saida = Vec::with_capacity(conteudo.len() + 4);
    saida.push(tag);
    escrever_comprimento(conteudo.len(), &mut saida);
    saida.extend_from_slice(conteudo);
    saida
}

/// Concatena os pedacos ja codificados -- o conteudo de uma SEQUENCE ou SET e
/// a colagem dos filhos, em ordem.
fn concatenar(itens: &[Vec<u8>]) -> Vec<u8> {
    let total = itens.iter().map(Vec::len).sum();
    let mut saida = Vec::with_capacity(total);
    for i in itens {
        saida.extend_from_slice(i);
    }
    saida
}

pub fn booleano(v: bool) -> Vec<u8> {
    // DER: verdadeiro e SEMPRE 0xFF (BER aceitaria qualquer byte nao-zero).
    tlv(TAG_BOOLEAN, &[if v { 0xff } else { 0x00 }])
}

/// Normaliza uma magnitude nao-negativa para a forma DER de um `INTEGER`:
/// tira zeros a frente, mas mantem um `0x00` quando o byte de topo tem o bit
/// alto ligado -- senao o numero seria lido como negativo.
fn normalizar_magnitude(magnitude_be: &[u8]) -> Vec<u8> {
    let inicio = magnitude_be
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(magnitude_be.len());
    let corpo = &magnitude_be[inicio..];
    if corpo.is_empty() {
        return vec![0x00]; // o zero e um unico octeto 0x00
    }
    let mut saida = Vec::with_capacity(corpo.len() + 1);
    if corpo[0] & 0x80 != 0 {
        saida.push(0x00);
    }
    saida.extend_from_slice(corpo);
    saida
}

/// `INTEGER` a partir de uma magnitude nao-negativa big-endian.
pub fn inteiro(magnitude_be: &[u8]) -> Vec<u8> {
    tlv(TAG_INTEGER, &normalizar_magnitude(magnitude_be))
}

/// `INTEGER` a partir de um `u64`.
pub fn inteiro_u64(n: u64) -> Vec<u8> {
    inteiro(&n.to_be_bytes())
}

fn codificar_base128(mut v: u64, saida: &mut Vec<u8>) {
    // Base 128, big-endian: o ultimo byte (menos significativo) tem o bit de
    // continuacao em 0, todos os anteriores em 1.
    let mut pilha = [0u8; 10];
    let mut n = 0;
    pilha[n] = (v & 0x7f) as u8;
    n += 1;
    v >>= 7;
    while v > 0 {
        pilha[n] = ((v & 0x7f) as u8) | 0x80;
        n += 1;
        v >>= 7;
    }
    for i in (0..n).rev() {
        saida.push(pilha[i]);
    }
}

/// `OBJECT IDENTIFIER` a partir dos arcos. Os dois primeiros arcos entram
/// juntos num unico numero, `40*arco0 + arco1`, como manda o X.690.
///
/// Espera ao menos dois arcos, com `arco0 <= 2`; abaixo disso nao ha OID.
pub fn oid(arcos: &[u64]) -> Vec<u8> {
    assert!(arcos.len() >= 2, "um OID tem ao menos dois arcos");
    assert!(arcos[0] <= 2, "o primeiro arco de um OID e 0, 1 ou 2");
    let mut conteudo = Vec::new();
    codificar_base128(40 * arcos[0] + arcos[1], &mut conteudo);
    for &a in &arcos[2..] {
        codificar_base128(a, &mut conteudo);
    }
    tlv(TAG_OID, &conteudo)
}

pub fn octet_string(bytes: &[u8]) -> Vec<u8> {
    tlv(TAG_OCTET_STRING, bytes)
}

/// `BIT STRING`: o primeiro octeto do conteudo diz quantos bits sobram sem uso
/// no ultimo byte. Chaves e assinaturas usam sempre `0`.
pub fn bit_string(bytes: &[u8], bits_nao_usados: u8) -> Vec<u8> {
    assert!(bits_nao_usados < 8, "sobram no maximo 7 bits");
    let mut conteudo = Vec::with_capacity(bytes.len() + 1);
    conteudo.push(bits_nao_usados);
    conteudo.extend_from_slice(bytes);
    tlv(TAG_BIT_STRING, &conteudo)
}

pub fn nulo() -> Vec<u8> {
    tlv(TAG_NULL, &[])
}

pub fn printable_string(s: &str) -> Vec<u8> {
    tlv(TAG_PRINTABLE_STRING, s.as_bytes())
}

pub fn utf8_string(s: &str) -> Vec<u8> {
    tlv(TAG_UTF8_STRING, s.as_bytes())
}

/// `UTCTime` no formato `YYMMDDHHMMSSZ`. O texto ja tem de vir pronto -- quem
/// monta a data e o `x509.rs`, que sabe a regra do RFC 5280 sobre quando usar
/// `UTCTime` (ate 2049) e quando usar `GeneralizedTime`.
pub fn utc_time(texto: &str) -> Vec<u8> {
    tlv(TAG_UTC_TIME, texto.as_bytes())
}

/// `GeneralizedTime` no formato `YYYYMMDDHHMMSSZ`.
pub fn generalized_time(texto: &str) -> Vec<u8> {
    tlv(TAG_GENERALIZED_TIME, texto.as_bytes())
}

pub fn sequencia(itens: &[Vec<u8>]) -> Vec<u8> {
    tlv(TAG_SEQUENCE, &concatenar(itens))
}

pub fn conjunto(itens: &[Vec<u8>]) -> Vec<u8> {
    tlv(TAG_SET, &concatenar(itens))
}

/// Tag de contexto `[n]` explicita: embrulha um (ou mais) valores JA
/// codificados. Explicita quer dizer "tag de contexto por fora, tag real por
/// dentro" -- e o que a versao `[0]` de um certificado usa.
pub fn contexto_explicito(n: u8, itens: &[Vec<u8>]) -> Vec<u8> {
    tlv(tag_contexto(n), &concatenar(itens))
}

// ============================================================== LEITURA ====

/// Um elemento lido: a tag e a fatia de conteudo, sem copia.
#[derive(Debug, Clone, Copy)]
pub struct Elemento<'a> {
    pub tag: u8,
    pub conteudo: &'a [u8],
}

/// Le o comprimento DER. Devolve (comprimento, bytes consumidos pelo campo de
/// comprimento). Recusa a forma indefinida e a forma longa nao-minima.
fn ler_comprimento(b: &[u8]) -> Result<(usize, usize)> {
    let primeiro = *b.first().ok_or_else(|| erro("faltou o comprimento"))?;
    if primeiro < 0x80 {
        return Ok((primeiro as usize, 1));
    }
    if primeiro == 0x80 {
        return Err(erro("comprimento indefinido nao existe em DER"));
    }
    let n = (primeiro & 0x7f) as usize;
    if n > 8 {
        return Err(erro("comprimento longo demais"));
    }
    if b.len() < 1 + n {
        return Err(erro("comprimento cortado"));
    }
    if b[1] == 0 {
        return Err(erro("comprimento longo com zero a frente nao e minimo"));
    }
    let mut len = 0usize;
    for &x in &b[1..1 + n] {
        len = (len << 8) | x as usize;
    }
    if len < 0x80 {
        return Err(erro("comprimento na forma longa quando cabia na curta"));
    }
    Ok((len, 1 + n))
}

/// Le UM TLV do inicio de `entrada` e devolve o elemento mais o que sobra.
pub fn analisar(entrada: &[u8]) -> Result<(Elemento<'_>, &[u8])> {
    let tag = *entrada.first().ok_or_else(|| erro("entrada vazia"))?;
    // Tag de numero alto (5 bits todos em 1) e a forma de multiplos bytes, que
    // este codec nao emite nem precisa ler.
    if tag & 0x1f == 0x1f {
        return Err(erro("tag de numero alto nao suportada"));
    }
    let (comprimento, bytes_len) = ler_comprimento(&entrada[1..])?;
    let inicio = 1 + bytes_len;
    let fim = inicio
        .checked_add(comprimento)
        .ok_or_else(|| erro("comprimento estoura"))?;
    if entrada.len() < fim {
        return Err(erro("conteudo cortado"));
    }
    Ok((
        Elemento {
            tag,
            conteudo: &entrada[inicio..fim],
        },
        &entrada[fim..],
    ))
}

/// Le um TLV e exige a tag esperada.
pub fn esperar(entrada: &[u8], tag: u8) -> Result<(Elemento<'_>, &[u8])> {
    let (el, resto) = analisar(entrada)?;
    if el.tag != tag {
        return Err(erro(&format!(
            "esperava a tag 0x{tag:02x}, veio 0x{:02x}",
            el.tag
        )));
    }
    Ok((el, resto))
}

/// Quebra o conteudo de um construido (SEQUENCE, SET, contexto) na lista dos
/// TLVs que ele carrega, do primeiro ao ultimo, sem sobra.
pub fn filhos(conteudo: &[u8]) -> Result<Vec<Elemento<'_>>> {
    let mut resto = conteudo;
    let mut saida = Vec::new();
    while !resto.is_empty() {
        let (el, r) = analisar(resto)?;
        saida.push(el);
        resto = r;
    }
    Ok(saida)
}

/// Le a magnitude de um `INTEGER` nao-negativo, conferindo a minimalidade do
/// DER e recusando um valor negativo (bit alto do primeiro byte).
pub fn decodificar_inteiro(conteudo: &[u8]) -> Result<Vec<u8>> {
    if conteudo.is_empty() {
        return Err(erro("INTEGER vazio"));
    }
    if conteudo.len() >= 2 && conteudo[0] == 0 && conteudo[1] & 0x80 == 0 {
        return Err(erro("INTEGER com 0x00 sobrando na frente"));
    }
    if conteudo.len() >= 2 && conteudo[0] == 0xff {
        return Err(erro("INTEGER com 0xff sobrando na frente"));
    }
    if conteudo[0] & 0x80 != 0 {
        return Err(erro("INTEGER negativo onde se esperava nao-negativo"));
    }
    // Tira o unico 0x00 de sinal, se houver, e devolve a magnitude.
    let inicio = usize::from(conteudo[0] == 0 && conteudo.len() > 1);
    Ok(conteudo[inicio..].to_vec())
}

/// Le um `OBJECT IDENTIFIER` de volta para os arcos.
pub fn decodificar_oid(conteudo: &[u8]) -> Result<Vec<u64>> {
    if conteudo.is_empty() {
        return Err(erro("OID vazio"));
    }
    let mut arcos = Vec::new();
    let mut i = 0;
    let mut primeiro = true;
    while i < conteudo.len() {
        let mut v: u64 = 0;
        loop {
            let b = conteudo[i];
            // Um byte de continuacao com v ainda em 0 seria um 0x80 inicial:
            // codificacao nao-minima, proibida em DER.
            if v == 0 && b == 0x80 {
                return Err(erro("arco de OID com 0x80 a frente nao e minimo"));
            }
            v = v
                .checked_shl(7)
                .ok_or_else(|| erro("arco de OID grande demais"))?
                | u64::from(b & 0x7f);
            i += 1;
            if b & 0x80 == 0 {
                break;
            }
            if i >= conteudo.len() {
                return Err(erro("arco de OID cortado"));
            }
        }
        if primeiro {
            // Desfaz o 40*arco0 + arco1 do primeiro par.
            let (a0, a1) = if v < 40 {
                (0, v)
            } else if v < 80 {
                (1, v - 40)
            } else {
                (2, v - 80)
            };
            arcos.push(a0);
            arcos.push(a1);
            primeiro = false;
        } else {
            arcos.push(v);
        }
    }
    Ok(arcos)
}

/// Le um `BIT STRING`: devolve (bits nao usados, os bytes).
pub fn decodificar_bit_string(conteudo: &[u8]) -> Result<(u8, Vec<u8>)> {
    let (&nao_usados, resto) = conteudo
        .split_first()
        .ok_or_else(|| erro("BIT STRING vazio"))?;
    if nao_usados >= 8 {
        return Err(erro("BIT STRING diz sobrar 8 ou mais bits"));
    }
    if nao_usados > 0 && resto.is_empty() {
        return Err(erro("BIT STRING sem byte mas com bits sobrando"));
    }
    Ok((nao_usados, resto.to_vec()))
}

/// Le um `BOOLEAN`, exigindo a forma DER (0x00 ou 0xFF).
pub fn decodificar_booleano(conteudo: &[u8]) -> Result<bool> {
    match conteudo {
        [0x00] => Ok(false),
        [0xff] => Ok(true),
        _ => Err(erro("BOOLEAN fora da forma DER (so 0x00 ou 0xFF)")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A ida e a volta de cada tipo: codificar e ler de volta tem de dar o
    /// mesmo valor, e a codificacao tem de ser reproduzivel byte a byte.
    #[test]
    fn ida_e_volta_inteiro() {
        // (magnitude de entrada, magnitude que a leitura tem de devolver)
        let casos: [(Vec<u8>, Vec<u8>); 8] = [
            (vec![0x00], vec![0x00]), // o zero
            (vec![0x01], vec![0x01]),
            (vec![0x7f], vec![0x7f]),
            (vec![0x80], vec![0x80]), // bit alto: ganha 0x00 de sinal na fita
            (vec![0xff], vec![0xff]),
            (vec![0x01, 0x00], vec![0x01, 0x00]), // 256
            (vec![0x00, 0x00, 0x2a], vec![0x2a]), // zeros a frente normalizam
            (
                (0..20).map(|i| 0x80u8 | i).collect(),
                (0..20).map(|i| 0x80u8 | i).collect(),
            ), // serie de 20 bytes com o bit alto ligado
        ];
        for (magnitude, esperada) in casos {
            let der = inteiro(&magnitude);
            let (el, resto) = esperar(&der, TAG_INTEGER).unwrap();
            assert!(resto.is_empty(), "sobrou byte");
            let volta = decodificar_inteiro(el.conteudo).unwrap();
            assert_eq!(volta, esperada, "magnitude de volta errada");
            // Reserializar tem de reproduzir os mesmos bytes: DER e unico.
            assert_eq!(inteiro(&volta), der, "DER nao e reproduzivel");
        }
    }

    #[test]
    fn inteiro_recusa_forma_nao_minima() {
        // 0x00 0x2a: o 0x00 e desnecessario (0x2a ja e positivo) -> recusa.
        assert!(decodificar_inteiro(&[0x00, 0x2a]).is_err());
        // 0xff a frente de um nao-negativo -> recusa.
        assert!(decodificar_inteiro(&[0xff, 0x01]).is_err());
        // negativo (bit alto) onde se espera nao-negativo -> recusa.
        assert!(decodificar_inteiro(&[0x80]).is_err());
        // vazio -> recusa.
        assert!(decodificar_inteiro(&[]).is_err());
        // 0x00 sozinho e o zero, valido.
        assert_eq!(decodificar_inteiro(&[0x00]).unwrap(), vec![0x00]);
        // 0x00 0x80 e valido: o 0x80 precisa do sinal.
        assert_eq!(decodificar_inteiro(&[0x00, 0x80]).unwrap(), vec![0x80]);
    }

    #[test]
    fn ida_e_volta_oid() {
        for arcos in [
            vec![1u64, 3, 101, 112],           // id-Ed25519
            vec![1, 3, 101, 110],              // id-X25519
            vec![2, 5, 4, 3],                  // CN
            vec![2, 5, 29, 19],                // basicConstraints
            vec![1, 2, 840, 113_549, 1, 1, 1], // rsaEncryption (arco grande)
            vec![0, 39],
            vec![2, 999, 1234567890],
        ] {
            let der = oid(&arcos);
            let (el, resto) = esperar(&der, TAG_OID).unwrap();
            assert!(resto.is_empty());
            assert_eq!(decodificar_oid(el.conteudo).unwrap(), arcos);
            assert_eq!(oid(&decodificar_oid(el.conteudo).unwrap()), der);
        }
    }

    #[test]
    fn oid_recusa_arco_nao_minimo() {
        // 0x80 a frente de um arco = codificacao nao-minima.
        assert!(decodificar_oid(&[0x2b, 0x80, 0x01]).is_err());
    }

    #[test]
    fn ida_e_volta_bit_string() {
        let bytes = [0xde, 0xad, 0xbe, 0xef];
        let der = bit_string(&bytes, 0);
        let (el, _) = esperar(&der, TAG_BIT_STRING).unwrap();
        let (nao_usados, volta) = decodificar_bit_string(el.conteudo).unwrap();
        assert_eq!(nao_usados, 0);
        assert_eq!(volta, bytes);
        assert_eq!(bit_string(&volta, nao_usados), der);
    }

    #[test]
    fn ida_e_volta_booleano() {
        for v in [true, false] {
            let der = booleano(v);
            let (el, _) = esperar(&der, TAG_BOOLEAN).unwrap();
            assert_eq!(decodificar_booleano(el.conteudo).unwrap(), v);
        }
        // BER aceitaria 0x01 como verdadeiro; DER nao.
        assert!(decodificar_booleano(&[0x01]).is_err());
    }

    #[test]
    fn ida_e_volta_octet_e_strings() {
        let bytes = b"\x00\x01\x02phxsql\xff";
        let der = octet_string(bytes);
        let (el, _) = esperar(&der, TAG_OCTET_STRING).unwrap();
        assert_eq!(el.conteudo, bytes);

        let der = utf8_string("Adriano Boller");
        let (el, _) = esperar(&der, TAG_UTF8_STRING).unwrap();
        assert_eq!(el.conteudo, b"Adriano Boller");

        let der = printable_string("PhxSql");
        let (el, _) = esperar(&der, TAG_PRINTABLE_STRING).unwrap();
        assert_eq!(el.conteudo, b"PhxSql");

        let der = utc_time("260911120000Z");
        let (el, _) = esperar(&der, TAG_UTC_TIME).unwrap();
        assert_eq!(el.conteudo, b"260911120000Z");

        let der = generalized_time("20500911120000Z");
        let (el, _) = esperar(&der, TAG_GENERALIZED_TIME).unwrap();
        assert_eq!(el.conteudo, b"20500911120000Z");
    }

    #[test]
    fn sequencia_e_filhos() {
        let seq = sequencia(&[inteiro_u64(2), oid(&[1, 3, 101, 112]), nulo()]);
        let (el, resto) = esperar(&seq, TAG_SEQUENCE).unwrap();
        assert!(resto.is_empty());
        let fs = filhos(el.conteudo).unwrap();
        assert_eq!(fs.len(), 3);
        assert_eq!(fs[0].tag, TAG_INTEGER);
        assert_eq!(decodificar_inteiro(fs[0].conteudo).unwrap(), vec![0x02]);
        assert_eq!(fs[1].tag, TAG_OID);
        assert_eq!(
            decodificar_oid(fs[1].conteudo).unwrap(),
            vec![1, 3, 101, 112]
        );
        assert_eq!(fs[2].tag, TAG_NULL);
    }

    #[test]
    fn contexto_explicito_embrulha_e_desembrulha() {
        let versao = contexto_explicito(0, &[inteiro_u64(2)]);
        assert_eq!(versao[0], 0xa0);
        let (el, _) = esperar(&versao, tag_contexto(0)).unwrap();
        let dentro = filhos(el.conteudo).unwrap();
        assert_eq!(dentro.len(), 1);
        assert_eq!(decodificar_inteiro(dentro[0].conteudo).unwrap(), vec![0x02]);
    }

    /// Comprimento na forma longa quando cabia na curta: DER proibe.
    #[test]
    fn leitor_recusa_comprimento_nao_minimo() {
        // tag OCTET STRING, comprimento 0x81 0x01 (forma longa dizendo "1"),
        // um byte de conteudo. Deveria ser so 0x04 0x01 0x00.
        assert!(analisar(&[0x04, 0x81, 0x01, 0x00]).is_err());
        // comprimento indefinido (0x80).
        assert!(analisar(&[0x30, 0x80, 0x00, 0x00]).is_err());
    }

    /// Um comprimento longo legitimo (conteudo > 127 bytes) vai e volta.
    #[test]
    fn comprimento_longo_legitimo() {
        let recheio = vec![0x41u8; 300];
        let der = octet_string(&recheio);
        // 0x04, 0x82, 0x01, 0x2C, depois os 300 bytes.
        assert_eq!(&der[..4], &[0x04, 0x82, 0x01, 0x2c]);
        let (el, resto) = esperar(&der, TAG_OCTET_STRING).unwrap();
        assert!(resto.is_empty());
        assert_eq!(el.conteudo, recheio.as_slice());
    }
}
