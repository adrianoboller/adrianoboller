//! O teto de UM REGISTRO do fio (`TETO_DO_REGISTRO`, 128 MiB), provado PELO
//! SOQUETE nos DOIS lados que recebem.
//!
//! # O que faltava, medido
//!
//! O pedido 303 fechou metade em `49a3af7`: o `TETO_DO_LOTE_SERVIDO` do lado
//! que SERVE ganhou prova. O lado que RECEBE -- o `TETO_DA_RESPOSTA` de
//! `docs/REPLICACAO.md` §18, que e este teto do fio lido pela replica em
//! `replica.rs:183` -- continuava sem prova nenhuma. Medido em 17/09/2026,
//! trocando `Canal::ler` por `ler_ate(leitor, u64::MAX - 1)` -- o fio inteiro
//! sem teto: a bateria do repositorio INTEIRO voltou **2.452 passando e 3
//! falhando**, e as tres sao as provas deste arquivo e a irma unitaria dele.
//! Nenhum teste que ja existia acusa o fio sem teto.
//!
//! # Por que soquete, e nao so teste de unidade
//!
//! O unitario irmao mede QUANTO se leu, com igualdade exata -- e isso e o dano
//! da memoria; o nome inteiro dele e
//! `fio::testes::a_leitura_padrao_para_no_teto_do_registro_e_nao_no_que_o_outro_lado_mandar`.
//! O que ele nao alcanca e o que depende do sistema operacional: se a recusa
//! chega ao outro lado ou se a conexao morre calada, e se a rodada seguinte
//! ainda consegue trabalhar. Essa e a licao do `BULKINSERT`: teste de unidade
//! nao prova queda de conexao, soquete prova.
//!
//! E a divisao entre os dois esta MEDIDA, e nao suposta -- e por isso nenhum
//! dos dois e redundante. Repondo o OUTRO defeito deste teto (o `take` fora,
//! que e a entrada `fio-sem-teto-de-registro` do catalogo de guardas), a prova
//! da replica aqui de baixo **PASSA**: a recusa continua chegando com o nome
//! certo, porque o veredito `lidos > teto` acontece do mesmo jeito -- so que
//! depois de a memoria ter sido gasta, e memoria gasta nao aparece no fio.
//! Quem quiser cortar uma das duas por parecerem a mesma prova, meca antes.
//!
//! # Os dois lados nao prometem a mesma coisa, e isso esta medido aqui
//!
//! * **replica** (`replica.rs:183`): recusa `LIMITE_EXCEDIDO` com o numero
//!   dentro, e a conexao NAO se reaproveita -- ela ficou no meio de uma linha.
//!   O que se prova e que a proxima rodada abre outra e trabalha.
//! * **servidor** (`servidor.rs:8881`): RESPONDE a recusa antes de fechar, e
//!   anota no `acessos.log` quantos bytes leu. Ate o pedido 216 a conexao caia
//!   calada -- sem resposta e sem linha no log --, e so a bancada
//!   `bancada/seguranca/porta.py` (caso 4b) via isso, de fora do alcance dos
//!   testes.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::error::PhxError;
use phxsql_core::fio::TETO_DO_REGISTRO;
use phxsql_core::json::Json;
use phxsql_server::replica::Cliente;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";

/// Quanto se manda ALEM do teto.
///
/// Pequeno de proposito: o que se prova aqui e a fronteira, e cada MiB a mais
/// e memoria e tempo de todo mundo que roda a bateria. Um MiB sobra para
/// distinguir "passou do teto" de "bateu no teto".
const FOLGA: u64 = 1024 * 1024;

/// Uma resposta grande, mas DENTRO do teto -- o comportamento velho.
const GRANDE_MAS_CABE: usize = 1024 * 1024;

/// Prazo de toda leitura e escrita desta bateria.
///
/// Teste que pendura nao reprova ninguem: ele trava a bateria inteira. Ele e
/// POR CHAMADA do sistema, e nao pela troca inteira: mandar 129 MiB por um
/// soquete de volta local leva centesimos, e nenhuma escrita sozinha chega
/// perto disto.
const PRAZO: Duration = Duration::from_secs(15);

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn pasta(nome: &str) -> DirTemp {
    let d = DirTemp::novo(&format!("teto-resposta-{nome}"));
    std::fs::create_dir_all(d.join("base")).unwrap();
    d
}

// ---------------------------------------------------------------------------
// Lado REPLICA: o source manda uma resposta maior que o teto
// ---------------------------------------------------------------------------

