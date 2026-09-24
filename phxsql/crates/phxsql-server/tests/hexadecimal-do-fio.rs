//! Hexadecimal que vem do fio, e o panico de fatiar texto por BYTE -- pedido
//! 446, provado PELO SOQUETE.
//!
//! # O defeito
//!
//! `phxsql_core::hash::de_hex` fatiava `&t[i..i + 2]` e conferia a paridade
//! com `len() % 2`, que conta BYTES. `"a€"` tem quatro bytes -- par --, e o
//! corte `t[2..4]` cai no meio do `€`: o `str` do Rust entra em panico em vez
//! de devolver erro. A copia fiel da funcao que morava em
//! `phxsql_core::carga::hex_para_bytes` tinha o mesmo corte, e o
//! `desescapar` da porta HTTP fatiava `bruto[i + 1..i + 3]` do mesmo jeito.
//!
//! # Por que soquete, e nao teste de unidade
//!
//! O que decide a gravidade nao e «a funcao entra em panico» -- isso a copia
//! compilada sozinha ja mostrava. E o que o panico LEVA junto: a conexao, a
//! porta, ou uma trava que outra thread vai pedir depois. E isso so aparece
//! com o servidor de pe, com as threads de verdade e as travas de verdade.
//!
//! # O que cada teste mede
//!
//! * a `prova` do `cluster_pulso` (o chamador que motivou o 446);
//! * um valor `Bin` do `inserir` -- o irmao que alcanca QUALQUER usuario com
//!   direito de inserir, e dentro da trava global de dados;
//! * o `%XX` do `GET /idiomas`, que e servido ANTES de qualquer credencial.
//!
//! Cada vermelho descreve o DANO medido com o defeito reposto, e nao o
//! veredito: «a conexao caiu sem resposta», «a trava ficou envenenada».

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::hash::para_hex;
use phxsql_core::json::Json;
use phxsql_core::x25519;
use phxsql_server::pulso;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token da prova do hexadecimal";

/// Quatro bytes, PAR, e o corte do segundo par cai no meio do `€`. Repetido
/// dezesseis vezes da os 64 bytes de uma prova HMAC-SHA256 em hexadecimal --
/// o tamanho que o pulso legitimo tem, para a recusa nao poder vir do
/// tamanho.
fn hex_que_corta_um_caractere() -> String {
    "a€".repeat(16)
}

fn privada(semente: &str) -> [u8; 32] {
    let bytes = phxsql_core::hash::de_hex(&semente.repeat(32)).unwrap();
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    k
}

/// Sobe o servidor e espera a porta de dados atender.
///
/// A porta e a REAL, lida do proprio servidor depois do `bind` -- pedido 401.
fn no_ar(s: Arc<Servidor>) -> (Arc<Servidor>, u16) {
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
    panic!("o servidor nao subiu na porta {porta}");
}

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

/// Uma linha do protocolo, e a resposta CRUA -- `None` quando a conexao
/// fechou sem responder, que e exatamente o dano que estes testes medem.
fn falar(porta: u16, pedido: &str) -> Option<String> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{pedido}").unwrap();
    escrita.flush().unwrap();
    let mut resp = String::new();
    match leitor.read_line(&mut resp) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(resp.trim_end().to_string()),
    }
}

fn ok(resposta: Option<String>, o_que: &str) -> Json {
    let texto = resposta.unwrap_or_else(|| panic!("{o_que}: a conexao caiu sem resposta"));
    let j = Json::analisar(&texto).unwrap();
    assert!(j.booleano_ou("ok", false), "{o_que}: {texto}");
    j
}

/* ------------------------------------------------ 1. a prova do pulso */

