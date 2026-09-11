//! Simulacao dos NIVEIS de criptografia do correio PhxSql, pedida pelo dono
//! (11/09/2026): provar ENVIO e RECEBIMENTO em cada nivel, mais respostas
//! assinadas e o nivel 3 Masson que EXIGE o id da Maconaria.
//!
//! A escada — cada nivel acrescenta UMA camada por DENTRO; o E2E (ECDH das
//! identidades) fica SEMPRE por cima, e o nivel entra no AAD, entao rebaixar
//! o nivel de um cofre quebra a leitura:
//!
//!   padrao          -> E2E. Sempre. Le com a senha da conta + a chave publica do outro.
//!   nivel 1         -> E2E + FRASE (segredo passado por telefone/SMS, fora da banda).
//!   nivel 2 (p12)   -> E2E + p12 (ECDH do certificado, cada um a SUA senha).
//!   nivel 3 (p12+p12) -> E2E + p12 + p12 (dois certificados, dois cadeados).
//!   nivel 3 Masson  -> E2E + p12 + p12 + id da MACONARIA (sem o id, NAO abre).
//!   respostas assinadas -> a resposta leva assinatura Ed25519; adulterar reprova.
//!
//! Prova real nos DOIS sentidos: cada nivel ABRE com os segredos certos e FALHA
//! com o segredo errado ou faltando — teste que so mostra o caminho feliz nao
//! prova cadeado nenhum. A cripto e a da casa (X25519/HKDF/ChaCha20-Poly1305/
//! PBKDF2/Ed25519), conferida contra vetor em phxsql-core. Zero-deps.
//!
//! Rodar (release por causa do PBKDF2 200k):
//!   cargo run -q --release --example correio-niveis -p phxsql-core

use phxsql_core::{cifra, ed25519, hkdf, x25519, CHAVE_LEN, NONCE_LEN, TAG_LEN};

// Em producao o KDF usa 200_000 iteracoes (docs/SEGURANCA.md). O que esta
// simulacao prova e a LOGICA da escada — qual segredo abre qual camada —, e
// esse numero nao muda a logica; fica igual ao real de proposito.
const ITER: u32 = 200_000;
const INFO: &[u8] = b"phxsql-correio-niveis-v1";

fn rnd16() -> [u8; 16] {
    let mut b = [0u8; 16];
    cifra::sortear(&mut b);
    b
}
fn rnd_nonce() -> [u8; NONCE_LEN] {
    let mut b = [0u8; NONCE_LEN];
    cifra::sortear(&mut b);
    b
}

// -------- selo simetrico (nonce||tag||ct num blob so) --------
fn selar_sim(k: &[u8; CHAVE_LEN], aad: &[u8], claro: &[u8]) -> Vec<u8> {
    let nonce = rnd_nonce();
    let (ct, tag) = cifra::selar(k, &nonce, aad, claro);
    let mut out = Vec::with_capacity(NONCE_LEN + TAG_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&ct);
    out
}
fn abrir_sim(k: &[u8; CHAVE_LEN], aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < NONCE_LEN + TAG_LEN {
        return Err("camada malformada".into());
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&blob[..NONCE_LEN]);
    let mut tag = [0u8; TAG_LEN];
    tag.copy_from_slice(&blob[NONCE_LEN..NONCE_LEN + TAG_LEN]);
    let ct = &blob[NONCE_LEN + TAG_LEN..];
    cifra::abrir(k, &nonce, aad, ct, &tag)
        .map_err(|_| "nao decifra: segredo errado ou dado adulterado".to_string())
}

fn chave_senha(segredo: &str, sal: &[u8; 16]) -> [u8; CHAVE_LEN] {
    cifra::chave_de_senha(segredo, sal, ITER)
}
fn chave_ecdh(
    minha_priv: &[u8; CHAVE_LEN],
    sua_pub: &[u8; CHAVE_LEN],
    sal: &[u8; 16],
) -> Result<[u8; CHAVE_LEN], String> {
    let seg = x25519::segredo(minha_priv, sua_pub).map_err(|e| e.to_string())?;
    let mut k = [0u8; CHAVE_LEN];
    hkdf::derivar(sal, &seg, INFO, &mut k).map_err(|e| e.to_string())?;
    Ok(k)
}