/// O que o source de mentira responde a cada pedido.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    /// Uma resposta do tamanho de todo dia.
    Miuda,
    /// Uma resposta grande, e ainda assim dentro do teto.
    GrandeQueCabe,
    /// Uma linha so, maior que o teto.
    AcimaDoTeto,
}

/// Um "source" de mentira: aceita, le uma linha e responde o que mandarem.
///
/// Ele nao fala o protocolo inteiro de proposito -- o que se prova e a leitura
/// do fio, que acontece antes de qualquer campo ser olhado.
struct FonteFalsa {
    porta: u16,
    /// Quantas conexoes chegaram. E por ele que o teste sabe que a rodada
    /// seguinte abriu OUTRA conexao, em vez de reaproveitar a que ficou no
    /// meio de uma linha.
    conexoes: Arc<AtomicU64>,
}

impl FonteFalsa {
    fn subir(modo: Modo) -> FonteFalsa {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let conexoes = Arc::new(AtomicU64::new(0));
        let conta = Arc::clone(&conexoes);
        std::thread::spawn(move || {
            for fluxo in ouvinte.incoming() {
                let Ok(fluxo) = fluxo else { return };
                let conta = Arc::clone(&conta);
                std::thread::spawn(move || {
                    conta.fetch_add(1, Ordering::SeqCst);
                    // Prazo tambem do lado de ca: com o teto funcionando a
                    // replica vai embora no meio, e uma escrita sem prazo
                    // ficaria pendurada segurando a thread.
                    let _ = fluxo.set_write_timeout(Some(PRAZO));
                    let mut escrita = fluxo.try_clone().unwrap();
                    let mut leitor = BufReader::new(fluxo);
                    let mut pedido = String::new();
                    if leitor.read_line(&mut pedido).unwrap_or(0) == 0 {
                        return;
                    }
                    match modo {
                        Modo::Miuda => {
                            let _ = writeln!(
                                escrita,
                                r#"{{"ok":true,"resultado":{{"quem":"fonte-falsa"}}}}"#
                            );
                        }
                        Modo::GrandeQueCabe => {
                            let enchimento = "A".repeat(GRANDE_MAS_CABE);
                            let _ = writeln!(
                                escrita,
                                r#"{{"ok":true,"resultado":{{"quem":"fonte-falsa","enchimento":"{enchimento}"}}}}"#
                            );
                        }
                        Modo::AcimaDoTeto => despejar_acima_do_teto(&mut escrita),
                    }
                    let _ = escrita.flush();
                });
            }
        });
        FonteFalsa { porta, conexoes }
    }
}

/// Uma linha unica maior que o teto.
///
/// Nao e JSON valido de proposito: com o teto no lugar ninguem chega a
/// analisa-la, e com o defeito reposto a reprovacao sai na hora, em vez de
/// gastar o tempo da bateria analisando 129 MiB para reprovar do mesmo jeito.
fn despejar_acima_do_teto(escrita: &mut TcpStream) {
    let bloco = vec![b'A'; 64 * 1024];
    let total = TETO_DO_REGISTRO + FOLGA;
    let mut escritos = 0u64;
    while escritos < total {
        let n = bloco.len().min((total - escritos) as usize);
        if escrita.write_all(&bloco[..n]).is_err() {
            // A replica recusou e foi embora: o cano quebrado e o esperado.
            return;
        }
        escritos += n as u64;
    }
    let _ = escrita.write_all(b"\n");
}

