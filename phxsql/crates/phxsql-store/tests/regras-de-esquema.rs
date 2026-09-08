//! As regras de esquema no caminho de escrita: DEFAULT, CHECK, coluna
//! calculada, indice parcial e indice por expressao.
//!
//! # O que estes testes protegem
//!
//! 1. **a regra mora no motor**, e nao no servidor: `Table::inserir` e
//!    `Table::atualizar` a aplicam, entao FFI, replica e example passam por
//!    ela do mesmo jeito.
//! 2. **a replica NAO julga**: a imagem que chega ja veio com tudo aplicado
//!    na origem, e reaplicar seria recusar o que a origem aceitou -- o mesmo
//!    buraco do pedido 171.
//! 3. **o indice parcial vale em toda porta**: inserir, atualizar (sai e
//!    entra), excluir e `reindexar`.
//! 4. **a chave de busca nao reavalia a expressao**: quem procura por
//!    `lower(nome)` manda o valor baixo.

#[allow(
    dead_code,
    reason = "o modulo comum serve a varios testes; este usa so o DirTemp"
)]
mod comum;

use comum::DirTemp;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const ID: usize = 0;
const NOME: usize = 1;
const V: usize = 2;
const DOBRO: usize = 3;
const PRECO: usize = 4;

fn esquema() -> Schema {
    Schema::new(
        "t",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
            Column::new("v", ColumnType::Int8)
                .com_padrao("7")
                .unwrap()
                .com_check("v > 0")
                .unwrap(),
            Column::new("dobro", ColumnType::Int8)
                .com_calculada("v * 2")
                .unwrap(),
            Column::new(
                "preco",
                ColumnType::Decimal {
                    precisao: 12,
                    escala: 2,
                },
            )
            .com_padrao("ROUND(v * 1.5, 2)")
            .unwrap(),
        ],
        vec![
            IndexDef::new("pk", vec![IndexColumn::asc(ID)]).primaria(),
            IndexDef::new("so_grandes", vec![IndexColumn::asc(V)])
                .com_onde("v >= 10")
                .unwrap(),
            IndexDef::new("por_baixo", vec![IndexColumn::asc(NOME)])
                .com_expressao(0, "lower(nome)")
                .unwrap(),
        ],
    )
    .unwrap()
}

fn linha(id: i64, nome: &str, v: Value) -> Vec<Value> {
    // A calculada e o preco vem NULOS: e o motor que os preenche. E as duas
    // colunas de sistema vem no fim, como o servidor manda.
    vec![
        Value::Int(id),
        Value::Str(nome.into()),
        v,
        Value::Null,
        Value::Null,
        Value::Bool(false),
        Value::Null,
    ]
}

#[test]
fn o_default_entra_no_inserir_e_so_no_inserir() {
    let d = DirTemp::novo("default");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    let r = t.inserir(&linha(1, "Ana", Value::Null)).unwrap();
    let l = t.ler(r).unwrap().unwrap();
    assert_eq!(l[V], Value::Int(7), "v veio nulo e o padrao e 7");
    assert_eq!(
        l[PRECO],
        Value::Decimal(1050),
        "o padrao de preco e uma expressao: 7 * 1.5"
    );
    // Quem manda valor fica com o valor.
    let r2 = t.inserir(&linha(2, "Bia", Value::Int(3))).unwrap();
    assert_eq!(t.ler(r2).unwrap().unwrap()[V], Value::Int(3));
    // No atualizar, nulo e nulo... e aqui o CHECK `v > 0` da NULL e PASSA.
    let mut l2 = t.ler(r2).unwrap().unwrap();
    l2[V] = Value::Null;
    t.atualizar(r2, &l2).unwrap();
    assert_eq!(
        t.ler(r2).unwrap().unwrap()[V],
        Value::Null,
        "o DEFAULT nao vale no atualizar"
    );
}

#[test]
fn o_check_recusa_no_inserir_e_no_atualizar_e_aceita_o_legitimo() {
    let d = DirTemp::novo("check");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    let e = t
        .inserir(&linha(1, "Ana", Value::Int(-5)))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("CHECK da coluna v") && e.contains("v > 0"),
        "{e}"
    );
    // O CONTROLE: o permitido passa na mesma tabela.
    let r = t.inserir(&linha(1, "Ana", Value::Int(5))).unwrap();
    let mut l = t.ler(r).unwrap().unwrap();
    l[V] = Value::Int(-1);
    let e = t.atualizar(r, &l).unwrap_err().to_string();
    assert!(e.contains("CHECK da coluna v"), "{e}");
    assert_eq!(
        t.ler(r).unwrap().unwrap()[V],
        Value::Int(5),
        "a recusa nao gravou nada"
    );
    l[V] = Value::Int(9);
    t.atualizar(r, &l).unwrap();
    assert_eq!(t.ler(r).unwrap().unwrap()[V], Value::Int(9));
}

