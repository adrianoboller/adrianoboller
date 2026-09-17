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

// ------------------------------------------------------------------------
// O `Uuid` que nasce sozinho -- §B.2.5 do `docs/AUTONUMBER.md`
//
// A assimetria que estes testes fecham estava medida: `Sequence` nula ganhava
// numero e `Uuid` nulo dava erro. O que eles travam nao e' so o caso novo --
// sao os QUATRO casos vizinhos que tem de continuar exatamente como estavam,
// porque e' neles que mora o estrago de uma guarda imposta em vez de pedida.

use phxsql_core::schema::ForeignKey;

/// Tabela com a identidade `Uuid` declarada: chave primaria de coluna unica.
fn esquema_com_identidade(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria()],
    )
    .expect("esquema com identidade Uuid")
}

#[test]
fn uuid_primario_nulo_nasce_sozinho() {
    let d = temp("nasce");
    let mut t = Table::criar(&d, esquema_com_identidade("clientes")).unwrap();

    let a = t
        .inserir(&[Value::Null, Value::Str("primeira".into())])
        .unwrap();
    let b = t
        .inserir(&[Value::Null, Value::Str("segunda".into())])
        .unwrap();

    // Fecha e abre: o que vale e' o que foi ao disco, nao o que ficou na
    // memoria do processo.
    drop(t);
    let mut t = Table::abrir(&d, "clientes").unwrap();

    let Value::Uuid(ua) = t.ler(a).unwrap().unwrap()[0] else {
        panic!("a identidade da primeira linha nao virou Uuid");
    };
    let Value::Uuid(ub) = t.ler(b).unwrap().unwrap()[0] else {
        panic!("a identidade da segunda linha nao virou Uuid");
    };
    assert!(!ua.e_nulo() && !ub.e_nulo(), "nasceu, mas nasceu zerado");
    assert_ne!(ua, ub, "duas linhas com a mesma identidade");
    assert_eq!(ua.versao(), 7, "a identidade tem de ser v7, e nao v4");
    assert!(
        ub > ua,
        "v7 e' crescente: a segunda linha tem de vir depois"
    );

    // E a identidade gerada esta NO INDICE -- ela entrou antes das chaves,
    // senao a chave gravada seria a do nulo.
    assert_eq!(t.buscar("porId", &[Value::Uuid(ua)]).unwrap(), vec![a]);
    assert_eq!(t.buscar("porId", &[Value::Uuid(ub)]).unwrap(), vec![b]);
}

#[test]
fn uuid_escolhido_a_mao_passa_intacto() {
    // O irmao do «nasce sozinho»: quem manda o id continua mandando o id.
    let d = temp("a-mao");
    let mut t = Table::criar(&d, esquema_com_identidade("clientes")).unwrap();
    let meu = Uuid::v7();
    let r = t
        .inserir(&[Value::Uuid(meu), Value::Str("escolhida".into())])
        .unwrap();
    assert_eq!(t.ler(r).unwrap().unwrap()[0], Value::Uuid(meu));
}

#[test]
fn alterar_com_uuid_nulo_mantem_a_identidade() {
    // O caso que seria o PIOR estrago: um `atualizar` que chega com o id nulo
    // -- porque a tela mandou a linha inteira e o cliente nao preencheu --
    // NAO pode ganhar identidade nova. Trocar a identidade no meio da vida da
    // linha e' a mesma regra que impede a `Sequence` de se renumerar.
    let d = temp("altera");
    let mut t = Table::criar(&d, esquema_com_identidade("clientes")).unwrap();
    let r = t
        .inserir(&[Value::Null, Value::Str("antes".into())])
        .unwrap();
    let antes = t.ler(r).unwrap().unwrap()[0].clone();

    t.atualizar(r, &[Value::Null, Value::Str("depois".into())])
        .unwrap();
    let depois = t.ler(r).unwrap().unwrap();
    assert_eq!(antes, depois[0], "a identidade mudou numa alteracao");
    assert_eq!(depois[1], Value::Str("depois".into()));
}

