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

/// Tira do diretorio TODOS os arquivos de uma tabela, devolvendo o que levou.
///
/// Por que mover em vez de apagar: a tabela volta inteira, com o `.reg`, o
/// `.ndx` e os diarios como estavam. Apagar e recriar daria outra tabela.
fn esconder(dir: &std::path::Path, tabela: &str) -> Vec<std::path::PathBuf> {
    let cofre = dir.join("__cofre");
    std::fs::create_dir_all(&cofre).unwrap();
    let mut levados = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        let casa = p
            .file_stem()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == tabela);
        if casa && p.is_file() {
            let destino = cofre.join(p.file_name().unwrap());
            std::fs::rename(&p, &destino).unwrap();
            levados.push(destino);
        }
    }
    assert!(!levados.is_empty(), "nao achei arquivo nenhum de {tabela}");
    levados
}

/// Devolve o que a [`esconder`] levou.
fn devolver(dir: &std::path::Path, guardados: &[std::path::PathBuf]) {
    for g in guardados {
        std::fs::rename(g, dir.join(g.file_name().unwrap())).unwrap();
    }
    std::fs::remove_dir_all(dir.join("__cofre")).ok();
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

/// Tres niveis: a recusa da neta chega ANTES de a avo ser gravada.
///
/// `planejar_ao_alterar` so olha quem aponta para ESTA tabela -- um nivel. Com
/// `avo <- mae (cascata) <- neta (restringir)` isso nao bastava: o plano da
/// avo passava, a avo ia para o disco, e so entao a cascata chamava
/// `mae.atualizar`, que achava a neta com `restringir` e recusava. **A avo
/// ficava gravada e a mae para tras**, sem queda nenhuma e so com declaracoes
/// legitimas. Quem fecha isso e a conferencia da ARVORE INTEIRA antes da
/// primeira escrita.
///
/// Prova real: tirar a chamada a `conferir_a_arvore` do `atualizar` devolve
/// `Int(7)` na avo e deixa a mae em `Int(1)` -- que e exatamente o defeito
/// medido.
#[test]
fn a_recusa_da_neta_chega_antes_de_a_avo_ser_gravada() {
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

    let erro = m
        .atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect_err("a cascata de tres niveis passou -- o cenario mudou");
    let texto = erro.to_string();
    assert!(
        texto.contains("restringir"),
        "a recusa tem de ser a da neta; veio: {texto}"
    );
    assert!(
        !texto.contains("a mae ja esta gravada"),
        "a recusa chegou DEPOIS de gravar -- e o defeito que esta prova trava: {texto}"
    );

    // O que importa: o disco NAO se mexeu, nem na avo nem na mae.
    let mut m = db.abrir_qualificada("clientes").unwrap();
    assert_eq!(
        m.ler(r).unwrap().unwrap()[0],
        Value::Int(1),
        "a avo foi gravada apesar da recusa"
    );
    assert_eq!(
        filha_aponta_para(&db, p),
        Value::Int(1),
        "a mae devia ter ficado como estava"
    );
}

// ---------------------------------------------------------------------------
// 2. A reaplicacao que nao conserta, e o relatorio que diz que consertou
// ---------------------------------------------------------------------------

/// O estado que a queda no meio da passada deixa, e o que a recuperacao faz
/// com ele.
///
/// # Como o estado pos-queda e montado, e por que assim
///
/// A queda que interessa cai ENTRE a gravacao da mae e a cascata: em disco
/// ficam a mae nova e a filha velha, sem rastro nenhum da cascata pendente.
/// Reproduzir isso por uma FALHA da cascata deixou de funcionar quando a
/// conferencia da arvore passou a recusar antes de gravar -- e isso e bom, e o
/// conserto do outro teste deste arquivo.
///
/// Entao o estado se monta pelo unico caminho que continua fiel: os arquivos
/// da filha saem do diretorio, a mae e alterada (o planejamento nao ve filha
/// nenhuma, entao nao cascateia), e a filha volta. O que fica em disco e
/// **byte a byte** o que a queda deixaria, e sem depender de nenhum caminho de
/// erro para produzi-lo.
///
/// Prova real: tirar a chamada a `recascatear` do `aplicar_uma` devolve
/// `Int(1)` na ultima linha, que e o defeito medido. E o teste 3 e a MESMA
/// marca com a mae ainda no valor velho -- sem ele, este passaria tambem numa
/// sonda que nao enxerga cascata nenhuma.
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
    drop(f);
    drop(m);

    // A filha sai de cena, a mae anda sozinha, a filha volta.
    let dir = db.caminho().to_path_buf();
    let guardados = esconder(&dir, "pedidos");
    let mut m = db.abrir_qualificada("clientes").unwrap();
    m.atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect("sem filha a vista, a alteracao da mae nao cascateia nada");
    m.sincronizar().unwrap();
    drop(m);
    devolver(&dir, &guardados);

    assert_eq!(
        filha_aponta_para(&db, p),
        Value::Int(1),
        "o cenario nao montou: a filha tinha de estar para tras"
    );

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

// ---------------------------------------------------------------------------
// 5. O pedido 172: a filha com o `.ndx` sujo
// ---------------------------------------------------------------------------

/// Deixa o `.ndx` da filha SUJO do jeito que ele fica de verdade.
///
/// # Por que nao se escreve o byte a mao
///
/// A marca de sujo e o byte 52 do cabecalho, e foi o que a frente da
/// durabilidade viu levantado depois de `SIGKILL` real
/// (`bancada/durabilidade/`). A primeira versao deste ajudante escrevia esse
/// byte direto no arquivo -- e o teste caiu com **`cabecalho com CRC
/// invalido`**, que e outro estado: o cabecalho tem CRC e ele protege a propria
/// marca. Virar o bit a mao nao simula uma queda, simula uma adulteracao, e o
/// motor distingue as duas -- corretamente.
///
/// O jeito honesto e deixar o CODIGO levantar a marca: um descritor com escrita
/// pendente marca o indice como sujo e grava o cabecalho com o CRC certo. Basta
/// nao sincronizar e manter o descritor vivo -- que e exatamente a situacao
/// medida.
fn descritor_deixando_o_indice_sujo(db: &Database, tabela: &str) -> Table {
    let mut t = db.abrir_qualificada(tabela).unwrap();
    // Aponta para 7, e nao para 1: a mae ja andou, e a chave conferida recusa
    // a filha de um pai que nao existe mais -- foi o segundo jeito errado de
    // montar este cenario. O valor nao importa aqui; o que importa e a escrita
    // ficar PENDENTE.
    t.inserir(&[Value::Int(999), Value::Int(7)]).unwrap();
    t
}

/// A recuperacao RECONSTROI o `.ndx` da filha e completa a cascata.
///
/// # O buraco, e por que ele era estrutural
///
/// O `completar()` ja reconstruia o indice sujo -- para toda tabela NOMEADA NA
/// MARCA. A filha da cascata nao esta nomeada em marca nenhuma, porque a
/// cascata nunca vira `Escrita`: a maquina existia, rodava, e nao alcancava
/// justamente a tabela que a cascata ia consertar.
///
/// A §5.5.3 dizia que consertar «pediria a recuperacao saber tentar de novo
/// mais tarde». Nao pede: pede reconstruir enquanto a marca ainda existe.
///
/// # Prova real
///
/// Desligar o `ligar_reconstrucao_do_indice_da_filha(true)` do `completar()`
/// faz este teste cair em `impossiveis` com a filha ainda no valor velho --
/// que e o estado medido em 9 de 21 corridas da matriz de durabilidade.
#[test]
fn a_recuperacao_reconstroi_o_indice_da_filha_e_completa_a_cascata() {
    let b = base("filha-suja");
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
    drop(f);
    drop(m);

    let dir = db.caminho().to_path_buf();
    let guardados = esconder(&dir, "pedidos");
    let mut m = db.abrir_qualificada("clientes").unwrap();
    m.atualizar(r, &[Value::Int(7), Value::Str("Ana".into())])
        .expect("sem filha a vista, a alteracao da mae nao cascateia nada");
    m.sincronizar().unwrap();
    drop(m);
    devolver(&dir, &guardados);

    // O que este teste acrescenta ao da secao 2: a queda tambem pegou o `.ndx`
    // da filha no meio da passada. O descritor fica VIVO durante a recuperacao,
    // porque e a escrita pendente dele que mantem a marca levantada.
    let _sujo = descritor_deixando_o_indice_sujo(&db, "pedidos");

    marca(&db, 4242, r, &[Value::Int(1), Value::Str("Ana".into())]);
    let rel = recuperar(&inst);

    assert!(
        rel.impossiveis.is_empty(),
        "a recusa por indice sujo voltou: {:?}",
        rel.impossiveis
    );
    assert_eq!(rel.completadas, 1);
    assert_eq!(
        filha_aponta_para(&db, p),
        Value::Int(7),
        "a cascata nao chegou na filha: o indice sujo bloqueou a busca reversa"
    );
    assert!(
        rel.indices_reconstruidos >= 1,
        "reconstruiu em silencio: o relatorio tem de CONTAR ({} contados)",
        rel.indices_reconstruidos
    );
}
