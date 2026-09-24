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

// ------------------------------------------------- coluna EXTERNA marcada
//
// Pedido 367. A trilha de coluna `Bin`/`Memo` marcada MENTIA nos tres
// sentidos, e os tres saiam da MESMA linha: o `atualizar` decodifica a linha
// velha com `carregar_externos = false` -- porque so precisa dela para os
// indices --, e nesse modo `Bin` e `Memo` voltam `Value::Null`. O par
// antes/depois da trilha comparava esse `Null` com o valor do chamador.
//
// Os tres testes abaixo sao a prova real, e os tres FALHAVAM antes do
// conserto: o primeiro com `antes` vazio, o segundo com zero registros, o
// terceiro com um registro que ninguem pediu.

/// `prontuarios`: a coluna externa marcada e o caso que a lei mais protege --
/// laudo medico (`Memo`) e biometria (`Bin`), os dois `sensivel`.
fn esquema_com_externo(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Sequence).obrigatoria(),
            Column::new("paciente", ColumnType::Str(60)).com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("laudo", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
            Column::new("foto", ColumnType::Bin).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![IndexDef::new("por_id", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

/// `(id, paciente, laudo, foto)` -- `laudo` e `foto` entram como o chamador
/// mandar, inclusive `Value::Null` para apagar.
fn ficha(paciente: &str, laudo: Value, foto: Value) -> Vec<Value> {
    vec![Value::Null, Value::Str(paciente.into()), laudo, foto]
}

/// O unico registro daquela coluna. Falha dizendo QUANTOS achou, porque
/// zero e um-a-mais sao defeitos diferentes: zero e a trilha calada, e dois e
/// a trilha inventando.
fn so_um<'a>(
    ev: &'a [phxsql_store::trilha::Evento],
    coluna: &str,
) -> &'a phxsql_store::trilha::Evento {
    let achados: Vec<_> = ev.iter().filter(|e| e.coluna == coluna).collect();
    assert_eq!(
        achados.len(),
        1,
        "esperava UM registro de {coluna}, achei {} (colunas: {:?})",
        achados.len(),
        ev.iter().map(|e| e.coluna.as_str()).collect::<Vec<_>>()
    );
    achados[0]
}

fn foto_de(n: usize, semente: u8) -> Value {
    Value::Bin((0..n).map(|i| (i as u8).wrapping_add(semente)).collect())
}

/// Alterar um laudo grava o laudo VELHO em `antes`, e nao um vazio.
///
/// # O defeito reposto
///
/// Trocar a leitura dos externos velhos por `Value::Null` -- que era o que o
/// `decodificar(&antigo, false)` entregava -- faz este teste falhar na linha
/// do `antes`, com `""` no lugar do laudo. Registro de auditoria que afirma
/// um fato falso e pior que registro ausente.
#[test]
fn a_trilha_de_memo_marcado_nao_mente_sobre_o_valor_antigo() {
    let d = temp("memo-antigo");
    let mut t = abrir(&d, esquema_com_externo("prontuarios"));
    let foto = foto_de(2048, 0);
    t.inserir(&ficha(
        "Ana Prado",
        Value::Memo("LAUDO_VELHO: benigno".into()),
        foto.clone(),
    ))
    .unwrap();
    t.atualizar(
        1,
        &ficha(
            "Ana Prado",
            Value::Memo("LAUDO_NOVO: carcinoma, CID C50".into()),
            foto,
        ),
    )
    .unwrap();

    let ev = t.trilha(0, 0).unwrap();
    let laudo = so_um(&ev, "laudo");
    assert_eq!(
        laudo.antes, "LAUDO_VELHO: benigno",
        "a trilha mentiu sobre o valor antigo do laudo"
    );
    assert_eq!(laudo.depois, "LAUDO_NOVO: carcinoma, CID C50");
    assert!(
        !laudo.antes_indisponivel(),
        "o laudo velho estava no .memo e era legivel"
    );
    assert!(!laudo.antes_redigido() && !laudo.depois_redigido());
    assert_eq!(
        ev.len(),
        1,
        "so o laudo mudou; a foto e o nome nao, e nao podem virar registro"
    );
}

/// Apagar o laudo gera registro. E o evento que a lei mais quer ver, e era o
/// unico que a trilha nao gravava: `Null != Null` e falso, e o filtro cortava.
///
/// # O defeito reposto
///
/// Voltar o `antes` do laudo para `Value::Null` iguala os dois lados e este
/// teste falha com zero registros -- a supressao de dado sensivel invisivel.
#[test]
fn apagar_memo_marcado_gera_registro_de_trilha() {
    let d = temp("memo-apagado");
    let mut t = abrir(&d, esquema_com_externo("prontuarios"));
    let foto = foto_de(2048, 0);
    t.inserir(&ficha(
        "Ana Prado",
        Value::Memo("LAUDO_VELHO: benigno".into()),
        foto.clone(),
    ))
    .unwrap();
    t.atualizar(1, &ficha("Ana Prado", Value::Null, foto))
        .unwrap();

    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(
        ev.iter().filter(|e| e.coluna == "laudo").count(),
        1,
        "apagar o laudo nao gerou registro nenhum: Null != Null e falso, e o \
         filtro cortava o evento que a lei mais quer ver"
    );
    let laudo = so_um(&ev, "laudo");
    assert_eq!(laudo.antes, "LAUDO_VELHO: benigno");
    assert_eq!(
        laudo.depois, "",
        "nulo e ausencia de valor, e a trilha o mostra como tal"
    );
    assert_eq!(laudo.identidade, "id=1");
    assert_eq!(
        ev.len(),
        1,
        "a foto nao foi tocada e nao pode virar registro"
    );
}

/// Salvar a ficha sem tocar na foto nao pode dizer que a foto mudou.
///
/// A garantia que o comentario do `trilhar_alteracao` ja prometia -- «salvar a
/// ficha sem mexer em nada geraria seis registros dizendo que nada
/// aconteceu» -- nao alcancava coluna externa.
///
/// # O defeito reposto
///
/// Voltar o `antes` da foto para `Value::Null` faz este teste falhar com um
/// registro dizendo `antes="" depois="2048 bytes"`.
#[test]
fn salvar_sem_tocar_no_bin_marcado_nao_gera_trilha() {
    let d = temp("bin-intocado");
    let mut t = abrir(&d, esquema_com_externo("prontuarios"));
    let laudo = Value::Memo("LAUDO: benigno".into());
    let foto = foto_de(2048, 0);
    t.inserir(&ficha("Ana Prado", laudo.clone(), foto.clone()))
        .unwrap();
    t.atualizar(1, &ficha("Ana Prado", laudo, foto)).unwrap();

    assert_eq!(
        t.total_da_trilha().unwrap(),
        0,
        "salvar sem tocar em nada gerou registro de trilha"
    );
    assert!(!t.tem_trilha(), "o .lgpd nasceu sem ter o que gravar");
}

/// E a prova pelo contrario do teste acima: trocar a foto por outra do MESMO
/// tamanho gera registro. Sem ela, «nao gera registro» passaria ate num
/// conserto que desligasse a trilha da coluna externa inteira.
#[test]
fn trocar_a_foto_por_outra_do_mesmo_tamanho_gera_registro() {
    let d = temp("bin-trocado");
    let mut t = abrir(&d, esquema_com_externo("prontuarios"));
    let laudo = Value::Memo("LAUDO: benigno".into());
    t.inserir(&ficha("Ana Prado", laudo.clone(), foto_de(2048, 0)))
        .unwrap();
    t.atualizar(1, &ficha("Ana Prado", laudo, foto_de(2048, 7)))
        .unwrap();

    let ev = t.trilha(0, 0).unwrap();
    let foto = so_um(&ev, "foto");
    // Biometria nao vira texto na trilha: vira tamanho. Ver `valor_para_trilha`.
    assert_eq!(foto.antes, "2048 bytes");
    assert_eq!(foto.depois, "2048 bytes");
    assert_eq!(ev.len(), 1, "so a foto mudou");
}

/// O teste do comportamento VELHO: coluna INLINE marcada trilha exatamente
/// como trilhava, na mesma tabela que agora tem coluna externa marcada.
///
/// Guarda nova entra pedida, nao imposta -- e o que nao se pediu e que a
/// leitura do externo velho mudasse o par antes/depois de quem nunca saiu do
/// `.reg`.
#[test]
fn coluna_inline_marcada_continua_trilhando_como_antes() {
    let d = temp("inline-como-antes");
    let mut t = abrir(&d, esquema_com_externo("prontuarios"));
    let laudo = Value::Memo("LAUDO: benigno".into());
    let foto = foto_de(2048, 0);
    t.inserir(&ficha("Ana Prado", laudo.clone(), foto.clone()))
        .unwrap();
    t.atualizar(1, &ficha("Ana Prado Silva", laudo, foto))
        .unwrap();

    let ev = t.trilha(0, 0).unwrap();
    let paciente = so_um(&ev, "paciente");
    assert_eq!(paciente.antes, "Ana Prado");
    assert_eq!(paciente.depois, "Ana Prado Silva");
    assert_eq!(paciente.usuario, 7);
    assert_eq!(paciente.ip, "192.0.2.10");
    assert_eq!(ev.len(), 1, "so o nome mudou");
}

/// Quando o `.memo` velho NAO se le, a trilha diz isso -- e nao inventa um
/// vazio que passaria por «o campo estava em branco».
///
/// O arquivo e adulterado de proposito com a tabela fechada; a leitura do
/// bloco falha no CRC-32 (ver `blob.rs`), e o registro sai com a marca.
#[test]
fn memo_marcado_ilegivel_sai_como_indisponivel_e_nao_como_vazio() {
    let d = temp("memo-ilegivel");
    {
        let mut t = abrir(&d, esquema_com_externo("prontuarios"));
        t.inserir(&ficha(
            "Ana Prado",
            Value::Memo("LAUDO_VELHO: benigno".into()),
            foto_de(64, 0),
        ))
        .unwrap();
        t.sincronizar().unwrap();
    }

    // Vira o ultimo byte do arquivo: ele esta dentro do conteudo do unico
    // bloco gravado, entao o CRC do bloco deixa de bater.
    let caminho = d.join("prontuarios.memo");
    let mut cru = std::fs::read(&caminho).unwrap();
    let ultimo = cru.len() - 1;
    cru[ultimo] ^= 0xFF;
    std::fs::write(&caminho, &cru).unwrap();

    let mut t = Table::abrir(&d, "prontuarios").unwrap();
    t.definir_usuario(7);
    t.definir_origem("192.0.2.10");
    t.atualizar(
        1,
        &ficha(
            "Ana Prado",
            Value::Memo("LAUDO_NOVO: carcinoma".into()),
            foto_de(64, 0),
        ),
    )
    .unwrap();

    let ev = t.trilha(0, 0).unwrap();
    let laudo = so_um(&ev, "laudo");
    assert!(
        laudo.antes_indisponivel(),
        "o valor velho era ilegivel e a trilha nao disse isso"
    );
    assert_ne!(
        laudo.antes, "",
        "vazio mentiria: diria que o campo estava em branco"
    );
    assert_eq!(laudo.depois, "LAUDO_NOVO: carcinoma");
    assert_eq!(ev.len(), 1, "so o laudo mudou");
}

// ------------------------------------------------------ expurgo (pedido 368)

/// `clientes` com o `email` marcado, SEM paginacao -- a tabela padrao do
/// protocolo, a que o formato B passou a expurgar.
fn esquema_padrao() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("email", ColumnType::Str(80)).com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn alterar_email(t: &mut Table, i: u32) {
    t.atualizar(1, &[Value::Int(1), Value::Str(format!("a{i}@x.com"))])
        .unwrap();
}

/// **O expurgo pela tabela padrao, de ponta a ponta.** Vinte alteracoes por
/// volume, fechados a pedido: tres volumes fechados (`clientes_001` a `_003`)
/// e o ativo 4 em `clientes.lgpd`. O expurgo com um limite depois de tudo
/// derruba os fechados e deixa o ativo, grava o rastro no `.reason` com o bit
/// do expurgo da trilha e sem a chave de linha, e a trilha que sobra continua
/// lendo, contando e crescendo -- nunca de volta ao volume 1.
///
/// O disco e conferido pelo SISTEMA DE ARQUIVOS (`exists` de cada nome), e
/// nao pelo que a tabela diz de si mesma.
#[test]
fn o_expurgo_pela_tabela_padrao_grava_o_rastro_e_so_derruba_volume_fechado() {
    let d = temp("expurgo");
    let mut t = abrir(&d, esquema_padrao());
    t.inserir(&[Value::Int(1), Value::Str("a0@x.com".into())])
        .unwrap();
    let mut n = 0;
    for _ in 0..3 {
        for _ in 0..20 {
            n += 1;
            alterar_email(&mut t, n);
        }
        assert!(t.fechar_volume_da_trilha(true, 0).unwrap().is_some());
    }
    for _ in 0..5 {
        n += 1;
        alterar_email(&mut t, n);
    }
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![1, 2, 3, 4]);
    for nome in [
        "clientes.lgpd",
        "clientes_001.lgpd",
        "clientes_002.lgpd",
        "clientes_003.lgpd",
    ] {
        assert!(d.join(nome).exists(), "{nome} nao existe");
    }
    let total_antes = t.total_da_trilha().unwrap();
    assert_eq!(total_antes, 65);

    // Sem motivo, recusa -- e nada sai.
    assert!(t.expurgar_trilha(i64::MAX, "   ").is_err());
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![1, 2, 3, 4]);

    let limite = phxsql_core::datahora::ms_de_instante_iso("2099-01-01").unwrap();
    let selado = t
        .expurgar_trilha(limite, "prazo de guarda vencido")
        .unwrap();
    let e = selado.expurgo();
    assert_eq!(e.volumes.len(), 3);
    assert_eq!(e.registros(), 60);
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![4]);
    for (nome, fica) in [
        ("clientes.lgpd", true),
        ("clientes_001.lgpd", false),
        ("clientes_002.lgpd", false),
        ("clientes_003.lgpd", false),
    ] {
        assert_eq!(
            d.join(nome).exists(),
            fica,
            "{nome}: o disco nao e o que o expurgo disse"
        );
    }

    // O rastro: quem, o motivo, o que saiu, o BIT -- e nenhuma chave de linha.
    let rastro: Vec<_> = t
        .motivos(0, 0)
        .unwrap()
        .into_iter()
        .filter(|m| m.tipo == phxsql_store::motivo::Tipo::Expurgo)
        .collect();
    assert_eq!(rastro.len(), 1, "{rastro:?}");
    assert!(rastro[0].expurgo_da_trilha(), "o rastro saiu sem o bit C1");
    assert_eq!(rastro[0].usuario, 7, "quem pediu nao ficou no rastro");
    assert_eq!(rastro[0].motivo, "prazo de guarda vencido");
    assert_eq!(rastro[0].rowid, 0);
    assert!(
        rastro[0].identidade.starts_with(".lgpd volumes 1-3 "),
        "{}",
        rastro[0].identidade
    );

    // A trilha que sobra.
    let resto = t.trilha(0, 0).unwrap();
    assert_eq!(resto.len() as u64, t.total_da_trilha().unwrap());
    assert_eq!(resto.len() as u64, total_antes - e.registros());

    // Reaberta, continua -- e o proximo volume e o 5.
    drop(t);
    let mut t = Table::abrir(&d, "clientes").unwrap();
    t.definir_usuario(7);
    n += 1;
    alterar_email(&mut t, n);
    assert_eq!(t.fechar_volume_da_trilha(true, 0).unwrap(), Some(4));
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![4, 5]);
    assert_eq!(
        t.trilha(0, 0).unwrap().len() as u64,
        t.total_da_trilha().unwrap()
    );
}

