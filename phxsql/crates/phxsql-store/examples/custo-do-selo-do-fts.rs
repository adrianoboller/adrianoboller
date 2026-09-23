//! Quanto custa selar a pagina do `.fts` -- medido, nunca estimado.
//!
//! Roda a MESMA carga duas vezes, no mesmo processo e na mesma maquina:
//! primeiro com o cofre desligado (`.fts` em claro, versao 1) e depois com
//! ele ligado (pagina selada, versao 2). O que se compara e trabalho igual,
//! e nao pergunta igual: as duas voltas indexam as mesmas linhas, com os
//! mesmos termos, e procuram as mesmas palavras.
//!
//! Mede tres coisas, porque o custo tem tres caras e so uma delas e o tempo:
//!
//! 1. **capacidade de folha** -- quantos termos cabem numa pagina. E o custo
//!    de FORMATO, e ele sai do proprio `.ndx` (`capacidade_de_folha`), nunca
//!    de uma conta refeita aqui: receita de numero copiada envelhece calada.
//! 2. **paginas e bytes do arquivo** -- o custo em DISCO, que e consequencia
//!    do primeiro e nem sempre aparece (uma folha a menos por pagina so
//!    cobra uma pagina a mais quando a divisao vira).
//! 3. **microssegundos por termo indexado e por busca** -- o custo em TEMPO,
//!    que e o ChaCha20 de uma pagina inteira por toque de disco. O cache de
//!    paginas esta ligado nos dois lados, entao o que se mede e o regime
//!    real e nao o pior caso.
//!
//! ```bash
//! cargo run --release --example custo-do-selo-do-fts -p phxsql-store
//! ```
//!
//! A ultima linha e `RESULTADO <json>`, como na `carga`.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, IndiceDeTexto, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::table::Table;

/// Doze palavras, como no `custo-do-fts-de-verdade`: texto de cadastro tem
/// vocabulario pequeno e repetido, e um vocabulario sorteado mediria uma
/// arvore que este banco nao tem.
const RECHEIO: [&str; 12] = [
    "pedido",
    "cliente",
    "nota",
    "fiscal",
    "entrega",
    "produto",
    "valor",
    "desconto",
    "parcela",
    "vencimento",
    "transportadora",
    "observacao",
];

const LINHAS: i64 = 20_000;

