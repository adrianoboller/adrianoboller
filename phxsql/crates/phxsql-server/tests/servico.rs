//! Parar e subir a porta de dados pela tela -- provado PELO SOQUETE.
//!
//! Teste unitario nao prova isto. O que se quer saber e se o `accept`
//! bloqueado acorda, se a porta e mesmo SOLTA quando o laco sai, e se conectar
//! nela depois disso e recusado pelo sistema operacional. Nada disso e
//! observavel de dentro do processo sem abrir um soquete de verdade -- e a
//! licao do `BULKINSERT` foi que um teste que passa por engano e pior que um
//! teste que falta.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-do-servico";

/// Uma porta livre, tomada e solta na hora -- o jeito de nao brigar com outro
/// teste rodando em paralelo.
fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
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
        ..Default::default()
    };
    // A interface web fica de fora: este teste e sobre a porta de dados, e
    // subir a web tomaria uma segunda porta sem necessidade.
    c.web.ligado = false;
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar_porta(porta, true).expect("o servidor nao subiu");
    s
}

/// Espera a porta ficar como se quer, ate dois segundos.
///
/// A troca de porta acontece em outra linha de execucao: o pedido volta
/// dizendo "subindo em", e o `bind` novo ja esta feito -- mas o laco velho
/// ainda pode estar entre o `accept` e o `drop`. Dormir um tempo fixo aqui
/// seria um teste que passa nesta maquina e falha na proxima.
fn esperar_porta(porta: u16, aberta: bool) -> Result<(), String> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(2);
    while Instant::now() < ate {
        let agora = TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok();
        if agora == aberta {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!(
        "a porta {porta} nao ficou {} em 2 s",
        if aberta { "aberta" } else { "fechada" }
    ))
}

/// Manda um pedido e devolve a resposta, pela porta de dados.
fn pedir(porta: u16, linha: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("nao conectei em {porta}: {e}"));
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{linha}").unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    resposta
}

fn pasta(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phxsql-servico-{}-{}-{nome}",
        std::process::id(),
        porta_livre()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn parar_solta_a_porta_e_subir_a_devolve() {
    let base = pasta("parar-subir");
    let porta = porta_livre();
    let _s = subir_servidor(&base, porta);

    // No ar.
    let r = pedir(
        porta,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico\"}}"),
    );
    assert!(r.contains("\"no_ar\":true"), "{r}");
    assert!(r.contains(&format!(":{porta}")), "{r}");

    // Parar. O `accept` esta bloqueado neste instante -- se ele nao acordar,
    // a porta continua aberta e o `esperar_porta` estoura.
    let r = pedir(
        porta,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico_parar\"}}"),
    );
    assert!(r.contains("\"ok\":true"), "{r}");
    esperar_porta(porta, false).unwrap();

    // E a porta esta MESMO solta: outro processo consegue prende-la. Sem o
    // `drop` do ouvinte, o laco teria parado de atender e continuado dono do
    // endereco -- que e o pior dos dois mundos.
    let ocupante =
        TcpListener::bind(("127.0.0.1", porta)).expect("a porta nao foi solta de verdade");
    drop(ocupante);
}

#[test]
fn trocar_de_porta_pela_tela() {
    let base = pasta("trocar");
    let velha = porta_livre();
    let s = subir_servidor(&base, velha);
    let nova = porta_livre();

    let r = pedir(
        velha,
        &format!(
            "{{\"token\":\"{TOKEN}\",\"op\":\"servico_subir\",\"bind\":\"127.0.0.1:{nova}\"}}"
        ),
    );
    assert!(r.contains("\"ok\":true"), "{r}");
    assert!(r.contains("\"trocou_de_porta\":true"), "{r}");

    esperar_porta(nova, true).unwrap();
    esperar_porta(velha, false).unwrap();

    let r = pedir(
        nova,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico\"}}"),
    );
    assert!(r.contains(&format!(":{nova}")), "{r}");
    // O arquivo NAO foi reescrito, e a tela tem de conseguir dizer isso.
    assert!(r.contains("\"difere_do_arquivo\":true"), "{r}");
    assert!(
        r.contains(&format!("127.0.0.1:{velha}")),
        "o bind do arquivo: {r}"
    );
    drop(s);
}

