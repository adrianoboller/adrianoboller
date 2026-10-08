//! Os vetores OFICIAIS da verificacao RSA, todos em `testdata/tls/` com a
//! origem no `LEIA-ME.md` de la: NIST CAVP (FIPS 186-3, `SigVer15` e
//! `SigVerPSS`), os do PKCS#1 v2.1 da RSA Labs (`pss-vect.txt`) e o Wycheproof
//! (C2SP). Cada caso conta: o teste diz quantos rodaram, para um parser que
//! pulasse tudo nao passar calado.

use super::*;
use crate::json::Json;

/// Hex do vetor. O CAVP as vezes escreve o numero sem o zero da frente
/// (comprimento impar): o meio-byte que falta e zero.
fn hx(t: &str) -> Vec<u8> {
    let t = t.trim();
    let t = if t.len() % 2 == 1 {
        format!("0{t}")
    } else {
        t.to_string()
    };
    crate::hash::de_hex(&t).unwrap_or_else(|| panic!("hex torto: {t:?}"))
}

fn resumo_do_nome(n: &str) -> Option<Resumo> {
    match n {
        "SHA1" | "SHA-1" => Some(Resumo::Sha1),
        "SHA256" | "SHA-256" => Some(Resumo::Sha256),
        "SHA384" | "SHA-384" => Some(Resumo::Sha384),
        "SHA512" | "SHA-512" => Some(Resumo::Sha512),
        _ => None,
    }
}

/// Le um `.rsp` do CAVP e devolve, por caso: `(n, e, sha, msg, s, sal, passa)`.
type CasoCavp = (Vec<u8>, Vec<u8>, Resumo, Vec<u8>, Vec<u8>, Vec<u8>, bool);