#[test]
fn uuid_que_nao_e_identidade_declarada_continua_recusando() {
    // O TESTE DO COMPORTAMENTO VELHO, e e' o que mais importa: tabela sem
    // chave primaria marcada continua recebendo o erro de coluna obrigatoria.
    // Quem usa esse erro para pegar «esqueci de preencher» continua pegando.
    let d = temp("sem-pk");
    let esq = Schema::new(
        "eventos",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        // `unico`, e nao `primaria`: a declaracao da identidade e' a marca de
        // primaria, e sem ela nada muda.
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut t = Table::criar(&d, esq).unwrap();
    let e = t
        .inserir(&[Value::Null, Value::Str("x".into())])
        .expect_err("uuid nulo sem chave primaria tinha de recusar");
    assert!(
        e.to_string().contains("obrigatoria") && e.to_string().contains("id"),
        "o erro velho tem de continuar nomeando a coluna, veio {e}"
    );
}

#[test]
fn uuid_que_aceita_nulo_continua_nulo() {
    // O outro comportamento velho: coluna `Uuid` que ACEITA nulo guarda nulo.
    // Gerar ali seria tirar de quem ja grava o direito de dizer «nao sei» --
    // e essa gravacao FUNCIONA hoje, que e' exatamente o cliente que uma
    // guarda imposta quebraria.
    let d = temp("nulavel");
    let esq = Schema::new(
        "anexos",
        vec![
            Column::new("ordem", ColumnType::Sequence).obrigatoria(),
            Column::new("origem", ColumnType::Uuid),
        ],
        vec![IndexDef::new("porOrdem", vec![IndexColumn::asc(0)]).primaria()],
    )
    .unwrap();
    let mut t = Table::criar(&d, esq).unwrap();
    let r = t.inserir(&[Value::Null, Value::Null]).unwrap();
    let linha = t.ler(r).unwrap().unwrap();
    assert_eq!(linha[0], Value::UInt(1), "a sequencia continua numerando");
    assert_eq!(linha[1], Value::Null, "o Uuid nulavel ganhou valor sozinho");
}

#[test]
fn uuid_de_referencia_nao_nasce_sozinho() {
    // A PETREA: «so existe filho se o pai existir primeiro». Numa tabela
    // 1-para-1 a chave primaria da filha TAMBEM aponta para a mae. Gerar um
    // v7 ali inventaria um pai -- e inventaria DEPOIS da conferencia, que
    // roda antes daqui e deixa o nulo passar. A orfa entraria sem ninguem ver.
    let d = temp("referencia");
    let mae = Schema::new(
        "pessoas",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria()],
    )
    .unwrap();
    Table::criar(&d, mae).unwrap();

    let filha = Schema::new(
        "fisicas",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("cpf", ColumnType::Str(11)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria()],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_pessoa",
        vec![0],
        "pessoas",
        vec!["id".into()],
    )])
    .unwrap();
    let mut t = Table::criar(&d, filha).unwrap();

    let e = t
        .inserir(&[Value::Null, Value::Str("00000000000".into())])
        .expect_err("a primaria que tambem e' referencia nao pode se inventar");
    assert!(
        e.to_string().contains("obrigatoria"),
        "tinha de cair no erro velho de coluna obrigatoria, veio {e}"
    );
    // E o estrago que o teste mede: NENHUMA linha entrou.
    assert_eq!(t.contar(phxsql_store::table::Visao::Todas).unwrap(), 0);
}

#[test]
fn a_replica_nao_gera_identidade() {
    // A replica COPIA o que veio. Gerar aqui daria dois ids para a mesma
    // linha -- um no source e outro aqui --, que e' o pior estrago possivel
    // numa coluna que e' identidade. Um nulo que chegue pela replicacao e'
    // divergencia, e divergencia se ANUNCIA.
    let d = temp("replica");
    let mut t = Table::criar(&d, esquema_com_identidade("clientes")).unwrap();

    let e = t
        .inserir_replicado(&[Value::Null, Value::Str("do source".into())])
        .expect_err("a replica gerou identidade propria");
    assert!(
        e.to_string().contains("obrigatoria"),
        "a replica tinha de recusar pelo erro de coluna obrigatoria, veio {e}"
    );
    assert_eq!(t.contar(phxsql_store::table::Visao::Todas).unwrap(), 0);

    // E o que a replica RECEBE preenchido entra igualzinho.
    let do_source = Uuid::v7();
    let r = t
        .inserir_replicado(&[Value::Uuid(do_source), Value::Str("do source".into())])
        .unwrap();
    assert_eq!(t.ler(r).unwrap().unwrap()[0], Value::Uuid(do_source));
}

#[test]
fn sequencia_e_identidade_na_mesma_tabela() {
    // As duas moram na MESMA funcao (`numerar`) para ninguem levar uma e
    // esquecer a outra. Este teste e' o que prova que a fusao nao perdeu
    // nenhuma das duas -- inclusive o EMPURRAO do contador da sequencia, que
    // e' o ramo que nao copia a linha.
    let d = temp("as-duas");
    let esq = Schema::new(
        "notas",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("numero", ColumnType::Sequence),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria(),
            IndexDef::new("porNumero", vec![IndexColumn::asc(1)]).unico(),
        ],
    )
    .unwrap();
    let mut t = Table::criar(&d, esq).unwrap();

    let a = t.inserir(&[Value::Null, Value::Null]).unwrap();
    assert_eq!(t.ler(a).unwrap().unwrap()[1], Value::UInt(1));

    // Sequencia escolhida a mao EMPURRA o contador, e a identidade nasce do
    // mesmo jeito.
    let b = t.inserir(&[Value::Null, Value::UInt(500)]).unwrap();
    let linha_b = t.ler(b).unwrap().unwrap();
    assert_eq!(linha_b[1], Value::UInt(500));
    assert!(matches!(linha_b[0], Value::Uuid(u) if !u.e_nulo()));

    let c = t.inserir(&[Value::Null, Value::Null]).unwrap();
    assert_eq!(
        t.ler(c).unwrap().unwrap()[1],
        Value::UInt(501),
        "o empurrao do contador se perdeu na fusao das duas regras"
    );
}
