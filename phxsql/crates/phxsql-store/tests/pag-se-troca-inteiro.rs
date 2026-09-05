//! Guarda: o `.pag` nunca aparece PELA METADE para quem le de fora.
//!
//! # O defeito, e o numero que o mediu
//!
//! O `.pag` e' um descritor **derivado**: o motor nunca o le, e apagar o
//! arquivo nao quebra a tabela (`docs/FORMATO.md` secao 9). Ele existe para
//! quem esta do lado de fora -- uma camada SQL, um ETL, um relatorio.
//!
//! Ate' 05/09/2026 `pag::escrever` usava `std::fs::write`, que abre com
//! `O_TRUNC`. Entre o zerar e o terminar de escrever o arquivo esta vazio ou
//! partido, e isso acontece a CADA `Table::sincronizar()`. Medido: a janela
//! truncada dura **33,2 us por regravacao** -- 36% dos 93 us da gravacao -- e
//! um leitor que insista durante a regravacao pega o arquivo partido em
//! **82,4% das leituras** (482.246 de 585.169). Com temporario + `rename`,
//! **0 de 606.086**.
//!
//! O conserto e' de ATOMICIDADE e nao de durabilidade, e a diferenca e' a
//! decisao: um `.pag` perdido se regrava sozinho no proximo `sincronizar`; um
//! `.pag` pela metade mente para a unica plateia que ele tem. Por isso **nao
//! ha `fsync` novo aqui**, e a catraca `TETO_FSYNC_POR_FECHO_V2` fica em 8.
//!
//! # Por que ela mede o EFEITO, e nao a chamada
//!
//! Conferir «`escrever` chamou `rename`» seria medir a intencao. Esta guarda
//! poe um leitor de fora insistindo enquanto o motor regrava, e conta quantas
//! vezes ele pegou JSON que nao fecha -- que e' exatamente o que o ETL veria.
//!
//! **Prova real (05/09/2026):** trocando o corpo de `pag::escrever` de volta
//! por `std::fs::write(&caminho, texto)`, esta guarda falha na corrida com
//! **1.856 leituras partidas de 118.526**.

mod comum;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// Quantas regravacoes o escritor faz. Contagem, e nao relogio: uma maquina
/// lenta nao pode tornar a guarda frouxa.
const REGRAVACOES: usize = 300;

#[test]
fn quem_le_de_fora_nunca_pega_o_pag_pela_metade() {
    let d = comum::DirTemp::novo("pag-inteiro");

    // Particao alfanumerica: e' o `.pag` GRANDE, com a lista dos 37 baldes --
    // o caso em que a janela truncada e' maior.
    let esquema = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_paginacao(Paginacao::nova(50, 999).unwrap())
    .unwrap();

    let mut t = Table::criar(&d, esquema).unwrap();
    for i in 1..=200 {
        t.inserir(&[Value::Int(i), Value::Str(format!("nome {i}"))])
            .unwrap();
    }
    let alvo = t.gravar_pag().unwrap();
    assert!(
        alvo.exists(),
        "o .pag tem de existir antes de a guarda comecar"
    );

    let pare = AtomicBool::new(false);
    let lidas = AtomicU64::new(0);
    let partidas = AtomicU64::new(0);

    std::thread::scope(|s| {
        s.spawn(|| {
            // O leitor de FORA: nao sabe nada do motor, so le o arquivo.
            while !pare.load(Ordering::Relaxed) {
                lidas.fetch_add(1, Ordering::Relaxed);
                match std::fs::read_to_string(&alvo) {
                    // JSON que nao fecha em `}` esta pela metade. Analisar o
                    // texto inteiro seria mais caro e nao diria mais: o que
                    // o `O_TRUNC` deixa e' um prefixo.
                    Ok(txt) => {
                        if !txt.trim_end().ends_with('}') {
                            partidas.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    // Sumir tambem conta: o arquivo nao pode deixar de existir.
                    Err(_) => {
                        partidas.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
        for _ in 0..REGRAVACOES {
            t.gravar_pag().unwrap();
        }
        pare.store(true, Ordering::Relaxed);
    });

    let l = lidas.load(Ordering::Relaxed);
    let p = partidas.load(Ordering::Relaxed);
    assert!(
        l > 0,
        "o leitor nao chegou a ler nada; sem leitura a guarda nao prova nada"
    );
    assert_eq!(
        p, 0,
        "de {l} leituras, {p} pegaram o .pag pela metade -- e' o que um ETL \
         de fora veria. O arquivo tem de ser trocado inteiro, por `rename`"
    );

    // E o temporario nao sobra no caminho feliz: o `rename` o leva embora.
    let sobra = d.join("clientes.pag.novo");
    assert!(
        !sobra.exists(),
        "o temporario {sobra:?} ficou para tras depois de {REGRAVACOES} \
         regravacoes bem-sucedidas"
    );
}
