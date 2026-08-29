//! O transporte MCP -- provado PELO PROCESSO, e nao pela funcao.
//!
//! Os quinze testes do `mcp.rs` chamam `Ponte::atender` e conferem a resposta.
//! Nenhum deles enxerga o que este arquivo enxerga: se a resposta SAI do
//! processo enquanto o cliente ainda esta escrevendo. Um cliente MCP manda uma
//! mensagem e ESPERA, com a entrada aberta -- e um laco que le tudo antes de
//! responder trava os dois lados sem erro em lugar nenhum.
//!
//! E a mesma licao do `BULKINSERT`: o que depende do sistema operacional se
//! prova contra o sistema operacional. Ali foi o soquete; aqui e o cano.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

/// Uma pasta so deste teste, com um `config.json` dentro.
fn preparar(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-mcp-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let config = format!(
        r#"{{"bind":"127.0.0.1:0","base":{:?},"token":"t","web":{{"ligado":false}}}}"#,
        d.join("dados").display().to_string()
    );
    std::fs::write(d.join("config.json"), config).unwrap();
    d
}

/// Sobe `phxsqld --mcp` com a entrada e a saida em canos.
fn subir(dir: &std::path::Path, extras: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_phxsqld"))
        .arg("--config")
        .arg(dir.join("config.json"))
        .arg("--mcp")
        .args(extras)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("nao consegui subir o phxsqld --mcp")
}

/// Manda as linhas e devolve as respostas, uma por linha.
///
/// A entrada e fechada ANTES de ler tudo de proposito: e assim que um cliente
/// MCP termina a sessao, e e o que prova que o laco sai no EOF em vez de
/// pendurar.
fn conversar(filho: &mut Child, linhas: &[&str]) -> Vec<phxsql_core::json::Json> {
    let mut entrada = filho.stdin.take().unwrap();
    for l in linhas {
        writeln!(entrada, "{l}").unwrap();
    }
    entrada.flush().unwrap();
    drop(entrada);

    let saida = BufReader::new(filho.stdout.take().unwrap());
    saida
        .lines()
        .map(|l| phxsql_core::json::Json::analisar(&l.unwrap()).expect("resposta nao e JSON"))
        .collect()
}

/// O aperto de mao inteiro, como um cliente MCP de verdade o faz: `initialize`,
/// a notificacao `initialized` (que NAO recebe resposta), `tools/list` e um
/// `tools/call`.
#[test]
fn o_aperto_de_mao_completo_atravessa_o_cano() {
    let dir = preparar("aperto");
    let mut filho = subir(&dir, &[]);
    let respostas = conversar(
        &mut filho,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"phx_bancos","arguments":{}}}"#,
        ],
    );
    let _ = filho.wait();

    // TRES respostas: a notificacao e a linha em branco nao geram nenhuma.
    // Se gerassem, o cliente veria uma resposta a mais e perderia o passo.
    assert_eq!(
        respostas.len(),
        3,
        "a notificacao ou a linha em branco geraram resposta: {respostas:?}"
    );

    let init = respostas[0].campo("result").expect("initialize falhou");
    assert_eq!(init.texto_ou("protocolVersion", ""), "2025-06-18");
    assert_eq!(
        init.campo("serverInfo").unwrap().texto_ou("name", ""),
        "phxsql"
    );

    let tools = respostas[1]
        .campo("result")
        .unwrap()
        .campo("tools")
        .and_then(phxsql_core::json::Json::lista)
        .unwrap();
    let nomes: Vec<String> = tools
        .iter()
        .map(|t| t.texto_ou("name", "").to_string())
        .collect();
    assert!(nomes.contains(&"phx_esquema".to_string()), "{nomes:?}");
    assert!(nomes.contains(&"phx_sql".to_string()), "{nomes:?}");
    // Somente leitura vem ligado: as que escrevem nao sao nem anunciadas.
    assert!(!nomes.contains(&"phx_inserir".to_string()), "{nomes:?}");
    // E toda ferramenta anunciada traz o esquema que o MCP exige.
    for t in tools {
        let e = t.campo("inputSchema").unwrap();
        assert_eq!(e.texto_ou("type", ""), "object");
        assert!(!t.texto_ou("description", "").is_empty());
    }

    // A chamada foi ao servidor de verdade: a pasta acabou de nascer e nao tem
    // banco nenhum, entao a resposta e a lista vazia do PhxSql -- e nao um
    // erro de ponte, que e o que apareceria se o executor nao estivesse ligado.
    let chamada = respostas[2].campo("result").unwrap();
    assert_eq!(chamada.campo("isError").unwrap().booleano(), Some(false));
    let texto = chamada.campo("content").unwrap().lista().unwrap()[0]
        .texto_ou("text", "")
        .to_string();
    assert_eq!(
        phxsql_core::json::Json::analisar(&texto)
            .expect("o conteudo devolvido nao e JSON")
            .lista()
            .map(<[phxsql_core::json::Json]>::len),
        Some(0),
        "{texto}"
    );
}