/// O rastro do esvaziar da LIXEIRA continua sem o bit: o tipo 4 tem dois
/// donos, e quem os separa e o byte 9, nunca o texto.
///
/// **Defeito reposto** (o `registrar_expurgo_da_trilha` sem a flag): o
/// rastro da trilha sai com o byte 9 em 0 e a primeira asserção cai.
#[test]
fn o_bit_do_expurgo_separa_a_trilha_da_lixeira_no_disco() {
    let d = temp("bit-c1");
    let mut t = abrir(&d, esquema_padrao());
    t.inserir(&[Value::Int(1), Value::Str("a0@x.com".into())])
        .unwrap();
    alterar_email(&mut t, 1);
    t.fechar_volume_da_trilha(true, 0).unwrap();
    let limite = phxsql_core::datahora::ms_de_instante_iso("2099-01-01").unwrap();
    t.expurgar_trilha(limite, "retencao").unwrap();
    t.esvaziar_lixeira("limpeza").unwrap();
    t.sincronizar().unwrap();
    let m = t.motivos(0, 0).unwrap();
    let flags: Vec<(bool, u8)> = m
        .iter()
        .filter(|m| m.tipo == phxsql_store::motivo::Tipo::Expurgo)
        .map(|m| (m.expurgo_da_trilha(), m.flags))
        .collect();
    assert_eq!(flags, vec![(true, 1), (false, 0)]);
    // E no DISCO: o byte 9 do registro do rastro, lido cru do `.reason`.
    let bruto = std::fs::read(d.join("clientes.reason")).unwrap();
    let pos = bruto
        .windows(13)
        .position(|w| w == b".lgpd volumes")
        .expect("o rastro em claro no .reason");
    let inicio = pos - 48 - "retencao".len();
    assert_eq!(bruto[inicio + 8], 4, "tipo");
    assert_eq!(bruto[inicio + 9], 1, "o bit C1 nao foi ao disco");
}

