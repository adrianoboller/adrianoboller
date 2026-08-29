//! Quanto custa cifrar, e quanto isso pesa numa insercao e numa leitura.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store   # o binario do medidor
//! cargo run --release --example custo-da-cifra -- [linhas]
//! ```
//!
//! Existe por causa da regra da casa: *receita de fora se mede contra o nosso
//! gargalo antes de virar plano*. Antes de desenhar a cifra dos arquivos de
//! dados, o numero que decide o desenho e um so -- **quantos MB/s o
//! ChaCha20-Poly1305 que ja esta escrito aqui faz nesta maquina**, e quanto
//! disso cabe dentro de uma insercao.
//!
//! Ele mede quatro coisas, nesta ordem:
//!
//! 1. a vazao do AEAD em quatro tamanhos, do payload de um slot ate um bloco
//!    de `.memo`;
//! 2. o custo por operacao nesses mesmos tamanhos -- que e o numero que se
//!    compara com os 7,5 us da insercao;
//! 3. as parcelas do AEAD separadas (so ChaCha20, so Poly1305), para saber se
//!    ha o que otimizar;
//! 4. o PBKDF2 de 210.000 iteracoes, que e o custo de ABRIR e que explica por
//!    que o cofre guarda a chave derivada.
//!
//! A ultima linha e `RESULTADO <json>`, como na `carga`.

use std::time::Instant;

use phxsql_core::cifra::{self, CHAVE_LEN, NONCE_LEN, TAG_LEN};
use phxsql_core::frogcript;

/// Tamanhos que importam aqui, e o que cada um representa no disco.
const CASOS: [(usize, &str); 6] = [
    (64, "payload de slot curto (.reg)"),
    (128, "payload de slot medio (.reg)"),
    (256, "payload de slot largo (.reg)"),
    (4096, "pagina do .ndx"),
    (8192, "bloco tipico de .memo"),
    (65536, "bloco grande de .bin"),
];

fn mbs(bytes: u64, segundos: f64) -> f64 {
    (bytes as f64 / 1_048_576.0) / segundos
}

/// Repete ate passar de meio segundo, para o relogio nao ser o ruido.
fn medir(tamanho: usize, minimo: f64, mut passo: impl FnMut(&mut [u8])) -> (f64, u64) {
    let mut dados = vec![0u8; tamanho];
    for (i, b) in dados.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let mut voltas = 0u64;
    let inicio = Instant::now();
    loop {
        for _ in 0..64 {
            passo(&mut dados);
        }
        voltas += 64;
        if inicio.elapsed().as_secs_f64() >= minimo {
            break;
        }
    }
    (inicio.elapsed().as_secs_f64(), voltas)
}

