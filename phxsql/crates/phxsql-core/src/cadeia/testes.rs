//! A validacao de caminho contra o NIST PKITS (secoes escolhidas, e as
//! fora com o motivo) e o nome do servidor contra a RFC 9525 e o OpenSSL.

use super::*;
use crate::json::Json;

fn pasta() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/pkits")
}

/// 1 de janeiro de 2026: os certificados do PKITS valem de 2010 a 2030, e
/// os testes de data (4.2) mexem em 1950, 2047 e 2050. Instante fixo para o
/// teste nao virar outro em 2031.
const AGORA: i64 = 1_767_225_600;

/// Os testes do PKITS que ficam FORA, cada um com o motivo -- a lista e
/// conferida: um nome aqui que nao existe nos vetores reprova.
const FORA: [(&str, &str); 9] = [
    ("4.1.4 Valid DSA Signatures Test4", "DSA nao e aceito"),
    (
        "4.1.5 Valid DSA Parameter Inheritance Test5",
        "DSA nao e aceito",
    ),
    (
        "4.1.6 Invalid DSA Signature Test6",
        "DSA nao e aceito (recusa por isso, nao pela assinatura)",
    ),
    (
        "4.5.2 Invalid Basic Self-Issued Old With New Test2",
        "invalido por REVOGACAO (CRL)",
    ),
    (
        "4.5.5 Invalid Basic Self-Issued New With Old Test5",
        "invalido por REVOGACAO (CRL)",
    ),
    (
        "4.5.7 Invalid Basic Self-Issued CRL Signing Key Test7",
        "invalido por REVOGACAO (CRL)",
    ),
    (
        "4.5.8 Invalid Basic Self-Issued CRL Signing Key Test8",
        "invalido porque a CRL nao se confere",
    ),
    (
        "4.7.4 Invalid keyUsage Critical cRLSign False Test4",
        "invalido porque a CRL nao se confere",
    ),
    (
        "4.7.5 Invalid keyUsage Not Critical cRLSign False Test5",
        "invalido porque a CRL nao se confere",
    ),
];

/// Os dois cujo `CertPath` nao e o caminho: o certificado do meio so assina
/// a CRL (PKITS §4.5.4 e §4.5.6).
const CAMINHO_COM_CRL: [&str; 2] = [
    "4.5.4 Valid Basic Self-Issued New With Old Test4",
    "4.5.6 Valid Basic Self-Issued CRL Signing Key Test6",
];

#[test]
fn nist_pkits_secoes_4_1_4_2_4_3_4_5_4_6_4_7_4_13_4_16() {
    let p = pasta();
    let vetores =
        Json::analisar(&std::fs::read_to_string(p.join("vetores.json")).unwrap()).unwrap();
    let lista = vetores.lista().unwrap();
    for (nome, _) in FORA {
        assert!(
            lista.iter().any(|v| v.texto_ou("Name", "") == nome),
            "a lista FORA cita um teste que nao existe: {nome}"
        );
    }
    let (mut rodou, mut validos) = (0, 0);
    let mut erros = Vec::new();
    for v in lista {
        let nome = v.texto_ou("Name", "");
        if FORA.iter().any(|(n, _)| *n == nome) {
            continue;
        }
        let ders: Vec<Vec<u8>> = v
            .campo("CertPath")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .map(|c| std::fs::read(p.join("certs").join(c.texto().unwrap())).unwrap())
            .collect();
        let deve = v.booleano_ou("ShouldValidate", false);
        // O veredito e o da BUSCA, com as intermediarias fora de ordem: e o
        // que o cliente TLS faz, e e o que acerta os 4.5.4 e 4.5.6, cujo
        // `CertPath` traz um certificado que so serve para conferir a CRL.
        let folha = ders.last().unwrap();
        let mut inter: Vec<Vec<u8>> = ders[1..ders.len() - 1].to_vec();
        inter.reverse();
        let busca = validar(folha, &inter, &ders[..1], AGORA, Uso::Qualquer);
        if busca.is_ok() != deve {
            erros.push(format!("{nome} (busca): {busca:?}"));
            continue;
        }
        // E o caminho DADO, validado como veio, tem de dizer o mesmo --
        // menos nos dois com o certificado de CRL no meio.
        if !CAMINHO_COM_CRL.contains(&nome) {
            // Certificado que nem se le e caminho invalido.
            let direto = ders
                .iter()
                .map(|d| Certificado::analisar(d))
                .collect::<Result<Vec<_>>>()
                .and_then(|certs| {
                    let (ancora, caminho) = certs.split_first().unwrap();
                    validar_caminho(ancora, caminho, AGORA, Uso::Qualquer)
                });
            if direto.is_ok() != deve {
                erros.push(format!("{nome} (caminho dado): {direto:?}"));
                continue;
            }
        }
        validos += usize::from(deve);
        rodou += 1;
    }
    assert!(
        erros.is_empty(),
        "{} divergencias:\n{}",
        erros.len(),
        erros.join("\n")
    );
    assert_eq!(rodou, 86, "95 vetores do recorte menos os 9 fora");
    assert!(validos >= 40, "so {validos} validos");
}

