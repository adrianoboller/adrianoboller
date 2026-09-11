//! Simulacao do MASSON no correio PhxSql, com as duas regras novas do dono
//! (11/09/2026):
//!
//!   1. VALIDAR se o usuario e mesmo Masson. Nao basta declarar o id da Loja
//!      (aquilo e so o segredo que cifra a 3a camada, ver correio-niveis). A
//!      LOJA assina uma CREDENCIAL (Ed25519) dizendo "fulano e membro"; o
//!      correio confere essa assinatura contra a chave publica CONHECIDA da
//!      Loja. Quem so declara o id, ou traz credencial assinada por outro, NAO
//!      passa — a mesma logica do certificado, provada contra vetor.
//!
//!   2. No caso da Maconaria, o pagamento faz um SPLIT para a Loja desejada:
//!      parte vai para a pessoa, parte para a Loja. A conta e em CENTAVOS
//!      INTEIROS, nunca em float — dinheiro nao pode ser criado nem sumir num
//!      arredondamento (garantia de dado, papel do DBA). pessoa + loja == total,
//!      exato, sempre.
//!
//! Prova real nos DOIS sentidos: credencial da Loja certa VALIDA e a forjada
//! (ou adulterada, ou de outra Loja) NAO valida; e um nao-validado nao recebe
//! o split (nao entra como Masson). Zero-deps, so a cripto da casa.
//!
//! Rodar: cargo run -q --example correio-masson -p phxsql-core

use phxsql_core::ed25519;

// A credencial que a Loja assina. O texto e canonico e explicito: quem, de
// qual Loja, ate quando. Conferir por CHAVE (a assinatura), nunca por comparar
// a frase — se um dia mudar a redacao, a assinatura continua sendo a verdade.
fn texto_credencial(user: &str, loja: &str, ate: &str) -> Vec<u8> {
    format!("phxmail-masson-v1|membro:{user}|loja:{loja}|ate:{ate}").into_bytes()
}

/// Uma Loja maconica: um par de chaves Ed25519 (a publica e PUBLICADA, a
/// privada so a Loja tem) e a chave pix onde ela recebe a parte do split.
struct Loja {
    id: String,
    sig_pub: [u8; ed25519::CHAVE_LEN],
    sig_priv: [u8; ed25519::CHAVE_LEN],
    chave_pix: String,
}
fn criar_loja(id: &str, chave_pix: &str) -> Loja {
    let priv_ = ed25519::gerar_privada();
    Loja {
        id: id.into(),
        sig_pub: ed25519::chave_publica(&priv_),
        sig_priv: priv_,
        chave_pix: chave_pix.into(),
    }
}

/// A credencial de membro emitida pela Loja.
struct Credencial {
    user: String,
    loja: String,
    ate: String,
    assinatura: [u8; ed25519::ASSINATURA_LEN],
}
fn emitir_credencial(loja: &Loja, user: &str, ate: &str) -> Credencial {
    let msg = texto_credencial(user, &loja.id, ate);
    Credencial {
        user: user.into(),
        loja: loja.id.clone(),
        ate: ate.into(),
        assinatura: ed25519::assinar(&loja.sig_priv, &msg),
    }
}

/// A validacao: confere a assinatura da credencial contra a chave publica
/// CONHECIDA da Loja (a que o correio ja tem cadastrada). Se a credencial diz
/// ser de uma Loja e a chave publica e de outra, ou se foi assinada por
/// qualquer chave que nao a da Loja, reprova.
fn validar_masson(loja_id: &str, loja_pub: &[u8; ed25519::CHAVE_LEN], cred: &Credencial) -> bool {
    if cred.loja != loja_id {
        return false;
    }
    let msg = texto_credencial(&cred.user, &cred.loja, &cred.ate);
    ed25519::conferir(loja_pub, &msg, &cred.assinatura)
}

/// Split em CENTAVOS inteiros. A taxa da Loja vem em pontos-base (bps: 1000 =
/// 10%). A parte da Loja e o piso da divisao e a da pessoa e o RESTO — assim
/// pessoa + loja == total sempre, sem centavo criado nem perdido. Devolver o
/// resto para a pessoa (e nao para a Loja) e decisao: quem paga nunca paga a
/// mais por um arredondamento.
fn split_pagamento(total_centavos: u64, taxa_loja_bps: u64) -> (u64, u64) {
    let loja = total_centavos * taxa_loja_bps / 10_000;
    let pessoa = total_centavos - loja;
    (pessoa, loja)
}

/// Aceitar uma relacao MASSON: so entra quem a Loja validou. O retorno e a
/// lista de destinos do pagamento (pessoa e Loja), em centavos.
fn aceitar_masson(
    loja: &Loja,
    cred: &Credencial,
    chave_pix_pessoa: &str,
    total_centavos: u64,
    taxa_loja_bps: u64,
) -> Result<Vec<(String, u64)>, String> {
    if !validar_masson(&loja.id, &loja.sig_pub, cred) {
        return Err("RECUSADO: credencial da Loja nao confere — nao e Masson validado".into());
    }
    let (pessoa, parte_loja) = split_pagamento(total_centavos, taxa_loja_bps);
    Ok(vec![
        (chave_pix_pessoa.to_string(), pessoa),
        (loja.chave_pix.clone(), parte_loja),
    ])
}

