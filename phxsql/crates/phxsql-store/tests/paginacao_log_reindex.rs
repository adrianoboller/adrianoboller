//! As pecas novas trabalhando juntas: tabela paginada, diario e reindex.

mod comum;

use comum::{DirTemp, Rng};

use phxsql_core::error::PhxError;
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::log::Operacao;
use phxsql_store::table::Table;

const ID: usize = 0;
const NOME: usize = 1;
const CIDADE: usize = 2;
const FICHA: usize = 3;

fn esquema(paginacao: Option<Paginacao>) -> Schema {
    let e = Schema::new(
        "cadastroClientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(ID)]).unico(),
            IndexDef::new("porNome", vec![IndexColumn::asc(NOME).sem_caixa()]),
        ],
    )
    .unwrap();
    match paginacao {
        Some(p) => e.com_paginacao(p),
        None => e,
    }
}

fn cliente(id: i64, nome: &str, cidade: &str) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(nome.into()),
        Value::Str(cidade.into()),
        Value::Null,
    ]
}

#[test]
fn tabela_paginada_de_ponta_a_ponta() {
    let dir = DirTemp::novo("pag-e2e");
    // 50 registros por arquivo, ate 99 arquivos.
    let pag = Paginacao::nova(50, 99).unwrap();
    let mut t = Table::criar(&dir.0, esquema(Some(pag))).unwrap();

    const N: i64 = 500;
    let mut ids: Vec<i64> = (1..=N).collect();
    Rng::nova(11).embaralhar(&mut ids);

    for id in &ids {
        let mut l = cliente(*id, &format!("Cliente {id:04}"), "Blumenau");
        if id % 25 == 0 {
            l[FICHA] = Value::Memo(format!("ficha longa do cliente {id} ").repeat(30));
        }
        t.inserir(&l).unwrap();
    }

    // 500 registros / 50 por arquivo = 10 volumes de .reg.
    let (vreg, _, _, _) = t.volumes_por_arquivo();
    assert_eq!(vreg.len(), 10, "volumes de .reg: {vreg:?}");
    assert!(dir.0.join("cadastroClientes_001.reg").exists());
    assert!(dir.0.join("cadastroClientes_010.reg").exists());
    // O .ndx nao pagina: continua sendo um arquivo so.
    assert!(dir.0.join("cadastroClientes.ndx").exists());

    // Toda linha volta certa, atravessando os dez volumes.
    for (i, id) in ids.iter().enumerate() {
        let rowid = i as u64 + 1;
        let l = t.ler(rowid).unwrap().unwrap();
        assert_eq!(l[ID], Value::Int(*id), "rowid {rowid}");
        assert_eq!(l[NOME], Value::Str(format!("Cliente {id:04}")));
    }

    // O indice continua funcionando: ele guarda rowid global.
    for id in &ids {
        assert_eq!(t.buscar("porId", &[Value::Int(*id)]).unwrap().len(), 1);
    }

    // A varredura do .reg sai na ordem de digitacao, cruzando volumes.
    let vivos = t.varrer().unwrap();
    assert_eq!(vivos.len(), N as usize);
    let ordem: Vec<i64> = vivos
        .iter()
        .map(|(_, l)| l[ID].como_i64().unwrap())
        .collect();
    assert_eq!(ordem, ids, "a ordem de digitacao se perdeu entre volumes");

    let rel = t.verificar().unwrap();
    assert_eq!(rel.registros, N as u64);
    assert_eq!(rel.volumes.0, 10);
    assert_eq!(rel.eventos, N as u64);
}

