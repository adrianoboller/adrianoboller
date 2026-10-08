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
use phxsql_store::ndx::panico_de_teste::{armar, armar_na, desarmar, Ponto};
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
    // Pedido 657: «ABORTADA» sai de tres recusas do `reg.rs`; a do conjunto
    // incompleto e a unica que diz «sumiu antes da troca».
    assert!(e.to_string().contains("sumiu antes da troca"), "{e}");
    drop(t);
    conferir_inteira(&dir, "clientes", false);
}

// ------------------------- pedido 631: a irma em troca na busca reversa

/// Uma tabela qualquer no mesmo diretorio, com duas linhas, para excluir.
fn vizinha(dir: &std::path::Path) -> Table {
    let e = Schema::new(
        "vizinha",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut t = Table::criar(dir, e).unwrap();
    t.inserir(&[Value::Int(1)]).unwrap();
    t.inserir(&[Value::Int(2)]).unwrap();
    t.sincronizar().unwrap();
    t
}

/// **Pedido 631, o irmao que nao pode virar recusa eterna:** desde que a
/// irma que nao abre RECUSA a exclusao, a irma em troca interrompida tem de
/// ABRIR -- e abre, pelo `terminar_troca_interrompida` do `RegFile::abrir`.
/// Nas duas mortes: a troca decidida (termina para a frente) e a sobra da
/// FASE A (fica, e nao atrapalha).
///
/// # Prova real
///
/// Troque o `RegFile::abrir` do `irmas::abrir_irma` por uma abertura que nao
/// cura (`RegFile::abrir_sem_escrever`, com o `None` virando erro): a troca
/// decidida «nao abre», e a exclusao da vizinha recusa -- medido, o teste
/// cai no `unwrap_or_else` do `excluir_de_vez`.
#[test]
fn a_irma_em_troca_interrompida_nao_tranca_a_exclusao_da_vizinha() {
    for (rotulo, ponto, com_coluna) in [
        ("irma-decidida", Ponto::FaseBDepoisDoVolume1, true),
        ("irma-sobra", Ponto::FaseBAntesDaPrimeiraTroca, false),
    ] {
        let (_cat, _db, d) = base(rotulo, true);
        let dir = d.join("loja");
        let mut v = vizinha(&dir);
        morrer_na_fase_b(&dir, ponto);
        assert!(
            !novos_do_reg(&dir, "clientes").is_empty(),
            "{rotulo}: o panico nao deixou `*.novo`"
        );
        let saiu = v
            .excluir_de_vez(1, "t")
            .unwrap_or_else(|e| panic!("{rotulo}: a irma em troca trancou: {e}"));
        assert!(saiu, "{rotulo}");
        assert!(v.excluir_suave(2, "t").unwrap(), "{rotulo}");
        conferir_inteira(&dir, "clientes", com_coluna);
    }
}

// ------------------- pedido 632: o `*.novo` pela metade da troca de uma fase

/// Dois indices de nome comprido: o bloco de esquema cresce mais que a folga
/// de 64 bytes antes do slot 1, e a regravacao vai pelo caminho CARO.
fn indices_compridos() -> Vec<IndexDef> {
    (0..2)
        .map(|i| {
            IndexDef::new(
                format!("porNome_indice_de_nome_bem_comprido_para_nao_caber_{i}"),
                vec![IndexColumn::asc(NOME)],
            )
        })
        .collect()
}

/// As linhas de `clientes`, com qualquer versao do esquema.
fn conferir_linhas(dir: &std::path::Path) {
    let mut t = Table::abrir(dir, "clientes").unwrap_or_else(|e| panic!("clientes nao abre: {e}"));
    let linhas = t.varrer_com(Visao::Todas).unwrap();
    assert_eq!(linhas.len() as i64, LINHAS, "clientes perdeu linhas");
    for (i, (rowid, v)) in linhas.iter().enumerate() {
        assert_eq!(*rowid, i as u64 + 1);
        assert_eq!(v[NOME], Value::Str(format!("cliente {:04}", i + 1)));
    }
}

/// **Pedido 632:** o `acrescentar_indices` regrava o esquema de uma fase so.
/// Ate aqui ele escrevia E trocava volume a volume: o volume 1 ja novo, uma
/// queda no meio do `*.novo` do volume 2 deixava um arquivo de cabecalho
/// valido e sem slot nenhum -- e a abertura seguinte o tomava pela troca
/// decidida e o renomeava por cima do volume 2 velho.
///
/// # Prova real
///
/// Com o caminho caro do `regravar_esquema` de volta ao laco de antes
/// (escrever e trocar volume a volume), medido em 01/10/2026: a abertura
/// renomeia o `clientes#002.reg.novo` de 1.088 bytes (cabecalho e esquema,
/// zero slot) por cima do `clientes#002.reg` de 3.900 -- as 30 linhas do
/// volume 2 destruidas no disco -- e a tabela nao abre mais («ficou pela
/// metade» no volume 3). Com o laco de antes e o cinto do `novo_completo`, o
/// volume 2 fica inteiro mas a tabela tambem nao abre: o teste cai do mesmo
/// jeito, e e o conserto da ORDEM que o faz passar.
#[test]
fn a_queda_no_meio_do_novo_do_volume_2_nao_perde_linha() {
    let (_cat, _db, d) = base("uma-fase-v2", true);
    let dir = d.join("loja");
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    armar_na(Ponto::NoMeioDoNovoDoVolume, 2);
    let morreu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = t.acrescentar_indices(indices_compridos());
    }));
    desarmar();
    assert!(
        morreu.is_err(),
        "o panico nao aconteceu: o esquema coube e a regravacao nao foi pelo caminho caro"
    );
    drop(t);
    conferir_linhas(&dir);
    // A abertura gravavel recolheu a sobra, e a operacao refeita fecha.
    assert_eq!(novos_do_reg(&dir, "clientes"), Vec::<String>::new());
    let mut t = Table::abrir(&dir, "clientes").unwrap();
    t.acrescentar_indices(indices_compridos()).unwrap();
    drop(t);
    conferir_linhas(&dir);
}

