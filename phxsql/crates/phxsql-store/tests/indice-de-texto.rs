//! O indice de texto pela TABELA: declarar, gravar, achar, alterar e excluir.
//!
//! Estes testes provam o que o `docs/FTS.md` §7 exige, e o que mais importa
//! neles nao e "achar": e **nunca achar a mais**. Achar a menos e atraso e o
//! indice o declara; achar a mais e mentira, e nao ha como o cliente perceber.

use phxsql_core::schema::{Column, IndexColumn, IndexDef, IndiceDeTexto, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn esquema() -> Schema {
    Schema::new(
        "docs",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("titulo", ColumnType::Str(80)),
            Column::new("corpo", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema")
    .com_indices_de_texto(vec![
        IndiceDeTexto::new("porTitulo", 1),
        IndiceDeTexto::new("porCorpo", 2),
    ])
    .expect("indices de texto")
}

fn nova(nome: &str) -> (Table, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("phx-fts-tab-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    (Table::criar(&dir, esquema()).expect("criar"), dir)
}

fn linha(i: i64, titulo: &str, corpo: &str) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(titulo.into()),
        Value::Memo(corpo.into()),
    ]
}

/// Achar pelo `Str` inline e pelo `Memo` externo -- e o `Memo` e o caso que a
/// lacuna do HFSQL(R) nomeia: "procurar uma palavra dentro de um `.memo`".
#[test]
fn acha_no_titulo_e_no_corpo() {
    let (mut t, _d) = nova("basico");
    let a = t
        .inserir(&linha(1, "pedido urgente", "o cliente fenix pediu"))
        .unwrap();
    let b = t
        .inserir(&linha(2, "nota fiscal", "entrega comum"))
        .unwrap();

    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a]
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids,
        vec![a]
    );
    assert_eq!(
        t.procurar_texto("porTitulo", "nota").unwrap().rowids,
        vec![b]
    );
    // O termo do corpo NAO pode aparecer no indice do titulo.
    assert!(t
        .procurar_texto("porTitulo", "fenix")
        .unwrap()
        .rowids
        .is_empty());
}

/// A dobra de acento, que e a razao de o indice existir em portugues.
#[test]
fn acha_sem_acento_o_que_foi_gravado_com() {
    let (mut t, _d) = nova("dobra");
    let a = t
        .inserir(&linha(1, "a Fênix", "renasceu em Blumenau"))
        .unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix").unwrap().rowids,
        vec![a]
    );
    assert_eq!(
        t.procurar_texto("porTitulo", "FENIX").unwrap().rowids,
        vec![a]
    );
}

/// **O teste que mais importa.** Alterar tem de tirar as palavras VELHAS.
///
/// Sem o `desindexar_texto` no `atualizar`, o indice fica apontando a palavra
/// que a linha nao tem mais -- e a busca por ela devolve uma linha que nao
/// casa. Achar a mais e mentira, e o cliente nao tem como perceber.
#[test]
fn alterar_tira_a_palavra_velha_do_indice() {
    let (mut t, _d) = nova("alterar");
    let a = t
        .inserir(&linha(1, "pedido antigo", "corpo original"))
        .unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "antigo").unwrap().rowids,
        vec![a]
    );

    t.atualizar(a, &linha(1, "pedido novo", "corpo trocado"))
        .unwrap();

    assert!(
        t.procurar_texto("porTitulo", "antigo")
            .unwrap()
            .rowids
            .is_empty(),
        "a palavra velha ficou no indice: ele passou a achar a MAIS"
    );
    assert_eq!(
        t.procurar_texto("porTitulo", "novo").unwrap().rowids,
        vec![a]
    );
    assert!(
        t.procurar_texto("porCorpo", "original")
            .unwrap()
            .rowids
            .is_empty(),
        "a palavra velha do MEMO ficou no indice -- e o memo e o caso em que \
         o texto antigo so existe no `.memo`, que o `atualizar` decodifica \
         SEM externos"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "trocado").unwrap().rowids,
        vec![a]
    );
}

/// Excluir de vez tem de tirar a linha do indice, e o memo tambem.
#[test]
fn excluir_de_vez_tira_do_indice() {
    let (mut t, _d) = nova("excluir");
    let a = t
        .inserir(&linha(1, "pedido fenix", "corpo com raridade"))
        .unwrap();
    let b = t.inserir(&linha(2, "pedido comum", "outro corpo")).unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a, b]
    );

    t.excluir_de_vez(a, "teste").unwrap();

    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![b]
    );
    assert!(t
        .procurar_texto("porTitulo", "fenix")
        .unwrap()
        .rowids
        .is_empty());
    assert!(
        t.procurar_texto("porCorpo", "raridade")
            .unwrap()
            .rowids
            .is_empty(),
        "o termo do memo sobreviveu a exclusao"
    );
}

/// O indice acha EXATAMENTE o que a varredura acha -- conjuntos, nao contagens.
///
/// E a prova da §7.1 do `FTS.md`: indice que acha a mais e pior que indice que
/// acha a menos, e comparar contagem esconderia uma troca.
#[test]
fn o_indice_acha_o_mesmo_que_a_varredura() {
    let (mut t, _d) = nova("igual");
    let mut esperados = Vec::new();
    for i in 0..60i64 {
        let titulo = if i % 7 == 0 {
            format!("pedido fenix {i}")
        } else {
            format!("pedido comum {i}")
        };
        let r = t.inserir(&linha(i, &titulo, "corpo")).unwrap();
        if i % 7 == 0 {
            esperados.push(r);
        }
    }
    let pelo_indice = t.procurar_texto("porTitulo", "fenix").unwrap().rowids;
    assert_eq!(pelo_indice, esperados, "o indice divergiu da verdade");
    assert!(!esperados.is_empty(), "o teste tem de achar alguma coisa");
}

/// Tabela SEM indice de texto nao ganha o arquivo, e a busca recusa dizendo.
#[test]
fn tabela_sem_indice_de_texto_nao_paga_nada() {
    let dir = std::env::temp_dir().join(format!("phx-fts-sem-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let e = Schema::new(
        "simples",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut t = Table::criar(&dir, e).unwrap();
    t.inserir(&[Value::Int(1)]).unwrap();
    assert!(
        !dir.join("simples.fts").exists(),
        "nasceu um `.fts` numa tabela que nao o declarou"
    );
    assert!(t.procurar_texto("qualquer", "x").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// O `.fts` e DERIVADO: apaga-lo custa tempo, nunca dado.
///
/// Reabrir a tabela sem o arquivo tem de reconstrui-lo -- porque a alternativa
/// e uma busca que devolve VAZIO em silencio, e resposta vazia errada e pior
/// que espera.
#[test]
fn apagar_o_fts_e_reabrir_reconstroi_em_vez_de_achar_nada() {
    let (mut t, dir) = nova("derivado");
    let a = t.inserir(&linha(1, "pedido fenix", "corpo raro")).unwrap();
    t.sincronizar().unwrap();
    drop(t);

    std::fs::remove_file(dir.join("docs.fts")).expect("apagar o .fts");

    let mut t = Table::abrir(&dir, "docs").unwrap();
    assert_eq!(
        t.procurar_texto("porTitulo", "fenix").unwrap().rowids,
        vec![a],
        "o `.fts` nao foi reconstruido, e a busca devolveu vazio em silencio"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "raro").unwrap().rowids,
        vec![a]
    );
}
