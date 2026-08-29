//! Encerrar uma atividade -- provado PELO SOQUETE, e com o arquivo conferido
//! depois.
//!
//! # Por que nao basta teste unitario
//!
//! O de unidade prova a MECANICA da marca: que `siga` devolve erro quando o
//! serial bate. Ele nao prova o que interessa, que sao tres coisas de fora:
//!
//! 1. que o laco de verdade -- o do `checksum`, dentro do servidor -- chama o
//!    ponto de cancelamento; tire a chamada de la e o unitario continua
//!    passando enquanto a operacao roda ate o fim;
//! 2. que a marca atravessa DUAS CONEXOES: quem manda encerrar e uma thread,
//!    quem esta trabalhando e outra;
//! 3. que a tabela continua inteira depois. Cancelamento que estraga arquivo
//!    nao e cancelamento, e a unica forma de saber e reabrir e conferir.
//!
//! E a licao do `BULKINSERT`: o que depende do sistema operacional -- aqui,
//! duas threads e um soquete -- se prova contra o sistema operacional.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-da-telemetria";
/// Linhas da tabela da prova.
///
/// Tem de dar uma soma de verificacao LONGA o bastante para caber um
/// encerramento no meio dela, e curta o bastante para o teste nao demorar. A
/// 200.000 a soma leva da ordem de meio segundo aqui, e o encerramento entra
/// na primeira dezena de milhares de linhas.
const LINHAS: usize = 200_000;

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn pasta(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phxsql-telemetria-{}-{}-{nome}",
        std::process::id(),
        porta_livre()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn subir_servidor(base: &std::path::Path, porta: u16) -> Arc<Servidor> {
    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        max_linhas: 500_000,
        ..Default::default()
    };
    c.web.ligado = false;
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return s;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu em {porta}");
}

/// Uma conexao aberta, para os pedidos de um mesmo cliente.
struct Conexao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Conexao {
    fn abrir(porta: u16) -> Conexao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(5)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(120)))
            .unwrap();
        Conexao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn pedir(&mut self, corpo: &str) -> String {
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        self.escrita.flush().unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        r
    }
}

/// Cria a tabela e enche.
fn encher(porta: u16) {
    let mut c = Conexao::abrir(porta);
    let r = c.pedir("\"op\":\"criar_database\",\"database\":\"loja\"");
    assert!(r.contains("\"ok\":true"), "{r}");
    let r = c.pedir(
        "\"op\":\"criar_tabela\",\"database\":\"loja\",\"tabela\":\"clientes\",\
         \"colunas\":[{\"nome\":\"nome\",\"tipo\":\"Str\",\"tamanho\":40},\
         {\"nome\":\"valor\",\"tipo\":\"Int8\"}]",
    );
    assert!(r.contains("\"ok\":true"), "{r}");
    let mut feitas = 0;
    while feitas < LINHAS {
        let linhas: Vec<String> = (0..10_000)
            .map(|i| {
                format!(
                    "{{\"nome\":\"Cliente {}\",\"valor\":{}}}",
                    feitas + i,
                    i % 97
                )
            })
            .collect();
        let r = c.pedir(&format!(
            "\"op\":\"inserir_lote\",\"database\":\"loja\",\"tabela\":\"clientes\",\"linhas\":[{}]",
            linhas.join(",")
        ));
        assert!(r.contains("\"ok\":true"), "{}", &r[..r.len().min(300)]);
        feitas += 10_000;
    }
}

/// O valor de um campo de texto da resposta, sem analisador de JSON.
fn campo<'a>(resposta: &'a str, nome: &str) -> &'a str {
    resposta
        .split(&format!("\"{nome}\":\""))
        .nth(1)
        .and_then(|p| p.split('"').next())
        .unwrap_or("")
}

/// Acha o `id` da atividade que esta executando a operacao pedida.
fn achar_atividade(porta: u16, op: &str) -> Option<String> {
    let mut c = Conexao::abrir(porta);
    let r = c.pedir("\"op\":\"telemetria\",\"amostras\":1");
    // Sem analisador de JSON aqui de proposito: o teste nao deve depender da
    // ordem dos campos, e sim do par que interessa. Cada atividade vira um
    // objeto entre chaves; basta achar o que traz a operacao procurada.
    for pedaco in r.split("{\"id\":\"").skip(1) {
        let (id, resto) = pedaco.split_once('"')?;
        if resto.contains(&format!("\"op\":\"{op}\"")) {
            return Some(id.to_string());
        }
    }
    None
}

