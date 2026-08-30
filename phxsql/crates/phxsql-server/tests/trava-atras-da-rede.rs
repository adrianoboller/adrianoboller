//! A trava de dados NAO fica presa atras de uma leitura de rede.
//!
//! # O defeito que este arquivo trava
//!
//! Ate a 0.18 o laco da replica tomava a trava global de dados na primeira
//! linha de `alcancar_tabela` e a segurava ate o fim -- e no meio dela mora
//! `replica::puxar`, que e uma IDA E VOLTA DE REDE. Numa rede sa isso e
//! invisivel: a resposta chega em microssegundos. Com um corte SILENCIOSO --
//! pacote que some, e nao porta que recusa -- a leitura fica pendurada ate o
//! prazo de 30 s do cliente da replica, e a trava fica presa junto. Todo
//! pedido de cliente que precise dela espera atras: medido na bancada, `ping`
//! respondia em 4 ms e `varrer` em 30.079 ms no MESMO servidor.
//!
//! # Por que este teste e por soquete, e nao unitario
//!
//! Porque o que se prova aqui e o comportamento de um SOQUETE que nao
//! responde, e isso e do sistema operacional. E a licao do `BULKINSERT`: teste
//! unitario nao prova queda de conexao, soquete prova. O "source" daqui e um
//! `TcpListener` de mentira que atende o `posicao` e depois EMUDECE no
//! `replicar` -- aceita o pedido, guarda o soquete aberto e nunca responde.
//!
//! # O PRAZO, que e o que separa reprovar de travar
//!
//! Com o defeito reposto a sonda nao FALHA -- ela PENDURA, por 30 s, e uma
//! bateria que pendura nao reprova ninguem: ela trava. Entao toda sonda daqui
//! roda numa thread propria e e colhida por `recv_timeout`. Estourou o prazo,
//! o teste reprova dizendo isso, e a thread pendurada morre com o processo.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_server::{Config, Origem, Papel, Servidor};

/// Quanto a sonda espera antes de dizer que a trava esta presa.
///
/// Folgado para uma maquina carregada e MUITO menor que os 30 s do prazo de
/// leitura da replica -- e essa distancia e o teste.
const PRAZO_DA_SONDA: Duration = Duration::from_secs(8);