#[test]
fn a_calculada_e_sempre_recalculada_e_ignora_o_que_veio() {
    let d = DirTemp::novo("calculada");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    let r = t.inserir(&linha(1, "Ana", Value::Int(3))).unwrap();
    assert_eq!(t.ler(r).unwrap().unwrap()[DOBRO], Value::Int(6));
    // O merge do UPDATE devolve o valor velho da calculada junto com a linha
    // -- e o motor recalcula, em vez de recusar ou de acreditar.
    let mut l = t.ler(r).unwrap().unwrap();
    l[V] = Value::Int(10);
    assert_eq!(l[DOBRO], Value::Int(6), "o valor velho vem no merge");
    t.atualizar(r, &l).unwrap();
    assert_eq!(t.ler(r).unwrap().unwrap()[DOBRO], Value::Int(20));
    // Valor inventado na calculada tambem e ignorado: ela nao tem dado proprio.
    let r2 = t
        .inserir(&{
            let mut l = linha(2, "Bia", Value::Int(4));
            l[DOBRO] = Value::Int(999);
            l
        })
        .unwrap();
    assert_eq!(t.ler(r2).unwrap().unwrap()[DOBRO], Value::Int(8));
}

#[test]
fn o_indice_parcial_guarda_so_quem_passa_no_filtro_em_toda_porta() {
    let d = DirTemp::novo("parcial");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    let pequeno = t.inserir(&linha(1, "Ana", Value::Int(5))).unwrap();
    let grande = t.inserir(&linha(2, "Bia", Value::Int(12))).unwrap();
    let acha = |t: &mut Table, v: i64| t.buscar("so_grandes", &[Value::Int(v)]).unwrap();
    // O CONTROLE: a incluida aparece; a filtrada nao.
    assert_eq!(acha(&mut t, 12), vec![grande]);
    assert_eq!(acha(&mut t, 5), Vec::<u64>::new());
    // Sai do filtro no atualizar: a chave some do indice.
    let mut l = t.ler(grande).unwrap().unwrap();
    l[V] = Value::Int(2);
    t.atualizar(grande, &l).unwrap();
    assert_eq!(acha(&mut t, 2), Vec::<u64>::new());
    assert_eq!(acha(&mut t, 12), Vec::<u64>::new());
    // Entra no filtro no atualizar: a chave nasce no indice.
    let mut l = t.ler(pequeno).unwrap().unwrap();
    l[V] = Value::Int(50);
    t.atualizar(pequeno, &l).unwrap();
    assert_eq!(acha(&mut t, 50), vec![pequeno]);
    // Excluir de vez tira a chave sem reclamar de quem nunca esteve la.
    t.excluir_de_vez(grande, "teste").unwrap();
    t.excluir_de_vez(pequeno, "teste").unwrap();
    assert_eq!(acha(&mut t, 50), Vec::<u64>::new());
    // E o reindexar reconstroi respeitando o filtro.
    t.inserir(&linha(3, "Ciro", Value::Int(1))).unwrap();
    let r4 = t.inserir(&linha(4, "Dora", Value::Int(40))).unwrap();
    t.reindexar().unwrap();
    assert_eq!(acha(&mut t, 40), vec![r4]);
    assert_eq!(acha(&mut t, 1), Vec::<u64>::new());
}

#[test]
fn o_indice_por_expressao_acha_pelo_valor_transformado() {
    let d = DirTemp::novo("expressao");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    let ana = t.inserir(&linha(1, "Ana", Value::Int(5))).unwrap();
    let bia = t.inserir(&linha(2, "BIA", Value::Int(5))).unwrap();
    // Quem procura manda o valor ja baixo -- a chave de busca nao reavalia.
    assert_eq!(
        t.buscar("por_baixo", &[Value::Str("ana".into())]).unwrap(),
        vec![ana]
    );
    assert_eq!(
        t.buscar("por_baixo", &[Value::Str("bia".into())]).unwrap(),
        vec![bia]
    );
    assert_eq!(
        t.buscar("por_baixo", &[Value::Str("Ana".into())]).unwrap(),
        Vec::<u64>::new()
    );
    // Mudou o nome, mudou a chave.
    let mut l = t.ler(ana).unwrap().unwrap();
    l[NOME] = Value::Str("ANITA".into());
    t.atualizar(ana, &l).unwrap();
    assert_eq!(
        t.buscar("por_baixo", &[Value::Str("anita".into())])
            .unwrap(),
        vec![ana]
    );
    assert_eq!(
        t.buscar("por_baixo", &[Value::Str("ana".into())]).unwrap(),
        Vec::<u64>::new()
    );
    // E sobrevive ao reabrir: a expressao esta no PSCH.
    drop(t);
    let mut t = Table::abrir(&d.0, "t").unwrap();
    assert_eq!(
        t.buscar("por_baixo", &[Value::Str("bia".into())]).unwrap(),
        vec![bia]
    );
}

#[test]
fn a_replica_aplica_a_imagem_e_nao_julga() {
    let d = DirTemp::novo("replica");
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    // A origem aceitou v = -5? Nao aceitaria -- mas se a imagem chega assim,
    // a replica grava: recusar aqui e perder o evento e travar o par.
    let mut l = linha(1, "Ana", Value::Int(-5));
    l[DOBRO] = Value::Int(-10);
    l[PRECO] = Value::Decimal(-750);
    let r = t.inserir_replicado(&l).unwrap();
    let lida = t.ler(r).unwrap().unwrap();
    assert_eq!(lida[V], Value::Int(-5));
    assert_eq!(
        lida[DOBRO],
        Value::Int(-10),
        "a replica nao recalcula: a imagem manda"
    );
}
