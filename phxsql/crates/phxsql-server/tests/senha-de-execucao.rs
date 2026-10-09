//! Pedido 767, fatia P14: a SENHA DE EXECUCAO pelo soquete e pela web, de
//! verdade, com a varredura dos BYTES no fim.
//!
//! O roteiro inteiro corre contra um servidor no ar -- porta de dados, porta
//! web, Profiler gravando em arquivo, `acessos.log`, `diretivas.log`,
//! `ocorrencias.log` --, e no fim cada arquivo do diretorio e cada resposta
//! recebida sao varridos atras dos bytes das senhas. E o molde das guardas da
//! casa: a senha nao aparece porque NAO ESTA la, e nao porque um campo foi
//! conferido.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "token-da-prova-da-senha-de-execucao";
/// As senhas tem letras que nao aparecem em nenhum outro texto do servidor:
/// achar os bytes delas em qualquer arquivo e achar a senha.
const LOGIN: &str = "LoginDaAna-q7w3";
const EXECUCAO: &str = "ExecucaoDaAna-z9k2";
const ERRADA: &str = "ErradaDaAna-m4x8";
const NOVA: &str = "NovaExecucao-p2v6";

fn subir(base: &Path) -> Arc<Servidor> {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let bar = |p: PathBuf| p.display().to_string().replace('\\', "/");
    let h = phxsql_core::senha::cifrar_com(LOGIN, 1);
    let caminho = base.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0", "base": "{}", "token": "{TOKEN}",
              "log_acessos": "{}", "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}", "jobs": "{}",
              "cifra_fio": {{ "exigir": false }},
              "usuarios": [ {{ "id": 2, "login": "ana", "nome": "Ana",
                               "senha_hash": "{h}", "ativo": true, "supervisor": true }} ],
              "web": {{ "ligado": true, "bind": "127.0.0.1:0" }}
            }}"#,
            bar(base.join("base")),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    s
}

