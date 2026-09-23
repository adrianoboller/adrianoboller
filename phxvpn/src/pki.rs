//! A infraestrutura de chaves da VPN: uma AC Ed25519 propria, o certificado de
//! cada servidor OpenVPN e o de cada membro de rede.
//!
//! # Por que Ed25519, e nao RSA
//!
//! O OpenVPN 2.6 (OpenSSL 1.1.1 ou 3) aceita certificado Ed25519 no TLS, e o
//! Ed25519 ja esta escrito e conferido contra a RFC 8032 no `phxsql-core`.
//! RSA exigiria escrever aritmetica de 2048 bits e geracao de primos -- mais
//! codigo de risco para o mesmo servico. O `x509.rs` do nucleo so emite
//! autoassinado de um CN, sem extensao; o OpenVPN precisa de emissor diferente
//! do sujeito e das extensoes que o `remote-cert-tls` confere. Por isso a
//! emissao mora aqui, montada com o ASN.1 do nucleo.
//!
//! # As extensoes que importam para o OpenVPN
//!
//! * `basicConstraints` -- a AC diz `CA:TRUE`; folha diz que nao e AC, senao um
//!   membro poderia assinar certificado de outro.
//! * `keyUsage` -- AC: `keyCertSign, cRLSign`; folha: `digitalSignature`.
//! * `extendedKeyUsage` -- `serverAuth` no servidor, `clientAuth` no membro. E
//!   o que o `remote-cert-tls server` do cliente confere: sem ele, um membro
//!   com certificado valido poderia se passar pelo servidor.
//! * `subjectKeyIdentifier` / `authorityKeyIdentifier` -- ligam folha a AC.

use phxsql_core::asn1;
use phxsql_core::base64;
use phxsql_core::ed25519;
use phxsql_core::senha::bytes_aleatorios;
use phxsql_core::sha1::sha1;
use phxsql_core::x509::Validade;

const OID_ED25519: [u64; 4] = [1, 3, 101, 112];
const OID_CN: [u64; 4] = [2, 5, 4, 3];
const OID_O: [u64; 4] = [2, 5, 4, 10];
const OID_BASIC: [u64; 4] = [2, 5, 29, 19];
const OID_KEY_USAGE: [u64; 4] = [2, 5, 29, 15];
const OID_EXT_KEY_USAGE: [u64; 4] = [2, 5, 29, 37];
const OID_SKI: [u64; 4] = [2, 5, 29, 14];
const OID_AKI: [u64; 4] = [2, 5, 29, 35];
const OID_CRL_NUMBER: [u64; 4] = [2, 5, 29, 20];
const OID_SERVER_AUTH: [u64; 9] = [1, 3, 6, 1, 5, 5, 7, 3, 1];
const OID_CLIENT_AUTH: [u64; 9] = [1, 3, 6, 1, 5, 5, 7, 3, 2];

/// Que papel o certificado cumpre.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Papel {
    Ac,
    Servidor,
    Membro,
}

/// Um par emitido: o certificado (DER) e a semente Ed25519 da chave privada.
pub struct Emitido {
    pub der: Vec<u8>,
    pub privada: [u8; 32],
    pub serie_hex: String,
    pub cn: String,
}

/// Quem assina: o nome (O + CN) e a chave da AC.
pub struct Ac<'a> {
    pub organizacao: &'a str,
    pub cn: &'a str,
    pub privada: &'a [u8; 32],
}

fn nome(organizacao: &str, cn: &str) -> Vec<u8> {
    let rdn = |oid: &[u64], v: &str| {
        asn1::conjunto(&[asn1::sequencia(&[asn1::oid(oid), asn1::utf8_string(v)])])
    };
    asn1::sequencia(&[rdn(&OID_O, organizacao), rdn(&OID_CN, cn)])
}

fn extensao(oid: &[u64], critica: bool, valor: Vec<u8>) -> Vec<u8> {
    let mut partes = vec![asn1::oid(oid)];
    if critica {
        partes.push(asn1::booleano(true));
    }
    partes.push(asn1::octet_string(&valor));
    asn1::sequencia(&partes)
}

/// `[0] IMPLICIT OCTET STRING` do `keyIdentifier` da AKI: a mesma codificacao
/// da `OCTET STRING`, com a tag trocada para contexto 0 primitiva.
fn id_chave_implicito(id: &[u8]) -> Vec<u8> {
    let mut v = asn1::octet_string(id);
    v[0] = 0x80;
    v
}

