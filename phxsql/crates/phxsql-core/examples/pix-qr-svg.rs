//! Emite o QR de uma cobranca pix como SVG, para a maquete do correio embutir
//! um QR DE VERDADE (gerado pelo motor), nao um desenho. Zero-deps.
//!
//! Uso: cargo run -q --example pix-qr-svg -p phxsql-core -- <chave> <centavos> <nome> <cidade> <txid>
//! Sem argumentos, usa um exemplo. Escreve o SVG no stdout.

use phxsql_core::pix::{montar, Cobranca};
use phxsql_core::qr::{gerar, Nivel};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let chave = a
        .first()
        .map(String::as_str)
        .unwrap_or("juliana@pix.com.br");
    let centavos: u64 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(12_000);
    let nome = a.get(2).map(String::as_str).unwrap_or("JULIANA PRADO");
    let cidade = a.get(3).map(String::as_str).unwrap_or("BLUMENAU");
    let txid = a.get(4).map(String::as_str).unwrap_or("CONF-ADR-JUL-0001");

    let cob = Cobranca {
        chave,
        valor_centavos: centavos,
        nome,
        cidade,
        txid,
    };
    let br = montar(&cob).expect("BR Code");
    let qr = gerar(br.as_bytes(), Nivel::M).expect("QR");

    let dim = qr.dim();
    let quiet = 4; // zona de silencio obrigatoria (4 modulos)
    let escala = 8; // px por modulo
    let lado = (dim + 2 * quiet) * escala;

    // Fundo claro, modulos escuros. currentColor nao serve num QR: leitor precisa
    // de preto sobre branco com contraste real, entao a cor e fixa de proposito.
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{lado}\" height=\"{lado}\" \
         viewBox=\"0 0 {lado} {lado}\" role=\"img\" aria-label=\"QR do pix\">"
    ));
    svg.push_str(&format!(
        "<rect width=\"{lado}\" height=\"{lado}\" fill=\"#ffffff\"/>"
    ));
    for r in 0..dim {
        for c in 0..dim {
            if qr.escuro(r, c) {
                let x = (c + quiet) * escala;
                let y = (r + quiet) * escala;
                svg.push_str(&format!(
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{escala}\" height=\"{escala}\" fill=\"#010418\"/>"
                ));
            }
        }
    }
    svg.push_str("</svg>");

    eprintln!(
        "BR Code ({} bytes), QR v{} {}x{}:",
        br.len(),
        qr.versao,
        dim,
        dim
    );
    eprintln!("{br}");
    println!("{svg}");
}
