//! Pedido 676: a venda de varias tabelas chega a replica INTEIRA, ou nao
//! chega -- provado PELO SOQUETE, com o fio derrubado no meio do envio.
//!
//! # O defeito
//!
//! O diario e por tabela, e ate o 676 a replica alcancava uma tabela de cada
//! vez, lote a lote, cada lote sob a propria tomada da trava. Uma venda com
//! itens e pagamento -- um `COMMIT` so na origem -- aparecia no central tabela
//! por tabela: primeiro os itens, depois o pagamento, por fim a venda. E se o
//! fio caisse no meio, o central ficava com os itens SEM a venda ate a
//! conexao voltar, legivel por qualquer cliente dele e indistinguivel de um
//! estado completo (pedido 299).
//!
//! # Como o fio cai no lugar certo, sem sorte
//!
//! Entre a replica e a origem ha um repasse que conta os `replicar`. No
//! N-esimo (o teste escolhe), ele derruba as duas pontas e passa a recusar
//! conexao ate o teste mandar voltar. A replica alcanca as tabelas na ordem do `posicao` --
//! `itens`, `pagamentos`, `vendas`, por nome --, entao o segundo `replicar`
//! acontece DEPOIS de os itens terem chegado e ANTES de a venda chegar:
//!
//! - **3 itens**: o primeiro `replicar` traz os itens inteiros. A aplicacao
//!   por tabela (o defeito) os grava ali, e o fio cai pedindo `pagamentos`.
//! - **600 itens**: o primeiro `replicar` traz 500 (o lote). A aplicacao por
//!   lote (o defeito) grava 500 itens, e o fio cai pedindo o resto.
//!
//! - **600 itens, no quarto**: as tres tabelas ja tem algo na mao quando o
//!   fio cai, e o que segura a venda e o id de transacao (ver o teste).
//!
//! Em todos, o central parado com o fio caido tem de mostrar a venda inteira
//! ou nada -- e depois que o fio volta, inteira.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "venda-inteira-na-replica";
const ESPERA: Duration = Duration::from_secs(30);

struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let _ = tentar(self.porta, r#""op":"servico_parar""#);
    }
}

fn config(base: &Path, id: &str, papel: Papel) -> Config {
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
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = papel;
    c.replicacao.id_servidor = id.into();
    c.replicacao.imagem_da_linha = true;
    c
}

fn subir(c: Config) -> NoAr {
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr { _s: s, porta }
}

fn tentar(porta: u16, corpo: &str) -> Option<Json> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().ok()?;
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .ok()?;
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).ok()?;
    Json::analisar(&resposta).ok()
}

/// Uma conexao que FICA -- a transacao vive na sessao.
struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        // Seiscentos pedidos em fila: sem isto cada um paga a espera do
        // Nagle com o ACK atrasado (~40 ms), e a prova leva meio minuto.
        fluxo.set_nodelay(true).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn exigir(&mut self, corpo: &str) {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        let j = Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"));
        assert!(j.booleano_ou("ok", false), "{corpo} -> {r}");
    }
}

fn contar(porta: u16, tabela: &str) -> Option<usize> {
    tentar(
        porta,
        &format!(r#""op":"varrer","database":"loja","tabela":"{tabela}","max":5000"#),
    )
    .and_then(|r| r.campo("resultado").cloned())
    .and_then(|r| r.campo("linhas").and_then(Json::lista).map(<[Json]>::len))
}

/// A origem com UMA venda de `itens` itens e um pagamento, num `COMMIT` so.
fn origem_com_a_venda(porta: u16, itens: usize) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"criar_database","database":"loja""#);
    for tabela in ["vendas", "itens", "pagamentos"] {
        b.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Int8"}}]"#
        ));
    }
    b.exigir(r#""op":"begin","database":"loja""#);
    b.exigir(r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"id":1,"venda":1}"#);
    for i in 1..=itens {
        b.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"itens","linha":{{"id":{i},"venda":1}}"#
        ));
    }
    b.exigir(
        r#""op":"inserir","database":"loja","tabela":"pagamentos","linha":{"id":1,"venda":1}"#,
    );
    b.exigir(r#""op":"commit""#);
}

