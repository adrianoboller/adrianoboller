//! Ver nao e matar, e matar e pelo motor que ja existe -- pedido 707, fatia A7,
//! provado PELO SOQUETE.
//!
//! O aquario mostra tarefas (`aquario_retrato`, A5); encerrar uma e outra
//! coisa, e e o `telemetria_encerrar` de sempre, agora aceitando a tarefa do
//! retrato (`dados:17#42`). O que esta bateria trava:
//!
//! 1. quem administra encerra a tarefa VISTA pelo id do retrato, a conexao dona
//!    dela recebe o `CANCELADO`, e a trilha (`acessos.log`) diz o alvo -- op,
//!    base, tabela e quem encerrou; e o id velho do retrato nao mata o pedido
//!    seguinte da mesma conexao;
//! 2. quem so monitora o servidor nao encerra nada -- nem com `administrar`
//!    numa base so, mandando o `"database"` que o portao geral le;
//! 3. tarefa do servico (replicacao) e recusada, ate para o administrador;
//! 4. o comportamento VELHO: quem tem `administrar` no `"*"` continua
//!    derrubando conexao pelo `encerrar_sessao`.
//!
//! Por que soquete e nao `despachar`: a marca atravessa duas conexoes, e a
//! queda da conexao so se ve do lado de fora (a licao do `BULKINSERT`).

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-do-aquario-a7";
const SENHA: &str = "segredo-de-teste";
/// Linhas da tabela: a soma de verificacao tem de durar o bastante para caber
/// um retrato e um encerramento no meio -- a mesma medida do
/// `tests/telemetria.rs`.
const LINHAS: usize = 200_000;

fn subir(base: &std::path::Path) -> (Arc<Servidor>, u16) {
    // Uma iteracao so: a senha nao e o assunto, e 210.000 iteracoes por login
    // fariam a bateria levar segundos por nada.
    let h = phxsql_core::senha::cifrar_com(SENHA, 1);
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:0", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "jobs": {jobs:?}, "max_linhas": 500000,
              "root": {{ "login": "root", "senha_hash": "{h}" }},
              "usuarios": [
                {{ "id": 2, "login": "olha", "nome": "So Olha", "senha_hash": "{h}",
                   "nivel": "leitor", "ativo": true,
                   "bases": {{ "*": {{ "ler": true, "monitorar": true }},
                               "loja": {{ "ler": true, "administrar": true }} }} }},
                {{ "id": 3, "login": "curinga", "nome": "Administra no Curinga",
                   "senha_hash": "{h}", "nivel": "leitor", "ativo": true,
                   "bases": {{ "*": {{ "ler": true, "administrar": true }} }} }} ],
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
        jobs = base.join("jobs.json").display().to_string(),
    );
    // O ESCAPE ESCRITO: a cifra do fio nasce exigida (pedido 370); esta
    // bateria conecta em claro porque o que ela mede e o portao de ENCERRAR.
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return (s, porta);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu em {porta}");
}

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn entrar(porta: u16, login: &str) -> Ligacao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let f = TcpStream::connect_timeout(&alvo, Duration::from_secs(5)).unwrap();
        f.set_read_timeout(Some(Duration::from_secs(120))).unwrap();
        let mut c = Ligacao {
            escrita: f.try_clone().unwrap(),
            leitor: BufReader::new(f),
        };
        let r = c.pedir(&format!(
            r#""op":"login","usuario":"{login}","senha":"{SENHA}""#
        ));
        assert!(
            r.booleano_ou("ok", false),
            "login de {login}: {}",
            r.escrever()
        );
        c
    }

    /// Sem `unwrap`: escrever num soquete que o servidor ja fechou e o caso
    /// que um dos testes quer ver, e ele se le no `ler` (linha vazia).
    fn mandar(&mut self, corpo: &str) {
        let _ = writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}");
        let _ = self.escrita.flush();
    }

    /// A linha crua; vazia quando o servidor fechou a conexao.
    fn ler(&mut self) -> String {
        let mut r = String::new();
        match self.leitor.read_line(&mut r) {
            Ok(_) => r,
            Err(_) => String::new(),
        }
    }

    fn pedir(&mut self, corpo: &str) -> Json {
        self.mandar(corpo);
        let r = self.ler();
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }
}

