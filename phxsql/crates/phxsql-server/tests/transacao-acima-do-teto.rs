//! **Pedido 685, decisao do dono (07/10/2026): a transacao acima do teto e
//! RECUSADA na origem, no COMMIT -- e a que cabe chega inteira na replica.**
//!
//! # O que existia
//!
//! Ate o 685, a transacao que passava do `TETO_DA_TRANSACAO` na memoria da
//! replica ia EM PEDACOS, cada um sob uma tomada, contada em
//! `transacoes_em_pedacos`: uma garantia que nao valia para a carga grande.
//! O dono decidiu que «a venda chega inteira ou nao chega» vale sem excecao.
//!
//! # A prova, com um teto de teste
//!
//! Os 64 MiB de fabrica pediriam uma transacao de 64 MiB. O teto se injeta
//! (`definir_teto_da_transacao_para_teste`) e vale para a origem e a replica
//! do mesmo processo -- e a mesma constante dos dois lados. O tamanho `S` da
//! transacao sai da propria recusa (sonda com teto 1); depois:
//!
//! - teto `S - 1` (a transacao e o teto + 1): o COMMIT recusa, e nada foi
//!   gravado -- nem na origem, nem, depois, na replica;
//! - teto `S + 1` (a transacao e o teto - 1): o COMMIT grava, e a venda chega
//!   ao central INTEIRA, com `transacoes_em_pedacos` zero. Os 600 itens nao
//!   cabem num lote (500), entao a replica tem a transacao pela metade na mao
//!   e decide pelo teto: a conta da origem por baixo da dela partiria a venda.
//!
//! Um `#[test]` so neste arquivo, porque o teto injetado e do processo.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "transacao-acima-do-teto";
const ESPERA: Duration = Duration::from_secs(30);
const ITENS: usize = 600;

struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let mut l = Ligacao::nova(self.porta);
        let _ = l.pedir(r#""op":"servico_parar""#);
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

    fn pedir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }

    fn exigir(&mut self, corpo: &str) -> Json {
        let j = self.pedir(corpo);
        assert!(j.booleano_ou("ok", false), "{corpo} -> {}", j.escrever());
        j
    }
}

fn contar(porta: u16, tabela: &str) -> usize {
    let mut l = Ligacao::nova(porta);
    let r = l.pedir(&format!(
        r#""op":"varrer","database":"loja","tabela":"{tabela}","max":5000"#
    ));
    r.campo("resultado")
        .and_then(|r| r.campo("linhas"))
        .and_then(Json::lista)
        .map_or(0, <[Json]>::len)
}

/// A venda de `ITENS` itens, num COMMIT so. Devolve a resposta do COMMIT.
fn a_venda(l: &mut Ligacao, venda: usize) -> Json {
    l.exigir(r#""op":"begin","database":"loja""#);
    l.exigir(&format!(
        r#""op":"inserir","database":"loja","tabela":"vendas","linha":{{"id":{venda},"venda":{venda}}}"#
    ));
    for i in 1..=ITENS {
        let id = venda * 10_000 + i;
        l.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"itens","linha":{{"id":{id},"venda":{venda}}}"#
        ));
    }
    l.pedir(r#""op":"commit""#)
}

/// O primeiro numero do texto depois do `[SQLSTATE]`: o tamanho que a
/// recusa diz.
fn primeiro_numero(texto: &str) -> usize {
    let depois = texto.split_once(']').map_or(texto, |(_, d)| d);
    let digitos: String = depois
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digitos
        .parse()
        .unwrap_or_else(|_| panic!("a recusa nao diz o tamanho: {texto:?}"))
}

fn recusada(r: &Json) -> String {
    assert!(
        !r.booleano_ou("ok", true),
        "o COMMIT acima do teto foi ACEITO: {}",
        r.escrever()
    );
    assert!(
        r.escrever().contains("LIMITE_EXCEDIDO"),
        "a recusa nao e de limite: {}",
        r.escrever()
    );
    let texto = r.texto_ou("erro", "").to_string();
    assert!(
        texto.contains("divida a carga"),
        "a recusa nao diz o que fazer: {texto}"
    );
    texto
}

fn replica_de(base: &Path, porta: u16) -> NoAr {
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
        pino_tls: String::new(),
        espelho: false,
    }];
    subir(c)
}

#[test]
fn a_transacao_acima_do_teto_e_recusada_no_commit_e_a_que_cabe_chega_inteira() {
    let base_o = DirTemp::novo("teto-origem");
    let base_c = DirTemp::novo("teto-central");
    let caixa = subir(config(&base_o.0, "caixa01", Papel::Source));
    let mut l = Ligacao::nova(caixa.porta);
    l.exigir(r#""op":"criar_database","database":"loja""#);
    for tabela in ["vendas", "itens"] {
        l.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Int8"}}]"#
        ));
    }

    // A sonda: com teto 1, a recusa diz quanto a venda custa.
    phxsql_store::log::definir_teto_da_transacao_para_teste(1);
    let s = primeiro_numero(&recusada(&a_venda(&mut l, 1)));
    assert!(
        s > ITENS * phxsql_store::log::CUSTO_DO_EVENTO,
        "tamanho {s}"
    );

    // Teto + 1: recusada no COMMIT, e nada gravado.
    phxsql_store::log::definir_teto_da_transacao_para_teste(s - 1);
    let texto = recusada(&a_venda(&mut l, 2));
    assert_eq!(primeiro_numero(&texto), s, "a mesma venda mediu diferente");
    assert_eq!(
        (contar(caixa.porta, "vendas"), contar(caixa.porta, "itens")),
        (0, 0),
        "a transacao recusada no COMMIT deixou linha gravada na origem"
    );
    // A transacao terminou: um COMMIT novo nao acha transacao nenhuma.
    let r = l.pedir(r#""op":"commit""#);
    assert!(
        !r.booleano_ou("ok", true),
        "a recusa deixou a transacao viva"
    );

    // Teto - 1: grava, e chega inteira.
    phxsql_store::log::definir_teto_da_transacao_para_teste(s + 1);
    let r = a_venda(&mut l, 3);
    assert!(
        r.booleano_ou("ok", false),
        "a venda de {s} bytes com teto {} foi recusada: {}",
        s + 1,
        r.escrever()
    );
    drop(l);

    let central = replica_de(&base_c.0, caixa.porta);
    let ate = Instant::now() + ESPERA;
    loop {
        let r = (
            contar(central.porta, "vendas"),
            contar(central.porta, "itens"),
        );
        if r == (1, ITENS) {
            break;
        }
        assert!(
            Instant::now() < ate,
            "a venda nao chegou ao central em {} s: {r:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let estado = Ligacao::nova(central.porta).exigir(r#""op":"replicacao_estado""#);
    let em_pedacos = estado
        .campo("resultado")
        .unwrap_or(&estado)
        .campo("origens")
        .and_then(|o| o.campo("caixa01"))
        .and_then(|o| o.campo("transacoes_em_pedacos"))
        .and_then(Json::inteiro)
        .unwrap_or_else(|| panic!("sem transacoes_em_pedacos: {}", estado.escrever()));
    assert_eq!(
        em_pedacos, 0,
        "a transacao que a origem aceitou abaixo do teto chegou EM PEDACOS: a conta \
         da origem ficou abaixo da conta da replica"
    );

    phxsql_store::log::definir_teto_da_transacao_para_teste(0);
    drop(central);
    drop(caixa);
}
