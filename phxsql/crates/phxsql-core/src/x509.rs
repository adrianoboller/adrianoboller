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
