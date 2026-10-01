//! A troca de volumes que uma queda deixou pela metade, vista por quem NAO e a
//! abertura: a copia de tabela (pedido 624) e o lixo da FASE A que ninguem
//! recolhia (pedido 625).
//!
//! As quedas sao de verdade -- panico de teste no ponto exato da FASE B
//! (`ndx::panico_de_teste`), e nao arquivos montados a mao: o estado no disco
//! e o que o codigo de producao deixa, e muda junto com ele.

#[allow(
    dead_code,
    reason = "o modulo comum serve a varios testes; este usa so o DirTemp"
)]
mod comum;

use comum::DirTemp;

use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::{Database, Instancia};
use phxsql_store::ndx::panico_de_teste::{armar, desarmar, Ponto};
use phxsql_store::table::{SemEscrever, Table, Visao};

const NOME: usize = 1;
const LINHAS: i64 = 95;

fn esquema(paginada: bool) -> Schema {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    if paginada {
        // 30 linhas por volume: as 95 viram quatro volumes.
        e.com_paginacao(Paginacao::nova(30, 9).unwrap()).unwrap()
    } else {
        e
    }
}

fn cliente(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("cliente {i:04}"))]
}

/// A base `loja` com a tabela `clientes` e as 95 linhas.
fn base(rotulo: &str, paginada: bool) -> (Instancia, Database, DirTemp) {
    let d = DirTemp::novo(&format!("troca-{rotulo}"));
    let cat = Instancia::nova(&d).unwrap();
    let db = cat.criar_database("loja").unwrap();
    let mut t = db.criar_tabela(None, esquema(paginada)).unwrap();
    for i in 1..=LINHAS {
        t.inserir(&cliente(i)).unwrap();
    }
    t.sincronizar().unwrap();
    (cat, db, d)
}

/// Os `*.novo` do `.reg` de `tabela` na pasta, em ordem.
fn novos_do_reg(dir: &std::path::Path, tabela: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.starts_with(tabela) && f.ends_with(".reg.novo"))
        .collect();
    v.sort();
    v
}

/// O `acrescentar_coluna` que morre no ponto `p` da FASE B.
fn morrer_na_fase_b(dir: &std::path::Path, p: Ponto) {
    let mut t = Table::abrir(dir, "clientes").unwrap();
    armar(p);
    let morreu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = t.acrescentar_coluna(
            Column::new("situacao", ColumnType::Str(12)),
            Some(Value::Str("ok".into())),
        );
    }));
    desarmar();
    assert!(morreu.is_err(), "o panico armado em {p:?} nao aconteceu");
}

/// A tabela inteira e legivel, com a coluna nova ou sem ela.
fn conferir_inteira(dir: &std::path::Path, tabela: &str, com_coluna: bool) {
    let mut t = Table::abrir(dir, tabela).unwrap_or_else(|e| panic!("{tabela} nao abre: {e}"));
    assert_eq!(
        t.esquema().coluna_por_nome("situacao").is_some(),
        com_coluna,
        "{tabela} com a versao errada do esquema"
    );
    let linhas = t.varrer_com(Visao::Todas).unwrap();
    assert_eq!(linhas.len() as i64, LINHAS, "{tabela} perdeu linhas");
    for (i, (rowid, v)) in linhas.iter().enumerate() {
        assert_eq!(*rowid, i as u64 + 1);
        assert_eq!(v[NOME], Value::Str(format!("cliente {:04}", i + 1)));
    }
}

// ------------------------------------------- pedido 624: a copia no meio

