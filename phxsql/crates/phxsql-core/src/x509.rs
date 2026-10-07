//! Certificado X.509 v3 autoassinado com Ed25519 (RFC 8410), sem dependencia
//! externa.
//!
//! # Para que serve aqui
//!
//! O correio nativo precisa PUBLICAR duas chaves de cada dono num formato que
//! qualquer ferramenta leia: a chave de ASSINATURA (Ed25519, RFC 8032) e a
//! chave de TROCA (X25519, RFC 7748). O recipiente padrao para uma chave
//! publica com um nome e uma validade e o certificado X.509 -- e e o que vai
//! dentro do `.p12` da fatia 2. Ver `docs/CORREIO-P12.md`.
//!
//! # O que a RFC 8410 muda em relacao ao X.509 "de RSA"
//!
//! Duas coisas, e as duas mordem quem copia um exemplo antigo:
//!
//! 1. O `AlgorithmIdentifier` do Ed25519 e do X25519 tem os PARAMETROS
//!    AUSENTES -- nao e `NULL`, e a ausencia. Poe um `NULL` ali e o OpenSSL
//!    recusa. E por isso que [`alg_id`] escreve so o OID.
//! 2. Os OIDs sao curtos e proprios: `1.3.101.112` para Ed25519 e
//!    `1.3.101.110` para X25519. A chave publica entra CRUA na `BIT STRING`
//!    da `SubjectPublicKeyInfo` -- 32 bytes, sem envelope.
//!
//! # As duas variantes de certificado, e por que o assinante e sempre Ed25519
//!
//! X25519 e uma chave de ACORDO (ECDH); ela nao assina nada. Entao quem assina
//! os DOIS certificados e a Ed25519 do dono:
//!
//! * o certificado de IDENTIDADE carrega a chave Ed25519 e e verdadeiramente
//!   autoassinado (a chave que assina e a chave que ele publica);
//! * o certificado de TROCA carrega a chave X25519 e e assinado pela MESMA
//!   Ed25519 do dono -- o emissor e o dono, o sujeito e o dono, mas a chave
//!   publicada e a de ECDH. E o par natural: a identidade avaliza a chave de
//!   troca.

use crate::asn1;
use crate::datahora::civil_de_dias;
use crate::ed25519;

/// OID `id-Ed25519` = 1.3.101.112 (RFC 8410).
const OID_ED25519: [u64; 4] = [1, 3, 101, 112];
/// OID `id-X25519` = 1.3.101.110 (RFC 8410).
const OID_X25519: [u64; 4] = [1, 3, 101, 110];
/// OID do atributo `commonName` (CN) = 2.5.4.3 (RFC 5280).
const OID_CN: [u64; 4] = [2, 5, 4, 3];

/// Qual chave o certificado PUBLICA no campo `subjectPublicKeyInfo`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AlgoChave {
    /// Chave de assinatura Ed25519.
    Ed25519,
    /// Chave de troca X25519 (ECDH).
    X25519,
}

impl AlgoChave {
    fn oid(self) -> &'static [u64] {
        match self {
            AlgoChave::Ed25519 => &OID_ED25519,
            AlgoChave::X25519 => &OID_X25519,
        }
    }
}

/// A validade do certificado, em milissegundos desde a epoca Unix, UTC.
#[derive(Clone, Copy, Debug)]
pub struct Validade {
    pub nao_antes_ms: i64,
    pub nao_depois_ms: i64,
}

impl Validade {
    /// De agora ate daqui a `anos` anos (365 dias por ano, o bastante para uma
    /// validade -- nao e calendario fino).
    pub fn de_agora_por_anos(anos: i64) -> Validade {
        let agora = agora_ms();
        Validade {
            nao_antes_ms: agora,
            nao_depois_ms: agora + anos * 365 * 86_400_000,
        }
    }
}

/// O sujeito do certificado: o nome (CN) e a chave publica que ele carrega.
pub struct Sujeito<'a> {
    pub cn: &'a str,
    pub algo: AlgoChave,
    pub chave_publica: &'a [u8; 32],
}

/// Um certificado emitido, com as pecas que a prova precisa ver de perto.
pub struct Certificado {
    /// A `TBSCertificate` codificada em DER -- e o que foi assinado.
    pub tbs: Vec<u8>,
    /// A assinatura Ed25519 sobre a `tbs` (64 bytes).
    pub assinatura: [u8; ed25519::ASSINATURA_LEN],
    /// O `Certificate` inteiro em DER.
    pub der: Vec<u8>,
}

/// Milissegundos desde a epoca, do relogio do sistema. So `std`.
fn agora_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64),
    }
}