const TOKEN: &str = "trava-atras-da-rede";

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn pasta(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phxsql-trava-rede-{}-{}-{nome}",
        std::process::id(),
        porta_livre()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Um "source" de mentira: responde `posicao` e escolhe o que fazer no
/// `replicar`.
struct FonteFalsa {
    porta: u16,
    /// Quantos `replicar` ja chegaram -- e por ele que o teste sabe que a
    /// replica esta DENTRO do trecho que interessa.
    pedidos: Arc<AtomicU64>,
    /// Emudecer no `replicar`: aceita, guarda o soquete e nao responde.
    muda: Arc<AtomicU64>,
    /// Quantos eventos o source diz ter.
    eventos: u64,
}

impl FonteFalsa {
    fn subir(eventos: u64, muda: bool) -> FonteFalsa {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let pedidos = Arc::new(AtomicU64::new(0));
        let mudo = Arc::new(AtomicU64::new(u64::from(muda)));
        let conta = Arc::clone(&pedidos);
        let quieto = Arc::clone(&mudo);
        // Os soquetes emudecidos ficam VIVOS aqui: soltar o `TcpStream` o
        // fecharia, o nucleo mandaria um FIN e a replica leria "conexao
        // fechada" na hora -- que e justamente o corte BARULHENTO, o que o
        // defeito nao produz. O silencio precisa do soquete de pe.
        let presos: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
        std::thread::spawn(move || {
            for fluxo in ouvinte.incoming() {
                let Ok(fluxo) = fluxo else { return };
                let conta = Arc::clone(&conta);
                let quieto = Arc::clone(&quieto);
                let presos = Arc::clone(&presos);
                std::thread::spawn(move || {
                    let mut escrita = fluxo.try_clone().unwrap();
                    let mut leitor = BufReader::new(fluxo);
                    loop {
                        let mut linha = String::new();
                        if leitor.read_line(&mut linha).unwrap_or(0) == 0 {
                            return;
                        }
                        let op = campo(&linha, "op");
                        match op.as_str() {
                            "posicao" => {
                                let _ = writeln!(
                                    escrita,
                                    "{{\"ok\":true,\"resultado\":{{\"imagem_da_linha\":true,\
                                     \"id_servidor\":\"fonte-falsa\",\"tabelas\":\
                                     {{\"clientes\":{{\"eventos\":{eventos}}}}}}}}}"
                                );
                            }
                            "replicar" => {
                                conta.fetch_add(1, Ordering::SeqCst);
                                if quieto.load(Ordering::SeqCst) == 1 {
                                    // O silencio: guarda o soquete e some.
                                    presos.lock().unwrap().push(escrita);
                                    return;
                                }
                                // Nada a mandar, e a posicao nao anda: a
                                // replica sai do laco sem aplicar nada.
                                let _ = writeln!(
                                    escrita,
                                    "{{\"ok\":true,\"resultado\":{{\"desde\":0,\"ate\":0,\
                                     \"total\":{eventos},\"fim\":true,\"eventos\":[]}}}}"
                                );
                            }
                            _ => {
                                let _ = writeln!(escrita, "{{\"ok\":true,\"resultado\":{{}}}}");
                            }
                        }
                        let _ = escrita.flush();
                    }
                });
            }
        });
        FonteFalsa {
            porta,
            pedidos,
            muda: mudo,
            eventos,
        }
    }
}

/// O valor de um campo de texto do JSON, sem montar um analisador aqui.
fn campo(linha: &str, nome: &str) -> String {
    let marca = format!("\"{nome}\":\"");
    match linha.find(&marca) {
        None => String::new(),
        Some(i) => {
            let resto = &linha[i + marca.len()..];
            resto[..resto.find('"').unwrap_or(0)].to_string()
        }
    }
}

fn subir_replica(base: &std::path::Path, porta: u16, fonte: &FonteFalsa) -> Arc<Servidor> {
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
    c.replicacao.papel = Papel::Replica;
    c.replicacao.id_servidor = "replica-do-teste".into();
    c.replicacao.imagem_da_linha = true;
    c.replicacao.origens = vec![Origem {
        // A cifra do fio nao entra aqui de proposito: esta guarda mede a
        // trava com o fio em claro, que e o caminho que a cifra promete
        // deixar como estava.
        cifra: false,
        chave_do_fio: String::new(),
        nome: "fonte-falsa".into(),
        host: "127.0.0.1".into(),
        porta: fonte.porta,
        token: String::new(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
    }];
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar_porta(porta);
    s
}

fn esperar_porta(porta: u16) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("a porta {porta} nao abriu em 5 s");
}

/// Um pedido pela porta de dados, com PRAZO -- ver a nota do topo.
///
/// Devolve `Err` quando o prazo estourou, e nunca pendura o teste.
fn pedir_com_prazo(porta: u16, linha: &str) -> Result<(String, Duration), String> {
    let (envio, volta) = mpsc::channel();
    let pedido = linha.replace('\n', " ");
    std::thread::spawn(move || {
        let inicio = Instant::now();
        let r = (|| -> std::io::Result<String> {
            let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
            let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3))?;
            // Prazo de leitura MAIOR que o da sonda: quem decide a reprovacao
            // e o `recv_timeout` la fora, e nao um erro de soquete que diria
            // "o servidor caiu" quando ele so esta esperando a trava.
            fluxo.set_read_timeout(Some(Duration::from_secs(60)))?;
            let mut escrita = fluxo.try_clone()?;
            let mut leitor = BufReader::new(fluxo);
            writeln!(escrita, "{pedido}")?;
            let mut resposta = String::new();
            leitor.read_line(&mut resposta)?;
            Ok(resposta)
        })();
        let _ = envio.send((r, inicio.elapsed()));
    });
    match volta.recv_timeout(PRAZO_DA_SONDA) {
        Ok((Ok(r), t)) => Ok((r, t)),
        Ok((Err(e), _)) => Err(format!("erro de soquete: {e}")),
        Err(_) => Err(format!(
            "sem resposta em {} s -- a trava de dados esta presa",
            PRAZO_DA_SONDA.as_secs()
        )),
    }
}

fn exigir(porta: u16, linha: &str) -> String {
    let (r, _) = pedir_com_prazo(porta, linha).unwrap_or_else(|e| panic!("{linha}: {e}"));
    assert!(r.contains("\"ok\":true"), "{linha} -> {r}");
    r
}

fn criar_tabela(porta: u16) {
    let t = format!("\"token\":\"{TOKEN}\"");
    // Sem `exigir`: o laco da replica chama `garantir_database` na primeira
    // rodada, entao o banco pode ja existir -- e isso nao e erro nenhum, e a
    // corrida normal entre o teste e o laco.
    let _ = pedir_com_prazo(
        porta,
        &format!("{{{t},\"op\":\"criar_database\",\"database\":\"loja\"}}"),
    );
    exigir(
        porta,
        &format!(
            "{{{t},\"op\":\"criar_tabela\",\"database\":\"loja\",\"tabela\":\"clientes\",
              \"colunas\":[{{\"nome\":\"id\",\"tipo\":\"Int4\",\"obrigatoria\":true}}]}}"
        ),
    );
}