// ------------------------------------------------- RFC 9525 e OpenSSL ----

/// Os exemplos do texto da RFC 9525 §6.3 e §7.1.
#[test]
fn rfc_9525_os_exemplos_do_texto() {
    // §6.3: caixa ignorada.
    assert!(dns_casa("WWW.BigCompany.Example", "www.bigcompany.example"));
    // §6.3 regra 2 e §7.1: curinga so como o rotulo mais a esquerda inteiro,
    // e um so.
    assert!(dns_casa("foo.bigcompany.example", "*.bigcompany.example"));
    assert!(!dns_casa(
        "a.b.bigcompany.example",
        "*.*.bigcompany.example"
    ));
    // «A wildcard ... can only match one label».
    assert!(!dns_casa("a.b.bigcompany.example", "*.bigcompany.example"));
    assert!(!dns_casa("bigcompany.example", "*.bigcompany.example"));
    // Curinga parcial nao e o rotulo inteiro: ignorado.
    assert!(!dns_casa("foo.bigcompany.example", "f*.bigcompany.example"));
    assert!(!dns_casa("foo.bigcompany.example", "foo.*.example"));
}

/// A pasta de trabalho dos testes com `openssl`: o guarda do `tls::testes`,
/// que apaga no `Drop` -- um segundo guarda aqui seria um segundo `temp_dir`
/// fora do catalogo do `conferidor_temporarios`.
use crate::tls::testes::Dir;

fn openssl(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let s = std::process::Command::new("openssl")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    s
}

fn der_de_pem(caminho: &std::path::Path) -> Vec<u8> {
    crate::x509::blocos_pem(&std::fs::read_to_string(caminho).unwrap(), "CERTIFICATE")
        .unwrap()
        .remove(0)
}

/// O mesmo nome conferido por esta casa e pelo `openssl x509 -checkhost /
/// -checkip`, nos certificados que o OPENSSL gera -- outra mao, outra
/// leitura do SAN. Os casos sao os em que o OpenSSL de fabrica e a RFC 9525
/// concordam; o curinga parcial (`f*.`), em que o OpenSSL de fabrica aceita e
/// a RFC manda ignorar, fica nos exemplos do texto acima.
#[test]
fn o_nome_confere_igual_ao_openssl() {
    let d = Dir::novo("cadeia-nomes");
    let casos: [(&str, &str, bool); 9] = [
        ("DNS:www.exemplo.com", "WWW.Exemplo.COM", true),
        ("DNS:*.exemplo.com", "a.exemplo.com", true),
        ("DNS:*.exemplo.com", "a.b.exemplo.com", false),
        ("DNS:*.exemplo.com", "exemplo.com", false),
        ("DNS:exemplo.com,DNS:outro.org", "outro.org", true),
        ("DNS:exemplo.com", "exemplo.com.br", false),
        ("IP:10.0.0.1", "10.0.0.1", true),
        ("DNS:10.0.0.1", "10.0.0.1", false),
        ("IP:::1", "::1", true),
    ];
    for (i, (san, nome, esperado)) in casos.iter().enumerate() {
        let pem = d.0.join(format!("c{i}.pem"));
        let s = openssl(
            &d.0,
            &[
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-nodes",
                "-keyout",
                "k.pem",
                "-out",
                pem.to_str().unwrap(),
                "-days",
                "2",
                "-subj",
                "/CN=nao-conta.exemplo",
                "-addext",
                &format!("subjectAltName={san}"),
            ],
        );
        assert!(s.status.success(), "{}", String::from_utf8_lossy(&s.stderr));
        let nosso = conferir_folha_tls(&der_de_pem(&pem), nome).is_ok();
        let ip = nome.parse::<std::net::IpAddr>().is_ok();
        let deles = openssl(
            &d.0,
            &[
                "x509",
                "-noout",
                "-in",
                pem.to_str().unwrap(),
                if ip { "-checkip" } else { "-checkhost" },
                nome,
            ],
        );
        let deles_ok = String::from_utf8_lossy(&deles.stdout).contains("does match");
        assert_eq!(nosso, *esperado, "{san} x {nome}: esta casa");
        assert_eq!(deles_ok, *esperado, "{san} x {nome}: o openssl");
    }
}

