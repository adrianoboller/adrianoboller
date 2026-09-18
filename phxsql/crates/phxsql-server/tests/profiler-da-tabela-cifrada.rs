//! A prova real do pedido 356: **a marca de coluna TAMBEM cega o arquivo**.
//!
//! # O defeito que este arquivo trava
//!
//! O Profiler decidia esconder o texto do pedido pela lista `cifra.tabelas`
//! (`Profiler::tabela_e_sigilosa`), e a cifra do dado acontece pela marca de
//! coluna (`Schema::tem_dado_pessoal`, lido pelo `reg.rs` na criacao). Dois
//! campos para uma garantia so: a tabela cujo `.reg` nasceu CIFRADO e que
//! ninguem listou -- o padrao de todo `config.json`, porque a lista nasce
//! vazia -- gravava o valor marcado **em claro no `perfil.txt`**, ao lado do
//! `.reg` cifrado. E o arquivo e justamente o que viaja com o disco e entra no
//! backup, onde nao ha login para conferir nem portao para atravessar.
//!
//! # Por que um teste de INTEGRACAO, e nao um de unidade
//!
//! Por duas razoes que um teste de unidade nao alcanca:
//!
//! 1. `cofre::definir` mexe num global do PROCESSO. Ligar a cifra dentro do
//!    binario da biblioteca faria os outros testes do mesmo binario nascerem
//!    com o cofre ligado no meio da corrida.
//! 2. O defeito **e o encontro** de duas metades -- a config que alimenta o
//!    Profiler e o disco que cifra a tabela. Provar cada metade em separado
//!    era exatamente o que deixava o furo passar: os dez testes do pedido 195
//!    passavam, e a tabela cifrada ia em claro para o arquivo.
//!
//! Entao aqui sobe um servidor de verdade, com a cifra ligada pelo
//! `config.json`, e o pedido entra pelo SOQUETE -- que e onde o ponto de
//! captura mora. O alvo da conferencia e o ARQUIVO EM DISCO, nunca a resposta.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_server::config::Config;
use phxsql_server::servidor::Servidor;
use phxsql_store::cofre;

/// A trava que serializa os testes deste arquivo: o cofre e global ao processo.
static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

/// O valor da coluna MARCADA. Se ele aparecer no `perfil.txt`, o furo voltou.
const CPF: &str = "111.222.333-44";

/// O valor de uma tabela SEM marca nenhuma, no mesmo servidor e na mesma
/// sessao de observacao. Ele TEM de aparecer: e o controle que prova que o
/// instrumento nao ficou cego por acidente.
const CIDADE: &str = "Blumenau";

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn esperar_porta(porta: u16) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

/// Uma linha de JSON para o servidor, uma linha de volta.
fn pedir(porta: u16, corpo: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{{\"token\":\"t\",{}}}", corpo.replace('\n', " ")).unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    assert!(resposta.contains("\"ok\":true"), "{corpo} -> {resposta}");
    resposta
}

/// Sobe um servidor com a cifra LIGADA e `cifra.tabelas` VAZIO -- o padrao de
/// todo `config.json` de hoje, e a condicao exata do furo.
fn servidor_com_cofre(d: &DirTemp) -> u16 {
    let porta = porta_livre();
    let caminho = d.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "token": "t",
              "bind": "127.0.0.1:{porta}",
              "base": "{}",
              "cifra": {{ "ligada": true, "senha": "a chave do cofre", "iteracoes": 10000 }}
            }}"#,
            d.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    assert!(cofre::ligado(), "o config.json nao ligou o cofre");
    assert!(
        c.cifra.tabelas.is_empty(),
        "a lista tem de estar VAZIA: com ela cheia o teste passaria pelo \
         caminho velho e nao provaria nada"
    );
    c.web.ligado = false;
    c.log_acessos = d.join("acessos.log");
    c.blacklist = d.join("blacklist.json");
    c.dblink = d.join("dblink.json");
    c.jobs = d.join("jobs.json");
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar_porta(porta);
    porta
}