/// **O P1 do papel C, pela `Table`.** O fechamento faz nascer o ativo novo e
/// nao o leva ao disco (o `sincronizar` depois dele ainda deve a trilha); a
/// queda de energia antes do writeback deixa `clientes.lgpd` com 0 byte. A
/// tabela tem de ABRIR -- leitura e escrita --, com a trilha inteira do
/// volume fechado, e o proximo evento nasce o ativo 2 por cima do arquivo
/// vazio.
///
/// **Defeito reposto** (sem a regra do nascimento interrompido no
/// `TrilhaFile::abrir`): `Table::abrir` cai com «failed to fill whole
/// buffer», o medido pelo papel C.
#[test]
fn a_tabela_abre_com_o_ativo_da_trilha_de_zero_byte() {
    let d = temp("ativo-zero");
    let mut t = abrir(&d, esquema_padrao());
    t.inserir(&[Value::Int(1), Value::Str("a0@x.com".into())])
        .unwrap();
    alterar_email(&mut t, 1);
    assert_eq!(t.fechar_volume_da_trilha(true, 0).unwrap(), Some(1));
    drop(t);
    std::fs::OpenOptions::new()
        .write(true)
        .open(d.join("clientes.lgpd"))
        .unwrap()
        .set_len(0)
        .unwrap();

    let mut t = match Table::abrir(&d, "clientes") {
        Ok(t) => t,
        Err(e) => panic!("a tabela trancou pelo ativo de 0 byte: {e}"),
    };
    t.definir_usuario(7);
    assert_eq!(t.trilha(0, 0).unwrap().len(), 1);
    alterar_email(&mut t, 2);
    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(ev.len(), 2);
    assert_eq!(ev.last().unwrap().depois, "a2@x.com");
    assert_eq!(
        lgpd_no_disco(&d),
        vec!["clientes.lgpd", "clientes_001.lgpd"]
    );
}