/// Uma cadeia de tres niveis gerada pelo OPENSSL: raiz RSA 2048, intermediaria
/// P-384 (assinada em SHA-384) e folha P-256. `ext_inter` e `ext_folha` sao as
/// extensoes; devolve (raiz, intermediaria, folha) em DER.
pub(crate) fn cadeia_do_openssl(
    d: &std::path::Path,
    ext_inter: &str,
    ext_folha: &str,
    dias_folha: &str,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let ok = |s: std::process::Output| {
        assert!(s.status.success(), "{}", String::from_utf8_lossy(&s.stderr));
    };
    ok(openssl(
        d,
        &[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "raiz.key",
            "-out",
            "raiz.pem",
            "-subj",
            "/O=PhxSql Teste/CN=Raiz de Teste",
            "-days",
            "30",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
        ],
    ));
    std::fs::write(
        d.join("ext"),
        format!("[inter]\n{ext_inter}\n[folha]\n{ext_folha}\n"),
    )
    .unwrap();
    ok(openssl(
        d,
        &[
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            "ec_paramgen_curve:P-384",
            "-out",
            "inter.key",
        ],
    ));
    ok(openssl(
        d,
        &[
            "req",
            "-new",
            "-key",
            "inter.key",
            "-subj",
            "/O=PhxSql Teste/CN=Intermediaria",
            "-out",
            "inter.csr",
        ],
    ));
    ok(openssl(
        d,
        &[
            "x509",
            "-req",
            "-in",
            "inter.csr",
            "-CA",
            "raiz.pem",
            "-CAkey",
            "raiz.key",
            "-CAcreateserial",
            "-days",
            "20",
            "-sha384",
            "-extfile",
            "ext",
            "-extensions",
            "inter",
            "-out",
            "inter.pem",
        ],
    ));
    ok(openssl(
        d,
        &[
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-out",
            "folha.key",
        ],
    ));
    ok(openssl(
        d,
        &[
            "req",
            "-new",
            "-key",
            "folha.key",
            "-subj",
            "/CN=localhost",
            "-out",
            "folha.csr",
        ],
    ));
    ok(openssl(
        d,
        &[
            "x509",
            "-req",
            "-in",
            "folha.csr",
            "-CA",
            "inter.pem",
            "-CAkey",
            "inter.key",
            "-CAcreateserial",
            "-days",
            dias_folha,
            "-sha256",
            "-extfile",
            "ext",
            "-extensions",
            "folha",
            "-out",
            "folha.pem",
        ],
    ));
    (
        der_de_pem(&d.join("raiz.pem")),
        der_de_pem(&d.join("inter.pem")),
        der_de_pem(&d.join("folha.pem")),
    )
}

pub(crate) const EXT_INTER: &str =
    "basicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign";
pub(crate) const EXT_FOLHA: &str =
    "basicConstraints=CA:FALSE\nkeyUsage=critical,digitalSignature\n\
     extendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost,IP:127.0.0.1";

fn agora_real() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// O veredito desta casa e o do `openssl verify -x509_strict` sobre a mesma
/// cadeia, nos dois sentidos: a boa passa nos dois, cada defeito cai nos dois.
#[test]
fn cadeias_do_openssl_conferem_igual_ao_openssl_verify() {
    // (nome, extensoes da intermediaria, da folha, nome pedido, deve passar)
    let casos: [(&str, &str, &str, &str, bool); 6] = [
        ("boa", EXT_INTER, EXT_FOLHA, "localhost", true),
        ("boa-ip", EXT_INTER, EXT_FOLHA, "127.0.0.1", true),
        (
            "inter-nao-ca",
            "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,keyCertSign",
            EXT_FOLHA,
            "localhost",
            false,
        ),
        (
            "inter-sem-keycertsign",
            "basicConstraints=critical,CA:TRUE\nkeyUsage=critical,digitalSignature",
            EXT_FOLHA,
            "localhost",
            false,
        ),
        (
            "critica-desconhecida",
            EXT_INTER,
            "basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost\n1.3.6.1.4.1.99999.1=critical,ASN1:NULL",
            "localhost",
            false,
        ),
        (
            "inter-so-email",
            "basicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign\nextendedKeyUsage=emailProtection",
            EXT_FOLHA,
            "localhost",
            false,
        ),
    ];
    for (rotulo, ei, ef, nome, deve) in casos {
        let d = Dir::novo(&format!("cadeia-{rotulo}"));
        let (raiz, inter, folha) = cadeia_do_openssl(&d.0, ei, ef, "10");
        let nosso = validar_servidor_tls(&[folha, inter], &[raiz], nome, agora_real());
        assert_eq!(nosso.is_ok(), deve, "{rotulo}: {nosso:?}");
        let mut v = vec![
            "verify",
            "-x509_strict",
            "-purpose",
            "sslserver",
            "-CAfile",
            "raiz.pem",
            "-untrusted",
            "inter.pem",
        ];
        let flag = if nome.parse::<std::net::IpAddr>().is_ok() {
            "-verify_ip"
        } else {
            "-verify_hostname"
        };
        v.extend([flag, nome, "folha.pem"]);
        let deles = openssl(&d.0, &v);
        assert_eq!(
            deles.status.success(),
            deve,
            "{rotulo}: o openssl verify discorda: {}{}",
            String::from_utf8_lossy(&deles.stdout),
            String::from_utf8_lossy(&deles.stderr)
        );
    }
}