/// O repasse entre a replica e a origem, que derruba o fio no `corte`-esimo
/// `replicar` e recusa conexao ate `voltar`.
struct Repasse {
    porta: u16,
    cortou: Arc<AtomicBool>,
    caido: Arc<AtomicBool>,
}

impl Repasse {
    fn subir(origem: u16, corte: u64) -> Repasse {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let cortou = Arc::new(AtomicBool::new(false));
        let caido = Arc::new(AtomicBool::new(false));
        let pedidos = Arc::new(AtomicU64::new(0));
        let (c1, c2) = (Arc::clone(&cortou), Arc::clone(&caido));
        std::thread::spawn(move || {
            for cliente in ouvinte.incoming() {
                let Ok(cliente) = cliente else { return };
                if c2.load(Ordering::SeqCst) {
                    // Fio caido: a conexao nasce e morre na hora.
                    let _ = cliente.shutdown(Shutdown::Both);
                    continue;
                }
                let Ok(servidor) = TcpStream::connect(("127.0.0.1", origem)) else {
                    continue;
                };
                let (cortou, caido, pedidos) =
                    (Arc::clone(&c1), Arc::clone(&c2), Arc::clone(&pedidos));
                let mut de_volta_cli = cliente.try_clone().unwrap();
                let mut de_volta_srv = servidor.try_clone().unwrap();
                // Origem -> replica: bytes crus.
                std::thread::spawn(move || {
                    let mut buf = [0u8; 16 * 1024];
                    loop {
                        match de_volta_srv.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if de_volta_cli.write_all(&buf[..n]).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    let _ = de_volta_cli.shutdown(Shutdown::Both);
                });
                // Replica -> origem: linha a linha, contando os `replicar`.
                std::thread::spawn(move || {
                    let mut leitor = BufReader::new(cliente.try_clone().unwrap());
                    let mut ida = servidor;
                    loop {
                        let mut linha = String::new();
                        match leitor.read_line(&mut linha) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {}
                        }
                        if linha.contains("\"op\":\"replicar\"")
                            && pedidos.fetch_add(1, Ordering::SeqCst) + 1 == corte
                        {
                            // O FIO CAI: as duas pontas, no meio do envio.
                            caido.store(true, Ordering::SeqCst);
                            cortou.store(true, Ordering::SeqCst);
                            break;
                        }
                        if ida.write_all(linha.as_bytes()).is_err() {
                            break;
                        }
                    }
                    let _ = cliente.shutdown(Shutdown::Both);
                    let _ = ida.shutdown(Shutdown::Both);
                });
            }
        });
        Repasse {
            porta,
            cortou,
            caido,
        }
    }

    fn voltar(&self) {
        self.caido.store(false, Ordering::SeqCst);
    }
}

fn replica_por(base: &Path, porta: u16) -> NoAr {
    let mut c = config(base, "central", Papel::Replica);
    c.somente_leitura = true;
    c.replicacao.origens = vec![Origem {
        nome: "caixa01".into(),
        host: "127.0.0.1".into(),
        porta,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
    }];
    subir(c)
}

/// O que o central mostra da venda: `(vendas, itens, pagamentos)`, com as
/// tabelas que ainda nao existem contando zero.
///
/// As tres contagens sao tres pedidos, e a replica pode aplicar o grupo
/// ENTRE eles: medido, `(0, 600, 1)` -- `vendas` lida antes da tomada e as
/// outras depois, um estado que nunca existiu (a mae entra primeiro). Por isso
/// o sanduiche: `vendas` de novo no fim, e o retrato so vale se ela nao mudou.
/// A aplicacao e UMA tomada e sempre move `vendas`, entao as duas iguais dizem
/// que nada foi aplicado no meio; a venda pela metade de verdade (`vendas` = 1
/// com itens faltando) continua sendo vista, porque ela nao muda `vendas`.
fn retrato(porta: u16) -> (usize, usize, usize) {
    loop {
        let antes = contar(porta, "vendas").unwrap_or(0);
        let itens = contar(porta, "itens").unwrap_or(0);
        let pagamentos = contar(porta, "pagamentos").unwrap_or(0);
        if contar(porta, "vendas").unwrap_or(0) == antes {
            return (antes, itens, pagamentos);
        }
    }
}

