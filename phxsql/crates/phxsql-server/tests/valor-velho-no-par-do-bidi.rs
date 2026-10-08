//! Pedido 724 (a ponta a ponta do F1, pedido 709, no bidirecional): a marca do
//! `COMMIT` que so esperava a janela nao volta o valor velho no arranque -- e,
//! no bidirecional, nao o leva ao PAR como o mais novo. Provado contra o
//! sistema operacional, com `SIGKILL`.
//!
//! # O defeito, no caminho em que ele e pior
//!
//! O central faz `COMMIT` X=«a» com a janela de durabilidade parada e depois
//! uma escrita SOLTA X=«b»; o par (caixa01) recebe o «b». `SIGKILL` no
//! central antes do fecho da janela. O arranque reaplicava a marca sem
//! condicao: X voltava a «a» com um evento de carimbo do arranque, que vence
//! o «mais recente vence» -- e o caixa01 tambem voltava a «a».
//!
//! # A porta fixa
//!
//! O caixa01 puxa do central por uma origem ESTATICA: o central tem de voltar
//! na mesma porta. Ela sai de um ouvinte reservado e solto logo antes do
//! `spawn`; se um vizinho a tomar nesse meio, o central nao sobe e a prova
//! recomeca com outra porta -- nunca aceita a porta errada calada.
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "valor-velho-no-par";
const ESPERA: Duration = Duration::from_secs(30);

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

    fn exigir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        let r = Json::analisar(&r).unwrap();
        assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
        r
    }
}

fn tentar(porta: u16, corpo: &str) -> Option<Json> {
    let fluxo = TcpStream::connect(("127.0.0.1", porta)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").ok()?;
    let mut r = String::new();
    leitor.read_line(&mut r).ok()?;
    Json::analisar(&r).ok()
}

/// `(rowid daqui, nome)` do cliente 1, ou `None` enquanto ele nao chegou.
fn cliente_1(porta: u16) -> Option<(i64, String)> {
    let r = tentar(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    )?;
    r.campo("resultado")?
        .campo("linhas")?
        .lista()?
        .iter()
        .find(|l| l.inteiro_ou("id", -1) == 1)
        .map(|l| {
            (
                l.inteiro_ou("rowid", -1),
                l.texto_ou("nome", "").to_string(),
            )
        })
}

fn esperar_nome(porta: u16, quem: &str, nome: &str) {
    let ate = Instant::now() + ESPERA;
    loop {
        let visto = cliente_1(porta);
        if visto.as_ref().is_some_and(|(_, n)| n == nome) {
            return;
        }
        assert!(
            Instant::now() < ate,
            "{quem} nao mostrou {nome:?} em {} s: {visto:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn subir_caixa(base: &Path, porta_central: u16) -> (Arc<Servidor>, u16) {
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
    c.replicacao.papel = Papel::Multi;
    c.replicacao.id_servidor = "caixa01".into();
    c.replicacao.imagem_da_linha = true;
    c.replicacao.origens = vec![Origem {
        nome: "central".into(),
        host: "127.0.0.1".into(),
        porta: porta_central,
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
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    (s, porta)
}

/// O central, na porta FIXA `porta`, com a janela parada.
fn subir_central(dir: &Path, vez: u32, porta: u16, porta_caixa: u16) -> Option<Filho> {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:{porta}",
                "token": "{TOKEN}",
                "base": {base:?},
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }},
                "recursos": {{ "durabilidade": "por_lote", "lote_operacoes": 1000000,
                               "lote_milissegundos": 600000 }},
                "replicacao": {{
                    "papel": "multi", "id_servidor": "central",
                    "imagem_da_linha": true,
                    "origens": [{{ "nome": "caixa01", "host": "127.0.0.1",
                                   "porta": {porta_caixa}, "token": "{TOKEN}",
                                   "databases": ["loja"], "reconectar_em": 1 }}]
                }}
            }}"#,
            base = dir.join("dados").display().to_string(),
            chave = dir.join("chave-do-fio.hex").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join(format!("stderr-{vez}.txt"));
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(dir)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    match porta_do_phxsqld(&mut filho, &erro_padrao) {
        Ok(p) if p == porta => Some(filho),
        _ => None,
    }
}

fn marcas(dir: &Path) -> usize {
    std::fs::read_dir(dir.join("dados").join("loja"))
        .map(|ls| {
            ls.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("transacao_"))
                .count()
        })
        .unwrap_or(0)
}

/// Uma tentativa inteira com a porta `p` do central. `None` = a porta foi
/// tomada por um vizinho entre o soltar e o `bind`.
fn tentativa(p: u16) -> Option<(String, String)> {
    let base_x = DirTemp::novo("par-caixa");
    let base_c = DirTemp::novo("par-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_caixa, porta_x) = subir_caixa(&base_x.0, p);
    let mut x = Ligacao::nova(porta_x);
    x.exigir(r#""op":"criar_database","database":"loja""#);
    x.exigir(
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"nome","tipo":"Str(20)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    x.exigir(r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"0"}"#);

    let central = subir_central(&base_c.0, 1, p, porta_x)?;
    esperar_nome(p, "o central", "0");
    let (r, _) = cliente_1(p).unwrap();
    let alterar = |nome: &str| {
        format!(
            r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":{r},
               "valores":{{"id":1,"nome":"{nome}"}}"#
        )
    };
    let mut c = Ligacao::nova(p);
    c.exigir(r#""op":"begin","database":"loja""#);
    c.exigir(&alterar("a"));
    c.exigir(r#""op":"commit""#);
    assert_eq!(
        marcas(&base_c.0),
        1,
        "premissa: a marca do COMMIT nao ficou pendente"
    );
    c.exigir(&alterar("b"));
    esperar_nome(porta_x, "o caixa01", "b");
    assert_eq!(
        marcas(&base_c.0),
        1,
        "premissa: a marca saiu antes do SIGKILL"
    );
    drop(c);
    drop(central);

    let central = subir_central(&base_c.0, 2, p, porta_x)?;
    // Algumas rodadas de folga (uma por segundo, nos dois sentidos): o que o
    // arranque gravasse ja teria chegado ao par.
    std::thread::sleep(Duration::from_secs(4));
    let no_central = cliente_1(p).map(|(_, n)| n).unwrap_or_default();
    let no_caixa = cliente_1(porta_x).map(|(_, n)| n).unwrap_or_default();
    drop(central);
    Some((no_central, no_caixa))
}

/// **A ponta a ponta do F1 no bidirecional.** Vermelho medido com o conserto
/// do 709 reposto (a pergunta `a_linha_ja_passou` tirada): o central volta a
/// «a» no arranque, e o caixa01 recebe o «a» como o mais novo.
#[test]
fn a_marca_do_commit_nao_leva_o_valor_velho_ao_par() {
    for _ in 0..3 {
        let (ouvinte, p) = comum::ouvinte_reservado();
        drop(ouvinte);
        if let Some((central, caixa)) = tentativa(p) {
            assert_eq!(
                (central.as_str(), caixa.as_str()),
                ("b", "b"),
                "o arranque do central reaplicou o COMMIT por cima da solta, e o par \
                 recebeu o valor velho"
            );
            return;
        }
    }
    panic!("a porta do central foi tomada tres vezes seguidas");
}