/// O cinto do mesmo pedido, na ABERTURA: uma troca decidida (volume 1 ja
/// novo) cujo `*.novo` do volume 3 nao tem todos os slots do volume velho nao
/// entra no lugar dele. A abertura recusa o conjunto misturado -- recusa que
/// se le, em vez de 30 linhas sumidas sem aviso.
///
/// O `*.novo` encurtado aqui e montado a mao, e e de proposito: depois do
/// conserto do caminho de uma fase, nenhum caminho do motor o deixa assim (a
/// FASE A sincroniza todos antes do primeiro `rename`). A prova e do cinto,
/// para o dia em que um caminho novo trocar antes de escrever tudo.
///
/// # Prova real
///
/// Sem a conferencia da contagem de slots no `separar_novos_ao_lado`
/// (medido em 01/10/2026), o `clientes#003.reg.novo` encurtado e renomeado
/// por cima do volume velho, a tabela ABRE, e a varredura morre em
/// «failed to fill whole buffer» -- o volume 3 ja perdido no disco.
#[test]
fn a_troca_decidida_nao_renomeia_novo_incompleto() {
    let (_cat, _db, d) = base("cinto-curto", true);
    let dir = d.join("loja");
    morrer_na_fase_b(&dir, Ponto::FaseBDepoisDoVolume1);
    let novos = novos_do_reg(&dir, "clientes");
    let curto = novos
        .iter()
        .find(|n| n.contains("003"))
        .unwrap_or_else(|| panic!("sem o `*.novo` do volume 3: {novos:?}"))
        .clone();
    let caminho = dir.join(&curto);
    let tamanho = std::fs::metadata(&caminho).unwrap().len();
    let velho = std::fs::metadata(dir.join("clientes#003.reg"))
        .unwrap()
        .len();
    // Um slot a menos e um pedaco: nem multiplo, nem completo.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&caminho)
        .unwrap()
        .set_len(tamanho - 100)
        .unwrap();
    match Table::abrir(&dir, "clientes") {
        Err(e) => {
            let e = e.to_string();
            assert!(e.contains("pela metade"), "{e}");
            // A recusa nao manda repor o arquivo que perde linhas.
            assert!(e.contains("INCOMPLETO") && !e.contains("reponha"), "{e}");
        }
        Ok(mut t) => {
            let n = t.varrer_com(Visao::Todas).map(|l| l.len());
            panic!("o conjunto com o volume 3 encurtado abriu: {n:?}")
        }
    }
    assert!(
        caminho.exists(),
        "o `*.novo` encurtado sumiu: foi renomeado"
    );
    assert_eq!(
        std::fs::metadata(dir.join("clientes#003.reg"))
            .unwrap()
            .len(),
        velho,
        "o volume 3 velho foi trocado"
    );
}
