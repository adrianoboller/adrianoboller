//! As threads e os seus tetos -- pedido 248.
use super::*;
use std::io::Read;

fn config_base(dir: &std::path::Path) -> Config {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    // O ESCAPE ESCRITO, e ele esta aqui por assunto: desde 18/09/2026 a
    // cifra do fio nasce exigida (pedido 370), e estes testes conectam em
    // claro porque o que eles medem e OUTRA coisa. Sem esta linha, a
    // recusa que eles leriam seria a da cifra, e a prova mediria o portao
    // errado -- teste que passa (ou falha) por engano.
    c.cifra_fio.exigir = false;
    c
}

/// Sobe a porta de dados pelo laco DE PRODUCAO (`aceitar_ate_mandarem_parar`),
/// e nao pelo atalho dos outros testes: a vaga e pedida ali, e e ali que
/// a prova tem de passar.
fn porta_de_dados_de_verdade(s: &Arc<Servidor>) -> u16 {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let s = Arc::clone(s);
    std::thread::spawn(move || s.aceitar_ate_mandarem_parar(&ouvinte));
    porta
}

fn porta_web(s: &Arc<Servidor>) -> u16 {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    s.aceitar_http(ouvinte, "web", None, |s, fluxo, par| {
        s.atender_http(fluxo, par)
    });
    porta
}

