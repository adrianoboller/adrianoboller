//! Validacao de cadeia X.509 (RFC 5280 §6.1, subconjunto declarado) e do
//! nome do servidor (RFC 9525) -- pedido 572, T6c-2.
//!
//! # Para que existe
//!
//! Entre dois PhxSql o TLS confia por PINO (T6b). Para falar com servidor de
//! FORA (PostgreSQL, MySQL, SMTP -- a T6d) nao ha pino a combinar: ha uma
//! autoridade, e a pergunta e se a folha mostrada descende dela e se chama
//! quem se queria.
//!
//! # O que este validador faz, e o que NAO faz (decidido, nao esquecido)
//!
//! Faz, pela §6.1: assinatura de cada elo pela chave do anterior (RSA PKCS#1
//! v1.5 e PSS-nao, ECDSA P-256/P-384, Ed25519), validade no instante dado,
//! encadeamento de NOME com a comparacao da §7.1 (caixa e espacos), `cA` e
//! `pathLenConstraint` (§4.2.1.9, com o certificado auto-emitido fora da
//! conta), `keyCertSign` (§4.2.1.3), restricoes de nome (§4.2.1.10: nome
//! distinto, e-mail, DNS, URI e IP) e extensao critica desconhecida recusada.
//!
//! NAO faz, e cada um tem o motivo escrito:
//!
//! * **revogacao (CRL/OCSP)** -- nenhum dos clientes TLS de referencia a faz
//!   na validacao comum (o Go e o `webpki` nao consultam; os navegadores usam
//!   listas proprias empurradas, nao a CRL do certificado). Buscar CRL na rede
//!   no meio de um aperto seria outra conexao, sem prazo nosso;
//! * **politicas** (`certificatePolicies` e companhia, §6.1.3 d-j) -- a mesma
//!   convergencia: o Go e o `webpki` nao as processam para servidor TLS. As
//!   extensoes de politica que obrigam algo (`policyConstraints`,
//!   `policyMappings`, `inhibitAnyPolicy`), quando CRITICAS, recusam -- e a
//!   recusa fechada da §4.2, nunca a aceitacao calada;
//! * **assinatura DSA e SHA-1** -- recusadas nomeando o algoritmo.
//!
//! Tudo aqui mexe com dado publico: nao ha tempo constante a guardar.

use crate::asn1::{self, Elemento};
use crate::error::{PhxError, Result};

fn torto(m: &str) -> PhxError {
    PhxError::Corrompido(format!("X.509: {m}"))
}

fn recusa(m: String) -> PhxError {
    PhxError::Autorizacao(format!("cadeia X.509: {m}"))
}

// --------------------------------------------------------------- OIDs ----

const OID_BASIC_CONSTRAINTS: &[u64] = &[2, 5, 29, 19];
const OID_KEY_USAGE: &[u64] = &[2, 5, 29, 15];
const OID_EXT_KEY_USAGE: &[u64] = &[2, 5, 29, 37];
const OID_SAN: &[u64] = &[2, 5, 29, 17];
const OID_NAME_CONSTRAINTS: &[u64] = &[2, 5, 29, 30];
const OID_SKI: &[u64] = &[2, 5, 29, 14];
const OID_AKI: &[u64] = &[2, 5, 29, 35];
const OID_CERT_POLICIES: &[u64] = &[2, 5, 29, 32];
const OID_CRL_DP: &[u64] = &[2, 5, 29, 31];
const OID_AIA: &[u64] = &[1, 3, 6, 1, 5, 5, 7, 1, 1];
const OID_SERVER_AUTH: &[u64] = &[1, 3, 6, 1, 5, 5, 7, 3, 1];
const OID_EMAIL: &[u64] = &[1, 2, 840, 113549, 1, 9, 1];

const OID_RSA_SHA256: &[u64] = &[1, 2, 840, 113549, 1, 1, 11];
const OID_RSA_SHA384: &[u64] = &[1, 2, 840, 113549, 1, 1, 12];
const OID_RSA_SHA512: &[u64] = &[1, 2, 840, 113549, 1, 1, 13];
const OID_ECDSA_SHA256: &[u64] = &[1, 2, 840, 10045, 4, 3, 2];
const OID_ECDSA_SHA384: &[u64] = &[1, 2, 840, 10045, 4, 3, 3];
const OID_ECDSA_SHA512: &[u64] = &[1, 2, 840, 10045, 4, 3, 4];
const OID_ED25519: &[u64] = &[1, 3, 101, 112];

/// As extensoes que este validador ENTENDE -- critica fora desta lista
/// recusa (§4.2). A politica de certificado entra como entendida e ignorada
/// (ver o cabecalho); as de politica que obrigam algo, nao.
const ENTENDIDAS: &[&[u64]] = &[
    OID_BASIC_CONSTRAINTS,
    OID_KEY_USAGE,
    OID_EXT_KEY_USAGE,
    OID_SAN,
    OID_NAME_CONSTRAINTS,
    OID_SKI,
    OID_AKI,
    OID_CERT_POLICIES,
    OID_CRL_DP,
    OID_AIA,
];

/// Maior cadeia que se tenta montar: folha, ate oito intermediarias e a
/// ancora. Cadeia publica real tem 3 ou 4; o teto e contra quem manda um
/// novelo de certificados para fazer este lado verificar assinatura a toa.
pub const PROFUNDIDADE_MAXIMA: usize = 10;

// ---------------------------------------------------------- o parser ----

