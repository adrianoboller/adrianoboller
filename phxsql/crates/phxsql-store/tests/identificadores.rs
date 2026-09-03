//! UUID, identificador de 256 bits e sequencia, de ponta a ponta.
//!
//! Testar isto so no `core` nao bastaria: o que interessa e o id sobreviver a
//! volta inteira -- virar bytes no slot, virar chave no `.ndx`, voltar do
//! disco depois de fechar e abrir a tabela.

mod comum;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::uuid::{Uuid, Uuid256};
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn temp(nome: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("id-{nome}"))
}

fn esquema_ids() -> Schema {
    Schema::new(
        "eventos",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("hash", ColumnType::Uuid256),
            Column::new("ordem", ColumnType::Sequence),
            Column::new("titulo", ColumnType::Str(40)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porHash", vec![IndexColumn::asc(1)]),
            IndexDef::new("porOrdem", vec![IndexColumn::asc(2)]).unico(),
        ],
    )
    .expect("esquema com identificadores")
}

#[test]
fn uuid_vai_ao_disco_e_volta_igual() {
    let d = temp("volta");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();

    let id = Uuid::v7();
    let hash =
        Uuid256::de_texto("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            .unwrap();
    let rowid = t
        .inserir(&[
            Value::Uuid(id),
            Value::Uuid256(hash),
            Value::Null,
            Value::Str("bloco".into()),
        ])
        .unwrap();

    // Fecha e abre: o que importa e o que esta no arquivo, nao o que ficou na
    // memoria do processo.
    drop(t);
    let mut t = Table::abrir(&d, "eventos").unwrap();
    let linha = t.ler(rowid).unwrap().unwrap();
    assert_eq!(linha[0], Value::Uuid(id));
    assert_eq!(linha[1], Value::Uuid256(hash));
    assert_eq!(linha[2], Value::UInt(1), "a primeira sequencia e 1");
}

#[test]
fn uuid_acha_pelo_indice_unico() {
    let d = temp("indice");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();

    let mut ids = Vec::new();
    for i in 0..200 {
        let id = Uuid::v7();
        ids.push(id);
        t.inserir(&[
            Value::Uuid(id),
            Value::Null,
            Value::Null,
            Value::Str(format!("linha {i}")),
        ])
        .unwrap();
    }

    // Busca pontual por um id do meio.
    let achados = t.buscar("porId", &[Value::Uuid(ids[137])]).unwrap();
    assert_eq!(achados.len(), 1);
    let linha = t.ler(achados[0]).unwrap().unwrap();
    assert_eq!(linha[3], Value::Str("linha 137".into()));

    // Um id que nunca entrou nao acha nada.
    assert!(t
        .buscar("porId", &[Value::Uuid(Uuid::v7())])
        .unwrap()
        .is_empty());
}

#[test]
fn uuid_repetido_bate_no_indice_unico() {
    let d = temp("duplicado");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();
    let id = Uuid::v7();
    t.inserir(&[Value::Uuid(id), Value::Null, Value::Null, Value::Null])
        .unwrap();
    let erro = t.inserir(&[Value::Uuid(id), Value::Null, Value::Null, Value::Null]);
    assert!(erro.is_err(), "o mesmo UUID entrou duas vezes");
}

#[test]
fn v7_entra_no_indice_ja_ordenado() {
    // A razao de existir do v7 neste motor: a ordem do indice e a ordem de
    // criacao, entao percorrer o indice devolve os registros na ordem em que
    // foram gravados -- sem ordenar nada.
    let d = temp("ordem");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();

    let mut ids = Vec::new();
    for i in 0..300 {
        let id = Uuid::v7();
        ids.push(id);
        t.inserir(&[
            Value::Uuid(id),
            Value::Null,
            Value::Null,
            Value::Str(format!("{i}")),
        ])
        .unwrap();
    }

    let pelo_indice = t.varrer_indice("porId").unwrap();
    assert_eq!(pelo_indice.len(), 300);
    for (esperado, rowid) in ids.iter().zip(pelo_indice.iter()) {
        let linha = t.ler(*rowid).unwrap().unwrap();
        assert_eq!(
            linha[0],
            Value::Uuid(*esperado),
            "o indice devolveu fora da ordem de criacao"
        );
    }
}

#[test]
fn sequencia_conta_sozinha_e_nao_reaproveita() {
    let d = temp("sequencia");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();

    for _ in 0..5 {
        t.inserir(&[
            Value::Uuid(Uuid::v7()),
            Value::Null,
            Value::Null,
            Value::Null,
        ])
        .unwrap();
    }
    let numeros: Vec<u64> = (1..=5)
        .map(|r| match t.ler(r).unwrap().unwrap()[2] {
            Value::UInt(n) => n,
            ref outro => panic!("esperado numero, veio {outro:?}"),
        })
        .collect();
    assert_eq!(numeros, vec![1, 2, 3, 4, 5]);

    // Excluir NAO devolve o numero: a proxima linha continua de onde parou.
    t.excluir(3).unwrap();
    t.excluir(4).unwrap();
    let novo = t
        .inserir(&[
            Value::Uuid(Uuid::v7()),
            Value::Null,
            Value::Null,
            Value::Null,
        ])
        .unwrap();
    assert_eq!(t.ler(novo).unwrap().unwrap()[2], Value::UInt(6));
}

#[test]
fn sequencia_escrita_a_mao_empurra_o_contador() {
    // O defeito que este teste existe para impedir: gravar 500 na mao e depois
    // deixar o motor numerar, e ele devolver 1 -- por cima do que ja existe.
    let d = temp("empurra");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();

    t.inserir(&[
        Value::Uuid(Uuid::v7()),
        Value::Null,
        Value::UInt(500),
        Value::Null,
    ])
    .unwrap();
    let seguinte = t
        .inserir(&[
            Value::Uuid(Uuid::v7()),
            Value::Null,
            Value::Null,
            Value::Null,
        ])
        .unwrap();
    assert_eq!(t.ler(seguinte).unwrap().unwrap()[2], Value::UInt(501));
}

#[test]
fn sequencia_sobrevive_a_fechar_e_abrir() {
    let d = temp("persiste");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();
    for _ in 0..3 {
        t.inserir(&[
            Value::Uuid(Uuid::v7()),
            Value::Null,
            Value::Null,
            Value::Null,
        ])
        .unwrap();
    }
    t.sincronizar().unwrap();
    drop(t);

    let mut t = Table::abrir(&d, "eventos").unwrap();
    assert_eq!(t.sequencia_atual(), 4, "o contador voltou atras");
    let novo = t
        .inserir(&[
            Value::Uuid(Uuid::v7()),
            Value::Null,
            Value::Null,
            Value::Null,
        ])
        .unwrap();
    assert_eq!(t.ler(novo).unwrap().unwrap()[2], Value::UInt(4));
}

#[test]
fn alterar_nao_renumera_a_linha() {
    // A sequencia identifica a linha. Se uma alteracao com nulo gerasse numero
    // novo, a identidade mudaria por baixo de quem guardou o numero.
    let d = temp("alterar");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();
    let id = Uuid::v7();
    let r = t
        .inserir(&[
            Value::Uuid(id),
            Value::Null,
            Value::Null,
            Value::Str("antes".into()),
        ])
        .unwrap();
    assert_eq!(t.ler(r).unwrap().unwrap()[2], Value::UInt(1));

    t.atualizar(
        r,
        &[
            Value::Uuid(id),
            Value::Null,
            Value::Null,
            Value::Str("depois".into()),
        ],
    )
    .unwrap();

    let linha = t.ler(r).unwrap().unwrap();
    assert_eq!(linha[2], Value::UInt(1), "a alteracao renumerou a linha");
    assert_eq!(linha[3], Value::Str("depois".into()));
}

#[test]
fn duas_sequencias_na_mesma_tabela_sao_recusadas() {
    // Elas dividiriam o mesmo contador do cabecalho, o que so pareceria um
    // defeito. Melhor recusar o esquema.
    let erro = Schema::new(
        "duas",
        vec![
            Column::new("a", ColumnType::Sequence),
            Column::new("b", ColumnType::Sequence),
        ],
        vec![],
    );
    assert!(erro.is_err());
}

#[test]
fn hash_de_256_bits_indexa_e_acha() {
    let d = temp("hash256");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();

    let mut hashes = Vec::new();
    for i in 0..50 {
        let h = Uuid256::aleatorio();
        hashes.push(h);
        t.inserir(&[
            Value::Uuid(Uuid::v7()),
            Value::Uuid256(h),
            Value::Null,
            Value::Str(format!("bloco {i}")),
        ])
        .unwrap();
    }

    let achados = t.buscar("porHash", &[Value::Uuid256(hashes[30])]).unwrap();
    assert_eq!(achados.len(), 1);
    assert_eq!(
        t.ler(achados[0]).unwrap().unwrap()[3],
        Value::Str("bloco 30".into())
    );
}

#[test]
fn id_em_texto_tambem_entra() {
    // E como o id chega pelo protocolo: uma string.
    let d = temp("texto");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();
    let id = Uuid::v7();
    let r = t
        .inserir(&[
            Value::Str(id.to_string()),
            Value::Str(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
            ),
            Value::Null,
            Value::Null,
        ])
        .unwrap();

    let linha = t.ler(r).unwrap().unwrap();
    assert_eq!(linha[0], Value::Uuid(id), "o texto nao virou o mesmo id");
    // E a busca pelo indice acha usando texto tambem.
    let achados = t.buscar("porId", &[Value::Str(id.to_string())]).unwrap();
    assert_eq!(achados, vec![r]);
}

#[test]
fn reindex_reconstroi_os_indices_de_id() {
    // O reindex le o `.reg` e remonta o `.ndx` do zero. Se a chave de um UUID
    // nao for reproduzida igual, os ids somem do indice sem aviso.
    let d = temp("reindex");
    let mut t = Table::criar(&d, esquema_ids()).unwrap();
    let mut ids = Vec::new();
    for i in 0..80 {
        let id = Uuid::v7();
        ids.push(id);
        t.inserir(&[
            Value::Uuid(id),
            Value::Uuid256(Uuid256::aleatorio()),
            Value::Null,
            Value::Str(format!("{i}")),
        ])
        .unwrap();
    }
    t.sincronizar().unwrap();
    t.reindexar().unwrap();

    for (i, id) in ids.iter().enumerate() {
        let achados = t.buscar("porId", &[Value::Uuid(*id)]).unwrap();
        assert_eq!(achados.len(), 1, "id {i} sumiu do indice depois do reindex");
    }
    // A sequencia tambem: 80 linhas, numeros 1..80, todos no indice unico.
    assert_eq!(t.buscar("porOrdem", &[Value::UInt(80)]).unwrap().len(), 1);
}