/// O tiro no pe que este item mais arrisca: trocar para uma porta ocupada.
///
/// O endereco novo e preso ANTES de o velho ser solto. Se o `bind` falha, a
/// falha volta e nada muda -- em vez de a maquina ficar sem porta de dados
/// nenhuma e sem caminho de volta.
#[test]
fn porta_ocupada_nao_derruba_o_que_estava_no_ar() {
    let base = pasta("ocupada");
    let porta = porta_livre();
    let _s = subir_servidor(&base, porta);

    let ocupada = porta_livre();
    let _dono = TcpListener::bind(("127.0.0.1", ocupada)).unwrap();

    let r = pedir(
        porta,
        &format!(
            "{{\"token\":\"{TOKEN}\",\"op\":\"servico_subir\",\"bind\":\"127.0.0.1:{ocupada}\"}}"
        ),
    );
    assert!(r.contains("\"ok\":false"), "{r}");
    assert!(r.contains("Nada mudou"), "a recusa tem de dizer isso: {r}");

    // E a prova: a porta de sempre continua atendendo.
    std::thread::sleep(Duration::from_millis(200));
    let r = pedir(
        porta,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico\"}}"),
    );
    assert!(r.contains("\"no_ar\":true"), "{r}");
}

#[test]
fn endereco_escrito_errado_e_recusado_antes_de_qualquer_coisa() {
    let base = pasta("errado");
    let porta = porta_livre();
    let _s = subir_servidor(&base, porta);

    for bind in ["nao-e-endereco", "127.0.0.1:99999", ""] {
        let r = pedir(
            porta,
            &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico_subir\",\"bind\":\"{bind}\"}}"),
        );
        // O vazio quer dizer "sobe onde estava", e ai a recusa e outra: ja
        // esta no ar. As duas sao recusas, e nenhuma das duas troca nada.
        assert!(r.contains("\"ok\":false"), "{bind:?}: {r}");
    }
    let r = pedir(
        porta,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico\"}}"),
    );
    assert!(r.contains("\"no_ar\":true"), "{r}");
}

/// **O caminho de volta.** E a pergunta que este item mais precisa responder:
/// quem parou a porta de dados pela tela, como volta?
///
/// Pela mesma tela. O processo continua vivo e a interface web continua no ar
/// na porta dela -- entao parar a porta de dados nunca e um alcapao. Este
/// teste desce a porta de dados pela porta de dados, prova que ela morreu, e
/// a levanta de novo PELA WEB.
#[test]
fn a_web_levanta_a_porta_de_dados_depois_de_parada() {
    let base = pasta("volta");
    let porta = porta_livre();
    let porta_web = porta_livre();

    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: base.clone(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    c.web.ligado = true;
    c.web.bind = format!("127.0.0.1:{porta_web}");
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar_porta(porta, true).unwrap();
    esperar_porta(porta_web, true).unwrap();

    pedir(
        porta,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico_parar\"}}"),
    );
    esperar_porta(porta, false).unwrap();
    // A web continua de pe: e o que garante que ha caminho de volta.
    esperar_porta(porta_web, true).unwrap();

    let r = pela_web(
        porta_web,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico_subir\"}}"),
    );
    assert!(r.contains("\"ok\":true"), "{r}");
    esperar_porta(porta, true).unwrap();

    // E ela voltou funcionando, e nao so aberta.
    let r = pedir(porta, &format!("{{\"token\":\"{TOKEN}\",\"op\":\"ping\"}}"));
    assert!(r.contains("\"ok\":true"), "{r}");
    drop(s);
}

