//! Pedido 300 §2.7: a replica CONTA a filha que entra sem a mae -- provado
//! PELO SOQUETE, com um source e uma replica de verdade.
//!
//! A replica aplica e nao julga a chave estrangeira (`Table::julga_integridade`,
//! com a medida: julgar la perdia 2 de 2 eventos). A replicacao anda por
//! TABELA, e a filha chega antes da mae -- entao o invariante petreo «so existe
//! filho se o pai existir primeiro» nao vale na replica no intervalo, e nao
//! valia NADA que dissesse isso. Os tres maduros convergem em contar a
//! divergencia do aplicador e deixa-la visivel: `orfas_na_replica` no
//! `replicacao_estado`.
//!
//! # Como se fabrica a filha antes da mae, sem sorte
//!
//! A replica alcanca as tabelas na ordem do `posicao` do source, que e a de
//! `Database::todas_as_tabelas` -- ORDENADA por nome. Uma filha chamada
//! `a_itens` passa antes da mae `clientes`; uma chamada `pedidos`, depois.
//! Se a ordem um dia deixar de ser por nome, a primeira prova cai dizendo que
//! nao contou, e a segunda continua valendo -- nenhuma passa por engano.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "orfas-na-replica";
const ESPERA: Duration = Duration::from_secs(20);
const LINHAS: i64 = 3;

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

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = tentar(porta, corpo).unwrap_or_else(|| panic!("{corpo}: sem resposta de {porta}"));
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

fn contar(porta: u16, tabela: &str) -> usize {
    tentar(
        porta,
        &format!(r#""op":"varrer","database":"loja","tabela":"{tabela}","max":100"#),
    )
    .and_then(|r| r.campo("resultado").cloned())
    .and_then(|r| r.campo("linhas").and_then(Json::lista).map(<[Json]>::len))
    .unwrap_or(0)
}

/// O source com a mae `clientes` e a filha `filha`, as duas com [`LINHAS`]
/// linhas -- a filha apontando para mae que EXISTE (o source julga).
fn source_com(porta: u16, filha: &str) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
           "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    exigir(
        porta,
        &format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{filha}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"cliente","tipo":"Int8"}}],
               "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}},
                          {{"nome":"por_cliente","colunas":["cliente"]}}],
               "chaves_estrangeiras":[{{"nome":"fk_cliente","colunas":["cliente"],
                                       "tabela_ref":"clientes","colunas_ref":["id"]}}]"#
        ),
    );
    for i in 1..=LINHAS {
        exigir(
            porta,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{i}}}"#
            ),
        );
        exigir(
            porta,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"{filha}",
                   "linha":{{"id":{i},"cliente":{i}}}"#
            ),
        );
    }
}

/// O par: o source ja com tudo escrito, e a replica subindo DEPOIS -- a
/// primeira rodada dela alcanca as duas tabelas, na ordem do source.
fn par(nome: &str, filha: &str) -> (NoAr, NoAr, DirTemp, DirTemp) {
    par_em(nome, filha, false)
}

/// [`par`] no modo BIDIRECIONAL: os dois `multi`, e quem puxa grava pela
/// chave (`inserir_replicado`) em vez de pelo rowid -- o caminho irmao.
fn par_em(nome: &str, filha: &str, bidi: bool) -> (NoAr, NoAr, DirTemp, DirTemp) {
    let base_s = DirTemp::novo(&format!("orfas-source-{nome}"));
    let base_r = DirTemp::novo(&format!("orfas-replica-{nome}"));
    let (papel_s, papel_r) = if bidi {
        (Papel::Multi, Papel::Multi)
    } else {
        (Papel::Source, Papel::Replica)
    };
    let s = subir(config(&base_s.0, "fonte", papel_s));
    source_com(s.porta, filha);
    let mut cr = config(&base_r.0, "copia", papel_r);
    cr.somente_leitura = !bidi;
    cr.replicacao.origens = vec![Origem {
        nome: "fonte".into(),
        host: "127.0.0.1".into(),
        porta: s.porta,
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
    let r = subir(cr);
    let ate = Instant::now() + ESPERA;
    while contar(r.porta, filha) < LINHAS as usize || contar(r.porta, "clientes") < LINHAS as usize
    {
        assert!(
            Instant::now() < ate,
            "a replica nao alcancou as duas tabelas em {} s",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    (s, r, base_s, base_r)
}

fn orfas(porta: u16) -> Json {
    exigir(porta, r#""op":"replicacao_estado""#)
        .campo("orfas_na_replica")
        .cloned()
        .unwrap_or(Json::Nulo)
}

/// **A prova real.** `a_itens` chega antes de `clientes`: as tres filhas
/// entram sem mae (a replica nao recusa -- recusar perderia as tres) e as tres
/// sao CONTADAS. A mae chega depois e o dado converge; o numero fica, porque
/// conta o que ACONTECEU, como o `apply_error_count` do PostgreSQL.
///
/// **Defeito reposto** (o ramo da replica em `Table::conferir_as_maes` sem o
/// veredito -- `pendente` sempre `None`, que e o silencio de antes): o campo
/// vem vazio e o teste cai na asercao da contagem.
#[test]
fn a_filha_que_chega_antes_da_mae_e_contada() {
    let (_s, r, _bs, _br) = par("antes", "a_itens");
    let o = orfas(r.porta);
    let t = o
        .campo("loja/a_itens")
        .unwrap_or_else(|| panic!("as filhas entraram sem mae e nada contou: {}", o.escrever()));
    assert_eq!(t.inteiro_ou("orfas", -1), LINHAS, "{}", o.escrever());
    assert_eq!(t.inteiro_ou("sem_conferir", -1), 0, "{}", o.escrever());
    // E a replica NAO recusou: as tres filhas estao la.
    assert_eq!(contar(r.porta, "a_itens"), LINHAS as usize);
}

/// **O comportamento velho, e o falso positivo que ela nao pode ter.** A
/// filha `pedidos` chega DEPOIS da mae: cada linha encontra a sua, e o campo
/// fica vazio -- a contagem nao acusa replica sa.
#[test]
fn com_a_mae_primeiro_nada_e_contado() {
    let (_s, r, _bs, _br) = par("depois", "pedidos");
    let o = orfas(r.porta);
    assert!(
        o.campo("loja/pedidos").is_none(),
        "filha com mae contada como orfa: {}",
        o.escrever()
    );
}

/// **O irmao, no bidirecional.** O `inserir_replicado` tambem nao julga, e
/// passa pela MESMA conferencia que conta. Sem a contagem ligada no laco do
/// bidirecional (`contar_orfas` em `aplicar_lote_bidi`), a replica fiel conta
/// e este caminho fica calado -- e e esse o defeito que a prova repoe.
#[test]
fn no_bidirecional_a_filha_antes_da_mae_tambem_e_contada() {
    let (_s, r, _bs, _br) = par_em("bidi", "a_itens", true);
    let o = orfas(r.porta);
    let n = o
        .campo("loja/a_itens")
        .map(|t| t.inteiro_ou("orfas", -1))
        .unwrap_or(0);
    assert_eq!(
        n,
        LINHAS,
        "o bidirecional gravou filhas sem mae caladas: {}",
        o.escrever()
    );
}
