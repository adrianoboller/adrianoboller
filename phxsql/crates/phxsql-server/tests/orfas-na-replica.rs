//! Pedido 300 §2.7: a replica CONTA a filha que entra sem a mae -- provado
//! PELO SOQUETE, com um source e uma replica de verdade.
//!
//! A replica aplica e nao julga a chave estrangeira (`Table::julga_integridade`,
//! com a medida: julgar la perdia 2 de 2 eventos). Os tres maduros convergem
//! em contar a divergencia do aplicador e deixa-la visivel: `orfas_na_replica`
//! no `replicacao_estado`.
//!
//! # O que mudou com o pedido 676, e por que a fabrica mudou junto
//!
//! Ate o 676 a replicacao andava por TABELA, e a filha chegava antes da mae
//! pela ordem dos nomes (`a_itens` antes de `clientes`): era assim que estas
//! provas fabricavam a orfa. Desde o 676 a replica junta as tabelas pelo id
//! de transacao e aplica cada grupo sob UMA tomada, com a mae antes da filha
//! (`ordem_das_maes`) -- a filha que so chegava antes por causa do nome deixou
//! de ser orfa, e conta-la seria alarme falso. A orfa que sobra e a de
//! verdade: a mae que NAO chega. A fabrica agora e essa -- a `clientes` da
//! replica e de OUTRA historia (criada por conta, posta antes de ela subir),
//! entao a replica a recusa e as filhas entram sem mae nenhuma.

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

/// A mae, `clientes`.
const CLIENTES: &str = r#""op":"criar_tabela","database":"loja","tabela":"clientes",
   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
   "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#;

/// A mae de OUTRA forma: o mesmo nome, indice sem unicidade -- o
/// bidirecional a recusa, e a conferencia da filha continua tendo onde
/// procurar.
const CLIENTES_SEM_CHAVE: &str = r#""op":"criar_tabela","database":"loja","tabela":"clientes",
   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
   "indices":[{"nome":"pk_id","colunas":["id"]}]"#;

/// O source com a mae `clientes` e a filha `filha`, as duas com [`LINHAS`]
/// linhas -- a filha apontando para mae que EXISTE (o source julga).
fn source_com(porta: u16, filha: &str) {
    tabelas(porta, filha);
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

/// O database e as duas tabelas, sem linha nenhuma.
fn tabelas(porta: u16, filha: &str) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(porta, CLIENTES);
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
}