/// Um `AlgorithmIdentifier` da RFC 8410: so o OID, SEM o campo de parametros.
///
/// A ausencia e deliberada e conferida contra ferramenta -- ver o modulo.
fn alg_id(oid_arcos: &[u64]) -> Vec<u8> {
    asn1::sequencia(&[asn1::oid(oid_arcos)])
}

/// Um `Name` (RFC 5280) com um unico RDN: o CN.
///
/// `Name` e `SEQUENCE OF SET OF SEQUENCE { tipo OID, valor }`. O valor do CN
/// vai como `UTF8String`, que e o que as ferramentas de hoje escrevem.
fn nome_com_cn(cn: &str) -> Vec<u8> {
    let atributo = asn1::sequencia(&[asn1::oid(&OID_CN), asn1::utf8_string(cn)]);
    let rdn = asn1::conjunto(&[atributo]);
    asn1::sequencia(&[rdn])
}

/// `subjectPublicKeyInfo`: o algoritmo da chave e a chave crua numa BIT STRING.
fn spki(algo: AlgoChave, chave_publica: &[u8; 32]) -> Vec<u8> {
    asn1::sequencia(&[alg_id(algo.oid()), asn1::bit_string(chave_publica, 0)])
}

/// Um `Time` do campo de validade. RFC 5280: ate 2049 e `UTCTime`
/// (`YYMMDDHHMMSSZ`); de 2050 em diante e `GeneralizedTime`
/// (`YYYYMMDDHHMMSSZ`).
fn tempo(ms: i64) -> Vec<u8> {
    let dias = ms.div_euclid(86_400_000) as i32;
    let (ano, mes, dia) = civil_de_dias(dias);
    let resto = ms.rem_euclid(86_400_000);
    let h = resto / 3_600_000;
    let mi = (resto / 60_000) % 60;
    let s = (resto / 1_000) % 60;
    if (1950..2050).contains(&ano) {
        let aa = (ano % 100) as u32;
        asn1::utc_time(&format!("{aa:02}{mes:02}{dia:02}{h:02}{mi:02}{s:02}Z"))
    } else {
        asn1::generalized_time(&format!("{ano:04}{mes:02}{dia:02}{h:02}{mi:02}{s:02}Z"))
    }
}

/// Monta a `TBSCertificate` em DER: versao v3, serie, algoritmo da assinatura,
/// emissor, validade, sujeito e a chave publica. Emissor e sujeito sao o mesmo
/// DN -- e o que "autoassinado/mesmo dono" quer dizer.
fn montar_tbs(sujeito: &Sujeito, validade: &Validade, serie: &[u8]) -> Vec<u8> {
    let dn = nome_com_cn(sujeito.cn);
    asn1::sequencia(&[
        // version [0] EXPLICIT INTEGER { v3(2) }
        asn1::contexto_explicito(0, &[asn1::inteiro_u64(2)]),
        // serialNumber
        asn1::inteiro(serie),
        // signature: quem assina o certificado e SEMPRE a Ed25519 do dono.
        alg_id(&OID_ED25519),
        // issuer (== subject, autoassinado)
        dn.clone(),
        // validity
        asn1::sequencia(&[tempo(validade.nao_antes_ms), tempo(validade.nao_depois_ms)]),
        // subject
        dn,
        // subjectPublicKeyInfo
        spki(sujeito.algo, sujeito.chave_publica),
    ])
}

/// Emite o certificado: monta a TBS, assina com a Ed25519 do dono e embrulha
/// em `Certificate { tbsCertificate, signatureAlgorithm, signatureValue }`.
///
/// `ed25519_privada` e a semente de 32 bytes da chave de assinatura do dono.
/// Para o certificado de IDENTIDADE, `sujeito.chave_publica` e a publica DESSA
/// chave (verdadeiramente autoassinado); para o de TROCA, e a publica X25519 do
/// mesmo dono.
pub fn emitir(
    sujeito: &Sujeito,
    validade: &Validade,
    serie: &[u8],
    ed25519_privada: &[u8; ed25519::CHAVE_LEN],
) -> Certificado {
    let tbs = montar_tbs(sujeito, validade, serie);
    let assinatura = ed25519::assinar(ed25519_privada, &tbs);
    let der = asn1::sequencia(&[
        tbs.clone(),
        alg_id(&OID_ED25519),
        asn1::bit_string(&assinatura, 0),
    ]);
    Certificado {
        tbs,
        assinatura,
        der,
    }
}

/// Atalho que devolve so a DER do certificado.
pub fn cert_autoassinado(
    sujeito: &Sujeito,
    validade: &Validade,
    serie: &[u8],
    ed25519_privada: &[u8; ed25519::CHAVE_LEN],
) -> Vec<u8> {
    emitir(sujeito, validade, serie, ed25519_privada).der
}