fn casos_cavp(texto: &str) -> Vec<CasoCavp> {
    let (mut n, mut e, mut sha, mut msg, mut s, mut sal) = (
        Vec::new(),
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut casos = Vec::new();
    for linha in texto.lines() {
        let Some((k, v)) = linha.split_once(" = ") else {
            continue;
        };
        match k.trim() {
            "n" => n = hx(v),
            "e" => e = hx(v),
            "SHAAlg" => sha = resumo_do_nome(v.trim()),
            "Msg" => msg = hx(v),
            "S" => s = hx(v),
            "SaltVal" => sal = if v.trim() == "00" { Vec::new() } else { hx(v) },
            "Result" => {
                let passa = v.trim_start().starts_with('P');
                let r = sha.expect("Result antes de SHAAlg");
                casos.push((
                    n.clone(),
                    e.clone(),
                    r,
                    msg.clone(),
                    s.clone(),
                    sal.clone(),
                    passa,
                ));
                sal.clear();
            }
            _ => {}
        }
    }
    casos
}

fn verifica_v15(n: &[u8], e: &[u8], r: Resumo, m: &[u8], s: &[u8]) -> bool {
    ChavePublica::nova(n, e).is_ok_and(|c| verificar_pkcs1v15(&c, r, m, s))
}

fn verifica_pss(n: &[u8], e: &[u8], r: Resumo, m: &[u8], s: &[u8], sal: usize) -> bool {
    ChavePublica::nova(n, e).is_ok_and(|c| verificar_pss(&c, r, m, s, sal))
}

#[test]
fn cavp_sigver15_fips_186_3() {
    let casos = casos_cavp(include_str!("../../testdata/tls/SigVer15_186-3.rsp"));
    assert_eq!(
        casos.len(),
        216,
        "o arquivo cortado tem 216 casos (sem SHA-224)"
    );
    let mut passam = 0;
    for (i, (n, e, r, m, s, _, passa)) in casos.iter().enumerate() {
        assert_eq!(verifica_v15(n, e, *r, m, s), *passa, "caso {i} ({r:?})");
        passam += usize::from(*passa);
    }
    assert!(
        passam > 30,
        "so {passam} casos validos: o parser perdeu algo"
    );
}

#[test]
fn cavp_sigverpss_fips_186_3() {
    let casos = casos_cavp(include_str!("../../testdata/tls/SigVerPSS_186-3.rsp"));
    assert_eq!(casos.len(), 216);
    for (i, (n, e, r, m, s, sal, passa)) in casos.iter().enumerate() {
        // O sal do caso diz o tamanho; nos casos «F» com o sal do grupo, o
        // tamanho e o mesmo -- e o defeito esta em outro lugar.
        assert_eq!(
            verifica_pss(n, e, *r, m, s, sal.len()),
            *passa,
            "caso {i} ({r:?}, sal {})",
            sal.len()
        );
    }
}

/// Os 60 exemplos do PKCS#1 v2.1 (RSA Labs), SHA-1, chaves de 1024 a 2048
/// bits -- inclusive as de 1025..1031, que exercitam o `emLen < k` e os bits
/// de cima mascarados (§9.1.2 passo 6).
#[test]
fn pss_vect_do_pkcs1_v2_1() {
    let texto = include_str!("../../testdata/tls/pss-vect.txt");
    let (mut rotulo, mut buf) = (String::new(), Vec::new());
    let (mut n, mut e, mut msg, mut sal) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut rodou = 0;
    let mut fechar = |rotulo: &str,
                      buf: &[u8],
                      n: &mut Vec<u8>,
                      e: &mut Vec<u8>,
                      msg: &mut Vec<u8>,
                      sal: &mut Vec<u8>| {
        match rotulo {
            "RSA modulus n:" => *n = buf.to_vec(),
            "RSA public exponent e:" => *e = buf.to_vec(),
            "Message to be signed:" => *msg = buf.to_vec(),
            "Salt:" => *sal = buf.to_vec(),
            "Signature:" => {
                let c = ChavePublica::nova(n, e).unwrap();
                assert!(
                    verificar_pss(&c, Resumo::Sha1, msg, buf, sal.len()),
                    "exemplo {rodou}"
                );
                let mut torta = buf.to_vec();
                torta[buf.len() / 2] ^= 1;
                assert!(!verificar_pss(&c, Resumo::Sha1, msg, &torta, sal.len()));
                rodou += 1;
            }
            _ => {}
        }
    };
    for linha in texto.lines() {
        if let Some(r) = linha.strip_prefix("# ") {
            fechar(&rotulo, &buf, &mut n, &mut e, &mut msg, &mut sal);
            rotulo = r.trim().to_string();
            buf.clear();
        } else if !linha.trim().is_empty() && !linha.starts_with('#') {
            buf.extend(hx(&linha.replace(' ', "")));
        }
    }
    fechar(&rotulo, &buf, &mut n, &mut e, &mut msg, &mut sal);
    assert_eq!(rodou, 60, "o pss-vect tem 10 chaves x 6 exemplos");
}

fn grupos(texto: &str) -> Vec<Json> {
    let j = Json::analisar(texto).unwrap();
    j.campo("testGroups")
        .and_then(Json::lista)
        .unwrap()
        .to_vec()
}

#[test]
fn wycheproof_rsa_pkcs1_2048_sha256() {
    let mut rodou = 0;
    for g in grupos(include_str!(
        "../../testdata/tls/rsa_signature_2048_sha256_test.json"
    )) {
        let chave = ChavePublica::de_der(&hx(g.texto_ou("publicKeyAsn", ""))).unwrap();
        let r = resumo_do_nome(g.texto_ou("sha", "")).unwrap();
        for t in g.campo("tests").and_then(Json::lista).unwrap() {
            let resultado = t.texto_ou("result", "");
            if resultado == "acceptable" {
                continue;
            }
            let ok = verificar_pkcs1v15(
                &chave,
                r,
                &hx(t.texto_ou("msg", "")),
                &hx(t.texto_ou("sig", "")),
            );
            assert_eq!(
                ok,
                resultado == "valid",
                "tcId {}: {}",
                t.inteiro_ou("tcId", 0),
                t.texto_ou("comment", "")
            );
            rodou += 1;
        }
    }
    assert_eq!(rodou, 258);
}

#[test]
fn wycheproof_rsa_pss_2048_sha256_mgf1_32() {
    let mut rodou = 0;
    for g in grupos(include_str!(
        "../../testdata/tls/rsa_pss_2048_sha256_mgf1_32_test.json"
    )) {
        let chave = ChavePublica::de_der(&hx(g.texto_ou("publicKeyAsn", ""))).unwrap();
        let r = resumo_do_nome(g.texto_ou("sha", "")).unwrap();
        assert_eq!(g.texto_ou("mgfSha", ""), "SHA-256");
        let sal = g.inteiro_ou("sLen", -1) as usize;
        for t in g.campo("tests").and_then(Json::lista).unwrap() {
            let ok = verificar_pss(
                &chave,
                r,
                &hx(t.texto_ou("msg", "")),
                &hx(t.texto_ou("sig", "")),
                sal,
            );
            assert_eq!(
                ok,
                t.texto_ou("result", "") == "valid",
                "tcId {}: {}",
                t.inteiro_ou("tcId", 0),
                t.texto_ou("comment", "")
            );
            rodou += 1;
        }
    }
    assert_eq!(rodou, 108);
}

#[test]
fn chave_fora_da_faixa_se_recusa() {
    // 512 bits: abaixo do piso.
    let n = [0xc3u8; 64];
    assert!(ChavePublica::nova(&n, &[1, 0, 1]).is_err());
    let mut n = vec![0xc3u8; 256];
    n[255] = 0xc2;
    assert!(ChavePublica::nova(&n, &[1, 0, 1]).is_err(), "modulo par");
    n[255] = 0xc3;
    assert!(ChavePublica::nova(&n, &[2]).is_err(), "expoente par");
    assert!(ChavePublica::nova(&n, &[1]).is_err(), "expoente 1");
}

/// O TAMANHO do sal e conferido, e nao deduzido do lugar do `0x01`: o TLS 1.3
/// manda o sal ter o tamanho do resumo (RFC 8446 §4.2.3), e quem deduz
/// aceitaria a assinatura de sal vazio onde a norma pede 32 bytes. O
/// Wycheproof de sal fixo tambem derruba a deducao; esta prova cruza os grupos
/// do CAVP (sal 0 contra sal do tamanho do resumo) nos 36 casos validos.
#[test]
fn o_tamanho_do_sal_e_conferido_e_nao_deduzido() {
    let casos = casos_cavp(include_str!("../../testdata/tls/SigVerPSS_186-3.rsp"));
    let mut cruzados = 0;
    for (n, e, r, m, s, sal, passa) in &casos {
        if !*passa {
            continue;
        }
        let errado = if sal.is_empty() { r.tamanho() } else { 0 };
        assert!(
            !verifica_pss(n, e, *r, m, s, errado),
            "sal de {} bytes aceito como {errado}",
            sal.len()
        );
        cruzados += 1;
    }
    assert_eq!(cruzados, 36);
}
