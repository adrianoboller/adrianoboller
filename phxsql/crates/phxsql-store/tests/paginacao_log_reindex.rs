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
        Some(p) => e.com_paginacao(p).unwrap(),
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

// ===================================================================
// Particao por periodo
// ===================================================================

use phxsql_core::paginacao::{ModoParticao, Periodo};

fn dias(ano: i32, mes: u32, dia: u32) -> i32 {
    phxsql_core::datahora::dias_de_civil(ano, mes, dia)
}

/// Tabela com uma data obrigatoria e uma descricao, particionada por periodo.
fn tabela_por_periodo(dir: &std::path::Path, periodo: Periodo, teto: u64) -> Table {
    let esquema = Schema::new(
        "lancamentos",
        vec![
            Column::new("quando", ColumnType::Date).obrigatoria(),
            Column::new("texto", ColumnType::Str(20)),
        ],
        vec![],
    )
    .unwrap()
    .com_paginacao(
        Paginacao::nova(teto, 99)
            .unwrap()
            .com_modo(ModoParticao::PorPeriodo { coluna: 0, periodo })
            .unwrap(),
    )
    .unwrap();
    Table::criar(dir, esquema).unwrap()
}

#[test]
fn volume_corta_quando_o_mes_vira() {
    let dir = DirTemp::novo("periodo-mes");
    let mut t = tabela_por_periodo(dir.0.as_path(), Periodo::Mensal, 1000);

    // Tres meses, com quantidades diferentes: 2 em janeiro, 1 em fevereiro,
    // 3 em marco. Nenhum volume enche -- so o calendario corta.
    let linhas = [
        (2026, 1, "jan a"),
        (2026, 1, "jan b"),
        (2026, 2, "fev a"),
        (2026, 3, "mar a"),
        (2026, 3, "mar b"),
        (2026, 3, "mar c"),
    ];
    for (a, m, txt) in linhas {
        t.inserir(&[Value::Date(dias(a, m, 15)), Value::Str(txt.into())])
            .unwrap();
    }

    let mut vols: Vec<String> = std::fs::read_dir(dir.0.as_path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".reg"))
        .collect();
    vols.sort();
    assert_eq!(
        vols,
        vec![
            "lancamentos_001.reg".to_string(),
            "lancamentos_002.reg".to_string(),
            "lancamentos_003.reg".to_string()
        ],
        "tres meses tinham de dar tres volumes"
    );

    // E a ordem de digitacao continua de pe, atravessando os volumes.
    let lidos: Vec<String> = (1..=6)
        .map(|r| match &t.ler(r).unwrap().unwrap()[1] {
            Value::Str(s) => s.clone(),
            outro => panic!("esperava texto, veio {outro:?}"),
        })
        .collect();
    assert_eq!(
        lidos,
        vec!["jan a", "jan b", "fev a", "mar a", "mar b", "mar c"]
    );
}

#[test]
fn linha_atrasada_fica_no_volume_corrente() {
    // A regra que define o desenho: a ordem de digitacao manda. Um lancamento
    // de janeiro digitado depois de um de marco NAO volta para o volume de
    // janeiro -- isso seria escrever no meio de um arquivo ja fechado.
    let dir = DirTemp::novo("periodo-atrasada");
    let mut t = tabela_por_periodo(dir.0.as_path(), Periodo::Mensal, 1000);

    t.inserir(&[Value::Date(dias(2026, 1, 10)), Value::Str("jan".into())])
        .unwrap();
    t.inserir(&[Value::Date(dias(2026, 3, 10)), Value::Str("mar".into())])
        .unwrap();
    let atrasada = t
        .inserir(&[
            Value::Date(dias(2026, 1, 31)),
            Value::Str("jan tarde".into()),
        ])
        .unwrap();

    assert_eq!(atrasada, 3, "o rowid continua sequencial");
    // Voltou para janeiro? Se tivesse voltado, existiriam so dois volumes e o
    // de janeiro teria tres slots. Ela ficou no corrente: tres volumes.
    let quantos = std::fs::read_dir(dir.0.as_path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".reg"))
        .count();
    assert_eq!(quantos, 3, "a atrasada abriu um volume, nao voltou");

    match &t.ler(3).unwrap().unwrap()[1] {
        Value::Str(s) => assert_eq!(s, "jan tarde"),
        outro => panic!("esperava texto, veio {outro:?}"),
    }
}

