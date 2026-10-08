//! Pedido 682: o PROCESSO da replica que morre no meio de um grupo nao deixa
//! a venda pela metade -- provado contra o sistema operacional, com `SIGKILL`.
//!
//! # O defeito
//!
//! O 676 aplica cada transacao da origem sob UMA tomada da trava: nenhum
//! leitor VIVO ve o meio dela. Mas o processo que morre no meio do grupo
//! deixa no disco parte da venda, e o arranque seguinte a servia assim ate a
//! rodada seguinte completar -- ou para sempre, com a origem fora do ar.
//!
//! # Como o processo morre no lugar certo, sem sorte
//!
//! O `phxsqld` de `debug` le `PHXSQL_TESTE_PARAR_NO_GRUPO=N`: depois do N-esimo
//! evento aplicado num grupo da replica ele avisa no erro padrao e para ali,
//! com a trava na mao. O teste le o aviso e mata o processo por fora
//! (`Child::kill` e `SIGKILL` no Unix) -- nada de `abort`, nada de panico: o
//! processo nao tem chance de arrumar nada.
//!
//! # O que se olha, e quando
//!
//! A reabertura acontece com a origem INALCANCAVEL (porta fechada): o que o
//! central mostra e so o que o arranque fez com o disco, sem rodada nenhuma
//! completando por tras. Com o defeito reposto (sem a marca do grupo), o
//! retrato e a venda pela metade. Depois, de volta a origem verdadeira, uma
//! segunda venda chega: o diario completado pelo arranque continua o da
//! origem, e a conferencia de continuidade nao rompe a tabela.
#![cfg(unix)]

mod comum;
use comum::{pedir, porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Papel, Servidor};

const TOKEN: &str = "venda-inteira-na-queda-da-replica";
const ITENS: usize = 5;
const ESPERA: Duration = Duration::from_secs(30);

/// A origem, aqui dentro: so ela escreve, e a replica e o processo de fora.
fn subir_origem(base: &Path) -> (Arc<Servidor>, u16) {
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
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "caixa01".into();
    c.replicacao.imagem_da_linha = true;
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    (s, porta)
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

    fn exigir(&mut self, corpo: &str) {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        let j = Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"));
        assert!(j.booleano_ou("ok", false), "{corpo} -> {r}");
    }
}

fn criar_as_tabelas(porta: u16) {
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
}

/// A venda `n`, com `ITENS` itens e um pagamento, num `COMMIT` so.
fn vender(porta: u16, n: usize) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"begin","database":"loja""#);
    b.exigir(&format!(
        r#""op":"inserir","database":"loja","tabela":"vendas","linha":{{"id":{n},"venda":{n}}}"#
    ));
    for i in 1..=ITENS {
        let id = (n - 1) * ITENS + i;
        b.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"itens","linha":{{"id":{id},"venda":{n}}}"#
        ));
    }
    b.exigir(&format!(
        r#""op":"inserir","database":"loja","tabela":"pagamentos","linha":{{"id":{n},"venda":{n}}}"#
    ));
    b.exigir(r#""op":"commit""#);
}

