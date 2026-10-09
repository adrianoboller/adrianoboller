//! Quanto custa selar a pagina do `.ndx` -- o numero que o pedido 395 pedia
//! antes de o pedido 339 (achado 2) ligar o selo na arvore sobre coluna
//! marcada.
//!
//! Trabalho IGUAL dos dois lados: o mesmo `NdxFile`, a mesma chave `Str(40)`
//! (um nome, o caso da coluna marcada), as mesmas chaves na mesma ordem
//! embaralhada, as mesmas buscas -- e o cofre LIGADO nas duas voltas. A unica
//! diferenca e `NdxFile::criar` (versao 1) contra `NdxFile::criar_selado`
//! (versao 2). Medir pela `Table` trocaria tambem o `.reg`, e o numero diria
//! o custo de duas cifras como se fosse o de uma.
//!
//! O cache de paginas fica no padrao: o que se mede e o regime real, em que
//! cada falta de pagina paga um ChaCha20 de uma pagina inteira.
//!
//! ```bash
//! cargo run --release --example custo-do-selo-do-ndx -p phxsql-store -- [chaves] [voltas]
//! ```
//!
//! A ultima linha e `RESULTADO <json>`, com mediana e faixa min-max de cada
//! lado (regra do pedido 155: sem faixa nao se declara diferenca).

use std::time::Instant;

use phxsql_core::keyenc::{escrever_componente, largura_componente};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::ndx::NdxFile;

/// Xorshift64*, para embaralhar sem crate de fora.
struct Rng(u64);

impl Rng {
    fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

const TIPO: ColumnType = ColumnType::Str(40);

fn esquema() -> Schema {
    Schema::new(
        "t",
        vec![Column::new("nome", TIPO)],
        vec![IndexDef::new("porNome", vec![IndexColumn::asc(0)])],
    )
    .expect("esquema")
}

fn chave(i: u64) -> Vec<u8> {
    let mut buf = vec![0u8; largura_componente(&TIPO).expect("largura")];
    let v = Value::Str(format!("Fulano de Tal {i:010}"));
    escrever_componente(&v, &TIPO, false, false, &mut buf).expect("chave");
    buf
}

struct Volta {
    us_por_insercao: f64,
    us_por_busca: f64,
    paginas: u64,
    folha: usize,
}

fn volta(selado: bool, ordem: &[u64], buscas: &[u64]) -> Volta {
    let dir = std::env::temp_dir().join(format!(
        "custo-selo-ndx-{}-{}",
        if selado { "selado" } else { "claro" },
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("diretorio");
    let caminho = dir.join("t.ndx");
    let mut n = if selado {
        NdxFile::criar_selado(&caminho, &esquema()).expect("criar selado")
    } else {
        NdxFile::criar(&caminho, &esquema()).expect("criar")
    };
    assert_eq!(n.selado(), selado, "a volta nao mede o que diz medir");

    let inicio = Instant::now();
    for (rowid, i) in ordem.iter().enumerate() {
        n.inserir(0, &chave(*i), rowid as u64 + 1).expect("inserir");
    }
    n.sincronizar().expect("sincronizar");
    let inserir = inicio.elapsed();

    let inicio = Instant::now();
    let mut achados = 0usize;
    for i in buscas {
        achados += n.buscar(0, &chave(*i)).expect("buscar").len();
    }
    let buscar = inicio.elapsed();
    assert_eq!(achados, buscas.len(), "cada busca tinha de achar uma linha");

    let paginas = n.paginas();
    let folha = n.capacidade_de_folha(0);
    drop(n);
    let _ = std::fs::remove_dir_all(&dir);
    Volta {
        us_por_insercao: inserir.as_secs_f64() * 1e6 / ordem.len() as f64,
        us_por_busca: buscar.as_secs_f64() * 1e6 / buscas.len() as f64,
        paginas,
        folha,
    }
}

fn mediana(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).expect("numero"));
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn main() {
    let arg = |i: usize, padrao: u64| {
        std::env::args()
            .nth(i)
            .and_then(|s| s.parse().ok())
            .unwrap_or(padrao)
    };
    let chaves = arg(1, 200_000);
    let voltas = arg(2, 3).max(1) as usize;

    cofre::desligar();
    cofre::definir("medicao do custo do selo do ndx", cofre::ITERACOES_MINIMAS).expect("cofre");

    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut ordem: Vec<u64> = (1..=chaves).collect();
    for i in (1..ordem.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        ordem.swap(i, j);
    }
    let buscas: Vec<u64> = (0..20_000).map(|_| rng.proximo() % chaves + 1).collect();

    let (mut ic, mut is, mut bc, mut bs) = (vec![], vec![], vec![], vec![]);
    let (mut pc, mut ps, mut fc, mut fs) = (0, 0, 0, 0);
    // Alternadas, para a deriva da maquina cair dos dois lados.
    for _ in 0..voltas {
        let c = volta(false, &ordem, &buscas);
        let s = volta(true, &ordem, &buscas);
        ic.push(c.us_por_insercao);
        is.push(s.us_por_insercao);
        bc.push(c.us_por_busca);
        bs.push(s.us_por_busca);
        (pc, ps, fc, fs) = (c.paginas, s.paginas, c.folha, s.folha);
    }
    cofre::desligar();

    let (ic, icmin, icmax) = mediana(&mut ic);
    let (is, ismin, ismax) = mediana(&mut is);
    let (bc, bcmin, bcmax) = mediana(&mut bc);
    let (bs, bsmin, bsmax) = mediana(&mut bs);
    println!("chaves {chaves}  voltas {voltas}  buscas {}", buscas.len());
    println!("folha        {fc} em claro -> {fs} selada");
    println!("paginas      {pc} -> {ps}");
    println!(
        "us/insercao  {ic:.3} [{icmin:.3}-{icmax:.3}] -> {is:.3} [{ismin:.3}-{ismax:.3}]  ({:.2}x)",
        is / ic
    );
    println!(
        "us/busca     {bc:.3} [{bcmin:.3}-{bcmax:.3}] -> {bs:.3} [{bsmin:.3}-{bsmax:.3}]  ({:.2}x)",
        bs / bc
    );
    println!(
        "RESULTADO {{\"chaves\":{chaves},\"voltas\":{voltas},\"folha_claro\":{fc},\
         \"folha_selada\":{fs},\"paginas_claro\":{pc},\"paginas_selado\":{ps},\
         \"us_insercao_claro\":[{ic:.3},{icmin:.3},{icmax:.3}],\
         \"us_insercao_selado\":[{is:.3},{ismin:.3},{ismax:.3}],\
         \"us_busca_claro\":[{bc:.3},{bcmin:.3},{bcmax:.3}],\
         \"us_busca_selado\":[{bs:.3},{bsmin:.3},{bsmax:.3}]}}"
    );
}
