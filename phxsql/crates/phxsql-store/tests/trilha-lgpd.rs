//! A trilha de LGPD (`.lgpd`) vista pela tabela inteira.
//!
//! # Por que estes testes moram aqui, e nao dentro do modulo
//!
//! Os do modulo provam o ARQUIVO: cabecalho, CRC, corte de UTF-8, redacao.
//! Estes provam a REGRA -- quem gera trilha e quem nao gera --, e para isso
//! precisam de uma `Table` de verdade, com esquema, indice e as tres formas de
//! excluir.
//!
//! Os que mexem no interruptor global ficam no fim, com a razao explicada la:
//! e a mesma do `corte-do-diario.rs`.

mod comum;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn temp(nome: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("trilha-lgpd-{nome}"))
}

/// `clientes` como a tela do Adriano a desenhou: seis colunas marcadas de
/// nove, `limite_credito` deliberadamente SEM marca (o caso «depende»).
fn esquema_marcado(nome: &str) -> Schema {
    let p = |c: Column| c.com_dado_pessoal(DadoPessoal::Pessoal);
    Schema::new(
        nome,
        vec![
            Column::new("id_cliente", ColumnType::Sequence).obrigatoria(),
            p(Column::new("nome", ColumnType::Str(60))),
            p(Column::new("cpf", ColumnType::Str(14)).obrigatoria()),
            p(Column::new("email", ColumnType::Str(80))),
            p(Column::new("telefone", ColumnType::Str(20))),
            p(Column::new("endereco", ColumnType::Str(120))),
            p(Column::new("data_nascimento", ColumnType::Date)),
            Column::new("limite_credito", ColumnType::Int8),
            Column::new("data_cadastro", ColumnType::DateTime),
        ],
        vec![IndexDef::new("por_cpf", vec![IndexColumn::asc(2)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

/// A mesma tabela, sem marca nenhuma. E o controle do custo-zero.
fn esquema_sem_marca(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id_cliente", ColumnType::Sequence).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)),
            Column::new("cpf", ColumnType::Str(14)).obrigatoria(),
            Column::new("email", ColumnType::Str(80)),
            Column::new("telefone", ColumnType::Str(20)),
            Column::new("endereco", ColumnType::Str(120)),
            Column::new("data_nascimento", ColumnType::Date),
            Column::new("limite_credito", ColumnType::Int8),
            Column::new("data_cadastro", ColumnType::DateTime),
        ],
        vec![IndexDef::new("por_cpf", vec![IndexColumn::asc(2)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn linha(cpf: &str, email: &str, telefone: &str) -> Vec<Value> {
    vec![
        Value::Null,
        Value::Str("Ana Prado".into()),
        Value::Str(cpf.into()),
        Value::Str(email.into()),
        Value::Str(telefone.into()),
        Value::Str("Rua das Flores, 10".into()),
        Value::Date(7405),
        Value::Int(150_000),
        Value::DateTime(1_700_000_000_000),
    ]
}

fn abrir(dir: &std::path::Path, e: Schema) -> Table {
    let mut t = Table::criar(dir, e).unwrap();
    t.definir_usuario(7);
    t.definir_origem("192.0.2.10");
    t
}

// ------------------------------------------------------------- o que grava

#[test]
fn alterar_coluna_marcada_grava_antes_e_depois() {
    let d = temp("alterar");
    let mut t = abrir(&d, esquema_marcado("clientes"));
    t.inserir(&linha("012", "ana@x.com", "9111")).unwrap();
    t.atualizar(1, &linha("012", "ana.nova@x.com", "9333"))
        .unwrap();

    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(ev.len(), 2, "duas colunas mudaram, esperava dois registros");
    let por: std::collections::HashMap<_, _> = ev.iter().map(|e| (e.coluna.as_str(), e)).collect();

    let email = por["email"];
    assert_eq!(email.antes, "ana@x.com");
    assert_eq!(email.depois, "ana.nova@x.com");
    assert_eq!(email.ip, "192.0.2.10", "o IP nao foi gravado");
    assert_eq!(email.usuario, 7, "quem alterou nao foi gravado");
    assert_eq!(email.rowid, 1);
    // A identidade sai da chave primaria, e nao do rowid: e ela que continua
    // significando a mesma pessoa depois de a linha sumir.
    assert_eq!(email.identidade, "cpf=012");
    assert_eq!(por["telefone"].antes, "9111");
    assert_eq!(por["telefone"].depois, "9333");
    assert!(
        !por.contains_key("nome"),
        "coluna que nao mudou virou registro"
    );
}

/// Salvar a ficha sem mexer em nada nao pode gerar seis registros dizendo que
/// nada aconteceu -- eles afogariam os que provam alguma coisa.
#[test]
fn salvar_sem_mudar_nada_nao_grava() {
    let d = temp("sem-mudanca");
    let mut t = abrir(&d, esquema_marcado("clientes"));
    t.inserir(&linha("012", "ana@x.com", "9111")).unwrap();
    t.atualizar(1, &linha("012", "ana@x.com", "9111")).unwrap();
    assert_eq!(t.total_da_trilha().unwrap(), 0);
    assert!(!t.tem_trilha(), "o .lgpd nasceu sem ter o que gravar");
}

/// Coluna NAO marcada nao gera trilha, mesmo mudando.
#[test]
fn coluna_sem_marca_nao_gera_trilha() {
    let d = temp("nao-marcada");
    let mut t = abrir(&d, esquema_marcado("clientes"));
    t.inserir(&linha("012", "ana@x.com", "9111")).unwrap();
    let mut nova = linha("012", "ana@x.com", "9111");
    nova[7] = Value::Int(999_999); // limite_credito, o «depende» sem marca
    t.atualizar(1, &nova).unwrap();
    assert_eq!(
        t.total_da_trilha().unwrap(),
        0,
        "uma coluna sem marca gerou trilha"
    );
}

// -------------------------------------------------------- o que NAO grava

/// O pedido do Adriano, em teste: insert, delete e soft delete nao geram
/// trilha. O `.log`, a `.trash` e o `.reason` ja registram os tres, e um
/// segundo registro do mesmo fato cria duas verdades sobre ele.
///
/// **Este e o teste do defeito reposto**: fazer `Table::inserir` chamar
/// `trilhar_alteracao` (ou tirar o `return` de `excluir_suave`) o quebra na
/// hora, na linha que conta os registros.
#[test]
fn insert_delete_e_soft_delete_nao_geram_trilha() {
    let d = temp("nao-gravam");
    let mut t = abrir(&d, esquema_marcado("clientes"));

    t.inserir(&linha("012", "ana@x.com", "9111")).unwrap();
    t.inserir(&linha("013", "bruno@x.com", "9222")).unwrap();
    assert_eq!(t.total_da_trilha().unwrap(), 0, "o insert gravou trilha");
    assert!(!t.tem_trilha(), "o insert criou o .lgpd");

    t.excluir_suave(2, "prova").unwrap();
    assert_eq!(
        t.total_da_trilha().unwrap(),
        0,
        "o soft delete gravou trilha"
    );

    t.restaurar(2, "prova").unwrap();
    assert_eq!(t.total_da_trilha().unwrap(), 0, "o restaurar gravou trilha");

    t.excluir_de_vez(2, "prova").unwrap();
    assert_eq!(
        t.total_da_trilha().unwrap(),
        0,
        "o delete fisico gravou trilha"
    );
    assert!(!t.tem_trilha(), "o .lgpd nasceu sem alteracao nenhuma");

    // E a outra metade do argumento: os tres CONTINUAM registrados onde
    // sempre estiveram. Sem esta parte, o teste acima provaria apenas que a
    // auditoria sumiu.
    assert!(
        t.total_de_motivos().unwrap() >= 3,
        "o .reason parou de registrar as exclusoes"
    );
    assert!(t.eventos().unwrap() >= 3, "o .log parou de registrar");
}

// ------------------------------------------------------------- custo zero

/// **O portao do custo-zero.** Tabela sem coluna marcada nao cria arquivo, nao
/// grava nada e nao muda de comportamento.
///
/// **Este e o outro teste do defeito reposto**: tirar o
/// `if self.colunas_marcadas.is_empty()` de `trilhar_alteracao` faz o
/// `tem_trilha()` virar `true` e o `assert` de arquivo cair.
#[test]
fn tabela_sem_coluna_marcada_nao_paga_nada() {
    let d = temp("custo-zero");
    let mut t = abrir(&d, esquema_sem_marca("produtos"));
    for i in 0..20 {
        t.inserir(&linha(&format!("{i:03}"), "a@x.com", "9111"))
            .unwrap();
        t.atualizar(i + 1, &linha(&format!("{i:03}"), "b@y.com", "9222"))
            .unwrap();
    }
    assert!(t.colunas_marcadas().is_empty());
    assert!(!t.tem_dado_pessoal());
    assert_eq!(t.total_da_trilha().unwrap(), 0);
    assert!(!t.tem_trilha(), "tabela sem marca ganhou um .lgpd");
    assert!(
        !d.join("produtos.lgpd").exists(),
        "o arquivo apareceu no disco"
    );
    // O acesso tambem nao: o portao e o mesmo.
    t.registrar_acesso(1, "varrer tudo", 20).unwrap();
    assert!(
        !t.tem_trilha(),
        "registrar_acesso criou o arquivo sem marca"
    );
}

/// Comportamento velho: uma tabela sem marca abre, le, grava e VERIFICA igual.
/// E o teste que trava a regra da casa -- guarda nova que muda quem nao pediu
/// nao e guarda.
#[test]
fn sem_marca_nada_muda() {
    let d = temp("velho");
    let mut t = abrir(&d, esquema_sem_marca("produtos"));
    t.inserir(&linha("012", "ana@x.com", "9111")).unwrap();
    t.atualizar(1, &linha("012", "ana.nova@x.com", "9333"))
        .unwrap();
    let l = t.ler(1).unwrap().expect("a linha sumiu");
    assert_eq!(l[3], Value::Str("ana.nova@x.com".into()));

    let rel = t.verificar().unwrap();
    assert_eq!(rel.registros, 1);
    assert_eq!(rel.trilha, 0, "a verificacao inventou trilha");

    // Reabrir tem de continuar funcionando, sem o arquivo existir.
    drop(t);
    let mut de_novo = Table::abrir(&d, "produtos").unwrap();
    assert_eq!(de_novo.total_da_trilha().unwrap(), 0);
    assert!(!de_novo.tem_trilha());
    assert_eq!(de_novo.registros(), 1);
}

// ----------------------------------------------------------------- redacao

/// Coluna marcada que guarda senha nao entrega o valor -- nem para a trilha,
/// nem para o `grep` no arquivo.
#[test]
fn senha_em_coluna_marcada_nao_vai_para_a_trilha() {
    let d = temp("senha");
    let e = Schema::new(
        "contas",
        vec![
            Column::new("id", ColumnType::Sequence).obrigatoria(),
            Column::new("login", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("senha_acesso", ColumnType::Str(120))
                .com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![IndexDef::new("por_login", vec![IndexColumn::asc(1)])
            .unico()
            .primaria()],
    )
    .unwrap();
    let mut t = abrir(&d, e);
    let l = |senha: &str| {
        vec![
            Value::Null,
            Value::Str("ana".into()),
            Value::Str(senha.into()),
        ]
    };
    t.inserir(&l("SENHA_VELHA_9911")).unwrap();
    t.atualizar(1, &l("SENHA_NOVA_2288")).unwrap();

    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(ev.len(), 1);
    assert!(ev[0].antes_redigido(), "o valor velho nao foi redigido");
    assert!(ev[0].depois_redigido(), "o valor novo nao foi redigido");
    assert!(!ev[0].antes.contains("SENHA_VELHA"));
    assert!(!ev[0].depois.contains("SENHA_NOVA"));

    let cru = std::fs::read(d.join("contas.lgpd")).unwrap();
    assert!(
        !janela(&cru, b"SENHA_NOVA_2288"),
        "a senha nova esta no arquivo"
    );
    assert!(
        !janela(&cru, b"SENHA_VELHA_9911"),
        "a senha velha esta no arquivo"
    );
    // A prova pelo contrario: o `grep` FUNCIONA neste arquivo. Sem ela os dois
    // asserts acima passariam ate num arquivo vazio, e teste que passa por
    // engano e pior que teste que falta.
    assert!(
        janela(&cru, b"senha_acesso"),
        "o grep nao acha nem o nome da coluna: o teste passaria por engano"
    );
}

fn janela(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

// ------------------------------------------------------------------ acesso

#[test]
fn acesso_e_um_registro_por_operacao() {
    let d = temp("acesso");
    let mut t = abrir(&d, esquema_marcado("clientes"));
    for i in 0..50 {
        t.inserir(&linha(&format!("{i:03}"), "a@x.com", "9111"))
            .unwrap();
    }
    t.registrar_acesso(0, "varrer ordem=digitacao visao=ativas", 50)
        .unwrap();
    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(ev.len(), 1, "50 linhas lidas viraram mais de um registro");
    assert_eq!(ev[0].linhas, 50, "a contagem de linhas nao foi gravada");
    assert_eq!(ev[0].identidade, "varrer ordem=digitacao visao=ativas");
    // As colunas marcadas que a operacao tocou vao na lista, em ordem.
    assert_eq!(
        ev[0].coluna,
        "nome,cpf,email,telefone,endereco,data_nascimento"
    );
    assert_eq!(ev[0].ip, "192.0.2.10");
}

/// Uma consulta que nao devolveu ninguem nao expos dado de ninguem.
#[test]
fn acesso_que_nao_devolveu_linha_nao_grava() {
    let d = temp("acesso-vazio");
    let mut t = abrir(&d, esquema_marcado("clientes"));
    t.inserir(&linha("012", "ana@x.com", "9111")).unwrap();
    t.registrar_acesso(0, "por_cpf=999", 0).unwrap();
    assert_eq!(t.total_da_trilha().unwrap(), 0);
    assert!(!t.tem_trilha());
}

// ------------------------------------------------------------ marcar depois

/// Marcar uma coluna DEPOIS liga a trilha dela; desmarcar volta ao custo-zero.
/// O que este teste trava e a lista guardada: se `marcar_dado_pessoal` nao
/// recalculasse `colunas_marcadas`, a coluna recem-marcada continuaria muda e
/// ninguem descobriria por leitura.
#[test]
fn marcar_depois_liga_a_trilha_da_coluna() {
    let d = temp("marcar-depois");
    let mut t = abrir(&d, esquema_sem_marca("produtos"));
    t.inserir(&linha("012", "ana@x.com", "9111")).unwrap();
    t.atualizar(1, &linha("012", "b@x.com", "9111")).unwrap();
    assert_eq!(t.total_da_trilha().unwrap(), 0);

    t.marcar_dado_pessoal(&[("email".to_string(), DadoPessoal::Pessoal)])
        .unwrap();
    assert_eq!(t.colunas_marcadas(), vec!["email"]);
    t.atualizar(1, &linha("012", "c@x.com", "9111")).unwrap();
    assert_eq!(t.total_da_trilha().unwrap(), 1, "a marca nova nao pegou");

    t.marcar_dado_pessoal(&[("email".to_string(), DadoPessoal::Nao)])
        .unwrap();
    assert!(t.colunas_marcadas().is_empty());
    t.atualizar(1, &linha("012", "d@x.com", "9111")).unwrap();
    assert_eq!(
        t.total_da_trilha().unwrap(),
        1,
        "desmarcar nao desligou a trilha"
    );

    // E a marca sobrevive a reabertura, porque foi para o `.reg`.
    t.marcar_dado_pessoal(&[("telefone".to_string(), DadoPessoal::Sensivel)])
        .unwrap();
    t.sincronizar().unwrap();
    drop(t);
    let de_novo = Table::abrir(&d, "produtos").unwrap();
    assert_eq!(de_novo.colunas_marcadas(), vec!["telefone"]);
}