#[test]
fn o_teto_de_registros_corta_antes_do_periodo_virar() {
    // `registros_por_arquivo` continua sendo teto: um mes movimentado nao pode
    // estourar o arquivo so porque o calendario nao virou.
    let dir = DirTemp::novo("periodo-teto");
    let mut t = tabela_por_periodo(dir.0.as_path(), Periodo::Anual, 3);

    for i in 0..7 {
        t.inserir(&[
            Value::Date(dias(2026, 5, 1 + i)),
            Value::Str(format!("l{i}")),
        ])
        .unwrap();
    }
    let quantos = std::fs::read_dir(dir.0.as_path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".reg"))
        .count();
    assert_eq!(
        quantos, 3,
        "7 linhas com teto 3 dao 3 volumes, no mesmo ano"
    );

    for i in 0..7u64 {
        match &t.ler(i + 1).unwrap().unwrap()[1] {
            Value::Str(s) => assert_eq!(s, &format!("l{i}")),
            outro => panic!("esperava texto, veio {outro:?}"),
        }
    }
}

#[test]
fn as_fronteiras_sobrevivem_a_fechar_e_abrir() {
    // O endereco de um rowid depende da tabela de fronteiras, e ela e remontada
    // lendo o cabecalho de cada volume. Se isso nao funcionar, reabrir a tabela
    // devolve a LINHA ERRADA -- e em silencio, que e o pior jeito.
    let dir = DirTemp::novo("periodo-reabrir");
    {
        let mut t = tabela_por_periodo(dir.0.as_path(), Periodo::Mensal, 1000);
        for (m, n) in [(1u32, 2), (2, 1), (3, 4)] {
            for i in 0..n {
                t.inserir(&[
                    Value::Date(dias(2026, m, 10)),
                    Value::Str(format!("m{m}-{i}")),
                ])
                .unwrap();
            }
        }
        t.sincronizar().unwrap();
    }

    let mut t = Table::abrir(dir.0.as_path(), "lancamentos").unwrap();
    let esperado = ["m1-0", "m1-1", "m2-0", "m3-0", "m3-1", "m3-2", "m3-3"];
    for (i, texto) in esperado.iter().enumerate() {
        match &t.ler(i as u64 + 1).unwrap().unwrap()[1] {
            Value::Str(s) => assert_eq!(s, texto, "rowid {} veio errado", i + 1),
            outro => panic!("esperava texto, veio {outro:?}"),
        }
    }

    // E continua anexando no volume certo depois de reabrir.
    let novo = t
        .inserir(&[Value::Date(dias(2026, 4, 1)), Value::Str("m4-0".into())])
        .unwrap();
    assert_eq!(novo, 8);
    match &t.ler(8).unwrap().unwrap()[1] {
        Value::Str(s) => assert_eq!(s, "m4-0"),
        outro => panic!("esperava texto, veio {outro:?}"),
    }
}

#[test]
fn bimestre_e_semestre_agrupam_os_meses_certos() {
    let dir = DirTemp::novo("periodo-bimestre");
    let mut t = tabela_por_periodo(dir.0.as_path(), Periodo::Bimestral, 1000);
    // jan e fev sao o mesmo bimestre; marco abre o proximo.
    for (m, txt) in [(1u32, "jan"), (2, "fev"), (3, "mar")] {
        t.inserir(&[Value::Date(dias(2026, m, 5)), Value::Str(txt.into())])
            .unwrap();
    }
    let quantos = std::fs::read_dir(dir.0.as_path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".reg"))
        .count();
    assert_eq!(quantos, 2, "jan+fev num volume, mar noutro");
}

#[test]
fn particao_por_periodo_exige_coluna_de_data_obrigatoria() {
    let com = |colunas: Vec<Column>, coluna: u16| {
        Schema::new("t", colunas, vec![]).unwrap().com_paginacao(
            Paginacao::nova(100, 99)
                .unwrap()
                .com_modo(ModoParticao::PorPeriodo {
                    coluna,
                    periodo: Periodo::Mensal,
                })
                .unwrap(),
        )
    };

    // Coluna que nao existe.
    assert!(com(vec![Column::new("a", ColumnType::Int8).obrigatoria()], 9).is_err());
    // Coluna que existe mas nao e data.
    assert!(com(vec![Column::new("a", ColumnType::Int8).obrigatoria()], 0).is_err());
    // Data, mas aceita nulo: sem data nao ha periodo em que a linha caiba.
    assert!(com(vec![Column::new("a", ColumnType::Date)], 0).is_err());
    // Data obrigatoria: passa.
    assert!(com(vec![Column::new("a", ColumnType::Date).obrigatoria()], 0).is_ok());
    // DateTime tambem serve.
    assert!(com(
        vec![Column::new("a", ColumnType::DateTime).obrigatoria()],
        0
    )
    .is_ok());
}