fn esquema() -> Schema {
    Schema::new(
        "docs",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            // MARCADA: e o que faz o `.fts` nascer selado quando o cofre
            // esta ligado, e e o caso do pedido 340.
            Column::new("corpo", ColumnType::Str(200)).com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema")
    .com_indices_de_texto(vec![IndiceDeTexto::new("porCorpo", 1)])
    .expect("indice de texto")
}

fn corpo(i: i64) -> String {
    let mut s = String::with_capacity(120);
    for k in 0..8 {
        s.push_str(RECHEIO[((i as usize) + k) % RECHEIO.len()]);
        s.push(' ');
    }
    // Um termo unico por linha, para a arvore crescer de verdade em vez de
    // repetir doze chaves vinte mil vezes.
    s.push_str(&format!("documento{i:07}"));
    s
}

struct Medida {
    selado: bool,
    capacidade: usize,
    paginas: u64,
    bytes: u64,
    us_por_termo: f64,
    us_por_busca: f64,
    termos: u64,
}

fn rodada(rotulo: &str, com_cofre: bool) -> Medida {
    cofre::desligar();
    if com_cofre {
        cofre::definir("medicao do custo do selo", cofre::ITERACOES_MINIMAS).expect("cofre");
    }
    let dir = std::env::temp_dir().join(format!("custo-selo-fts-{rotulo}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("diretorio");

    let mut t = Table::criar(&dir, esquema()).expect("criar");
    let inicio = Instant::now();
    for i in 1..=LINHAS {
        t.inserir(&[Value::Int(i), Value::Str(corpo(i))])
            .expect("inserir");
    }
    t.sincronizar().expect("sincronizar");
    let gravar = inicio.elapsed();

    // 9 termos por linha: 8 do recheio (com repeticao dentro da linha caindo
    // para chaves distintas) mais o unico. O numero vem do arquivo, nao daqui.
    let termos = t.fts_qtd_chaves(0);

    let inicio = Instant::now();
    let mut achados = 0usize;
    for i in (1..=LINHAS).step_by(20) {
        achados += t
            .procurar_texto("porCorpo", &format!("documento{i:07}"))
            .expect("procurar")
            .rowids
            .len();
    }
    let buscas = (LINHAS as usize).div_ceil(20);
    let procurar = inicio.elapsed();
    assert_eq!(
        achados, buscas,
        "a busca tinha de achar uma linha por palavra"
    );

    let capacidade = t.fts_capacidade_de_folha(0);
    let paginas = t.fts_paginas();
    let selado = t.fts_selado();
    drop(t);
    let bytes = std::fs::metadata(dir.join(format!("docs.{}", phxsql_store::fts::EXT_FTS)))
        .expect("arquivo")
        .len();
    let _ = std::fs::remove_dir_all(&dir);
    cofre::desligar();

    Medida {
        selado,
        capacidade,
        paginas,
        bytes,
        us_por_termo: gravar.as_secs_f64() * 1e6 / termos as f64,
        us_por_busca: procurar.as_secs_f64() * 1e6 / buscas as f64,
        termos,
    }
}

fn main() {
    let claro = rodada("claro", false);
    let selo = rodada("selado", true);
    assert!(!claro.selado, "a rodada em claro nao podia estar selada");
    assert!(selo.selado, "a rodada com cofre tinha de estar selada");
    assert_eq!(
        claro.termos, selo.termos,
        "as duas rodadas tem de indexar os MESMOS termos -- senao a comparacao \
         e de trabalho diferente, e o numero nao vale"
    );

    let perda_de_folha =
        100.0 * (claro.capacidade - selo.capacidade) as f64 / claro.capacidade as f64;
    let em_disco = 100.0 * (selo.bytes as f64 - claro.bytes as f64) / claro.bytes as f64;

    println!(
        "linhas          {LINHAS}   termos indexados {}",
        claro.termos
    );
    println!(
        "folha           {} em claro -> {} selada   ({perda_de_folha:.3}% de capacidade)",
        claro.capacidade, selo.capacidade
    );
    println!(
        "paginas         {} -> {}   bytes {} -> {}   ({em_disco:+.3}% em disco)",
        claro.paginas, selo.paginas, claro.bytes, selo.bytes
    );
    println!(
        "us por termo    {:.3} -> {:.3}   ({:.3}x)",
        claro.us_por_termo,
        selo.us_por_termo,
        selo.us_por_termo / claro.us_por_termo
    );
    println!(
        "us por busca    {:.3} -> {:.3}   ({:.3}x)",
        claro.us_por_busca,
        selo.us_por_busca,
        selo.us_por_busca / claro.us_por_busca
    );
    println!(
        "RESULTADO {{\"linhas\":{LINHAS},\"termos\":{},\
         \"folha_claro\":{},\"folha_selada\":{},\"perda_de_folha_pct\":{perda_de_folha:.3},\
         \"paginas_claro\":{},\"paginas_selado\":{},\
         \"bytes_claro\":{},\"bytes_selado\":{},\"disco_pct\":{em_disco:.3},\
         \"us_por_termo_claro\":{:.3},\"us_por_termo_selado\":{:.3},\
         \"us_por_busca_claro\":{:.3},\"us_por_busca_selado\":{:.3}}}",
        claro.termos,
        claro.capacidade,
        selo.capacidade,
        claro.paginas,
        selo.paginas,
        claro.bytes,
        selo.bytes,
        claro.us_por_termo,
        selo.us_por_termo,
        claro.us_por_busca,
        selo.us_por_busca,
    );
}
