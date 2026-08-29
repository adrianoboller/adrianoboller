//! Quanto custa a trilha de LGPD (`.lgpd`) -- e quanto custa quando nao ha.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store   # binario velho mede o passado
//! cargo run --release --example custo-da-trilha -- [linhas]
//! ```
//!
//! Tres perguntas, nesta ordem:
//!
//! 1. **A tabela SEM coluna marcada paga alguma coisa?** E o portao do
//!    custo-zero, e a resposta tem de ser "nao mensuravel". Mede a mesma
//!    tabela, mesmas linhas, mesma alteracao -- uma com zero colunas marcadas
//!    e outra montada antes de a trilha existir nao da para fazer, entao o
//!    controle e a tabela sem marca.
//! 2. **Quanto custa com as seis colunas marcadas do caso real?** A tela que o
//!    Adriano desenhou marca seis das nove colunas de `clientes`: nome, cpf,
//!    email, telefone, endereco e data_nascimento. Nao e o caso raro, e o
//!    comum -- entao e ele que tem de ser medido.
//! 3. **O acesso por OPERACAO contra o acesso por LINHA.** E a decisao de
//!    desenho, e ela nao se decide por gosto: as duas sao medidas na mesma
//!    varredura, e a razao entre elas e o argumento.
//!
//! O que este medidor NAO faz e citar numero de outro dia. Todo numero que ele
//! imprime saiu da rodada que o imprimiu.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::table::Table;
use phxsql_store::trilha;

/// As nove colunas da tela, com as seis marcadas que o Adriano marcou.
fn esquema(nome: &str, marcar: bool) -> Schema {
    let pessoal = |c: Column| {
        if marcar {
            c.com_dado_pessoal(DadoPessoal::Pessoal)
        } else {
            c
        }
    };
    let colunas = vec![
        Column::new("id_cliente", ColumnType::Sequence).obrigatoria(),
        pessoal(Column::new("nome", ColumnType::Str(60))),
        pessoal(Column::new("cpf", ColumnType::Str(14)).obrigatoria()),
        pessoal(Column::new("email", ColumnType::Str(80))),
        pessoal(Column::new("telefone", ColumnType::Str(20))),
        pessoal(Column::new("endereco", ColumnType::Str(120))),
        pessoal(Column::new("data_nascimento", ColumnType::Date)),
        Column::new(
            "limite_credito",
            ColumnType::Decimal {
                precisao: 12,
                escala: 2,
            },
        ),
        Column::new("data_cadastro", ColumnType::DateTime),
    ];
    let indices = vec![IndexDef::new("por_cpf", vec![IndexColumn::asc(2)])
        .unico()
        .primaria()];
    Schema::new(nome, colunas, indices).expect("esquema")
}

fn linha(i: u64, sufixo: &str) -> Vec<Value> {
    vec![
        Value::Null,
        Value::Str(format!("Cliente {i}")),
        Value::Str(format!("{:011}-{:02}", i, i % 100)),
        Value::Str(format!("cliente{i}{sufixo}@exemplo.com")),
        Value::Str(format!("+55 47 9{:04}-{:04}", i % 10000, i % 10000)),
        Value::Str(format!("Rua {i}, numero {}{sufixo}", i % 900)),
        Value::Date(7000 + (i % 9000) as i32),
        Value::Decimal(150_000 + i as i128),
        Value::DateTime(1_700_000_000_000 + i as i64),
    ]
}