/// **A prova real, nos dois sentidos, na mesma corrida e no mesmo arquivo.**
///
/// A tabela com coluna marcada nasce cifrada (conferido pelo `esquema`, e nao
/// suposto: e a premissa do teste). O `inserir` dela entra pelo soquete com o
/// Profiler ligado e gravando em arquivo, e o CPF **nao** pode estar no
/// `perfil.txt`. No mesmo servidor, na mesma sessao de observacao e no mesmo
/// arquivo, a tabela SEM marca continua com o texto: e o controle que prova
/// que o conserto nao cegou quem nao pediu nada.
#[test]
fn tabela_cifrada_pela_marca_de_coluna_tambem_cega_o_arquivo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = DirTemp::novo("prof-cifrada");
    let porta = servidor_com_cofre(&d);
    let arquivo = d.join("perfil.txt");

    pedir(porta, r#""op":"criar_database","database":"loja""#);
    pedir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"cpf","tipo":"Str(14)","dado_pessoal":"sensivel"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    pedir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"cidades",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"nome","tipo":"Str(40)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );

    // A PREMISSA, medida e nao suposta: o `.reg` da marcada esta cifrado e o
    // da outra nao. Sem isto o teste poderia passar num servidor onde nada
    // cifrou, provando o contrario do que promete.
    let e1 = pedir(
        porta,
        r#""op":"esquema","database":"loja","tabela":"clientes""#,
    );
    assert!(
        e1.contains("\"material\":\"cifrado\""),
        "a premissa caiu: a tabela marcada nao nasceu cifrada -- {e1}"
    );
    let e2 = pedir(
        porta,
        r#""op":"esquema","database":"loja","tabela":"cidades""#,
    );
    assert!(
        e2.contains("\"material\":\"em_claro\""),
        "a tabela sem marca nasceu cifrada, e o controle deixa de valer -- {e2}"
    );

    let ligou = pedir(
        porta,
        &format!(r#""op":"profiler_ligar","arquivo":"{}""#, arquivo.display()),
    );
    assert!(
        ligou.contains("\"tabelas_sem_texto\":[]"),
        "a lista declarada tem de estar vazia na resposta -- {ligou}"
    );

    pedir(
        porta,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{{"id":1,"cpf":"{CPF}"}}"#
        ),
    );
    pedir(
        porta,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"cidades",
               "linha":{{"id":1,"nome":"{CIDADE}"}}"#
        ),
    );

    // O ANEL e o controle do outro lado: a tela e do administrador, que tem o
    // `config.json` e portanto a senha do cofre -- esconder dele seria teatro.
    let anel = pedir(porta, r#""op":"profiler","max":50"#);
    assert!(
        anel.contains(CPF),
        "o anel ficou cego: o administrador perdeu a tela por uma regra que \
         so devia valer para o arquivo -- {anel}"
    );

    pedir(porta, r#""op":"profiler_desligar""#);
    let texto = std::fs::read_to_string(&arquivo).unwrap();
    assert!(
        !texto.contains(CPF),
        "o valor da coluna MARCADA foi para o perfil.txt em claro, ao lado do \
         .reg cifrado -- e ninguem declarou a tabela em cifra.tabelas, que e \
         o padrao:\n{texto}"
    );
    assert!(
        texto.contains(CIDADE),
        "o conserto cegou a tabela que ninguem marcou e ninguem declarou: \
         guarda nova entra PEDIDA, nao imposta:\n{texto}"
    );
    // O tamanho fica, como no caminho da lista: e o que permite achar o
    // pedido gigante sem ler o conteudo.
    assert!(
        texto.contains("cifrado"),
        "o arquivo omitiu sem dizer que omitiu -- quem ler daqui a seis meses \
         vai procurar defeito onde ha decisao:\n{texto}"
    );

    cofre::desligar();
}

/// **A coluna ao lado, achada procurando quem NAO tinha o campo novo.**
///
/// O texto do erro cita o valor que o cliente mandou. Medido em 18/09/2026,
/// com o pedido ja tapado pelo conserto acima:
///
/// ```text
/// inserir  loja.clientes  ERRO 0ms  115B  <tabela com .reg cifrado: pedido nao
/// gravado>  <- [SP000018] limite excedido: 123456 nao cabe em inteiro de 16 bits
/// ```
///
/// O `123456` era o valor de uma coluna MARCADA. Tapar o pedido e deixar o
/// erro passar e cegar uma coluna e vazar pela vizinha, na mesma linha do
/// mesmo arquivo -- entao o erro de evento sigiloso vira o TAMANHO, como o
/// pedido. Nos dois sentidos: a tabela sem marca continua com o texto do erro
/// inteiro, que e o que se usa para diagnosticar.
#[test]
fn o_erro_que_cita_o_valor_tambem_fica_fora_do_arquivo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = DirTemp::novo("prof-erro");
    let porta = servidor_com_cofre(&d);
    let arquivo = d.join("perfil.txt");

    pedir(porta, r#""op":"criar_database","database":"loja""#);
    // `Int2` de proposito: o valor fora da faixa faz o erro CITAR o numero.
    pedir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"salario","tipo":"Int2","dado_pessoal":"sensivel"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    pedir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"cidades",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"habitantes","tipo":"Int2"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    pedir(
        porta,
        &format!(r#""op":"profiler_ligar","arquivo":"{}""#, arquivo.display()),
    );

    // Os dois pedidos ERRAM pelo mesmo motivo, e so um deles e de tabela
    // cifrada: e o par que separa «protegeu» de «cegou tudo».
    recusado(
        porta,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"salario":123456}"#,
    );
    recusado(
        porta,
        r#""op":"inserir","database":"loja","tabela":"cidades","linha":{"id":1,"habitantes":654321}"#,
    );

    let anel = pedir(porta, r#""op":"profiler","max":50"#);
    assert!(
        anel.contains("123456"),
        "o anel perdeu o texto do erro: a tela e do administrador -- {anel}"
    );

    pedir(porta, r#""op":"profiler_desligar""#);
    let texto = std::fs::read_to_string(&arquivo).unwrap();
    assert!(
        !texto.contains("123456"),
        "o valor da coluna marcada vazou pela coluna de ERRO, com o pedido \
         tapado ao lado:\n{texto}"
    );
    assert!(
        texto.contains("654321"),
        "o erro da tabela que ninguem marcou tambem sumiu -- e o texto do \
         erro e justamente o que se usa para diagnosticar:\n{texto}"
    );
    assert!(
        texto.contains("erro nao gravado"),
        "sumiu com o erro sem dizer que sumiu:\n{texto}"
    );

    cofre::desligar();
}

/// Um pedido que o servidor tem de RECUSAR. Devolve a resposta.
///
/// Separado do `pedir` porque aquele exige `"ok":true` -- e aqui o erro e o
/// ponto do teste.
fn recusado(porta: u16, corpo: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{{\"token\":\"t\",{}}}", corpo.replace('\n', " ")).unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    assert!(
        resposta.contains("\"ok\":false"),
        "o servidor ACEITOU o que o teste precisa que ele recuse: {corpo} -> {resposta}"
    );
    resposta
}
