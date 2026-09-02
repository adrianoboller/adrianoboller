//! A cascata que a RECUPERACAO nao refazia -- e o relatorio dizendo que refez.
//!
//! A Frente A entregou o `ao_alterar: cascata` executando, e apontou um risco
//! que sobrou: as escritas da cascata nao entram no conjunto de escrita nem na
//! marca `.tx`. Medi antes de aceitar o tamanho do buraco, e ele era MENOR e
//! PIOR do que o relato dizia.
//!
//! **Menor:** dentro da transacao o comportamento observavel ja estava certo.
//! Pela sonda por soquete, a cascata empilha, o `ROLLBACK` nao deixa nada, e o
//! `COMMIT` a executa. Nao havia vazamento nenhum.
//!
//! **Pior:** o que nao se refazia era a REAPLICACAO. `aplicar_uma` reaplica a
//! alteracao por `Table::atualizar`, que e o metodo que carrega a cascata --
//! e foi dai que eu conclui, LENDO, que a recuperacao se consertava sozinha.
//! Medido, nao se consertava: a cascata e planejada pelo DELTA da mae
//! (`planejar_ao_alterar` compara antes e depois), e na reaplicacao a mae ja
//! esta no valor de destino. `antes == depois` -- plano vazio, filha para
//! tras. E a recuperacao devolvia `Ok`, contava a operacao em `reaplicadas` e
//! nao punha nada em `impossiveis`: **o relatorio do arranque dizia que o
//! commit foi completado.**
//!
//! O conserto e a marca `.tx` **v2**, que guarda a linha ANTIGA do
//! `atualizar` -- de graca, porque o servidor ja a le do disco ao empilhar --
//! e a recuperacao replaneja a cascata com ela por `Table::recascatear`.
//!
//! O que estes testes travam:
//!
//! 1. a orfa que nasce **sem queda nenhuma**, a tres niveis de cascata. Ela
//!    continua acontecendo, e esta aqui escrita em vez de escondida;
//! 2. a recuperacao refazendo a cascata que a queda deixou pela metade;
//! 3. a prova real do outro sentido, que e o que impede o 2 de passar por
//!    engano;
//! 4. a marca **v1** continuando a ser lida, porque descartar marca velha e
//!    jogar fora transacao confirmada por causa de uma mudanca nossa.

use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::transacao::{recuperar, Acao, Escrita};
use phxsql_store::catalogo::{Database, Instancia};
use phxsql_store::table::Table;

fn base(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-casc-rec-{rotulo}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `clientes`: a mae. `id` indexado, porque a cascata exige indice dos dois
/// lados e sem ele o motor recusa antes de gravar.
fn clientes(db: &Database) -> Table {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    db.criar_tabela(None, e).unwrap()
}

/// `pedidos`: filha de `clientes` com `ao_alterar: cascata`.
fn pedidos(db: &Database) -> Table {
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
    .ao_alterar(AcaoRi::Cascata)])
    .unwrap();
    db.criar_tabela(None, e).unwrap()
}

/// `itens`: NETA. Aponta para a coluna que a cascata da mae vai mexer, e
/// declara `ao_alterar: restringir` -- que e uma declaracao legitima.
fn itens(db: &Database) -> Table {
    let e = Schema::new(
        "itens",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("pedido_cliente", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porPc", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_pc",
        vec![1],
        "pedidos",
        vec!["cliente_id".into()],
    )
    .ao_alterar(AcaoRi::Restringir)])
    .unwrap();
    db.criar_tabela(None, e).unwrap()
}

/// O `cliente_id` da linha `r` de `pedidos`, RELIDO do disco.
fn filha_aponta_para(db: &Database, r: u64) -> Value {
    let mut t = db.abrir_qualificada("pedidos").unwrap();
    t.ler(r).unwrap().unwrap()[1].clone()
}