// ======================================================= TLS com P-256 ====
//
// Pedido 572: o certificado do SERVIDOR TLS. O navegador nao aceita Ed25519
// em certificado, entao a chave e ECDSA P-256 (`crate::p256`), e o nome que
// ele confere e o `subjectAltName` -- o CN nao conta mais para ninguem (RFC
// 6125 §6.4.4, e o Chrome desde a versao 58).

/// OID `id-ecPublicKey` = 1.2.840.10045.2.1 (RFC 5480).
const OID_EC_PUBLICA: [u64; 6] = [1, 2, 840, 10045, 2, 1];
/// OID `prime256v1` = 1.2.840.10045.3.1.7 (RFC 5480).
const OID_P256: [u64; 7] = [1, 2, 840, 10045, 3, 1, 7];
/// OID `ecdsa-with-SHA256` = 1.2.840.10045.4.3.2 (RFC 5758).
const OID_ECDSA_SHA256: [u64; 7] = [1, 2, 840, 10045, 4, 3, 2];
const OID_SAN: [u64; 4] = [2, 5, 29, 17];
const OID_BASIC_CONSTRAINTS: [u64; 4] = [2, 5, 29, 19];
const OID_KEY_USAGE: [u64; 4] = [2, 5, 29, 15];
const OID_EXT_KEY_USAGE: [u64; 4] = [2, 5, 29, 37];
/// `id-kp-serverAuth` = 1.3.6.1.5.5.7.3.1.
const OID_SERVER_AUTH: [u64; 9] = [1, 3, 6, 1, 5, 5, 7, 3, 1];

/// `subjectPublicKeyInfo` de uma chave P-256 `04 || X || Y`.
pub fn spki_p256(publica: &[u8; 65]) -> Vec<u8> {
    asn1::sequencia(&[
        asn1::sequencia(&[asn1::oid(&OID_EC_PUBLICA), asn1::oid(&OID_P256)]),
        asn1::bit_string(publica, 0),
    ])
}

/// A assinatura ECDSA em DER: `SEQUENCE { INTEGER r, INTEGER s }` -- e assim
/// que ela vai no certificado e no `CertificateVerify` do TLS 1.3.
pub fn der_ecdsa(r: &[u8; 32], s: &[u8; 32]) -> Vec<u8> {
    asn1::sequencia(&[asn1::inteiro(r), asn1::inteiro(s)])
}

/// O inverso de [`der_ecdsa`]: `(r, s)` de 32 bytes cada. `None` para DER
/// torto, sobra depois da sequencia ou inteiro maior que a ordem cabe -- a
/// entrada vem da rede (o `CertificateVerify` de um servidor alheio).
pub fn ecdsa_de_der(der: &[u8]) -> Option<([u8; 32], [u8; 32])> {
    let (seq, resto) = asn1::esperar(der, asn1::TAG_SEQUENCE).ok()?;
    if !resto.is_empty() {
        return None;
    }
    let partes = asn1::filhos(seq.conteudo).ok()?;
    let [r, s] = partes.as_slice() else {
        return None;
    };
    let mut saida = [[0u8; 32]; 2];
    for (el, dest) in [r, s].into_iter().zip(saida.iter_mut()) {
        if el.tag != asn1::TAG_INTEGER {
            return None;
        }
        // O `decodificar_inteiro` ja tira o 0x00 de sinal e recusa o DER nao
        // minimo; o que ainda passa de 32 bytes nao cabe na ordem da P-256.
        let m = asn1::decodificar_inteiro(el.conteudo).ok()?;
        if m.len() > 32 {
            return None;
        }
        dest[32 - m.len()..].copy_from_slice(&m);
    }
    Some((saida[0], saida[1]))
}

/// A chave publica que um certificado alheio carrega, ja reconhecida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChavePublica {
    /// ECDSA/ECDH P-256, ponto `04 || X || Y`.
    P256(Vec<u8>),
    /// Ed25519.
    Ed25519([u8; 32]),
    /// Qualquer outra: o OID do algoritmo, para a recusa NOMEAR o que veio
    /// (RSA, P-384...) em vez de dizer so «invalida».
    Outra(Vec<u64>),
}

