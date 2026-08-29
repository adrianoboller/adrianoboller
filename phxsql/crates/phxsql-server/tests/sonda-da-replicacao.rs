//! A sonda da replicacao -- provada PELO SOQUETE, entre dois servidores.
//!
//! `replicacao_sondar` existe para o assistente de replicacao testar a conexao
//! com o outro servidor ANTES de configurar qualquer coisa, pelo MESMO caminho
//! que o laco da replica vai usar. Teste unitario nao prova esse caminho: o
//! que se quer saber e se um servidor consegue mesmo abrir a conexao,
//! apresentar o token e ler a posicao do outro -- e isso so um soquete de
//! verdade mostra (a licao do `BULKINSERT`).
//!
//! O tropeco que este arquivo ja pagou por existir: a primeira versao da op
//! lia o token do OUTRO servidor do campo `token` -- o mesmo campo que
//! autentica quem pede AQUI. Um cliente TCP nao tem como mandar dois valores
//! no mesmo nome, e o JSON ficaria com um deles em silencio. Dai o
//! `token_remoto`, e o teste que confere os dois papeis no mesmo pedido.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_server::{Config, Servidor};

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn subir_servidor(base: &std::path::Path, porta: u16, token: &str) -> Arc<Servidor> {
    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: token.into(),
        ..Default::default()
    };
    c.web.ligado = false;
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar_porta(porta).expect("o servidor nao subiu");
    s
}

fn esperar_porta(porta: u16) -> Result<(), String> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(2);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("a porta {porta} nao abriu em 2 s"))
}

fn pedir(porta: u16, linha: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("nao conectei em {porta}: {e}"));
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{}", linha.replace('\n', " ")).unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    resposta
}

fn pasta(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phxsql-sonda-{}-{}-{nome}",
        std::process::id(),
        porta_livre()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// O caminho feliz e as recusas que importam, no mesmo cenario -- subir um
/// par de servidores por assert seria pagar o arranque quatro vezes.
#[test]
fn a_sonda_le_o_outro_servidor_pelo_soquete() {
    let base_a = pasta("outro");
    let base_b = pasta("daqui");
    let porta_a = porta_livre();
    let porta_b = porta_livre();
    let _a = subir_servidor(&base_a, porta_a, "token-do-outro");
    let _b = subir_servidor(&base_b, porta_b, "token-daqui");

    // O "outro lado" ganha um banco com duas linhas: e a posicao que a sonda
    // tem de enxergar de fora.
    let t = r#""token":"token-do-outro""#;
    assert!(pedir(
        porta_a,
        &format!(r#"{{{t},"op":"criar_database","database":"loja"}}"#)
    )
    .contains(r#""ok":true"#));
    assert!(pedir(
        porta_a,
        &format!(
            r#"{{{t},"op":"criar_tabela","database":"loja","tabela":"clientes",
                "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}}]}}"#
        )
    )
    .contains(r#""ok":true"#));
    for id in 1..=2 {
        assert!(pedir(
            porta_a,
            &format!(
                r#"{{{t},"op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id}}}}}"#
            )
        )
        .contains(r#""ok":true"#));
    }

    // O caminho feliz: o servidor "daqui" abre a conexao para o outro, se
    // apresenta com o token DELE e le a posicao. Os numeros tem de ser os
    // de verdade -- duas linhas inseridas sao dois eventos e dois registros.
    let resposta = pedir(
        porta_b,
        &format!(
            r#"{{"token":"token-daqui","op":"replicacao_sondar",
                "host":"127.0.0.1","porta":{porta_a},"token_remoto":"token-do-outro"}}"#
        ),
    );
    assert!(
        resposta.contains(r#""ok":true"#),
        "a sonda falhou: {resposta}"
    );
    assert!(resposta.contains(r#""papel":"isolado""#), "{resposta}");
    assert!(resposta.contains("loja"), "faltou o banco: {resposta}");
    assert!(resposta.contains(r#""eventos":2"#), "{resposta}");
    assert!(resposta.contains(r#""registros":2"#), "{resposta}");
    // A credencial nao volta: nem o token do outro, nem pedaco dele.
    assert!(
        !resposta.contains("token-do-outro"),
        "a resposta vazou o token remoto: {resposta}"
    );

    // Sem o token do outro, o OUTRO recusa -- e a sonda repassa a recusa em
    // vez de inventar uma resposta propria.
    let resposta = pedir(
        porta_b,
        &format!(
            r#"{{"token":"token-daqui","op":"replicacao_sondar",
                "host":"127.0.0.1","porta":{porta_a}}}"#
        ),
    );
    assert!(
        resposta.contains(r#""ok":false"#),
        "sondar sem o token do outro devia falhar: {resposta}"
    );

    // Sem host nem origem nao ha o que sondar, e o erro diz o que falta.
    let resposta = pedir(
        porta_b,
        r#"{"token":"token-daqui","op":"replicacao_sondar"}"#,
    );
    assert!(resposta.contains(r#""ok":false"#), "{resposta}");
    assert!(
        resposta.contains("host"),
        "o erro nao diz o que falta: {resposta}"
    );

    // E quem pede sem o token DAQUI nao chega nem a sonda: os dois campos de
    // token tem papeis diferentes, e este e o teste que trava a diferenca.
    let resposta = pedir(
        porta_b,
        &format!(
            r#"{{"op":"replicacao_sondar",
                "host":"127.0.0.1","porta":{porta_a},"token_remoto":"token-do-outro"}}"#
        ),
    );
    assert!(
        resposta.contains(r#""ok":false"#),
        "sem o token local o pedido devia ser recusado: {resposta}"
    );

    let _ = std::fs::remove_dir_all(&base_a);
    let _ = std::fs::remove_dir_all(&base_b);
}