/// A replica de `porta`, ainda sem esperar nada.
fn replica_de(base: &Path, porta: u16, bidi: bool) -> NoAr {
    let papel = if bidi { Papel::Multi } else { Papel::Replica };
    let mut cr = config(base, "copia", papel);
    cr.somente_leitura = !bidi;
    cr.replicacao.origens = vec![Origem {
        nome: "fonte".into(),
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
    subir(cr)
}

/// Espera ate `pronto` ou estoura dizendo `o_que`.
fn esperar(o_que: &str, mut pronto: impl FnMut() -> bool) {
    let ate = Instant::now() + ESPERA;
    while !pronto() {
        assert!(
            Instant::now() < ate,
            "{o_que} nao aconteceu em {} s",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Copia uma arvore de diretorios.
fn copiar(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).unwrap();
    for e in std::fs::read_dir(de).unwrap() {
        let e = e.unwrap();
        let alvo = para.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copiar(&e.path(), &alvo);
        } else {
            std::fs::copy(e.path(), alvo).unwrap();
        }
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
    let papel_s = if bidi { Papel::Multi } else { Papel::Source };
    let s = subir(config(&base_s.0, "fonte", papel_s));
    source_com(s.porta, filha);
    let r = replica_de(&base_r.0, s.porta, bidi);
    esperar("a replica alcancar as duas tabelas", || {
        contar(r.porta, filha) >= LINHAS as usize && contar(r.porta, "clientes") >= LINHAS as usize
    });
    (s, r, base_s, base_r)
}

fn orfas(porta: u16) -> Json {
    exigir(porta, r#""op":"replicacao_estado""#)
        .campo("orfas_na_replica")
        .cloned()
        .unwrap_or(Json::Nulo)
}

/// **A prova real.** A `clientes` desta replica e de OUTRA historia: a
/// replica a recusa (a mae nunca chega), as tres filhas entram sem mae (a
/// replica nao recusa -- recusar perderia as tres) e as tres sao CONTADAS. O
/// numero fica, porque conta o que ACONTECEU, como o `apply_error_count` do
/// PostgreSQL.
///
/// **Defeito reposto** (o ramo da replica em `Table::conferir_as_maes` sem o
/// veredito -- `pendente` sempre `None`, que e o silencio de antes): o campo
/// vem vazio e o teste cai na asercao da contagem.
#[test]
fn a_filha_cuja_mae_nao_chega_e_contada() {
    // A mae de outra historia: criada por um servidor a parte, com o mesmo
    // esquema e outra linhagem, e posta na replica antes de ela subir.
    let base_x = DirTemp::novo("orfas-outra-historia");
    {
        let x = subir(config(&base_x.0, "outro", Papel::Source));
        exigir(x.porta, r#""op":"criar_database","database":"loja""#);
        exigir(x.porta, CLIENTES);
    }
    let base_s = DirTemp::novo("orfas-source-sem-mae");
    let base_r = DirTemp::novo("orfas-replica-sem-mae");
    copiar(&base_x.0.join("loja"), &base_r.0.join("loja"));
    let s = subir(config(&base_s.0, "fonte", Papel::Source));
    source_com(s.porta, "a_itens");
    let r = replica_de(&base_r.0, s.porta, false);
    esperar("a filha chegar", || {
        contar(r.porta, "a_itens") >= LINHAS as usize
    });
    assert_eq!(
        contar(r.porta, "clientes"),
        0,
        "a mae de outra historia recebeu linha"
    );
    let o = orfas(r.porta);
    let t = o
        .campo("loja/a_itens")
        .unwrap_or_else(|| panic!("as filhas entraram sem mae e nada contou: {}", o.escrever()));
    assert_eq!(t.inteiro_ou("orfas", -1), LINHAS, "{}", o.escrever());
    assert_eq!(t.inteiro_ou("sem_conferir", -1), 0, "{}", o.escrever());
    // E a replica NAO recusou: as tres filhas estao la.
    assert_eq!(contar(r.porta, "a_itens"), LINHAS as usize);
    drop(r);
    drop(s);
}

/// **Pedido 676: a mae do MESMO commit entra antes da filha.** Uma transacao
/// so grava `clientes` 1 e `a_itens` 1. O grupo vai sob uma tomada, mas tabela
/// por tabela -- e `a_itens` vem antes de `clientes` pelo nome. Sem a vez das
/// maes (`ordem_das_maes`), a filha seria aplicada primeiro e CONTADA como
/// orfa, sem nunca ter sido visivel sem a mae: alarme falso.
#[test]
fn a_mae_do_mesmo_commit_entra_antes_da_filha() {
    let base_s = DirTemp::novo("orfas-source-um-commit");
    let base_r = DirTemp::novo("orfas-replica-um-commit");
    let s = subir(config(&base_s.0, "fonte", Papel::Source));
    tabelas(s.porta, "a_itens");
    let mut b = Ligacao::nova(s.porta);
    b.exigir(r#""op":"begin","database":"loja""#);
    b.exigir(r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1}"#);
    b.exigir(r#""op":"inserir","database":"loja","tabela":"a_itens","linha":{"id":1,"cliente":1}"#);
    b.exigir(r#""op":"commit""#);
    let r = replica_de(&base_r.0, s.porta, false);
    esperar("o commit chegar", || {
        contar(r.porta, "a_itens") == 1 && contar(r.porta, "clientes") == 1
    });
    let o = orfas(r.porta);
    assert!(
        o.campo("loja/a_itens").is_none(),
        "a filha do mesmo commit foi aplicada antes da mae e contada: {}",
        o.escrever()
    );
}

/// Uma conexao que FICA -- a transacao vive na sessao.
struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
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
/// bidirecional (`contar_orfas` em `aplicar_tabela_bidi`), a replica fiel
/// conta e este caminho fica calado -- e e esse o defeito que a prova repoe.
///
/// A fabrica e a da replica fiel desde o 676, e pelo mesmo motivo: desde o
/// pedido 681 o bidirecional tambem junta as tabelas pelo id de transacao, e
/// a mae gravada antes entra antes -- a filha que so chegava primeiro pelo
/// NOME deixou de ser orfa. A orfa de verdade e a mae que NAO chega: a
/// `clientes` desta ponta foi criada por conta, SEM chave unica, e o
/// bidirecional a recusa (sem chave nao ha identidade entre servidores).
#[test]
fn no_bidirecional_a_filha_antes_da_mae_tambem_e_contada() {
    let base_x = DirTemp::novo("orfas-bidi-sem-chave");
    {
        let x = subir(config(&base_x.0, "outro", Papel::Source));
        exigir(x.porta, r#""op":"criar_database","database":"loja""#);
        exigir(x.porta, CLIENTES_SEM_CHAVE);
    }
    let base_s = DirTemp::novo("orfas-source-bidi");
    let base_r = DirTemp::novo("orfas-replica-bidi");
    copiar(&base_x.0.join("loja"), &base_r.0.join("loja"));
    let s = subir(config(&base_s.0, "fonte", Papel::Multi));
    source_com(s.porta, "a_itens");
    let r = replica_de(&base_r.0, s.porta, true);
    esperar("a filha chegar", || {
        contar(r.porta, "a_itens") >= LINHAS as usize
    });
    assert_eq!(
        contar(r.porta, "clientes"),
        0,
        "a mae sem chave recebeu linha pelo bidirecional"
    );
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
