//! Pedido 225 -- os caminhos padrao do `config.json` resolvem contra o
//! DIRETORIO DO CONFIG, nao contra o diretorio de trabalho do processo.
//!
//! Teste UNITARIO nao prova isto: `Config::ler` sempre foi chamado, nos
//! testes, com o cwd do processo de teste sendo o mesmo em toda parte (o
//! `cargo test` roda de um lugar so). O defeito so aparece quando o
//! `phxsqld` de VERDADE sobe de um diretorio diferente do diretorio do
//! config -- e foi assim que a bancada `bancada/proibidos/provar.py` deixou
//! `jobs.json`/`jobs.log` na raiz do repositorio (o cwd de quem a chamou),
//! em vez de na base dela. Prova real: sobe o binario de um diretorio, com o
//! `config.json` NOUTRO, e confere onde os arquivos nasceram.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn esperar_porta_aberta(porta: u16) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(100)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o phxsqld nao subiu na porta {porta} em 5 s");
}

/// Guarda que mata o processo no `Drop` -- inclusive quando o teste falha no
/// meio de uma asserção, que e o caso que um `kill` no fim do corpo nunca
/// alcança (a mesma licao do pedido 150 para diretorio de teste).
struct Filho(Child);

impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn pedir(porta: u16, linha: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{linha}").unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    resposta
}

/// **O teste do pedido 225.** Um `config.json` em `.../cfg/`, com os
/// caminhos no PADRAO (sem `base`, `log_acessos`, `blacklist`, `dblink` nem
/// `jobs` escritos -- e por isso o padrao de cada um e o que esta em jogo), e
/// o `phxsqld` iniciado de `.../outro/`, um diretorio IRMAO e vazio.
///
/// Confere dos dois lados: os arquivos NASCEM ao lado do config (a Figura
/// que faltava), e NAO nascem no diretorio de onde o processo foi chamado
/// (o defeito de verdade, medido antes desta rodada com `jobs.json` e
/// `jobs.log` na raiz do repositorio).
#[test]
fn servidor_de_outro_diretorio_escreve_ao_lado_do_config() {
    let raiz = DirTemp::novo("caminhos-do-config");
    let cfg_dir = raiz.join("cfg");
    let outro_dir = raiz.join("outro");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::create_dir_all(&outro_dir).unwrap();

    let porta = porta_livre();
    let config_json = format!(
        r#"{{
            "bind": "127.0.0.1:{porta}",
            "token": "t",
            "web": {{ "ligado": false }}
        }}"#
    );
    let config_path = cfg_dir.join("config.json");
    std::fs::write(&config_path, config_json).unwrap();

    let filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config_path)
            // O CORACAO do teste: o processo sobe de OUTRO diretorio, que nao
            // tem nenhuma relacao com onde o config.json mora.
            .current_dir(&outro_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    esperar_porta_aberta(porta);

    // `base` ("dados") e `log_acessos` ("acessos.log") nascem no ARRANQUE,
    // sem pedido nenhum -- `Raiz::nova` cria o diretorio da base, e
    // `LogAcessos::abrir` cria o arquivo vazio.
    assert!(
        cfg_dir.join("dados").is_dir(),
        "a base (\"dados\") tinha de nascer ao lado do config.json, em {}",
        cfg_dir.display()
    );
    assert!(
        cfg_dir.join("acessos.log").is_file(),
        "o log de acessos tinha de nascer ao lado do config.json"
    );

    // `jobs.json` so nasce quando algo escreve nele -- um `job_salvar`, sem
    // cadastro (este config nao tem nenhum), passa direto pela sessao.
    let r = pedir(
        porta,
        r#"{"token":"t","op":"job_salvar","job":{"nome":"prova-225","pedido":{"op":"ping"}}}"#,
    );
    assert!(r.contains("\"salvo\""), "job_salvar recusou: {r}");
    assert!(
        cfg_dir.join("jobs.json").is_file(),
        "o jobs.json tinha de nascer ao lado do config.json, nao em {}",
        outro_dir.display()
    );

    // E o lado que importa tanto quanto: NADA nasceu no diretorio de onde o
    // processo foi chamado. E o defeito de verdade -- medido nesta mesma
    // arvore com `jobs.json`/`jobs.log` na raiz do repositorio.
    for nome in [
        "dados",
        "acessos.log",
        "jobs.json",
        "jobs.log",
        "blacklist.json",
        "dblink.json",
    ] {
        assert!(
            !outro_dir.join(nome).exists(),
            "{nome} nasceu no diretorio de trabalho do processo ({}) -- \
             exatamente o defeito do pedido 225",
            outro_dir.display()
        );
    }

    drop(filho);
}