/// A resposta do source maior que o teto e RECUSADA pelo nome, e a rodada
/// seguinte continua funcionando.
///
/// # O defeito que este teste trava
///
/// Sem teto, quem decide quanta memoria a replica reserva e o outro lado do
/// fio -- e o outro lado do fio pode ser um source enganado, um proxy quebrado
/// ou alguem sentado no meio. O teto existe desde o pedido 147; a prova, so
/// desde hoje.
#[test]
fn a_resposta_acima_do_teto_e_recusada_por_limite_e_a_rodada_seguinte_abre_outra() {
    let fonte = FonteFalsa::subir(Modo::AcimaDoTeto);

    let mut cliente = Cliente::conectar("127.0.0.1", fonte.porta, "", PRAZO).unwrap();
    let erro = match cliente.pedir(vec![("op", Json::texto_de("replicar"))]) {
        Err(e) => e,
        Ok(j) => panic!(
            "a resposta acima do teto foi ACEITA: {}",
            j.escrever().len()
        ),
    };
    assert!(
        matches!(erro, PhxError::LimiteExcedido(_)),
        "a recusa tem de ser LIMITE_EXCEDIDO, e veio {erro:?}"
    );
    // O numero DENTRO da recusa, como `docs/REPLICACAO.md` §18 promete: quem
    // opera compara o teto com o que mediu, e "mais de 128 MiB" nao se compara
    // com 134.217.728.
    assert!(
        erro.to_string().contains(&TETO_DO_REGISTRO.to_string()),
        "a recusa nao traz o teto em bytes: {erro}"
    );

    // A CONEXAO nao se reaproveita -- ela ficou no meio de uma linha --, e o
    // que precisa continuar de pe e a REPLICACAO: a rodada seguinte abre
    // outra. Uma replica que morre com a primeira resposta torta para para
    // sempre, e e isso que este pedaco impede.
    drop(cliente);
    let outra = FonteFalsa::subir(Modo::Miuda);
    let mut cliente = Cliente::conectar("127.0.0.1", outra.porta, "", PRAZO).unwrap();
    let r = cliente
        .pedir(vec![("op", Json::texto_de("posicao"))])
        .unwrap();
    assert_eq!(r.texto_ou("quem", ""), "fonte-falsa");
    assert_eq!(
        fonte.conexoes.load(Ordering::SeqCst),
        1,
        "a rodada recusada abriu mais de uma conexao no source do teto"
    );
}

/// O COMPORTAMENTO VELHO: resposta grande, e dentro do teto, atravessa como
/// sempre atravessou. Teto que recusa o que cabe nao e teto, e parede.
#[test]
fn a_resposta_grande_que_cabe_no_teto_atravessa_como_sempre() {
    let fonte = FonteFalsa::subir(Modo::GrandeQueCabe);
    let mut cliente = Cliente::conectar("127.0.0.1", fonte.porta, "", PRAZO).unwrap();
    let r = cliente
        .pedir(vec![("op", Json::texto_de("replicar"))])
        .unwrap();
    assert_eq!(r.texto_ou("quem", ""), "fonte-falsa");
    assert_eq!(r.texto_ou("enchimento", "").len(), GRANDE_MAS_CABE);
}

// ---------------------------------------------------------------------------
// Lado SERVIDOR: o cliente manda um pedido maior que o teto
// ---------------------------------------------------------------------------

fn subir_servidor(base: &std::path::Path, porta: u16) -> Arc<Servidor> {
    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: base.join("base"),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // O ESCAPE ESCRITO: desde 18/09/2026 a cifra do fio nasce exigida
    // (pedido 370, ordem do dono), e esta bateria conecta em claro porque o
    // que ela mede e o teto da resposta e do pedido. Sem esta
    // linha a recusa lida aqui seria a da cifra, e a prova passaria a medir
    // o portao errado.
    c.cifra_fio.exigir = false;
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
    panic!("o servidor nao subiu na porta {porta}");
}

fn abrir(porta: u16) -> (TcpStream, BufReader<TcpStream>) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
    fluxo.set_read_timeout(Some(PRAZO)).unwrap();
    fluxo.set_write_timeout(Some(PRAZO)).unwrap();
    (fluxo.try_clone().unwrap(), BufReader::new(fluxo))
}