/// O `subjectPublicKeyInfo` de um certificado, em DER cru (o TLV inteiro).
///
/// Cru porque e dele que sai o pino (`SHA-256` do SPKI, a forma do
/// `--pinnedpubkey` do curl e do HPKP): re-codificar o campo trocaria o
/// pino de quem o codifica de outro jeito.
pub fn spki_do_certificado(cert: &[u8]) -> crate::error::Result<&[u8]> {
    let torto = |m: &str| crate::error::PhxError::Corrompido(format!("X.509: {m}"));
    let (c, _) = asn1::esperar(cert, asn1::TAG_SEQUENCE)?;
    let (tbs, _) = asn1::esperar(c.conteudo, asn1::TAG_SEQUENCE)?;
    let mut resto = tbs.conteudo;
    // A versao [0] e opcional (v1 nao a traz); o SPKI e o setimo campo com
    // ela e o sexto sem ela.
    let (primeiro, _) = asn1::analisar(resto)?;
    let pular = if primeiro.tag == asn1::tag_contexto(0) {
        6
    } else {
        5
    };
    for _ in 0..pular {
        resto = asn1::analisar(resto)?.1;
    }
    let (spki, depois) = asn1::analisar(resto)?;
    if spki.tag != asn1::TAG_SEQUENCE {
        return Err(torto(
            "o subjectPublicKeyInfo nao esta onde a RFC 5280 o poe",
        ));
    }
    Ok(&resto[..resto.len() - depois.len()])
}

/// Le o SPKI e diz que chave e.
pub fn chave_do_spki(spki: &[u8]) -> crate::error::Result<ChavePublica> {
    let torto = |m: &str| crate::error::PhxError::Corrompido(format!("X.509: {m}"));
    let (s, sobra) = asn1::esperar(spki, asn1::TAG_SEQUENCE)?;
    if !sobra.is_empty() {
        return Err(torto("sobra depois do subjectPublicKeyInfo"));
    }
    let partes = asn1::filhos(s.conteudo)?;
    let [alg, bits] = partes.as_slice() else {
        return Err(torto("subjectPublicKeyInfo sem os dois campos"));
    };
    if alg.tag != asn1::TAG_SEQUENCE || bits.tag != asn1::TAG_BIT_STRING {
        return Err(torto("subjectPublicKeyInfo com tipo errado"));
    }
    let a = asn1::filhos(alg.conteudo)?;
    let Some(o) = a.first().filter(|o| o.tag == asn1::TAG_OID) else {
        return Err(torto("algoritmo da chave sem OID"));
    };
    let oid_alg = asn1::decodificar_oid(o.conteudo)?;
    let (nao_usados, chave) = asn1::decodificar_bit_string(bits.conteudo)?;
    if nao_usados != 0 {
        return Err(torto("chave publica com bits sobrando"));
    }
    if oid_alg == OID_EC_PUBLICA {
        let curva = match a.get(1) {
            Some(c) if c.tag == asn1::TAG_OID => asn1::decodificar_oid(c.conteudo)?,
            _ => return Err(torto("chave EC sem a curva")),
        };
        if curva != OID_P256 {
            return Ok(ChavePublica::Outra(curva));
        }
        return Ok(ChavePublica::P256(chave));
    }
    if oid_alg == OID_ED25519 {
        let k: [u8; 32] = chave
            .try_into()
            .map_err(|_| torto("chave Ed25519 sem 32 bytes"))?;
        return Ok(ChavePublica::Ed25519(k));
    }
    Ok(ChavePublica::Outra(oid_alg))
}

fn extensao(oid: &[u64], critica: bool, valor: Vec<u8>) -> Vec<u8> {
    let mut campos = vec![asn1::oid(oid)];
    if critica {
        campos.push(asn1::booleano(true));
    }
    campos.push(asn1::octet_string(&valor));
    asn1::sequencia(&campos)
}

/// Um nome alternativo: IP (v4 ou v6) quando o texto se le como IP, senao
/// DNS. O navegador confere IP contra `iPAddress`, nunca contra `dNSName`.
fn nome_alternativo(nome: &str) -> Vec<u8> {
    match nome.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => asn1::contexto_implicito(7, &ip.octets()),
        Ok(std::net::IpAddr::V6(ip)) => asn1::contexto_implicito(7, &ip.octets()),
        Err(_) => asn1::contexto_implicito(2, nome.as_bytes()),
    }
}

