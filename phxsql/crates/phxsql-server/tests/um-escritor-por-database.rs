//! Pedido 677 (325 F1) -- um escritor por database, provado PELO SOQUETE.
//!
//! No espelho do 325 cada caixa e um `phxsqld` dono do `caixaNN`, e recebe do
//! central o database do cadastro. O caixa nao roda em `somente_leitura` --
//! ele precisa vender --, entao ate aqui nada impedia o operador de
//! cadastrar um cliente no database que vem do central: a escrita local
//! tomava o lugar do evento seguinte do source e a replicacao parava (pedido
//! 300 (4)), ou dois caixas cadastravam o mesmo cliente. A guarda le o campo
//! que ja dizia de onde o database vem -- `replicacao.origens[].databases` --
//! e recusa no portao unico, para o pedido direto e para o derivado do SQL --
//! so na origem que pediu, com `"espelho": true`.

mod comum;
use comum::DirTemp;

use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "escritor";

/// A frase do contrato (MANUAL 16.1) que a recusa tem de trazer.
const CONTRATO: &str = "Cliente novo e produto novo so se cadastram com o central no ar";

fn config_base(base: &std::path::Path) -> Config {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // Escape escrito: o que se mede e o portao da escrita, nao a cifra.
    c.cifra_fio.exigir = false;
    c.web.ligado = false;
    c
}

fn subir(c: Config) -> (Arc<Servidor>, u16) {
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    (s, porta)
}