/// **Pedido 624:** a troca DECIDIDA (volume 1 ja novo) e nao terminada. A
/// copia levava o volume 1 na largura nova e os outros na velha, sem os
/// `*.novo` -- uma copia que nao abre.
///
/// # Prova real
///
/// Sem o `terminar_troca_antes_de_copiar` no `copiar_os_arquivos`, a copia
/// reprova ao abrir com «uma alteracao de estrutura ficou pela metade».
#[test]
fn duplicar_no_meio_da_troca_decidida_leva_uma_versao_so() {
    let (_cat, db, d) = base("dup-decidida", true);
    let dir = d.join("loja");
    morrer_na_fase_b(&dir, Ponto::FaseBDepoisDoVolume1);
    assert!(
        !novos_do_reg(&dir, "clientes").is_empty(),
        "o ponto do panico nao deixou a troca pela metade"
    );
    db.duplicar_tabela("clientes", "copia").unwrap();
    conferir_inteira(&dir, "copia", true);
    // A origem foi terminada pela MESMA decisao, e abre inteira tambem.
    assert_eq!(novos_do_reg(&dir, "clientes"), Vec::<String>::new());
    conferir_inteira(&dir, "clientes", true);
}

/// O IRMAO: o colar em outro database passa pelo mesmo laco de copia.
///
/// # Prova real
///
/// A mesma do de cima: sem o conserto, a `clientes` colada em `outra` nao
/// abre.
#[test]
fn colar_no_meio_da_troca_decidida_leva_uma_versao_so() {
    let (cat, db, d) = base("colar-decidida", true);
    let outra = cat.criar_database("outra").unwrap();
    morrer_na_fase_b(&d.join("loja"), Ponto::FaseBDepoisDoVolume1);
    db.copiar_tabela_para("clientes", &outra, "clientes")
        .unwrap();
    conferir_inteira(&d.join("outra"), "clientes", true);
}

/// Terminar a troca e ESCREVER na origem: com a tabela congelada (uma
/// reescrita em curso) a copia recusa dizendo por que, e nada nasce.
#[test]
fn a_copia_da_tabela_congelada_com_troca_decidida_recusa() {
    let (_cat, db, d) = base("dup-congelada", true);
    let dir = d.join("loja");
    morrer_na_fase_b(&dir, Ponto::FaseBDepoisDoVolume1);
    let congelada =
        phxsql_store::congelamento::congelar(&dir, "clientes", "teste".to_string()).unwrap();
    let e = db
        .duplicar_tabela("clientes", "copia")
        .expect_err("a copia terminou a troca de uma tabela congelada");
    assert!(e.to_string().contains("REESCRITA"), "{e}");
    assert!(!db.existe_tabela(None, "copia").unwrap());
    drop(congelada);
    db.duplicar_tabela("clientes", "copia").unwrap();
    conferir_inteira(&dir, "copia", true);
}

/// O comportamento VELHO: a FASE A morta antes de decidir (volume 1 velho) e
/// lixo, e a copia leva a tabela velha e inteira -- sem a sobra, e sem apaga-la
/// (a copia e leitura da origem).
#[test]
fn a_copia_com_sobra_de_fase_a_leva_a_tabela_velha() {
    let (_cat, db, d) = base("dup-sobra", true);
    let dir = d.join("loja");
    morrer_na_fase_b(&dir, Ponto::FaseBAntesDaPrimeiraTroca);
    let sobras = novos_do_reg(&dir, "clientes");
    assert_eq!(
        sobras.len(),
        4,
        "a FASE A nao deixou um `*.novo` por volume"
    );
    db.duplicar_tabela("clientes", "copia").unwrap();
    assert_eq!(novos_do_reg(&dir, "copia"), Vec::<String>::new());
    assert_eq!(novos_do_reg(&dir, "clientes"), sobras, "a copia apagou");
    conferir_inteira(&dir, "copia", false);
}

// --------------------------------------- pedido 625: a sobra sem dono