/// Validade, ancora e nome, pela mesma cadeia boa.
#[test]
fn vencida_ancora_alheia_e_nome_errado_recusam() {
    let d = Dir::novo("cadeia-tempo");
    let (raiz, inter, folha) = cadeia_do_openssl(&d.0, EXT_INTER, EXT_FOLHA, "1");
    let agora = agora_real();
    let cadeia = [folha.clone(), inter.clone()];
    let raizes = [raiz.clone()];
    assert!(validar_servidor_tls(&cadeia, &raizes, "localhost", agora).is_ok());
    // Dois dias adiante, a folha de um dia venceu.
    let e = validar_servidor_tls(&cadeia, &raizes, "localhost", agora + 2 * 86_400)
        .unwrap_err()
        .to_string();
    assert!(e.contains("fora da validade"), "{e}");
    // Outra raiz com o MESMO nome: a assinatura nao fecha.
    let d2 = Dir::novo("cadeia-outra-raiz");
    let (outra, _, _) = cadeia_do_openssl(&d2.0, EXT_INTER, EXT_FOLHA, "1");
    assert!(validar_servidor_tls(&cadeia, &[outra], "localhost", agora).is_err());
    // Nome que o SAN nao tem; e o CN («localhost») nao conta para outro.
    assert!(validar_servidor_tls(&cadeia, &raizes, "exemplo.com", agora).is_err());
    // Sem a intermediaria, nao ha caminho.
    assert!(validar_servidor_tls(&[folha], &[raiz], "localhost", agora).is_err());
}

/// O pacote de raizes do conteiner se le quase inteiro -- RSA, P-256 e
/// P-384 -- e o numero de fora fica medido aqui, nao suposto.
#[test]
fn as_raizes_do_sistema_se_leem() {
    let Ok((raizes, fora)) = ancoras("sistema") else {
        // Sem pacote (Windows, conteiner minimo): a recusa e a resposta.
        assert!(ancoras("sistema")
            .unwrap_err()
            .to_string()
            .contains("tls_ca"));
        return;
    };
    eprintln!("raizes do sistema: {} lidas, {fora} fora", raizes.len());
    assert!(raizes.len() >= 100, "so {} raizes", raizes.len());
    assert_eq!(fora, 0, "raiz do sistema que nao se le");
    let mut tipos = std::collections::BTreeMap::new();
    for r in &raizes {
        let c = Certificado::analisar(r).unwrap();
        let k = match crate::x509::chave_do_spki(c.spki) {
            Ok(crate::x509::ChavePublica::Rsa(_)) => "rsa",
            Ok(crate::x509::ChavePublica::P256(_)) => "p256",
            Ok(crate::x509::ChavePublica::P384(_)) => "p384",
            Ok(_) => "outra",
            Err(_) => "ilegivel",
        };
        *tipos.entry(k).or_insert(0) += 1;
    }
    eprintln!("tipos: {tipos:?}");
    assert_eq!(tipos.get("ilegivel"), None, "{tipos:?}");
}

#[test]
fn tls_ca_de_arquivo_le_e_recusa_o_torto() {
    let d = Dir::novo("cadeia-tls-ca");
    let (raiz, _, _) = cadeia_do_openssl(&d.0, EXT_INTER, EXT_FOLHA, "2");
    let (lidas, _) = ancoras(d.0.join("raiz.pem").to_str().unwrap()).unwrap();
    assert_eq!(lidas, vec![raiz]);
    std::fs::write(d.0.join("vazio.pem"), "nada aqui").unwrap();
    assert!(ancoras(d.0.join("vazio.pem").to_str().unwrap()).is_err());
    assert!(ancoras(d.0.join("nao-existe.pem").to_str().unwrap()).is_err());
}