fn origem(nome: &str, porta: u16, database: &str) -> Origem {
    Origem {
        nome: nome.into(),
        host: "127.0.0.1".into(),
        porta,
        token: TOKEN.into(),
        databases: vec![database.into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
        pino_tls: String::new(),
        // A guarda e PEDIDA: o espelho do 325 escreve o campo.
        espelho: true,
    }
}

/// Um no do espelho: papel `replica`, SEM `somente_leitura`, puxando
/// `database` da origem `nome`.
fn subir_no(base: &std::path::Path, id: &str, o: Origem) -> (Arc<Servidor>, u16) {
    let mut c = config_base(base);
    c.replicacao.papel = Papel::Replica;
    c.replicacao.id_servidor = id.into();
    c.replicacao.imagem_da_linha = true;
    c.replicacao.origens = vec![o];
    subir(c)
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let linha = format!("{{\"token\":\"{TOKEN}\",{}}}", corpo.replace('\n', " "));
    let resposta = comum::pedir(porta, &linha);
    Json::analisar(&resposta)
        .unwrap_or_else(|e| panic!("{corpo}: resposta ilegivel ({e}): {resposta}"))
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo);
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

/// Recusa que diz DE ONDE o database vem e traz o contrato.
fn exigir_recusa_do_espelho(porta: u16, corpo: &str, base: &str, origem: &str) {
    let r = pedir(porta, corpo);
    assert!(
        !r.booleano_ou("ok", true),
        "a escrita local em {base}, que vem por replicacao, passou: {corpo} -> {}",
        r.escrever()
    );
    let erro = r.texto_ou("erro", "");
    assert!(
        erro.contains(&format!("database {base} vem da origem {origem}"))
            && erro.contains(CONTRATO),
        "a recusa tem de dizer de onde {base} vem e trazer o contrato: {erro}"
    );
}

fn criar_tabela(porta: u16, database: &str, tabela: &str) {
    exigir(
        porta,
        &format!(r#""op":"criar_database","database":"{database}""#),
    );
    exigir(
        porta,
        &format!(
            r#""op":"criar_tabela","database":"{database}","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}}],
               "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}}]"#
        ),
    );
}

fn ids(porta: u16, database: &str, tabela: &str) -> Vec<i64> {
    let r = pedir(
        porta,
        &format!(r#""op":"varrer","database":"{database}","tabela":"{tabela}","max":100"#),
    );
    if !r.booleano_ou("ok", false) {
        return Vec::new();
    }
    let mut v: Vec<i64> = r
        .campo("resultado")
        .and_then(|x| x.campo("linhas"))
        .and_then(Json::lista)
        .map(|l| l.iter().filter_map(|x| x.campo("id")?.inteiro()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn esperar_ids(porta: u16, database: &str, tabela: &str, quer: &[i64]) {
    let ate = Instant::now() + Duration::from_secs(20);
    while Instant::now() < ate {
        if ids(porta, database, tabela) == quer {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "{database}/{tabela} nao chegou a {quer:?} em 20 s (esta em {:?})",
        ids(porta, database, tabela)
    );
}

/// **O caixa com o central NO AR.** O cadastro desce, a venda grava no
/// `caixa01`, e o cadastro feito no caixa e recusado -- pelo pedido direto e
/// pelo SQL, que chega pelo irmao do `despachar` (`executar_derivado`). E a
/// guarda nao barra a propria replicacao: o cliente cadastrado no central
/// depois da recusa continua chegando.
///
/// **Defeito reposto** (o braco de `origem_que_traz` tirado do portao 2b):
/// o `inserir` em `loja/clientes` passa e o teste cai na primeira recusa.
#[test]
fn o_caixa_vende_no_dele_e_nao_cadastra_no_do_central() {
    let base_c = DirTemp::novo("escritor-central");
    let base_x = DirTemp::novo("escritor-caixa");
    let mut c = config_base(&base_c);
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "central".into();
    c.replicacao.imagem_da_linha = true;
    let (_central, porta_c) = subir(c);
    criar_tabela(porta_c, "loja", "clientes");
    for id in 1..=3 {
        exigir(
            porta_c,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id}}}"#
            ),
        );
    }

    let (_caixa, porta_x) = subir_no(&base_x, "caixa01", origem("central", porta_c, "loja"));
    esperar_ids(porta_x, "loja", "clientes", &[1, 2, 3]);

    // A venda: database do proprio caixa.
    criar_tabela(porta_x, "caixa01", "vendas");
    exigir(
        porta_x,
        r#""op":"inserir","database":"caixa01","tabela":"vendas","linha":{"id":1}"#,
    );

    // O cadastro no caixa: direto e pelo SQL.
    exigir_recusa_do_espelho(
        porta_x,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":99}"#,
        "loja",
        "central",
    );
    exigir_recusa_do_espelho(
        porta_x,
        r#""op":"sql","database":"loja","texto":"INSERT INTO clientes (id) VALUES (98)""#,
        "loja",
        "central",
    );

    // A replicacao continua: o cadastro feito NO central chega, e nada local
    // entrou por cima.
    exigir(
        porta_c,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":4}"#,
    );
    esperar_ids(porta_x, "loja", "clientes", &[1, 2, 3, 4]);
    assert_eq!(ids(porta_x, "caixa01", "vendas"), vec![1]);
}

/// **O caixa com o central CAIDO** (origem numa porta fechada): a venda
/// passa, o cadastro recusa com o contrato -- inclusive criar o database do
/// central, que ainda nem chegou.
#[test]
fn com_o_central_caido_o_caixa_vende_e_nao_cadastra() {
    let base_x = DirTemp::novo("escritor-caixa-sozinho");
    let (_caixa, porta_x) = subir_no(
        &base_x,
        "caixa02",
        origem("central", comum::porta_fechada(), "loja"),
    );
    criar_tabela(porta_x, "caixa02", "vendas");
    exigir(
        porta_x,
        r#""op":"inserir","database":"caixa02","tabela":"vendas","linha":{"id":1}"#,
    );
    exigir_recusa_do_espelho(
        porta_x,
        r#""op":"criar_database","database":"loja""#,
        "loja",
        "central",
    );
}

/// **O outro lado do espelho:** no central, o `caixaNN` que ele recebe e so
/// leitura, e o database dele mesmo continua aberto.
#[test]
fn no_central_o_caixa_e_so_leitura() {
    let base_c = DirTemp::novo("escritor-central-replica");
    let (_central, porta_c) = subir_no(
        &base_c,
        "central",
        origem("caixa01", comum::porta_fechada(), "caixa01"),
    );
    criar_tabela(porta_c, "loja", "clientes");
    exigir(
        porta_c,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1}"#,
    );
    exigir_recusa_do_espelho(
        porta_c,
        r#""op":"criar_database","database":"caixa01""#,
        "caixa01",
        "caixa01",
    );
}

/// **O comportamento VELHO:** origem sem `"espelho"` (todo config anterior ao
/// 677) continua gravando no database recebido, pelo pedido direto e pelo
/// SQL. A guarda entra pedida, nao imposta: ligada por omissao ela recusou de
/// um dia para o outro o arranjo de `trava-atras-da-rede.rs`.
///
/// **Defeito reposto** (o `o.espelho &&` tirado de `origem_que_traz`): o
/// primeiro `inserir` em `loja/clientes` e recusado e o teste cai.
#[test]
fn sem_espelho_a_escrita_local_continua_como_antes() {
    let base_x = DirTemp::novo("escritor-sem-espelho");
    let mut o = origem("central", comum::porta_fechada(), "loja");
    o.espelho = false;
    let (_caixa, porta_x) = subir_no(&base_x, "caixa03", o);
    criar_tabela(porta_x, "loja", "clientes");
    exigir(
        porta_x,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1}"#,
    );
    exigir(
        porta_x,
        r#""op":"sql","database":"loja","texto":"INSERT INTO clientes (id) VALUES (2)""#,
    );
    assert_eq!(ids(porta_x, "loja", "clientes"), vec![1, 2]);
}