/// Certificado de servidor TLS autoassinado, ECDSA P-256 com SHA-256.
///
/// `nomes` vao no `subjectAltName` (o primeiro tambem vira o CN, so para quem
/// le). Leva `basicConstraints` CA:FALSE, `keyUsage` digitalSignature e
/// `extKeyUsage` serverAuth -- o que o navegador confere num certificado de
/// folha. `None` se a chave privada nao e valida.
pub fn cert_tls_p256(
    nomes: &[&str],
    validade: &Validade,
    serie: &[u8],
    privada: &[u8; 32],
) -> Option<Vec<u8>> {
    let publica = crate::p256::chave_publica(privada)?;
    let dn = nome_com_cn(nomes.first().copied().unwrap_or("phxsql"));
    let alg = asn1::sequencia(&[asn1::oid(&OID_ECDSA_SHA256)]);
    let san = asn1::sequencia(
        &nomes
            .iter()
            .map(|n| nome_alternativo(n))
            .collect::<Vec<_>>(),
    );
    let extensoes = asn1::sequencia(&[
        extensao(&OID_BASIC_CONSTRAINTS, true, asn1::sequencia(&[])),
        // digitalSignature: bit 0 -> 0x80, um bit usado de 8 (7 nao usados).
        extensao(&OID_KEY_USAGE, true, asn1::bit_string(&[0x80], 7)),
        extensao(
            &OID_EXT_KEY_USAGE,
            false,
            asn1::sequencia(&[asn1::oid(&OID_SERVER_AUTH)]),
        ),
        extensao(&OID_SAN, false, san),
    ]);
    let tbs = asn1::sequencia(&[
        asn1::contexto_explicito(0, &[asn1::inteiro_u64(2)]),
        asn1::inteiro(serie),
        alg.clone(),
        dn.clone(),
        asn1::sequencia(&[tempo(validade.nao_antes_ms), tempo(validade.nao_depois_ms)]),
        dn,
        spki_p256(&publica),
        asn1::contexto_explicito(3, &[extensoes]),
    ]);
    let (r, s) = crate::p256::assinar(privada, &tbs)?;
    Some(asn1::sequencia(&[
        tbs,
        alg,
        asn1::bit_string(&der_ecdsa(&r, &s), 0),
    ]))
}

/// Os blocos `-----BEGIN <rotulo>-----` de um texto PEM, em DER, na ordem.
pub fn blocos_pem(texto: &str, rotulo: &str) -> crate::error::Result<Vec<Vec<u8>>> {
    let inicio = format!("-----BEGIN {rotulo}-----");
    let fim = format!("-----END {rotulo}-----");
    let mut saida = Vec::new();
    let mut resto = texto;
    while let Some(i) = resto.find(&inicio) {
        let depois = &resto[i + inicio.len()..];
        let j = depois.find(&fim).ok_or_else(|| {
            crate::error::PhxError::Corrompido(format!("PEM: {inicio} sem o {fim}"))
        })?;
        let corpo: String = depois[..j].split_whitespace().collect();
        saida.push(crate::base64::decodificar(&corpo)?);
        resto = &depois[j + fim.len()..];
    }
    Ok(saida)
}

/// PEM em texto: o DER em Base64 com linhas de 64, entre as marcas.
pub fn para_pem(der: &[u8], rotulo: &str) -> String {
    let b64 = crate::base64::codificar(der);
    let mut s = format!("-----BEGIN {rotulo}-----\n");
    for pedaco in b64.as_bytes().chunks(64) {
        s.push_str(std::str::from_utf8(pedaco).unwrap_or_default());
        s.push('\n');
    }
    s.push_str(&format!("-----END {rotulo}-----\n"));
    s
}

/// A chave privada P-256 de um PEM: `EC PRIVATE KEY` (SEC 1, RFC 5915) ou
/// `PRIVATE KEY` (PKCS#8, RFC 5208) com curva P-256. Outra curva ou outro
/// algoritmo e recusa com o nome, nunca um palpite.
pub fn chave_p256_de_pem(texto: &str) -> crate::error::Result<[u8; 32]> {
    use crate::error::PhxError;
    let recusa = |m: &str| PhxError::Corrompido(format!("chave TLS: {m}"));
    if let Some(der) = blocos_pem(texto, "EC PRIVATE KEY")?.into_iter().next() {
        return chave_sec1(&der);
    }
    let der = blocos_pem(texto, "PRIVATE KEY")?
        .into_iter()
        .next()
        .ok_or_else(|| recusa("nem EC PRIVATE KEY nem PRIVATE KEY no PEM"))?;
    let (seq, _) = asn1::esperar(&der, asn1::TAG_SEQUENCE)?;
    let itens = asn1::filhos(seq.conteudo)?;
    if itens.len() < 3 {
        return Err(recusa("PKCS#8 com campos faltando"));
    }
    let alg = asn1::filhos(itens[1].conteudo)?;
    let oid = |e: &asn1::Elemento<'_>| asn1::decodificar_oid(e.conteudo);
    if alg.len() != 2 || oid(&alg[0])? != OID_EC_PUBLICA || oid(&alg[1])? != OID_P256 {
        return Err(recusa("PKCS#8 que nao e ECDSA P-256 -- so P-256 e aceita"));
    }
    chave_sec1(itens[2].conteudo)
}

