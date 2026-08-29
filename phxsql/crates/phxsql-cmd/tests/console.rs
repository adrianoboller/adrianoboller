//! O console -- provado PELO SOQUETE, contra um servidor de verdade.
//!
//! Um console que se testa com um cliente de mentira prova que ele monta o
//! pedido que ele mesmo espera. O que interessa e outra coisa: se o pedido
//! chega, se o desafio-resposta passa, se o `/help` traz o catalogo DAQUELE
//! servidor e se a permissao continua valendo do outro lado.
//!
//! E a mesma licao do `BULKINSERT`: o que depende do outro lado se prova
//! contra o outro lado.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use phxsql_cmd::Console;
use phxsql_core::json::Json;
use phxsql_server::servidor::Servidor;
use phxsql_server::{Cadastro, Config};

const TOKEN: &str = "token-do-console";

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn pasta(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-cmd-{nome}-{}-{}",
        std::process::id(),
        porta_livre()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Um servidor de verdade, com uma base `loja` de tres clientes.
///
/// `cadastro` vazio quer dizer servidor sem usuarios -- o token de servico
/// entra e pode tudo, que e o modo em que a maioria dos testes roda.
fn subir(dir: &std::path::Path, porta: u16, cadastro: Cadastro) -> Arc<Servidor> {
    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: TOKEN.into(),
        cadastro,
        ..Default::default()
    };
    c.web.ligado = false;
    let s = Servidor::novo(c).unwrap();

    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar(porta);
    s
}

/// As tabelas nascem pela PORTA, e nao pela API: assim o teste tambem exercita
/// o caminho que o console vai usar.
fn povoar(porta: u16) {
    povoar_como(porta, None);
}

/// Num servidor COM cadastro o token de servico nao basta: o login e
/// obrigatorio, e povoar tem de entrar como alguem que pode criar.
fn povoar_como(porta: u16, quem: Option<(&str, &str)>) {
    let mut c = Console::ligar("127.0.0.1", porta, TOKEN, Duration::from_secs(5)).unwrap();
    if let Some((u, senha)) = quem {
        c.entrar(u, senha).expect("o login de quem povoa falhou");
    }
    for linha in [
        "criar_database database=loja",
        r#"{"op":"criar_tabela","database":"loja","tabela":"clientes",
            "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                       {"nome":"nome","tipo":"Str(30)"},
                       {"nome":"cidade","tipo":"Str(30)"}],
            "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        r#"{"op":"criar_tabela","database":"loja","tabela":"folha",
            "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
            "indices":[{"nome":"porId","colunas":["id"],"unico":true}]}"#,
    ] {
        let r = c.executar_linha(linha);
        assert!(!r.texto().starts_with("erro"), "{linha} -> {}", r.texto());
    }
    for (id, nome, cidade) in [
        (1, "Adriano", "Blumenau"),
        (2, "Ana Maria", "Joinville"),
        (3, "Joao", "Blumenau"),
    ] {
        let r = c.executar_linha(&format!(
            r#"inserir database=loja tabela=clientes valores={{"id":{id},"nome":"{nome}","cidade":"{cidade}"}}"#
        ));
        assert!(!r.texto().starts_with("erro"), "{}", r.texto());
    }
}