fn reais(centavos: u64) -> String {
    format!("R$ {},{:02}", centavos / 100, centavos % 100)
}

fn main() {
    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| {
        println!("   {}  {}", if c { " ok  " } else { "FALHA" }, n);
        res.push((c, n.to_string()));
    };

    println!("===== correio PhxSql: MASSON — validacao da Loja e split do pagamento =====\n");

    // A Loja publica a sua chave; emite a credencial do adriano.
    let loja = criar_loja("LOJA-ADR-4211", "loja4211@pix.com.br");
    let cred = emitir_credencial(&loja, "adriano", "2027-12-31");

    println!("1) validar se e MESMO Masson (credencial assinada pela Loja):");
    ok(
        validar_masson(&loja.id, &loja.sig_pub, &cred),
        "credencial assinada pela Loja CERTA valida o membro",
    );

    // Um impostor gera o PROPRIO par e assina uma credencial dizendo que o
    // adriano e membro. Contra a chave publica da Loja, nao cola.
    let impostor = criar_loja("LOJA-ADR-4211", "impostor@pix.com.br");
    let cred_forjada = emitir_credencial(&impostor, "adriano", "2027-12-31");
    ok(
        !validar_masson(&loja.id, &loja.sig_pub, &cred_forjada),
        "credencial FORJADA (assinada por outra chave) NAO valida",
    );

    // Declarar o id sem credencial assinada: nao ha o que conferir -> reprova.
    let so_declarou = Credencial {
        user: "adriano".into(),
        loja: "LOJA-ADR-4211".into(),
        ate: "2027-12-31".into(),
        assinatura: [0u8; ed25519::ASSINATURA_LEN],
    };
    ok(
        !validar_masson(&loja.id, &loja.sig_pub, &so_declarou),
        "so DECLARAR o id (sem credencial da Loja) NAO valida",
    );

    // Credencial legitima, mas de OUTRA Loja: nao serve para esta.
    let outra = criar_loja("LOJA-OUTRA-9000", "outra@pix.com.br");
    let cred_outra = emitir_credencial(&outra, "adriano", "2027-12-31");
    ok(
        !validar_masson(&loja.id, &loja.sig_pub, &cred_outra),
        "credencial de OUTRA Loja nao vale para esta",
    );

    // Adulterar o texto (esticar a validade) quebra a assinatura.
    let mut cred_adulterada = emitir_credencial(&loja, "adriano", "2027-12-31");
    cred_adulterada.ate = "2099-12-31".into();
    ok(
        !validar_masson(&loja.id, &loja.sig_pub, &cred_adulterada),
        "adulterar a validade da credencial reprova a assinatura",
    );

    println!("\n2) split do pagamento para a Loja (centavos inteiros):");
    // R$ 1.000,00, taxa da Loja 10%.
    let (pessoa, parte_loja) = split_pagamento(100_000, 1_000);
    println!(
        "   total {} -> pessoa {} + Loja {}",
        reais(100_000),
        reais(pessoa),
        reais(parte_loja)
    );
    ok(
        pessoa == 90_000 && parte_loja == 10_000,
        "10% de R$ 1.000,00: pessoa R$ 900,00, Loja R$ 100,00",
    );
    ok(
        pessoa + parte_loja == 100_000,
        "pessoa + Loja == total (nada criado nem perdido)",
    );

    // Valor que NAO divide redondo: R$ 33,33, taxa 10%.
    let (p2, l2) = split_pagamento(3_333, 1_000);
    println!(
        "   total {} -> pessoa {} + Loja {}",
        reais(3_333),
        reais(p2),
        reais(l2)
    );
    ok(
        p2 + l2 == 3_333 && l2 == 333 && p2 == 3_000,
        "R$ 33,33 a 10%: soma EXATA (o resto fica com quem paga, nao com a Loja)",
    );

    println!("\n3) so entra como Masson quem a Loja validou:");
    // Com credencial valida: aceita e devolve o split.
    let destinos = aceitar_masson(&loja, &cred, "adriano@pix.com.br", 50_000, 1_500).unwrap();
    let soma: u64 = destinos.iter().map(|(_, v)| v).sum();
    let loja_recebe = destinos
        .iter()
        .any(|(chave, v)| chave == "loja4211@pix.com.br" && *v == 7_500);
    ok(
        destinos.len() == 2 && soma == 50_000 && loja_recebe,
        "Masson validado: split em 2 destinos, Loja recebe os 15% (R$ 75,00 de R$ 500,00)",
    );
    // Com credencial forjada: nem aceita, nem gera split.
    ok(
        aceitar_masson(&loja, &cred_forjada, "adriano@pix.com.br", 50_000, 1_500).is_err(),
        "credencial forjada: NAO aceita e NAO gera pagamento nenhum",
    );

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
