//! Pedido 299, R6: a replica DIZ em que transacao da origem ela esta.
//!
//! # O defeito
//!
//! Com a venda aplicada inteira (676) e a parada por transacao (F2), a
//! replica sabia em que transacao estava -- e nao dizia. O `replicacao_estado`
//! mostrava a posicao de cada tabela, e a pergunta «a ultima venda que entrou
//! aqui inteira e qual?» nao tinha resposta legivel. Os tres maduros a
//! respondem: `pg_replication_origin_status.remote_lsn`, o `gtid_slave_pos`
//! do MariaDB e o `gtid_executed` do MySQL.
//!
//! # A prova
//!
//! Pelo soquete, contra dois `Servidor` de verdade: a origem grava uma venda
//! de tres tabelas num `COMMIT` e um autocommit depois; o id de cada uma sai
//! do `replicar` da propria origem, que e o que viaja no fio. A replica tem de
//! dizer, em `ultima_transacao_inteira.loja.tx`, o id do autocommit -- o
//! ultimo. Com a anotacao tirada do caminho que aplica (o defeito reposto), o
//! campo fica vazio e a prova cai pelo prazo. O par bidirecional e o irmao
//! (o mesmo `Juntador`, outro aplicador).

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "ultima-transacao-na-replica";
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
        fluxo.set_nodelay(true).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn exigir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        let j = Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"));
        assert!(j.booleano_ou("ok", false), "{corpo} -> {r}");
        j
    }
}

/// A origem com uma venda de tres tabelas num `COMMIT` e um autocommit
/// depois -- devolve os ids das duas transacoes, lidos do `replicar` dela.
fn origem_com_duas_transacoes(porta: u16) -> (String, String) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"criar_database","database":"loja""#);
    for tabela in ["vendas", "itens", "pagamentos"] {
        b.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Int8"}}],
               "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}}]"#
        ));
    }
    b.exigir(r#""op":"begin","database":"loja""#);
    b.exigir(r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"id":1,"venda":1}"#);
    for i in 1..=3 {
        b.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"itens","linha":{{"id":{i},"venda":1}}"#
        ));
    }
    b.exigir(
        r#""op":"inserir","database":"loja","tabela":"pagamentos","linha":{"id":1,"venda":1}"#,
    );
    b.exigir(r#""op":"commit""#);
    b.exigir(r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"id":2,"venda":2}"#);
    let r = b.exigir(r#""op":"replicar","database":"loja","tabela":"vendas","desde":0,"max":10"#);
    let txs: Vec<String> = r
        .campo("resultado")
        .and_then(|r| r.campo("eventos"))
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|e| e.texto_ou("tx", "").to_string())
        .collect();
    assert_eq!(txs.len(), 2, "{}", r.escrever());
    assert_ne!(txs[0], txs[1]);
    (txs[0].clone(), txs[1].clone())
}

fn replica_por(base: &Path, porta: u16, bidi: bool) -> NoAr {
    let papel = if bidi { Papel::Multi } else { Papel::Replica };
    let mut c = config(base, "central", papel);
    c.somente_leitura = !bidi;
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
        pino_tls: String::new(),
        espelho: false,
    }];
    subir(c)
}

fn a_replica_diz_a_ultima_transacao(nome: &str, bidi: bool) {
    let base_o = DirTemp::novo(&format!("ultima-tx-origem-{nome}"));
    let base_c = DirTemp::novo(&format!("ultima-tx-central-{nome}"));
    let papel = if bidi { Papel::Multi } else { Papel::Source };
    let caixa = subir(config(&base_o.0, "caixa01", papel));
    let (_venda, ultima) = origem_com_duas_transacoes(caixa.porta);
    let central = replica_por(&base_c.0, caixa.porta, bidi);

    let ate = Instant::now() + ESPERA;
    loop {
        let estado = tentar(central.porta, r#""op":"replicacao_estado""#)
            .and_then(|r| r.campo("resultado").cloned())
            .unwrap_or(Json::Nulo);
        let origem = estado
            .campo("origens")
            .and_then(|o| o.campo("caixa01"))
            .cloned()
            .unwrap_or(Json::Nulo);
        let dita = origem
            .campo("ultima_transacao_inteira")
            .and_then(|u| u.campo("loja"))
            .map(|l| l.texto_ou("tx", "").to_string())
            .unwrap_or_default();
        if dita == ultima {
            // R5, o comportamento velho: sem regra de tabela, nada fica de
            // fora, e o mapa fica vazio.
            assert_eq!(
                origem.campo("tabelas_fora_do_escopo").map(Json::escrever),
                Some("{}".to_string()),
                "{}",
                origem.escrever()
            );
            break;
        }
        assert!(
            Instant::now() < ate,
            "a replica nao disse a ultima transacao da origem ({ultima}) em {} s: \
             disse {dita:?} -- {}",
            ESPERA.as_secs(),
            origem.escrever()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(central);
    drop(caixa);
}

/// **A replica fiel diz a ultima transacao aplicada inteira.**
#[test]
fn a_replica_fiel_diz_a_ultima_transacao_que_entrou_inteira() {
    a_replica_diz_a_ultima_transacao("fiel", false);
}

/// **O irmao: o par bidirecional diz a mesma coisa.**
#[test]
fn o_bidirecional_diz_a_ultima_transacao_que_entrou_inteira() {
    a_replica_diz_a_ultima_transacao("bidi", true);
}