/// A trilha gravada ANTES do formato B pelo codigo de antes
/// (`tests/fixtures/trilha-antes-do-b/`, commit `82a17ef`), copiada para um
/// diretorio temporario e aberta pela tabela: le tudo, conta tudo, e o
/// proximo evento segue a regra nova.
fn migrar(caso: &str) -> (comum::DirTemp, Table) {
    let origem = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/trilha-antes-do-b")
        .join(caso);
    let d = temp(&format!("migra-{caso}"));
    for e in std::fs::read_dir(&origem).unwrap().flatten() {
        std::fs::copy(e.path(), d.join(e.file_name())).unwrap();
    }
    let mut t = Table::abrir(&d, "clientes").unwrap();
    t.definir_usuario(7);
    (d, t)
}

fn lgpd_no_disco(d: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".lgpd"))
        .collect();
    v.sort();
    v
}

/// **Migracao 1: a trilha de arquivo unico** (tabela sem paginacao). O
/// `clientes.lgpd` de antes ja e o ativo, volume 1: le os 5, e o proximo
/// evento entra NELE, sem arquivo novo. Fechado a pedido, vira
/// `clientes_001.lgpd` e se expurga.
#[test]
fn a_trilha_de_arquivo_unico_de_antes_vira_o_ativo() {
    let (d, mut t) = migrar("unico");
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![1]);
    assert_eq!(t.total_da_trilha().unwrap(), 5);
    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(ev.last().unwrap().depois, "a5@x.com");
    alterar_email(&mut t, 6);
    assert_eq!(lgpd_no_disco(&d), vec!["clientes.lgpd"]);
    assert_eq!(t.total_da_trilha().unwrap(), 6);
    assert_eq!(t.fechar_volume_da_trilha(true, 0).unwrap(), Some(1));
    let limite = phxsql_core::datahora::ms_de_instante_iso("2099-01-01").unwrap();
    t.expurgar_trilha(limite, "migracao").unwrap();
    assert_eq!(lgpd_no_disco(&d), vec!["clientes.lgpd"]);
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![2]);
}

