//! O que e do SERVIDOR nao se alcanca nomeando uma base -- pedido 756,
//! provado PELO SOQUETE.
//!
//! O portao geral le o `"database"` do pedido. Quem tinha `administrar` so na
//! `loja` mandava `"database":"loja"` e listava as sessoes de todas as bases
//! (login, IP, op) -- e, pela mesma porta, lia o cadastro de usuarios e o log
//! de acessos, e parava o servico. O que esta bateria trava:
//!
//! 1. quem administra uma base so nao lista as sessoes (`sessoes`,
//!    `processlist`) nem ve a conexao de outra base, com ou sem `"database"`;
//! 2. nenhuma op de `OPS_DO_SERVIDOR` passa para ele pelo `"database"`;
//! 3. o comportamento VELHO: o `root` e quem tem `administrar` no `"*"` (com
//!    nivel de leitor) continuam vendo todas as sessoes -- e o `lojista`
//!    continua administrando a PROPRIA base (`backups` da `loja`).
//!
//! Por que soquete e nao `despachar`: a conexao vitima tem de existir de
//! verdade, com login e IP, para o vazamento ser o vazamento.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::usuarios::OPS_DO_SERVIDOR;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-das-ops-do-servidor-756";
const SENHA: &str = "segredo-de-teste";

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
                {{ "id": 2, "login": "lojista", "nome": "Administra a Loja", "senha_hash": "{h}",
                   "nivel": "leitor", "ativo": true,
                   "bases": {{ "loja": {{ "ler": true, "administrar": true }} }} }},
                {{ "id": 3, "login": "curinga", "nome": "Administra no Curinga",
                   "senha_hash": "{h}", "nivel": "leitor", "ativo": true,
                   "bases": {{ "*": {{ "ler": true, "administrar": true }} }} }},
                {{ "id": 4, "login": "vitima", "nome": "Trabalha no RH",
                   "senha_hash": "{h}", "nivel": "leitor", "ativo": true,
                   "bases": {{ "rh": {{ "ler": true }} }} }} ],
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
        jobs = base.join("jobs.json").display().to_string(),
    );
    // O ESCAPE ESCRITO: a cifra do fio nasce exigida (pedido 370); esta
    // bateria conecta em claro porque o que ela mede e o portao das ops do servidor.
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

/// Recusa de PERMISSAO, e nao outro erro qualquer: uma op que falhasse por
/// pedido malformado (`desbloquear` sem `ip`) depois de passar o portao
/// contaria como recusa e esconderia o furo.
fn recusado_por_permissao(r: &Json) -> bool {
    !r.booleano_ou("ok", true) && r.inteiro_ou("codigo", 0) == 4001
}

/// A resposta traz a conexao do `vitima` (base `rh`)?
fn ve_a_vitima(r: &Json) -> bool {
    r.booleano_ou("ok", false) && res(r).escrever().contains("\"vitima\"")
}

/// **A prova do 756.** Defeito reposto que derruba este teste: o portao geral
/// lendo o `"database"` do pedido tambem para `sessoes`/`processlist` (tirar
/// as duas de `OPS_DO_SERVIDOR`, ou o `match` do `e_do_servidor` no
/// `portoes_do_pedido`) -- o `lojista` volta a ver o `vitima`.
#[test]
fn quem_administra_uma_base_nao_lista_as_sessoes_das_outras() {
    let d = DirTemp::novo("sessoes-756");
    let (_s, porta) = subir(&d);
    let mut vitima = Ligacao::entrar(porta, "vitima");
    let _ = vitima.pedir(r#""op":"bancos","database":"rh""#);
    let mut lojista = Ligacao::entrar(porta, "lojista");
    for op in ["sessoes", "processlist"] {
        for db in ["", r#","database":"loja""#] {
            let r = lojista.pedir(&format!(r#""op":"{op}"{db}"#));
            assert!(
                !ve_a_vitima(&r),
                "{op}{db}: quem administra so a loja viu a conexao do rh: {}",
                r.escrever()
            );
            assert!(
                recusado_por_permissao(&r),
                "{op}{db}: devia recusar por permissao, veio {}",
                erro(&r)
            );
        }
    }
}

/// Toda op da lista, e nao so as duas do pedido: o furo era do CAMPO, e o
/// campo e o mesmo em todas. Com o defeito reposto (o `"database"` lido para
/// todas) a lista dos que passaram sai no recado -- medido em 09/10/2026,
/// entre elas `usuarios`, `acessos`, `config` e `servico_parar`.
#[test]
fn nenhuma_op_do_servidor_passa_pelo_database_de_uma_base() {
    let d = DirTemp::novo("ops-756");
    let (_s, porta) = subir(&d);
    let mut lojista = Ligacao::entrar(porta, "lojista");
    let mut passaram = Vec::new();
    for op in OPS_DO_SERVIDOR {
        let r = lojista.pedir(&format!(r#""op":"{op}","database":"loja""#));
        if !recusado_por_permissao(&r) {
            passaram.push(format!("{op}: {}", erro(&r)));
        }
    }
    assert!(
        passaram.is_empty(),
        "{} de {} ops do servidor passaram para quem administra so a loja:\n{}",
        passaram.len(),
        OPS_DO_SERVIDOR.len(),
        passaram.join("\n")
    );
}

/// O comportamento VELHO, que a guarda nova nao pode tirar: o administrador
/// do servidor e o `"*"` com nivel de leitor continuam vendo tudo (o mesmo
/// criterio do `encerrar_sessao`, pedido 755 -- e nao `e_admin`); e o
/// `lojista` continua administrando a propria base.
#[test]
fn o_servidor_e_o_curinga_continuam_vendo_tudo_e_a_base_continua_da_base() {
    let d = DirTemp::novo("velho-756");
    let (_s, porta) = subir(&d);
    let mut vitima = Ligacao::entrar(porta, "vitima");
    let _ = vitima.pedir(r#""op":"bancos","database":"rh""#);
    for quem in ["root", "curinga"] {
        let mut c = Ligacao::entrar(porta, quem);
        for op in ["sessoes", "processlist"] {
            for db in ["", r#","database":"loja""#] {
                let r = c.pedir(&format!(r#""op":"{op}"{db}"#));
                assert!(
                    ve_a_vitima(&r),
                    "{quem} {op}{db} deixou de ver a conexao do rh: {}",
                    r.escrever()
                );
            }
        }
    }
    let mut lojista = Ligacao::entrar(porta, "lojista");
    let r = lojista.pedir(r#""op":"backups","database":"loja""#);
    assert!(
        r.booleano_ou("ok", false),
        "o lojista perdeu a administracao da propria base: {}",
        erro(&r)
    );
}
