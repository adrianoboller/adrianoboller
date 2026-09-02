//! A conferencia da chave estrangeira, e o achado que so a MEDICAO trouxe.
//!
//! Os testes de uma tabela so passavam todos. O medidor `custo-da-fk` caiu na
//! primeira rodada -- e o motivo nao era do medidor: a conferencia abre a MAE
//! num SEGUNDO descritor, e um segundo descritor sobre tabela com escrita
//! pendente le um indice que ainda nao foi para o disco. O store recusa, e
//! recusa certo: ler seria pior.
//!
//! Isso alcanca o servidor, e nao so o medidor. O caminho do `commit` mantem
//! VARIAS tabelas abertas ao mesmo tempo (o mapa `abertas`), entao mae e filha
//! na mesma transacao caem exatamente aqui.
//!
//! *Interface -- e garantia -- so se prova exercitando.*

use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn dir(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-fk-{nome}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn mae(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn filha(d: &std::path::Path, conferindo: bool) -> Table {
    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )
    .conferindo(conferindo)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// A mae ABERTA e ja gravada e vista normalmente.
///
/// Este e o caso comum -- a mae ja existe quando a filha entra -- e ele tinha
/// de ficar num teste proprio, porque e ele que impede o teste seguinte de
/// "passar" declarando que a conferencia nunca funciona.
#[test]
fn a_mae_aberta_e_ja_gravada_e_vista() {
    let d = dir("mae-aberta");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    // Continua ABERTA de proposito: o que impede a leitura nao e o descritor,
    // e a escrita pendente.
    let mut f = filha(&d, true);
    f.inserir(&[Value::Int(10), Value::Int(1)])
        .expect("a mae gravada e aberta nao foi vista");
}

/// O LIMITE, medido e dito com todas as letras: a conferencia le o que ja foi
/// gravado.
///
/// A mae inserida e NAO sincronizada nao esta visivel -- e isso alcanca o
/// servidor, porque o `commit` mantem varias tabelas abertas ao mesmo tempo:
/// mae e filha na mesma transacao caem aqui. **E o mesmo buraco do
/// read-your-own-writes** (SP000006 do roteiro), e nao um defeito a parte:
/// quem nao enxerga a propria escrita tambem nao enxerga a mae que acabou de
/// inserir.
///
/// O que este teste TRAVA nao e a limitacao -- e a QUALIDADE do recado. Sozinho,
/// o erro cru dizia "indice corrompido: reconstrua", mandando o leitor reparar
/// um arquivo sao.
///
/// A causa crua CONTINUA na mensagem, e de proposito: e ela que diz qual
/// arquivo e qual guarda recusou. O que mudou e que ela virou parenteses
/// dentro da explicacao, em vez de ser a mensagem inteira. Por isso o teste
/// afirma que a explicacao vem DEPOIS da causa, e nao que a causa sumiu --
/// jogar a causa fora trocaria um recado ruim por um recado cego.
#[test]
fn a_mae_nao_gravada_recusa_dizendo_por_que() {
    let d = dir("mae-pendente");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    // SEM sincronizar, de proposito.
    let mut f = filha(&d, true);
    let e = f
        .inserir(&[Value::Int(10), Value::Int(1)])
        .expect_err("a mae pendente foi vista -- o limite caducou, atualize o docs");
    let txt = e.to_string();
    assert!(
        txt.contains("mesma transacao") || txt.contains("ja foi gravado"),
        "o recado nao explica o limite: {txt}"
    );
    let (i_causa, i_expl) = (
        txt.find("reconstrua").unwrap_or(usize::MAX),
        txt.find("ja foi gravado").unwrap_or(0),
    );
    assert!(
        i_expl > i_causa,
        "a causa crua ficou por ultimo e vira a ultima palavra do recado: {txt}"
    );
    assert!(
        matches!(e, PhxError::Integridade(_)),
        "familia errada: {txt}"
    );
}

/// Com a chave DESLIGADA nada disso acontece -- o caminho antigo continua
/// intacto, inclusive com a mae aberta.
#[test]
fn sem_conferir_a_mae_aberta_nao_muda_nada() {
    let d = dir("mae-aberta-off");
    let mut m = mae(&d);
    m.inserir(&[Value::Int(1)]).unwrap();
    let mut f = filha(&d, false);
    f.inserir(&[Value::Int(10), Value::Int(999)])
        .expect("o caminho sem conferencia mudou");
}
