//! Parar e subir a porta de dados pela tela -- provado PELO SOQUETE.
//!
//! Teste unitario nao prova isto. O que se quer saber e se o `accept`
//! bloqueado acorda, se a porta e mesmo SOLTA quando o laco sai, e se conectar
//! nela depois disso e recusado pelo sistema operacional. Nada disso e
//! observavel de dentro do processo sem abrir um soquete de verdade -- e a
//! licao do `BULKINSERT` foi que um teste que passa por engano e pior que um
//! teste que falta.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-do-servico";

/// Pede a porta 0 e devolve a REAL, lida do proprio servidor -- pedido 401.
///
/// # O que este arquivo ja pagou por escolher um numero por fora
///
/// Ate o pedido 401 a porta nascia de um `porta_livre()` que reservava,
/// soltava e guardava um conjunto dos numeros ja entregues NESTE processo --
/// um remendo, nao um conserto: a CI pegou em 03/09/2026 a mesma familia de
/// corrida, so que entre threads do MESMO binario, um `bind(:0)` de uma
/// thread recebendo a porta que a outra tinha acabado de soltar. O servidor
/// avisou «Address already in use», o `esperar_porta` viu a porta ABERTA --
/// era quem a tinha tomado --, e o teste conversou com um soquete alheio.
/// Pedir porta 0 e ler a REAL de volta fecha a janela por construcao: nao ha
/// mais numero nenhum escolhido por fora para outra thread disputar.
fn subir_servidor(base: &std::path::Path) -> (Arc<Servidor>, u16) {
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
    // A interface web fica de fora: este teste e sobre a porta de dados, e
    // subir a web tomaria uma segunda porta sem necessidade.
    // O ESCAPE ESCRITO: desde 18/09/2026 a cifra do fio nasce exigida
    // (pedido 370, ordem do dono), e esta bateria conecta em claro porque o
    // que ela mede e o servico -- parar, subir, trocar de porta e o
    // firewall. Sem esta linha a recusa lida aqui seria a da cifra, e a prova
    // passaria a medir o portao errado.
    c.cifra_fio.exigir = false;
    c.web.ligado = false;
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    esperar_porta(porta, true).expect("o servidor nao subiu");
    (s, porta)
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
    // A escrita pode falhar com `BrokenPipe`/`ConnectionReset` SEM que nada
    // esteja errado: quando o IP esta bloqueado, o servidor recusa ANTES de
    // ler o pedido -- escreve a recusa e fecha. Se o fechamento chega antes do
    // nosso `write`, o `unwrap` derrubava o teste pelo COMPORTAMENTO CERTO.
    //
    // Foi o que aconteceu numa corrida de `--workspace`, com a maquina cheia
    // de binarios em paralelo; sozinho o teste passa sempre, porque ali o
    // cliente ganha a corrida. Ignorar a falha de escrita nao afrouxa nada: a
    // resposta ja esta no soquete, e as asercoes sobre ela continuam iguais.
    let _ = writeln!(escrita, "{linha}");
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    resposta
}

fn pasta(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("servico-{nome}"))
}

