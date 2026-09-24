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
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::paginacao::Paginacao;
use phxsql_server::config::{Config, Segredo};
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
    // A senha esta mesmo la -- senao a prova passaria por nao haver segredo
    // nenhum para vazar.
    assert_eq!(c.cifra.senha().unwrap(), "segredo do cofre");
    let texto = c.para_json().escrever();
    assert!(!texto.contains("segredo do cofre"), "{texto}");
    // E nem no `Debug`, que e por onde um diagnostico apressado vazaria.
    // CONTA, e nao so pergunta: o vermelho diz quantas vezes a senha saiu e
    // mostra onde, que e o dano -- e nao so que algo deu errado.
    let depurado = format!("{:?}", c.cifra);
    let vazou = depurado.matches("segredo do cofre").count();
    assert_eq!(
        vazou, 0,
        "a senha da cifra saiu {vazou} vez(es) no Debug: {depurado}"
    );
}

/// O `Debug` do proprio [`Segredo`] nunca mostra o valor.
///
/// # Por que um teste do TIPO, ao lado do da `Cifra`
///
/// Desde o pedido 372 a senha da cifra e um `Segredo`, e a protecao do
/// `Debug` mora em DOIS lugares: no rotulo que o `impl Debug for Cifra`
/// escreve a mao, e no `impl Debug for Segredo`. O teste de cima so alcanca o
/// primeiro -- o `Debug` da `Cifra` nao chama o do `Segredo`, e nenhum dos
/// cinco donos o chama hoje. Medido em 24/09/2026, repondo o `Debug` do tipo
/// imprimindo o valor: nenhum teste do `--lib` nem deste arquivo caia.
///
/// E e ELE que torna inocente o `.field("senha", &self.senha)`: com o tipo
/// intacto aquela troca imprime `(oculto)`, e com o tipo derivado ela imprime
/// `Segredo { valor: "..." }`. Camada que ninguem prova e camada que alguem
/// «simplifica» -- o comentario do proprio `impl` diz que nenhum dono o chama.
///
/// Os dois estados que CARREGAM valor em memoria: o do arquivo e o que veio
/// selado e abriu. As duas formas (`{:?}` e `{:#?}`) pelo mesmo motivo das
/// irmas do `config.rs`: `impl` escrito a mao pode tratar uma e esquecer a
/// outra.
#[test]
fn o_debug_do_segredo_nunca_mostra_o_valor() {
    const VALOR: &str = "valor-cru-do-segredo";
    let j = Json::analisar(&format!(r#"{{"senha":"{VALOR}"}}"#)).unwrap();
    let (do_arquivo, _) = Segredo::ler(&j, "senha", "o teste do Debug");
    let aberto = Segredo::aberto_do_envelope(VALOR.to_string());
    for (estado, s) in [("do arquivo", &do_arquivo), ("aberto do envelope", &aberto)] {
        // O valor esta mesmo dentro -- senao a prova passaria por vazio.
        assert_eq!(
            s.valor().unwrap(),
            VALOR,
            "o segredo {estado} nao guardou o valor"
        );
        for depurado in [format!("{s:?}"), format!("{s:#?}")] {
            let vazou = depurado.matches(VALOR).count();
            assert_eq!(
                vazou, 0,
                "o segredo {estado} saiu {vazou} vez(es) no Debug: {depurado}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// O material POR TABELA, pelo soquete -- a outra metade do `cifra.ligada`
// ---------------------------------------------------------------------------

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
    let caminho = d.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "token": "t",
              "bind": "127.0.0.1:0",
              "base": "{}",
              "cifra": {{ "ligada": true, "senha": "a chave do cofre", "iteracoes": 10000 }}
            }}"#,
            d.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    assert!(cofre::ligado(), "o config.json nao ligou o cofre");
    // O ESCAPE ESCRITO: desde 18/09/2026 a cifra do FIO nasce exigida (pedido
    // 370, ordem do dono), e este teste fala com a porta de dados em claro
    // porque o que ele mede e a cifra EM REPOUSO -- outra cifra, outro
    // arquivo. Sem esta linha a recusa lida aqui seria a do fio, e a prova
    // passaria a medir o portao errado.
    c.cifra_fio.exigir = false;
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
    let porta = comum::porta_real(|| s.porta_dos_dados());
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