fn tempo(ms: i64) -> Vec<u8> {
    use phxsql_core::datahora::civil_de_dias;
    let dias = ms.div_euclid(86_400_000) as i32;
    let (ano, mes, dia) = civil_de_dias(dias);
    let resto = ms.rem_euclid(86_400_000);
    let (h, mi, s) = (
        resto / 3_600_000,
        (resto / 60_000) % 60,
        (resto / 1_000) % 60,
    );
    if (1950..2050).contains(&ano) {
        let aa = (ano % 100) as u32;
        asn1::utc_time(&format!("{aa:02}{mes:02}{dia:02}{h:02}{mi:02}{s:02}Z"))
    } else {
        asn1::generalized_time(&format!("{ano:04}{mes:02}{dia:02}{h:02}{mi:02}{s:02}Z"))
    }
}

/// Serie aleatoria de 16 bytes com o bit alto limpo: a RFC 5280 pede inteiro
/// positivo de ate 20 bytes, e aleatoria impede adivinhar a proxima.
fn serie_nova() -> Vec<u8> {
    let mut s = bytes_aleatorios(16);
    s[0] &= 0x7f;
    s[0] |= 0x01;
    s
}

/// Emite um certificado. Para `Papel::Ac`, `ac` e ignorada e ele sai
/// autoassinado com a chave nova.
pub fn emitir(
    papel: Papel,
    organizacao: &str,
    cn: &str,
    anos: i64,
    ac: Option<&Ac>,
) -> Result<Emitido, String> {
    emitir_com_cn(papel, organizacao, anos, ac, |_| cn.to_string())
}

/// Emite com o CN montado a partir da serie (em hex) -- para o CN ser unico
/// por emissao, como o do membro (`login.rede.serie`).
pub fn emitir_com_cn(
    papel: Papel,
    organizacao: &str,
    anos: i64,
    ac: Option<&Ac>,
    cn: impl FnOnce(&str) -> String,
) -> Result<Emitido, String> {
    let privada: [u8; 32] = bytes_aleatorios(32).try_into().expect("32 bytes");
    let serie = serie_nova();
    let cn = cn(&phxsql_core::hash::para_hex(&serie));
    emitir_com(
        papel,
        organizacao,
        &cn,
        &Validade::de_agora_por_anos(anos),
        ac,
        privada,
        serie,
    )
}

fn emitir_com(
    papel: Papel,
    organizacao: &str,
    cn: &str,
    validade: &Validade,
    ac: Option<&Ac>,
    privada: [u8; 32],
    serie: Vec<u8>,
) -> Result<Emitido, String> {
    let publica = ed25519::chave_publica(&privada);
    let ski = sha1(&publica);
    let sujeito = nome(organizacao, cn);
    let (emissor, chave_emissora, aki) = match (papel, ac) {
        (Papel::Ac, _) => (sujeito.clone(), privada, ski),
        (_, Some(a)) => (
            nome(a.organizacao, a.cn),
            *a.privada,
            sha1(&ed25519::chave_publica(a.privada)),
        ),
        (_, None) => return Err("certificado de folha precisa de AC emissora".into()),
    };

    let mut exts = Vec::new();
    match papel {
        Papel::Ac => {
            exts.push(extensao(
                &OID_BASIC,
                true,
                asn1::sequencia(&[asn1::booleano(true)]),
            ));
            // keyCertSign (bit 5) e cRLSign (bit 6): 0b0000_0110, 1 bit sobrando.
            exts.push(extensao(&OID_KEY_USAGE, true, asn1::bit_string(&[0x06], 1)));
        }
        Papel::Servidor | Papel::Membro => {
            exts.push(extensao(&OID_BASIC, true, asn1::sequencia(&[])));
            // digitalSignature (bit 0): 0b1000_0000, 7 bits sobrando.
            exts.push(extensao(&OID_KEY_USAGE, true, asn1::bit_string(&[0x80], 7)));
            let uso: &[u64] = if papel == Papel::Servidor {
                &OID_SERVER_AUTH
            } else {
                &OID_CLIENT_AUTH
            };
            exts.push(extensao(
                &OID_EXT_KEY_USAGE,
                false,
                asn1::sequencia(&[asn1::oid(uso)]),
            ));
        }
    }
    exts.push(extensao(&OID_SKI, false, asn1::octet_string(&ski)));
    exts.push(extensao(
        &OID_AKI,
        false,
        asn1::sequencia(&[id_chave_implicito(&aki)]),
    ));

    let alg = asn1::sequencia(&[asn1::oid(&OID_ED25519)]);
    let tbs = asn1::sequencia(&[
        asn1::contexto_explicito(0, &[asn1::inteiro_u64(2)]),
        asn1::inteiro(&serie),
        alg.clone(),
        emissor,
        asn1::sequencia(&[tempo(validade.nao_antes_ms), tempo(validade.nao_depois_ms)]),
        sujeito,
        asn1::sequencia(&[alg.clone(), asn1::bit_string(&publica, 0)]),
        asn1::contexto_explicito(3, &[asn1::sequencia(&exts)]),
    ]);
    let assinatura = ed25519::assinar(&chave_emissora, &tbs);
    let der = asn1::sequencia(&[tbs, alg, asn1::bit_string(&assinatura, 0)]);
    Ok(Emitido {
        der,
        privada,
        serie_hex: phxsql_core::hash::para_hex(&serie),
        cn: cn.to_string(),
    })
}