fn temp(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-custo-trilha-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Insere `n` linhas e depois altera TODAS elas, devolvendo (insercao, alteracao).
fn rodada(nome: &str, marcar: bool, n: u64) -> (f64, f64, u64) {
    let dir = temp(nome);
    let mut t = Table::criar(&dir, esquema(nome, marcar)).expect("criar");
    t.definir_usuario(7);
    t.definir_origem("192.0.2.10");

    let inicio = Instant::now();
    for i in 1..=n {
        t.inserir(&linha(i, "")).expect("inserir");
    }
    let insercao = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;

    // A alteracao muda TRES das seis colunas marcadas -- email, telefone e
    // endereco. Alterar as seis seria o pior caso e nao o caso; alterar uma
    // seria o melhor. Tres e o meio, e e o que uma edicao de ficha faz.
    let inicio = Instant::now();
    for i in 1..=n {
        t.atualizar(i, &linha(i, "x")).expect("atualizar");
    }
    let alteracao = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;

    let registros = t.total_da_trilha().expect("total");
    let tem = t.tem_trilha();
    let bytes: u64 = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "lgpd"))
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    println!(
        "  {nome:22} insercao {insercao:7.2} us/linha   alteracao {alteracao:7.2} us/linha   \
         trilha {registros:>7} reg  {bytes:>9} bytes  arquivo={}",
        if tem { "sim" } else { "NAO" }
    );
    let _ = std::fs::remove_dir_all(&dir);
    (insercao, alteracao, registros)
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(5_000);

    println!("PhxSql -- custo da trilha de LGPD, {n} linhas por rodada\n");

    println!("1) O portao do custo-zero: tabela SEM coluna marcada");
    println!("   (a trilha esta LIGADA nas duas; o que muda e so a marca)\n");
    let (ins_sem, alt_sem, reg_sem) = rodada("sem-marca", false, n);
    let (ins_com, alt_com, reg_com) = rodada("com-6-marcadas", true, n);

    println!();
    println!(
        "   insercao:  {ins_sem:.2} -> {ins_com:.2} us/linha  ({:+.1}%)",
        (ins_com / ins_sem - 1.0) * 100.0
    );
    println!(
        "   alteracao: {alt_sem:.2} -> {alt_com:.2} us/linha  ({:+.1}%)",
        (alt_com / alt_sem - 1.0) * 100.0
    );
    println!("   registros de trilha: sem marca {reg_sem}, com marca {reg_com}");
    println!(
        "   -> a insercao NAO grava trilha por desenho, entao a diferenca dela\n\
         \x20     e ruido de medicao; a alteracao paga os {} registros.",
        reg_com
    );

    // ------------------------------------------------------------ 2) desligar
    println!("\n2) A mesma tabela marcada, com a trilha DESLIGADA no config");
    trilha::definir(false, false);
    let (_, alt_desl, reg_desl) = rodada("marcada-desligada", true, n);
    trilha::definir(true, true);
    println!(
        "   alteracao: {alt_com:.2} (ligada) contra {alt_desl:.2} (desligada) us/linha; \
         registros {reg_desl}"
    );

    // -------------------------------------------- 3) acesso: operacao x linha
    println!("\n3) O registro de ACESSO: por operacao contra por linha");
    let dir = temp("acesso");
    let mut t = Table::criar(&dir, esquema("acesso", true)).expect("criar");
    t.definir_usuario(7);
    t.definir_origem("192.0.2.10");
    for i in 1..=n {
        t.inserir(&linha(i, "")).expect("inserir");
    }

    // Por operacao: e o que esta implementado -- UM registro para a varredura.
    let inicio = Instant::now();
    let rowids = t
        .pagina_por_posicao(0, n, phxsql_store::Visao::Ativas)
        .unwrap()
        .0;
    for &r in &rowids {
        let _ = t.ler(r).unwrap();
    }
    t.registrar_acesso(
        0,
        "varrer ordem=digitacao visao=ativas",
        rowids.len() as u64,
    )
    .unwrap();
    let por_operacao = inicio.elapsed().as_secs_f64() * 1e6;
    let reg_operacao = t.total_da_trilha().unwrap();
    let bytes_operacao: u64 = std::fs::metadata(dir.join("acesso.lgpd"))
        .map(|m| m.len())
        .unwrap_or(0);

    // Por linha: UM registro por linha lida. Nao esta implementado -- e o
    // desenho recusado -- entao aqui ele e simulado chamando o mesmo
    // `registrar_acesso` linha a linha, que e exatamente o que ele faria.
    let inicio = Instant::now();
    for &r in &rowids {
        let _ = t.ler(r).unwrap();
        t.registrar_acesso(r, &format!("rowid={r}"), 1).unwrap();
    }
    let por_linha = inicio.elapsed().as_secs_f64() * 1e6;
    let reg_linha = t.total_da_trilha().unwrap() - reg_operacao;
    let bytes_total: u64 = std::fs::metadata(dir.join("acesso.lgpd"))
        .map(|m| m.len())
        .unwrap_or(0);
    let bytes_linha = bytes_total - bytes_operacao;

    println!(
        "  varredura de {} linhas, {} colunas marcadas:",
        rowids.len(),
        6
    );
    println!(
        "    por operacao  {por_operacao:10.0} us   {reg_operacao:>8} registro(s)  \
         {bytes_operacao:>10} bytes"
    );
    println!(
        "    por linha     {por_linha:10.0} us   {reg_linha:>8} registro(s)  \
         {bytes_linha:>10} bytes"
    );
    println!(
        "    -> por linha custa {:.2}x o tempo e {:.0}x os bytes",
        por_linha / por_operacao,
        bytes_linha as f64 / bytes_operacao.max(1) as f64
    );
    println!(
        "    -> e por CELULA (uma por coluna marcada) seria mais 6x isso: {} registros",
        reg_linha * 6
    );
    let _ = std::fs::remove_dir_all(&dir);
}
