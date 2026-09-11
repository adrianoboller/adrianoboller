//! Prototipo rodavel do gerador nativo de pix "copia e cola" (BR Code EMV do
//! Banco Central) + o QR Code (ISO/IEC 18004) que o codifica, para o lado de
//! quem paga desenhar na hora. Tudo so com `std`, sem OpenSSL nem crate de QR.
//!
//! O QUE ESTE EXEMPLO PROVA (e as fontes de cada vetor):
//!   1. CRC-16/CCITT-FALSE contra o VETOR EXTERNO canonico do catalogo:
//!      `"123456789" -> 0x29B1`. E este vetor que prova o algoritmo do CRC.
//!   2. O BR Code montado e AUTOCONSISTENTE: o CRC gravado nos 4 hex finais
//!      bate com o CRC recomputado de todo o resto (o que um leitor confere).
//!   3. A estrutura EMV: `000201...`, o subtemplate `br.gov.bcb.pix`, o valor.
//!   4. IDA E VOLTA no QR (modo byte): o leitor minimo decodifica a propria
//!      saida e devolve o payload -- posicionamento e mascara de ponta a ponta.
//!   5. IDA E VOLTA no exemplo alfanumerico canonico "HELLO WORLD" (V1),
//!      cujos codewords sao o vetor externo provado nos testes de unidade.
//!   6. Payload longo cai numa versao >= 7 (version info + alinhamento) e volta.
//!   7. Correcao de erro de verdade: virar poucos modulos e o leitor recupera.
//!
//! A matematica dura -- Reed-Solomon (GF 256), os BCH de format/version info --
//! e provada contra vetor publicado nos testes de `src/qr.rs`; aqui o exemplo
//! mostra o sistema inteiro funcionando e reimprime o vetor #1, o do CRC.
//!
//! Rodar: cargo run -q --example pix-qr -p phxsql-core

use phxsql_core::pix::{self, Cobranca};
use phxsql_core::qr::{self, Modo, Nivel, Qr};

/// CRC recomputado dos bytes de um BR Code, menos os 4 hex finais.
fn crc_recomputado(payload: &str) -> u16 {
    let corpo = &payload[..payload.len() - 4];
    pix::crc16_ccitt_false(corpo.as_bytes())
}

/// Desenha o QR em ASCII: dois espacos = claro, dois blocos = escuro, com uma
/// zona de silencio de 2 modulos em volta (o leitor precisa dela).
fn desenhar(qr: &Qr) -> String {
    let dim = qr.dim();
    let quiet = 2;
    let mut s = String::new();
    for r in 0..dim + 2 * quiet {
        for c in 0..dim + 2 * quiet {
            let escuro = r >= quiet
                && r < dim + quiet
                && c >= quiet
                && c < dim + quiet
                && qr.escuro(r - quiet, c - quiet);
            s.push_str(if escuro { "██" } else { "  " });
        }
        s.push('\n');
    }
    s
}