// -------- chave privada guardada cifrada sob uma senha PROPRIA --------
struct SelaSenha {
    sal: [u8; 16],
    nonce: [u8; NONCE_LEN],
    ct: Vec<u8>,
    tag: [u8; TAG_LEN],
    rotulo: String,
}
fn selar_chave(senha: &str, priv_bytes: &[u8; CHAVE_LEN], rotulo: &str) -> SelaSenha {
    let sal = rnd16();
    let k = chave_senha(senha, &sal);
    let nonce = rnd_nonce();
    let (ct, tag) = cifra::selar(&k, &nonce, rotulo.as_bytes(), priv_bytes);
    SelaSenha {
        sal,
        nonce,
        ct,
        tag,
        rotulo: rotulo.into(),
    }
}
fn abrir_chave(s: &SelaSenha, senha: &str) -> Result<[u8; CHAVE_LEN], String> {
    let k = chave_senha(senha, &s.sal);
    let claro = cifra::abrir(&k, &s.nonce, s.rotulo.as_bytes(), &s.ct, &s.tag)
        .map_err(|_| format!("senha incorreta: nao abre {}", s.rotulo))?;
    let mut p = [0u8; CHAVE_LEN];
    p.copy_from_slice(&claro);
    Ok(p)
}

/// Uma pessoa carrega quatro pares de chaves, cada um selado sob a senha certa:
/// identidade (E2E), p12 pessoal, um segundo p12 (corporativo/Masson) e a chave
/// de ASSINATURA Ed25519. A senha destranca so a chave que e dela.
struct Pessoa {
    nome: String,
    id_pub: [u8; CHAVE_LEN],
    id_sel: SelaSenha,
    p12_pub: [u8; CHAVE_LEN],
    p12_sel: SelaSenha,
    p12b_pub: [u8; CHAVE_LEN],
    p12b_sel: SelaSenha,
    sig_pub: [u8; ed25519::CHAVE_LEN],
    sig_sel: SelaSenha,
}
fn criar_pessoa(nome: &str, s_conta: &str, s_p12: &str, s_p12b: &str) -> Pessoa {
    let idp = x25519::gerar_privada();
    let p12p = x25519::gerar_privada();
    let p12bp = x25519::gerar_privada();
    let sigp = ed25519::gerar_privada();
    Pessoa {
        nome: nome.into(),
        id_pub: x25519::chave_publica(&idp),
        id_sel: selar_chave(s_conta, &idp, &format!("{nome}:id")),
        p12_pub: x25519::chave_publica(&p12p),
        p12_sel: selar_chave(s_p12, &p12p, &format!("{nome}:p12")),
        p12b_pub: x25519::chave_publica(&p12bp),
        p12b_sel: selar_chave(s_p12b, &p12bp, &format!("{nome}:p12b")),
        sig_pub: ed25519::chave_publica(&sigp),
        sig_sel: selar_chave(s_conta, &sigp, &format!("{nome}:sig")),
    }
}

/// As senhas que UMA pessoa digita para destrancar as SUAS chaves.
struct Senhas<'a> {
    conta: &'a str,
    p12: &'a str,
    p12b: &'a str,
}
/// Segredos fora-da-banda que a escada pede: a frase (nivel 1) e o id da
/// Maconaria (nivel 3 Masson). Quem nao usa aquele nivel passa None.
struct Segredos<'a> {
    frase: Option<&'a str>,
    masson_id: Option<&'a str>,
}

/// O tipo de cada camada extra (a receita do nivel), SEM o segredo — o segredo
/// entra na hora, e a receita e a mesma no enviar e no ler (uma fonte so).
#[derive(Clone, Copy, PartialEq)]
enum Camada {
    Frase,
    P12,
    P12b,
    Masson,
}
fn receita(nivel: &str) -> Vec<Camada> {
    match nivel {
        "padrao" => vec![],
        "nivel 1" => vec![Camada::Frase],
        "nivel 2 (p12)" => vec![Camada::P12],
        "nivel 3 (p12+p12)" => vec![Camada::P12, Camada::P12b],
        "nivel 3 Masson" => vec![Camada::P12, Camada::P12b, Camada::Masson],
        _ => vec![],
    }
}

/// O cofre que viaja: o nivel fica GRAVADO e entra no AAD de todas as camadas,
/// entao trocar o nivel gravado quebra a leitura (nao da para rebaixar calado).
struct Cofre {
    de: String,
    para: String,
    nivel: String,
    sais: Vec<[u8; 16]>,
    sal_e2e: [u8; 16],
    nonce: [u8; NONCE_LEN],
    ct: Vec<u8>,
    tag: [u8; TAG_LEN],
}
fn ctx(de: &str, para: &str, nivel: &str) -> String {
    format!("{de}|{para}|{nivel}")
}