/// `ECPrivateKey ::= SEQUENCE { version 1, privateKey OCTET STRING, ... }`.
fn chave_sec1(der: &[u8]) -> crate::error::Result<[u8; 32]> {
    use crate::error::PhxError;
    let (seq, _) = asn1::esperar(der, asn1::TAG_SEQUENCE)?;
    let itens = asn1::filhos(seq.conteudo)?;
    let bruta = itens
        .get(1)
        .filter(|e| e.tag == asn1::TAG_OCTET_STRING)
        .ok_or_else(|| PhxError::Corrompido("chave TLS: ECPrivateKey sem a chave".into()))?;
    // Se a curva vem declarada ([0] parameters), tem de ser a P-256.
    if let Some(p) = itens.iter().find(|e| e.tag == asn1::tag_contexto(0)) {
        let (o, _) = asn1::esperar(p.conteudo, asn1::TAG_OID)?;
        if asn1::decodificar_oid(o.conteudo)? != OID_P256 {
            return Err(PhxError::Corrompido(
                "chave TLS: curva que nao e a P-256 -- so P-256 e aceita".into(),
            ));
        }
    }
    let mut d = [0u8; 32];
    if bruta.conteudo.len() > 32 {
        return Err(PhxError::Corrompido(
            "chave TLS: escalar maior que 32 bytes".into(),
        ));
    }
    d[32 - bruta.conteudo.len()..].copy_from_slice(bruta.conteudo);
    if crate::p256::chave_publica(&d).is_none() {
        return Err(PhxError::Corrompido(
            "chave TLS: escalar fora de [1, n-1]".into(),
        ));
    }
    Ok(d)
}

/// A chave privada P-256 em PEM PKCS#8 -- para gravar a que o servidor gerou.
pub fn pem_da_chave_p256(privada: &[u8; 32]) -> Option<String> {
    let publica = crate::p256::chave_publica(privada)?;
    let sec1 = asn1::sequencia(&[
        asn1::inteiro_u64(1),
        asn1::octet_string(privada),
        asn1::contexto_explicito(1, &[asn1::bit_string(&publica, 0)]),
    ]);
    let pkcs8 = asn1::sequencia(&[
        asn1::inteiro_u64(0),
        asn1::sequencia(&[asn1::oid(&OID_EC_PUBLICA), asn1::oid(&OID_P256)]),
        asn1::octet_string(&sec1),
    ]);
    Some(para_pem(&pkcs8, "PRIVATE KEY"))
}

#[cfg(test)]
mod testes_tls {
    use super::*;
    use std::process::Command;