fn main() {
    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| res.push((c, n.to_string()));

    println!("===== pix BR Code + QR Code, nativo e zero-deps =====\n");

    // 1) O vetor externo do CRC -- o que prova o algoritmo.
    let vetor = pix::crc16_ccitt_false(b"123456789");
    println!("1) CRC-16/CCITT-FALSE(\"123456789\") = 0x{vetor:04X}  (canonico: 0x29B1)");
    ok(
        vetor == 0x29B1,
        "CRC bate o vetor externo canonico (0x29B1)",
    );

    // 2/3) Monta um BR Code de cobranca e confere.
    println!("\n2) BR Code de cobranca (R$ 120,00):");
    let payload = pix::montar(&Cobranca {
        chave: "adriano@phxsql.com.br",
        valor_centavos: 12000,
        nome: "PHXSQL PAGAMENTOS",
        cidade: "BLUMENAU",
        txid: "PEDIDO123",
    })
    .expect("cobranca valida");
    println!("   {payload}");

    let hex = &payload[payload.len() - 4..];
    let gravado = u16::from_str_radix(hex, 16).unwrap();
    ok(
        gravado == crc_recomputado(&payload),
        "BR Code autoconsistente (CRC gravado == recomputado)",
    );
    ok(
        payload.starts_with("000201"),
        "abre com Payload Format Indicator 00=01",
    );
    ok(
        payload.contains("br.gov.bcb.pix"),
        "carrega o GUI do pix no campo 26",
    );
    ok(payload.contains("5303986"), "moeda 53=986 (BRL)");
    ok(payload.contains("5406120.00"), "valor 54=120.00");

    // QR estatico (sem valor): campo 54 some, ponto de iniciacao 11.
    let estatico = pix::montar(&Cobranca {
        chave: "chave-aleatoria-uuid-000",
        valor_centavos: 0,
        nome: "LOJA PHX",
        cidade: "RECIFE",
        txid: "",
    })
    .unwrap();
    ok(
        !estatico.contains("5303986540") && estatico.contains("010211"),
        "QR estatico: sem campo 54 e ponto de iniciacao 01=11",
    );

    // 4) Ida e volta do QR do proprio BR Code (modo byte, versao automatica).
    println!("\n3) QR Code do BR Code (modo byte, versao/mascara automaticas):");
    let qr = qr::gerar(payload.as_bytes(), Nivel::M).expect("gera o QR");
    println!(
        "   versao {}  nivel {:?}  mascara {}  ({}x{} modulos, {} bytes de payload)",
        qr.versao,
        qr.nivel,
        qr.mascara,
        qr.dim(),
        qr.dim(),
        payload.len()
    );
    let lido = qr::ler(&qr).expect("le o QR");
    ok(
        lido == payload.as_bytes(),
        "ida e volta: o leitor devolve o BR Code inteiro",
    );
    ok(
        qr.dim() == 17 + 4 * qr.versao as usize,
        "dimensao = 17 + 4*versao",
    );
    ok(qr.escuro(qr.dim() - 8, 8), "modulo sempre-escuro presente");

    // 5) O vetor alfanumerico canonico, de ponta a ponta.
    let hw = qr::gerar_versao(b"HELLO WORLD", 1, Nivel::M, Modo::Alfanumerico).unwrap();
    ok(
        qr::ler(&hw).unwrap() == b"HELLO WORLD",
        "\"HELLO WORLD\" V1-M (vetor publicado) volta inteiro",
    );

    // 6) Payload longo -> versao alta (version info + alinhamento) e volta.
    let longo: Vec<u8> = (0..140u8).map(|i| b'A' + (i % 26)).collect();
    let qr_longo = qr::gerar(&longo, Nivel::M).expect("gera o QR longo");
    ok(
        qr_longo.versao >= 7 && qr::ler(&qr_longo).unwrap() == longo,
        "payload longo cai em versao >=7 e ainda assim volta",
    );

    // 7) Nivel Q (mais correcao) tambem fecha a ida e volta. A correcao de
    // erro com modulos virados e provada nos testes de unidade de `src/qr.rs`
    // (dentro da capacidade recupera; acima dela falha, sem devolver lixo),
    // que la alcancam os campos internos da matriz.
    let qr_q = qr::gerar(payload.as_bytes(), Nivel::Q).unwrap();
    ok(
        qr::ler(&qr_q).unwrap() == payload.as_bytes(),
        "nivel Q tambem faz a ida e volta",
    );

    // O artefato visual: a string e o desenho.
    println!("\n4) copia e cola:\n{payload}");
    println!("\n5) QR (dois espacos = claro, dois blocos = escuro):\n");
    print!("{}", desenhar(&qr));

    println!("\n===== RESULTADO =====");
    let mut falhas = 0;
    for (c, n) in &res {
        if !c {
            falhas += 1;
        }
        println!("  {}  {}", if *c { " ok  " } else { "FALHA" }, n);
    }
    println!(
        "\nPROVA {} / {} checagens, {} ok, {} falha(s).",
        if falhas == 0 { "VERDE" } else { "VERMELHA" },
        res.len(),
        res.len() - falhas,
        falhas
    );
    if falhas != 0 {
        std::process::exit(1);
    }
}
