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

// ------------------------------------------------------- a queda e o irmao
//
// O `.fts` e um `.ndx` por dentro, e por isso ele herda a marca de «ficou
// para tras numa queda». Herda a marca e nao herdava o conserto: quem
// reconstroi o `.ndx` e o `reindexar`, e ele nao olhava o `.fts`. E o
// caminho IRMAO da petrea, com o agravante de a mensagem do proprio `.fts`
// mandar «reconstrua o indice de texto com `reindexar`» -- uma ordem que o
// codigo nao sabia cumprir.

/// Simula a queda: esquece a tabela sem fechar, como um `SIGKILL` faz.
///
/// `mem::forget` e a simulacao honesta -- o `Drop` nao roda, e o cabecalho no
/// disco fica com a marca de sujo levantada, que e exatamente o que a queda
/// do processo deixa.
fn derruba(t: Table) {
    std::mem::forget(t);
}

/// **A prova.** Depois da queda, a tabela reabre com o `.fts` marcado, e
/// **nenhuma gravacao passa** ate alguem reconstruir.
///
/// Com o defeito reposto (o `reindexar` sem o `.fts`) o `inserir` de baixo
/// falha: a marca continua la depois da reconstrucao.
#[test]
fn a_queda_marca_o_fts_e_o_reindexar_tem_de_baixar_a_marca() {
    let (mut t, d) = nova("queda");
    t.inserir(&linha(1, "pedido urgente", "corpo com fenix"))
        .unwrap();
    derruba(t);

    let mut t = Table::abrir(&d, "docs").expect("a tabela tem de reabrir");
    assert!(
        t.indice_precisa_reconstruir(),
        "a marca do .fts tem de ser VISTA por quem pergunta -- \
         perguntar so ao .ndx e nao ver a queda"
    );

    let indices = t.reindexar().expect("reindexar");
    assert!(!indices.is_empty());
    assert!(
        !t.indice_precisa_reconstruir(),
        "o reindexar tem de baixar a marca dos DOIS arquivos"
    );

    // E o que mais importa: a tabela volta a gravar e a busca volta a achar.
    let b = t
        .inserir(&linha(2, "nota fiscal", "corpo comum"))
        .expect("a tabela tem de voltar a gravar depois do reindexar");
    assert_eq!(
        t.procurar_texto("porTitulo", "nota").unwrap().rowids,
        vec![b]
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids.len(),
        1,
        "a linha de antes da queda tem de continuar no indice"
    );
}

/// Reconstruir NAO pode somar: chamar duas vezes tem de dar o mesmo resultado.
///
/// **Eu previ o defeito errado, e o teste mediu.** Achei que a segunda passada
/// acrescentaria a mesma chave e a busca devolveria o rowid duas vezes -- achar
/// a mais, que e mentira. O que acontece e melhor e pior ao mesmo tempo: a
/// arvore RECUSA (`chave completa ja existe no indice`), entao nao ha mentira,
/// mas o `reconstruir_fts` deixa de ser idempotente -- e era o `reindexar` que
/// ia bater nessa recusa em toda tabela que ja tivesse uma linha.
#[test]
fn reconstruir_duas_vezes_nao_duplica() {
    let (mut t, _d) = nova("duas-vezes");
    let a = t
        .inserir(&linha(1, "pedido urgente", "corpo com fenix"))
        .unwrap();

    t.reconstruir_fts().unwrap();
    t.reconstruir_fts().unwrap();

    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a],
        "reconstruir duas vezes duplicou a ocorrencia"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids,
        vec![a]
    );
}

/// `.fts` que nao abre nao pode derrubar a tabela -- ele e DERIVADO.
///
/// Dois casos caem aqui: arquivo corrompido, e contagem de indices divergente
/// (alguem declarou um indice de texto numa tabela que ja tem dados, que e uma
/// das tres coisas para as quais o `reindexar` existe). Com o defeito reposto
/// a tabela nem abria, e a mensagem mandava rodar o `reindexar` -- numa tabela
/// que nao abre. Refazer custa uma varredura e nunca custa dado.
#[test]
fn fts_corrompido_reconstroi_na_abertura_em_vez_de_derrubar_a_tabela() {
    let (mut t, d) = nova("corrompido");
    let a = t
        .inserir(&linha(1, "pedido urgente", "corpo com fenix"))
        .unwrap();
    drop(t);

    // Corrompe o `.fts` de proposito: nem magica, nem cabecalho.
    std::fs::write(d.join("docs.fts"), b"isto nao e um indice").unwrap();

    let mut t = Table::abrir(&d, "docs").expect("a tabela tem de abrir mesmo assim");
    assert_eq!(
        t.procurar_texto("porTitulo", "pedido").unwrap().rowids,
        vec![a],
        "o indice tem de nascer cheio, varrendo a tabela"
    );
    assert_eq!(
        t.procurar_texto("porCorpo", "fenix").unwrap().rowids,
        vec![a]
    );
}
