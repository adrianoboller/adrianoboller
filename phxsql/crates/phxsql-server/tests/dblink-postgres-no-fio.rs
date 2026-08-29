//! O DbLink falando PostgreSQL(R) **pelo soquete**, byte a byte.
//!
//! # Por que um servidor falso, e nao um teste unitario
//!
//! Porque a licao ja foi paga uma vez neste projeto: *teste unitario nao prova
//! queda de conexao -- soquete prova*. Os dez testes do `BULKINSERT` passavam
//! e a reserva nao era solta, e o que achou o defeito foi um cliente Python
//! falando o protocolo de verdade.
//!
//! Aqui o problema e o mesmo de outro lado: **nao ha PostgreSQL(R) instalado
//! nesta maquina**. Um teste que so montasse a cadeia de SQL provaria que a
//! cadeia esta escrita, e nao que o cliente a poe no fio do jeito certo -- e o
//! erro classico deste protocolo (`int32` de tamanho que **inclui a si mesmo**)
//! nao aparece em nenhuma cadeia de SQL.
//!
//! Entao o teste sobe um servidor que fala o protocolo de fio: le a mensagem
//! de abertura e confere os parametros, conduz o SCRAM-SHA-256 inteiro
//! **conferindo a prova do cliente**, e depois compara o `Q` recebido com o SQL
//! exato que o dialeto deve ter montado.
//!
//! # O que este teste NAO prova
//!
//! Que o SQL do dialeto e aceito por um PostgreSQL(R) de verdade. Um servidor
//! falso responde o que eu mandar ele responder; ele nao valida sintaxe, nao
//! tem `pg_class` e nao sabe se `unnest(...) WITH ORDINALITY` existe na versao
//! do outro lado. **Essa prova continua pendente**, e o que ela exige esta
//! escrito em `docs/DBLINK.md`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use phxsql_core::base64;
use phxsql_core::hash::{hmac_sha256, pbkdf2_sha256, sha256};
use phxsql_core::json::Json;
use phxsql_server::dblink::{Definicao, Motor};

const SENHA: &str = "pencil";
const SAL: &[u8] = b"salt do teste 16";
const ITERACOES: u32 = 4096;

// ---------------------------------------------------------------------------
// O servidor falso
// ---------------------------------------------------------------------------

/// O que o servidor falso viu passar, para o teste conferir depois.
#[derive(Debug, Default)]
struct Visto {
    parametros: Vec<(String, String)>,
    /// Toda mensagem do cliente, como `(tipo, corpo)`.
    mensagens: Vec<(u8, Vec<u8>)>,
    consultas: Vec<String>,
    /// A prova do SCRAM conferiu?
    prova_confere: bool,
    /// O tamanho declarado bateu com o lido, em toda mensagem?
    tamanhos_batem: bool,
}

struct Fio {
    fluxo: TcpStream,
}

impl Fio {
    /// Le uma mensagem com byte de tipo. Devolve `(tipo, corpo, tamanho_declarado)`.
    fn ler(&mut self) -> Option<(u8, Vec<u8>, i32)> {
        let mut cab = [0u8; 5];
        self.fluxo.read_exact(&mut cab).ok()?;
        let tamanho = i32::from_be_bytes([cab[1], cab[2], cab[3], cab[4]]);
        let mut corpo = vec![0u8; (tamanho - 4).max(0) as usize];
        if !corpo.is_empty() {
            self.fluxo.read_exact(&mut corpo).ok()?;
        }
        Some((cab[0], corpo, tamanho))
    }

    fn escrever(&mut self, tipo: u8, carga: &[u8]) {
        let mut m = vec![tipo];
        m.extend_from_slice(&((carga.len() + 4) as i32).to_be_bytes());
        m.extend_from_slice(carga);
        self.fluxo.write_all(&m).unwrap();
        self.fluxo.flush().unwrap();
    }