/// Uma extensao: OID, criticidade e o valor (o conteudo do OCTET STRING).
#[derive(Debug, Clone)]
pub struct Extensao<'a> {
    pub oid: Vec<u64>,
    pub critica: bool,
    pub valor: &'a [u8],
}

/// Um certificado aberto o bastante para validar.
#[derive(Debug, Clone)]
pub struct Certificado<'a> {
    pub der: &'a [u8],
    /// O `tbsCertificate` inteiro (o TLV), que e o que se assina.
    pub tbs: &'a [u8],
    pub alg: Vec<u64>,
    pub assinatura: Vec<u8>,
    pub versao: u8,
    /// Os nomes CRUS (o TLV do `Name`).
    pub emissor: &'a [u8],
    pub sujeito: &'a [u8],
    /// Segundos desde a epoca.
    pub nao_antes: i64,
    pub nao_depois: i64,
    pub spki: &'a [u8],
    pub extensoes: Vec<Extensao<'a>>,
}

/// O TLV inteiro de um elemento, a partir do resto antes e depois dele.
fn tlv<'a>(antes: &'a [u8], depois: &[u8]) -> &'a [u8] {
    &antes[..antes.len() - depois.len()]
}

/// UTCTime `AAMMDDHHMMSSZ` ou GeneralizedTime `AAAAMMDDHHMMSSZ` (§4.1.2.5:
/// so essas formas, sempre em Zulu e com segundos) para segundos.
fn tempo(el: &Elemento) -> Result<i64> {
    let t = el.conteudo;
    let (ano, resto) = match el.tag {
        asn1::TAG_UTC_TIME if t.len() == 13 => {
            let aa = digitos(&t[..2])? as i32;
            // §4.1.2.5.1: 50..99 e 19xx, 00..49 e 20xx.
            (if aa >= 50 { 1900 + aa } else { 2000 + aa }, &t[2..])
        }
        asn1::TAG_GENERALIZED_TIME if t.len() == 15 => (digitos(&t[..4])? as i32, &t[4..]),
        _ => return Err(torto("tempo fora das duas formas da RFC 5280 §4.1.2.5")),
    };
    if resto[10] != b'Z' {
        return Err(torto("tempo sem o Z"));
    }
    let mes = digitos(&resto[0..2])?;
    let dia = digitos(&resto[2..4])?;
    let (h, m, s) = (
        digitos(&resto[4..6])?,
        digitos(&resto[6..8])?,
        digitos(&resto[8..10])?,
    );
    if !(1..=12).contains(&mes) || !(1..=31).contains(&dia) || h > 23 || m > 59 || s > 59 {
        return Err(torto("tempo com campo fora da faixa"));
    }
    let dias = crate::datahora::dias_de_civil(ano, mes, dia) as i64;
    Ok(dias * 86_400 + (h * 3600 + m * 60 + s) as i64)
}

fn digitos(b: &[u8]) -> Result<u32> {
    let mut v = 0u32;
    for &c in b {
        if !c.is_ascii_digit() {
            return Err(torto("digito esperado no tempo"));
        }
        v = v * 10 + (c - b'0') as u32;
    }
    Ok(v)
}

fn alg_de(el: &Elemento) -> Result<Vec<u64>> {
    if el.tag != asn1::TAG_SEQUENCE {
        return Err(torto("AlgorithmIdentifier nao e SEQUENCE"));
    }
    let partes = asn1::filhos(el.conteudo)?;
    match partes.first() {
        Some(o) if o.tag == asn1::TAG_OID => asn1::decodificar_oid(o.conteudo),
        _ => Err(torto("AlgorithmIdentifier sem OID")),
    }
}

