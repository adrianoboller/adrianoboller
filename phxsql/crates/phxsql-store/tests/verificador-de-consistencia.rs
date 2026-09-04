//! O verificador de consistência referencial, e a recusa de declarar uma chave
//! conferida sobre dado que já viola.
//!
//! Os dois lados do mesmo achado da sonda (`--example sonda-fk-buracos`, item
//! 4): dava para declarar `verificar: true` numa tabela que já tinha órfã, e a
//! órfã continuava lá. A tabela nascia com uma promessa falsa.
//!
//! **O verificador RELATA, não conserta.** Consertar dado do dono sem ele
//! pedir é pior que o defeito: uma órfã pode ser lixo de importação, e pode
//! ser a única cópia de um pedido cujo cliente alguém apagou por engano — e as
//! duas são indistinguíveis daqui.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::error::PhxError;
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::integridade::{self, Falha};
use phxsql_store::table::Table;

fn mae(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn fk(conferindo: bool) -> ForeignKey {
    ForeignKey::new("fk_cliente", vec![1], "clientes", vec!["id".into()]).conferindo(conferindo)
}

/// A filha, com índice pela chave (que a conferência exige dos dois lados).
fn filha(d: &std::path::Path, conferindo: bool) -> Table {
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
    .com_chaves_estrangeiras(vec![fk(conferindo)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

/// Uma base com mãe viva e filha certa. O controle: sem ele, um verificador
/// que acusasse tudo passaria por todos os testes abaixo.
#[test]
fn base_limpa_nao_acusa_nada() {
    let d = DirTemp::novo("vc-limpa");
    let mut m = mae(&d.0);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);
    let mut f = filha(&d.0, true);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    let r = integridade::conferir_diretorio(&d.0).unwrap();
    assert!(r.limpo(), "base limpa acusada: {:?}", r.violacoes);
    assert_eq!(r.tabelas, 2);
    assert_eq!(r.chaves, 1);
}

/// A órfã aparece com tabela, chave, rowid e valor — e nada é consertado.
#[test]
fn a_orfa_aparece_e_o_verificador_nao_a_conserta() {
    let d = DirTemp::novo("vc-orfa");
    mae(&d.0).sincronizar().unwrap();
    // `verificar: false` é como se GRAVA uma órfã sem quebrar regra nenhuma:
    // a chave está declarada e não é imposta.
    let mut f = filha(&d.0, false);
    f.inserir(&[Value::Int(10), Value::Int(999)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    let r = integridade::conferir_diretorio(&d.0).unwrap();
    assert_eq!(r.violacoes.len(), 1, "{:?}", r.violacoes);
    let v = &r.violacoes[0];
    assert_eq!(v.tabela, "pedidos");
    assert_eq!(v.chave, "fk_cliente");
    assert_eq!(v.rowid, Some(1));
    assert_eq!(v.valor, vec![Value::Int(999)]);
    assert_eq!(v.falha, Falha::MaeAusente);
    assert!(!v.conferida, "a chave não pediu conferência, e isso se diz");

    // E a linha continua lá, intacta: ele RELATA.
    let mut f = Table::abrir(&d.0, "pedidos").unwrap();
    assert_eq!(f.registros(), 1);
    assert_eq!(f.ler(1).unwrap().unwrap()[1], Value::Int(999));
}

/// «Existir» não é «estar viva»: a mãe excluída de forma suave é uma falha
/// PRÓPRIA, e não a mesma de mãe ausente. Misturar as duas mandaria o dono
/// procurar uma linha que está lá.
#[test]
fn a_mae_excluida_suave_e_uma_falha_com_nome_proprio() {
    let d = DirTemp::novo("vc-suave");
    let mut m = mae(&d.0);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();

    let mut f = filha(&d.0, false);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    // A chave não é conferida, então o `conferir_filhas` não a vê e a mãe sai.
    m.excluir_suave(1, "saiu").unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let r = integridade::conferir_diretorio(&d.0).unwrap();
    assert_eq!(r.violacoes.len(), 1, "{:?}", r.violacoes);
    assert_eq!(r.violacoes[0].falha, Falha::MaeExcluida);
}

/// NULO satisfaz — o mesmo `MATCH SIMPLE` da gravação. Conferir aqui e não lá
/// faria o verificador acusar linha que o motor aceita.
#[test]
fn nulo_satisfaz_e_nao_vira_violacao() {
    let d = DirTemp::novo("vc-nulo");
    mae(&d.0).sincronizar().unwrap();
    let mut f = filha(&d.0, true);
    f.inserir(&[Value::Int(10), Value::Null]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    let r = integridade::conferir_diretorio(&d.0).unwrap();
    assert!(r.limpo(), "nulo virou violação: {:?}", r.violacoes);
}

/// O índice que falta na filha aparece como falha de ESTRUTURA, e não como uma
/// violação por linha: ele trava a chave inteira, e some no dia do primeiro
/// `excluir` da mãe se ninguém perguntar antes.
#[test]
fn indice_que_falta_na_filha_e_falha_de_estrutura() {
    let d = DirTemp::novo("vc-sem-indice");
    let mut m = mae(&d.0);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        // Sem `porCliente`: a filha não sabe responder quem aponta para a mãe.
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![fk(false)])
    .unwrap();
    let mut f = Table::criar(&d.0, e).unwrap();
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    let r = integridade::conferir_diretorio(&d.0).unwrap();
    let est: Vec<_> = r
        .violacoes
        .iter()
        .filter(|v| v.falha.e_de_estrutura())
        .collect();
    assert_eq!(est.len(), 1, "{:?}", r.violacoes);
    assert_eq!(est[0].falha, Falha::SemIndiceNaFilha);
    assert_eq!(est[0].rowid, None, "falha de estrutura não é de uma linha");
    // E a linha em si está certa: a mãe existe e está viva.
    assert!(r.violacoes.iter().all(|v| v.falha.e_de_estrutura()));
}

/// O IRMÃO do teste acima, e ele faltava.
///
/// O verificador confere índice **dos dois lados** — na filha para responder
/// «alguém aponta para esta linha?» e na mãe para responder «existe este
/// pai?». O lado da filha estava provado desde que a sonda o achou; o lado da
/// mãe estava escrito e **nunca provado**, e foi a medição do pedido 175 que
/// o encontrou: contar «tabelas que declaram chave sem o índice» apoia metade
/// da resposta num ramo que nenhum teste exercitava.
#[test]
fn indice_que_falta_na_mae_e_falha_de_estrutura() {
    let d = DirTemp::novo("vc-sem-indice-mae");
    // A mãe indexa `nome`, e não `id`: nenhum índice cobre a coluna que a
    // chave referencia.
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("nome", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porNome", vec![IndexColumn::asc(1)])],
    )
    .unwrap();
    let mut m = Table::criar(&d.0, e).unwrap();
    m.inserir(&[Value::Int(1), Value::Str("ana".into())])
        .unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = filha(&d.0, false);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    let r = integridade::conferir_diretorio(&d.0).unwrap();
    let est: Vec<_> = r
        .violacoes
        .iter()
        .filter(|v| v.falha.e_de_estrutura())
        .collect();
    assert_eq!(est.len(), 1, "{:?}", r.violacoes);
    assert_eq!(est[0].falha, Falha::SemIndiceNaMae);
    assert_eq!(est[0].rowid, None, "falha de estrutura não é de uma linha");
    // E ele PARA na estrutura: sem índice na mãe não há como perguntar por
    // linha, e inventar uma varredura aqui faria o relatório medir outra
    // coisa. A linha 10 aponta para uma mãe que EXISTE — acusá-la seria
    // mentira, e não acusá-la por varredura seria sorte.
    assert!(
        r.violacoes.iter().all(|v| v.falha.e_de_estrutura()),
        "{:?}",
        r.violacoes
    );
}

// ---------------------------------------------------------------------------
// A recusa na DECLARAÇÃO
// ---------------------------------------------------------------------------

/// Declarar conferida sobre dado que já viola é prometer o que não se pode
/// cumprir. A recusa nomeia a linha.
#[test]
fn redeclarar_recusa_chave_conferida_sobre_orfa() {
    let d = DirTemp::novo("vc-decl-recusa");
    mae(&d.0).sincronizar().unwrap();
    let mut f = filha(&d.0, false);
    f.inserir(&[Value::Int(10), Value::Int(999)]).unwrap();
    f.sincronizar().unwrap();

    let erro = f
        .redeclarar_chaves_estrangeiras(vec![fk(true)])
        .expect_err("a tabela tem órfã: a chave não pode nascer conferida");
    let t = erro.to_string();
    assert!(matches!(erro, PhxError::Integridade(_)), "{erro:?}");
    assert!(t.contains("rowid 1"), "a recusa tem de nomear a linha: {t}");
    assert!(t.contains("999"), "e o valor: {t}");
    assert!(
        t.contains("verificar"),
        "e tem de dizer a saída de quem quer declarar assim mesmo: {t}"
    );
    // Nada foi declarado: recusar depois de gravar não é recusar.
    assert!(!f.esquema().chaves_estrangeiras()[0].verificar);
}

/// O comportamento VELHO não muda: quem declara com `verificar` desligado
/// continua declarando, órfã ou não. É a escolha escrita em vez da omissão.
#[test]
fn declarar_sem_conferir_continua_passando_com_orfa() {
    let d = DirTemp::novo("vc-decl-sem");
    mae(&d.0).sincronizar().unwrap();
    let mut f = filha(&d.0, false);
    f.inserir(&[Value::Int(10), Value::Int(999)]).unwrap();
    f.sincronizar().unwrap();

    f.redeclarar_chaves_estrangeiras(vec![fk(false)])
        .expect("declarar sem conferir continua como sempre foi");
}

/// E o controle da recusa: com o dado LIMPO, a chave nasce conferida.
/// Sem ele, um portão que recusasse toda declaração passaria pelos dois acima.
#[test]
fn com_dado_limpo_a_chave_nasce_conferida() {
    let d = DirTemp::novo("vc-decl-limpo");
    let mut m = mae(&d.0);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut f = filha(&d.0, false);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();

    f.redeclarar_chaves_estrangeiras(vec![fk(true)])
        .expect("dado limpo: a chave nasce conferida");
    assert!(f.esquema().chaves_estrangeiras()[0].verificar);
}

/// Redeclarar uma chave que **já** conferia não varre de novo.
///
/// A órfã aqui é plantada pelo caminho que legitimamente a produz: a réplica,
/// que aplica o que a origem já aceitou sem julgar — a linha da mãe pode
/// simplesmente ainda não ter chegado. Com a varredura cobrada de toda
/// redeclaração, um `ALTER TABLE` que só troca o `ao_alterar` reprovaria numa
/// réplica em dia, por dado que vai se resolver sozinho no próximo lote.
#[test]
fn redeclarar_chave_ja_conferida_nao_varre_de_novo() {
    // O source, para tirar dele a imagem de uma linha filha.
    let ds = DirTemp::novo("vc-decl-source");
    let mut ms = mae(&ds.0);
    ms.inserir(&[Value::Int(1)]).unwrap();
    ms.sincronizar().unwrap();
    drop(ms);
    let mut fs = filha(&ds.0, true).com_imagem_no_diario(true);
    fs.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    fs.sincronizar().unwrap();
    let (evento, imagem) = fs.diario_com_imagem(0, 0).unwrap().remove(0);
    drop(fs);

    // A réplica: a filha chega, a mãe ainda não.
    let dr = DirTemp::novo("vc-decl-replica");
    mae(&dr.0).sincronizar().unwrap();
    let mut f = filha(&dr.0, true);
    f.aplicar_evento(evento.operacao, evento.rowid, &imagem)
        .expect("a réplica aplica o que a origem já aceitou");
    f.sincronizar().unwrap();

    // A órfã está lá, e o verificador a vê.
    let r = integridade::conferir_diretorio(&dr.0).unwrap();
    assert_eq!(r.violacoes.len(), 1, "{:?}", r.violacoes);
    assert!(r.violacoes[0].conferida, "a chave desta tabela é conferida");

    // Mesmo assim, redeclarar a MESMA chave conferida passa: ela já era
    // garantida, e a garantia não mudou.
    let mut trocada = fk(true);
    // Só a ação muda: mudar de cascata para anular não torna falsa uma relação
    // que já era conferida.
    trocada.ao_alterar = phxsql_core::schema::AcaoRi::AnularCampos;
    f.redeclarar_chaves_estrangeiras(vec![trocada])
        .expect("chave já conferida não paga a varredura de novo");
}