/// Envia: aplica as camadas extras de DENTRO para fora e o E2E por cima.
fn enviar(
    de: &Pessoa,
    sd: &Senhas,
    para: &Pessoa,
    nivel: &str,
    seg: &Segredos,
    corpo: &[u8],
) -> Result<Cofre, String> {
    let aad = ctx(&de.nome, &para.nome, nivel);
    let mut payload = corpo.to_vec();
    let mut sais = vec![];
    for c in receita(nivel) {
        let sal = rnd16();
        let k: [u8; CHAVE_LEN] = match c {
            Camada::Frase => chave_senha(seg.frase.ok_or("nivel 1 exige a frase")?, &sal),
            Camada::Masson => {
                chave_senha(seg.masson_id.ok_or("Masson exige o id da Maconaria")?, &sal)
            }
            Camada::P12 => chave_ecdh(&abrir_chave(&de.p12_sel, sd.p12)?, &para.p12_pub, &sal)?,
            Camada::P12b => chave_ecdh(&abrir_chave(&de.p12b_sel, sd.p12b)?, &para.p12b_pub, &sal)?,
        };
        payload = selar_sim(&k, aad.as_bytes(), &payload);
        sais.push(sal);
    }
    let sal_e2e = rnd16();
    let ke = chave_ecdh(&abrir_chave(&de.id_sel, sd.conta)?, &para.id_pub, &sal_e2e)?;
    let nonce = rnd_nonce();
    let (ct, tag) = cifra::selar(&ke, &nonce, aad.as_bytes(), &payload);
    Ok(Cofre {
        de: de.nome.clone(),
        para: para.nome.clone(),
        nivel: nivel.into(),
        sais,
        sal_e2e,
        nonce,
        ct,
        tag,
    })
}

/// Le: abre o E2E e desfaz as camadas extras de fora para dentro (ordem inversa
/// da receita). `outro` e a contraparte (o ECDH e simetrico).
fn ler(
    c: &Cofre,
    leitor: &Pessoa,
    sl: &Senhas,
    outro: &Pessoa,
    seg: &Segredos,
) -> Result<Vec<u8>, String> {
    let aad = ctx(&c.de, &c.para, &c.nivel);
    let ke = chave_ecdh(
        &abrir_chave(&leitor.id_sel, sl.conta)?,
        &outro.id_pub,
        &c.sal_e2e,
    )?;
    let mut payload = cifra::abrir(&ke, &c.nonce, aad.as_bytes(), &c.ct, &c.tag)
        .map_err(|_| "E2E nao abre: senha da conta errada, ou nivel/dado adulterado".to_string())?;
    let tipos = receita(&c.nivel);
    for (c_t, sal) in tipos.iter().rev().zip(c.sais.iter().rev()) {
        let k: [u8; CHAVE_LEN] = match c_t {
            Camada::Frase => chave_senha(seg.frase.ok_or("nivel 1 exige a frase")?, sal),
            Camada::Masson => chave_senha(
                seg.masson_id
                    .ok_or("MASSON: necessario o id da Maconaria")?,
                sal,
            ),
            Camada::P12 => chave_ecdh(&abrir_chave(&leitor.p12_sel, sl.p12)?, &outro.p12_pub, sal)?,
            Camada::P12b => chave_ecdh(
                &abrir_chave(&leitor.p12b_sel, sl.p12b)?,
                &outro.p12b_pub,
                sal,
            )?,
        };
        payload = abrir_sim(&k, aad.as_bytes(), &payload)?;
    }
    Ok(payload)
}

// -------- respostas assinadas (Ed25519, autoria e nao-repudio) --------
fn msg_assinada(de: &str, para: &str, corpo: &[u8]) -> Vec<u8> {
    [de.as_bytes(), b"|", para.as_bytes(), b"|", corpo].concat()
}
fn assinar(
    assinante: &Pessoa,
    sa: &Senhas,
    de: &str,
    para: &str,
    corpo: &[u8],
) -> Result<[u8; 64], String> {
    let sp = abrir_chave(&assinante.sig_sel, sa.conta)?;
    Ok(ed25519::assinar(&sp, &msg_assinada(de, para, corpo)))
}
fn conferir(
    assinante_pub: &[u8; ed25519::CHAVE_LEN],
    de: &str,
    para: &str,
    corpo: &[u8],
    sig: &[u8; 64],
) -> bool {
    ed25519::conferir(assinante_pub, &msg_assinada(de, para, corpo), sig)
}

