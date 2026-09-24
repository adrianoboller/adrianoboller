//! As portas HTTP sob `cifra_fio.exigir` -- provadas PELO SOQUETE.
//!
//! # O defeito que este arquivo existe para impedir (pedido 370)
//!
//! Medido em 18/09/2026, no mesmo servidor e no mesmo instante, com
//! `cifra_fio.exigir: true`: a porta nativa recusava (`[SP000025] peca o
//! aperto de mao`) e, ao lado dela, `POST /api {"op":"login"}` devolvia **200**
//! com a sessao aberta e a senha em claro; `POST /api {"op":"ping"}` com o
//! MESMO token que a porta nativa acabara de recusar devolvia 200;
//! `POST /v1/login` devolvia 200 e sessao; `POST /mcp initialize` devolvia o
//! catalogo inteiro; e o explorador da especificacao servia 14.009 bytes.
//!
//! O interruptor decidia num lugar so -- o laco da porta de DADOS -- e era
//! anunciado por todos. **Interruptor de seguranca se mede pelo que ele
//! RECUSA**, e e por isso que a prova aqui e pelo soquete: um teste de unidade
//! sobre o campo do `Config` mediria de novo o anuncio.
//!
//! # A prova real, nos dois sentidos
//!
//! Cada recusa mede o DANO, e nao o veredito: um login recusado que deixasse a
//! sessao nascer passaria num teste que so olha o codigo HTTP -- a armadilha
//! ja paga nesta casa, em que a conferencia acontecia depois do estrago. Por
//! isso `assert!(!corpo.contains("sessao"))`, e nao so o 403.
//!
//! E os dois sentidos contrarios estao aqui inteiros, porque proteger sem
//! saida e estrago: com `"exigir": false` as quatro portas voltam a atender, e
//! com o proxy TLS declarado (`"atras_de_proxy": true`) elas atendem COM a
//! exigencia ligada -- que e o desenho que o dono escolheu para o navegador.

mod comum;
use comum::DirTemp;

use std::io::{BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";
const LOGIN: &str = "ana";
/// A senha VIAJA em claro no `POST /api` -- e justamente o que esta frente
/// existe para impedir quando a cifra e exigida.
const SENHA: &str = "segredo123";

fn pasta(nome: &str) -> DirTemp {
    let d = DirTemp::novo(&format!("portas-http-{nome}"));
    std::fs::create_dir_all(d.join("base")).unwrap();
    d
}

/// As quatro portas de um servidor destes testes.
struct Portas {
    web: u16,
    rest: u16,
    swagger: u16,
}

/// Sobe um servidor com as TRES portas HTTP no ar, de um `config.json` de
/// verdade.
///
/// `cifra_fio` entra como texto para o chamador decidir o que o arquivo diz --
/// inclusive NAO dizer nada, que e o `config.json` de quem so trocou o
/// binario. Montar o `Config` na mao escreveria o campo, e teste que escreve o
/// campo nao prova o padrao dele: a armadilha ja paga no `cifra-do-fio.rs`.
///
/// Uma iteracao de PBKDF2 no hash da senha: a senha real nao interessa a este
/// arquivo, e 210.000 iteracoes por login fariam a bateria levar segundos por
/// nada.
fn subir(base: &std::path::Path, cifra_fio: &str, atras_de_proxy: bool) -> (Arc<Servidor>, Portas) {
    let proxy = if atras_de_proxy {
        r#", "atras_de_proxy": true"#
    } else {
        ""
    };
    let h = phxsql_core::senha::cifrar_com(SENHA, 1);
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0",
              "base": "{}",
              "token": "{TOKEN}",
              "log_acessos": "{}",
              "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}",
              "jobs": "{}",
              "usuarios": [
                {{ "id": 2, "login": "{LOGIN}", "nome": "Ana", "senha_hash": "{h}",
                   "ativo": true, "supervisor": true }} ],
              "web":  {{ "ligado": true, "bind": "127.0.0.1:0"{proxy} }},
              "rest": {{ "ligado": true, "bind": "127.0.0.1:0",
                         "swagger_ligado": true,
                         "swagger_bind": "127.0.0.1:0"{proxy} }}{cifra_fio}
            }}"#,
            bar(base.join("base")),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let c = Config::ler(&caminho).unwrap();
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    // As quatro portas REAIS, lidas do proprio servidor -- pedido 401. O
    // `config.json` pediu "0" nas quatro; escolher um numero por fora e
    // solta-lo antes do `bind` de verdade era exatamente a janela que os dois
    // vermelhos deste arquivo (registrados no pedido 401) suspeitavam.
    let portas = Portas {
        web: comum::porta_real(|| s.porta_web()),
        rest: comum::porta_real(|| s.porta_rest()),
        swagger: comum::porta_real(|| s.porta_swagger()),
    };
    // Por CONDICAO, nunca por tempo fixo: dormir passa nesta maquina e falha
    // na proxima. As tres portas, porque as tres sao provadas.
    for porta in [portas.web, portas.rest, portas.swagger] {
        esperar(porta);
    }
    (s, portas)
}