fn a_venda_chega_inteira_ou_nao_chega(nome: &str, itens: usize, corte: u64) {
    let base_o = DirTemp::novo(&format!("venda-inteira-origem-{nome}"));
    let base_c = DirTemp::novo(&format!("venda-inteira-central-{nome}"));
    let caixa = subir(config(&base_o.0, "caixa01", Papel::Source));
    origem_com_a_venda(caixa.porta, itens);

    let repasse = Repasse::subir(caixa.porta, corte);
    let central = replica_por(&base_c.0, repasse.porta);

    let ate = Instant::now() + ESPERA;
    while !repasse.cortou.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < ate,
            "a replica nunca chegou ao segundo replicar"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // O fio esta caido, e o central esta parado no que aplicou ate a queda.
    // Olhado varias vezes: a replica tenta religar e o repasse recusa.
    for _ in 0..10 {
        let r = retrato(central.porta);
        assert!(
            r == (0, 0, 0) || r == (1, itens, 1),
            "com o fio caido no meio do envio, o central mostra a venda PELA \
             METADE: (vendas, itens, pagamentos) = {r:?}, de (1, {itens}, 1)"
        );
        std::thread::sleep(Duration::from_millis(150));
    }

    // O fio volta: a transacao e pedida de novo desde o comeco, e chega
    // inteira.
    repasse.voltar();
    let ate = Instant::now() + ESPERA;
    loop {
        let r = retrato(central.porta);
        if r == (1, itens, 1) {
            break;
        }
        assert!(
            r == (0, 0, 0),
            "o central mostrou a venda pela metade depois do fio voltar: {r:?}"
        );
        assert!(
            Instant::now() < ate,
            "a venda nao chegou ao central em {} s depois do fio voltar: {r:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(central);
    drop(caixa);
}

/// **A prova real do 676, entre tabelas.** Os tres itens chegam inteiros no
/// primeiro `replicar` e o fio cai pedindo o pagamento. Com a aplicacao por
/// tabela reposta, o central fica com `(0, 3, 0)`: itens sem venda.
#[test]
fn a_venda_de_varias_tabelas_nao_aparece_pela_metade_quando_o_fio_cai() {
    a_venda_chega_inteira_ou_nao_chega("tabelas", 3, 2);
}

/// **A prova real do 676, dentro da tabela.** Seiscentos itens nao cabem num
/// lote: o fio cai pedindo o resto deles. Com a aplicacao por lote reposta,
/// o central fica com `(0, 500, 0)`.
#[test]
fn a_venda_maior_que_um_lote_nao_aparece_pela_metade_quando_o_fio_cai() {
    a_venda_chega_inteira_ou_nao_chega("lotes", 600, 2);
}

/// **O id de transacao, e nao so a ordem de puxar.** O fio cai no QUARTO
/// `replicar` -- o segundo lote dos itens --, quando as tres tabelas ja
/// tem algo na mao. Sem o id (a tomada da trava sem a unidade do diario),
/// cada evento vira uma transacao de um evento so, a venda e os 500 itens
/// que chegaram se aplicam, e o central fica com `(1, 500, 0)`. Com a
/// aplicacao por lote reposta, `(0, 600, 1)`.
#[test]
fn a_venda_nao_aparece_pela_metade_mesmo_com_as_tres_tabelas_na_mao() {
    a_venda_chega_inteira_ou_nao_chega("id", 600, 4);
}