/// Um `POST /api` na interface web, sem sessao -- so o token, que e como um
/// servidor sem cadastro de usuarios trabalha.
fn pela_web(porta: u16, corpo: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    write!(
        escrita,
        "POST /api HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    )
    .unwrap();
    let mut resposta = String::new();
    BufReader::new(fluxo).read_to_string(&mut resposta).unwrap();
    resposta
}

/// Sobe um servidor com a politica de comandos proibidos -- o firewall do
/// proprio servidor, provado pelo soquete como manda a licao do BULKINSERT.
fn subir_com_politica(base: &std::path::Path, porta: u16, ajustar: impl FnOnce(&mut Config)) {
    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    c.web.ligado = false;
    ajustar(&mut c);
    let s = Servidor::novo(c).unwrap();
    std::thread::spawn(move || {
        let _ = s.escutar();
    });
    esperar_porta(porta, true).expect("o servidor nao subiu");
}

/// O que o teste unitario NAO prova: que a PROXIMA CONEXAO do IP bloqueado e
/// recusada na porta, com o erro nomeando o bloqueio e a duracao -- e que
/// `desbloquear` devolve a porta de verdade.
#[test]
fn ip_bloqueado_tem_a_proxima_conexao_recusada_e_soltar_devolve() {
    let base = pasta("firewall");
    let porta = porta_livre();
    subir_com_politica(&base, porta, |c| {
        c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
        c.politica.tentativas_para_bloqueio = 3;
        c.politica.bloqueio_minutos = 60;
    });

    let proibido = format!(
        "{{\"token\":\"{TOKEN}\",\"op\":\"excluir_tabela\",\"database\":\"x\",\"tabela\":\"y\"}}"
    );
    // Duas primeiras: recusam, contam, e a conexao seguinte AINDA entra.
    for n in 1..=2 {
        let r = pedir(porta, &proibido);
        assert!(r.contains("\"ok\":false"), "{r}");
        assert!(r.contains(&format!("tentativa {n} de 3")), "{r}");
        assert!(r.contains("\"codigo\":4001"), "{r}");
    }
    // A terceira bloqueia.
    let r = pedir(porta, &proibido);
    assert!(r.contains("o IP foi bloqueado"), "{r}");

    // A PROXIMA CONEXAO e recusada antes do token, nomeando ate quando.
    let r = pedir(porta, &format!("{{\"token\":\"{TOKEN}\",\"op\":\"ping\"}}"));
    assert!(r.contains("\"ok\":false"), "{r}");
    assert!(r.contains("bloqueado desde"), "{r}");
    assert!(r.contains("ate"), "{r}");
    assert!(r.contains("comando proibido pela politica"), "{r}");

    // Soltar por OUTRO processo (o caminho do phxsqld --desbloquear): mexe no
    // arquivo, e o servidor rele sozinho.
    {
        let politica = phxsql_server::Politica::default();
        let mut bl = phxsql_server::Blacklist::abrir(base.join("blacklist.json")).unwrap();
        assert!(bl.desbloquear("127.0.0.1", &politica).unwrap());
    }
    let r = pedir(porta, &format!("{{\"token\":\"{TOKEN}\",\"op\":\"ping\"}}"));
    assert!(
        r.contains("\"ok\":true"),
        "desbloquear nao devolveu a porta: {r}"
    );
}

/// Whitelist pelo soquete: o IP protegido pede o comando proibido a vontade,
/// recusa apos recusa, e a conexao seguinte SEMPRE entra.
#[test]
fn whitelist_no_soquete_recusa_sem_nunca_bloquear() {
    let base = pasta("whitelist");
    let porta = porta_livre();
    subir_com_politica(&base, porta, |c| {
        c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
        c.politica.whitelist = vec!["127.0.0.1".into()];
    });

    let proibido = format!(
        "{{\"token\":\"{TOKEN}\",\"op\":\"excluir_tabela\",\"database\":\"x\",\"tabela\":\"y\"}}"
    );
    for _ in 0..5 {
        let r = pedir(porta, &proibido);
        assert!(r.contains("\"ok\":false"), "{r}");
        assert!(r.contains("esta proibida neste servidor"), "{r}");
        assert!(
            !r.contains("o IP foi bloqueado"),
            "a resposta nao pode mentir que bloqueou: {r}"
        );
    }
    let r = pedir(porta, &format!("{{\"token\":\"{TOKEN}\",\"op\":\"ping\"}}"));
    assert!(r.contains("\"ok\":true"), "whitelist deixou bloquear: {r}");
}