/// Lista de revogacao (CRL v2, RFC 5280 secao 5) assinada pela AC.
///
/// E o que o `crl-verify` do OpenVPN le a cada conexao nova: certificado com
/// serie na lista nao passa do aperto TLS, mesmo valido e dentro do prazo.
/// Sem ela, sair da rede so apagava o `ccd/` -- e reentrar recriava o mesmo
/// CN, reativando um perfil roubado (achado A1 da revisao de seguranca).
///
/// `numero` e o `cRLNumber`, que so cresce. `series` em bytes big-endian.
pub fn crl(ac: &Ac, series: &[Vec<u8>], numero: u64, validade: &Validade) -> Vec<u8> {
    let alg = asn1::sequencia(&[asn1::oid(&OID_ED25519)]);
    let aki = sha1(&ed25519::chave_publica(ac.privada));
    let mut partes = vec![
        asn1::inteiro_u64(1), // v2
        alg.clone(),
        nome(ac.organizacao, ac.cn),
        tempo(validade.nao_antes_ms),
        tempo(validade.nao_depois_ms),
    ];
    // Lista vazia: o campo SOME (e OPCIONAL, e `SEQUENCE {}` vazio o OpenSSL
    // recusa).
    if !series.is_empty() {
        partes.push(asn1::sequencia(
            &series
                .iter()
                .map(|s| asn1::sequencia(&[asn1::inteiro(s), tempo(validade.nao_antes_ms)]))
                .collect::<Vec<_>>(),
        ));
    }
    partes.push(asn1::contexto_explicito(
        0,
        &[asn1::sequencia(&[
            extensao(&OID_CRL_NUMBER, false, asn1::inteiro_u64(numero)),
            extensao(
                &OID_AKI,
                false,
                asn1::sequencia(&[id_chave_implicito(&aki)]),
            ),
        ])],
    ));
    let tbs = asn1::sequencia(&partes);
    let assinatura = ed25519::assinar(ac.privada, &tbs);
    asn1::sequencia(&[tbs, alg, asn1::bit_string(&assinatura, 0)])
}

/// PEM de 64 colunas, o que OpenSSL e OpenVPN esperam.
pub fn pem(rotulo: &str, der: &[u8]) -> String {
    let b64 = base64::codificar(der);
    let mut s = format!("-----BEGIN {rotulo}-----\n");
    for linha in b64.as_bytes().chunks(64) {
        s.push_str(std::str::from_utf8(linha).expect("base64 e ascii"));
        s.push('\n');
    }
    s.push_str(&format!("-----END {rotulo}-----\n"));
    s
}

pub fn cert_pem(der: &[u8]) -> String {
    pem("CERTIFICATE", der)
}

/// Chave privada Ed25519 em PKCS#8 (RFC 8410 secao 7): a semente dentro de uma
/// `OCTET STRING` dentro de outra.
pub fn chave_pem(privada: &[u8; 32]) -> String {
    let der = asn1::sequencia(&[
        asn1::inteiro_u64(0),
        asn1::sequencia(&[asn1::oid(&OID_ED25519)]),
        asn1::octet_string(&asn1::octet_string(privada)),
    ]);
    pem("PRIVATE KEY", &der)
}

/// Le a DER do primeiro bloco PEM do texto.
pub fn de_pem(texto: &str) -> Result<Vec<u8>, String> {
    let corpo: String = texto
        .lines()
        .skip_while(|l| !l.starts_with("-----BEGIN"))
        .skip(1)
        .take_while(|l| !l.starts_with("-----END"))
        .collect();
    base64::decodificar(&corpo).map_err(|e| format!("PEM invalido: {e}"))
}

