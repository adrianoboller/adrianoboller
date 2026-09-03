//! O interruptor da trilha de LGPD -- num arquivo SO PARA ELE.
//!
//! # Por que este teste nao mora junto com os outros
//!
//! Porque ele mexe num global do PROCESSO, e o `cargo test` roda os testes do
//! mesmo binario em paralelo, em threads que dividem esse global. Enquanto
//! este teste estava dentro de `trilha-lgpd.rs`, ele desligava a trilha no
//! meio da corrida e o `acesso_e_um_registro_por_operacao`, rodando ao lado,
//! achava zero registro onde esperava um.
//!
//! Isso nao apareceu na primeira rodada: apareceu na terceira, porque e uma
//! corrida e corrida nao falha sempre. **Teste que falha as vezes e pior que
//! teste que falta** -- o que falta se ve, e o que pisca vira "roda de novo
//! que passa" ate alguem parar de acreditar na bateria inteira.
//!
//! O `diario.rs` ja tinha escrito essa armadilha, com estas palavras, sobre o
//! corte de volume: todo teste que mexe no global vive num arquivo separado,
//! porque cada arquivo de teste e um PROCESSO, e processo nao divide global
//! com processo. Eu li o aviso e caí nele mesmo assim; fica aqui a segunda
//! testemunha.

mod comum;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::table::Table;
use phxsql_store::trilha;

/// A trava que poe os testes DESTE arquivo em fila.
///
/// Separar em arquivo proprio resolve a corrida com os outros testes, que
/// rodam noutro processo -- mas nao resolve a corrida DESTES dois entre si,
/// que dividem o mesmo global no mesmo processo. Sem esta trava eu teria
/// trocado uma corrida por outra menor, que e o jeito mais facil de achar que
/// se consertou alguma coisa.
static EM_FILA: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn fila() -> std::sync::MutexGuard<'static, ()> {
    // Um panico de outro teste envenena a trava; aqui isso nao e motivo para
    // derrubar este, porque o que ela protege e a ORDEM e nao um dado.
    EM_FILA.lock().unwrap_or_else(|e| e.into_inner())
}

fn temp(nome: &str) -> comum::DirTemp {
    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    comum::DirTemp::novo(&format!("interruptor-{nome}"))
}

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Sequence).obrigatoria(),
            Column::new("cpf", ColumnType::Str(14))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("email", ColumnType::Str(80)).com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![IndexDef::new("por_cpf", vec![IndexColumn::asc(1)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn linha(email: &str) -> Vec<Value> {
    vec![
        Value::Null,
        Value::Str("012".into()),
        Value::Str(email.into()),
    ]
}

/// Um teste so, em sequencia, que devolve o global no fim.
#[test]
fn o_interruptor_desliga_os_dois_lados() {
    let _fila = fila();
    // O padrao tem de ser LIGADO: e a exigencia legal, e e o que foi pedido.
    // Este assert e o que trava o padrao -- se alguem inverter a constante,
    // ele cai aqui e nao seis meses depois numa auditoria.
    assert!(trilha::alteracoes_ligadas(), "a trilha nasceu desligada");
    assert!(trilha::acessos_ligados(), "o acesso nasceu desligado");

    let d = temp("desliga");
    let mut t = Table::criar(&d, esquema()).unwrap();
    t.definir_usuario(7);
    t.definir_origem("192.0.2.10");
    t.inserir(&linha("ana@x.com")).unwrap();

    // Ligada: grava.
    t.atualizar(1, &linha("a@x.com")).unwrap();
    assert_eq!(t.total_da_trilha().unwrap(), 1);

    // Desligada nos dois lados: nao grava nada, e nem cria arquivo novo.
    trilha::definir(false, false);
    t.atualizar(1, &linha("b@x.com")).unwrap();
    t.registrar_acesso(1, "rowid=1", 1).unwrap();
    assert_eq!(
        t.total_da_trilha().unwrap(),
        1,
        "desligada, a trilha continuou gravando"
    );

    // So a alteracao: o acesso continua mudo.
    trilha::definir(true, false);
    t.atualizar(1, &linha("c@x.com")).unwrap();
    t.registrar_acesso(1, "rowid=1", 1).unwrap();
    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(ev.len(), 2, "esperava so as duas alteracoes");
    assert!(
        ev.iter().all(|e| e.tipo == trilha::Tipo::Alteracao),
        "o acesso gravou com o interruptor dele desligado"
    );

    // So o acesso: a alteracao e que fica muda.
    trilha::definir(false, true);
    t.atualizar(1, &linha("d@x.com")).unwrap();
    t.registrar_acesso(1, "rowid=1", 1).unwrap();
    let ev = t.trilha(0, 0).unwrap();
    assert_eq!(ev.len(), 3, "esperava as duas alteracoes mais um acesso");
    assert_eq!(ev[2].tipo, trilha::Tipo::Acesso);

    trilha::definir(true, true);
}

/// Desligada, uma tabela marcada nao ganha arquivo nenhum -- o custo-zero
/// tambem vale para quem desligou.
#[test]
fn desligada_nao_cria_o_arquivo() {
    let _fila = fila();
    trilha::definir(false, false);
    let d = temp("sem-arquivo");
    let mut t = Table::criar(&d, esquema()).unwrap();
    t.inserir(&linha("ana@x.com")).unwrap();
    t.atualizar(1, &linha("outro@x.com")).unwrap();
    let existe = t.tem_trilha();
    let no_disco = d.join("clientes.lgpd").exists();
    trilha::definir(true, true);

    assert!(!existe, "desligada, a trilha nasceu assim mesmo");
    assert!(!no_disco, "desligada, o .lgpd apareceu no disco");
}