impl<'a> Certificado<'a> {
    pub fn analisar(der: &'a [u8]) -> Result<Certificado<'a>> {
        let (c, sobra) = asn1::esperar(der, asn1::TAG_SEQUENCE)?;
        if !sobra.is_empty() {
            return Err(torto("sobra depois do certificado"));
        }
        let partes = asn1::filhos(c.conteudo)?;
        let [tbs_el, alg_el, ass_el] = partes.as_slice() else {
            return Err(torto("certificado sem os tres campos"));
        };
        if tbs_el.tag != asn1::TAG_SEQUENCE || ass_el.tag != asn1::TAG_BIT_STRING {
            return Err(torto("certificado com tipo errado"));
        }
        let alg = alg_de(alg_el)?;
        let (nao_usados, assinatura) = asn1::decodificar_bit_string(ass_el.conteudo)?;
        if nao_usados != 0 {
            return Err(torto("assinatura com bits sobrando"));
        }
        let (_, depois_tbs) = asn1::analisar(c.conteudo)?;
        let tbs = tlv(c.conteudo, depois_tbs);

        let mut r = tbs_el.conteudo;
        let mut versao = 1u8;
        let (el, prox) = asn1::analisar(r)?;
        if el.tag == asn1::tag_contexto(0) {
            let (v, _) = asn1::esperar(el.conteudo, asn1::TAG_INTEGER)?;
            let v = asn1::decodificar_inteiro(v.conteudo)?;
            versao = match v.as_slice() {
                [0] => 1,
                [1] => 2,
                [2] => 3,
                _ => return Err(torto("versao desconhecida")),
            };
            r = prox;
        }
        // Numero de serie: so se pula -- ha certificado com serie negativa,
        // e o valor nao decide nada aqui.
        let (serie, prox) = asn1::analisar(r)?;
        if serie.tag != asn1::TAG_INTEGER {
            return Err(torto("serie nao e INTEGER"));
        }
        r = prox;
        let (alg_tbs, prox) = asn1::analisar(r)?;
        // §4.1.1.2: o algoritmo de dentro tem de ser o de fora.
        if alg_de(&alg_tbs)? != alg {
            return Err(torto(
                "o algoritmo do tbsCertificate nao e o do certificado",
            ));
        }
        r = prox;
        let (emissor_el, prox) = asn1::analisar(r)?;
        let emissor = tlv(r, prox);
        if emissor_el.tag != asn1::TAG_SEQUENCE {
            return Err(torto("emissor nao e Name"));
        }
        r = prox;
        let (validade, prox) = asn1::esperar(r, asn1::TAG_SEQUENCE)?;
        let vs = asn1::filhos(validade.conteudo)?;
        let [a, b] = vs.as_slice() else {
            return Err(torto("validade sem os dois tempos"));
        };
        let (nao_antes, nao_depois) = (tempo(a)?, tempo(b)?);
        r = prox;
        let (sujeito_el, prox) = asn1::analisar(r)?;
        let sujeito = tlv(r, prox);
        if sujeito_el.tag != asn1::TAG_SEQUENCE {
            return Err(torto("sujeito nao e Name"));
        }
        r = prox;
        let (spki_el, prox) = asn1::analisar(r)?;
        if spki_el.tag != asn1::TAG_SEQUENCE {
            return Err(torto("subjectPublicKeyInfo fora do lugar"));
        }
        let spki = tlv(r, prox);
        r = prox;
        let mut extensoes = Vec::new();
        while !r.is_empty() {
            let (el, prox) = asn1::analisar(r)?;
            r = prox;
            match el.tag {
                // issuerUniqueID e subjectUniqueID (§4.1.2.8): so se pulam.
                0x81 | 0x82 | 0xa1 | 0xa2 => {}
                t if t == asn1::tag_contexto(3) => {
                    if versao != 3 {
                        return Err(torto("extensoes num certificado que nao e v3"));
                    }
                    let (seq, sobra) = asn1::esperar(el.conteudo, asn1::TAG_SEQUENCE)?;
                    if !sobra.is_empty() {
                        return Err(torto("sobra depois das extensoes"));
                    }
                    for e in asn1::filhos(seq.conteudo)? {
                        let p = asn1::filhos(e.conteudo)?;
                        let (oid, critica, valor) = match p.as_slice() {
                            [o, v] => (o, false, v),
                            [o, c, v] if c.tag == asn1::TAG_BOOLEAN => {
                                (o, asn1::decodificar_booleano(c.conteudo)?, v)
                            }
                            _ => return Err(torto("extensao malformada")),
                        };
                        if oid.tag != asn1::TAG_OID || valor.tag != asn1::TAG_OCTET_STRING {
                            return Err(torto("extensao com tipo errado"));
                        }
                        let oid = asn1::decodificar_oid(oid.conteudo)?;
                        if extensoes.iter().any(|x: &Extensao| x.oid == oid) {
                            // §4.2: «A certificate MUST NOT include more than
                            // one instance of a particular extension».
                            return Err(torto("extensao repetida"));
                        }
                        extensoes.push(Extensao {
                            oid,
                            critica,
                            valor: valor.conteudo,
                        });
                    }
                }
                _ => return Err(torto("campo desconhecido no tbsCertificate")),
            }
        }
        Ok(Certificado {
            der,
            tbs,
            alg,
            assinatura,
            versao,
            emissor,
            sujeito,
            nao_antes,
            nao_depois,
            spki,
            extensoes,
        })
    }

    pub fn extensao(&self, oid: &[u64]) -> Option<&Extensao<'a>> {
        self.extensoes.iter().find(|e| e.oid == oid)
    }

    /// `basicConstraints`: (cA, pathLenConstraint).
    fn restricoes_basicas(&self) -> Result<Option<(bool, Option<u64>)>> {
        let Some(e) = self.extensao(OID_BASIC_CONSTRAINTS) else {
            return Ok(None);
        };
        let (seq, sobra) = asn1::esperar(e.valor, asn1::TAG_SEQUENCE)?;
        if !sobra.is_empty() {
            return Err(torto("sobra no basicConstraints"));
        }
        let mut ca = false;
        let mut len = None;
        for el in asn1::filhos(seq.conteudo)? {
            match el.tag {
                asn1::TAG_BOOLEAN => ca = asn1::decodificar_booleano(el.conteudo)?,
                asn1::TAG_INTEGER => {
                    let v = asn1::decodificar_inteiro(el.conteudo)?;
                    if v.len() > 4 {
                        return Err(torto("pathLenConstraint grande demais"));
                    }
                    len = Some(v.iter().fold(0u64, |a, b| (a << 8) | *b as u64));
                }
                _ => return Err(torto("basicConstraints malformado")),
            }
        }
        Ok(Some((ca, len)))
    }

    /// Os bits do `keyUsage` (bit 0 = digitalSignature, 5 = keyCertSign).
    fn uso(&self) -> Result<Option<u16>> {
        let Some(e) = self.extensao(OID_KEY_USAGE) else {
            return Ok(None);
        };
        let (bs, _) = asn1::esperar(e.valor, asn1::TAG_BIT_STRING)?;
        let (_, b) = asn1::decodificar_bit_string(bs.conteudo)?;
        let mut v = 0u16;
        for (i, byte) in b.iter().take(2).enumerate() {
            for j in 0..8 {
                if byte & (0x80 >> j) != 0 {
                    v |= 1 << (8 * i + j);
                }
            }
        }
        Ok(Some(v))
    }

