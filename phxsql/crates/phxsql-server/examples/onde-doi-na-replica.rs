//! A replica aplica 4.357 eventos/s contra 28.914 linhas/s do master. Onde?
//!
//! ```bash
//! cargo run --release --example onde-doi-na-replica -- [eventos]
//! ```
//!
//! 4.357/s sao **229 us por evento**, e uma insercao local completa com dois
//! indices custa 15,9 us. O buraco e de 14x, e a explicacao que estava escrita
//! -- «aplicar decodifica a imagem para `Value` e reencoda o payload» -- nao
//! cabe nele: decodificar e reencodar sao parte dos 15,9, e nem todos eles.
//!
//! Este medidor separa as quatro coisas que acontecem por evento, sem rede no
//! meio, para a rede nao esconder o resto:
//!
//! - o **hexadecimal** da imagem, nos dois sentidos -- ela viaja como texto;
//! - o **JSON** do lote, montado no source e analisado na replica;
//! - o `aplicar_evento`, que decodifica a imagem e insere;
//! - uma insercao local pura, como piso de comparacao.

use std::time::Instant;

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::valores::{bytes_para_hex, hex_para_bytes};
use phxsql_store::log::Operacao;
use phxsql_store::table::Table;

const CIDADES: [&str; 8] = [
    "Blumenau",
    "Joinville",
    "Itajai",
    "Curitiba",
    "Chapeco",
    "Lages",
    "Florianopolis",
    "Criciuma",
];

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Cliente {i:08}")),
        Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
    ]
}

fn dir_limpo(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-repl-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn us(d: f64, n: usize) -> f64 {
    d * 1e6 / n as f64
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);

    // ------------------------------------------------- o source, e as imagens
    let dir = dir_limpo("source");
    let mut fonte = Table::criar(&dir, esquema()).unwrap();
    fonte.ligar_imagem_no_diario(true);
    for i in 1..=n as i64 {
        fonte.inserir(&linha(i)).unwrap();
    }
    fonte.sincronizar().unwrap();

    let eventos = fonte.diario_com_imagem(0, n as u64).unwrap();
    assert_eq!(eventos.len(), n);
    let bytes_imagem: usize = eventos.iter().map(|(_, im)| im.len()).sum();

    println!(
        "=== {n} eventos, imagem media de {} bytes ===\n",
        bytes_imagem / n
    );

    // ---------------------------------------------------------- 1. o hex, ida
    let inicio = Instant::now();
    let hexes: Vec<String> = eventos.iter().map(|(_, im)| bytes_para_hex(im)).collect();
    let hex_ida = inicio.elapsed().as_secs_f64();

    // -------------------------------------------------------- 2. o hex, volta
    let inicio = Instant::now();
    let mut voltou = 0usize;
    for h in &hexes {
        voltou += hex_para_bytes(h).unwrap().len();
    }
    let hex_volta = inicio.elapsed().as_secs_f64();
    assert_eq!(voltou, bytes_imagem);

    // ------------------------------------ 3. o JSON do lote, montar e analisar
    // Do jeito que `op_replicar` monta: um objeto por evento, em lotes de 500.
    const LOTE: usize = 500;
    let inicio = Instant::now();
    let mut textos: Vec<String> = Vec::new();
    for (bloco, pedaco) in eventos.chunks(LOTE).enumerate() {
        let lista: Vec<Json> = pedaco
            .iter()
            .enumerate()
            .map(|(i, (e, _))| {
                Json::objeto(vec![
                    ("operacao", Json::texto_de(e.operacao.nome())),
                    ("rowid", Json::de_u64(e.rowid)),
                    ("versao", Json::de_u64(e.versao)),
                    ("carimbo_ms", Json::Numero(e.carimbo as f64)),
                    ("usuario", Json::de_u64(e.usuario as u64)),
                    ("imagem", Json::texto_de(hexes[bloco * LOTE + i].clone())),
                ])
            })
            .collect();
        textos.push(Json::objeto(vec![("eventos", Json::Lista(lista))]).escrever());
    }
    let json_montar = inicio.elapsed().as_secs_f64();
    let bytes_json: usize = textos.iter().map(|t| t.len()).sum();

    let inicio = Instant::now();
    let mut lidos = 0usize;
    for t in &textos {
        let j = Json::analisar(t).unwrap();
        lidos += j.campo("eventos").and_then(Json::lista).unwrap().len();
    }
    let json_analisar = inicio.elapsed().as_secs_f64();
    assert_eq!(lidos, n);

    // ---------------------------------------------------- 4. o aplicar_evento
    let dir_r = dir_limpo("replica");
    let mut replica = Table::criar(&dir_r, esquema()).unwrap();
    let inicio = Instant::now();
    for (e, imagem) in &eventos {
        replica
            .aplicar_evento(Operacao::Inclusao, e.rowid, imagem)
            .unwrap();
    }
    replica.sincronizar().unwrap();
    let aplicar = inicio.elapsed().as_secs_f64();

    // --------------------------------------------- 5. o piso: insercao local
    let dir_l = dir_limpo("local");
    let mut local = Table::criar(&dir_l, esquema()).unwrap();
    let inicio = Instant::now();
    for i in 1..=n as i64 {
        local.inserir(&linha(i)).unwrap();
    }
    local.sincronizar().unwrap();
    let inserir = inicio.elapsed().as_secs_f64();

    let caminho = hex_ida + hex_volta + json_montar + json_analisar + aplicar;
    let mostrar = |rotulo: &str, d: f64| {
        println!(
            "  {rotulo:<34} {:>8.2} us/evento   {:>5.1}%",
            us(d, n),
            d * 100.0 / caminho
        );
    };

    println!("  -- no source --");
    mostrar("hexadecimal da imagem (ida)", hex_ida);
    mostrar("montar o JSON do lote", json_montar);
    println!("  -- na replica --");
    mostrar("analisar o JSON do lote", json_analisar);
    mostrar("hexadecimal da imagem (volta)", hex_volta);
    mostrar("aplicar_evento (decodifica + insere)", aplicar);
    println!("  {:-<34} {:->8}", "", "");
    println!(
        "  {:<34} {:>8.2} us/evento",
        "o caminho todo, sem rede",
        us(caminho, n)
    );
    println!(
        "\n  {:<34} {:>8.2} us/evento   <- o piso",
        "insercao local pura, para comparar",
        us(inserir, n)
    );
    println!(
        "\n  A imagem tem {} bytes e viaja com {} caracteres de JSON:\n  \
         o hexadecimal DOBRA cada byte, e o resto e a moldura do objeto.",
        bytes_imagem / n,
        bytes_json / n
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir_r);
    let _ = std::fs::remove_dir_all(&dir_l);
}