/// **Pedido 625:** a FASE A morta deixa a copia inteira do `.reg` ao lado, e
/// ela ficava enquanto a tabela vivesse. A abertura GRAVAVEL a recolhe; as
/// de leitura nao apagam nada.
///
/// # Prova real
///
/// Sem o `recolher_sobras_da_fase_a` no `Table::abrir_com`, os quatro
/// `clientes#00N.reg.novo` continuam no disco depois da abertura.
#[test]
fn a_abertura_gravavel_recolhe_a_sobra_da_fase_a_e_a_leitura_nao() {
    let (_cat, _db, d) = base("recolhe", true);
    let dir = d.join("loja");
    morrer_na_fase_b(&dir, Ponto::FaseBAntesDaPrimeiraTroca);
    let sobras = novos_do_reg(&dir, "clientes");
    assert_eq!(sobras.len(), 4);

    // Leitura nao apaga -- nem a da ficha compartilhada, nem o `RegFile`
    // que serve quem so le o esquema.
    match Table::abrir_para_ler(&dir, "clientes").unwrap() {
        SemEscrever::Aberta(_) => {}
        SemEscrever::PrecisaEscrever(m) => panic!("a sobra mandou escrever: {m}"),
    }
    phxsql_store::reg::RegFile::abrir(&dir, "clientes").unwrap();
    assert_eq!(novos_do_reg(&dir, "clientes"), sobras, "a leitura apagou");

    conferir_inteira(&dir, "clientes", false);
    assert_eq!(
        novos_do_reg(&dir, "clientes"),
        Vec::<String>::new(),
        "a abertura gravavel deixou a copia do .reg sem dono no disco"
    );
}

/// O mesmo sem paginacao: a tabela de um volume so tambem tem FASE A, e a
/// varredura de antes nem olhava o disco quando a paginacao estava desligada.
///
/// # Prova real
///
/// A mesma do de cima; e com o `separar_novos_ao_lado` saindo cedo para a
/// tabela sem paginacao (o `return` de antes), o `clientes.reg.novo` fica.
#[test]
fn a_sobra_da_tabela_sem_paginacao_tambem_se_recolhe() {
    let (_cat, _db, d) = base("recolhe-1vol", false);
    let dir = d.join("loja");
    morrer_na_fase_b(&dir, Ponto::FaseBAntesDaPrimeiraTroca);
    assert_eq!(novos_do_reg(&dir, "clientes"), vec!["clientes.reg.novo"]);
    conferir_inteira(&dir, "clientes", false);
    assert_eq!(novos_do_reg(&dir, "clientes"), Vec::<String>::new());
}

/// O `*.novo` de uma troca VIVA tem dono, e a abertura gravavel de um vizinho
/// (o que escapou do congelamento) nao o apaga: a FASE B termina.
///
/// # Prova real
///
/// Sem a pergunta `tem_dono` no `recolher_sobras_da_fase_a`, o vizinho apaga
/// o palco, e a FASE B recusa «sumiu antes da troca».
#[test]
fn o_novo_de_uma_troca_viva_nao_e_sobra() {
    let (_cat, _db, d) = base("com-dono", true);
    let dir = d.join("loja");
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    let pendente = t
        .acrescentar_coluna_fase_a(
            Column::new("situacao", ColumnType::Str(12)),
            Some(Value::Str("ok".into())),
        )
        .unwrap();
    drop(Table::abrir(&dir, "clientes").unwrap());
    assert_eq!(novos_do_reg(&dir, "clientes").len(), 4, "o vizinho apagou");
    t.acrescentar_coluna_fase_b(pendente).unwrap();
    drop(t);
    conferir_inteira(&dir, "clientes", true);
}

/// Se um `*.novo` some assim mesmo, a FASE B recusa ANTES do primeiro
/// `rename`: nada trocado, a tabela velha e inteira.
///
/// # Prova real
///
/// Sem a conferencia do comeco do `alargar_fase_b`, o volume 1 e o 2 trocam,
/// o 3 fica velho sem o `*.novo` dele, e a tabela nunca mais abre («pela
/// metade»).
#[test]
fn a_fase_b_recusa_quando_um_novo_sumiu() {
    let (_cat, _db, d) = base("sumiu", true);
    let dir = d.join("loja");
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    let pendente = t
        .acrescentar_coluna_fase_a(
            Column::new("situacao", ColumnType::Str(12)),
            Some(Value::Str("ok".into())),
        )
        .unwrap();
    let novos = novos_do_reg(&dir, "clientes");
    std::fs::remove_file(dir.join(&novos[2])).unwrap();
    let e = t
        .acrescentar_coluna_fase_b(pendente)
        .expect_err("a FASE B trocou um conjunto sem uma das pecas");
    assert!(e.to_string().contains("ABORTADA"), "{e}");
    drop(t);
    conferir_inteira(&dir, "clientes", false);
}