    /// Os OIDs do `extKeyUsage`, quando ha.
    fn usos_estendidos(&self) -> Result<Option<Vec<Vec<u64>>>> {
        let Some(e) = self.extensao(OID_EXT_KEY_USAGE) else {
            return Ok(None);
        };
        let (seq, _) = asn1::esperar(e.valor, asn1::TAG_SEQUENCE)?;
        asn1::filhos(seq.conteudo)?
            .iter()
            .map(|o| asn1::decodificar_oid(o.conteudo))
            .collect::<Result<Vec<_>>>()
            .map(Some)
    }

    /// Os nomes do `subjectAltName` (§4.2.1.6): (tag, conteudo).
    fn nomes_alternativos(&self) -> Result<Vec<(u8, &'a [u8])>> {
        let Some(e) = self.extensao(OID_SAN) else {
            return Ok(Vec::new());
        };
        let (seq, sobra) = asn1::esperar(e.valor, asn1::TAG_SEQUENCE)?;
        if !sobra.is_empty() {
            return Err(torto("sobra no subjectAltName"));
        }
        Ok(asn1::filhos(seq.conteudo)?
            .into_iter()
            .map(|g| (g.tag, g.conteudo))
            .collect())
    }

    fn auto_emitido(&self) -> Result<bool> {
        nomes_iguais(self.emissor, self.sujeito)
    }
}

// ---------------------------------------------- nomes distintos (§7.1) ----

/// Um atributo de nome normalizado: o OID e o valor comparavel.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Atributo {
    oid: Vec<u64>,
    /// `Ok(texto)` normalizado quando e cadeia de texto; `Err(tlv)` cru
    /// quando nao e -- compara byte a byte.
    valor: std::result::Result<String, Vec<u8>>,
}

/// O texto de uma cadeia ASN.1 de texto, ou `None` quando o tipo nao e de
/// texto.
fn texto_asn1(tag: u8, b: &[u8]) -> Option<String> {
    match tag {
        // PrintableString, IA5String, UTF8String, VisibleString
        0x13 | 0x16 | 0x0c | 0x1a => String::from_utf8(b.to_vec()).ok(),
        // TeletexString: lido como Latin-1, que e como o mundo o usa.
        0x14 => Some(b.iter().map(|&c| c as char).collect()),
        // BMPString (UTF-16BE)
        0x1e => {
            if !b.len().is_multiple_of(2) {
                return None;
            }
            let u: Vec<u16> = b
                .chunks(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&u).ok()
        }
        // UniversalString (UTF-32BE)
        0x1c => {
            if !b.len().is_multiple_of(4) {
                return None;
            }
            b.chunks(4)
                .map(|c| char::from_u32(u32::from_be_bytes([c[0], c[1], c[2], c[3]])))
                .collect()
        }
        _ => None,
    }
}