fn contem(agulha: &[u8], palheiro: &[u8]) -> bool {
    palheiro.is_empty() || agulha.windows(palheiro.len()).any(|w| w == palheiro)
}

fn main() {
    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| {
        println!("   {}  {}", if c { " ok  " } else { "FALHA" }, n);
        res.push((c, n.to_string()));
    };

    println!("===== correio PhxSql: SIMULACAO dos niveis de criptografia =====\n");

    // Duas pessoas, cada uma com senha de conta, de p12 e do segundo p12.
    let a = criar_pessoa("adriano", "conta-A-9f", "p12-A-3k", "p12b-A-7q");
    let b = criar_pessoa("juliana", "conta-B-8h", "p12-B-2m", "p12b-B-5w");
    let sa = Senhas {
        conta: "conta-A-9f",
        p12: "p12-A-3k",
        p12b: "p12b-A-7q",
    };
    let sb = Senhas {
        conta: "conta-B-8h",
        p12: "p12-B-2m",
        p12b: "p12b-B-5w",
    };
    let nada = Segredos {
        frase: None,
        masson_id: None,
    };
    const MASSON_ID: &str = "LOJA-ADR-4211";

    // 0) PADRAO — E2E, sempre.
    println!("0) envio e recebimento PADRAO (E2E ECDH das identidades):");
    let corpo0 = b"Fechamento de setembro segue no proximo e-mail.";
    let c0 = enviar(&a, &sa, &b, "padrao", &nada, corpo0).unwrap();
    ok(
        !contem(&c0.ct, corpo0),
        "envio esconde o texto (so ciphertext no fio)",
    );
    let r0 = ler(&c0, &b, &sb, &a, &nada).unwrap();
    ok(
        r0 == corpo0,
        "juliana recebe com a PROPRIA senha + a chave publica do adriano",
    );
    let senha_errada = Senhas {
        conta: "conta-B-ERRADA",
        ..sb_ref(&sb)
    };
    ok(
        ler(&c0, &b, &senha_errada, &a, &nada).is_err(),
        "senha da conta ERRADA nao abre",
    );

    // 1) NIVEL 1 — E2E + frase fora-da-banda.
    println!("\n1) envio e recebimento NIVEL 1 (E2E + FRASE por telefone/SMS):");
    let corpo1 = b"Codigo do cofre: 4471.";
    let frase = "girassol-quarenta-e-dois";
    let seg1 = Segredos {
        frase: Some(frase),
        masson_id: None,
    };
    let c1 = enviar(&a, &sa, &b, "nivel 1", &seg1, corpo1).unwrap();
    ok(!contem(&c1.ct, corpo1), "envio esconde o texto");
    let r1 = ler(&c1, &b, &sb, &a, &seg1).unwrap();
    ok(r1 == corpo1, "recebe com a senha da conta + a frase certa");
    let seg1_errada = Segredos {
        frase: Some("frase-que-nao-e-a-combinada"),
        masson_id: None,
    };
    ok(
        ler(&c1, &b, &sb, &a, &seg1_errada).is_err(),
        "frase ERRADA nao abre (mesmo com a senha certa)",
    );
    ok(
        ler(&c1, &b, &sb, &a, &nada).is_err(),
        "sem a frase nao abre",
    );

    // 2) NIVEL 2 — E2E + p12 (cada um a sua senha de p12).
    println!("\n2) envio e recebimento NIVEL 2 (E2E + p12, ECDH do certificado):");
    let corpo2 = b"Contrato assinado em anexo, confidencial.";
    let c2 = enviar(&a, &sa, &b, "nivel 2 (p12)", &nada, corpo2).unwrap();
    ok(!contem(&c2.ct, corpo2), "envio esconde o texto");
    let r2 = ler(&c2, &b, &sb, &a, &nada).unwrap();
    ok(r2 == corpo2, "recebe com a senha da conta + a senha do p12");
    let sb_p12_errada = Senhas {
        p12: "p12-B-ERRADA",
        ..sb_ref(&sb)
    };
    ok(
        ler(&c2, &b, &sb_p12_errada, &a, &nada).is_err(),
        "senha do p12 ERRADA nao abre",
    );

    // 3) NIVEL 3 — E2E + p12 + p12 (dois certificados, dois cadeados).
    println!("\n3) envio e recebimento NIVEL 3 (E2E + p12 + p12):");
    let corpo3 = b"Chave-mestra do backup: nao repassar.";
    let c3 = enviar(&a, &sa, &b, "nivel 3 (p12+p12)", &nada, corpo3).unwrap();
    ok(!contem(&c3.ct, corpo3), "envio esconde o texto");
    let r3 = ler(&c3, &b, &sb, &a, &nada).unwrap();
    ok(r3 == corpo3, "recebe com a senha da conta + os DOIS p12");
    let sb_sem_p12b = Senhas {
        p12b: "p12b-B-ERRADA",
        ..sb_ref(&sb)
    };
    ok(
        ler(&c3, &b, &sb_sem_p12b, &a, &nada).is_err(),
        "faltando UM dos dois p12 nao abre",
    );

    // 4) NIVEL 3 MASSON — E2E + p12 + p12 + id da Maconaria.
    println!("\n4) envio e recebimento NIVEL 3 MASSON (p12 + p12 + id da Maconaria):");
    let corpo4 = b"Convocacao da Loja, so para iniciados.";
    let seg4 = Segredos {
        frase: None,
        masson_id: Some(MASSON_ID),
    };
    let c4 = enviar(&a, &sa, &b, "nivel 3 Masson", &seg4, corpo4).unwrap();
    ok(!contem(&c4.ct, corpo4), "envio esconde o texto");
    let r4 = ler(&c4, &b, &sb, &a, &seg4).unwrap();
    ok(
        r4 == corpo4,
        "recebe com os dois p12 + o id CERTO da Maconaria",
    );
    let seg4_errada = Segredos {
        frase: None,
        masson_id: Some("LOJA-OUTRA-0000"),
    };
    ok(
        ler(&c4, &b, &sb, &a, &seg4_errada).is_err(),
        "id da Maconaria ERRADO nao abre (necessario o id certo)",
    );
    ok(
        ler(&c4, &b, &sb, &a, &nada).is_err(),
        "SEM o id da Maconaria nao abre — e obrigatorio no Masson",
    );

    // 5) O nivel e AUTENTICADO: rebaixar o cofre gravado quebra a leitura.
    println!("\n5) o nivel entra no AAD — rebaixar o cofre nao cola:");
    let mut c4_rebaixado = enviar(&a, &sa, &b, "nivel 3 Masson", &seg4, corpo4).unwrap();
    c4_rebaixado.nivel = "padrao".into();
    ok(
        ler(&c4_rebaixado, &b, &sb, &a, &seg4).is_err(),
        "trocar o nivel gravado de 'Masson' para 'padrao' quebra o E2E",
    );

    // 6) RESPOSTAS ASSINADAS — Ed25519, autoria e nao-repudio.
    println!("\n6) resposta ASSINADA de juliana para adriano (Ed25519):");
    let resp = b"Recebido e de acordo. Podemos fechar.";
    // a resposta viaja cifrada (padrao) E carrega a assinatura de juliana
    let cr = enviar(&b, &sb, &a, "padrao", &nada, resp).unwrap();
    let sig = assinar(&b, &sb, "juliana", "adriano", resp).unwrap();
    let lido = ler(&cr, &a, &sa, &b, &nada).unwrap();
    ok(lido == resp, "adriano recebe e decifra a resposta");
    ok(
        conferir(&b.sig_pub, "juliana", "adriano", &lido, &sig),
        "a assinatura de juliana CONFERE sobre o texto recebido",
    );
    let adulterado = b"Recebido e de acordo. Podemos fechar por 1 milhao.";
    ok(
        !conferir(&b.sig_pub, "juliana", "adriano", adulterado, &sig),
        "texto ADULTERADO reprova a assinatura",
    );
    // um forjador sem a chave de assinatura de juliana nao produz assinatura valida
    let falsario = criar_pessoa("falsario", "x", "y", "z");
    let sig_falsa = ed25519::assinar(
        &abrir_chave(&falsario.sig_sel, "x").unwrap(),
        &msg_assinada("juliana", "adriano", resp),
    );
    ok(
        !conferir(&b.sig_pub, "juliana", "adriano", resp, &sig_falsa),
        "assinatura de OUTRA chave (falsario) nao passa como sendo de juliana",
    );

    // ---- resultado ----
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
    if falhas != 0 {
        std::process::exit(1);
    }
}

// Ajudante: as senhas sao emprestimos (&str), entao "copiar com um campo
// trocado" (`..`) precisa de uma copia rasa. Como sao referencias, isto e
// barato e nao duplica segredo nenhum.
fn sb_ref<'a>(s: &Senhas<'a>) -> Senhas<'a> {
    Senhas {
        conta: s.conta,
        p12: s.p12,
        p12b: s.p12b,
    }
}
