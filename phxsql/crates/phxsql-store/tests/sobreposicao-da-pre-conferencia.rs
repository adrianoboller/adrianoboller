//! Os dois buracos da sobreposicao que a LEITURA tolerava e a chave
//! estrangeira nao tolera -- consertados antes da pre-conferencia do COMMIT
//! (pedido 448), com prova propria.
//!
//! A pre-conferencia le a lista pela sobreposicao, com visibilidade de
//! prefixo: a escrita i ve o disco mais as escritas 0..i-1. Se a sobreposicao
//! mentir sobre o prefixo, a pre-conferencia aprova o que a passada recusa
//! depois da marca -- e o pedido inteiro nao fecha justamente o caso que o
//! motivou. As duas provas aqui sao do `store` sozinho, sem servidor: a do
//! servidor (`pre_conferencia_448`) prova o COMMIT; esta prova o chao.

mod comum;
use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{MaesEmProgresso, Pendente, Table};

fn dir(nome: &str) -> comum::DirTemp {
    comum::DirTemp::novo(&format!("sobreposicao-448-{nome}"))
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

fn filha(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )
    .conferindo(true)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// Empresta UMA mae, com o prefixo que ela carrega -- o papel que o mapa da
/// pre-conferencia faz no servidor.
struct UmaMae<'a>(&'a mut Table);

impl MaesEmProgresso for UmaMae<'_> {
    fn mae(&mut self, tabela_ref: &str) -> Option<&mut Table> {
        (self.0.nome() == tabela_ref).then_some(&mut *self.0)
    }
}

/// **Buraco (a):** a linha do DISCO cuja chave o prefixo alterou.
///
/// O `buscar` da sobreposicao so tirava do resultado a linha `Sumida` e so
/// acrescentava a NASCIDA. A mae alterada de 1 para 5 continuava achada pela
/// chave velha e nao era achada pela nova -- e `[atualizar M 1->5, inserir
/// filha->1]` passaria na pre-conferencia para quebrar na passada, depois da
/// marca.
///
/// Prova real: devolver o `buscar` a «tira so a Sumida, acrescenta so a
/// nascida» faz as duas assertivas cairem -- a chave velha acha `[1]` e a
/// nova acha `[]`.
#[test]
fn o_buscar_acha_pela_chave_nova_a_linha_do_disco_que_o_prefixo_alterou() {
    let d = dir("chave-alterada");
    let mut m = mae(&d);
    let r = m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut m = Table::abrir(&d, "clientes").unwrap();
    let mut linha = m.ler(r).unwrap().unwrap();
    linha[0] = Value::Int(5);
    m.sobrepor_mais(r, Pendente::Alteracao(&linha, &[]))
        .unwrap();

    assert_eq!(
        (
            m.buscar("porId", &[Value::Int(1)]).unwrap(),
            m.buscar("porId", &[Value::Int(5)]).unwrap(),
        ),
        (vec![], vec![r]),
        "(chave velha, chave nova): a sobreposicao mentiu sobre a linha do disco \
         que o prefixo alterou"
    );
}

/// **Buraco (a), o controle:** a linha do disco que o prefixo alterou SEM
/// mexer na chave continua achada por ela -- e a nascida continua achada no
/// fim, na ordem de digitacao. E o comportamento velho da leitura.
#[test]
fn o_buscar_continua_achando_a_chave_que_nao_mudou_e_a_nascida() {
    let d = dir("chave-parada");
    let mut m = mae(&d);
    let r = m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut m = Table::abrir(&d, "clientes").unwrap();
    let linha = m.ler(r).unwrap().unwrap();
    m.sobrepor_mais(r, Pendente::Alteracao(&linha, &[]))
        .unwrap();
    m.sobrepor_mais(2, Pendente::Insercao(&[Value::Int(2)]))
        .unwrap();
    assert_eq!(m.buscar("porId", &[Value::Int(1)]).unwrap(), vec![r]);
    assert_eq!(m.buscar("porId", &[Value::Int(2)]).unwrap(), vec![2]);
}

