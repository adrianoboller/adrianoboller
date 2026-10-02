//! Defeito (c) do pedido 229 -- o pedaco LOCAL: um contador de `Sequence`
//! atras do dado repete numero, calado.
//!
//! O bloco 16 da sonda mediu: `ajustar_sequencia` para tras numa tabela SEM
//! indice unico devolve ids `[1, 2, 3, 1]` -- quatro linhas, um id repetido,
//! nenhum erro. O CRC-32 do cabecalho nao pega isso, porque `ajustar_sequencia`
//! reescreve o cabecalho com CRC valido; so pega bytes adulterados (bloco 17).
//!
//! `reconciliar_sequencia` e o caminho de reparo que faltava, e `reparar` passou
//! a chama-lo. Prova real nos dois sentidos: sem a reconciliacao o id repete
//! (o defeito reposto), com ela o contador volta para depois do maior gravado.
//!
//! A promocao de uma replica ATRASADA e outra coisa, e nao se conserta aqui:
//! os numeros que o master emitiu e esta ponta nunca recebeu nao estao neste
//! `.reg`. Isso esta documentado em `docs/AUTONUMBER.md`, defeito (c).

mod comum;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn esquema_sem_unico() -> Schema {
    // SEM indice unico sobre o id, de proposito: e o unico jeito de o repeat
    // acontecer calado. Com indice unico, o proprio indice recusaria (bloco 6).
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Sequence).obrigatoria(),
            Column::new("c", ColumnType::Str(30)).obrigatoria(),
        ],
        vec![IndexDef::new("porC", vec![IndexColumn::asc(1)])],
    )
    .expect("esquema com Sequence sem indice unico")
}

fn id_da_linha(t: &mut Table, rowid: u64) -> u64 {
    match &t.ler(rowid).unwrap().unwrap()[0] {
        Value::UInt(n) => *n,
        outro => panic!("id nao e UInt: {outro:?}"),
    }
}

/// O DEFEITO, reposto: sem reconciliar, ajustar para tras faz o id repetir.
/// Este teste documenta o bloco 16 -- ele passa hoje e continua passando,
/// porque descreve o que acontece QUANDO NAO se reconcilia.
#[test]
fn contador_atras_do_dado_repete_o_numero_calado() {
    let d = comum::DirTemp::novo("reconciliar-repete");
    let mut t = Table::criar(&d, esquema_sem_unico()).unwrap();

    for i in 0..3 {
        t.inserir(&[Value::Null, Value::Str(format!("n{i}"))])
            .unwrap();
    }
    // Contador do administrador para tras, abaixo do maior gravado (3).
    t.ajustar_sequencia(1).unwrap();

    let rowid = t
        .inserir(&[Value::Null, Value::Str("repetido".into())])
        .unwrap();
    assert_eq!(
        id_da_linha(&mut t, rowid),
        1,
        "sem reconciliar, o contador atrasado reemite o id 1 -- e o defeito"
    );
}

/// O CONSERTO: reconciliar empurra o contador para depois do maior gravado, e
/// a proxima insercao nao repete.
///
/// Reponha o defeito trocando o `t.reconciliar_sequencia()` por nada: a
/// insercao volta a sair com id 1 e a ultima asserção cai.
#[test]
fn reconciliar_empurra_o_contador_para_depois_do_maior() {
    let d = comum::DirTemp::novo("reconciliar-conserta");
    let mut t = Table::criar(&d, esquema_sem_unico()).unwrap();

    for i in 0..3 {
        t.inserir(&[Value::Null, Value::Str(format!("n{i}"))])
            .unwrap();
    }
    t.ajustar_sequencia(1).unwrap();

    // O reparo do contador: devolve o maior valor gravado (3).
    let maior = t.reconciliar_sequencia().unwrap();
    assert_eq!(maior, 3, "o maior id gravado e 3");

    let rowid = t
        .inserir(&[Value::Null, Value::Str("depois".into())])
        .unwrap();
    assert_eq!(
        id_da_linha(&mut t, rowid),
        4,
        "reconciliado, o proximo id e maior+1 = 4, e nao repete"
    );
}