#[test]
fn diario_registra_as_tres_operacoes_com_data_e_hora() {
    let dir = DirTemp::novo("diario");
    let mut t = Table::criar(&dir.0, esquema(None)).unwrap();
    t.definir_usuario(7);

    let rowid = t.inserir(&cliente(1, "Antigo", "Blumenau")).unwrap();
    t.atualizar(rowid, &cliente(1, "Novo", "Blumenau")).unwrap();
    t.inserir(&cliente(2, "Outro", "Itajai")).unwrap();
    t.excluir(rowid).unwrap();

    assert!(dir.0.join("cadastroClientes.log").exists());
    assert_eq!(t.eventos().unwrap(), 4);

    let d = t.diario(0, 0).unwrap();
    assert_eq!(d.len(), 4);
    assert_eq!(d[0].operacao, Operacao::Inclusao);
    assert_eq!(d[1].operacao, Operacao::Alteracao);
    assert_eq!(d[2].operacao, Operacao::Inclusao);
    assert_eq!(d[3].operacao, Operacao::Exclusao);

    // A alteracao registra a versao nova do registro.
    assert_eq!(d[1].versao, 2);
    // O usuario definido assina os eventos.
    assert!(d.iter().all(|e| e.usuario == 7));
    // O carimbo e cronologico e vem com data e hora legiveis.
    assert!(d[0].carimbo <= d[3].carimbo);
    assert_eq!(d[0].instante_iso().len(), 23);

    // Historico de um registro so.
    let h = t.historico(rowid).unwrap();
    assert_eq!(h.len(), 3, "inclusao, alteracao e exclusao do rowid 1");
    assert!(h.iter().all(|e| e.rowid == rowid));

    // Excluir de novo nao gera evento.
    assert!(!t.excluir(rowid).unwrap());
    assert_eq!(t.eventos().unwrap(), 4);

    t.verificar().unwrap();
}

#[test]
fn insercao_recusada_nao_suja_o_diario() {
    let dir = DirTemp::novo("diario-recusa");
    let mut t = Table::criar(&dir.0, esquema(None)).unwrap();
    t.inserir(&cliente(1, "Primeiro", "Blumenau")).unwrap();
    assert!(t.inserir(&cliente(1, "Repetido", "Blumenau")).is_err());
    assert_eq!(t.eventos().unwrap(), 1, "a tentativa recusada nao e evento");
    t.verificar().unwrap();
}

#[test]
fn reindex_recria_o_ndx_do_zero() {
    let dir = DirTemp::novo("reindex");
    let mut t = Table::criar(&dir.0, esquema(None)).unwrap();
    for i in 1..=200i64 {
        t.inserir(&cliente(i, &format!("Cliente {i:03}"), "Blumenau"))
            .unwrap();
    }
    // Exclui um terco: a arvore fica com paginas subocupadas.
    for rowid in (1..=200u64).step_by(3) {
        t.excluir(rowid).unwrap();
    }
    let antes = t.verificar().unwrap();

    let indices = t.reindexar().unwrap();
    assert_eq!(indices.len(), 2);
    for (_, qtd) in &indices {
        assert_eq!(*qtd, antes.registros);
    }

    let depois = t.verificar().unwrap();
    assert_eq!(depois.registros, antes.registros);
    assert_eq!(depois.indices, indices);

    // As buscas continuam certas depois da reconstrucao.
    for i in 1..=200i64 {
        let rowid = i as u64;
        let esperado = if rowid % 3 == 1 { 0 } else { 1 };
        assert_eq!(
            t.buscar("porId", &[Value::Int(i)]).unwrap().len(),
            esperado,
            "id {i}"
        );
    }
}

#[test]
fn reindex_reconstroi_indice_apagado() {
    let dir = DirTemp::novo("reindex-perdido");
    {
        let mut t = Table::criar(&dir.0, esquema(None)).unwrap();
        for i in 1..=100i64 {
            t.inserir(&cliente(i, &format!("Cliente {i:03}"), "Itajai"))
                .unwrap();
        }
        t.sincronizar().unwrap();
    }

    // O .ndx some por completo -- disco cheio, cópia incompleta, o que for.
    std::fs::remove_file(dir.0.join("cadastroClientes.ndx")).unwrap();
    assert!(Table::abrir(&dir.0, "cadastroClientes").is_err());

    // Recriar o .ndx vazio e reindexar traz a tabela de volta.
    let esq = esquema(None);
    phxsql_store::ndx::NdxFile::criar(dir.0.join("cadastroClientes.ndx"), &esq).unwrap();
    let mut t = Table::abrir(&dir.0, "cadastroClientes").unwrap();
    assert_eq!(t.registros(), 100);
    assert!(t.buscar("porId", &[Value::Int(42)]).unwrap().is_empty());

    t.reindexar().unwrap();
    assert_eq!(t.buscar("porId", &[Value::Int(42)]).unwrap(), vec![42]);
    let rel = t.verificar().unwrap();
    assert_eq!(rel.registros, 100);
    for (_, qtd) in &rel.indices {
        assert_eq!(*qtd, 100);
    }
}