#[test]
fn parar_solta_a_porta_e_subir_a_devolve() {
    let base = pasta("parar-subir");
    let (_s, porta) = subir_servidor(&base);

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
    let (s, velha) = subir_servidor(&base);

    // O pedido TAMBEM manda porta 0: quem troca pela tela nao precisa
    // adivinhar um numero livre, e o teste nao teria como saber um numero
    // real sem antes ligar nele -- exatamente a corrida do pedido 401, agora
    // do lado do `servico_subir`. A resposta confirma a troca; o numero novo
    // sai de `porta_dos_dados()` depois dela.
    let r = pedir(
        velha,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico_subir\",\"bind\":\"127.0.0.1:0\"}}"),
    );
    assert!(r.contains("\"ok\":true"), "{r}");
    assert!(r.contains("\"trocou_de_porta\":true"), "{r}");

    let nova = comum::porta_trocou_para(|| s.porta_dos_dados(), velha);
    esperar_porta(nova, true).unwrap();
    esperar_porta(velha, false).unwrap();

    let r = pedir(
        nova,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"servico\"}}"),
    );
    assert!(r.contains(&format!(":{nova}")), "{r}");
    // O arquivo NAO foi reescrito, e a tela tem de conseguir dizer isso: o
    // `bind` configurado continua o texto "0" que este teste escreveu -- e
    // nunca vira um numero de porta, porque a resolucao de porta 0 acontece
    // so no `bind`, nunca na leitura do campo bruto do config.
    assert!(r.contains("\"difere_do_arquivo\":true"), "{r}");
    assert!(
        r.contains("\"bind_configurado\":\"127.0.0.1:0\""),
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
    let (_s, porta) = subir_servidor(&base);

    // O dono nasce e FICA: nunca solta o que reservou, entao nao ha janela
    // nenhuma para outro processo tomar o numero antes do `servico_subir`
    // tentar -- e e exatamente essa janela que o pedido 401 fecha.
    let _dono = TcpListener::bind("127.0.0.1:0").unwrap();
    let ocupada = _dono.local_addr().unwrap().port();

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
    let (_s, porta) = subir_servidor(&base);

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
    // O ESCAPE ESCRITO: desde 18/09/2026 a cifra do fio nasce exigida
    // (pedido 370, ordem do dono), e esta bateria conecta em claro porque o
    // que ela mede e o servico -- parar, subir, trocar de porta e o
    // firewall. Sem esta linha a recusa lida aqui seria a da cifra, e a prova
    // passaria a medir o portao errado.
    c.cifra_fio.exigir = false;
    c.web.ligado = true;
    c.web.bind = "127.0.0.1:0".into();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    let porta_web = comum::porta_real(|| s.porta_web());
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
    // O `unwrap` daqui foi o que a CI mostrou, e ele nao dizia nada: um
    // `WouldBlock` cru. Ele acontece quando quem atende na porta NAO e o nosso
    // servidor -- outro processo a tomou e nao fala HTTP --, e e essa a frase
    // que o proximo a ler precisa.
    if let Err(e) = BufReader::new(fluxo).read_to_string(&mut resposta) {
        panic!(
            "a porta {porta} aceitou a conexao mas nao respondeu como o nosso \
             servidor ({e}) -- provavel colisao de porta: quem atende ali e de \
             outro processo"
        );
    }
    resposta
}

/// Sobe um servidor com a politica de comandos proibidos -- o firewall do
/// proprio servidor, provado pelo soquete como manda a licao do BULKINSERT.
///
/// Pede a porta 0 e devolve a REAL, lida do proprio servidor -- pedido 401.
fn subir_com_politica(base: &std::path::Path, ajustar: impl FnOnce(&mut Config)) -> u16 {
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
    // O ESCAPE ESCRITO: desde 18/09/2026 a cifra do fio nasce exigida
    // (pedido 370, ordem do dono), e esta bateria conecta em claro porque o
    // que ela mede e o servico -- parar, subir, trocar de porta e o
    // firewall. Sem esta linha a recusa lida aqui seria a da cifra, e a prova
    // passaria a medir o portao errado.
    c.cifra_fio.exigir = false;
    c.web.ligado = false;
    ajustar(&mut c);
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    esperar_porta(porta, true).expect("o servidor nao subiu");
    porta
}

/// O que o teste unitario NAO prova: que a PROXIMA CONEXAO do IP bloqueado e
/// recusada na porta, com o erro nomeando o bloqueio e a duracao -- e que
/// `desbloquear` devolve a porta de verdade.
#[test]
fn ip_bloqueado_tem_a_proxima_conexao_recusada_e_soltar_devolve() {
    let base = pasta("firewall");
    let porta = subir_com_politica(&base, |c| {
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
    let porta = subir_com_politica(&base, |c| {
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