/// **A prova de que a ponte responde linha a linha.** O cliente escreve UMA
/// mensagem e fica esperando a resposta com a entrada ainda aberta -- que e
/// como um cliente MCP vive a conversa inteira.
///
/// Trocar o laco por `lines().collect()` -- o que se escreve sem pensar -- faz
/// este teste PENDURAR ate o tempo estourar, e foi assim que ele se provou.
#[test]
fn a_resposta_sai_antes_de_a_entrada_fechar() {
    let dir = preparar("flush");
    let mut filho = subir(&dir, &[]);
    let mut entrada = filho.stdin.take().unwrap();
    let mut saida = BufReader::new(filho.stdout.take().unwrap());

    writeln!(entrada, r#"{{"jsonrpc":"2.0","id":1,"method":"ping"}}"#).unwrap();
    entrada.flush().unwrap();

    let mut linha = String::new();
    saida.read_line(&mut linha).unwrap();
    let r = phxsql_core::json::Json::analisar(&linha).unwrap();
    assert_eq!(r.campo("id").unwrap().inteiro(), Some(1));
    assert!(r.campo("result").is_some(), "{linha}");

    // E a conversa continua: a mesma conexao aceita a proxima mensagem.
    writeln!(
        entrada,
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#
    )
    .unwrap();
    entrada.flush().unwrap();
    let mut linha = String::new();
    saida.read_line(&mut linha).unwrap();
    assert_eq!(
        phxsql_core::json::Json::analisar(&linha)
            .unwrap()
            .campo("id")
            .unwrap()
            .inteiro(),
        Some(2)
    );

    drop(entrada);
    let _ = filho.wait();
}

/// A ferramenta que escreve nao existe para a ponte padrao, e a recusa diz POR
/// QUE -- fingir que ela nao existe deixaria o modelo tentando de novo.
#[test]
fn sem_escrita_a_ferramenta_de_gravar_e_recusada_com_o_motivo() {
    let dir = preparar("leitura");
    let mut filho = subir(&dir, &[]);
    let r = conversar(
        &mut filho,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"phx_inserir","arguments":{"database":"d","tabela":"t","valores":{}}}}"#,
        ],
    );
    let _ = filho.wait();
    let msg = r[0]
        .campo("error")
        .expect("a escrita passou")
        .texto_ou("message", "")
        .to_string();
    assert!(msg.contains("somente de leitura"), "{msg}");
}

/// Com `--escrita`, a mesma ferramenta aparece e chega ao servidor.
#[test]
fn com_escrita_a_ferramenta_aparece_no_anuncio() {
    let dir = preparar("escrita");
    let mut filho = subir(&dir, &["--escrita"]);
    let r = conversar(
        &mut filho,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#],
    );
    let _ = filho.wait();
    let nomes: Vec<String> = r[0]
        .campo("result")
        .unwrap()
        .campo("tools")
        .and_then(phxsql_core::json::Json::lista)
        .unwrap()
        .iter()
        .map(|t| t.texto_ou("name", "").to_string())
        .collect();
    assert!(nomes.contains(&"phx_inserir".to_string()), "{nomes:?}");
}

/// Linha que nao e JSON nao derruba a sessao: responde o erro do padrao e a
/// proxima mensagem continua valendo. Um cliente que manda lixo uma vez nao
/// pode perder a conversa inteira.
#[test]
fn lixo_no_cano_nao_derruba_a_sessao() {
    let dir = preparar("lixo");
    let mut filho = subir(&dir, &[]);
    let r = conversar(
        &mut filho,
        &[
            "isto nao e json",
            r#"{"jsonrpc":"2.0","id":9,"method":"ping"}"#,
        ],
    );
    let _ = filho.wait();
    assert_eq!(r.len(), 2);
    assert_eq!(
        r[0].campo("error")
            .unwrap()
            .campo("code")
            .unwrap()
            .inteiro(),
        Some(-32_700)
    );
    assert_eq!(r[1].campo("id").unwrap().inteiro(), Some(9));
    assert!(r[1].campo("result").is_some());
}
