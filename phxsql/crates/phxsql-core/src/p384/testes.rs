//! Os vetores OFICIAIS da verificacao ECDSA P-384: RFC 6979 A.2.6 (extraidos
//! do texto oficial por script, o tamanho de cada um conferido), NIST CAVP
//! `SigVer` (FIPS 186-3, so a P-384) e Wycheproof `ecdsa_secp384r1_sha384`.

use super::*;
use crate::json::Json;
use crate::rsa::Resumo;

fn hx(t: &str) -> Vec<u8> {
    let t = t.trim();
    let t = if t.len() % 2 == 1 {
        format!("0{t}")
    } else {
        t.to_string()
    };
    crate::hash::de_hex(&t).unwrap_or_else(|| panic!("hex torto: {t:?}"))
}

fn ponto(x: &[u8], y: &[u8]) -> Vec<u8> {
    let mut p = vec![4u8];
    for c in [x, y] {
        let mut v = vec![0u8; TAM - c.len().min(TAM)];
        v.extend_from_slice(c);
        p.extend_from_slice(&v);
    }
    p
}

fn resumo(sha: &str) -> Option<Resumo> {
    match sha {
        "SHA-1" => Some(Resumo::Sha1),
        "SHA-256" => Some(Resumo::Sha256),
        "SHA-384" => Some(Resumo::Sha384),
        "SHA-512" => Some(Resumo::Sha512),
        _ => None,
    }
}

const A26_UX: &str = "ec3a4e415b4e19a4568618029f427fa5da9a8bc4ae92e02e06aae5286b300c64def8f0ea9055866064a254515480bc13";
const A26_UY: &str = "8015d9b72d7d57244ea8ef9ac0c621896708a59367f9dfb9f54ca84b3f1c9db1288b231c3ae0d4fe7344fd2533264720";
const A26: [(&str, &str, &str, &str); 8] = [
    ("SHA-1", "sample", "ec748d839243d6fbef4fc5c4859a7dffd7f3abddf72014540c16d73309834fa37b9ba002899f6fda3a4a9386790d4eb2", "a3bcfa947beef4732bf247ac17f71676cb31a847b9ff0cbc9c9ed4c1a5b3facf26f49ca031d4857570ccb5ca4424a443"),
    ("SHA-256", "sample", "21b13d1e013c7fa1392d03c5f99af8b30c570c6f98d4ea8e354b63a21d3daa33bde1e888e63355d92fa2b3c36d8fb2cd", "f3aa443fb107745bf4bd77cb3891674632068a10ca67e3d45db2266fa7d1feebefdc63eccd1ac42ec0cb8668a4fa0ab0"),
    ("SHA-384", "sample", "94edbb92a5ecb8aad4736e56c691916b3f88140666ce9fa73d64c4ea95ad133c81a648152e44acf96e36dd1e80fabe46", "99ef4aeb15f178cea1fe40db2603138f130e740a19624526203b6351d0a3a94fa329c145786e679e7b82c71a38628ac8"),
    ("SHA-512", "sample", "ed0959d5880ab2d869ae7f6c2915c6d60f96507f9cb3e047c0046861da4a799cfe30f35cc900056d7c99cd7882433709", "512c8cceee3890a84058ce1e22dbc2198f42323ce8aca9135329f03c068e5112dc7cc3ef3446defceb01a45c2667fdd5"),
    ("SHA-1", "test", "4bc35d3a50ef4e30576f58cd96ce6bf638025ee624004a1f7789a8b8e43d0678acd9d29876daf46638645f7f404b11c7", "d5a6326c494ed3ff614703878961c0fde7b2c278f9a65fd8c4b7186201a2991695ba1c84541327e966fa7b50f7382282"),
    ("SHA-256", "test", "6d6defac9ab64dabafe36c6bf510352a4cc27001263638e5b16d9bb51d451559f918eedaf2293be5b475cc8f0188636b", "2d46f3becbcc523d5f1a1256bf0c9b024d879ba9e838144c8ba6baeb4b53b47d51ab373f9845c0514eefb14024787265"),
    ("SHA-384", "test", "8203b63d3c853e8d77227fb377bcf7b7b772e97892a80f36ab775d509d7a5feb0542a7f0812998da8f1dd3ca3cf023db", "ddd0760448d42d8a43af45af836fce4de8be06b485e9b61b827c2f13173923e06a739f040649a667bf3b828246baa5a5"),
    ("SHA-512", "test", "a0d5d090c9980faf3c2ce57b7ae951d31977dd11c775d314af55f76c676447d06fb6495cd21b4b6e340fc236584fb277", "976984e59b4c77b0e8e4460dca3d9f20e07b9bb1f63beefaf576f6b2e8b224634a2092cd3792e0159ad9cee37659c736"),
];