/// Reconciliar so empurra para a FRENTE: um contador ja adiantado (por buraco
/// de exclusao fisica, por exemplo) fica onde esta -- o numero excluido nunca
/// volta, e reconciliar nao o traz de volta.
#[test]
fn reconciliar_nunca_recua_o_contador() {
    let d = comum::DirTemp::novo("reconciliar-nao-recua");
    let mut t = Table::criar(&d, esquema_sem_unico()).unwrap();

    for i in 0..3 {
        t.inserir(&[Value::Null, Value::Str(format!("n{i}"))])
            .unwrap();
    }
    // Contador bem a frente do dado, de propria vontade do administrador.
    t.ajustar_sequencia(1000).unwrap();

    let maior = t.reconciliar_sequencia().unwrap();
    assert_eq!(maior, 3);

    let rowid = t
        .inserir(&[Value::Null, Value::Str("depois".into())])
        .unwrap();
    assert_eq!(
        id_da_linha(&mut t, rowid),
        1000,
        "reconciliar nao puxa o contador para tras -- o 1000 do administrador fica"
    );
}

/// Tabela sem coluna `Sequence`: reconciliar e no-op, devolve 0 e nao explode.
#[test]
fn sem_sequencia_reconciliar_e_zero() {
    let d = comum::DirTemp::novo("reconciliar-sem-seq");
    let esquema = Schema::new(
        "notas",
        vec![Column::new("c", ColumnType::Str(30)).obrigatoria()],
        vec![],
    )
    .unwrap();
    let mut t = Table::criar(&d, esquema).unwrap();
    t.inserir(&[Value::Str("x".into())]).unwrap();
    assert_eq!(t.reconciliar_sequencia().unwrap(), 0);
}

// ------------------------------------------- pedido 290: a tabela sem saida