/// **O indice das chaves pendentes anda junto depois da primeira busca.**
///
/// A primeira busca monta o `ChavesPendentes`; dai em diante o
/// `sobrepor_mais` o atualiza so para o rowid que mudou. Se ele esquecesse, a
/// segunda busca responderia pelo retrato da primeira -- e a pre-conferencia,
/// que busca a cada escrita, veria um prefixo parado.
///
/// Prova real: tirar o `c.trocar(rowid, novas)` do `dobrar` (deixando
/// o indice velho no lugar) faz a busca pela chave 5 voltar vazia e a da
/// chave 1 voltar `[1]`.
#[test]
fn o_indice_das_pendentes_acompanha_a_troca_depois_da_primeira_busca() {
    let d = dir("indice-anda");
    let mut m = mae(&d);
    let r = m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut m = Table::abrir(&d, "clientes").unwrap();
    m.sobrepor_mais(2, Pendente::Insercao(&[Value::Int(2)]))
        .unwrap();
    // A primeira busca monta o indice.
    assert_eq!(m.buscar("porId", &[Value::Int(1)]).unwrap(), vec![r]);
    assert_eq!(m.buscar("porId", &[Value::Int(2)]).unwrap(), vec![2]);
    let mut linha = m.ler(r).unwrap().unwrap();
    linha[0] = Value::Int(5);
    m.sobrepor_mais(r, Pendente::Alteracao(&linha, &[]))
        .unwrap();
    m.sobrepor_mais(2, Pendente::Exclusao).unwrap();
    m.sobrepor_mais(3, Pendente::Insercao(&[Value::Int(1)]))
        .unwrap();
    assert_eq!(
        (
            m.buscar("porId", &[Value::Int(1)]).unwrap(),
            m.buscar("porId", &[Value::Int(5)]).unwrap(),
            m.buscar("porId", &[Value::Int(2)]).unwrap(),
        ),
        (vec![3], vec![r], vec![]),
        "(chave 1, chave 5, chave 2): o indice das pendentes ficou no retrato da \
         primeira busca"
    );
}

/// **Buraco (b):** a conferencia de «mae viva» lia `reg.ler`, POR BAIXO da
/// sobreposicao. Em `[excluir_suave M, inserir filha->M]` ela via a mae viva
/// que o proprio prefixo ja tinha marcado.
///
/// Prova real: devolver o laco do `reg.ler` a `conferir_uma_fk` faz a
/// pre-conferencia APROVAR a filha -- `Ok` onde tem de ser «esta EXCLUIDA».
#[test]
fn a_mae_marcada_no_prefixo_nao_esta_viva_para_a_filha() {
    let d = dir("mae-marcada");
    let mut m = mae(&d);
    assert!(
        m.esquema().coluna_softdeleted().is_some(),
        "a prova precisa da exclusao suave"
    );
    let r = m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);
    let mut f = filha(&d);
    f.sincronizar().unwrap();

    let mut m = Table::abrir(&d, "clientes").unwrap();
    m.sobrepor_mais(r, Pendente::Marca(true)).unwrap();
    let e = f
        .pre_conferir(
            1,
            Pendente::Insercao(&[Value::Int(10), Value::Int(1)]),
            "",
            &mut UmaMae(&mut m),
        )
        .expect_err("a mae que o prefixo excluiu de forma suave foi vista viva");
    assert!(
        matches!(e, PhxError::Integridade(_)) && e.to_string().contains("EXCLUIDA"),
        "{e}"
    );

    // O controle: sem a marca no prefixo, a mesma filha passa.
    let mut m = Table::abrir(&d, "clientes").unwrap();
    f.pre_conferir(
        1,
        Pendente::Insercao(&[Value::Int(10), Value::Int(1)]),
        "",
        &mut UmaMae(&mut m),
    )
    .expect("a mae viva no disco e sem troca pendente tinha de ser vista");
}