/// A marca `.tx` que a passada teria deixado: um `atualizar` de `clientes`
/// levando a chave para 7, com a linha ANTIGA que a v2 carrega.
fn marca(db: &Database, id: u64, rowid: u64, antiga: &[Value]) {
    phxsql_server::transacao::gravar_marca(
        db.caminho(),
        id,
        0,
        &[Escrita {
            database: "t".into(),
            tabela: "clientes".into(),
            acao: Acao::Atualizar,
            rowid,
            linha: vec![Value::Int(7), Value::Str("Ana".into())],
            linha_antiga: antiga.to_vec(),
            motivo: String::new(),
        }],
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// 1. A orfa que nao precisa de queda nenhuma
// ---------------------------------------------------------------------------

/// Tres niveis bastam: a mae grava, a cascata recusa, e ninguem desfaz.
///
/// `planejar_ao_alterar` de `clientes` so olha quem aponta para `clientes` --
/// ve `pedidos` com cascata, aprova, e a mae vai para o disco. So entao a
/// cascata chama `pedidos.atualizar`, que planeja a PROPRIA cascata, acha
/// `itens` com `restringir` e recusa. A mae ficou em 7 e a filha em 1.
///
/// Prova real: com `itens` fora do caminho (o segundo teste tira a linha), a
/// mesma chamada leva a filha para 7. Se este teste passar por acaso, aquele
/// falha.
#[test]
fn a_cascata_de_tres_niveis_recusa_depois_de_gravar_a_mae() {
    let b = base("orfa");
    let inst = Instancia::nova(&b).unwrap();
    let db = inst.criar_database("t").unwrap();
    let mut m = clientes(&db);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = pedidos(&db);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    let mut n = itens(&db);
    n.inserir(&[Value::Int(100), Value::Int(1)]).unwrap();
    n.sincronizar().unwrap();

    let saida = m.atualizar(r, &[Value::Int(7), Value::Str("Ana".into())]);
    let erro = saida.expect_err("a cascata de tres niveis passou -- o cenario mudou");
    let texto = erro.to_string();
    assert!(
        texto.contains("a mae ja esta gravada"),
        "o erro tem de DIZER que a mae ficou gravada; veio: {texto}"
    );

    // O estado em disco: a mae andou, a filha nao. Nenhuma queda envolvida.
    let mut m = db.abrir_qualificada("clientes").unwrap();
    assert_eq!(
        m.ler(r).unwrap().unwrap()[0],
        Value::Int(7),
        "a mae devia estar gravada com a chave nova"
    );
    assert_eq!(
        filha_aponta_para(&db, p),
        Value::Int(1),
        "a filha devia ter ficado para tras -- e este e o defeito"
    );
}

// ---------------------------------------------------------------------------
// 2. A reaplicacao que nao conserta, e o relatorio que diz que consertou
// ---------------------------------------------------------------------------

/// O estado que a queda no meio da passada deixa, e o que a recuperacao faz
/// com ele.
///
/// O teste 1 produz, pela API publica e sem matar processo nenhum, o MESMO
/// estado em disco que um `SIGKILL` entre a gravacao da mae e a cascata: mae
/// nova, filha velha, e nenhum rastro da cascata pendente. Tirada a neta que
/// travava o caminho, escreve-se a marca `.tx` que a passada teria deixado e
/// chama-se a recuperacao de verdade.
///
/// Prova real: tirar a chamada a `recascatear` do `aplicar_uma` devolve
/// `Int(1)` na ultima linha, que e exatamente o defeito medido. E o teste 3 e
/// a MESMA marca com a mae ainda no valor velho -- sem ele, este passaria
/// tambem numa sonda que nao enxerga cascata nenhuma.
#[test]
fn a_recuperacao_refaz_a_cascata_que_a_queda_deixou_pela_metade() {
    let b = base("reaplica");
    let inst = Instancia::nova(&b).unwrap();
    let db = inst.criar_database("t").unwrap();
    let mut m = clientes(&db);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = pedidos(&db);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    let mut n = itens(&db);
    let i = n.inserir(&[Value::Int(100), Value::Int(1)]).unwrap();
    n.sincronizar().unwrap();

    // Produz o estado pos-queda e depois tira o estorvo: o que a recuperacao
    // ve e so o disco, e o disco ficou igual ao de uma queda.
    m.atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect_err("cenario");
    n.excluir_de_vez(i, "sai do caminho: a queda nao tem neta")
        .unwrap();
    n.sincronizar().unwrap();
    assert_eq!(
        filha_aponta_para(&db, p),
        Value::Int(1),
        "o cenario mudou: a filha ja devia estar para tras antes da recuperacao"
    );
    drop(m);
    drop(f);
    drop(n);

    marca(&db, 4242, r, &[Value::Int(1), Value::Str("Ana".into())]);
    let rel = recuperar(&inst);

    assert_eq!(rel.achadas, 1);
    assert_eq!(rel.completadas, 1);
    assert_eq!(rel.reaplicadas, 1);
    assert!(rel.impossiveis.is_empty(), "{:?}", rel.impossiveis);
    assert_eq!(
        filha_aponta_para(&db, p),
        Value::Int(7),
        "a recuperacao tinha de replanejar a cascata com a linha antiga da marca"
    );
}

// ---------------------------------------------------------------------------
// 3. A prova real do outro sentido
// ---------------------------------------------------------------------------

/// A MESMA marca, com a mae ainda no valor velho: aqui a recuperacao cascateia.
///
/// E o que impede o teste 2 de passar por engano. Se a cascata na reaplicacao
/// for consertada, este continua verde e o outro fica vermelho -- que e
/// exatamente o aviso que se quer.
#[test]
fn com_a_mae_no_valor_velho_a_recuperacao_cascateia() {
    let b = base("controle");
    let inst = Instancia::nova(&b).unwrap();
    let db = inst.criar_database("t").unwrap();
    let mut m = clientes(&db);
    let r = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = pedidos(&db);
    let p = f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(m);
    drop(f);

    marca(&db, 4243, r, &[Value::Int(1), Value::Str("Ana".into())]);

    let rel = recuperar(&inst);
    assert_eq!(rel.reaplicadas, 1);
    assert_eq!(
        filha_aponta_para(&db, p),
        Value::Int(7),
        "a sonda nao enxerga cascata nenhuma -- o teste 2 nao prova nada"
    );
}

// ---------------------------------------------------------------------------
// 4. A marca da versao anterior continua sendo lida
// ---------------------------------------------------------------------------

/// Marca **v1** e commit que JA COMECOU -- descarta-la seria jogar fora uma
/// transacao confirmada por causa de uma mudanca nossa de formato.
///
/// A marca e escrita em v2 e rebaixada nos bytes: o campo de versao volta a 1
/// e o CRC do cabecalho e refeito. A linha antiga fica no disco e ninguem a
/// le -- ela mora DENTRO do bloco com tamanho proprio, entao sobra de bloco
/// nao atrapalha leitor nenhum. O que se perde e so a cascata refeita, que e
/// o que a v1 ja nao tinha.
///
/// Prova real: trocar a condicao de `ler_marca` de volta para
/// `versao != VERSAO` faz a marca ser descartada, a linha nunca ser gravada e
/// `achadas 1 / descartadas 1` aparecer no lugar de `completadas 1`.
#[test]
fn a_marca_da_versao_anterior_continua_sendo_completada() {
    let b = base("v1");
    let inst = Instancia::nova(&b).unwrap();
    let db = inst.criar_database("t").unwrap();
    let mut m = clientes(&db);
    m.inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let caminho = phxsql_server::transacao::gravar_marca(
        db.caminho(),
        99,
        0,
        &[Escrita {
            database: "t".into(),
            tabela: "clientes".into(),
            acao: Acao::Inserir,
            rowid: 2,
            linha: vec![Value::Int(2), Value::Str("Bia".into())],
            linha_antiga: Vec::new(),
            motivo: String::new(),
        }],
    )
    .unwrap();

    // Rebaixa para v1: versao em 8..12, CRC do cabecalho em 32..36.
    let mut bytes = std::fs::read(&caminho).unwrap();
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    let crc = phxsql_core::crc::crc32(&bytes[..32]);
    bytes[32..36].copy_from_slice(&crc.to_le_bytes());
    std::fs::write(&caminho, &bytes).unwrap();

    let rel = recuperar(&inst);
    assert_eq!(rel.achadas, 1);
    assert_eq!(
        rel.descartadas, 0,
        "marca v1 nao pode ser descartada: e commit que ja comecou"
    );
    assert_eq!(rel.completadas, 1);
    let mut m = db.abrir_qualificada("clientes").unwrap();
    assert_eq!(
        m.ler(2).unwrap().unwrap()[0],
        Value::Int(2),
        "a linha da marca v1 tinha de ter sido gravada"
    );
}
