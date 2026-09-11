//! Prova real da fatia 1 do correio `.p12` CONTRA A FERRAMENTA.
//!
//! A pétrea manda: o que depende do sistema/ferramenta se prova contra a
//! ferramenta, nao por teste unitario. Os testes de `x509.rs` e `asn1.rs`
//! provam a nossa cripto e a nossa gramatica; ESTE exemplo prova a UNICA coisa
//! que eles nao podem provar sozinhos -- que um OpenSSL de verdade le o nosso
//! DER, reconhece Ed25519 e X25519, e confere a nossa assinatura Ed25519 com o
//! verificador DELE.
//!
//! Prova nos dois sentidos (pétrea da prova real):
//!   * PASSA com o conserto: OpenSSL parseia, reconhece os algoritmos, e
//!     `pkeyutl -verify` e `verify` aceitam a assinatura.
//!   * FALHA com o defeito reposto: um byte virado na assinatura faz o OpenSSL
//!     recusar (e o nosso `conferir` tambem); um OID trocado faz o OpenSSL
//!     deixar de reconhecer Ed25519.
//!
//! Rodar: cargo run -q --example cert-openssl -p phxsql-core
//!
//! Se nao houver `openssl` na maquina, o exemplo diz PULADO e sai com 0 -- nao
//! ha o que provar contra uma ferramenta ausente. Aqui ha OpenSSL 3.0.13.

use phxsql_core::{asn1, ed25519, x25519, x509};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Roda `openssl` com os argumentos e devolve (saiu com codigo 0, stdout, stderr).
fn openssl(args: &[&str]) -> Option<(bool, String, String)> {
    let saida = Command::new("openssl").args(args).output().ok()?;
    Some((
        saida.status.success(),
        String::from_utf8_lossy(&saida.stdout).into_owned(),
        String::from_utf8_lossy(&saida.stderr).into_owned(),
    ))
}