fn esperar(porta: u16) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("a porta {porta} nao abriu em 5 s");
}

/// Um pedido HTTP cru, e a resposta como (codigo, corpo).
fn http(
    porta: u16,
    metodo: &str,
    caminho: &str,
    cabecalhos: &[(&str, &str)],
    corpo: &str,
) -> (u16, String) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let extras: String = cabecalhos
        .iter()
        .map(|(n, v)| format!("{n}: {v}\r\n"))
        .collect();
    let _ = write!(
        escrita,
        "{metodo} {caminho} HTTP/1.1\r\nHost: localhost\r\n{extras}\
         Content-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{corpo}",
        corpo.len()
    );
    let _ = escrita.flush();
    let mut resposta = String::new();
    let _ = BufReader::new(fluxo).read_to_string(&mut resposta);
    let codigo = resposta
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem codigo: {resposta:?}"));
    let corpo = resposta
        .split_once("\r\n\r\n")
        .map(|(_, c)| c.to_string())
        .unwrap_or_default();
    (codigo, corpo)
}

fn api(porta: u16, corpo: &str) -> (u16, String) {
    http(porta, "POST", "/api", &[], corpo)
}

fn rest(porta: u16, caminho: &str, corpo: &str) -> (u16, String) {
    http(
        porta,
        "POST",
        caminho,
        &[("Authorization", &format!("Bearer {TOKEN}"))],
        corpo,
    )
}

fn mcp(porta: u16) -> (u16, String) {
    http(
        porta,
        "POST",
        "/mcp",
        &[("Authorization", &format!("Bearer {TOKEN}"))],
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
    )
}