/// O `noA`, master de um cluster de tres, com chave estatica -- sem ela o
/// `conferir_identidade` recusa antes de chegar ao `de_hex`, e a prova
/// mediria outra porta.
///
/// `porta_b` e o endereco DECLARADO do `noB` -- em quase todos os testes
/// deste arquivo, uma porta onde ninguem escuta de proposito
/// (`comum::porta_fechada`, que so serve essa unica finalidade desde o
/// pedido 401: nenhum `porta_livre()` sobrevive neste arquivo). A entrada de
/// `noA` na propria lista de `nos` nao presta para nada -- ninguem conecta em
/// si mesmo -- entao ela pode continuar com o texto "0" sem afetar a prova.
fn subir_no_a(base: &std::path::Path, porta_b: u16) -> (Arc<Servidor>, u16) {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    let pino_b = para_hex(&x25519::chave_publica(&privada("bb")));
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0", "token": "{TOKEN}", {caminhos},
              "web": {{ "ligado": false }},
              "cifra_fio": {{ "ligada": true, "exigir": false, "chave_privada": "{priv_a}" }},
              "replicacao": {{ "papel": "source", "id_servidor": "noA", "imagem_da_linha": true }},
              "cluster": {{
                "id": "noA", "token": "{TOKEN}", "janela_inatividade_s": 3, "pulso_s": 1,
                "cifra": false,
                "nos": [
                  {{ "id": "noA", "endereco": "127.0.0.1", "porta": 0 }},
                  {{ "id": "noB", "endereco": "127.0.0.1", "porta": {porta_b}, "chave_do_fio": "{pino_b}" }},
                  {{ "id": "noC", "endereco": "127.0.0.1", "porta": 7497 }}
                ]
              }}
            }}"#,
            caminhos = caminhos(base),
            priv_a = "aa".repeat(32),
        ),
    )
    .unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
    no_ar(s)
}

/// O pulso do `noB` com a `prova` escolhida pelo teste. O resto do corpo e o
/// de um pulso legitimo: `para`, `quando` e um `nonce` no formato do emissor,
/// para nada antes do `de_hex` recusar.
fn pulso_com_prova(prova: &str) -> String {
    format!(
        r#"{{"token":"{TOKEN}","op":"cluster_pulso","id":"noB","papel":"replica","epoca":0,"posicao":0,"incompleta":false,"prioridade":0,"para":"noA","quando":{quando},"nonce":"{nonce}","prova":"{prova}"}}"#,
        quando = phxsql_server::agora_ms(),
        nonce = pulso::nonce(),
    )
}