/// O pedido maior que o teto e RESPONDIDO antes de a conexao fechar, e o
/// `acessos.log` diz quantos bytes o servidor leu.
///
/// # O defeito que este teste trava
///
/// Ate o pedido 216 a conexao caia calada: sem resposta, sem linha no log e
/// sem violacao. Memoria protegida, visibilidade nenhuma -- quem opera nao
/// tinha como saber que alguem tentou. A drenagem antes da resposta e o que
/// faz a resposta chegar: fechar um soquete com dado por ler manda RST, e o
/// RST joga fora a resposta que o cliente ainda nao leu.
#[test]
fn o_pedido_acima_do_teto_recebe_a_recusa_e_entra_no_log_com_o_tamanho() {
    let d = pasta("servidor");
    let porta = porta_livre();
    let _s = subir_servidor(&d, porta);
    let (mut escrita, mut leitor) = abrir(porta);

    // A linha nao e JSON valido, pela mesma razao do lado da replica: com o
    // teto no lugar ninguem chega a analisa-la -- a recusa acontece na LEITURA,
    // antes de qualquer campo ser olhado --, e com o defeito reposto a
    // reprovacao sai na hora dizendo o nome do erro que veio no lugar. Com um
    // `{"op":"ping",...}` de 129 MiB a primeira versao desta prova reprovava
    // por PRAZO, depois de 31 s analisando o que ia ser recusado do mesmo
    // jeito: reprovacao que nao nomeia a garantia quebrada nao ensina nada.
    // O pedido PLAUSIVEL de 128 MiB continua sendo exercido pela bancada
    // `bancada/seguranca/porta.py` (caso 4b), contra um servidor de verdade.
    let bloco = vec![b'A'; 64 * 1024];
    let enchimento = TETO_DO_REGISTRO + FOLGA;
    let mut escritos = 0u64;
    while escritos < enchimento {
        let n = bloco.len().min((enchimento - escritos) as usize);
        escrita.write_all(&bloco[..n]).unwrap();
        escritos += n as u64;
    }
    escrita.write_all(b"\n").unwrap();
    escrita.flush().unwrap();
    let mandados = TETO_DO_REGISTRO + FOLGA + 1;

    // 1. A RESPOSTA CHEGA. As tres saidas se distinguem de proposito, porque
    //    cada uma e um defeito diferente: EOF e a conexao morrendo calada (o
    //    pedido 216), e prazo estourado e o servidor PENDURADO -- medido em
    //    17/09/2026 com o `take` removido, quando a drenagem fica esperando
    //    uma quebra de linha que ja foi consumida e os dois lados esperam um
    //    do outro. Deixar o `unwrap` cru aqui reprovava com um
    //    "WouldBlock: Resource temporarily unavailable", que nao nomeia
    //    garantia nenhuma.
    let mut resposta = String::new();
    match leitor.read_line(&mut resposta) {
        Ok(0) => panic!("a conexao fechou SEM responder ao pedido acima do teto"),
        Ok(_) => {}
        Err(e) => panic!("o pedido acima do teto ficou {PRAZO:?} sem resposta: {e}"),
    }
    let j = Json::analisar(&resposta).unwrap();
    assert!(!j.booleano_ou("ok", true), "resposta: {resposta}");
    assert_eq!(j.texto_ou("nome", ""), "LIMITE_EXCEDIDO", "{resposta}");
    assert_eq!(j.inteiro_ou("codigo", 0), 3003, "{resposta}");
    assert!(
        j.texto_ou("erro", "")
            .contains(&TETO_DO_REGISTRO.to_string()),
        "a recusa nao traz o teto em bytes: {resposta}"
    );

    // 2. QUANTO foi lido, pela contabilidade do proprio servidor: o teto mais
    //    um que ele leu, mais o que drenou ate a quebra de linha. Tem de bater
    //    EXATAMENTE com o que se mandou -- e e esta a assercao que nao passa
    //    quando o teto some, porque ai a linha inteira vira um pedido comum e
    //    o log nem fala de "fio".
    let esperado = format!("lidos {mandados} bytes desta linha (teto {TETO_DO_REGISTRO})");
    let log = esperar_no_log(&d.join("acessos.log"), &esperado);
    assert!(
        log.contains(r#""op":"fio""#),
        "a linha do log nao e a do fio: {log}"
    );
}

/// O COMPORTAMENTO VELHO, no mesmo servidor: pedido de tamanho normal e
/// atendido como sempre foi.
#[test]
fn o_pedido_de_sempre_continua_sendo_atendido() {
    let d = pasta("velho");
    let porta = porta_livre();
    let _s = subir_servidor(&d, porta);
    let (mut escrita, mut leitor) = abrir(porta);
    writeln!(escrita, r#"{{"op":"ping","token":"{TOKEN}"}}"#).unwrap();
    escrita.flush().unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    let j = Json::analisar(&resposta).unwrap();
    assert!(j.booleano_ou("ok", false), "resposta: {resposta}");
}

/// Espera a linha aparecer no `acessos.log` -- por CONDICAO, e nao por tempo
/// fixo: o servidor responde ANTES de anotar, entao ler o arquivo na hora e
/// uma corrida perdida em maquina carregada.
fn esperar_no_log(caminho: &std::path::Path, pedaco: &str) -> String {
    let ate = Instant::now() + Duration::from_secs(10);
    let mut ultimo = String::new();
    while Instant::now() < ate {
        ultimo = std::fs::read_to_string(caminho).unwrap_or_default();
        if let Some(l) = ultimo.lines().find(|l| l.contains(pedaco)) {
            return l.to_string();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("o `acessos.log` nunca trouxe «{pedaco}»; o que ele tem:\n{ultimo}");
}