fn main() {
    let chave = [7u8; CHAVE_LEN];
    let nonce = [3u8; NONCE_LEN];
    let aad = [0u8; 24];

    println!("Custo do ChaCha20-Poly1305 que ja esta escrito (RFC 8439)");
    println!();
    println!(
        "{:>8}  {:>10}  {:>10}  {:>10}  o que e",
        "bytes", "selar us", "abrir us", "MB/s"
    );
    println!("{}", "-".repeat(72));

    let mut json = String::from("{\"aead\":[");
    for (n, (tamanho, rotulo)) in CASOS.iter().enumerate() {
        // Selar: o caminho da ESCRITA.
        let (s, v) = medir(*tamanho, 0.4, |d| {
            let (c, t) = cifra::selar(&chave, &nonce, &aad, d);
            std::hint::black_box((c, t));
        });
        let us_selar = s / v as f64 * 1e6;
        let vazao = mbs(*tamanho as u64 * v, s);

        // Abrir: o caminho da LEITURA. Sela uma vez fora do laco.
        let claro = vec![0xABu8; *tamanho];
        let (cifrado, tag) = cifra::selar(&chave, &nonce, &aad, &claro);
        let (s2, v2) = medir(1, 0.4, |_| {
            let r = cifra::abrir(&chave, &nonce, &aad, &cifrado, &tag);
            std::hint::black_box(r.is_ok());
        });
        let us_abrir = s2 / v2 as f64 * 1e6;

        println!(
            "{:>8}  {:>10.3}  {:>10.3}  {:>10.1}  {}",
            tamanho, us_selar, us_abrir, vazao, rotulo
        );
        if n > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"bytes\":{tamanho},\"selar_us\":{us_selar:.4},\
             \"abrir_us\":{us_abrir:.4},\"mbs\":{vazao:.1}}}"
        ));
    }
    json.push(']');

    // As duas parcelas separadas, para saber onde o tempo esta.
    println!();
    println!("As duas parcelas, num payload de 128 bytes e numa pagina de 4096:");
    for tamanho in [128usize, 4096] {
        let (s, v) = medir(tamanho, 0.4, |d| {
            cifra::chacha20(&chave, 1, &nonce, d);
            std::hint::black_box(&d[0]);
        });
        let so_cifra = s / v as f64 * 1e6;
        let (s2, v2) = medir(tamanho, 0.4, |d| {
            std::hint::black_box(cifra::poly1305(&chave, d));
        });
        let so_mac = s2 / v2 as f64 * 1e6;
        println!(
            "  {tamanho:>5} bytes: ChaCha20 {so_cifra:.3} us  +  Poly1305 {so_mac:.3} us  \
             = {:.3} us de conta pura",
            so_cifra + so_mac
        );
        json.push_str(&format!(
            ",\"parcelas_{tamanho}\":{{\"chacha_us\":{so_cifra:.4},\"poly_us\":{so_mac:.4}}}"
        ));
    }

    // O custo de ABRIR o arquivo: a derivacao da chave.
    println!();
    let sal = [9u8; 16];
    let inicio = Instant::now();
    let k = cifra::chave_de_senha("uma senha qualquer", &sal, 210_000);
    let ms_pbkdf2 = inicio.elapsed().as_secs_f64() * 1000.0;
    std::hint::black_box(k);
    println!("PBKDF2-SHA256 de 210.000 iteracoes: {ms_pbkdf2:.1} ms");
    println!(
        "  -- e por isso que o cofre guarda a chave derivada: sem cache, cada \
         abertura de tabela pagaria isto."
    );
    json.push_str(&format!(",\"pbkdf2_210k_ms\":{ms_pbkdf2:.1}"));

    // O que isso significa numa insercao, com os numeros do DESEMPENHO.md.
    println!();
    println!("O que cabe numa insercao de ~7,5 us (o numero medido em DESEMPENHO.md):");
    let (s, v) = medir(128, 0.4, |d| {
        let (c, t) = cifra::selar(&chave, &nonce, &aad, d);
        std::hint::black_box((c, t));
    });
    let us128 = s / v as f64 * 1e6;
    println!(
        "  um slot de 128 bytes selado custa {us128:.3} us = {:.1}% de uma insercao",
        us128 / 7.5 * 100.0
    );
    let (s, v) = medir(4096, 0.4, |d| {
        let (c, t) = cifra::selar(&chave, &nonce, &aad, d);
        std::hint::black_box((c, t));
    });
    let us4k = s / v as f64 * 1e6;
    println!(
        "  uma pagina de 4096 bytes selada custa {us4k:.3} us; a insercao toca \
         10,86 paginas por linha (medido), logo {:.1} us por linha se toda \
         pagina fosse cifrada a cada toque",
        us4k * 10.86
    );
    println!(
        "  a etiqueta cobra {TAG_LEN} bytes por pedaco cifrado: num slot de 128 \
         bytes sao {:.1}% de disco a mais",
        TAG_LEN as f64 / 128.0 * 100.0
    );
    json.push_str(&format!(
        ",\"slot128_pct_da_insercao\":{:.2},\"pagina4k_por_linha_us\":{:.2}",
        us128 / 7.5 * 100.0,
        us4k * 10.86
    ));

    // ------------------------------------------------------------ FrogCript
    println!();
    println!("O modo FrogCript, medido:");
    let ajuste = frogcript::Ajuste::default();
    let texto = "Fulano de Tal da Silva";
    let (s, v) = medir(1, 0.4, |_| {
        let p = frogcript::cifrar(&chave, texto, frogcript::Direcao::Direta, ajuste);
        std::hint::black_box(p.is_ok());
    });
    let frog_us = s / v as f64 * 1e6;
    let pacote = frogcript::cifrar(&chave, texto, frogcript::Direcao::Direta, ajuste).unwrap();
    println!(
        "  como esta escrito aqui (4 selagens ChaCha20-Poly1305, sem PBKDF2 por \
         valor): {frog_us:.3} us, e {} bytes para {} de texto",
        pacote.len(),
        texto.len()
    );
    println!(
        "  o AEAD direto, para o mesmo texto: {:.3} us e {} bytes",
        us128 * texto.len() as f64 / 128.0,
        texto.len() + TAG_LEN
    );

    // O FrogCript DE REFERENCIA deriva a chave por PBKDF2 de 200.000 iteracoes
    // a CADA selagem, com sal proprio -- sao quatro por cifragem. O numero sai
    // do PBKDF2 ja medido, e nao de um palpite.
    let pbkdf2_200k = ms_pbkdf2 * 200_000.0 / 210_000.0;
    println!(
        "  o frogcript.py de referencia deriva a chave 4 vezes por valor \
         (PBKDF2 de 200.000): {:.0} ms por valor, {:.0}x o que esta aqui",
        pbkdf2_200k * 4.0,
        pbkdf2_200k * 4.0 * 1000.0 / frog_us
    );
    // O tamanho dele: Base64 em duas camadas, sal de 16 e nonce de 12 por
    // selagem. A conta esta em SEGURANCA.md §10.4.
    let n = texto.len() as f64;
    let de_referencia = 256.0 * n / 81.0 + 327.0;
    println!(
        "  e o pacote dele sai em ~{:.0} bytes ({:.1}x o texto), contra {} aqui",
        de_referencia,
        de_referencia / n,
        pacote.len()
    );
    json.push_str(&format!(
        ",\"frogcript\":{{\"us\":{frog_us:.3},\"bytes\":{},\"texto\":{},\
         \"referencia_ms\":{:.0},\"referencia_bytes\":{:.0}}}",
        pacote.len(),
        texto.len(),
        pbkdf2_200k * 4.0,
        de_referencia
    ));

    println!();
    println!("RESULTADO {json}}}");
}