/// **A prova do pulso que corta um caractere e RECUSADA, com resposta.**
///
/// Com o `de_hex` velho, medido: a conexao cai sem uma linha de resposta --
/// a thread de atendimento morreu no corte do `€`. O servidor continua
/// atendendo outra conexao (o panico nao toca trava nenhuma: o
/// `conferir_identidade` nao segura nenhuma quando chama o `de_hex`), e e
/// isso que separa este caminho do irmao `Bin` logo abaixo.
#[test]
fn a_prova_do_pulso_que_corta_um_caractere_e_recusada_com_resposta() {
    let base = DirTemp::novo("hex-pulso");
    // O noB fica numa porta onde ninguem escuta: o laco de pulso do noA so
    // falha a conexao, e a prova mede so o PEDIDO.
    let (_a, porta) = subir_no_a(&base, comum::porta_fechada());

    let resposta = falar(porta, &pulso_com_prova(&hex_que_corta_um_caractere()));

    // O alcance primeiro, que e o que o vermelho tem de mostrar junto: o no
    // continua atendendo outra conexao, e as travas que o pedido seguinte
    // pede -- a do cluster e a de dados -- nao ficaram envenenadas.
    ok(
        falar(
            porta,
            &format!(r#"{{"token":"{TOKEN}","op":"cluster_estado"}}"#),
        ),
        "o cluster_estado depois da prova torta",
    );
    ok(
        falar(
            porta,
            &format!(r#"{{"token":"{TOKEN}","op":"criar_database","database":"depois"}}"#),
        ),
        "a trava de dados depois da prova torta",
    );

    let texto = resposta.unwrap_or_else(|| {
        panic!(
            "a conexao do cluster_pulso caiu SEM resposta: a thread de \
             atendimento morreu no de_hex da prova (\"a€\" x16, 64 bytes). \
             O no seguiu atendendo e nenhuma trava envenenou -- o dano e a \
             conexao"
        )
    });
    let j = Json::analisar(&texto).unwrap();
    assert!(
        !j.booleano_ou("ok", true),
        "a prova que nem e hexadecimal passou: {texto}"
    );
    // A frase unica do 435: o motivo vai ao log, nao ao fio.
    assert!(
        j.texto_ou("erro", "").contains("nao foi aceita"),
        "a recusa nao saiu pela porta unica da prova: {texto}"
    );
}

/// Um par no endereco do `noB` que responde a TODO pulso com a `prova` que
/// corta um caractere. Devolve a porta e quantos pulsos ele recebeu -- o
/// canario de que o laco do `noA` continua chegando nele.
fn par_que_responde_prova_torta() -> (u16, Arc<AtomicUsize>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let vistos = Arc::new(AtomicUsize::new(0));
    let contador = Arc::clone(&vistos);
    std::thread::spawn(move || {
        for conexao in ouvinte.incoming() {
            let Ok(conexao) = conexao else { return };
            let contador = Arc::clone(&contador);
            std::thread::spawn(move || {
                let Ok(mut escrita) = conexao.try_clone() else {
                    return;
                };
                for linha in BufReader::new(conexao).lines() {
                    if linha.is_err() {
                        return;
                    }
                    contador.fetch_add(1, Ordering::SeqCst);
                    let resposta = format!(
                        r#"{{"ok":true,"op":"cluster_pulso","resultado":{{"id":"noB","papel":"replica","epoca":0,"posicao":0,"incompleta":false,"prioridade":0,"para":"noA","quando":{quando},"nonce":"{nonce}","prova":"{prova}"}}}}"#,
                        quando = phxsql_server::agora_ms(),
                        nonce = pulso::nonce(),
                        prova = hex_que_corta_um_caractere(),
                    );
                    if writeln!(escrita, "{resposta}").is_err() {
                        return;
                    }
                }
            });
        }
    });
    (porta, vistos)
}

/// **O irmao do pedido: a RESPOSTA do pulso.** O laco do pulso do `noA`
/// confere a resposta do par pelo mesmo `conferir_identidade`, e o mesmo
/// `de_hex`.
///
/// Com o `de_hex` velho, medido: a thread de pulso do `noA` para o `noB`
/// morre no primeiro pulso e NUNCA mais sobe -- ela so se desmarca do
/// `pulsando` quando sai pelo caminho normal, e o supervisor nao sobe outra
/// para um id que ainda esta marcado. O par recebe 1 pulso e mais nada.
#[test]
fn a_prova_torta_na_resposta_nao_mata_o_laco_do_pulso() {
    let base = DirTemp::novo("hex-pulso-resposta");
    let (porta_b, vistos) = par_que_responde_prova_torta();
    let (_a, _porta) = subir_no_a(&base, porta_b);

    // pulso_s = 1: em 4,5 s o laco vivo pulsa quatro vezes ou mais.
    std::thread::sleep(Duration::from_millis(4_500));
    let recebidos = vistos.load(Ordering::SeqCst);
    assert!(
        recebidos >= 3,
        "o noA pulsou o noB {recebidos} vez(es) em 4,5 s com pulso_s = 1: a \
         thread de pulso morreu no de_hex da RESPOSTA e o supervisor nao sobe \
         outra para um id que continua marcado"
    );
}

/* ---------------------------------------------- 2. o valor Bin do inserir */

fn subir_simples(base: &std::path::Path) -> (Arc<Servidor>, u16) {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{ "bind": "127.0.0.1:0", "token": "{TOKEN}", {caminhos},
                  "web": {{ "ligado": false }}, "cifra_fio": {{ "exigir": false }} }}"#,
            caminhos = caminhos(base),
        ),
    )
    .unwrap();
    no_ar(Servidor::novo(Config::ler(&caminho).unwrap()).unwrap())
}