fn esperar(porta: u16) {
    let ate = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < ate {
        if std::net::TcpStream::connect(format!("127.0.0.1:{porta}")).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

fn console(porta: u16) -> Console {
    Console::ligar("127.0.0.1", porta, TOKEN, Duration::from_secs(5)).unwrap()
}

/// A linha digitada vira pedido, atravessa o soquete e volta como tabela.
#[test]
fn a_linha_vira_pedido_e_a_resposta_vira_tabela() {
    let dir = pasta("tabela");
    let porta = porta_livre();
    let _s = subir(&dir, porta, Cadastro::default());
    povoar(porta);

    let mut c = console(porta);
    c.executar_linha("/use loja");

    let t = c
        .executar_linha("varrer tabela=clientes")
        .texto()
        .to_string();
    // Cabecalho, tracos e tres linhas -- e o nome com espaco inteiro.
    assert!(t.contains("nome"), "{t}");
    assert!(t.contains("Ana Maria"), "{t}");
    assert!(t.contains("Blumenau"), "{t}");
    // **Nao muda a caixa do dado.** «BLUMENAU» na tela onde esta gravado
    // «Blumenau» e uma mentira sobre o dado.
    assert!(!t.contains("BLUMENAU"), "a tabela gritou o dado: {t}");
    // E os escalares da resposta viram a linha de resumo.
    assert!(t.contains("devolvidas: 3"), "{t}");
}

/// O `/use` preenche o `database`, e o que foi digitado vence o padrao.
#[test]
fn o_use_preenche_o_banco_e_o_digitado_vence() {
    let dir = pasta("use");
    let porta = porta_livre();
    let _s = subir(&dir, porta, Cadastro::default());
    povoar(porta);

    let mut c = console(porta);
    // Sem /use, a operacao que precisa de banco recusa.
    assert!(
        c.executar_linha("tabelas").texto().starts_with("erro"),
        "achou banco sem ninguem dizer qual"
    );
    c.executar_linha("/use loja");
    assert!(c.executar_linha("tabelas").texto().contains("clientes"));
    // O digitado vence o corrente: o /use e padrao, nao imposicao.
    assert!(c
        .executar_linha("tabelas database=nao_existe")
        .texto()
        .starts_with("erro"));
}

/// **O `/help` vem do servidor, e nao de uma lista escrita no console.**
/// Uma lista aqui envelheceria calada e o console documentaria um servidor que
/// ja mudou.
#[test]
fn o_help_vem_do_catalogo_pela_rede() {
    let dir = pasta("help");
    let porta = porta_livre();
    let _s = subir(&dir, porta, Cadastro::default());

    let mut c = console(porta);
    let lista = c.executar_linha("/help").texto().to_string();
    assert!(lista.contains("operacao"), "{lista}");
    assert!(lista.contains("buscar"), "{lista}");
    assert!(lista.contains("criar_tabela"), "{lista}");
    assert!(lista.contains("operacoes;"), "faltou o rodape: {lista}");

    let uma = c.executar_linha("/help buscar").texto().to_string();
    assert!(uma.contains("indice"), "{uma}");
    assert!(
        uma.contains("obrigatorio") || uma.contains("exige"),
        "{uma}"
    );
    assert!(uma.contains("exemplo:"), "{uma}");
    assert!(uma.contains("permissao: ler"), "{uma}");

    // Operacao que nao existe diz isso, e nao devolve tabela vazia.
    let nada = c
        .executar_linha("/help nao_existe_mesmo")
        .texto()
        .to_string();
    assert!(nada.contains("nao existe"), "{nada}");
}

/// SELECT digitado direto vira a op `sql`. Num console de banco, quem escreve
/// SELECT quer consultar -- e nao escrever `sql texto=...`.
#[test]
fn select_digitado_direto_vira_a_op_sql() {
    let dir = pasta("select");
    let porta = porta_livre();
    let _s = subir(&dir, porta, Cadastro::default());
    povoar(porta);

    let mut c = console(porta);
    c.executar_linha("/use loja");
    let t = c
        .executar_linha("SELECT nome FROM clientes WHERE id = 2")
        .texto()
        .to_string();
    assert!(t.contains("Ana Maria"), "{t}");
    assert!(t.contains("op: buscar"), "faltou dizer no que virou: {t}");

    // E o erro de sintaxe chega com a coluna, em vez de um «nao entendi».
    let e = c
        .executar_linha("SELECT * FRON clientes")
        .texto()
        .to_string();
    assert!(e.contains("coluna"), "{e}");
}

/// O `/cru` mostra o JSON como ele veio -- que e o que se quer quando a
/// duvida e sobre a RESPOSTA e nao sobre o dado.
#[test]
fn o_cru_alterna_entre_tabela_e_json() {
    let dir = pasta("cru");
    let porta = porta_livre();
    let _s = subir(&dir, porta, Cadastro::default());

    let mut c = console(porta);
    assert!(c.executar_linha("/cru").texto().contains("JSON"));
    let t = c.executar_linha("ping").texto().to_string();
    assert!(t.contains("\"phxsql\""), "{t}");
    assert!(c.executar_linha("/cru").texto().contains("tabela"));
}

/// Argumento sem `=` recusa dizendo o que fazer, em vez de mandar um pedido
/// meio montado para o servidor recusar com outra mensagem.
#[test]
fn argumento_sem_igual_recusa_com_o_recado_certo() {
    let dir = pasta("argumento");
    let porta = porta_livre();
    let _s = subir(&dir, porta, Cadastro::default());

    let mut c = console(porta);
    let e = c.executar_linha("tabelas loja").texto().to_string();
    assert!(e.contains("chave=valor"), "{e}");
    assert!(e.contains("/help tabelas"), "{e}");
}

/// **O desafio-resposta, contra um servidor com cadastro -- e a permissao
/// continua valendo do lado de la.**
///
/// A senha nao viaja: o console usa o mesmo caminho da replica, que manda o
/// HMAC do nonce. E o console nao ganha poder nenhum por ser console: a tabela
/// negada continua negada.
#[test]
fn entra_pelo_desafio_e_a_permissao_continua_valendo() {
    let dir = pasta("login");
    let porta = porta_livre();

    // A ana le a base inteira, menos a folha -- mas PODE cria-la, senao nem
    // haveria folha para ela nao ler. E a regra por tabela substituindo a da
    // base, que e como o direito por tabela funciona.
    let hash = phxsql_core::senha::cifrar("segredo-da-ana");
    let cadastro = Cadastro::de_json(
        &Json::analisar(&format!(
            r#"{{"usuarios":[{{"login":"ana","id":9,"senha_hash":"{hash}",
                 "bases":{{"*":{{"ler":true,"criar":true,"inserir":true,
                                 "tabelas":{{"folha":{{"criar":true}}}}}}}}}}]}}"#
        ))
        .unwrap(),
    )
    .unwrap();
    let _s = subir(&dir, porta, cadastro);
    povoar_como(porta, Some(("ana", "segredo-da-ana")));

    let mut c = console(porta);
    // Antes do login, com cadastro no servidor, a operacao com poder recusa.
    assert!(
        c.executar_linha("tabelas database=loja")
            .texto()
            .starts_with("erro"),
        "entrou sem login"
    );

    c.entrar("ana", "segredo-da-ana").expect("o login falhou");
    assert!(c
        .executar_linha("varrer database=loja tabela=clientes")
        .texto()
        .contains("Adriano"));

    let negado = c
        .executar_linha("varrer database=loja tabela=folha")
        .texto()
        .to_string();
    assert!(negado.starts_with("erro"), "leu a folha: {negado}");
    assert!(negado.contains("folha"), "{negado}");

    // E o /help daquela sessao esconde o que ela nao pode chamar.
    let ajuda = c.executar_linha("/help").texto().to_string();
    assert!(ajuda.contains("varrer"), "{ajuda}");
    assert!(
        !ajuda.contains("excluir_tabela"),
        "o /help ofereceu o que a ana nao pode: {ajuda}"
    );

    // Senha errada nao entra -- e a mensagem nao diz se o usuario existe.
    let mut outro = console(porta);
    assert!(outro.entrar("ana", "chute").is_err());
}