/// **A prova.** Uma soma de verificacao longa e encerrada de outra conexao, o
/// cliente recebe o erro certo, e a tabela continua inteira.
///
/// # O defeito reposto
///
/// Tire o `a.siga(1)?` do laco do `op_checksum` e este teste falha na
/// primeira asserção: a soma responde `"ok":true` com as 200.000 linhas
/// contadas, porque ninguem olhou a marca. E exatamente o defeito que o
/// unitario nao pega.
#[test]
fn a_soma_longa_e_encerrada_e_a_tabela_continua_integra() {
    let base = pasta("encerrar");
    let porta = porta_livre();
    let _s = subir_servidor(&base, porta);
    encher(porta);

    // A soma de referencia, com a tabela em paz. E o numero que a conferencia
    // do fim tem de reencontrar.
    let mut conferente = Conexao::abrir(porta);
    let antes =
        conferente.pedir("\"op\":\"checksum\",\"database\":\"loja\",\"tabela\":\"clientes\"");
    assert!(antes.contains("\"ok\":true"), "{antes}");
    let soma_antes = antes
        .split("\"checksum\":\"")
        .nth(1)
        .and_then(|p| p.split('"').next())
        .unwrap()
        .to_string();

    // A vitima: uma soma de verificacao numa conexao propria.
    let (envia, recebe) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut c = Conexao::abrir(porta);
        let r = c.pedir("\"op\":\"checksum\",\"database\":\"loja\",\"tabela\":\"clientes\"");
        let _ = envia.send(r);
    });

    // Espera ela aparecer na telemetria -- e a prova de que a bolha existe
    // enquanto a operacao esta em curso, e nao so depois.
    let mut id = None;
    let ate = Instant::now() + Duration::from_secs(10);
    while Instant::now() < ate && id.is_none() {
        id = achar_atividade(porta, "checksum");
        if id.is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let id = id.expect("a soma em curso nao apareceu na telemetria");

    let mut carrasco = Conexao::abrir(porta);
    let r = carrasco.pedir(&format!("\"op\":\"telemetria_encerrar\",\"id\":\"{id}\""));
    assert!(r.contains("\"ok\":true"), "{r}");
    assert!(
        r.contains("\"estado\":\"encerrando\""),
        "a soma devia estar em fase cancelavel: {r}"
    );

    // O cliente recebe o erro -- e ele diz o que aconteceu, com codigo e
    // nome, para quem integra nao ter de comparar redacao.
    let resposta = recebe
        .recv_timeout(Duration::from_secs(60))
        .expect("a soma nunca respondeu");
    assert!(
        resposta.contains("\"ok\":false"),
        "a soma terminou como se nada tivesse acontecido -- o laco nao \
         consultou a marca: {resposta}"
    );
    assert!(resposta.contains("\"nome\":\"CANCELADO\""), "{resposta}");
    assert!(resposta.contains("\"codigo\":6001"), "{resposta}");
    assert!(
        resposta.contains("integro"),
        "a mensagem tem de dizer que o arquivo esta inteiro: {resposta}"
    );

    // **A parte que separa cancelamento de estrago.** A tabela e reaberta e
    // conferida: mesma contagem, mesma soma. Um cancelamento que deixasse o
    // arquivo pela metade apareceria aqui, e em nenhum outro lugar.
    let depois =
        conferente.pedir("\"op\":\"checksum\",\"database\":\"loja\",\"tabela\":\"clientes\"");
    assert!(depois.contains("\"ok\":true"), "{depois}");
    assert!(
        depois.contains(&format!("\"checksum\":\"{soma_antes}\"")),
        "a soma mudou depois do encerramento: antes {soma_antes}, depois {depois}"
    );
    assert!(
        depois.contains(&format!("\"linhas\":{LINHAS}")),
        "a contagem mudou depois do encerramento: {depois}"
    );

    let v = conferente.pedir("\"op\":\"verificar\",\"database\":\"loja\",\"tabela\":\"clientes\"");
    assert!(v.contains("\"ok\":true"), "a verificacao recusou: {v}");
}