/// O protocolo e uma linha por pedido: o corpo escrito em varias linhas no
/// fonte, por legibilidade, viaja numa so.
fn pedido(corpo: &str) -> String {
    format!(r#"{{"token":"{TOKEN}",{}}}"#, corpo.replace('\n', " "))
}

/// **O irmao que custava o servidor inteiro.** `json_para_valor` le a
/// coluna `Bin` pelo `hex_para_bytes`, e o `op_inserir` chama a conversao
/// DEPOIS de tomar a trava global de dados -- ele precisa do esquema.
///
/// Com a copia velha, medido: a conexao do `inserir` cai sem resposta, e o
/// panico desenrola com a trava de escrita na mao. Veneno de `RwLock` e
/// permanente: dali em diante TODO pedido de dados, de TODA conexao, recebe
/// «uma operacao anterior entrou em panico e deixou a trava suja» ate o
/// processo reiniciar. Um usuario com direito de inserir numa tabela com
/// coluna binaria derrubava a base de todos.
#[test]
fn binario_que_corta_um_caractere_nao_envenena_a_trava_de_dados() {
    let base = DirTemp::novo("hex-bin");
    let (_s, porta) = subir_simples(&base);

    ok(
        falar(porta, &pedido(r#""op":"criar_database","database":"loja""#)),
        "criar_database",
    );
    ok(
        falar(
            porta,
            &pedido(
                r#""op":"criar_tabela","database":"loja","tabela":"anexos",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"dado","tipo":"Bin"}],
                   "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
            ),
        ),
        "criar_tabela",
    );

    let torto = falar(
        porta,
        &pedido(&format!(
            r#""op":"inserir","database":"loja","tabela":"anexos","linha":{{"id":1,"dado":"{}"}}"#,
            hex_que_corta_um_caractere()
        )),
    );

    // O DANO primeiro: a trava de dados, pedida por OUTRA conexao.
    let depois = falar(
        porta,
        &pedido(
            r#""op":"inserir","database":"loja","tabela":"anexos","linha":{"id":2,"dado":"00ff"}"#,
        ),
    )
    .expect("o inserir seguinte, por OUTRA conexao, caiu sem resposta");
    assert!(
        !depois.contains("trava suja"),
        "a trava de dados ficou ENVENENADA: o inserir seguinte, por OUTRA \
         conexao, recebeu {depois}"
    );
    assert!(
        Json::analisar(&depois).unwrap().booleano_ou("ok", false),
        "o inserir legitimo seguinte foi recusado: {depois}"
    );

    // E o pedido torto recebe uma recusa de tipo, e nao um fio cortado.
    let texto = torto.unwrap_or_else(|| {
        panic!("a conexao do inserir caiu SEM resposta: a thread morreu no hex_para_bytes")
    });
    let j = Json::analisar(&texto).unwrap();
    assert!(
        !j.booleano_ou("ok", true),
        "o binario torto entrou: {texto}"
    );
    assert!(
        j.texto_ou("erro", "").contains("hexadecimal"),
        "a recusa nao disse que era o hexadecimal: {texto}"
    );

    // So a linha legitima esta la.
    let lidas = ok(
        falar(
            porta,
            &pedido(r#""op":"varrer","database":"loja","tabela":"anexos","max":10"#),
        ),
        "varrer",
    );
    let n = lidas
        .campo("resultado")
        .and_then(|r| r.campo("linhas").or(Some(r)))
        .and_then(Json::lista)
        .map(<[Json]>::len);
    assert_eq!(n, Some(1), "{}", lidas.escrever());
}

/// **Pedido 453, pelo soquete: o valor torto nao volta inteiro nem vai ao
/// `acessos.log` inteiro.**
///
/// O dano e medido em BYTES nos dois lugares onde a mensagem de erro mora
/// depois de sair do motor: a linha de resposta e o crescimento do
/// `acessos.log`. Um megabyte de `z` numa coluna `Bin`, e o mesmo megabyte
/// numa coluna `Int8` -- o irmao pelo `json_para_valor`, que e o caminho de
/// todo `inserir`. Com o defeito, cada recusa escrevia o megabyte de volta no
/// fio E no disco: quem tem direito de inserir enchia o log a um megabyte por
/// pedido recusado.
#[test]
fn o_valor_torto_grande_nao_volta_inteiro_nem_vai_ao_log() {
    let base = DirTemp::novo("hex-eco");
    let (_s, porta) = subir_simples(&base);
    ok(
        falar(porta, &pedido(r#""op":"criar_database","database":"loja""#)),
        "criar_database",
    );
    ok(
        falar(
            porta,
            &pedido(
                r#""op":"criar_tabela","database":"loja","tabela":"anexos",
                   "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                              {"nome":"dado","tipo":"Bin"},
                              {"nome":"n","tipo":"Int8"}],
                   "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
            ),
        ),
        "criar_tabela",
    );
    let log = base.join("acessos.log");
    let mb = 1 << 20;
    let mut medidas = Vec::new();
    for (coluna, quem) in [("dado", "o Bin"), ("n", "o Int8")] {
        let antes = std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0);
        let resposta = falar(
            porta,
            &pedido(&format!(
                r#""op":"inserir","database":"loja","tabela":"anexos","linha":{{"id":7,"{coluna}":"{}"}}"#,
                "z".repeat(mb)
            )),
        )
        .unwrap_or_else(|| panic!("{quem}: a conexao caiu sem resposta"));
        assert!(
            !Json::analisar(&resposta).unwrap().booleano_ou("ok", true),
            "{quem}: o valor torto entrou"
        );
        // O servidor responde ANTES de anotar: espera a linha do log chegar.
        let ate = Instant::now() + Duration::from_secs(10);
        let mut cresceu = 0;
        while Instant::now() < ate {
            cresceu = std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0) - antes;
            if cresceu > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(cresceu > 0, "{quem}: a recusa nao foi anotada no log");
        eprintln!(
            "{quem}: resposta de {} bytes, acessos.log +{cresceu} bytes",
            resposta.len()
        );
        medidas.push((quem, resposta.len() as u64, cresceu));
    }
    // Os dois lugares, dos dois caminhos, de uma vez: o vermelho e a tabela
    // do dano, e nao so a primeira linha dela.
    let ecoam: Vec<String> = medidas
        .iter()
        .filter(|(_, fio, disco)| *fio >= 2048 || *disco >= 2048)
        .map(|(quem, fio, disco)| {
            format!("{quem}: resposta de {fio} bytes, acessos.log +{disco} bytes")
        })
        .collect();
    assert!(
        ecoam.is_empty(),
        "a recusa do valor torto de 1 MiB ecoou o valor: {ecoam:?}"
    );
}

/* ------------------------------------------ 3. o %XX da porta web, sem login */

fn subir_web(base: &std::path::Path) -> SocketAddr {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    // Duas vagas na web: se o panico vazasse a vaga, a terceira volta do
    // laco abaixo ja nao seria atendida.
    std::fs::write(
        &caminho,
        format!(
            r#"{{ "bind": "127.0.0.1:0", "token": "{TOKEN}", {caminhos},
                  "cifra_fio": {{ "exigir": false }},
                  "recursos": {{ "conexoes_web_max": 2 }},
                  "web": {{ "ligado": true, "bind": "127.0.0.1:0" }} }}"#,
            caminhos = caminhos(base),
        ),
    )
    .unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    // A REAL, lida do proprio servidor -- pedido 401.
    let web = comum::porta_real(|| s.porta_web());
    let alvo: SocketAddr = format!("127.0.0.1:{web}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(10);
    while Instant::now() < ate {
        if http(alvo, "/saude").starts_with("HTTP/1.1 200") {
            return alvo;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    panic!("a porta web nao subiu em {alvo}");
}

/// Um `GET` cru. Vazio = a conexao fechou sem uma linha de resposta.
fn http(alvo: SocketAddr, caminho: &str) -> String {
    let Ok(mut fluxo) = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)) else {
        return String::new();
    };
    let _ = fluxo.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = write!(fluxo, "GET {caminho} HTTP/1.1\r\nHost: x\r\n\r\n");
    let _ = fluxo.flush();
    let mut bruto = Vec::new();
    let _ = fluxo.read_to_end(&mut bruto);
    String::from_utf8_lossy(&bruto).into_owned()
}

/// **O `%` seguido de caractere de varios bytes, antes de qualquer login.**
///
/// O `/idiomas` e servido sem token (a tela de entrada precisa dos rotulos),
/// e o `parametro` desescapava o `%XX` fatiando o texto por byte. Com o
/// defeito reposto, medido: a conexao fecha sem uma linha de resposta. A vaga
/// da web volta (a `Permissao` morre no desenrolar -- pedido 248) e a porta
/// continua atendendo; o dano e a thread, nao a porta.
#[test]
fn percent_seguido_de_multibyte_no_idiomas_responde() {
    let base = DirTemp::novo("hex-web");
    let alvo = subir_web(&base);

    for volta in 0..4 {
        let r = http(alvo, "/idiomas?idioma=%€");
        assert!(
            r.starts_with("HTTP/1.1 200"),
            "volta {volta}: o GET /idiomas?idioma=%€ fechou SEM resposta -- a \
             thread da web morreu no desescapar, antes de qualquer login: {r:?}"
        );
        let r = http(alvo, "/idiomas?idioma=%a€");
        assert!(
            r.starts_with("HTTP/1.1 200"),
            "volta {volta}: o GET /idiomas?idioma=%a€ fechou SEM resposta: {r:?}"
        );
    }
    assert!(http(alvo, "/saude").starts_with("HTTP/1.1 200"));
}