/// Espera a replica entrar no `replicar` -- que e onde o defeito morava.
fn esperar_o_laco(fonte: &FonteFalsa) {
    let ate = Instant::now() + Duration::from_secs(20);
    while Instant::now() < ate {
        if fonte.pedidos.load(Ordering::SeqCst) > 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("a replica nunca pediu `replicar` em 20 s -- o cenario nao exercita o defeito");
}

/// A prova: com o source EMUDECIDO no `replicar`, o servidor continua
/// atendendo quem precisa da trava de dados.
///
/// As duas sondas juntas sao o diagnostico. `ping` nao toca na trava e
/// responde de qualquer jeito; se so ele respondesse, o servidor estaria no ar
/// e a trava presa -- que e exatamente o retrato do defeito. Aqui os dois tem
/// de responder.
#[test]
fn source_mudo_nao_prende_a_trava_de_dados() {
    let fonte = FonteFalsa::subir(100_000, true);
    let base = pasta("mudo");
    let porta = porta_livre();
    let _replica = subir_replica(&base, porta, &fonte);
    criar_tabela(porta);
    esperar_o_laco(&fonte);
    // O laco esta pendurado na leitura do soquete AGORA. Um instante para a
    // ida e volta que nunca vem se acomodar.
    std::thread::sleep(Duration::from_millis(300));

    let t = format!("\"token\":\"{TOKEN}\"");
    let (_, sem_trava) = pedir_com_prazo(porta, &format!("{{{t},\"op\":\"ping\"}}"))
        .expect("nem o `ping` respondeu -- o servidor caiu, e o defeito e outro");
    let (resposta, com_trava) = pedir_com_prazo(
        porta,
        &format!(
            "{{{t},\"op\":\"varrer\",\"database\":\"loja\",\"tabela\":\"clientes\",\"max\":1}}"
        ),
    )
    .unwrap_or_else(|e| {
        panic!(
            "`varrer` na replica com o source mudo: {e}. O `ping`, que nao \
             precisa da trava, respondeu em {sem_trava:?} -- entao o servidor \
             esta no ar e o que espera e a trava de dados, presa atras da \
             leitura de rede do laco da replica."
        )
    });
    assert!(resposta.contains("\"ok\":true"), "varrer -> {resposta}");
    // O contraste importa mais que o numero absoluto: numa maquina carregada
    // os dois sobem juntos, e o que nao pode acontecer e um deles ir para a
    // casa dos segundos enquanto o outro fica em milissegundos.
    assert!(
        com_trava < Duration::from_secs(5),
        "`varrer` levou {com_trava:?} com o source mudo (o `ping` levou \
         {sem_trava:?}): a trava de dados continua presa atras da rede"
    );
}

/// O comportamento VELHO, que nao pode mudar: com a rede sa, a replica fala
/// com o source e o servidor atende normalmente.
///
/// Sem este teste, um conserto que quebrasse a replicacao inteira passaria no
/// de cima com louvor -- laco que nao replica nada tambem nao segura trava
/// nenhuma.
#[test]
fn com_a_rede_sa_a_replica_conversa_e_o_servidor_atende() {
    let fonte = FonteFalsa::subir(100_000, false);
    let base = pasta("sadia");
    let porta = porta_livre();
    let _replica = subir_replica(&base, porta, &fonte);
    criar_tabela(porta);
    esperar_o_laco(&fonte);
    assert!(
        fonte.pedidos.load(Ordering::SeqCst) > 0,
        "a replica nem chegou a pedir `replicar`"
    );
    let t = format!("\"token\":\"{TOKEN}\"");
    let (r, quanto) = pedir_com_prazo(
        porta,
        &format!(
            "{{{t},\"op\":\"varrer\",\"database\":\"loja\",\"tabela\":\"clientes\",\"max\":1}}"
        ),
    )
    .expect("`varrer` com a rede sa");
    assert!(r.contains("\"ok\":true"), "varrer -> {r}");
    assert!(
        quanto < Duration::from_secs(5),
        "`varrer` levou {quanto:?} com a rede sa"
    );
    // O `muda` existe so para o outro teste; ler aqui evita o campo morto.
    assert_eq!(fonte.muda.load(Ordering::SeqCst), 0);
    assert_eq!(fonte.eventos, 100_000);
}