    fn autenticacao(&mut self, codigo: i32, resto: &[u8]) {
        let mut c = codigo.to_be_bytes().to_vec();
        c.extend_from_slice(resto);
        self.escrever(b'R', &c);
    }
}

fn cadeia_nula(v: &mut Vec<u8>, s: &str) {
    v.extend_from_slice(s.as_bytes());
    v.push(0);
}

fn cadeias_nulas(b: &[u8]) -> Vec<String> {
    b.split(|c| *c == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

/// Sobe o servidor falso. Devolve a porta e o canal por onde ele conta o que
/// viu depois de a conexao terminar.
fn subir_servidor(respostas: Vec<Resposta>) -> (u16, mpsc::Receiver<Visto>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let (envia, recebe) = mpsc::channel();

    thread::spawn(move || {
        let (soquete, _) = ouvinte.accept().unwrap();
        soquete
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut fio = Fio { fluxo: soquete };
        let mut visto = Visto {
            tamanhos_batem: true,
            ..Default::default()
        };

        // ---------------------------------------------------- StartupMessage
        //
        // Ela NAO tem byte de tipo: comeca direto pelo tamanho, e o "tipo" dela
        // e a versao do protocolo. E mais velha que o byte de tipo.
        let mut tam = [0u8; 4];
        fio.fluxo.read_exact(&mut tam).unwrap();
        let tamanho = i32::from_be_bytes(tam);
        let mut corpo = vec![0u8; tamanho as usize - 4];
        fio.fluxo.read_exact(&mut corpo).unwrap();
        // O `int32` de tamanho INCLUI a si mesmo -- e o erro classico de quem
        // escreve este protocolo pela primeira vez.
        if tamanho as usize != corpo.len() + 4 {
            visto.tamanhos_batem = false;
        }
        let versao = i32::from_be_bytes([corpo[0], corpo[1], corpo[2], corpo[3]]);
        assert_eq!(versao, 196_608, "versao do protocolo nao e a 3.0");
        let campos = cadeias_nulas(&corpo[4..]);
        for par in campos.chunks(2) {
            if par.len() == 2 {
                visto.parametros.push((par[0].clone(), par[1].clone()));
            }
        }

        // ------------------------------------------------------------- SCRAM
        let mut mecanismos = Vec::new();
        cadeia_nula(&mut mecanismos, "SCRAM-SHA-256");
        mecanismos.push(0);
        fio.autenticacao(10, &mecanismos);

        let (tipo, corpo, declarado) = fio.ler().unwrap();
        if declarado as usize != corpo.len() + 4 {
            visto.tamanhos_batem = false;
        }
        visto.mensagens.push((tipo, corpo.clone()));
        assert_eq!(tipo, b'p', "esperava SASLInitialResponse");
        let fim_nome = corpo.iter().position(|b| *b == 0).unwrap();
        let mecanismo = String::from_utf8_lossy(&corpo[..fim_nome]).into_owned();
        assert_eq!(mecanismo, "SCRAM-SHA-256");
        let n = i32::from_be_bytes(corpo[fim_nome + 1..fim_nome + 5].try_into().unwrap()) as usize;
        let primeira_do_cliente =
            String::from_utf8_lossy(&corpo[fim_nome + 5..fim_nome + 5 + n]).into_owned();
        assert_eq!(
            n,
            primeira_do_cliente.len(),
            "o tamanho declarado da resposta SASL nao bate"
        );

        // `n,,` e o cabecalho GS2 "sem binding de canal".
        let sem_cabecalho = primeira_do_cliente.strip_prefix("n,,").unwrap().to_string();
        let nonce_cliente = sem_cabecalho
            .split(',')
            .find_map(|p| p.strip_prefix("r="))
            .unwrap()
            .to_string();

        let nonce = format!("{nonce_cliente}servidor-falso");
        let servidor_primeira = format!("r={nonce},s={},i={ITERACOES}", base64::codificar(SAL));
        fio.autenticacao(11, servidor_primeira.as_bytes());

        let (tipo, corpo, _) = fio.ler().unwrap();
        visto.mensagens.push((tipo, corpo.clone()));
        assert_eq!(tipo, b'p', "esperava a client-final-message");
        let final_do_cliente = String::from_utf8_lossy(&corpo).into_owned();
        let sem_prova = final_do_cliente
            .rsplit_once(",p=")
            .map(|(a, _)| a.to_string())
            .unwrap();
        let prova = base64::decodificar(final_do_cliente.rsplit_once(",p=").unwrap().1).unwrap();

        // A conta do servidor, do RFC 5802. Se a prova do cliente bater com
        // ela, o cliente conhece a senha -- e a senha nunca passou pelo fio.
        let mut salgada = [0u8; 32];
        pbkdf2_sha256(SENHA.as_bytes(), SAL, ITERACOES, &mut salgada);
        let chave_cliente = hmac_sha256(&salgada, b"Client Key");
        let chave_guardada = sha256(&chave_cliente);
        let mensagem = format!("{sem_cabecalho},{servidor_primeira},{sem_prova}");
        let assinatura_cliente = hmac_sha256(&chave_guardada, mensagem.as_bytes());
        let esperada: Vec<u8> = chave_cliente
            .iter()
            .zip(assinatura_cliente.iter())
            .map(|(a, b)| a ^ b)
            .collect();
        visto.prova_confere = prova == esperada;

        let chave_servidor = hmac_sha256(&salgada, b"Server Key");
        let assinatura = hmac_sha256(&chave_servidor, mensagem.as_bytes());
        fio.autenticacao(
            12,
            format!("v={}", base64::codificar(&assinatura)).as_bytes(),
        );

        // ------------------------------------------------------- AuthenticationOk
        fio.autenticacao(0, &[]);
        let mut p = Vec::new();
        cadeia_nula(&mut p, "server_version");
        cadeia_nula(&mut p, "17.2 (servidor falso)");
        fio.escrever(b'S', &p);
        let mut k = 4242u32.to_be_bytes().to_vec();
        k.extend_from_slice(&7u32.to_be_bytes());
        fio.escrever(b'K', &k);
        fio.escrever(b'Z', b"I");

        // ------------------------------------------------------------ consultas
        let mut i = 0;
        while let Some((tipo, corpo, declarado)) = fio.ler() {
            if declarado as usize != corpo.len() + 4 {
                visto.tamanhos_batem = false;
            }
            visto.mensagens.push((tipo, corpo.clone()));
            match tipo {
                b'Q' => {
                    let sql = String::from_utf8_lossy(&corpo)
                        .trim_end_matches('\0')
                        .to_string();
                    visto.consultas.push(sql);
                    let r = respostas.get(i).cloned().unwrap_or_default();
                    i += 1;
                    responder(&mut fio, &r);
                }
                b'X' => break,
                _ => {}
            }
        }
        let _ = envia.send(visto);
    });

    (porta, recebe)
}

/// O que o servidor falso responde a uma consulta.
#[derive(Debug, Clone, Default)]
struct Resposta {
    colunas: Vec<(&'static str, u32)>,
    linhas: Vec<Vec<Option<&'static str>>>,
    /// Quando preenchido, responde `E` (erro) em vez de linhas.
    erro: Option<(&'static str, &'static str)>,
}

fn responder(fio: &mut Fio, r: &Resposta) {
    if let Some((codigo, mensagem)) = r.erro {
        let mut e = Vec::new();
        e.push(b'S');
        cadeia_nula(&mut e, "ERROR");
        e.push(b'C');
        cadeia_nula(&mut e, codigo);
        e.push(b'M');
        cadeia_nula(&mut e, mensagem);
        e.push(0);
        fio.escrever(b'E', &e);
        fio.escrever(b'Z', b"I");
        return;
    }

    let mut t = (r.colunas.len() as i16).to_be_bytes().to_vec();
    for (nome, oid) in &r.colunas {
        cadeia_nula(&mut t, nome);
        t.extend_from_slice(&0i32.to_be_bytes()); // oid da tabela
        t.extend_from_slice(&0i16.to_be_bytes()); // numero do atributo
        t.extend_from_slice(&(*oid as i32).to_be_bytes()); // oid do tipo
        t.extend_from_slice(&(-1i16).to_be_bytes()); // tamanho
        t.extend_from_slice(&(-1i32).to_be_bytes()); // modificador
        t.extend_from_slice(&0i16.to_be_bytes()); // formato: texto
    }
    fio.escrever(b'T', &t);

    for linha in &r.linhas {
        let mut d = (linha.len() as i16).to_be_bytes().to_vec();
        for v in linha {
            match v {
                // -1 e NULL de verdade; cadeia vazia tem tamanho 0, e nao e a
                // mesma coisa.
                None => d.extend_from_slice(&(-1i32).to_be_bytes()),
                Some(t) => {
                    d.extend_from_slice(&(t.len() as i32).to_be_bytes());
                    d.extend_from_slice(t.as_bytes());
                }
            }
        }
        fio.escrever(b'D', &d);
    }
    let mut c = Vec::new();
    cadeia_nula(&mut c, &format!("SELECT {}", r.linhas.len()));
    fio.escrever(b'C', &c);
    fio.escrever(b'Z', b"I");
}

// ---------------------------------------------------------------------------
// Os testes
// ---------------------------------------------------------------------------

fn ligacao(porta: u16) -> Definicao {
    Definicao::de_json(
        &Json::analisar(&format!(
            r#"{{"nome":"falso","motor":"postgres","host":"127.0.0.1",
                 "porta":{porta},"usuario":"adriano","senha":"{SENHA}",
                 "database":"erp","timeout_s":10}}"#
        ))
        .unwrap(),
    )
    .unwrap()
}

/// O aperto de mao inteiro: parametros da abertura, SCRAM conferido pelo
/// servidor, e o `Q` chegando com o SQL do dialeto certo.
#[test]
fn o_aperto_de_mao_e_a_consulta_chegam_como_o_protocolo_manda() {
    let (porta, recebe) = subir_servidor(vec![
        // A primeira consulta e o `ping`, que e um `SELECT 1`.
        Resposta {
            colunas: vec![("um", 23)],
            linhas: vec![vec![Some("1")]],
            erro: None,
        },
        Resposta {
            colunas: vec![
                ("current_user", 25),
                ("current_database", 25),
                ("version", 25),
            ],
            linhas: vec![vec![
                Some("adriano"),
                Some("erp"),
                Some("PostgreSQL 17.2 falso"),
            ]],
            erro: None,
        },
    ]);

    let d = ligacao(porta);
    assert!(d.motor.conecta(), "o motor devia estar aceso");
    let resposta = phxsql_server::dblink::operacoes::testar(&d, d.abrir().unwrap()).unwrap();

    let visto = recebe.recv_timeout(Duration::from_secs(10)).unwrap();

    // 1. Os parametros da abertura.
    let par = |k: &str| {
        visto
            .parametros
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(par("user").as_deref(), Some("adriano"));
    assert_eq!(par("database").as_deref(), Some("erp"));
    assert_eq!(par("client_encoding").as_deref(), Some("UTF8"));
    // Quem administra o PostgreSQL(R) precisa saber quem esta consultando: o
    // `application_name` aparece no `pg_stat_activity`.
    assert_eq!(par("application_name").as_deref(), Some("PhxSql DbLink"));

    // 2. O tamanho de TODA mensagem inclui os proprios 4 bytes.
    assert!(
        visto.tamanhos_batem,
        "algum `int32` de tamanho nao incluiu a si mesmo"
    );

    // 3. O SCRAM: a prova do cliente bate com a conta do servidor.
    assert!(
        visto.prova_confere,
        "a prova do SCRAM nao bate: o cliente nao provou conhecer a senha"
    );

    // 4. E a senha NUNCA passou pelo fio, em nenhuma mensagem.
    for (tipo, corpo) in &visto.mensagens {
        assert!(
            !corpo.windows(SENHA.len()).any(|j| j == SENHA.as_bytes()),
            "a senha viajou na mensagem {:?}",
            *tipo as char
        );
    }

    // 5. O SQL e o do dialeto do PostgreSQL(R), e nao o do MySQL(R).
    //
    // Sao duas: o `ping`, que e a ida e volta barata, e a pergunta que o
    // operador realmente quer.
    assert_eq!(
        visto.consultas,
        vec!["SELECT 1", Motor::Postgres.sql_quem_sou()]
    );
    assert!(
        !visto.consultas[1].contains("current_user()"),
        "foi o SQL do MySQL: {}",
        visto.consultas[1]
    );

    // 6. E a resposta chegou de volta inteira.
    let texto = resposta.escrever();
    assert!(texto.contains("adriano"), "{texto}");
    assert!(texto.contains("17.2"), "{texto}");
    assert!(texto.contains("4242"), "o PID do backend sumiu: {texto}");
}

/// `dblink_tabelas` monta o SQL do catalogo do PostgreSQL(R) e le a resposta.
#[test]
fn as_tabelas_saem_do_catalogo_do_postgres() {
    let (porta, recebe) = subir_servidor(vec![Resposta {
        colunas: vec![
            ("relname", 25),
            ("tipo", 25),
            ("motor", 25),
            ("reltuples", 20),
            ("bytes", 20),
            ("comentario", 25),
            ("nspname", 25),
        ],
        linhas: vec![
            vec![
                Some("clientes"),
                Some("BASE TABLE"),
                Some("postgres"),
                // O que o PostgreSQL(R) 14+ devolve para tabela nunca
                // analisada. Mostrar "-1 registros" seria pior que zero.
                Some("-1"),
                Some("81920"),
                Some("cadastro"),
                Some("public"),
            ],
            vec![
                Some("pedidos"),
                Some("VIEW"),
                Some("postgres"),
                Some("1200"),
                Some("0"),
                None,
                Some("public"),
            ],
        ],
        erro: None,
    }]);

    let d = ligacao(porta);
    let p = Json::analisar(r#"{"database":"public"}"#).unwrap();
    let r = phxsql_server::dblink::operacoes::tabelas(&d, d.abrir().unwrap(), &p).unwrap();
    let visto = recebe.recv_timeout(Duration::from_secs(10)).unwrap();

    let sql = &visto.consultas[0];
    assert!(sql.contains("pg_class"), "{sql}");
    assert!(sql.contains("'public'"), "{sql}");
    assert!(
        !sql.contains("information_schema.TABLES"),
        "foi o SQL do MySQL: {sql}"
    );
    assert!(!sql.contains('`'), "crase de MySQL no fio: {sql}");

    let texto = r.escrever();
    assert!(
        texto.contains("clientes") && texto.contains("pedidos"),
        "{texto}"
    );
    // O `-1` do `reltuples` vira zero, e nao um numero negativo na tela.
    assert!(!texto.contains("-1"), "o reltuples negativo vazou: {texto}");
    assert!(texto.contains("81920"), "{texto}");
}

/// `dblink_ler` monta `LIMIT n OFFSET m`, que os dois entendem -- e nunca o
/// `LIMIT m, n`, que so o MySQL(R) entende.
#[test]
fn a_paginacao_no_fio_e_a_que_o_postgres_entende() {
    let (porta, recebe) = subir_servidor(vec![Resposta {
        colunas: vec![("id", 20), ("nome", 25)],
        linhas: vec![vec![Some("1"), Some("Blumenau")]],
        erro: None,
    }]);

    let d = ligacao(porta);
    let p = Json::analisar(
        r#"{"tabela":"clientes","database":"public","limite":50,"salto":100,"ordem":"nome"}"#,
    )
    .unwrap();
    let r = phxsql_server::dblink::operacoes::ler(&d, d.abrir().unwrap(), &p).unwrap();
    let visto = recebe.recv_timeout(Duration::from_secs(10)).unwrap();

    let sql = &visto.consultas[0];
    assert_eq!(
        sql,
        "SELECT * FROM \"public\".\"clientes\" ORDER BY \"nome\" ASC LIMIT 51 OFFSET 100"
    );
    assert!(!sql.contains('`'), "crase de MySQL no fio: {sql}");
    // Uma linha a mais do que o teto e pedida de proposito: se ela vier, ha
    // mais pagina.
    assert!(sql.contains("LIMIT 51"), "{sql}");

    let texto = r.escrever();
    assert!(texto.contains("Blumenau"), "{texto}");
    assert!(texto.contains("\"tem_mais\":false"), "{texto}");
}

/// Um erro do servidor sai como erro do PhxSql, com o SQLSTATE traduzido -- e
/// o ciclo da consulta e lido ate o `ReadyForQuery`.
#[test]
fn erro_do_servidor_vira_erro_e_nao_desencontro() {
    let (porta, recebe) = subir_servidor(vec![
        Resposta {
            erro: Some(("42P01", "relation \"clientes\" does not exist")),
            ..Default::default()
        },
        Resposta {
            colunas: vec![("um", 23)],
            linhas: vec![vec![Some("1")]],
            erro: None,
        },
    ]);

    let d = ligacao(porta);
    let mut c = d.conectar_pg().unwrap();
    let e = c.consultar("SELECT * FROM clientes", 10).unwrap_err();
    assert!(e.to_string().contains("42P01"), "{e}");

    // A consulta SEGUINTE tem de ler a resposta DELA, e nao a que sobrou. E o
    // defeito mais dificil de achar neste protocolo: sair no `E` deixaria o
    // `Z` na fila e tudo funcionaria com um desencontro constante de uma
    // mensagem.
    let r = c.consultar("SELECT 1", 10).unwrap();
    assert_eq!(r.linhas.len(), 1);
    assert_eq!(r.colunas[0].nome, "um");
    c.encerrar();

    let visto = recebe.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(visto.consultas.len(), 2);
}

/// A senha em texto puro e o `md5` sao recusados, dizendo o que mudar.
#[test]
fn os_metodos_de_autenticacao_fracos_sao_recusados() {
    for (codigo, agulha) in [(3i32, "pg_hba.conf"), (5, "MD5")] {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        thread::spawn(move || {
            let (soquete, _) = ouvinte.accept().unwrap();
            let mut fio = Fio { fluxo: soquete };
            let mut tam = [0u8; 4];
            fio.fluxo.read_exact(&mut tam).unwrap();
            let tamanho = i32::from_be_bytes(tam);
            let mut corpo = vec![0u8; tamanho as usize - 4];
            fio.fluxo.read_exact(&mut corpo).unwrap();
            // `md5` manda 4 bytes de sal atras do codigo.
            let resto: &[u8] = if codigo == 5 { &[1, 2, 3, 4] } else { &[] };
            fio.autenticacao(codigo, resto);
            // Segura o soquete aberto para o cliente ler a resposta antes do
            // fim da conexao.
            thread::sleep(Duration::from_millis(200));
        });

        let d = ligacao(porta);
        let Err(e) = d.abrir() else {
            panic!("o cliente aceitou o metodo {codigo}")
        };
        let texto = e.to_string();
        assert!(
            texto.contains(agulha),
            "o erro do metodo {codigo} nao diz o que mudar: {texto}"
        );
    }
}