fn main() {
    // Diretorio temporario proprio, para nao pisar em nada e limpar no fim.
    let dir: PathBuf =
        std::env::temp_dir().join(format!("phxsql-cert-openssl-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let p = |nome: &str| dir.join(nome).to_string_lossy().into_owned();

    // Sem openssl, nao ha prova contra ferramenta -- diz e sai limpo.
    if openssl(&["version"]).map(|(ok, _, _)| ok) != Some(true) {
        println!("PULADO: openssl nao encontrado nesta maquina.");
        let _ = fs::remove_dir_all(&dir);
        return;
    }

    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| res.push((c, n.to_string()));

    // ---- gera os dois certificados do mesmo dono ----
    //
    // A Ed25519 do dono e a chave privada do TEST 1 da RFC 8032, para que a
    // publica impressa pelo OpenSSL seja o valor conhecido d75a9801... -- se o
    // OpenSSL imprimir esse valor, ele leu os mesmos bytes que embutimos.
    let sk =
        ed25519::chave_de_hex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
            .unwrap();
    let pk_ed = ed25519::chave_publica(&sk);
    let pk_x = x25519::chave_publica(&x25519::gerar_privada());
    let validade = x509::Validade::de_agora_por_anos(10);
    let cn = "adrianoboller@empresa.phxsql.com.br";

    let ident = x509::emitir(
        &x509::Sujeito {
            cn,
            algo: x509::AlgoChave::Ed25519,
            chave_publica: &pk_ed,
        },
        &validade,
        &[0x11, 0x22, 0x33, 0x44],
        &sk,
    );
    let troca = x509::emitir(
        &x509::Sujeito {
            cn,
            algo: x509::AlgoChave::X25519,
            chave_publica: &pk_x,
        },
        &validade,
        &[0x55],
        &sk,
    );
    fs::write(p("ident.der"), &ident.der).unwrap();
    fs::write(p("troca.der"), &troca.der).unwrap();
    fs::write(p("ident.tbs"), &ident.tbs).unwrap();
    fs::write(p("ident.sig"), ident.assinatura).unwrap();
    fs::write(p("troca.tbs"), &troca.tbs).unwrap();

    // ---- 1) OpenSSL parseia e reconhece os algoritmos ----
    println!("1) openssl x509 -text -noout sobre o certificado de IDENTIDADE (Ed25519):\n");
    let (ok_id, texto_id, err_id) = openssl(&[
        "x509",
        "-inform",
        "DER",
        "-in",
        &p("ident.der"),
        "-text",
        "-noout",
    ])
    .expect("openssl x509");
    // Imprime so o miolo que interessa: algoritmos e chave.
    for linha in texto_id.lines() {
        let l = linha.trim_start();
        if l.starts_with("Version")
            || l.starts_with("Signature Algorithm")
            || l.starts_with("Public Key Algorithm")
            || l.starts_with("Subject:")
            || l.starts_with("Issuer:")
        {
            println!("   {}", linha.trim_end());
        }
    }
    ok(
        ok_id && !texto_id.is_empty(),
        "openssl parseia o cert de identidade",
    );
    ok(
        texto_id.contains("Signature Algorithm: ED25519"),
        "openssl reconhece a assinatura ED25519",
    );
    ok(
        texto_id.contains("Public Key Algorithm: ED25519"),
        "openssl reconhece a chave publica ED25519",
    );
    ok(
        texto_id.contains("d7:5a:98:01"),
        "a publica lida pelo openssl e o vetor conhecido da RFC 8032",
    );
    if !ok_id {
        eprintln!("   stderr: {err_id}");
    }

    println!("\n2) openssl x509 -text -noout sobre o certificado de TROCA (X25519):\n");
    let (_, texto_tr, _) = openssl(&[
        "x509",
        "-inform",
        "DER",
        "-in",
        &p("troca.der"),
        "-text",
        "-noout",
    ])
    .expect("openssl x509");
    for linha in texto_tr.lines() {
        let l = linha.trim_start();
        if l.starts_with("Signature Algorithm") || l.starts_with("Public Key Algorithm") {
            println!("   {}", linha.trim_end());
        }
    }
    ok(
        texto_tr.contains("Public Key Algorithm: X25519"),
        "openssl reconhece a chave publica X25519",
    );
    ok(
        texto_tr.contains("Signature Algorithm: ED25519"),
        "o cert de troca foi assinado pela Ed25519 do dono",
    );

    // ---- 2) OpenSSL CONFERE a nossa assinatura Ed25519 ----
    println!("\n3) o verificador Ed25519 do OpenSSL aceita a NOSSA assinatura:");
    // Extrai a publica Ed25519 do cert de identidade, na forma que o OpenSSL usa.
    let (_, pub_pem, _) = openssl(&[
        "x509",
        "-in",
        &p("ident.der"),
        "-inform",
        "DER",
        "-pubkey",
        "-noout",
    ])
    .expect("openssl -pubkey");
    fs::write(p("ident.pub.pem"), &pub_pem).unwrap();

    // pkeyutl -verify -rawin: Ed25519 e assinatura de uma tacada, sobre a TBS crua.
    let verifica = |tbs: &str, sig: &str| -> bool {
        openssl(&[
            "pkeyutl",
            "-verify",
            "-pubin",
            "-inkey",
            &p("ident.pub.pem"),
            "-rawin",
            "-in",
            tbs,
            "-sigfile",
            sig,
        ])
        .map(|(ok, _, _)| ok)
            == Some(true)
    };
    ok(
        verifica(&p("ident.tbs"), &p("ident.sig")),
        "openssl pkeyutl -verify aceita a assinatura da identidade (autoassinada)",
    );
    // A assinatura da TROCA tambem e da Ed25519 do dono -> confere com a MESMA publica.
    fs::write(p("troca.sig"), troca.assinatura).unwrap();
    ok(
        verifica(&p("troca.tbs"), &p("troca.sig")),
        "openssl pkeyutl -verify aceita a assinatura do cert de troca",
    );

    // verify de cadeia: o cert de identidade e um autoassinado valido.
    let (_, pem_id, _) = openssl(&[
        "x509",
        "-in",
        &p("ident.der"),
        "-inform",
        "DER",
        "-outform",
        "PEM",
    ])
    .expect("der->pem");
    fs::write(p("ident.pem"), &pem_id).unwrap();
    let (ok_verify, out_verify, _) =
        openssl(&["verify", "-CAfile", &p("ident.pem"), &p("ident.pem")]).expect("openssl verify");
    println!("   openssl verify: {}", out_verify.trim());
    ok(
        ok_verify,
        "openssl verify aceita o autoassinado de identidade como raiz",
    );

    // ---- 3) FALHA-COM-DEFEITO: um byte virado na assinatura ----
    println!("\n4) falha-com-defeito -- um bit virado na assinatura:");
    let mut sig_torta = ident.assinatura;
    sig_torta[7] ^= 1;
    fs::write(p("ident.sig.bad"), sig_torta).unwrap();
    let openssl_recusa_sig = !verifica(&p("ident.tbs"), &p("ident.sig.bad"));
    let nosso_recusa_sig = !ed25519::conferir(&pk_ed, &ident.tbs, &sig_torta);
    println!(
        "   openssl pkeyutl -verify (sig torta): {}",
        if openssl_recusa_sig {
            "RECUSOU"
        } else {
            "aceitou (!)"
        }
    );
    ok(
        openssl_recusa_sig,
        "openssl RECUSA a assinatura com um bit virado",
    );
    ok(
        nosso_recusa_sig,
        "o nosso ed25519::conferir tambem recusa a assinatura torta",
    );

    // ---- 4) FALHA-COM-DEFEITO: OID trocado ----
    println!("\n5) falha-com-defeito -- OID id-Ed25519 (…112) trocado por um nao registrado:");
    // Troca todo 06 03 2B 65 70 (1.3.101.112) por 06 03 2B 65 63 (1.3.101.99).
    let alvo = [0x06u8, 0x03, 0x2b, 0x65, 0x70];
    let mut der_oidbad = ident.der.clone();
    let mut trocas = 0;
    let mut i = 0;
    while i + alvo.len() <= der_oidbad.len() {
        if der_oidbad[i..i + alvo.len()] == alvo {
            der_oidbad[i + 4] = 0x63;
            trocas += 1;
            i += alvo.len();
        } else {
            i += 1;
        }
    }
    fs::write(p("ident.oidbad.der"), &der_oidbad).unwrap();
    let (_, texto_bad, _) = openssl(&[
        "x509",
        "-inform",
        "DER",
        "-in",
        &p("ident.oidbad.der"),
        "-text",
        "-noout",
    ])
    .expect("openssl x509 oidbad");
    println!(
        "   openssl agora diz: {}",
        texto_bad
            .lines()
            .find(|l| l.trim_start().starts_with("Signature Algorithm"))
            .map(str::trim)
            .unwrap_or("(nao parseou)")
    );
    ok(trocas >= 1, "havia OID id-Ed25519 no DER para trocar");
    ok(
        !texto_bad.contains("ED25519"),
        "com o OID trocado, o openssl NAO reconhece mais ED25519",
    );

    // ---- 5) round-trip do ASN.1 ----
    println!("\n6) round-trip do codec ASN.1 (o cert inteiro le de volta em DER):");
    let round = (|| -> phxsql_core::Result<bool> {
        let (cert_el, resto) = asn1::esperar(&ident.der, asn1::TAG_SEQUENCE)?;
        if !resto.is_empty() {
            return Ok(false);
        }
        let campos = asn1::filhos(cert_el.conteudo)?;
        // Certificate = { tbs, sigAlg, sigValue }. Re-embrulhar o conteudo da
        // TBS numa SEQUENCE tem de reproduzir a TBS que assinamos, byte a byte.
        Ok(campos.len() == 3
            && campos[0].tag == asn1::TAG_SEQUENCE
            && campos[2].tag == asn1::TAG_BIT_STRING
            && asn1::sequencia(&[campos[0].conteudo.to_vec()]) == ident.tbs)
    })()
    .unwrap_or(false);
    ok(
        round,
        "o DER volta pelo nosso leitor e a TBS bate byte a byte",
    );

    // ---- placar ----
    println!("\n===== RESULTADO =====");
    let mut falhas = 0;
    for (c, n) in &res {
        if !c {
            falhas += 1;
        }
        println!("  {}  {}", if *c { " ok  " } else { "FALHA" }, n);
    }
    println!(
        "\n{} checagens, {} ok, {} falha(s).",
        res.len(),
        res.len() - falhas,
        falhas
    );
    println!(
        "{}",
        if falhas == 0 {
            "PROVA VERDE"
        } else {
            "PROVA VERMELHA"
        }
    );

    let _ = fs::remove_dir_all(&dir);
    if falhas != 0 {
        std::process::exit(1);
    }
}
