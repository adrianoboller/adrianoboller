//! A ponta que liga o `config.json` a cifra dos diarios.
//!
//! # Por que um arquivo so para isto
//!
//! `Cifra::aplicar` mexe num global do PROCESSO -- a chave vale para todo
//! diario aberto daqui em diante. Provar isso dentro do binario da biblioteca
//! faria os outros testes do mesmo binario nascerem com a cifra ligada no meio
//! da corrida. Um teste de integracao roda em outro processo, e ali o global e
//! so dele.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::paginacao::Paginacao;
use phxsql_server::config::Config;
use phxsql_server::servidor::Servidor;
use phxsql_store::cofre;
use phxsql_store::log::{LogFile, Operacao};

/// A trava que serializa os testes: o cofre e global ao processo.
static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("cfg-cifra-{rotulo}"))
}

/// O caminho inteiro: `config.json` liga a cifra, e o `.log` nasce cifrado.
///
/// E o teste que responde a pergunta que o projeto ja pagou caro para aprender:
/// o campo de configuracao **e lido por alguma linha de codigo**?
#[test]
fn o_campo_do_config_liga_a_cifra_de_verdade() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("liga");
    let caminho = d.join("config.json");
    std::fs::write(
        &caminho,
        r#"{
          "token": "um token qualquer",
          "base": "dados",
          "cifra": { "ligada": true, "senha": "a chave do cofre", "iteracoes": 10000 }
        }"#,
    )
    .unwrap();

    assert!(!cofre::ligado(), "o processo nao pode comecar com cofre");
    let c = Config::ler(&caminho).unwrap();
    assert!(c.cifra.ligada);
    assert!(
        cofre::ligado(),
        "o config.json ligou a cifra e nenhuma linha de codigo leu o campo"
    );

    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    l.registrar_com_imagem(Operacao::Inclusao, 1, 1, b"Blumenau")
        .unwrap();
    l.sincronizar().unwrap();
    let bruto = std::fs::read(d.join("t.log")).unwrap();
    assert_eq!(
        u16::from_le_bytes([bruto[8], bruto[9]]),
        3,
        "o .log nasceu na versao velha com a cifra ligada"
    );
    assert!(
        !bruto.windows(8).any(|j| j == b"Blumenau"),
        "o texto claro foi para o disco"
    );

    // E o `para_json` do config -- que a tela le -- nao leva a senha junto.
    let texto = c.para_json().escrever();
    assert!(
        !texto.contains("a chave do cofre"),
        "a senha vazou: {texto}"
    );

    cofre::desligar();
    std::fs::remove_dir_all(&d).unwrap();
}

/// Cifra ligada sem senha nao sobe -- e o erro diz qual campo preencher.
#[test]
fn cifra_ligada_sem_senha_recusa_a_subir() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("sem-senha");
    let caminho = d.join("config.json");
    std::fs::write(&caminho, r#"{"token":"t","cifra":{"ligada":true}}"#).unwrap();
    let Err(e) = Config::ler(&caminho) else {
        panic!("subiu com a cifra ligada e sem senha")
    };
    assert!(e.to_string().contains("senha"), "{e}");
    assert!(!cofre::ligado());
    std::fs::remove_dir_all(&d).unwrap();
}

/// Config sem a secao `cifra` nao liga nada, e nao vira aviso de campo
/// estranho. E o comportamento velho, que e o que mais importa proteger.
#[test]
fn config_de_ontem_continua_subindo_sem_cifra() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("velho");
    let caminho = d.join("config.json");
    std::fs::write(&caminho, r#"{"token":"t","bind":"0.0.0.0:5000"}"#).unwrap();
    let c = Config::ler(&caminho).unwrap();
    assert!(!c.cifra.ligada);
    assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    assert!(!cofre::ligado(), "um config sem cifra ligou o cofre");

    let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
    l.registrar(Operacao::Inclusao, 1, 1).unwrap();
    l.sincronizar().unwrap();
    let bruto = std::fs::read(d.join("t.log")).unwrap();
    assert_eq!(u16::from_le_bytes([bruto[8], bruto[9]]), 2);
    std::fs::remove_dir_all(&d).unwrap();
}