/// **Migracao 2: a trilha paginada** `_001` a `_003`, sem `clientes.lgpd`.
/// Os tres viram fechados, o ativo nasce 4 no primeiro evento, e o expurgo
/// derruba os tres de antes.
///
/// **Defeito reposto** (sem o ativo, a resolucao supoe o volume 1 em vez de
/// listar o diretorio): a trilha migrada aparece vazia, e a primeira
/// asserção cai.
#[test]
fn a_trilha_paginada_de_antes_vira_fechados_e_o_ativo_nasce_depois() {
    let (d, mut t) = migrar("paginada");
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![1, 2, 3]);
    assert_eq!(t.total_da_trilha().unwrap(), 60);
    assert_eq!(t.trilha(0, 0).unwrap()[59].depois, "a60@x.com");
    alterar_email(&mut t, 61);
    assert_eq!(
        lgpd_no_disco(&d),
        vec![
            "clientes.lgpd",
            "clientes_001.lgpd",
            "clientes_002.lgpd",
            "clientes_003.lgpd"
        ]
    );
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![1, 2, 3, 4]);
    let limite = phxsql_core::datahora::ms_de_instante_iso("2099-01-01").unwrap();
    let s = t.expurgar_trilha(limite, "migracao").unwrap();
    assert_eq!(s.expurgo().registros(), 60);
    assert_eq!(lgpd_no_disco(&d), vec!["clientes.lgpd"]);
    assert_eq!(t.total_da_trilha().unwrap(), 1);
}