fn login_em_claro() -> String {
    format!(r#"{{"token":"{TOKEN}","op":"login","usuario":"{LOGIN}","senha":"{SENHA}"}}"#)
}

/// A recusa da cifra, conferida pelo que ela DIZ.
///
/// Erro nomeado e com a saida escrita: e a unica coisa que quem esta do outro
/// lado recebe, e um 403 seco mandaria procurar a permissao errada.
fn recusa_nomeada(codigo: u16, corpo: &str, secao: &str) {
    assert_eq!(codigo, 403, "esperava recusa, veio {codigo}: {corpo}");
    assert!(
        corpo.contains("cifra_fio.exigir"),
        "a recusa nao nomeia o interruptor: {corpo}"
    );
    assert!(
        corpo.contains("atras_de_proxy") && corpo.contains(secao),
        "a recusa nao diz como por o proxy na secao {secao}: {corpo}"
    );
}

// ---------------------------------------------------------------------------
// O teste que mais importa
// ---------------------------------------------------------------------------

/// **Com a cifra exigida, as quatro portas HTTP recusam o texto claro.**
///
/// As quatro do inventario de 18/09/2026, uma a uma, e cada uma medindo o
/// dano: o `/api`, o `/v1`, o `/mcp` (que viaja na porta do REST e ninguem
/// tinha listado) e o explorador da especificacao.
///
/// # Como repor o defeito e ver o vermelho
///
/// Tire a primeira conferencia do `portao_de_rede_http` (o
/// `cifra_fio.exigir && !proxy`): o login volta a 200 com `"sessao"` no corpo
/// e a senha em claro no fio, com a porta de dados recusando ao lado.
#[test]
fn com_a_cifra_exigida_as_portas_http_recusam_e_dizem_o_que_fazer() {
    let base = pasta("recusa");
    let (_s, p) = subir(&base, r#", "cifra_fio": { "exigir": true }"#, false);

    // (a) o ping com o token que a porta de dados acabara de recusar.
    let (codigo, corpo) = api(p.web, &format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    recusa_nomeada(codigo, &corpo, "web");

    // (b) o login com a senha em claro -- e aqui se mede o DANO, nao o
    // veredito: a sessao nao pode ter nascido.
    let (codigo, corpo) = api(p.web, &login_em_claro());
    recusa_nomeada(codigo, &corpo, "web");
    assert!(
        !corpo.contains("sessao"),
        "a sessao nasceu apesar da recusa: {corpo}"
    );
    assert!(
        !corpo.contains("\"ok\":true"),
        "a recusa veio embrulhada num sucesso: {corpo}"
    );

    // (c) o REST.
    let (codigo, corpo) = rest(
        p.rest,
        "/v1/login",
        &format!(r#"{{"usuario":"{LOGIN}","senha":"{SENHA}"}}"#),
    );
    recusa_nomeada(codigo, &corpo, "rest");
    assert!(
        !corpo.contains("sessao"),
        "a sessao nasceu pelo REST apesar da recusa: {corpo}"
    );

    // (d) o MCP sobre HTTP, que entra pela porta do REST.
    let (codigo, corpo) = mcp(p.rest);
    recusa_nomeada(codigo, &corpo, "rest");
    assert!(
        !corpo.contains("serverInfo") && !corpo.contains("protocolVersion"),
        "o aperto de mao do MCP aconteceu apesar da recusa: {corpo}"
    );

    // (e) o explorador da especificacao -- 14.009 bytes na medicao de 18/09.
    let (codigo, corpo) = http(p.swagger, "GET", "/", &[], "");
    recusa_nomeada(codigo, &corpo, "rest");
    assert!(
        !corpo.contains("openapi") && corpo.len() < 2_000,
        "o explorador serviu a especificacao apesar da recusa ({} bytes)",
        corpo.len()
    );

    // E o que a recusa NAO pode fazer: contar o pedido como violacao e banir
    // quem so esta configurado do jeito de ontem. A porta continua atendendo
    // (para recusar) no pedido seguinte.
    let (codigo, _) = api(p.web, &format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    assert_eq!(codigo, 403, "a segunda tentativa mudou de tratamento");
}

/// **O comportamento VELHO: sem a exigencia, as quatro portas atendem.**
///
/// E o teste que mais importa numa guarda nova, e o motivo esta na lei da
/// casa: protecao que quebra todo cliente antigo nao e protecao, e estrago.
/// Quem escreve `"exigir": false` continua exatamente como antes -- e este
/// arquivo e o que prova que o escape escrito alcanca tambem as portas HTTP, e
/// nao so a de dados.
#[test]
fn com_o_escape_escrito_as_portas_http_continuam_como_antes() {
    let base = pasta("escape");
    let (_s, p) = subir(&base, r#", "cifra_fio": { "exigir": false }"#, false);

    let (codigo, corpo) = api(p.web, &format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    assert_eq!(codigo, 200, "{corpo}");

    let (codigo, corpo) = api(p.web, &login_em_claro());
    assert_eq!(codigo, 200, "{corpo}");
    assert!(
        corpo.contains("sessao"),
        "o login em claro tinha de abrir sessao aqui: {corpo}"
    );

    let (codigo, corpo) = rest(
        p.rest,
        "/v1/login",
        &format!(r#"{{"usuario":"{LOGIN}","senha":"{SENHA}"}}"#),
    );
    assert_eq!(codigo, 200, "{corpo}");
    assert!(corpo.contains("sessao"), "{corpo}");

    let (codigo, corpo) = mcp(p.rest);
    assert_eq!(codigo, 200, "{corpo}");
    assert!(corpo.contains("protocolVersion"), "{corpo}");

    let (codigo, corpo) = http(p.swagger, "GET", "/", &[], "");
    assert_eq!(codigo, 200, "{corpo}");
    assert!(
        corpo.len() > 2_000,
        "o explorador veio vazio: {}",
        corpo.len()
    );
}

/// **O proxy declarado e o escape escrito das portas HTTP.**
///
/// O dono decidiu o meio: o TLS do navegador e terminado por proxy reverso na
/// frente, e o motor continua zero dependencias (`docs/SEGURANCA.md` §7.1).
/// Sem este sentido, ligar a exigencia mataria a interface web de quem faz a
/// coisa certa -- e uma guarda que nao tem saida nenhuma vira o interruptor
/// que ninguem liga.
///
/// `"atras_de_proxy": true` nao e conferivel, e e por isso que ele e uma
/// DECLARACAO escrita: a mesma forma do `"exigir": false` e do
/// `"verificar": false` da chave conferida -- escolha escrita em vez de
/// omissao.
#[test]
fn o_proxy_declarado_deixa_as_portas_http_atenderem_com_a_cifra_exigida() {
    let base = pasta("proxy");
    let (_s, p) = subir(&base, r#", "cifra_fio": { "exigir": true }"#, true);

    let (codigo, corpo) = api(p.web, &format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#));
    assert_eq!(codigo, 200, "{corpo}");

    let (codigo, corpo) = rest(
        p.rest,
        "/v1/login",
        &format!(r#"{{"usuario":"{LOGIN}","senha":"{SENHA}"}}"#),
    );
    assert_eq!(codigo, 200, "{corpo}");

    let (codigo, corpo) = mcp(p.rest);
    assert_eq!(codigo, 200, "{corpo}");

    // O explorador entra pelo `atras_de_proxy` do REST: as duas portas da
    // secao sobem do mesmo bloco e do mesmo operador.
    let (codigo, corpo) = http(p.swagger, "GET", "/", &[], "");
    assert_eq!(codigo, 200, "{corpo}");
}

/// **O `config.json` de quem so trocou o binario: sem a secao `cifra_fio`, a
/// porta HTTP recusa.**
///
/// E a prova do PADRAO, e ela tem de ler um arquivo que nao declara o campo:
/// teste que escreve o campo nao prova o padrao dele. Se algum dia alguem
/// devolver `exigir` para `false` de fabrica, e este teste que fica vermelho.
#[test]
fn sem_a_secao_cifra_fio_a_porta_http_ja_nasce_recusando() {
    let base = pasta("padrao");
    let (_s, p) = subir(&base, "", false);

    let (codigo, corpo) = api(p.web, &login_em_claro());
    recusa_nomeada(codigo, &corpo, "web");
    assert!(!corpo.contains("sessao"), "{corpo}");
}

/// **O campo que mentia: `encryption_exigida` diz a verdade DESTA conexao.**
///
/// Ate 18/09/2026 ele publicava `cifra_fio.exigir` para quem quer que
/// perguntasse -- inclusive para a conexao HTTP em claro que fazia a pergunta,
/// dentro de uma funcao cuja documentacao diz «o que e verdade DESTA conexao».
/// Campo que declara protecao maior que a prestada e a familia do
/// `recursos.cache_paginas`, que anunciava cache sem haver cache, e aqui e
/// pior: alguem liga o interruptor e vai dormir confiando nele.
///
/// O cenario e o unico em que a pergunta pode ser feita por HTTP com a
/// exigencia ligada: o proxy declarado. **Com o defeito reposto, este teste
/// fica vermelho aqui** -- era exatamente este o caso que respondia `true`.
///
/// A outra metade -- a conexao de dados DENTRO do tunel, que responde `true` --
/// vive no `cifra-do-fio.rs`, que e quem tem o cliente do aperto de mao.
#[test]
fn a_diretiva_da_conexao_http_nao_anuncia_cifra_que_nao_ha() {
    let base = pasta("diretiva");
    let (_s, p) = subir(&base, r#", "cifra_fio": { "exigir": true }"#, true);

    let (codigo, corpo) = api(p.web, &login_em_claro());
    assert_eq!(codigo, 200, "{corpo}");
    let sessao = Json::analisar(&corpo)
        .unwrap()
        .texto_ou("sessao", "")
        .to_string();
    assert!(
        !sessao.is_empty(),
        "sem sessao nao da para perguntar: {corpo}"
    );

    let (codigo, corpo) = http(
        p.web,
        "POST",
        "/api",
        &[("X-Sessao", &sessao)],
        &format!(r#"{{"token":"{TOKEN}","op":"diretivas","escopo":"conexao"}}"#),
    );
    assert_eq!(codigo, 200, "{corpo}");
    let r = Json::analisar(&corpo).unwrap();
    let d = r.campo("resultado").unwrap_or(&r);
    assert_eq!(d.texto_ou("escopo", ""), "conexao", "{corpo}");
    // A porta por onde esta conexao entrou, dita em voz alta: sem ela, o
    // `false` de baixo seria um numero que quem le nao sabe interpretar.
    assert_eq!(d.texto_ou("via", ""), "http", "{corpo}");
    assert!(
        !d.booleano_ou("encryption_exigida", true),
        "a conexao HTTP em claro recebeu encryption_exigida=true: {corpo}"
    );
    assert!(
        !d.booleano_ou("encryption_neste_canal", true),
        "a conexao HTTP se declarou dentro do tunel: {corpo}"
    );
    // E a CAPACIDADE do servidor continua sendo dita, no campo que sempre a
    // disse: quem pergunta «este servidor atende o aperto?» continua sabendo.
    assert!(d.booleano_ou("encryption", false), "{corpo}");
}