#[test]
fn reindex_em_tabela_paginada() {
    let dir = DirTemp::novo("reindex-pag");
    let pag = Paginacao::nova(20, 99).unwrap();
    let mut t = Table::criar(&dir.0, esquema(Some(pag))).unwrap();
    for i in 1..=150i64 {
        t.inserir(&cliente(i, &format!("Cliente {i:03}"), "Joinville"))
            .unwrap();
    }
    for rowid in (1..=150u64).step_by(7) {
        t.excluir(rowid).unwrap();
    }
    let antes = t.verificar().unwrap();
    t.reindexar().unwrap();
    let depois = t.verificar().unwrap();
    assert_eq!(depois.registros, antes.registros);
    assert_eq!(depois.volumes.0, 8, "150 / 20 = 8 volumes");
    for (_, qtd) in &depois.indices {
        assert_eq!(*qtd, antes.registros);
    }
}

#[test]
fn tabela_cheia_recusa_e_mantem_a_integridade() {
    let dir = DirTemp::novo("cheia");
    // Capacidade 6: 3 por arquivo, 2 arquivos.
    let pag = Paginacao::nova(3, 2).unwrap();
    let mut t = Table::criar(&dir.0, esquema(Some(pag))).unwrap();
    for i in 1..=6i64 {
        t.inserir(&cliente(i, &format!("C{i}"), "Blumenau"))
            .unwrap();
    }
    let e = t.inserir(&cliente(7, "C7", "Blumenau")).unwrap_err();
    assert!(matches!(e, PhxError::LimiteExcedido(_)), "erro foi {e}");
    assert_eq!(t.registros(), 6);
    assert_eq!(t.eventos().unwrap(), 6, "a recusa nao virou evento");
    t.verificar().unwrap();
}

#[test]
fn chave_estrangeira_sobrevive_ao_fecha_e_abre() {
    let dir = DirTemp::novo("fk");
    let esq = esquema(None)
        .com_chaves_estrangeiras(vec![ForeignKey::new(
            "fkCidade",
            vec![CIDADE],
            "geografia.cidades",
            vec!["nome".to_string()],
        )
        .ao_excluir(AcaoRi::Restringir)
        .ao_alterar(AcaoRi::Cascata)])
        .unwrap();

    {
        let mut t = Table::criar(&dir.0, esq.clone()).unwrap();
        t.inserir(&cliente(1, "Alguem", "Blumenau")).unwrap();
        t.sincronizar().unwrap();
    }

    let t = Table::abrir(&dir.0, "cadastroClientes").unwrap();
    let fks = t.esquema().chaves_estrangeiras();
    assert_eq!(fks.len(), 1);
    assert_eq!(fks[0].nome, "fkCidade");
    assert_eq!(fks[0].colunas, vec![CIDADE]);
    assert_eq!(fks[0].tabela_ref, "geografia.cidades");
    assert_eq!(fks[0].colunas_ref, vec!["nome".to_string()]);
    assert_eq!(fks[0].ao_excluir, AcaoRi::Restringir);
    assert_eq!(fks[0].ao_alterar, AcaoRi::Cascata);
    assert_eq!(t.esquema(), &esq);
}

#[test]
fn paginada_fecha_e_reabre_sem_saber_a_geometria() {
    let dir = DirTemp::novo("reabre-pag");
    let pag = Paginacao::nova(7, 99).unwrap();
    {
        let mut t = Table::criar(&dir.0, esquema(Some(pag))).unwrap();
        for i in 1..=40i64 {
            t.inserir(&cliente(i, &format!("C{i:03}"), "Itajai"))
                .unwrap();
        }
        t.sincronizar().unwrap();
    }
    // Abrir so recebe o diretorio e o nome: a paginacao vem do esquema, que
    // esta dentro do primeiro volume.
    let mut t = Table::abrir(&dir.0, "cadastroClientes").unwrap();
    assert_eq!(t.esquema().paginacao().registros_por_arquivo, 7);
    assert_eq!(t.registros(), 40);
    assert_eq!(t.volumes_por_arquivo().0.len(), 6);
    assert_eq!(t.ler(40).unwrap().unwrap()[ID], Value::Int(40));
    t.inserir(&cliente(41, "Depois", "Itajai")).unwrap();
    assert_eq!(t.registros(), 41);
    t.verificar().unwrap();
}