/// **Migracao 3: a trilha paginada de sufixo com QUATRO digitos**
/// (`_0001` a `_0003`). Os nomes antigos ficam como estao -- nada se renomeia
/// nem se reescreve --, sao lidos pelo nome em que nasceram, o ativo nasce 4,
/// o volume 4 fecha no nome canonico `_004`, e o expurgo leva os quatro.
///
/// **Defeito reposto** (sem procurar o nome legado): a trilha migrada aparece
/// vazia, e a primeira asserção cai.
#[test]
fn a_trilha_de_sufixo_de_quatro_digitos_de_antes_continua_legivel() {
    let (d, mut t) = migrar("paginada-4-digitos");
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![1, 2, 3]);
    assert_eq!(t.total_da_trilha().unwrap(), 60);
    alterar_email(&mut t, 61);
    assert_eq!(t.fechar_volume_da_trilha(true, 0).unwrap(), Some(4));
    assert_eq!(
        lgpd_no_disco(&d),
        vec![
            "clientes.lgpd",
            "clientes_0001.lgpd",
            "clientes_0002.lgpd",
            "clientes_0003.lgpd",
            "clientes_004.lgpd"
        ]
    );
    drop(t);
    let mut t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(t.volumes_da_trilha().unwrap(), vec![1, 2, 3, 4, 5]);
    assert_eq!(t.total_da_trilha().unwrap(), 61);
    let limite = phxsql_core::datahora::ms_de_instante_iso("2099-01-01").unwrap();
    let s = t.expurgar_trilha(limite, "migracao").unwrap();
    assert_eq!(s.expurgo().registros(), 61);
    assert_eq!(lgpd_no_disco(&d), vec!["clientes.lgpd"]);
}