/// O `phxsqld` replica, puxando de `porta_origem`. `parar_em` liga o gancho
/// de `debug` que o para no meio do grupo.
fn subir_replica(dir: &Path, vez: u32, porta_origem: u16, parar_em: Option<u64>) -> (Filho, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "somente_leitura": true,
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }},
                "replicacao": {{
                    "papel": "replica", "id_servidor": "central",
                    "imagem_da_linha": true,
                    "origens": [{{ "nome": "caixa01", "host": "127.0.0.1",
                                   "porta": {porta_origem}, "token": "{TOKEN}",
                                   "databases": ["loja"], "reconectar_em": 1 }}]
                }}
            }}"#,
            base = dir.join("dados").display().to_string(),
            chave = dir.join("chave-do-fio.hex").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join(format!("stderr-{vez}.txt"));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_phxsqld"));
    cmd.arg("--config")
        .arg(&config)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()));
    match parar_em {
        Some(n) => cmd.env("PHXSQL_TESTE_PARAR_NO_GRUPO", n.to_string()),
        None => cmd.env_remove("PHXSQL_TESTE_PARAR_NO_GRUPO"),
    };
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    if parar_em.is_some() {
        let ate = Instant::now() + ESPERA;
        loop {
            let texto = std::fs::read_to_string(&erro_padrao).unwrap_or_default();
            if texto.contains("parado no meio do grupo da replica") {
                break;
            }
            assert!(
                Instant::now() < ate,
                "a replica nunca parou no meio do grupo: {texto}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    (filho, porta)
}

fn contar(porta: u16, tabela: &str) -> usize {
    let r = pedir(
        porta,
        &format!(
            r#"{{"token":"{TOKEN}","op":"varrer","database":"loja","tabela":"{tabela}","max":5000}}"#
        ),
    );
    Json::analisar(&r)
        .ok()
        .and_then(|j| j.campo("resultado").cloned())
        .and_then(|r| r.campo("linhas").and_then(Json::lista).map(<[Json]>::len))
        .unwrap_or(0)
}

/// `(vendas, itens, pagamentos)` no central, em SANDUICHE: `vendas` de novo
/// no fim, e o retrato so vale se ela nao mudou -- tres pedidos soltos dao
/// um estado que nunca existiu quando um grupo entra entre eles (ver
/// `venda-inteira-na-replica.rs`).
fn retrato(porta: u16) -> (usize, usize, usize) {
    loop {
        let antes = contar(porta, "vendas");
        let itens = contar(porta, "itens");
        let pagamentos = contar(porta, "pagamentos");
        if contar(porta, "vendas") == antes {
            return (antes, itens, pagamentos);
        }
    }
}

/// **A prova real do 682.** O processo da replica morre (`SIGKILL`) com 3 dos
/// 7 eventos da venda no disco, e reabre sem a origem: o arranque completa o
/// grupo pela marca, antes de a porta abrir. Com a marca do grupo removida
/// (o defeito reposto), o central reabre mostrando a venda pela metade.
#[test]
fn o_sigkill_no_meio_do_grupo_nao_deixa_a_venda_pela_metade() {
    let base_o = DirTemp::novo("queda-replica-origem");
    let base_c = DirTemp::novo("queda-replica-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_origem, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o);
    vender(porta_o, 1);

    // 1. A replica alcanca a venda e para no TERCEIRO evento do grupo.
    let (mut filho, _) = subir_replica(&base_c.0, 1, porta_o, Some(3));
    // 2. SIGKILL, por fora.
    filho.0.kill().unwrap();
    filho.0.wait().unwrap();
    drop(filho);

    // 3. Reabre com a origem inalcancavel: so o arranque mexe no disco.
    let (filho, porta_c) = subir_replica(&base_c.0, 2, comum::porta_fechada(), None);
    let r = retrato(porta_c);
    assert_eq!(
        r,
        (1, ITENS, 1),
        "depois do SIGKILL no meio do grupo, o central reabriu mostrando a venda \
         PELA METADE (vendas, itens, pagamentos) = {r:?}"
    );
    drop(filho);

    // 4. De volta a origem verdadeira: a segunda venda chega inteira, o que
    //    prova que o diario completado continua o da origem (a conferencia
    //    de continuidade rompe a tabela que nao continua).
    let (filho, porta_c) = subir_replica(&base_c.0, 3, porta_o, None);
    vender(porta_o, 2);
    let ate = Instant::now() + ESPERA;
    loop {
        let r = retrato(porta_c);
        if r == (2, 2 * ITENS, 2) {
            break;
        }
        assert!(
            r == (1, ITENS, 1),
            "a segunda venda apareceu pela metade, ou a primeira encolheu: {r:?}"
        );
        assert!(
            Instant::now() < ate,
            "a segunda venda nao chegou em {} s -- a continuidade rompeu? {r:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(filho);
}