fn res(j: &Json) -> &Json {
    j.campo("resultado").unwrap_or(j)
}

fn erro(j: &Json) -> String {
    format!("{} {}", j.texto_ou("nome", ""), j.texto_ou("erro", ""))
}

fn encher(porta: u16) {
    let mut c = Ligacao::entrar(porta, "root");
    let r = c.pedir(r#""op":"telemetria_ligar""#);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    let r = c.pedir(r#""op":"criar_database","database":"loja""#);
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    let r = c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"nome","tipo":"Str","tamanho":40},{"nome":"valor","tipo":"Int8"}]"#
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .as_str(),
    );
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    let mut feitas = 0;
    while feitas < LINHAS {
        let linhas: Vec<String> = (0..10_000)
            .map(|i| {
                format!(
                    "{{\"nome\":\"Cliente {}\",\"valor\":{}}}",
                    feitas + i,
                    i % 97
                )
            })
            .collect();
        let r = c.pedir(&format!(
            "\"op\":\"inserir_lote\",\"database\":\"loja\",\"tabela\":\"clientes\",\"linhas\":[{}]",
            linhas.join(",")
        ));
        assert!(r.booleano_ou("ok", false), "{}", erro(&r));
        feitas += 10_000;
    }
}

/// A tarefa do retrato que esta na operacao pedida -- e o objeto inteiro,
/// para quem quiser ler o `servico`. Espera ate 10 s ela aparecer.
fn tarefa_no_retrato(quem: &mut Ligacao, op: &str, nao: Option<&str>) -> Json {
    let ate = Instant::now() + Duration::from_secs(10);
    while Instant::now() < ate {
        let r = quem.pedir(r#""op":"aquario_retrato""#);
        assert!(r.booleano_ou("ok", false), "{}", erro(&r));
        if let Some(Json::Lista(tarefas)) = res(&r).campo("tarefas") {
            if let Some(t) = tarefas
                .iter()
                .find(|t| t.texto_ou("op", "") == op && Some(t.texto_ou("tarefa", "")) != nao)
            {
                return t.clone();
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("a tarefa {op} nao apareceu no aquario");
}

/// Uma soma de verificacao numa conexao propria; a resposta chega pelo canal.
fn soma_em_fundo(porta: u16) -> std::sync::mpsc::Receiver<String> {
    let (envia, recebe) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut c = Ligacao::entrar(porta, "root");
        for _ in 0..2 {
            c.mandar(r#""op":"checksum","database":"loja","tabela":"clientes""#);
            let r = c.ler();
            if envia.send(r).is_err() {
                return;
            }
        }
    });
    recebe
}

/// **A prova da A7.** O administrador ve a soma no aquario, encerra pela
/// tarefa do retrato, a conexao dela recebe o `CANCELADO`, e a trilha diz o
/// alvo. Depois, o MESMO id do retrato nao mata o pedido seguinte da conexao.
///
/// Defeitos repostos que derrubam este teste: o `telemetria_encerrar` sem a
/// forma `chave#serial` (cai no «nao ha atividade»); a trilha sem o alvo
/// (`database`/`tabela` vazios); e o encerrar mirando o serial CORRENTE em
/// vez do do retrato (a segunda soma morre em vez de terminar).
#[test]
fn o_admin_encerra_a_tarefa_do_aquario_e_a_trilha_diz_o_alvo() {
    let base = DirTemp::novo("a7-encerra");
    let (_s, porta) = subir(&base);
    encher(porta);

    let vitima = soma_em_fundo(porta);
    let mut adm = Ligacao::entrar(porta, "root");
    let t = tarefa_no_retrato(&mut adm, "checksum", None);
    let tarefa = t.texto_ou("tarefa", "").to_string();
    assert!(
        tarefa.contains('#'),
        "a tarefa nao veio no formato chave#serial: {tarefa}"
    );
    assert!(
        t.campo("servico").is_none(),
        "soma de cliente marcada como servico"
    );

    let r = adm.pedir(&format!(r#""op":"telemetria_encerrar","id":"{tarefa}""#));
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));
    assert_eq!(
        res(&r).texto_ou("estado", ""),
        "encerrando",
        "{}",
        r.escrever()
    );
    assert_eq!(res(&r).texto_ou("id", ""), tarefa);

    let resposta = vitima
        .recv_timeout(Duration::from_secs(60))
        .expect("a soma nunca respondeu");
    assert!(
        resposta.contains("\"ok\":false"),
        "a soma nao foi encerrada: {resposta}"
    );
    assert!(resposta.contains("\"nome\":\"CANCELADO\""), "{resposta}");
    assert!(
        resposta.contains("root"),
        "o cancelamento nao diz quem: {resposta}"
    );

    // A trilha: a linha do ato traz o ALVO, e nao a pergunta de quem encerrou.
    let log = std::fs::read_to_string(base.join("acessos.log")).unwrap();
    let linha = log
        .lines()
        .filter_map(|l| Json::analisar(l).ok())
        .find(|j| {
            j.texto_ou("op", "") == "telemetria_encerrar"
                && j.texto_ou("erro", "").contains(&tarefa)
        })
        .unwrap_or_else(|| panic!("o encerramento nao foi a trilha:\n{log}"));
    assert_eq!(
        linha.texto_ou("usuario", ""),
        "root",
        "{}",
        linha.escrever()
    );
    assert_eq!(
        linha.texto_ou("database", ""),
        "loja",
        "{}",
        linha.escrever()
    );
    assert_eq!(
        linha.texto_ou("tabela", ""),
        "clientes",
        "{}",
        linha.escrever()
    );
    assert!(
        linha.texto_ou("erro", "").contains("checksum"),
        "a trilha nao diz a operacao: {}",
        linha.escrever()
    );

    // O id VELHO do retrato, com a conexao ja na segunda soma: nada se mata.
    let segunda = tarefa_no_retrato(&mut adm, "checksum", Some(&tarefa));
    assert_ne!(segunda.texto_ou("tarefa", ""), tarefa);
    let r = adm.pedir(&format!(r#""op":"telemetria_encerrar","id":"{tarefa}""#));
    assert!(
        !r.booleano_ou("ok", true),
        "o id velho encerrou algo: {}",
        r.escrever()
    );
    assert!(erro(&r).contains("ja terminou"), "{}", erro(&r));
    let resposta = vitima
        .recv_timeout(Duration::from_secs(60))
        .expect("a segunda soma nunca respondeu");
    assert!(
        resposta.contains("\"ok\":true"),
        "o id velho do retrato matou o pedido seguinte da conexao: {resposta}"
    );
}

/// **Ver nao e matar.** `olha` tem `monitorar` no servidor e `administrar` so
/// na `loja`: ve o aquario, e nao encerra nada -- nem pela tarefa, nem pela
/// conexao, nem mandando o `"database":"loja"` que o portao geral le.
///
/// E o comportamento VELHO no mesmo teste: `curinga` (`administrar` no `"*"`,
/// nivel de leitor) continua derrubando a conexao, como sempre.
///
/// Defeito reposto: sem o portao proprio do `encerrar_sessao`, o pedido de
/// `olha` com `"database":"loja"` derruba a vitima e este teste cai na
/// primeira recusa. Com o portao trocado por `e_admin`, cai no fim, no
/// `curinga`.
#[test]
fn quem_so_monitora_nao_encerra_e_quem_administra_no_curinga_continua() {
    let base = DirTemp::novo("a7-monitorar");
    let (_s, porta) = subir(&base);
    let mut raiz = Ligacao::entrar(porta, "root");
    let r = raiz.pedir(r#""op":"telemetria_ligar""#);
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));

    // A vitima: uma conexao viva, que diz o proprio numero.
    let mut vitima = Ligacao::entrar(porta, "root");
    let t = vitima.pedir(r#""op":"telemetria","amostras":1"#);
    let eu = res(&t).texto_ou("voce", "").to_string();
    let numero = eu
        .strip_prefix("dados:")
        .expect("o campo `voce` nao veio")
        .to_string();

    let mut olha = Ligacao::entrar(porta, "olha");
    // Ve: o aquario responde.
    let r = olha.pedir(r#""op":"aquario_retrato""#);
    assert!(
        r.booleano_ou("ok", false),
        "monitorar nao ve o aquario: {}",
        erro(&r)
    );

    for corpo in [
        format!(r#""op":"telemetria_encerrar","id":"{eu}#1""#),
        format!(r#""op":"telemetria_encerrar","id":"{eu}#1","database":"loja""#),
        format!(r#""op":"telemetria_encerrar","id":"{eu}","database":"loja""#),
        format!(r#""op":"encerrar_sessao","id":{numero},"tipo":"conexao""#),
        format!(r#""op":"encerrar_sessao","id":{numero},"tipo":"conexao","database":"loja""#),
        format!(r#""op":"kill","id":{numero},"tipo":"conexao","database":"loja""#),
    ] {
        let r = olha.pedir(&corpo);
        assert!(
            !r.booleano_ou("ok", true),
            "quem so monitora encerrou: {corpo} -> {}",
            r.escrever()
        );
        assert!(
            erro(&r).starts_with("ACESSO_NEGADO"),
            "{corpo}: {}",
            erro(&r)
        );
    }
    let r = vitima.pedir(r#""op":"ping""#);
    assert!(
        r.booleano_ou("ok", false),
        "a vitima caiu: {}",
        r.escrever()
    );

    // O comportamento velho: administrar no `"*"` derruba, e a vitima ve o
    // soquete fechar.
    let mut curinga = Ligacao::entrar(porta, "curinga");
    let r = curinga.pedir(&format!(
        r#""op":"encerrar_sessao","id":{numero},"tipo":"conexao""#
    ));
    assert!(
        r.booleano_ou("ok", false),
        "o curinga perdeu o direito: {}",
        erro(&r)
    );
    vitima.mandar(r#""op":"ping""#);
    assert_eq!(
        vitima.ler(),
        "",
        "a conexao derrubada continuou respondendo"
    );
}

/// **Tarefa do servico nao se encerra, por ninguem.** Um `replicar` parado na
/// fila da trava (atras de uma soma longa) aparece no aquario marcado
/// `servico`, e o `telemetria_encerrar` do administrador -- pela tarefa e pelo
/// id velho -- e recusado com o motivo.
///
/// Defeito reposto: sem a recusa, o encerrar responde `ok` (a marca e posta na
/// replicacao) e este teste cai.
#[test]
fn tarefa_de_servico_e_recusada_ate_para_o_admin() {
    let base = DirTemp::novo("a7-servico");
    let (_s, porta) = subir(&base);
    encher(porta);

    let soma = soma_em_fundo(porta);
    let mut adm = Ligacao::entrar(porta, "root");
    tarefa_no_retrato(&mut adm, "checksum", None);

    // A replicacao chega por uma conexao comum, e espera a trava que a soma
    // segura.
    let (envia, recebe) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut c = Ligacao::entrar(porta, "root");
        c.mandar(r#""op":"replicar","database":"loja","tabela":"clientes","desde":0,"max":1"#);
        let _ = envia.send(c.ler());
    });
    let t = tarefa_no_retrato(&mut adm, "replicar", None);
    let tarefa = t.texto_ou("tarefa", "").to_string();
    assert_eq!(
        t.campo("servico").and_then(Json::booleano),
        Some(true),
        "o retrato nao marca a replicacao como servico: {}",
        t.escrever()
    );
    let chave = tarefa.split('#').next().unwrap().to_string();
    for id in [tarefa.as_str(), chave.as_str()] {
        let r = adm.pedir(&format!(r#""op":"telemetria_encerrar","id":"{id}""#));
        assert!(
            !r.booleano_ou("ok", true),
            "a replicacao foi marcada para encerrar ({id}): {}",
            r.escrever()
        );
        assert!(erro(&r).starts_with("ACESSO_NEGADO"), "{id}: {}", erro(&r));
        assert!(
            erro(&r).contains("replicacao"),
            "a recusa nao diz o motivo: {}",
            erro(&r)
        );
    }

    // A soma e a replicacao terminam por si; nada foi marcado.
    let r = soma
        .recv_timeout(Duration::from_secs(60))
        .expect("a soma nunca respondeu");
    assert!(r.contains("\"ok\":true"), "{r}");
    let r = recebe
        .recv_timeout(Duration::from_secs(60))
        .expect("a replicacao nunca respondeu");
    assert!(!r.contains("CANCELADO"), "a replicacao foi cancelada: {r}");
}