/// **O binario de verdade, pelo processo.** Prova o que a biblioteca nao pode
/// provar: que o `main` conecta, autentica, le da entrada padrao e imprime.
#[test]
fn o_binario_le_da_entrada_padrao_e_imprime() {
    let dir = pasta("processo");
    let porta = porta_livre();
    let _s = subir(&dir, porta, Cadastro::default());
    povoar(porta);

    let mut filho = Command::new(env!("CARGO_BIN_EXE_phxsqlcmd"))
        .args([
            "--host",
            "127.0.0.1",
            "--porta",
            &porta.to_string(),
            "--token",
            TOKEN,
            "--database",
            "loja",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("nao subiu o phxsqlcmd");

    let mut entrada = filho.stdin.take().unwrap();
    writeln!(entrada, "varrer tabela=clientes").unwrap();
    writeln!(entrada, "/sair").unwrap();
    // A linha DEPOIS do /sair nao pode ser executada: o console tem de sair
    // no comando, e nao no fim da entrada.
    writeln!(entrada, "excluir_tabela tabela=clientes confirmar=clientes").unwrap();
    entrada.flush().unwrap();
    drop(entrada);

    let saida: Vec<String> = BufReader::new(filho.stdout.take().unwrap())
        .lines()
        .map(|l| l.unwrap())
        .collect();
    let _ = filho.wait();
    let texto = saida.join("\n");
    assert!(texto.contains("Adriano"), "{texto}");
    assert!(texto.contains("Ana Maria"), "{texto}");
    assert!(
        !texto.contains("arquivos_apagados"),
        "executou a linha depois do /sair: {texto}"
    );

    // E a tabela continua la, que e a prova pelo outro lado.
    let mut c = console(porta);
    c.executar_linha("/use loja");
    assert!(c.executar_linha("tabelas").texto().contains("clientes"));
}

/// `--comando` roda uma linha e sai -- e sai com codigo de ERRO quando a linha
/// deu erro. Sem isso, um script encadeado seguiria como se desse certo.
#[test]
fn o_comando_unico_devolve_o_codigo_de_saida_certo() {
    let dir = pasta("comando");
    let porta = porta_livre();
    let _s = subir(&dir, porta, Cadastro::default());
    povoar(porta);

    let rodar = |linha: &str| -> (bool, String) {
        let s = Command::new(env!("CARGO_BIN_EXE_phxsqlcmd"))
            .args([
                "--host",
                "127.0.0.1",
                "--porta",
                &porta.to_string(),
                "--token",
                TOKEN,
                "--comando",
                linha,
            ])
            .output()
            .unwrap();
        (
            s.status.success(),
            String::from_utf8_lossy(&s.stdout).to_string(),
        )
    };

    let (ok, texto) = rodar("tabelas database=loja");
    assert!(ok, "{texto}");
    assert!(texto.contains("clientes"), "{texto}");

    let (ok, texto) = rodar("varrer database=loja tabela=nao_existe");
    assert!(!ok, "linha com erro saiu com sucesso: {texto}");
}