/// Encerrar uma atividade PARADA nao promete nada -- e diz isso em vez de
/// responder «encerrada» e nao encerrar coisa nenhuma.
#[test]
fn encerrar_atividade_ociosa_diz_que_nao_havia_nada() {
    let base = pasta("ociosa");
    let porta = porta_livre();
    let _s = subir_servidor(&base, porta);

    // Uma conexao que fez UM pedido e ficou quieta.
    let mut quieta = Conexao::abrir(porta);
    assert!(quieta.pedir("\"op\":\"ping\"").contains("\"ok\":true"));

    let mut c = Conexao::abrir(porta);
    let t = c.pedir("\"op\":\"telemetria\",\"amostras\":1");
    // O id da PROPRIA pergunta vem no campo `voce`. Adivinhar «dados:1» era
    // o que este teste fazia, e ele quebrou: a espera do arranque conecta uma
    // vez para saber se a porta subiu, e essa sondagem ja gastou o numero 1.
    let eu = campo(&t, "voce");
    let alvo = (t.split("{\"id\":\""))
        .skip(1)
        .filter_map(|p| p.split('"').next())
        .find(|id| *id != eu)
        .expect("so havia a propria atividade")
        .to_string();
    let r = c.pedir(&format!("\"op\":\"telemetria_encerrar\",\"id\":\"{alvo}\""));
    assert!(r.contains("\"ok\":true"), "{r}");
    assert!(r.contains("\"estado\":\"ociosa\""), "{r}");
    assert!(
        r.contains("encerrar_sessao"),
        "a resposta tem de apontar o caminho que DERRUBA a conexao: {r}"
    );
}

/// Encerrar a propria atividade e recusado: o pedido morreria antes de
/// responder o que aconteceu.
#[test]
fn ninguem_encerra_a_si_mesmo() {
    let base = pasta("eu-mesmo");
    let porta = porta_livre();
    let _s = subir_servidor(&base, porta);

    let mut c = Conexao::abrir(porta);
    let t = c.pedir("\"op\":\"telemetria\",\"amostras\":1");
    let eu = campo(&t, "voce");
    assert!(eu.starts_with("dados:"), "o campo `voce` nao veio: {t}");
    let r = c.pedir(&format!("\"op\":\"telemetria_encerrar\",\"id\":\"{eu}\""));
    assert!(r.contains("\"ok\":false"), "{r}");
    assert!(r.contains("propria atividade"), "{r}");
}

/// A telemetria DESLIGADA nao registra atividade nenhuma -- e ligar de volta
/// nao exige reconectar, porque a atividade e aberta a cada pedido.
#[test]
fn desligada_nao_ve_nada_e_ligar_de_volta_recupera_quem_ja_estava_conectado() {
    let base = pasta("interruptor");
    let porta = porta_livre();
    let _s = subir_servidor(&base, porta);

    let mut velha = Conexao::abrir(porta);
    assert!(velha.pedir("\"op\":\"ping\"").contains("\"ok\":true"));

    let mut c = Conexao::abrir(porta);
    assert!(c
        .pedir("\"op\":\"telemetria_desligar\"")
        .contains("\"ligada\":false"));
    // A conexao velha continua trabalhando, e nao aparece.
    assert!(velha.pedir("\"op\":\"ping\"").contains("\"ok\":true"));
    let r = c.pedir("\"op\":\"telemetria\",\"amostras\":1");
    assert!(r.contains("\"atividades\":[]"), "{r}");

    assert!(c
        .pedir("\"op\":\"telemetria_ligar\"")
        .contains("\"ligada\":true"));
    assert!(velha.pedir("\"op\":\"ping\"").contains("\"ok\":true"));
    let r = c.pedir("\"op\":\"telemetria\",\"amostras\":1");
    assert!(
        !r.contains("\"atividades\":[]"),
        "ligar de volta tinha de recuperar quem JA estava conectado -- senao \
         so as conexoes novas apareceriam: {r}"
    );
}

/// Toda thread do servidor esta no registro, com a finalidade escrita.
///
/// O teste nao lista nomes: ele exige que NENHUMA fique sem finalidade. Uma
/// lista de nomes envelheceria calada, e o que importa aqui e a regra, nao a
/// lista de hoje.
#[test]
fn nenhuma_thread_fica_sem_finalidade_escrita() {
    let base = pasta("threads");
    let porta = porta_livre();
    let _s = subir_servidor(&base, porta);

    let mut c = Conexao::abrir(porta);
    let r = c.pedir("\"op\":\"telemetria\",\"amostras\":1");
    let bloco = r
        .split("\"threads\":[")
        .nth(1)
        .expect("a telemetria nao devolveu as threads");
    let quantas = bloco.matches("\"finalidade\":\"").count();
    let nomes = bloco.matches("\"nome\":\"").count();
    assert!(nomes >= 3, "so {nomes} thread(s) registradas: {bloco}");
    assert_eq!(
        quantas, nomes,
        "ha thread sem finalidade escrita no registro"
    );
    assert!(
        !bloco.contains("\"finalidade\":\"\""),
        "ha finalidade vazia: {bloco}"
    );
    // As duas que existem em qualquer servidor, com ou sem cluster e replica.
    assert!(bloco.contains("aceitador-dados"), "{bloco}");
    assert!(bloco.contains("amostrador"), "{bloco}");
}