    fn tem_openssl() -> bool {
        Command::new("openssl").arg("version").output().is_ok()
    }

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn dir(nome: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("phx-x509tls-{nome}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    /// O OpenSSL le o certificado, acha o SAN, e a assinatura dele confere
    /// com a chave que ele publica.
    #[test]
    fn o_openssl_le_o_certificado_e_confere_a_assinatura() {
        if !tem_openssl() {
            eprintln!("sem openssl: NAO conferido");
            return;
        }
        let d = dir("cert");
        let privada = crate::p256::gerar_privada();
        let der = cert_tls_p256(
            &["phxsql.local", "127.0.0.1"],
            &Validade::de_agora_por_anos(1),
            &[1, 2, 3],
            &privada,
        )
        .unwrap();
        let pem = d.0.join("cert.pem");
        std::fs::write(&pem, para_pem(&der, "CERTIFICATE")).unwrap();
        let texto = Command::new("openssl")
            .args(["x509", "-noout", "-text", "-in"])
            .arg(&pem)
            .output()
            .unwrap();
        assert!(texto.status.success(), "o OpenSSL nao leu o certificado");
        let t = String::from_utf8_lossy(&texto.stdout);
        assert!(t.contains("DNS:phxsql.local"), "SAN sem o DNS: {t}");
        assert!(t.contains("IP Address:127.0.0.1"), "SAN sem o IP: {t}");
        assert!(t.contains("TLS Web Server Authentication"), "{t}");
        // Autoassinado: o proprio certificado confere a si mesmo.
        let v = Command::new("openssl")
            .args(["verify", "-check_ss_sig", "-partial_chain", "-trusted"])
            .arg(&pem)
            .arg(&pem)
            .output()
            .unwrap();
        assert!(
            v.status.success(),
            "o OpenSSL nao conferiu a assinatura: {}{}",
            String::from_utf8_lossy(&v.stdout),
            String::from_utf8_lossy(&v.stderr)
        );
    }

    /// A chave que o OpenSSL gera, nos dois formatos, e lida aqui e da a
    /// MESMA publica que ele diz.
    #[test]
    fn le_a_chave_do_openssl_em_sec1_e_pkcs8() {
        if !tem_openssl() {
            eprintln!("sem openssl: NAO conferido");
            return;
        }
        let d = dir("chave");
        let sec1 = d.0.join("sec1.pem");
        let pk8 = d.0.join("pk8.pem");
        let ok = |c: &mut Command| assert!(c.output().unwrap().status.success(), "{c:?}");
        ok(Command::new("openssl")
            .args([
                "ecparam",
                "-name",
                "prime256v1",
                "-genkey",
                "-noout",
                "-out",
            ])
            .arg(&sec1));
        ok(Command::new("openssl")
            .args(["pkcs8", "-topk8", "-nocrypt", "-in"])
            .arg(&sec1)
            .arg("-out")
            .arg(&pk8));
        let publica = Command::new("openssl")
            .args(["ec", "-pubout", "-outform", "DER", "-in"])
            .arg(&sec1)
            .output()
            .unwrap()
            .stdout;
        for f in [&sec1, &pk8] {
            let d = chave_p256_de_pem(&std::fs::read_to_string(f).unwrap()).unwrap();
            let q = crate::p256::chave_publica(&d).unwrap();
            assert_eq!(&publica[publica.len() - 65..], &q[..], "{}", f.display());
        }
        // E a que esta casa grava, o OpenSSL le.
        let nossa = d.0.join("nossa.pem");
        let privada = crate::p256::gerar_privada();
        std::fs::write(&nossa, pem_da_chave_p256(&privada).unwrap()).unwrap();
        ok(Command::new("openssl")
            .args(["pkey", "-noout", "-in"])
            .arg(&nossa));
    }

    #[test]
    fn chave_de_outra_curva_e_recusada_com_o_nome() {
        // PKCS#8 com o OID da secp384r1 no lugar da P-256.
        let pk8 = asn1::sequencia(&[
            asn1::inteiro_u64(0),
            asn1::sequencia(&[asn1::oid(&OID_EC_PUBLICA), asn1::oid(&[1, 3, 132, 0, 34])]),
            asn1::octet_string(&asn1::sequencia(&[
                asn1::inteiro_u64(1),
                asn1::octet_string(&[1u8; 32]),
            ])),
        ]);
        let e = chave_p256_de_pem(&para_pem(&pk8, "PRIVATE KEY")).unwrap_err();
        assert!(e.to_string().contains("P-256"), "{e}");

        // SEC 1 da secp256k1: escalar de 32 bytes, que a P-256 aceitaria como
        // numero -- so a curva declarada denuncia que a chave e de outra.
        let sec1 = asn1::sequencia(&[
            asn1::inteiro_u64(1),
            asn1::octet_string(&[1u8; 32]),
            asn1::contexto_explicito(0, &[asn1::oid(&[1, 3, 132, 0, 10])]),
        ]);
        let e = chave_p256_de_pem(&para_pem(&sec1, "EC PRIVATE KEY")).unwrap_err();
        assert!(e.to_string().contains("P-256"), "{e}");
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::asn1::{self, TAG_BIT_STRING, TAG_SEQUENCE};

    fn validade_fixa() -> Validade {
        // 2026-09-11 12:00:00Z ate 2036-09-11 12:00:00Z, deterministico e sem
        // numero magico: a data sai do proprio calendario da casa.
        use crate::datahora::dias_de_civil;
        let meio_dia = 12 * 3_600_000i64;
        Validade {
            nao_antes_ms: dias_de_civil(2026, 9, 11) as i64 * 86_400_000 + meio_dia,
            nao_depois_ms: dias_de_civil(2036, 9, 11) as i64 * 86_400_000 + meio_dia,
        }
    }

    fn priv_ed() -> [u8; 32] {
        // Chave privada 1 dos vetores da RFC 8032 (ja conferida no ed25519.rs).
        crate::ed25519::chave_de_hex(
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        )
        .unwrap()
    }

    /// O certificado de identidade Ed25519 e VERDADEIRAMENTE autoassinado: a
    /// chave que ele publica confere a assinatura dele proprio. E a prova real
    /// da fatia 1 sem sair de casa -- a interop com o OpenSSL esta no exemplo
    /// `cert-openssl`.
    #[test]
    fn identidade_ed25519_confere_a_propria_assinatura() {
        let sk = priv_ed();
        let pk = ed25519::chave_publica(&sk);
        let cert = emitir(
            &Sujeito {
                cn: "adrianoboller@empresa.phxsql.com.br",
                algo: AlgoChave::Ed25519,
                chave_publica: &pk,
            },
            &validade_fixa(),
            &[0x01, 0x02, 0x03, 0x04],
            &sk,
        );
        // A chave publicada confere a assinatura sobre a TBS.
        assert!(
            ed25519::conferir(&pk, &cert.tbs, &cert.assinatura),
            "o certificado de identidade nao confere a propria assinatura"
        );
        // Um bit virado na assinatura derruba a conferencia (falha-com-defeito).
        let mut torta = cert.assinatura;
        torta[10] ^= 1;
        assert!(!ed25519::conferir(&pk, &cert.tbs, &torta));
        // Um bit virado na TBS tambem: o que se assinou nao e mais o que se le.
        let mut tbs_torta = cert.tbs.clone();
        tbs_torta[20] ^= 1;
        assert!(!ed25519::conferir(&pk, &tbs_torta, &cert.assinatura));
    }

    /// O certificado de troca carrega X25519, mas quem o assina e a Ed25519 do
    /// dono. Entao a conferencia usa a Ed25519 -- a X25519 nem sabe assinar.
    #[test]
    fn troca_x25519_e_assinada_pela_ed25519_do_dono() {
        let sk = priv_ed();
        let pk_ed = ed25519::chave_publica(&sk);
        let pk_x = crate::x25519::chave_publica(&crate::x25519::gerar_privada());
        let cert = emitir(
            &Sujeito {
                cn: "adrianoboller@empresa.phxsql.com.br",
                algo: AlgoChave::X25519,
                chave_publica: &pk_x,
            },
            &validade_fixa(),
            &[0x05],
            &sk,
        );
        // Confere com a Ed25519 do dono, nao com a X25519 publicada.
        assert!(ed25519::conferir(&pk_ed, &cert.tbs, &cert.assinatura));
    }

    /// A DER que sai e parseavel pelo nosso proprio leitor, e a espinha bate:
    /// Certificate = SEQUENCE { tbs, algid, BIT STRING }.
    #[test]
    fn a_der_tem_a_espinha_de_um_certificate() {
        let sk = priv_ed();
        let pk = ed25519::chave_publica(&sk);
        let der = cert_autoassinado(
            &Sujeito {
                cn: "x",
                algo: AlgoChave::Ed25519,
                chave_publica: &pk,
            },
            &validade_fixa(),
            &[0x2a],
            &sk,
        );
        let (cert_el, resto) = asn1::esperar(&der, TAG_SEQUENCE).unwrap();
        assert!(resto.is_empty(), "sobrou byte depois do Certificate");
        let campos = asn1::filhos(cert_el.conteudo).unwrap();
        assert_eq!(campos.len(), 3, "Certificate tem tres campos");
        assert_eq!(campos[0].tag, TAG_SEQUENCE, "tbsCertificate");
        assert_eq!(campos[1].tag, TAG_SEQUENCE, "signatureAlgorithm");
        assert_eq!(campos[2].tag, TAG_BIT_STRING, "signatureValue");

        // O signatureAlgorithm de fora e Ed25519, sem parametros (RFC 8410).
        let alg = asn1::filhos(campos[1].conteudo).unwrap();
        assert_eq!(
            alg.len(),
            1,
            "AlgorithmIdentifier da RFC 8410 nao tem parametros"
        );
        assert_eq!(
            asn1::decodificar_oid(alg[0].conteudo).unwrap(),
            vec![1, 3, 101, 112]
        );

        // A assinatura na BIT STRING tem 0 bits sobrando e 64 bytes.
        let (nao_usados, bytes) = asn1::decodificar_bit_string(campos[2].conteudo).unwrap();
        assert_eq!(nao_usados, 0);
        assert_eq!(bytes.len(), ed25519::ASSINATURA_LEN);
    }

    /// A versao vem como `[0] EXPLICIT INTEGER 2` (v3), e o SPKI publica o OID
    /// certo para cada variante.
    #[test]
    fn versao_v3_e_oid_do_sujeito() {
        let sk = priv_ed();
        let pk = ed25519::chave_publica(&sk);
        for (algo, oid_esperado) in [
            (AlgoChave::Ed25519, vec![1u64, 3, 101, 112]),
            (AlgoChave::X25519, vec![1u64, 3, 101, 110]),
        ] {
            let cert = emitir(
                &Sujeito {
                    cn: "dono",
                    algo,
                    chave_publica: &pk,
                },
                &validade_fixa(),
                &[0x01],
                &sk,
            );
            let (cert_el, _) = asn1::esperar(&cert.der, TAG_SEQUENCE).unwrap();
            let campos = asn1::filhos(cert_el.conteudo).unwrap();
            let tbs = asn1::filhos(campos[0].conteudo).unwrap();
            // tbs[0] = version [0] EXPLICIT
            assert_eq!(tbs[0].tag, asn1::tag_contexto(0));
            let v = asn1::filhos(tbs[0].conteudo).unwrap();
            assert_eq!(
                asn1::decodificar_inteiro(v[0].conteudo).unwrap(),
                vec![0x02]
            );
            // tbs[6] = subjectPublicKeyInfo -> [algid, bitstring]; algid[0] = OID
            let spki = asn1::filhos(tbs[6].conteudo).unwrap();
            let algid = asn1::filhos(spki[0].conteudo).unwrap();
            assert_eq!(
                asn1::decodificar_oid(algid[0].conteudo).unwrap(),
                oid_esperado
            );
        }
    }
}