fn espera_ate(deadline: Duration, condicao: impl Fn() -> bool) -> bool {
    let fim = Instant::now() + deadline;
    while Instant::now() < fim {
        if condicao() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    condicao()
}

/// Um `ping` numa conexao nova, pelo cliente unico de teste. `None`: a
/// conexao foi recusada, ou caiu sem responder.
fn ping(porta: u16) -> Option<String> {
    crate::apoio_teste::Ligacao::tentar_com_prazo(porta, Duration::from_secs(3))?
        .pedir(r#"{"op":"ping","token":"t"}"#)
}

/// Le uma resposta HTTP inteira (cabecalho e corpo) ate o outro lado fechar.
fn resposta_http(c: &mut TcpStream) -> String {
    let mut tudo = Vec::new();
    let _ = c.read_to_end(&mut tudo);
    String::from_utf8_lossy(&tudo).into_owned()
}

fn get_saude(porta: u16) -> String {
    let mut c = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    c.write_all(b"GET /saude HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    resposta_http(&mut c)
}

/// **A prova real da permissao RAII.** Tres conexoes entram em panico
/// dentro do `atender`, com teto de duas vagas. Com o contador de mao
/// (`fetch_add`/`fetch_sub`), a devolucao ficava DEPOIS do corpo e o
/// panico a pulava: a vaga nunca voltava, e depois de duas a porta
/// fechava com o servidor de pe. Com a permissao no `Drop`, cada vaga
/// volta e a quarta conexao e atendida. O defeito reposto esta no
/// catalogo de guardas (`permissao-de-dados-sem-raii`).
///
/// # Por que a espera da vaga fica DENTRO do laco (pedido 267)
///
/// A versao anterior disparava os tres `ping` em fila e so no fim cobrava
/// «os tres panicos aconteceram». Isso cobra do motor uma garantia que ele
/// nao da: o cliente ve o fim da conexao quando o soquete morre no
/// desenrolar do panico, e a vaga volta um pouco DEPOIS, quando a thread
/// acaba. Medido (sonda do pedido 267, 500 rodadas do cenario dentro da
/// suite `--lib` inteira, com tres suites de carga ao lado, load 13-15):
/// a janela entre as duas coisas tem p50 de 1 us, p90 de 4,1 ms e maximo
/// de 27,8 ms, e em **20 rodadas de 500 (4,0%)** a terceira conexao
/// chegava dentro dela. Nas vinte, os contadores diziam `aceitas=3` e
/// `sem_vaga=1`: a conexao ENTROU e foi recusada por falta de vaga, que e
/// o comportamento certo acima do teto -- o teste e que reprovava o motor
/// por um defeito que nao existe.
///
/// **A grandeza mudou, e nao o numero.** Em vez de um total conferido no
/// fim, cada conexao prova a sua: panicou e devolveu a vaga, e a proxima
/// so parte com a vaga de volta. As tres continuam entrando com teto de
/// duas, que e o que prova o reaproveitamento. E a mensagem separa as duas
/// causas que o total confundia -- «nao panicou» e «nao entrou» --, pelos
/// contadores de aceitas e de recusas por falta de vaga.
#[test]
fn panico_dentro_do_atender_devolve_a_vaga_da_porta_de_dados() {
    let dir = DirTemp::novo("vaga-raii");
    let mut c = config_base(&dir);
    c.conexoes_max = 2;
    c.recursos.conexoes_max = 2;
    let s = Servidor::novo(c).unwrap();
    let porta = porta_de_dados_de_verdade(&s);
    assert_eq!(s.permissoes_de_dados.teto(), 2);

    s.panicos_de_teste.store(3, Ordering::SeqCst);
    for i in 0..3 {
        let faltavam = s.panicos_de_teste.load(Ordering::SeqCst);
        // Cada uma cai: a thread entra em panico antes de ler, e o
        // cliente ve o fim da conexao. O que interessa e o que sobra
        // DEPOIS: a vaga.
        let r = ping(porta);
        assert!(
            r.is_none(),
            "a conexao {i} devia ter caido, respondeu {r:?}"
        );
        // A assercao que o `ManuallyDrop` derruba, e ja na primeira volta.
        assert!(
            espera_ate(Duration::from_secs(5), || s.permissoes_de_dados.em_uso()
                == 0),
            "a vaga da conexao {i} nao voltou depois do panico: {} em uso",
            s.permissoes_de_dados.em_uso()
        );
        assert_eq!(
            s.panicos_de_teste.load(Ordering::SeqCst),
            faltavam - 1,
            "a conexao {i} nao entrou em panico (aceitas={} recusadas_por_falta_de_vaga={})",
            s.aceitas_de_teste.load(Ordering::SeqCst),
            s.sem_vaga_de_teste.load(Ordering::SeqCst)
        );
    }
    let r = ping(porta).expect("a quarta conexao tinha de ser atendida");
    assert!(r.contains("\"ok\":true"), "{r}");
}

/// **O instrumento que mediu o pedido 267** -- fora da bateria por custo
/// (`#[ignore]`), e nao por defeito.
///
/// Ele repete o cenario do teste de cima muitas vezes e conta, por `ping`:
/// se a conexao foi ACEITA (`aceitas`), se foi recusada por falta de vaga
/// (`sem_vaga`), e quanto tempo separa «o cliente viu o fim da conexao» de
/// «a vaga voltou» -- a janela em que a conexao seguinte leva recusa. A
/// espera do fim e OCUPADA de proposito: a janela e de microssegundos, e
/// um `sleep` de 5 ms mediria o proprio `sleep`.
///
/// `ESPERA=1` roda a forma NOVA do teste (espera a vaga voltar entre uma
/// conexao e a proxima); sem ela, a forma antiga, a que caia.
///
/// **Ele so reproduz DENTRO da suite inteira**: rodado sozinho, mesmo com
/// a maquina carregada por fora, deu 0 em 340 rodadas. A receita medida:
///
/// ```bash
/// # tres suites `--lib` em laco ao lado, para a carga passar de 12
/// RODADAS=500 <binario-da-suite> --include-ignored --nocapture --test-threads=4 \
///   2>&1 | grep SONDA267
/// ```
///
/// Medido em 16/09/2026, load 13-15: forma antiga **20 rodadas de 500
/// (4,0%)** com panico faltando, e nas vinte `aceitas=3 sem_vaga=1`;
/// forma nova **0 de 500**. Janela: p50 1-2 us, p90 4,1-6,9 ms, maximo
/// 36,7 ms.
#[test]
#[ignore]
fn sonda_267_corrida_dos_tres_panicos() {
    let rodadas: usize = std::env::var("RODADAS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let com_espera = std::env::var("ESPERA").is_ok();
    let mut sobrou = 0usize;
    let mut recusados = 0usize;
    let mut nao_aceitos = 0usize;
    let mut janelas: Vec<u128> = Vec::new();
    let mut em_uso_antes = [0usize; 3];
    for rodada in 0..rodadas {
        let dir = DirTemp::novo(&format!("sonda267-{rodada}"));
        let mut c = config_base(&dir);
        c.conexoes_max = 2;
        c.recursos.conexoes_max = 2;
        let s = Servidor::novo(c).unwrap();
        let porta = porta_de_dados_de_verdade(&s);
        s.panicos_de_teste.store(3, Ordering::SeqCst);
        for (i, uso_antes) in em_uso_antes.iter_mut().enumerate() {
            let antes_ac = s.aceitas_de_teste.load(Ordering::SeqCst);
            let antes_sv = s.sem_vaga_de_teste.load(Ordering::SeqCst);
            *uso_antes += s.permissoes_de_dados.em_uso();
            let _ = ping(porta);
            let fim_da_conexao = Instant::now();
            if s.aceitas_de_teste.load(Ordering::SeqCst) == antes_ac {
                nao_aceitos += 1;
            }
            if s.sem_vaga_de_teste.load(Ordering::SeqCst) > antes_sv {
                recusados += 1;
            }
            if com_espera {
                let _ = espera_ate(Duration::from_secs(5), || {
                    s.permissoes_de_dados.em_uso() == 0
                });
            }
            if i == 2 {
                while s.permissoes_de_dados.em_uso() > 0
                    && fim_da_conexao.elapsed() < Duration::from_secs(5)
                {
                    std::hint::spin_loop();
                }
                janelas.push(fim_da_conexao.elapsed().as_micros());
            }
        }
        let _ = espera_ate(Duration::from_secs(5), || {
            s.permissoes_de_dados.em_uso() == 0
        });
        let resto = s.panicos_de_teste.load(Ordering::SeqCst);
        if resto > 0 {
            sobrou += 1;
            eprintln!(
                "SONDA267 rodada={rodada} panicos_restantes={resto} aceitas={} sem_vaga={}",
                s.aceitas_de_teste.load(Ordering::SeqCst),
                s.sem_vaga_de_teste.load(Ordering::SeqCst)
            );
        }
    }
    janelas.sort_unstable();
    let p = |q: f64| janelas[((janelas.len() as f64 - 1.0) * q) as usize];
    eprintln!(
        "SONDA267 rodadas={rodadas} espera={com_espera} com_panico_faltando={sobrou} \
             pings_recusados_por_falta_de_vaga={recusados} pings_nao_aceitos={nao_aceitos} \
             em_uso_medio_antes_do_ping=[{:.2} {:.2} {:.2}] \
             janela_us p50={} p90={} p99={} max={}",
        em_uso_antes[0] as f64 / rodadas as f64,
        em_uso_antes[1] as f64 / rodadas as f64,
        em_uso_antes[2] as f64 / rodadas as f64,
        p(0.50),
        p(0.90),
        p(0.99),
        janelas[janelas.len() - 1]
    );
}

/// A recusa da porta de dados continua IMEDIATA e continua no log: e o
/// comportamento de sempre, e o teste e do comportamento velho.
#[test]
fn a_porta_de_dados_continua_recusando_na_hora_acima_do_teto() {
    let dir = DirTemp::novo("recusa-imediata");
    let mut c = config_base(&dir);
    c.conexoes_max = 1;
    c.recursos.conexoes_max = 1;
    let s = Servidor::novo(c).unwrap();
    let porta = porta_de_dados_de_verdade(&s);

    // A primeira ocupa a unica vaga e fica aberta.
    let mut a = crate::apoio_teste::Ligacao::tentar_com_prazo(porta, Duration::from_secs(3))
        .expect("a primeira conexao foi recusada");
    let linha = a
        .pedir(r#"{"op":"ping","token":"t"}"#)
        .expect("a primeira conexao caiu sem resposta");
    assert!(linha.contains("\"ok\":true"), "{linha}");
    assert_eq!(s.permissoes_de_dados.em_uso(), 1);

    // A segunda e recusada na hora -- conexao fechada sem resposta.
    let t0 = Instant::now();
    assert!(ping(porta).is_none(), "acima do teto tinha de recusar");
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "a recusa nao e imediata"
    );
    let log = std::fs::read_to_string(dir.join("acessos.log")).unwrap_or_default();
    assert!(log.contains("limite de conexoes atingido"), "{log}");

    drop(a);
    assert!(espera_ate(Duration::from_secs(5), || s
        .permissoes_de_dados
        .em_uso()
        == 0));
    assert!(
        ping(porta).is_some(),
        "a vaga tinha de voltar quando a primeira fechou"
    );
}

/// **O comportamento VELHO**: abaixo do teto nada muda na web -- toda
/// resposta e 200, nenhuma e 503, e nenhuma linha de recusa entra no log.
#[test]
fn abaixo_do_teto_a_web_nao_muda() {
    let dir = DirTemp::novo("web-abaixo-do-teto");
    let mut c = config_base(&dir);
    c.recursos.conexoes_web_max = 4;
    let s = Servidor::novo(c).unwrap();
    let porta = porta_web(&s);
    for _ in 0..6 {
        let r = get_saude(porta);
        assert!(r.starts_with("HTTP/1.1 200 "), "{r}");
        assert!(!r.contains("Retry-After"), "{r}");
    }
    assert!(espera_ate(Duration::from_secs(5), || s
        .permissoes_http
        .em_uso()
        == 0));
    let log = std::fs::read_to_string(dir.join("acessos.log")).unwrap_or_default();
    assert!(!log.contains("porta HTTP cheia"), "{log}");
}

/// Acima do teto, com a fila esgotada: 503 com `Retry-After`, linha no
/// log, e a vaga VOLTA quando o pedido que a segurava termina.
#[test]
fn acima_do_teto_a_web_responde_503_com_retry_after_e_a_vaga_volta() {
    let dir = DirTemp::novo("web-503");
    let mut c = config_base(&dir);
    c.recursos.conexoes_web_max = 1;
    c.recursos.fila_web_ms = 100;
    let s = Servidor::novo(c).unwrap();
    let porta = porta_web(&s);

    // A segura a unica vaga: manda o cabecalho SEM a linha vazia final,
    // e a thread dela fica esperando o resto.
    let mut a = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    a.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    a.write_all(b"GET /saude HTTP/1.1\r\nHost: x\r\n").unwrap();
    assert!(
        espera_ate(Duration::from_secs(5), || s.permissoes_http.em_uso() == 1),
        "A nao tomou a vaga"
    );

    // B espera os 100 ms da fila e recebe 503.
    let t0 = Instant::now();
    let r = get_saude(porta);
    let esperou = t0.elapsed();
    assert!(r.starts_with("HTTP/1.1 503 "), "{r}");
    assert!(r.contains("\r\nRetry-After: 1\r\n"), "{r}");
    assert!(r.contains("porta HTTP cheia"), "{r}");
    assert!(r.contains("\"retry_after_s\":1"), "{r}");
    assert!(
        esperou >= Duration::from_millis(100),
        "nao esperou a fila: {esperou:?}"
    );

    // C chega com a fila DECLARADA cheia e recebe o 503 sem esperar.
    let t0 = Instant::now();
    let r = get_saude(porta);
    assert!(r.starts_with("HTTP/1.1 503 "), "{r}");
    assert!(
        t0.elapsed() < Duration::from_millis(100),
        "com a fila declarada cheia a recusa tinha de ser imediata: {:?}",
        t0.elapsed()
    );

    // A termina o pedido e recebe 200: a vaga dela volta.
    a.write_all(b"\r\n").unwrap();
    let r = resposta_http(&mut a);
    assert!(r.starts_with("HTTP/1.1 200 "), "{r}");
    assert!(espera_ate(Duration::from_secs(5), || s
        .permissoes_http
        .em_uso()
        == 0));
    // Passa a janela da fila declarada cheia, para que D nao dependa
    // de quem chegou primeiro: a vaga ou o relogio.
    std::thread::sleep(Duration::from_millis(150));
    let r = get_saude(porta);
    assert!(
        r.starts_with("HTTP/1.1 200 "),
        "depois de A soltar, D tinha de entrar: {r}"
    );

    let log = std::fs::read_to_string(dir.join("acessos.log")).unwrap_or_default();
    assert!(
        log.contains("porta HTTP cheia (1 threads, fila de 100 ms)"),
        "{log}"
    );
}

/// O monitor em runtime: os tetos saem com a ocupacao lida AGORA, e o
/// que nao se mede ao vivo diz isso em vez de dizer zero.
#[test]
fn os_tetos_das_threads_saem_com_a_ocupacao_viva() {
    let dir = DirTemp::novo("tetos-vivos");
    let mut c = config_base(&dir);
    c.conexoes_max = 3;
    c.recursos.conexoes_max = 3;
    c.recursos.conexoes_web_max = 0;
    let s = Servidor::novo(c).unwrap();
    let _vaga = s.permissoes_de_dados.tentar().unwrap();
    let j = s.tetos_das_threads();
    let Json::Lista(itens) = &j else {
        panic!("tetos nao e lista: {}", j.escrever())
    };
    let acha = |f: &str| {
        itens
            .iter()
            .find(|i| i.texto_ou("familia", "") == f)
            .unwrap_or_else(|| panic!("faltou a familia {f}: {}", j.escrever()))
    };
    let dados = acha("dados");
    assert_eq!(dados.inteiro_ou("em_uso", -1), 1);
    assert_eq!(dados.inteiro_ou("teto", -1), 3);
    assert_eq!(dados.inteiro_ou("esperando", -1), 0);
    assert!(dados.booleano_ou("vivo", false));
    let http = acha("http");
    assert!(
        matches!(http.campo("teto"), Some(Json::Nulo)),
        "sem teto tem de sair nulo, nao um numero: {}",
        http.escrever()
    );
    let fecho = acha("fecho");
    assert!(
        !fecho.booleano_ou("vivo", true),
        "o fecho nao se mede ao vivo"
    );
    assert_eq!(fecho.inteiro_ou("teto", -1), FIOS_DO_FECHO as i64);
    assert!(
        fecho.campo("em_uso").is_none(),
        "em_uso inventado: {}",
        fecho.escrever()
    );
    assert!(acha("varredura").inteiro_ou("teto", 0) >= 1);
}

/// `conexoes_web_max: 0` e o comportamento de antes: sem teto, e o
/// contador continua contando.
#[test]
fn zero_no_teto_da_web_e_sem_teto() {
    let dir = DirTemp::novo("web-sem-teto");
    let mut c = config_base(&dir);
    c.recursos.conexoes_web_max = 0;
    let s = Servidor::novo(c).unwrap();
    assert!(!s.permissoes_http.limitado());
    assert_eq!(s.permissoes_http.em_uso(), 0);
    let porta = porta_web(&s);
    let r = get_saude(porta);
    assert!(r.starts_with("HTTP/1.1 200 "), "{r}");
}