/// A comparacao da §7.1 (RFC 4518, o essencial): sem espaco nas pontas,
/// espaco interno colapsado, caixa dobrada. E o que faz «Test CA» casar com
/// «test  ca», e um PrintableString casar com o mesmo texto em UTF8String.
fn normalizar(t: &str) -> String {
    t.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Um `Name` como lista de RDNs, cada um um conjunto ORDENADO de atributos.
fn rdns(nome: &[u8]) -> Result<Vec<Vec<Atributo>>> {
    let (seq, sobra) = asn1::esperar(nome, asn1::TAG_SEQUENCE)?;
    if !sobra.is_empty() {
        return Err(torto("sobra depois do Name"));
    }
    let mut saida = Vec::new();
    for rdn in asn1::filhos(seq.conteudo)? {
        if rdn.tag != asn1::TAG_SET {
            return Err(torto("RDN nao e SET"));
        }
        let mut atributos = Vec::new();
        for atv in asn1::filhos(rdn.conteudo)? {
            let p = asn1::filhos(atv.conteudo)?;
            let [o, v] = p.as_slice() else {
                return Err(torto("atributo de nome malformado"));
            };
            let oid = asn1::decodificar_oid(o.conteudo)?;
            let valor = match texto_asn1(v.tag, v.conteudo) {
                Some(t) => Ok(normalizar(&t)),
                None => {
                    let mut cru = vec![v.tag];
                    cru.extend_from_slice(v.conteudo);
                    Err(cru)
                }
            };
            atributos.push(Atributo { oid, valor });
        }
        if atributos.is_empty() {
            return Err(torto("RDN vazio"));
        }
        atributos.sort();
        saida.push(atributos);
    }
    Ok(saida)
}

fn nomes_iguais(a: &[u8], b: &[u8]) -> Result<bool> {
    Ok(rdns(a)? == rdns(b)?)
}

// --------------------------------------------- restricoes de nome ----

/// As restricoes de nome de UMA autoridade do caminho.
#[derive(Debug, Clone, Default)]
struct Restricoes {
    permitidas: Vec<(u8, Vec<u8>)>,
    excluidas: Vec<(u8, Vec<u8>)>,
}

/// As formas de nome que se sabem conferir: rfc822Name [1], dNSName [2],
/// directoryName [4], URI [6], iPAddress [7]. Restricao de outra forma
/// recusa a cadeia -- nao se sabe dizer se um nome dela cabe.
const FORMAS: [u8; 5] = [0x81, 0x82, 0xa4, 0x86, 0x87];

fn restricoes_de(valor: &[u8]) -> Result<Restricoes> {
    let (seq, sobra) = asn1::esperar(valor, asn1::TAG_SEQUENCE)?;
    if !sobra.is_empty() {
        return Err(torto("sobra no nameConstraints"));
    }
    let mut r = Restricoes::default();
    for el in asn1::filhos(seq.conteudo)? {
        let alvo = match el.tag {
            0xa0 => &mut r.permitidas,
            0xa1 => &mut r.excluidas,
            _ => return Err(torto("nameConstraints malformado")),
        };
        for sub in asn1::filhos(el.conteudo)? {
            // GeneralSubtree: base, e `minimum`/`maximum` que a §4.2.1.10
            // manda serem 0/ausente.
            let p = asn1::filhos(sub.conteudo)?;
            let Some(base) = p.first() else {
                return Err(torto("GeneralSubtree vazio"));
            };
            if p.len() > 1 {
                return Err(recusa(
                    "nameConstraints com minimum/maximum, que a RFC 5280 proibe".into(),
                ));
            }
            if !FORMAS.contains(&base.tag) {
                return Err(recusa(format!(
                    "nameConstraints de uma forma de nome que este validador nao confere (tag 0x{:02x})",
                    base.tag
                )));
            }
            alvo.push((base.tag, base.conteudo.to_vec()));
        }
    }
    Ok(r)
}

fn minusculo(b: &[u8]) -> String {
    String::from_utf8_lossy(b).to_ascii_lowercase()
}

/// DNS: `dominio` cabe na base quando e ela ou termina em `.base`. A base
/// com ponto a frente so aceita subdominio.
fn dns_cabe(nome: &str, base: &str) -> bool {
    let nome = nome.trim_end_matches('.');
    let base = base.trim_end_matches('.');
    if base.is_empty() {
        return true;
    }
    if let Some(sub) = base.strip_prefix('.') {
        return nome.len() > sub.len() + 1 && nome.ends_with(&format!(".{sub}"));
    }
    nome == base || nome.ends_with(&format!(".{base}"))
}

/// E-mail (§4.2.1.10): base com `@` e caixa postal inteira; base com ponto a
/// frente e qualquer host abaixo do dominio; base sem nenhum dos dois e o
/// host exato.
fn email_cabe(nome: &str, base: &str) -> bool {
    if base.contains('@') {
        return nome == base;
    }
    let Some((_, host)) = nome.rsplit_once('@') else {
        return false;
    };
    match base.strip_prefix('.') {
        Some(sub) => host.ends_with(&format!(".{sub}")),
        None => host == base,
    }
}

/// O host de um URI com autoridade (`esquema://[usuario@]host[:porta]...`).
fn host_do_uri(uri: &str) -> Option<String> {
    let (_, resto) = uri.split_once("://")?;
    let autoridade = resto.split(['/', '?', '#']).next()?;
    let host = autoridade.rsplit_once('@').map_or(autoridade, |(_, h)| h);
    let host = if host.starts_with('[') {
        host.split(']').next()?.trim_start_matches('[')
    } else {
        host.split(':').next()?
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

fn uri_cabe(nome: &str, base: &str) -> bool {
    let Some(host) = host_do_uri(nome) else {
        return false;
    };
    match base.strip_prefix('.') {
        Some(sub) => host.ends_with(&format!(".{sub}")),
        None => host == base,
    }
}

fn ip_cabe(ip: &[u8], base: &[u8]) -> bool {
    if base.len() != 2 * ip.len() {
        return false;
    }
    let (rede, mascara) = base.split_at(ip.len());
    ip.iter()
        .zip(rede)
        .zip(mascara)
        .all(|((a, r), m)| a & m == r & m)
}

/// O nome distinto `nome` comeca pelos RDNs da `base`?
fn dn_cabe(nome: &[Vec<Atributo>], base: &[Vec<Atributo>]) -> bool {
    nome.len() >= base.len() && nome[..base.len()] == *base
}

/// Um nome cabe numa base da mesma forma?
fn cabe(tag: u8, nome: &[u8], base: &[u8]) -> Result<bool> {
    Ok(match tag {
        0x81 => email_cabe(&minusculo(nome), &minusculo(base)),
        0x82 => dns_cabe(&minusculo(nome), &minusculo(base)),
        0x86 => uri_cabe(&String::from_utf8_lossy(nome), &minusculo(base)),
        0x87 => ip_cabe(nome, base),
        0xa4 => {
            // O directoryName vem com a etiqueta explicita: o conteudo e o
            // Name inteiro.
            dn_cabe(&rdns(nome)?, &rdns(base)?)
        }
        _ => false,
    })
}

impl Restricoes {
    /// Confere um nome de uma forma contra estas restricoes.
    fn aceita(&self, tag: u8, nome: &[u8]) -> Result<bool> {
        for (t, b) in &self.excluidas {
            if *t == tag && cabe(tag, nome, b)? {
                return Ok(false);
            }
        }
        let permitidas: Vec<_> = self.permitidas.iter().filter(|(t, _)| *t == tag).collect();
        if permitidas.is_empty() {
            return Ok(true);
        }
        for (_, b) in permitidas {
            if cabe(tag, nome, b)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Os nomes de um certificado que as restricoes alcancam: o sujeito (como
/// `directoryName`, se nao vazio), os e-mails do sujeito e o SAN.
fn nomes_restringiveis(c: &Certificado) -> Result<Vec<(u8, Vec<u8>)>> {
    let mut nomes = Vec::new();
    let sujeito = rdns(c.sujeito)?;
    if !sujeito.is_empty() {
        nomes.push((0xa4, c.sujeito.to_vec()));
    }
    for rdn in &sujeito {
        for a in rdn {
            if a.oid == OID_EMAIL {
                if let Ok(t) = &a.valor {
                    nomes.push((0x81, t.as_bytes().to_vec()));
                }
            }
        }
    }
    for (tag, conteudo) in c.nomes_alternativos()? {
        nomes.push((tag, conteudo.to_vec()));
    }
    Ok(nomes)
}

// ---------------------------------------------------- assinatura ----

/// Confere a assinatura de `c` com a chave publica `spki` do emissor.
fn conferir_assinatura(c: &Certificado, spki: &[u8]) -> Result<()> {
    use crate::rsa::Resumo;
    use crate::x509::ChavePublica;
    let chave = crate::x509::chave_do_spki(spki)?;
    let alg = c.alg.as_slice();
    let resumo = match alg {
        a if a == OID_RSA_SHA256 || a == OID_ECDSA_SHA256 => Some(Resumo::Sha256),
        a if a == OID_RSA_SHA384 || a == OID_ECDSA_SHA384 => Some(Resumo::Sha384),
        a if a == OID_RSA_SHA512 || a == OID_ECDSA_SHA512 => Some(Resumo::Sha512),
        a if a == OID_ED25519 => None,
        _ => {
            return Err(recusa(format!(
            "algoritmo de assinatura {} nao aceito (SHA-1, DSA e PSS em certificado nao entram)",
            c.alg
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(".")
        )))
        }
    };
    let rsa = [OID_RSA_SHA256, OID_RSA_SHA384, OID_RSA_SHA512].contains(&alg);
    let ok = match (&chave, resumo) {
        (ChavePublica::Rsa(k), Some(r)) if rsa => {
            crate::rsa::verificar_pkcs1v15(k, r, c.tbs, &c.assinatura)
        }
        (ChavePublica::P256(q), Some(r)) if !rsa => {
            let h = r.calcular(c.tbs);
            let mut h32 = [0u8; 32];
            // ECDSA corta o resumo no tamanho da ordem (SEC 1 §4.1.4).
            h32.copy_from_slice(&h[..32]);
            match crate::x509::ecdsa_de_der(&c.assinatura) {
                Some((r, s)) => crate::p256::verificar_resumo(q, &h32, &r, &s),
                None => false,
            }
        }
        (ChavePublica::P384(q), Some(r)) if !rsa => {
            let h = r.calcular(c.tbs);
            match crate::p384::assinatura_de_der(&c.assinatura) {
                Some((r, s)) => crate::p384::verificar_resumo(q, &h, &r, &s),
                None => false,
            }
        }
        (ChavePublica::Ed25519(q), None) => match <&[u8; 64]>::try_from(c.assinatura.as_slice()) {
            Ok(a) => crate::ed25519::conferir(q, c.tbs, a),
            Err(_) => false,
        },
        _ => {
            return Err(recusa(
                "o algoritmo da assinatura nao casa com a chave do emissor".into(),
            ))
        }
    };
    if ok {
        Ok(())
    } else {
        Err(recusa(
            "assinatura que nao confere com a chave do emissor".into(),
        ))
    }
}

// ------------------------------------------------------ o caminho ----

/// Para que a cadeia vai servir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uso {
    /// So a §6.1.
    Qualquer,
    /// Servidor TLS: o `extKeyUsage` de cada autoridade do caminho, quando
    /// existe, tem de incluir `serverAuth` (ou `anyExtendedKeyUsage`) --
    /// a restricao que o Go e o `webpki` aplicam ao caminho inteiro, e sem a
    /// qual uma autoridade limitada a e-mail emitiria servidor.
    ServidorTls,
}

const OID_QUALQUER_USO: &[u64] = &[2, 5, 29, 37, 0];

/// Valida um CAMINHO ja montado (§6.1): `ancora` (o certificado de
/// confianca, do qual so valem o nome e a chave) e `caminho` da primeira
/// emitida pela ancora ate a folha. `agora` em segundos.
pub fn validar_caminho(
    ancora: &Certificado,
    caminho: &[Certificado],
    agora: i64,
    uso: Uso,
) -> Result<()> {
    let n = caminho.len();
    if n == 0 {
        return Err(recusa("caminho vazio".into()));
    }
    let mut emissor = ancora.sujeito;
    let mut chave = ancora.spki;
    let mut comprimento_max = n;
    let mut restricoes: Vec<Restricoes> = Vec::new();
    for (i, c) in caminho.iter().enumerate() {
        let ultimo = i + 1 == n;
        let rotulo = if ultimo {
            "a folha".to_string()
        } else {
            format!("o elo {}", i + 1)
        };
        // (a) assinatura, validade, encadeamento do nome.
        conferir_assinatura(c, chave).map_err(|e| recusa(format!("{rotulo}: {e}")))?;
        if agora < c.nao_antes || agora > c.nao_depois {
            return Err(recusa(format!("{rotulo} esta fora da validade")));
        }
        if !nomes_iguais(c.emissor, emissor)? {
            return Err(recusa(format!(
                "{rotulo}: o emissor nao e o sujeito de quem o assinou"
            )));
        }
        let auto = c.auto_emitido()?;
        // (b, c) restricoes de nome -- o auto-emitido do meio fica de fora.
        if ultimo || !auto {
            for (tag, nome) in nomes_restringiveis(c)? {
                for r in &restricoes {
                    if !r.aceita(tag, &nome)? {
                        return Err(recusa(format!(
                            "{rotulo} tem um nome fora das restricoes de nome de uma autoridade"
                        )));
                    }
                }
            }
        }
        if !ultimo {
            // (k) tem de ser autoridade.
            if c.versao != 3 {
                return Err(recusa(format!(
                    "{rotulo} e v1/v2 e nao pode ser autoridade"
                )));
            }
            match c.restricoes_basicas()? {
                Some((true, len)) => {
                    // (l) e (m)
                    if !auto {
                        if comprimento_max == 0 {
                            return Err(recusa(format!(
                                "{rotulo} passa do pathLenConstraint de quem o emitiu"
                            )));
                        }
                        comprimento_max -= 1;
                    }
                    if let Some(l) = len {
                        comprimento_max = comprimento_max.min(l as usize);
                    }
                }
                _ => {
                    return Err(recusa(format!(
                        "{rotulo} nao e autoridade (basicConstraints sem cA)"
                    )))
                }
            }
            if uso == Uso::ServidorTls {
                if let Some(usos) = c.usos_estendidos()? {
                    if !usos
                        .iter()
                        .any(|o| o == OID_SERVER_AUTH || o == OID_QUALQUER_USO)
                    {
                        return Err(recusa(format!(
                            "{rotulo} restringe o uso e nao inclui serverAuth"
                        )));
                    }
                }
            }
            // (n) keyCertSign
            if let Some(u) = c.uso()? {
                if u & (1 << 5) == 0 {
                    return Err(recusa(format!(
                        "{rotulo} nao pode assinar certificado (keyUsage)"
                    )));
                }
            }
            // (g) restricoes de nome desta autoridade valem dali para baixo.
            if let Some(e) = c.extensao(OID_NAME_CONSTRAINTS) {
                restricoes.push(restricoes_de(e.valor)?);
            }
            emissor = c.sujeito;
            chave = c.spki;
        }
        // (o) critica desconhecida.
        if let Some(e) = c
            .extensoes
            .iter()
            .find(|e| e.critica && !ENTENDIDAS.contains(&e.oid.as_slice()))
        {
            return Err(recusa(format!(
                "{rotulo} tem a extensao critica {} que este validador nao entende",
                e.oid
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(".")
            )));
        }
    }
    Ok(())
}

/// Monta e valida o caminho de uma folha ate uma das `ancoras`, usando os
/// `intermediarios` que o servidor mandou (em qualquer ordem, com sobra).
/// Devolve o comprimento do caminho achado. A busca e em profundidade, com
/// teto ([`PROFUNDIDADE_MAXIMA`]) e sem repetir certificado.
pub fn validar(
    folha: &[u8],
    intermediarios: &[Vec<u8>],
    ancoras: &[Vec<u8>],
    agora: i64,
    uso: Uso,
) -> Result<usize> {
    let folha = Certificado::analisar(folha)?;
    let inter: Vec<Certificado> = intermediarios
        .iter()
        .filter_map(|d| Certificado::analisar(d).ok())
        .collect();
    let anc: Vec<Certificado> = ancoras
        .iter()
        .filter_map(|d| Certificado::analisar(d).ok())
        .collect();
    let mut caminho = vec![folha];
    let mut ultimo_erro = recusa("nenhuma ancora de confianca emitiu esta cadeia".into());
    if buscar(&mut caminho, &inter, &anc, agora, uso, &mut ultimo_erro) {
        return Ok(caminho.len());
    }
    Err(ultimo_erro)
}

fn buscar<'a>(
    caminho: &mut Vec<Certificado<'a>>,
    inter: &[Certificado<'a>],
    anc: &[Certificado<'a>],
    agora: i64,
    uso: Uso,
    erro: &mut PhxError,
) -> bool {
    let topo = caminho.last().expect("caminho nunca vazio").clone();
    for a in anc {
        if nomes_iguais(topo.emissor, a.sujeito).unwrap_or(false) {
            let ordem: Vec<Certificado> = caminho.iter().rev().cloned().collect();
            match validar_caminho(a, &ordem, agora, uso) {
                Ok(()) => return true,
                Err(e) => *erro = e,
            }
        }
    }
    if caminho.len() >= PROFUNDIDADE_MAXIMA {
        return false;
    }
    for c in inter {
        if caminho.iter().any(|x| x.der == c.der) {
            continue;
        }
        if nomes_iguais(topo.emissor, c.sujeito).unwrap_or(false) {
            caminho.push(c.clone());
            if buscar(caminho, inter, anc, agora, uso, erro) {
                return true;
            }
            caminho.pop();
        }
    }
    false
}

// ------------------------------------------- servidor TLS (RFC 9525) ----

/// O nome casa com um `dNSName` apresentado? RFC 9525 §6.3: caixa ignorada,
/// ponto final ignorado; o curinga so como o rotulo MAIS A ESQUERDA inteiro
/// (`*.exemplo.com`), casa um rotulo e so um, e nunca o dominio de cima.
pub fn dns_casa(referencia: &str, apresentado: &str) -> bool {
    let r = referencia.trim_end_matches('.').to_ascii_lowercase();
    let a = apresentado.trim_end_matches('.').to_ascii_lowercase();
    if r.is_empty() || a.is_empty() {
        return false;
    }
    match a.strip_prefix("*.") {
        Some(resto) => {
            // O curinga precisa de pelo menos dois rotulos abaixo dele, e o
            // resto nao pode ter outro curinga.
            if resto.contains('*') || !resto.contains('.') {
                return false;
            }
            match r.split_once('.') {
                Some((primeiro, depois)) => !primeiro.is_empty() && depois == resto,
                None => false,
            }
        }
        None => !a.contains('*') && r == a,
    }
}

/// Confere a folha para um servidor TLS chamado `nome` (DNS ou IP literal):
/// o nome no `subjectAltName` (RFC 9525 -- o CN NAO conta), o `extKeyUsage`
/// com `serverAuth` quando ha um, e `digitalSignature` quando ha `keyUsage`.
pub fn conferir_folha_tls(folha: &[u8], nome: &str) -> Result<()> {
    let c = Certificado::analisar(folha)?;
    if let Some(usos) = c.usos_estendidos()? {
        if !usos.iter().any(|o| o == OID_SERVER_AUTH) {
            return Err(recusa("a folha nao tem serverAuth no extKeyUsage".into()));
        }
    }
    if let Some(u) = c.uso()? {
        if u & 1 == 0 {
            return Err(recusa(
                "a folha nao tem digitalSignature no keyUsage".into(),
            ));
        }
    }
    let nomes = c.nomes_alternativos()?;
    let casou = match nome.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        // RFC 9525 §6.2: IP casa so com iPAddress, nunca com dNSName.
        Ok(ip) => {
            let bytes = match ip {
                std::net::IpAddr::V4(v) => v.octets().to_vec(),
                std::net::IpAddr::V6(v) => v.octets().to_vec(),
            };
            nomes
                .iter()
                .any(|(t, v)| *t == 0x87 && *v == bytes.as_slice())
        }
        Err(_) => nomes
            .iter()
            .any(|(t, v)| *t == 0x82 && dns_casa(nome, &String::from_utf8_lossy(v))),
    };
    if !casou {
        return Err(recusa(format!(
            "a folha nao vale para o nome {nome:?} (subjectAltName)"
        )));
    }
    Ok(())
}

/// A cadeia de um servidor TLS, como o `Certificate` a traz (a folha
/// primeiro), conferida contra as `ancoras` e para o `nome`: o caminho pela
/// §6.1 com o uso de servidor, e a folha pela RFC 9525.
pub fn validar_servidor_tls(
    cadeia: &[Vec<u8>],
    ancoras: &[Vec<u8>],
    nome: &str,
    agora: i64,
) -> Result<()> {
    let Some((folha, inter)) = cadeia.split_first() else {
        return Err(recusa("o servidor nao mandou certificado".into()));
    };
    conferir_folha_tls(folha, nome)?;
    validar(folha, inter, ancoras, agora, Uso::ServidorTls).map(|_| ())
}

// ------------------------------------------------------- ancoras ----

/// Onde as distribuicoes guardam o pacote de raizes em PEM, na ordem em que
/// se procura. E o mesmo inventario que o Go (`crypto/x509/root_linux.go`) e
/// o OpenSSL das distribuicoes usam; no Windows e no macOS o repositorio do
/// sistema nao e um arquivo, e ai `tls_ca` tem de nomear um.
pub const PACOTES_DO_SISTEMA: [&str; 5] = [
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/tls/certs/ca-bundle.crt",
    "/etc/ssl/ca-bundle.pem",
    "/etc/ssl/cert.pem",
    "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
];

/// As ancoras de um `tls_ca`: `"sistema"` le o pacote da distribuicao; o
/// resto e o caminho de um PEM com um ou mais `CERTIFICATE`. Certificado que
/// nao se le recusa o arquivo inteiro -- ancora pela metade e confianca que
/// ninguem escolheu --, menos no pacote do sistema, que se le como vem e diz
/// quantos ficaram de fora.
pub fn ancoras(tls_ca: &str) -> Result<(Vec<Vec<u8>>, usize)> {
    if tls_ca.trim() == "sistema" {
        for p in PACOTES_DO_SISTEMA {
            if let Ok(texto) = std::fs::read_to_string(p) {
                let todos = crate::x509::blocos_pem(&texto, "CERTIFICATE")?;
                let total = todos.len();
                let bons: Vec<Vec<u8>> = todos
                    .into_iter()
                    .filter(|d| Certificado::analisar(d).is_ok())
                    .collect();
                let fora = total - bons.len();
                if !bons.is_empty() {
                    return Ok((bons, fora));
                }
            }
        }
        return Err(PhxError::Esquema(format!(
            "tls_ca \"sistema\": nenhum pacote de raizes em {} -- escreva o caminho \
             de um PEM no tls_ca",
            PACOTES_DO_SISTEMA.join(", ")
        )));
    }
    let texto = std::fs::read_to_string(tls_ca.trim())
        .map_err(|e| PhxError::Esquema(format!("tls_ca: nao consegui ler {tls_ca:?}: {e}")))?;
    let todos = crate::x509::blocos_pem(&texto, "CERTIFICATE")?;
    if todos.is_empty() {
        return Err(PhxError::Esquema(format!(
            "tls_ca: {tls_ca:?} nao tem nenhum CERTIFICATE em PEM"
        )));
    }
    for (i, d) in todos.iter().enumerate() {
        Certificado::analisar(d).map_err(|e| {
            PhxError::Esquema(format!(
                "tls_ca: o certificado {} de {tls_ca:?}: {e}",
                i + 1
            ))
        })?;
    }
    Ok((todos, 0))
}

#[cfg(test)]
pub(crate) mod testes;