/// Uma CONEXAO da porta de dados: a sessao e dela.
struct Conexao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Conexao {
    fn nova(porta: u16) -> Conexao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        Conexao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn pedir(&mut self, corpo: &str, respostas: &mut Vec<String>) -> Json {
        writeln!(self.escrita, r#"{{"token":"{TOKEN}",{corpo}}}"#).unwrap();
        let mut linha = String::new();
        self.leitor.read_line(&mut linha).unwrap();
        respostas.push(linha.clone());
        Json::analisar(&linha).unwrap_or_else(|e| panic!("nao e JSON ({e}): {linha}"))
    }

    fn entrar(porta: u16, respostas: &mut Vec<String>) -> Conexao {
        let mut c = Conexao::nova(porta);
        let r = c.pedir(
            &format!(r#""op":"login","usuario":"ana","senha":"{LOGIN}""#),
            respostas,
        );
        assert!(r.booleano_ou("ok", false), "{}", r.escrever());
        c
    }
}

/// `POST /api` com o `X-Sessao`; devolve o JSON.
fn api(porta: u16, sessao: &str, corpo: &str, respostas: &mut Vec<String>) -> Json {
    let mut c = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    let corpo = format!(r#"{{"token":"{TOKEN}",{corpo}}}"#);
    let cab = if sessao.is_empty() {
        String::new()
    } else {
        format!("X-Sessao: {sessao}\r\n")
    };
    write!(
        c,
        "POST /api HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\n{cab}\
         Content-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    )
    .unwrap();
    let mut v = Vec::new();
    let _ = c.read_to_end(&mut v);
    let r = String::from_utf8_lossy(&v).into_owned();
    respostas.push(r.clone());
    let corpo = r.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("");
    Json::analisar(corpo).unwrap_or_else(|e| panic!("nao e JSON ({e}): {r}"))
}

fn ok(r: &Json, onde: &str) {
    assert!(r.booleano_ou("ok", false), "{onde}: {}", r.escrever());
}

fn recusou(r: &Json, codigo: i64, onde: &str) {
    assert!(
        !r.booleano_ou("ok", true),
        "{onde}: passou -- {}",
        r.escrever()
    );
    assert_eq!(
        r.inteiro_ou("codigo", 0),
        codigo,
        "{onde}: {}",
        r.escrever()
    );
}

const CRIAR: &str =
    r#""op":"criar_tabela","database":"b","tabela":"c","colunas":[{"nome":"id","tipo":"Int4"}]"#;
const DROP: &str = r#""op":"excluir_tabela","database":"b","tabela":"c","confirmar":"c""#;

/// Todo arquivo debaixo de `dir`, recursivo.
fn arquivos(dir: &Path, saida: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            arquivos(&p, saida);
        } else {
            saida.push(p);
        }
    }
}

#[test]
fn a_senha_de_execucao_libera_a_sessao_e_nao_aparece_em_lugar_nenhum() {
    let d = DirTemp::novo("senha-de-execucao");
    let s = subir(&d.0);
    let dados = comum::porta_real(|| s.porta_dos_dados());
    let web = comum::porta_real(|| s.porta_web());
    let mut resp = Vec::new();

    let mut c1 = Conexao::entrar(dados, &mut resp);
    let perfil = d.0.join("perfil.txt");
    let r = c1.pedir(
        &format!(
            r#""op":"profiler_ligar","guardar":500,"arquivo":"{}""#,
            perfil.display().to_string().replace('\\', "/")
        ),
        &mut resp,
    );
    ok(&r, "profiler_ligar");
    ok(
        &c1.pedir(r#""op":"criar_database","database":"b""#, &mut resp),
        "criar_database",
    );
    ok(&c1.pedir(CRIAR, &mut resp), "criar_tabela");
    recusou(&c1.pedir(DROP, &mut resp), 4009, "DROP sem liberar");

    // Sem senha cadastrada, nada libera.
    let r = c1.pedir(
        &format!(r#""op":"liberar_execucao","senha":"{EXECUCAO}""#),
        &mut resp,
    );
    recusou(&r, 4001, "liberar sem cadastro");

    // O primeiro cadastro: sem a senha de login, recusa; igual a de login,
    // recusa; com a de login e diferente dela, cadastra.
    let r = c1.pedir(
        &format!(
            r#""op":"senha_execucao_definir","senha":"{ERRADA}","nova_senha_execucao":"{EXECUCAO}""#
        ),
        &mut resp,
    );
    recusou(&r, 4001, "cadastro com a de login errada");
    let r = c1.pedir(
        &format!(
            r#""op":"senha_execucao_definir","senha":"{LOGIN}","nova_senha_execucao":"{LOGIN}""#
        ),
        &mut resp,
    );
    recusou(&r, 2001, "cadastro igual a de login");
    let r = c1.pedir(
        &format!(
            r#""op":"senha_execucao_definir","senha":"{LOGIN}","nova_senha_execucao":"{EXECUCAO}""#
        ),
        &mut resp,
    );
    ok(&r, "cadastro");

    // RED: a errada nao libera; a de LOGIN no lugar dela nao libera.
    let r = c1.pedir(
        &format!(r#""op":"liberar_execucao","senha":"{ERRADA}""#),
        &mut resp,
    );
    recusou(&r, 4001, "senha errada");
    recusou(&c1.pedir(DROP, &mut resp), 4009, "DROP depois da errada");
    let r = c1.pedir(
        &format!(r#""op":"liberar_execucao","senha":"{LOGIN}""#),
        &mut resp,
    );
    recusou(&r, 4001, "senha de login no lugar");
    recusou(&c1.pedir(DROP, &mut resp), 4009, "DROP depois da de login");

    // A certa, pelo SQL: libera ESTA sessao.
    let r = c1.pedir(
        &format!(r#""op":"sql","texto":"UNLOCK EXECUTION IDENTIFIED BY '{EXECUCAO}'""#),
        &mut resp,
    );
    ok(&r, "UNLOCK EXECUTION");

    // RED: outra sessao da mesma pessoa nao herda.
    let mut c2 = Conexao::entrar(dados, &mut resp);
    recusou(&c2.pedir(DROP, &mut resp), 4009, "DROP noutra sessao");

    ok(&c1.pedir(DROP, &mut resp), "DROP na sessao liberada");
    ok(&c1.pedir(CRIAR, &mut resp), "recriar");

    // RED: o trancar volta a recusar -- pelo SQL.
    ok(
        &c1.pedir(r#""op":"sql","texto":"LOCK EXECUTION""#, &mut resp),
        "LOCK",
    );
    recusou(&c1.pedir(DROP, &mut resp), 4009, "DROP depois do trancar");

    // A troca pede a de execucao ATUAL; a de login nao basta.
    let r = c2.pedir(
        &format!(
            r#""op":"senha_execucao_definir","senha_execucao":"{LOGIN}","nova_senha_execucao":"{NOVA}""#
        ),
        &mut resp,
    );
    recusou(&r, 4001, "troca com a de login");
    let r = c2.pedir(
        &format!(
            r#""op":"senha_execucao_definir","senha_execucao":"{EXECUCAO}","nova_senha_execucao":"{NOVA}""#
        ),
        &mut resp,
    );
    ok(&r, "troca");

    // Pela WEB: o id da sessao GIRA ao liberar, e o velho morre.
    let r = api(
        web,
        "",
        &format!(r#""op":"login","usuario":"ana","senha":"{LOGIN}""#),
        &mut resp,
    );
    ok(&r, "login web");
    let id0 = r.texto_ou("sessao", "").to_string();
    assert!(!id0.is_empty(), "{}", r.escrever());
    recusou(
        &api(web, &id0, DROP, &mut resp),
        4009,
        "DROP web sem liberar",
    );
    let r = api(
        web,
        &id0,
        &format!(r#""op":"liberar_execucao","senha":"{NOVA}""#),
        &mut resp,
    );
    ok(&r, "liberar web");
    let id1 = r.texto_ou("sessao", "").to_string();
    assert!(
        !id1.is_empty() && id1 != id0,
        "o id nao girou: {id0} -> {id1}"
    );
    ok(&api(web, &id1, DROP, &mut resp), "DROP web liberado");
    let velho = api(web, &id0, CRIAR, &mut resp);
    assert!(
        !velho.booleano_ou("ok", true),
        "o id velho continuou valendo: {}",
        velho.escrever()
    );
    ok(&api(web, &id1, CRIAR, &mut resp), "recriar web");
    ok(
        &api(web, &id1, r#""op":"trancar_execucao""#, &mut resp),
        "trancar web",
    );
    recusou(
        &api(web, &id1, DROP, &mut resp),
        4009,
        "DROP web depois do trancar",
    );

    // O anel do Profiler tambem e resposta.
    ok(
        &c1.pedir(r#""op":"profiler","max":500"#, &mut resp),
        "profiler",
    );
    ok(
        &c1.pedir(r#""op":"profiler_desligar""#, &mut resp),
        "profiler_desligar",
    );
    drop(c1);
    drop(c2);
    // O carteiro grava as ocorrencias fora da trava; da a ele a vez.
    std::thread::sleep(Duration::from_millis(1500));

    // A VARREDURA. Nenhum arquivo do diretorio e nenhuma resposta carrega os
    // bytes de nenhuma das quatro senhas.
    let mut todos = Vec::new();
    arquivos(&d.0, &mut todos);
    assert!(todos.iter().any(|p| p.ends_with("perfil.txt")), "{todos:?}");
    assert!(
        todos.iter().any(|p| p.ends_with("acessos.log")),
        "{todos:?}"
    );
    assert!(
        todos.iter().any(|p| p.ends_with("senhas-de-execucao.json")),
        "{todos:?}"
    );
    let diretivas = std::fs::read_to_string(d.0.join("diretivas.log")).unwrap_or_default();
    assert!(
        diretivas.contains("protecao.liberou"),
        "sem a trilha do liberou: {diretivas}"
    );
    assert!(diretivas.contains("protecao.definiu"), "{diretivas}");
    // A falha virou ocorrencia, e o bloqueio tambem.
    let ocorrencias: String = todos
        .iter()
        .filter(|p| p.to_string_lossy().contains("ocorrencias"))
        .map(|p| std::fs::read_to_string(p).unwrap_or_default())
        .collect();
    assert!(
        ocorrencias.contains("senha_de_execucao_recusada"),
        "{ocorrencias}"
    );
    assert!(ocorrencias.contains("comando_bloqueado"), "{ocorrencias}");
    for senha in [LOGIN, EXECUCAO, ERRADA, NOVA] {
        for arquivo in &todos {
            // O config.json traz o HASH da de login, nunca ela.
            let bytes = std::fs::read(arquivo).unwrap_or_default();
            assert!(
                !bytes.windows(senha.len()).any(|j| j == senha.as_bytes()),
                "a senha {senha:?} esta em {}: {}",
                arquivo.display(),
                String::from_utf8_lossy(&bytes)
                    .lines()
                    .filter(|l| l.contains(senha))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        for r in &resp {
            assert!(
                !r.contains(senha),
                "a senha {senha:?} voltou numa resposta: {r}"
            );
        }
    }
}
