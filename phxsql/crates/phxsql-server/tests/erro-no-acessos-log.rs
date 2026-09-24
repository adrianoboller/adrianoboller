//! O texto do erro no `acessos.log` -- pedido 497, provado PELO SOQUETE.
//!
//! # O defeito
//!
//! O `acessos.log` nunca gravou o corpo do pedido, e por isso foi declarado
//! «limpo» (`docs/SEGURANCA.md` §13.9). Mas ele grava o campo `erro`, que e o
//! `e.to_string()` da recusa -- e havia recusa que interpolava o texto do
//! pedido: `texto sem fechar na expressao: "nome = 'SEGREDO123"`, todo erro de
//! sintaxe da expressao (que citava a expressao INTEIRA) e todo erro de
//! sintaxe do SQL cujo simbolo ofensor era um literal (`e veio "'...'"`). Um
//! CPF no `WHERE`, ou uma senha num `CREATE USER` torto, ia em claro para o
//! disco.
//!
//! # Por que o conserto mora na origem, e o teste no soquete
//!
//! Texto de erro nao se analisa: ele chega ao `anotar` montado, em qualquer um
//! dos seis idiomas, com o valor em qualquer posicao da frase. So quem monta a
//! mensagem sabe o que e literal -- entao o literal sai redigido ali, e o
//! `acessos.log`, a resposta, o Profiler e o historico dos jobs recebem a
//! mesma mensagem ja sem ele. O teste e pelo soquete porque o que se afirma e
//! o que chega ao ARQUIVO pelas duas portas que anotam (a de dados e a web), e
//! nao o que uma funcao devolve.
//!
//! # O que o vermelho diz
//!
//! Cada caminho que vazou, pela porta e pelo lugar (resposta ou log), e nao
//! so o primeiro. E cada caso prova que PASSOU pelo caminho que diz passar
//! (o trecho esperado na recusa): um caso que recusasse por outro motivo --
//! permissao, tabela que falta -- passaria sem a marca por engano.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token da prova do erro no acessos.log";

/// O dado que nao pode chegar ao disco. Cabe no teto do `citar` (48 bytes)
/// de proposito: uma marca longa viraria tamanho por OUTRO motivo, e o teste
/// passaria sem provar a redacao.
///
/// A excecao sao os casos do P1, e ela e o ponto deles: la o valor E o
/// diagnostico (`nao entendi a duracao "5 segundos"`, pedido 453), o curto
/// continua citado de proposito, e o que se prova e o TETO -- entao a marca
/// vai dentro de um valor de 60 bytes, acima dele.
const MARCA: &str = "SEGREDO123";

/// Um caminho que recusa com texto do pedido.
struct Caso {
    caminho: &'static str,
    /// O corpo do pedido, sem o token.
    corpo: &'static str,
    /// Um trecho que so a recusa DAQUELE caminho tem -- ou vazio, quando o
    /// pedido tem de DAR CERTO (o roteiro com a senha num comentario roda).
    trecho: &'static str,
    /// O `perfil.txt` tambem tem de sair limpo? Quando sim, o trecho que SO
    /// a forma redigida tem: sem ele, uma linha que o Profiler nem redigiu
    /// passaria como limpa. So onde a senha viaja (as letras `PASSWORD`,
    /// `IDENTIFIED`): nos outros o Profiler mostra o SQL e o pedido por
    /// desenho -- a coluna «perfil» do parecer SEC do 497. No comentario, a
    /// forma redigida e o texto SEM ele.
    perfil: Option<&'static str>,
    /// Transacao so vale pela porta de dados: pela web o `BEGIN` recusa
    /// antes de ler o prazo, e o caso provaria outra recusa.
    pela_web: bool,
}

