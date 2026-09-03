//! A réplica **aplica**, ela não **julga** — e a cascata do source chega até lá.
//!
//! O que estes testes protegem, e o defeito que motivou cada um:
//!
//! 1. **A réplica não confere chave estrangeira.** Medido antes da guarda
//!    (`--example sonda-replica-fk`): com `clientes.ins → pedidos.ins →
//!    clientes.alt` no source, a réplica recusava a inclusão da filha nas
//!    ordens «mãe primeiro» e «filha primeiro» — `pedidos` ficava com **0 dos
//!    2 eventos**, e a linha não existia. A guarda causava a perda de dado que
//!    ela existe para impedir: a replicação anda por tabela, e não há ordem
//!    global entre tabelas.
//! 2. **A réplica não refaz a cascata.** O source já cascateou, e o evento que
//!    a cascata dele gerou vem replicado por conta própria. Refazer aqui
//!    deixava no diário da réplica um evento que o source nunca mandou.
//! 3. **A cascata do source grava a imagem no diário da filha.** A cascata
//!    abre a filha num handle próprio; nascendo com o padrão (`imagem` desligada)
//!    o evento de alteração da filha ia para o diário **sem a imagem**, e a
//!    réplica o recusava com «veio sem imagem» — nas TRÊS ordens. Ninguém
//!    percebia porque quem liga a imagem liga na tabela que abre, e esta o
//!    motor abria por baixo.
//! 4. **A marca de réplica não vaza.** Depois de um `aplicar_evento`, o mesmo
//!    handle tem de voltar a conferir integridade numa escrita local. Sem o
//!    par liga/desliga, um `return` no meio deixaria o handle sem portão.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::log::Operacao;
use phxsql_store::table::Table;