// --------------------------------------- o teto que o formato B tirou (A1)

/// `clientes` com o `email` marcado e a paginacao do `.reg` com teto de TRES
/// volumes e 2 KiB por volume de diario -- o cenario do achado A1 do parecer
/// do DBA (pedido 368).
fn esquema_com_teto(max_arquivos: u32) -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("email", ColumnType::Str(80)).com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
    .com_paginacao(
        phxsql_core::paginacao::Paginacao::nova(1_000, max_arquivos)
            .unwrap()
            .com_bytes_por_arquivo(2_048)
            .unwrap(),
    )
    .unwrap()
}

/// **O teto de volumes da trilha nao existe mais** -- a metade «teto» do
/// achado A1 do DBA deixa de reproduzir.
///
/// Antes do formato B a trilha seguia a paginacao do `.reg`: o teto dela era o
/// `max_arquivos` da TABELA, e no teto o `atualizar` gravava a linha e o
/// `.log` e so entao falhava na trilha -- linha alterada, sem trilha, e erro
/// para o cliente. Medido pelo DBA: com `max_arquivos 3`, a 71a alteracao
/// falhava. Agora a trilha tem numeracao propria, sem teto: as 100 passam, a
/// linha fica com o ultimo valor e a trilha com o ultimo registro.
///
/// **Com o codigo de antes** (`git archive 82a17ef`) este teste cai na
/// alteracao 64 (o DBA mediu 71 com registro menor: sem IP), com
/// `LimiteExcedido` da trilha.
///
/// Por que 100 e nao 200: neste esquema o `.log` -- que continua seguindo a
/// paginacao da tabela, e nao e deste pedido -- bate no MESMO teto de 3
/// volumes na alteracao 135. E o outro lado do achado A1 (o diario tambem tem
/// teto de volumes), fica registrado e nao e desta frente.
#[test]
fn o_teto_de_volumes_da_trilha_nao_existe_mais() {
    let d = temp("sem-teto");
    let mut t = abrir(&d, esquema_com_teto(3));
    t.inserir(&[Value::Int(1), Value::Str("a0@x.com".into())])
        .unwrap();
    for i in 1..=100 {
        if let Err(e) = t.atualizar(1, &[Value::Int(1), Value::Str(format!("a{i}@x.com"))]) {
            panic!("a alteracao {i} falhou: {e}");
        }
    }
    let linha = t.ler(1).unwrap().unwrap();
    assert_eq!(linha[1], Value::Str("a100@x.com".into()));
    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(ev.len(), 100, "a trilha perdeu registro");
    assert_eq!(ev.last().unwrap().depois, "a100@x.com");
}