/// A tabela com faixa (`passo = 2`).
fn esquema_com_faixa() -> Schema {
    Schema::new(
        "numerada",
        vec![
            Column::new("id", ColumnType::Sequence),
            Column::new("c", ColumnType::Str(30)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_passo_da_sequencia(2)
    .unwrap()
}

/// Regrava o contador da sequencia (bytes 36..44 do cabecalho do volume 1),
/// com o CRC certo -- o ESTADO que o contador `v + 1` do defeito deixava no
/// disco, sem precisar do binario defeituoso para produzi-lo.
fn gravar_contador_cru(dir: &std::path::Path, nome: &str, valor: u64) {
    let caminho = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            n.starts_with(nome) && n.ends_with(".reg")
        })
        .expect("o .reg da tabela");
    let mut bytes = std::fs::read(&caminho).unwrap();
    // Tabela sem cifra: cabecalho de 128 bytes, CRC nos 4 ultimos.
    bytes[36..44].copy_from_slice(&valor.to_le_bytes());
    let crc = phxsql_core::crc::crc32(&bytes[..124]);
    bytes[124..128].copy_from_slice(&crc.to_le_bytes());
    std::fs::write(&caminho, bytes).unwrap();
}

/// **NAO 290-b do parecer do DBA: a tabela gravada pelo contador `v + 1`
/// tinha de ter saida, e tem -- pelo MAIOR valor gravado.**
///
/// O estado no disco e o que a versao defeituosa deixava: faixa 0 de 2, a
/// linha numerada 2 e o contador em 3 (fora da faixa). A abertura recusa
/// nomeando as DUAS causas e a saida; `realinhar_sequencia` abre sem a
/// conferencia, poe o contador em 4 e a tabela volta a abrir e a numerar na
/// faixa dela.
///
/// # O vermelho
///
/// Com o realinhamento abrindo pela porta CONFERIDA, ele cai na mesma recusa
/// e a tabela continua sem saida.
#[test]
fn a_tabela_com_o_contador_defeituoso_volta_a_abrir_pelo_maior_gravado() {
    // O no DECLARA a faixa 0, como o servidor declara sempre: desde o pedido
    // 615 quem nao declarou abre a tabela de qualquer faixa para ler, e a
    // recusa da abertura -- o caso deste teste -- so existe para quem
    // declarou. Os vizinhos deste arquivo numeram na faixa 0 de qualquer
    // jeito, entao o global do processo nao muda nada para eles.
    phxsql_store::no::definir_inicio_da_sequencia(0);
    let d = comum::DirTemp::novo("faixa-sem-saida");
    {
        let mut t = Table::criar(&d, esquema_com_faixa()).unwrap();
        let r = t.inserir(&[Value::Null, Value::Str("um".into())]).unwrap();
        assert_eq!(id_da_linha(&mut t, r), 2);
    }
    gravar_contador_cru(&d, "numerada", 3);

    let e = Table::abrir(&d, "numerada")
        .err()
        .expect("a faixa recusa")
        .to_string();
    assert!(
        e.contains("pelo_maior") && e.contains("inicio_da_sequencia"),
        "a recusa tem de nomear as duas causas e a saida: {e}"
    );

    let (antes, maior, depois) = Table::realinhar_sequencia(&d, "numerada").unwrap();
    assert_eq!((antes, maior, depois), (3, 2, 4));

    let mut t = Table::abrir(&d, "numerada").expect("realinhada, a tabela abre");
    let r = t
        .inserir(&[Value::Null, Value::Str("dois".into())])
        .unwrap();
    assert_eq!(
        id_da_linha(&mut t, r),
        4,
        "o proximo e o da faixa acima do maior"
    );
}

/// O `reparar` reconcilia pela MESMA conta: numa tabela com faixa, o `maior +
/// 1` cru caia fora dela e o `ajustar_sequencia` recusava -- o reparo virava
/// erro justamente onde o contador mais precisava.
#[test]
fn reconciliar_numa_tabela_com_faixa_fica_na_faixa() {
    let d = comum::DirTemp::novo("faixa-reconcilia");
    let mut t = Table::criar(&d, esquema_com_faixa()).unwrap();
    for i in 0..3 {
        t.inserir(&[Value::Null, Value::Str(format!("n{i}"))])
            .unwrap();
    }
    t.ajustar_sequencia(2).unwrap();
    assert_eq!(t.reconciliar_sequencia().unwrap(), 6);
    let r = t
        .inserir(&[Value::Null, Value::Str("depois".into())])
        .unwrap();
    assert_eq!(id_da_linha(&mut t, r), 8);
}

// -------------------------------- pedido 229 c-pleno: o contador do source

/// A replica adota o `proxima` do source: o numero que ele entregou e ela nao
/// recebeu nao sai de novo, e o contador sobrevive a reabrir a tabela. Tire o
/// corpo do `adotar_sequencia_do_source` e o proximo id volta a ser o 3.
#[test]
fn adotar_o_contador_do_source_pula_o_que_ele_ja_entregou() {
    let d = comum::DirTemp::novo("adota-contador");
    {
        let mut t = Table::criar(&d, esquema_sem_unico()).unwrap();
        for i in 0..2 {
            t.inserir(&[Value::Null, Value::Str(format!("n{i}"))])
                .unwrap();
        }
        assert!(
            t.adotar_sequencia_do_source(6).unwrap(),
            "andou de 3 para 6"
        );
        // Adotar de novo o mesmo valor, ou um menor, nao mexe em nada.
        assert!(!t.adotar_sequencia_do_source(6).unwrap());
        assert!(!t.adotar_sequencia_do_source(4).unwrap());
        assert_eq!(t.sequencia_atual(), 6);
    }
    let mut t = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(t.sequencia_atual(), 6, "o contador adotado esta no disco");
    let r = t
        .inserir(&[Value::Null, Value::Str("promovida".into())])
        .unwrap();
    assert_eq!(id_da_linha(&mut t, r), 6);
}

/// «Nao disse» (zero) e tabela sem `Sequence` nao tocam em nada: o source de
/// antes do campo continua sendo uma origem valida.
#[test]
fn adotar_zero_ou_sem_sequencia_nao_faz_nada() {
    let d = comum::DirTemp::novo("adota-nada");
    let mut t = Table::criar(&d, esquema_sem_unico()).unwrap();
    t.inserir(&[Value::Null, Value::Str("a".into())]).unwrap();
    assert!(!t.adotar_sequencia_do_source(0).unwrap());
    assert_eq!(t.sequencia_atual(), 2);

    let d2 = comum::DirTemp::novo("adota-sem-seq");
    let esquema = Schema::new(
        "notas",
        vec![Column::new("c", ColumnType::Str(30)).obrigatoria()],
        vec![],
    )
    .unwrap();
    let mut n = Table::criar(&d2, esquema).unwrap();
    assert!(!n.adotar_sequencia_do_source(500).unwrap());
    assert_eq!(n.sequencia_atual(), 0);
}

/// Numa tabela com faixa o contador adotado cai na faixa DESTE no, e nao no
/// numero cru do source -- que pode ser o proximo do outro no.
#[test]
fn adotar_o_contador_cai_na_faixa_deste_no() {
    phxsql_store::no::definir_inicio_da_sequencia(0);
    let d = comum::DirTemp::novo("adota-faixa");
    let mut t = Table::criar(&d, esquema_com_faixa()).unwrap();
    t.inserir(&[Value::Null, Value::Str("a".into())]).unwrap();
    assert!(t.adotar_sequencia_do_source(7).unwrap());
    assert_eq!(t.sequencia_atual(), 8, "7 nao e do no par: o proximo e o 8");
    let r = t.inserir(&[Value::Null, Value::Str("b".into())]).unwrap();
    assert_eq!(id_da_linha(&mut t, r), 8);
}

/// **Pedido 650: o contador que o source manda tem teto.** Um source com
/// defeito (ou hostil) anunciando `u64::MAX - 1` nao pode gastar a numeracao
/// da replica -- que, promovida, emitiria ids absurdos --, nem derrubar a
/// adocao com `panic` de overflow dentro de `na_faixa`.
///
/// # O vermelho
///
/// Sem o teto, o contador salta para ~2^64 (o `assert_eq!` do contador cai);
/// sem a saturacao do `na_faixa`, o `u64::MAX` com passo 2 e `panic` em debug.
#[test]
fn contador_do_source_acima_de_2_53_e_recusado_sem_panico() {
    phxsql_store::no::definir_inicio_da_sequencia(0);
    let d = comum::DirTemp::novo("adota-teto");
    let mut t = Table::criar(&d, esquema_com_faixa()).unwrap();
    t.inserir(&[Value::Null, Value::Str("a".into())]).unwrap();
    let antes = t.sequencia_atual();
    for hostil in [u64::MAX - 1, u64::MAX, (1u64 << 53) + 1] {
        let e = t.adotar_sequencia_do_source(hostil).unwrap_err();
        assert_eq!(e.nome(), "LIMITE_EXCEDIDO", "{e}");
        assert_eq!(t.sequencia_atual(), antes, "o contador andou com {hostil}");
    }
    // O teto e inclusivo e nao recusa quem esta atras: o legitimo anda.
    assert!(t.adotar_sequencia_do_source(1 << 53).unwrap());
    assert!(t.sequencia_atual() <= (1u64 << 53) + 2);
}

/// O mesmo, com passo 3 como no pedido: a faixa nao pode dar a volta.
#[test]
fn contador_hostil_com_passo_3_nao_da_a_volta() {
    let d = comum::DirTemp::novo("adota-teto-passo3");
    let esquema = Schema::new(
        "tres",
        vec![
            Column::new("id", ColumnType::Sequence),
            Column::new("c", ColumnType::Str(30)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_passo_da_sequencia(3)
    .unwrap();
    let mut t = Table::criar(&d, esquema).unwrap();
    let antes = t.sequencia_atual();
    assert!(t.adotar_sequencia_do_source(u64::MAX - 1).is_err());
    assert_eq!(t.sequencia_atual(), antes);
}