fn esq_mae() -> Schema {
    Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn esq_filha() -> Schema {
    Schema::new(
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
    .unwrap()
}

fn abrir(d: &std::path::Path, nome: &str) -> Table {
    Table::abrir(d, nome).unwrap().com_imagem_no_diario(true)
}

/// Um evento do diário de uma tabela, pronto para viajar.
struct Evento {
    tabela: &'static str,
    operacao: Operacao,
    rowid: u64,
    imagem: Vec<u8>,
}

/// Monta o source: mãe, filha apontando para ela, e uma alteração da chave da
/// mãe que **cascateia** até a filha. Devolve os eventos das duas tabelas.
fn source(dir: &std::path::Path) -> Vec<Evento> {
    let mut m = Table::criar(dir, esq_mae())
        .unwrap()
        .com_imagem_no_diario(true);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = Table::criar(dir, esq_filha())
        .unwrap()
        .com_imagem_no_diario(true);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    // A alteração da chave da mãe leva a filha junto (`ao_alterar: cascata`).
    let mut m = abrir(dir, "clientes");
    m.atualizar(1, &[Value::Int(2)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut saida = Vec::new();
    for tabela in ["clientes", "pedidos"] {
        let mut t = abrir(dir, tabela);
        for (e, imagem) in t.diario_com_imagem(0, 0).unwrap() {
            saida.push(Evento {
                tabela,
                operacao: e.operacao,
                rowid: e.rowid,
                imagem,
            });
        }
    }
    saida
}

/// Aplica um evento abrindo a tabela do zero — o ciclo do lote da réplica.
fn aplicar(dir: &std::path::Path, e: &Evento) -> Result<(), String> {
    let mut t = abrir(dir, e.tabela);
    let r = t
        .aplicar_evento(e.operacao, e.rowid, &e.imagem)
        .map(|_| ())
        .map_err(|x| x.to_string());
    t.sincronizar().unwrap();
    r
}

/// Cria as duas tabelas VAZIAS na réplica, com o mesmo esquema do source.
fn replica_vazia(dir: &std::path::Path) {
    Table::criar(dir, esq_mae()).unwrap().sincronizar().unwrap();
    Table::criar(dir, esq_filha())
        .unwrap()
        .sincronizar()
        .unwrap();
}

fn cliente_da_filha(dir: &std::path::Path) -> Option<Value> {
    let mut f = Table::abrir(dir, "pedidos").unwrap();
    f.ler(1).unwrap().map(|l| l[1].clone())
}

/// O source tem quatro eventos; a réplica os aplica em QUALQUER ordem de
/// tabela e fecha igual. Era o que não acontecia: nas ordens «mãe primeiro» e
/// «filha primeiro» a filha nem chegava a existir.
#[test]
fn a_replica_converge_nas_tres_ordens_de_tabela() {
    let ds = DirTemp::novo("repint-source");
    let eventos = source(&ds.0);
    assert_eq!(eventos.len(), 4, "o source tem de ter 4 eventos");

    // A cascata do source deixou a filha na chave nova.
    assert_eq!(cliente_da_filha(&ds.0), Some(Value::Int(2)));

    // Três ordens de entrega, todas legítimas: a replicação anda por tabela e
    // não existe ordem global entre elas.
    let ordens: [(&str, [usize; 4]); 3] = [
        ("mae primeiro", [0, 1, 2, 3]),
        ("filha primeiro", [2, 3, 0, 1]),
        ("entrelacada", [0, 2, 1, 3]),
    ];
    for (rotulo, ordem) in ordens {
        let dr = DirTemp::novo("repint-replica");
        replica_vazia(&dr.0);
        for i in ordem {
            let e = &eventos[i];
            aplicar(&dr.0, e).unwrap_or_else(|x| {
                panic!(
                    "ordem {rotulo}: {} {:?} rowid {} recusado: {x}",
                    e.tabela, e.operacao, e.rowid
                )
            });
        }
        assert_eq!(
            cliente_da_filha(&dr.0),
            Some(Value::Int(2)),
            "ordem {rotulo}: a filha da réplica divergiu do source"
        );
        // E o diário da réplica tem os MESMOS eventos do source: nem a menos
        // (evento recusado) nem a mais (cascata refeita aqui).
        for (tabela, quantos) in [("clientes", 2u64), ("pedidos", 2u64)] {
            let mut t = Table::abrir(&dr.0, tabela).unwrap();
            assert_eq!(
                t.eventos().unwrap(),
                quantos,
                "ordem {rotulo}: {tabela} na réplica tem diário diferente do source"
            );
        }
    }
}

/// A imagem do evento que a CASCATA gerou não pode vir vazia: sem ela a
/// réplica não sabe para que valor a filha mudou, e recusa.
#[test]
fn o_evento_da_cascata_carrega_a_imagem_da_linha() {
    let ds = DirTemp::novo("repint-imagem");
    let eventos = source(&ds.0);
    let cascata = eventos
        .iter()
        .find(|e| e.tabela == "pedidos" && e.operacao == Operacao::Alteracao)
        .expect("a cascata tem de ter gerado uma alteracao em pedidos");
    assert!(
        !cascata.imagem.is_empty(),
        "o evento da cascata foi para o diário sem a imagem da linha: a \
         réplica o recusaria com «veio sem imagem»"
    );
}

/// A marca de réplica é de UM evento, e não do handle: depois de aplicar, o
/// mesmo handle volta a conferir integridade numa escrita local.
#[test]
fn a_marca_de_replica_nao_vaza_para_a_escrita_local() {
    let ds = DirTemp::novo("repint-vaza-s");
    let eventos = source(&ds.0);
    let dr = DirTemp::novo("repint-vaza-r");
    replica_vazia(&dr.0);

    let e = eventos
        .iter()
        .find(|e| e.tabela == "pedidos" && e.operacao == Operacao::Inclusao)
        .unwrap();
    let mut f = abrir(&dr.0, "pedidos");
    // A mãe ainda não chegou: a réplica aceita mesmo assim.
    f.aplicar_evento(e.operacao, e.rowid, &e.imagem).unwrap();

    // A escrita LOCAL no mesmo handle continua conferindo.
    let erro = f
        .inserir(&[Value::Int(11), Value::Int(777)])
        .expect_err("a escrita local tem de continuar conferindo a chave");
    let t = erro.to_string();
    assert!(
        t.contains("fk_cliente"),
        "o erro tinha de ser da chave estrangeira, e veio: {t}"
    );
}