const CASOS: &[Caso] = &[
    Caso {
        caminho: "expressao, texto sem fechar",
        corpo: r#""op":"varrer","database":"loja","tabela":"t","expressao":"nome = 'SEGREDO123""#,
        trecho: "texto sem fechar",
        perfil: None,
        pela_web: true,
    },
    Caso {
        caminho: "expressao, erro de sintaxe",
        corpo: r#""op":"varrer","database":"loja","tabela":"t","expressao":"nome = 'SEGREDO123' AND AND n""#,
        trecho: "apareceu onde faltava um valor",
        perfil: None,
        pela_web: true,
    },
    Caso {
        caminho: "expressao, conta com texto",
        corpo: r#""op":"varrer","database":"loja","tabela":"t","expressao":"n + 'SEGREDO123' > 0""#,
        trecho: "precisa de numero",
        perfil: None,
        pela_web: true,
    },
    Caso {
        caminho: "SQL, literal sem fechar",
        corpo: r#""op":"sql","database":"loja","texto":"SELECT n FROM t WHERE nome = 'SEGREDO123""#,
        trecho: "nao fechado",
        perfil: None,
        pela_web: true,
    },
    Caso {
        caminho: "SQL, sobrou um literal",
        corpo: r#""op":"sql","database":"loja","texto":"SELECT n FROM t WHERE n = 1 'SEGREDO123'""#,
        trecho: "sobrou",
        perfil: None,
        pela_web: true,
    },
    Caso {
        caminho: "SQL, literal onde vinha outra coisa",
        corpo: r#""op":"sql","database":"loja","texto":"INSERT INTO t (n) VALUES 'SEGREDO123'""#,
        trecho: "esperava",
        perfil: None,
        pela_web: true,
    },
    Caso {
        caminho: "CREATE USER, senha mal fechada",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE USER c PASSWORD 'SEGREDO123""#,
        trecho: "nao fechado",
        perfil: Some("<comando invalido"),
        pela_web: true,
    },
    Caso {
        caminho: "CREATE USER, literal no lugar do login",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE USER 'SEGREDO123' PASSWORD 'x'""#,
        trecho: "login",
        perfil: Some("'***'"),
        pela_web: true,
    },
    // B1: a senha com aspas nao dobradas deixa um pedaco dela SOBRANDO.
    Caso {
        caminho: "B1, CREATE USER, pedaco de senha que sobra",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE USER c PASSWORD 'ab'SEGREDO123'cd'""#,
        trecho: "sobrou",
        perfil: Some("'***'"),
        pela_web: true,
    },
    // B2: a senha que nao e literal de aspas simples -- o costume do MySQL(R).
    Caso {
        caminho: "B2, CREATE USER, senha entre aspas duplas",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE USER c PASSWORD \"SEGREDO123\"""#,
        trecho: "aspas simples",
        perfil: Some("'***'"),
        pela_web: true,
    },
    Caso {
        caminho: "B2, ALTER USER, senha sem aspas",
        corpo: r#""op":"sql","database":"loja","texto":"ALTER USER c PASSWORD SEGREDO123""#,
        trecho: "aspas simples",
        perfil: Some("'***'"),
        pela_web: true,
    },
    // P1: o literal do SQL que vira campo do pedido montado, e o id do fio.
    Caso {
        caminho: "P1, BEGIN TRANSACTION TIMEOUT acima do teto",
        corpo: r#""op":"sql","database":"loja","texto":"BEGIN TRANSACTION TIMEOUT 'SEGREDO123xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'""#,
        trecho: "nao entendi a duracao",
        perfil: None,
        pela_web: false,
    },
    Caso {
        caminho: "P1, LOCK TIMEOUT acima do teto",
        corpo: r#""op":"sql","database":"loja","texto":"BEGIN LOCK TIMEOUT 'SEGREDO123xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'""#,
        trecho: "nao entendi a duracao",
        perfil: None,
        pela_web: false,
    },
    Caso {
        caminho: "P1, sessao web acima do teto",
        corpo: r#""op":"encerrar_sessao","id":"SEGREDO123xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx""#,
        trecho: "nao ha sessao web",
        perfil: None,
        pela_web: true,
    },
    // P2: aspas DUPLAS no lugar de valor -- o texto do MySQL(R) e do MariaDB.
    Caso {
        caminho: "P2, valor entre aspas duplas",
        corpo: r#""op":"sql","database":"loja","texto":"INSERT INTO t (n, nome) VALUES (2, \"SEGREDO123\")""#,
        trecho: "aspas simples",
        perfil: None,
        pela_web: true,
    },
    Caso {
        caminho: "P2, aspas duplas onde vinha palavra",
        corpo: r#""op":"sql","database":"loja","texto":"INSERT INTO t (n) VALUES \"SEGREDO123\"""#,
        trecho: "esperava",
        perfil: None,
        pela_web: true,
    },
    // Segunda volta do parecer SEC. (1) O portao lido por espaco: o
    // comentario escondia o cadastro. Agora SAO cadastro e chegam ao
    // `usuario_criar` -- que recusa porque o servidor ficaria sem supervisor.
    Caso {
        caminho: "2a volta, comentario de bloco antes do CREATE USER",
        corpo: r#""op":"sql","database":"loja","texto":"/* odbc */ CREATE USER c PASSWORD 'SEGREDO123'""#,
        trecho: "usuario_criar",
        perfil: Some("'***'"),
        pela_web: true,
    },
    Caso {
        caminho: "2a volta, comentario de linha antes do CREATE USER",
        corpo: r#""op":"sql","database":"loja","texto":"-- x\nCREATE USER c PASSWORD 'SEGREDO123'""#,
        trecho: "usuario_criar",
        perfil: Some("'***'"),
        pela_web: true,
    },
    Caso {
        caminho: "2a volta, comentario entre CREATE e USER",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE/**/USER c PASSWORD 'SEGREDO123'""#,
        trecho: "usuario_criar",
        perfil: Some("'***'"),
        pela_web: true,
    },
    // (2) A senha depois de `IDENTIFIED BY` -- MySQL(R) e MariaDB.
    Caso {
        caminho: "2a volta, CREATE USER IDENTIFIED BY entre aspas duplas",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE USER c IDENTIFIED BY \"SEGREDO123\"""#,
        trecho: "exige PASSWORD",
        perfil: Some("'***'"),
        pela_web: true,
    },
    Caso {
        caminho: "2a volta, CREATE USER IDENTIFIED BY sem aspas",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE USER c IDENTIFIED BY SEGREDO123""#,
        trecho: "exige PASSWORD",
        perfil: Some("'***'"),
        pela_web: true,
    },
    Caso {
        caminho: "2a volta, ALTER USER IDENTIFIED BY",
        corpo: r#""op":"sql","database":"loja","texto":"ALTER USER c IDENTIFIED BY \"SEGREDO123\"""#,
        trecho: "exige PASSWORD",
        perfil: Some("'***'"),
        pela_web: true,
    },
    // (3) A senha em comando que este tradutor nem executa.
    Caso {
        caminho: "2a volta, ALTER ROLE PASSWORD (PostgreSQL)",
        corpo: r#""op":"sql","database":"loja","texto":"ALTER ROLE c PASSWORD 'SEGREDO123'""#,
        trecho: "nao e um comando desta camada",
        perfil: Some("'***'"),
        pela_web: true,
    },
    Caso {
        caminho: "2a volta, SET PASSWORD FOR (MySQL, MariaDB)",
        corpo: r#""op":"sql","database":"loja","texto":"SET PASSWORD FOR c = 'SEGREDO123'""#,
        trecho: "nao e um comando desta camada",
        perfil: Some("'***'"),
        pela_web: true,
    },
    // Terceira volta do parecer SEC. B3: o valor do `?` ao lado do SQL com
    // senha -- o `SQLBindParameter` do ODBC.
    Caso {
        caminho: "B3, parametro ligado ao PASSWORD ?",
        corpo: r#""op":"sql","database":"loja","texto":"ALTER USER c PASSWORD ?","parametros":["SEGREDO123"]"#,
        trecho: "aspas simples",
        perfil: Some(r#""parametros":"***""#),
        pela_web: true,
    },
    // B4: o portao pelos simbolos dizia «nao», e o Profiler guarda os bytes.
    // O roteiro com a linha comentada RODA -- trecho vazio: tem de dar
    // certo --, e a redacao dele e o comentario sumir.
    Caso {
        caminho: "B4, linha comentada com a senha",
        corpo: r#""op":"sql","database":"loja","texto":"SELECT n FROM t; -- ALTER USER c PASSWORD 'SEGREDO123'""#,
        trecho: "",
        perfil: Some(r#""SELECT n FROM t;""#),
        pela_web: true,
    },
    Caso {
        caminho: "B4, bloco comentado com a senha",
        corpo: r#""op":"sql","database":"loja","texto":"SELECT n FROM t; /* ALTER USER c PASSWORD 'SEGREDO123' */""#,
        trecho: "",
        perfil: Some(r#""SELECT n FROM t;""#),
        pela_web: true,
    },
    Caso {
        caminho: "B4, comentario executavel do MySQL",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE USER c /*!80000 IDENTIFIED BY 'SEGREDO123' */""#,
        trecho: "exige PASSWORD",
        perfil: Some(r#""CREATE USER c""#),
        pela_web: true,
    },
    Caso {
        caminho: "B4, MASTER_PASSWORD",
        corpo: r#""op":"sql","database":"loja","texto":"CHANGE MASTER TO MASTER_PASSWORD='SEGREDO123'""#,
        trecho: "nao e um comando desta camada",
        perfil: Some("MASTER_PASSWORD '***'"),
        pela_web: true,
    },
    Caso {
        caminho: "B4, SOURCE_PASSWORD entre aspas duplas",
        corpo: r#""op":"sql","database":"loja","texto":"CHANGE REPLICATION SOURCE TO SOURCE_PASSWORD=\"SEGREDO123\"""#,
        trecho: "nao e um comando desta camada",
        perfil: Some("SOURCE_PASSWORD '***'"),
        pela_web: true,
    },
    Caso {
        caminho: "B4, senha dentro do literal de conexao (PostgreSQL)",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE SUBSCRIPTION s CONNECTION 'host=h password=SEGREDO123' PUBLICATION p""#,
        trecho: "cria TRIGGER ou PROCEDURE",
        perfil: Some("CONNECTION '***'"),
        pela_web: true,
    },
    Caso {
        caminho: "B4, PASSWORD entre aspas duplas",
        corpo: r#""op":"sql","database":"loja","texto":"CREATE USER c \"PASSWORD\" 'SEGREDO123'""#,
        trecho: "exige PASSWORD",
        perfil: Some("CREATE USER c '***'"),
        pela_web: true,
    },
];

fn caminhos(base: &std::path::Path) -> String {
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    format!(
        r#""base": "{b}", "log_acessos": "{l}", "seguranca": {{ "blacklist": "{bl}" }},
           "dblink": "{d}", "jobs": "{j}""#,
        b = bar(base.join("base")),
        l = bar(base.join("acessos.log")),
        bl = bar(base.join("blacklist.json")),
        d = bar(base.join("dblink.json")),
        j = bar(base.join("jobs.json")),
    )
}

/// Sobe com a porta de dados E a web: as duas anotam no `acessos.log`, por
/// caminhos irmaos (`atender` e `api_http`), e o 497 nasceu apontando a web.
fn subir(base: &std::path::Path) -> (Arc<Servidor>, u16, u16) {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{ "bind": "127.0.0.1:0", "token": "{TOKEN}", {caminhos},
                  "web": {{ "ligado": true, "bind": "127.0.0.1:0" }},
                  "cifra_fio": {{ "exigir": false }} }}"#,
            caminhos = caminhos(base),
        ),
    )
    .unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let dados = comum::porta_real(|| s.porta_dos_dados());
    let web = comum::porta_real(|| s.porta_web());
    for porta in [dados, web] {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let ate = Instant::now() + Duration::from_secs(5);
        while TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_err() {
            assert!(
                Instant::now() < ate,
                "o servidor nao subiu na porta {porta}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    (s, dados, web)
}

fn pedido(corpo: &str) -> String {
    format!(r#"{{"token":"{TOKEN}",{corpo}}}"#)
}

/// Uma linha pela porta de dados; a resposta crua.
fn pela_porta_de_dados(porta: u16, corpo: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    writeln!(escrita, "{}", pedido(corpo)).unwrap();
    escrita.flush().unwrap();
    let mut resp = String::new();
    BufReader::new(fluxo).read_line(&mut resp).unwrap();
    resp.trim_end().to_string()
}

/// Um `POST /api` pela porta web, so com o token; devolve o CORPO.
fn pela_web(porta: u16, corpo: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let json = pedido(corpo);
    write!(
        escrita,
        "POST /api HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
    .unwrap();
    let mut resposta = String::new();
    BufReader::new(fluxo).read_to_string(&mut resposta).unwrap();
    match resposta.split_once("\r\n\r\n") {
        Some((_, corpo)) => corpo.to_string(),
        None => resposta,
    }
}

/// O servidor responde ANTES de anotar: espera a linha nova chegar ao log.
fn esperar_o_log_crescer(log: &std::path::Path, antes: u64) -> u64 {
    let ate = Instant::now() + Duration::from_secs(10);
    loop {
        let agora = std::fs::metadata(log).map(|m| m.len()).unwrap_or(0);
        if agora > antes {
            return agora;
        }
        assert!(Instant::now() < ate, "a recusa nao foi anotada no log");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// **O dado do literal nao chega ao `acessos.log`, nem volta na resposta,
/// por nenhum dos caminhos que recusam com texto do pedido -- pelas duas
/// portas que anotam.**
///
/// Com o defeito reposto (a expressao citada inteira, ou o literal no
/// `descrever` do SQL), o vermelho lista cada caminho, porta e lugar que
/// vazou -- a tabela do dano, e nao so a primeira linha dela.
#[test]
fn o_literal_do_pedido_nao_chega_ao_acessos_log() {
    let base = DirTemp::novo("erro-no-log");
    let (_s, dados, web) = subir(&base);
    for corpo in [
        r#""op":"criar_database","database":"loja""#,
        r#""op":"criar_tabela","database":"loja","tabela":"t",
            "colunas":[{"nome":"n","tipo":"Int8"},{"nome":"nome","tipo":"Str(40)"}]"#,
        r#""op":"inserir","database":"loja","tabela":"t","linha":{"n":1,"nome":"Blumenau"}"#,
    ] {
        let r = pela_porta_de_dados(dados, &corpo.replace('\n', " "));
        assert!(
            Json::analisar(&r).unwrap().booleano_ou("ok", false),
            "preparo: {r}"
        );
    }

    // O Profiler ligado, num arquivo da pasta do teste: o B2 vazava SO por
    // ele, com a resposta e o log limpos.
    let perfil = base.join("perfil.txt");
    let r = pela_porta_de_dados(
        dados,
        &format!(
            r#""op":"profiler_ligar","arquivo":"{}""#,
            perfil.display().to_string().replace('\\', "/")
        ),
    );
    assert!(
        Json::analisar(&r).unwrap().booleano_ou("ok", false),
        "profiler_ligar: {r}"
    );

    let log = base.join("acessos.log");
    let mut vazou = Vec::new();
    let mut outro_caminho = Vec::new();
    let mut log_mudo = Vec::new();
    let mut perfil_sem_redacao = Vec::new();
    for (porta_nome, porta) in [("dados", dados), ("web", web)] {
        for caso in CASOS {
            if porta_nome == "web" && !caso.pela_web {
                continue;
            }
            let Caso {
                caminho,
                corpo,
                trecho,
                perfil: prova_do_perfil,
                ..
            } = caso;
            let antes = std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0);
            let perfil_antes = std::fs::metadata(&perfil).map(|m| m.len()).unwrap_or(0);
            let resposta = if porta_nome == "dados" {
                pela_porta_de_dados(porta, corpo)
            } else {
                pela_web(porta, corpo)
            };
            let j = Json::analisar(&resposta).unwrap_or_else(|_| {
                panic!("{porta_nome}/{caminho}: resposta nao e JSON: {resposta}")
            });
            let erro = j.texto_ou("erro", "").to_string();
            let passou_pelo_caminho = if trecho.is_empty() {
                j.booleano_ou("ok", false)
            } else {
                !j.booleano_ou("ok", true) && erro.contains(trecho)
            };
            if !passou_pelo_caminho {
                outro_caminho.push(format!("{porta_nome}/{caminho}: {resposta}"));
            }
            if resposta.contains(MARCA) {
                vazou.push(format!("{porta_nome}/{caminho}, na RESPOSTA: {erro}"));
            }
            let depois = esperar_o_log_crescer(&log, antes);
            let bytes = std::fs::read(&log).unwrap();
            let linhas = String::from_utf8_lossy(&bytes[antes as usize..depois as usize]);
            for linha in linhas.lines().filter(|l| l.contains(MARCA)) {
                vazou.push(format!("{porta_nome}/{caminho}, no acessos.log: {linha}"));
            }
            // E o log continua dizendo POR QUE recusou: a redacao tira o
            // dado, nao a recusa.
            let diz_o_porque = linhas.lines().any(|l| {
                Json::analisar(l)
                    .map(|a| a.texto_ou("erro", "").contains(trecho))
                    .unwrap_or(false)
            });
            if !trecho.is_empty() && !diz_o_porque {
                log_mudo.push(format!("{porta_nome}/{caminho}: {linhas}"));
            }
            // O Profiler escreve no `terminou`, ANTES do `anotar`: com o log
            // crescido, a linha do perfil ja esta no arquivo.
            if let Some(prova) = prova_do_perfil {
                let bytes = std::fs::read(&perfil).unwrap_or_default();
                let novas = String::from_utf8_lossy(&bytes[perfil_antes as usize..]);
                for linha in novas.lines().filter(|l| l.contains(MARCA)) {
                    vazou.push(format!("{porta_nome}/{caminho}, no perfil.txt: {linha}"));
                }
                // E a redacao RODOU: sem isto, uma linha que o Profiler nem
                // redigiu passaria como limpa.
                if !novas.contains(prova) {
                    perfil_sem_redacao.push(format!("{porta_nome}/{caminho}: {novas}"));
                }
            }
        }
    }
    assert!(
        outro_caminho.is_empty(),
        "caso que nao passou pelo caminho que diz passar -- sem isso a ausencia \
         da marca nao provaria nada:\n  {}",
        outro_caminho.join("\n  ")
    );
    assert!(
        vazou.is_empty(),
        "o literal do pedido vazou:\n  {}",
        vazou.join("\n  ")
    );
    assert!(
        log_mudo.is_empty(),
        "o acessos.log perdeu o motivo da recusa:\n  {}",
        log_mudo.join("\n  ")
    );
    assert!(
        perfil_sem_redacao.is_empty(),
        "caso de senha cujo perfil nao mostra redacao nenhuma -- a ausencia da \
         marca ali nao prova nada:\n  {}",
        perfil_sem_redacao.join("\n  ")
    );
}