/// O JSON da configuracao inteira nunca carrega a senha da cifra.
#[test]
fn a_resposta_do_protocolo_nao_leva_a_senha() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let j = Json::analisar(
        r#"{"token":"t","cifra":{"ligada":true,"senha":"segredo do cofre","iteracoes":10000}}"#,
    )
    .unwrap();
    let c = Config::de_json(&j).unwrap();
    let texto = c.para_json().escrever();
    assert!(!texto.contains("segredo do cofre"), "{texto}");
    // E nem no `Debug`, que e por onde um diagnostico apressado vazaria.
    assert!(!format!("{:?}", c.cifra).contains("segredo do cofre"));
}

// ---------------------------------------------------------------------------
// O material POR TABELA, pelo soquete -- a outra metade do `cifra.ligada`
// ---------------------------------------------------------------------------

/// Uma porta que ninguem mais esta usando, na faixa deste arquivo. Os outros
/// binarios de teste tem as suas (7200, 7250, 7300); faixas separadas porque
/// `cargo test` roda os binarios em paralelo.
fn porta_livre() -> u16 {
    static PROXIMA: AtomicU16 = AtomicU16::new(7350);
    loop {
        let porta = PROXIMA.fetch_add(1, Ordering::SeqCst);
        assert!(porta < 7399, "acabaram as portas entre 7350 e 7398");
        if let Ok(l) = TcpListener::bind(("127.0.0.1", porta)) {
            drop(l);
            return porta;
        }
    }
}

fn esperar_porta(porta: u16) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

/// Uma linha de JSON para o servidor, uma linha de volta.
fn pedir(porta: u16, corpo: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{{\"token\":\"t\",{}}}", corpo.replace('\n', " ")).unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    assert!(resposta.contains("\"ok\":true"), "{corpo} -> {resposta}");
    resposta
}

/// `{"op":"config"}` responde `cifra.ligada: true` pelo PROCESSO; `esquema`
/// responde `material` pelo ARQUIVO -- e os dois so contam a mesma historia
/// quando toda tabela com dado pessoal nasceu depois do cofre.
///
/// # Por que este teste existe -- pedido 210
///
/// Ate 09/09/2026 uma tabela cujas unicas colunas marcadas eram externas
/// (`Memo`/`Bin`) nascia em claro com o cofre ligado, e `cifra.ligada: true`
/// era meia-verdade: o servidor tinha chave e a tabela nao a usava. Este
/// teste sobe o servidor com a cifra ligada pelo `config.json`, cria a tabela
/// exatamente nesse formato, e exige `material: cifrado` na resposta -- e
/// `em_claro` numa tabela sem marca, criada no mesmo servidor, porque o campo
/// e do arquivo e nao do interruptor.
#[test]
fn o_esquema_diz_o_material_de_cada_tabela_com_a_cifra_ligada() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let d = dir("material");
    let porta = porta_livre();
    let caminho = d.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "token": "t",
              "bind": "127.0.0.1:{porta}",
              "base": "{}",
              "cifra": {{ "ligada": true, "senha": "a chave do cofre", "iteracoes": 10000 }}
            }}"#,
            d.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    assert!(cofre::ligado(), "o config.json nao ligou o cofre");
    c.web.ligado = false;
    c.log_acessos = d.join("acessos.log");
    c.blacklist = d.join("blacklist.json");
    c.dblink = d.join("dblink.json");
    c.jobs = d.join("jobs.json");
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar_porta(porta);

    pedir(porta, r#""op":"criar_database","database":"loja""#);
    // O formato do pedido 210: a UNICA coluna marcada e um Memo.
    pedir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"fichas",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"obs","tipo":"Memo","dado_pessoal":"sensivel"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    pedir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"simples",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );

    let cfg = pedir(porta, r#""op":"config""#);
    assert!(cfg.contains("\"ligada\":true"), "{cfg}");
    let fichas = pedir(
        porta,
        r#""op":"esquema","database":"loja","tabela":"fichas""#,
    );
    assert!(
        fichas.contains("\"material\":\"cifrado\""),
        "a tabela so de externas nao nasceu cifrada, ou o esquema nao diz: {fichas}"
    );
    let simples = pedir(
        porta,
        r#""op":"esquema","database":"loja","tabela":"simples""#,
    );
    assert!(
        simples.contains("\"material\":\"em_claro\""),
        "tabela sem marca tem de dizer em_claro mesmo com o cofre ligado: {simples}"
    );
    // O relatorio de conformidade traz o mesmo campo no achado.
    let lgpd = pedir(porta, r#""op":"dados_pessoais","database":"loja""#);
    assert!(
        lgpd.contains("\"material\":\"cifrado\""),
        "o achado de `fichas` nao diz o material: {lgpd}"
    );

    cofre::desligar();
}