#[test]
fn rfc_6979_a_2_6() {
    let q = ponto(&hx(A26_UX), &hx(A26_UY));
    for (sha, msg, r, s) in A26 {
        let h = resumo(sha).unwrap().calcular(msg.as_bytes());
        assert!(verificar_resumo(&q, &h, &hx(r), &hx(s)), "{sha} {msg}");
        // A mesma assinatura sobre a outra mensagem nao vale.
        let outra = if msg == "sample" { "test" } else { "sample" };
        let h2 = resumo(sha).unwrap().calcular(outra.as_bytes());
        assert!(!verificar_resumo(&q, &h2, &hx(r), &hx(s)), "{sha} trocada");
    }
}

#[test]
fn cavp_sigver_p384() {
    let texto = include_str!("../../testdata/tls/ecdsa_SigVer_P-384.rsp");
    let (mut sha, mut msg, mut qx, mut qy, mut r, mut s) = (
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let (mut rodou, mut passam) = (0, 0);
    for linha in texto.lines() {
        if let Some(cab) = linha.strip_prefix("[P-384,") {
            sha = resumo(cab.trim_end_matches(']'));
            continue;
        }
        let Some((k, v)) = linha.split_once(" = ") else {
            continue;
        };
        match k {
            "Msg" => msg = hx(v),
            "Qx" => qx = hx(v),
            "Qy" => qy = hx(v),
            "R" => r = hx(v),
            "S" => s = hx(v),
            "Result" => {
                let passa = v.starts_with('P');
                let h = sha.expect("cabecalho").calcular(&msg);
                assert_eq!(
                    verificar_resumo(&ponto(&qx, &qy), &h, &r, &s),
                    passa,
                    "caso {rodou} ({v})"
                );
                rodou += 1;
                passam += usize::from(passa);
            }
            _ => {}
        }
    }
    assert_eq!(rodou, 60);
    assert!(passam >= 10, "so {passam} validos");
}

#[test]
fn wycheproof_ecdsa_secp384r1_sha384() {
    let j = Json::analisar(include_str!(
        "../../testdata/tls/ecdsa_secp384r1_sha384_test.json"
    ))
    .unwrap();
    let mut rodou = 0;
    for g in j.campo("testGroups").and_then(Json::lista).unwrap() {
        let chave = g.campo("publicKey").unwrap();
        assert_eq!(chave.texto_ou("curve", ""), "secp384r1");
        let q = hx(chave.texto_ou("uncompressed", ""));
        for t in g.campo("tests").and_then(Json::lista).unwrap() {
            let msg = hx(t.texto_ou("msg", ""));
            let ok = assinatura_de_der(&hx(t.texto_ou("sig", "")))
                .is_some_and(|(r, s)| verificar(&q, &msg, &r, &s));
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
    assert_eq!(rodou, 504);
}

#[test]
fn ponto_fora_da_curva_se_recusa() {
    let mut q = ponto(&hx(A26_UX), &hx(A26_UY));
    let (_, msg, r, s) = A26[2];
    let h = crate::sha512::sha384(msg.as_bytes());
    assert!(verificar_resumo(&q, &h, &hx(r), &hx(s)));
    q[TAM] ^= 1;
    assert!(
        ponto_publico(&q).is_none(),
        "o ponto adulterado passou na curva"
    );
}