/// Chave estatica do `tls-crypt` do OpenVPN: 256 bytes aleatorios no formato
/// «OpenVPN Static key V1», 16 linhas de 32 hex. Ela cifra e autentica o
/// proprio aperto de mao TLS -- quem nao a tem nao chega nem a ver o
/// certificado do servidor, e o servidor nao gasta CPU com varredura.
pub fn chave_tls_crypt() -> String {
    let bytes = bytes_aleatorios(256);
    let mut s = String::from("-----BEGIN OpenVPN Static key V1-----\n");
    for linha in bytes.chunks(16) {
        s.push_str(&phxsql_core::hash::para_hex(linha));
        s.push('\n');
    }
    s.push_str("-----END OpenVPN Static key V1-----\n");
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ac() -> Emitido {
        emitir(Papel::Ac, "Empresa Teste", "phxvpn AC", 10, None).unwrap()
    }

    /// A folha e assinada pela AC: a publica da AC confere a assinatura sobre a
    /// TBS da folha, e a publica da propria folha NAO confere (prova de que o
    /// emissor e mesmo a AC, e nao autoassinatura disfarcada).
    #[test]
    fn folha_confere_contra_a_ac_e_nao_contra_si_mesma() {
        let a = ac();
        let emissora = Ac {
            organizacao: "Empresa Teste",
            cn: "phxvpn AC",
            privada: &a.privada,
        };
        let f = emitir(
            Papel::Membro,
            "Empresa Teste",
            "joao@rede",
            1,
            Some(&emissora),
        )
        .unwrap();
        let (cert, _) = asn1::esperar(&f.der, asn1::TAG_SEQUENCE).unwrap();
        let partes = asn1::filhos(cert.conteudo).unwrap();
        let (_, depois_tbs) = asn1::analisar(cert.conteudo).unwrap();
        let tbs = &cert.conteudo[..cert.conteudo.len() - depois_tbs.len()];
        let (_, assinatura) = asn1::decodificar_bit_string(partes[2].conteudo).unwrap();
        let assinatura: [u8; 64] = assinatura.try_into().unwrap();
        let pub_ac = ed25519::chave_publica(&a.privada);
        let pub_folha = ed25519::chave_publica(&f.privada);
        assert!(ed25519::conferir(&pub_ac, tbs, &assinatura));
        assert!(!ed25519::conferir(&pub_folha, tbs, &assinatura));
    }

    /// Prova contra o OpenSSL: a CRL assinada pela AC derruba a verificacao
    /// do certificado revogado e deixa passar o outro. Roda so onde houver o
    /// binario `openssl`; sem ele, diz que nao rodou.
    #[test]
    fn crl_revoga_no_openssl() {
        let Some(openssl) = crate::supervisor::achar_no_path("openssl") else {
            eprintln!("NAO RODOU: sem openssl no PATH");
            return;
        };
        let a = ac();
        let emissora = Ac {
            organizacao: "Empresa Teste",
            cn: "phxvpn AC",
            privada: &a.privada,
        };
        let f1 = emitir(Papel::Membro, "Empresa Teste", "ana.1", 1, Some(&emissora)).unwrap();
        let f2 = emitir(Papel::Membro, "Empresa Teste", "bia.1", 1, Some(&emissora)).unwrap();
        let serie = phxsql_core::hash::de_hex(&f1.serie_hex).unwrap();
        let d = std::env::temp_dir().join(format!("phxvpn-crl-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let gravar = |n: &str, t: String| std::fs::write(d.join(n), t).unwrap();
        gravar("ca.pem", cert_pem(&a.der));
        gravar("f1.pem", cert_pem(&f1.der));
        gravar("f2.pem", cert_pem(&f2.der));
        let v = Validade::de_agora_por_anos(1);
        gravar("crl.pem", pem("X509 CRL", &crl(&emissora, &[serie], 1, &v)));
        gravar("vazia.pem", pem("X509 CRL", &crl(&emissora, &[], 2, &v)));
        let verificar = |cert: &str, lista: &str| {
            std::process::Command::new(&openssl)
                .current_dir(&d)
                .args([
                    "verify",
                    "-crl_check",
                    "-CAfile",
                    "ca.pem",
                    "-CRLfile",
                    lista,
                    cert,
                ])
                .output()
                .unwrap()
        };
        let r1 = verificar("f1.pem", "crl.pem");
        assert!(!r1.status.success(), "o revogado passou");
        assert!(
            String::from_utf8_lossy(&r1.stdout).contains("revoked")
                || String::from_utf8_lossy(&r1.stderr).contains("revoked")
        );
        assert!(
            verificar("f2.pem", "crl.pem").status.success(),
            "o nao revogado caiu"
        );
        assert!(
            verificar("f1.pem", "vazia.pem").status.success(),
            "CRL vazia derrubou alguem"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn folha_sem_ac_e_recusada() {
        assert!(emitir(Papel::Servidor, "E", "s", 1, None).is_err());
    }

    #[test]
    fn pem_ida_e_volta() {
        let a = ac();
        assert_eq!(de_pem(&cert_pem(&a.der)).unwrap(), a.der);
        assert!(chave_pem(&a.privada).starts_with("-----BEGIN PRIVATE KEY-----\n"));
    }

    #[test]
    fn tls_crypt_tem_16_linhas_de_32_hex() {
        let k = chave_tls_crypt();
        let linhas: Vec<&str> = k.lines().collect();
        assert_eq!(linhas.len(), 18);
        assert!(linhas[1..17].iter().all(|l| l.len() == 32));
    }
}
