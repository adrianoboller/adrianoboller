//! Pedido 638 -- um comando de firewall que PENDURA nao pode parar o servidor.
//!
//! O defeito: `Firewall::aplicar` rodava `Command::output()` sem prazo DENTRO
//! do `lista_negra.lock()`, e `barrado()` pega esse mesmo mutex em TODA
//! conexao. Quem errava o token N vezes -- sem credencial nenhuma -- bastava
//! para o comando de firewall pendurar com o mutex preso e o servidor inteiro
//! parar de aceitar. Teste unitario nao prova isto: o que se mede e se OUTRO
//! cliente, por OUTRO soquete, ainda e atendido.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_server::{Config, Firewall, Servidor};

const TOKEN: &str = "teste-do-firewall";

fn conectar(porta: u16) -> TcpStream {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).expect("nao conectei")
}

/// Uma linha para o servidor e a resposta, com `espera` de prazo de leitura.
/// `None` quando o servidor nao respondeu a tempo -- que e exatamente o que
/// este arquivo existe para detectar.
fn pedir(porta: u16, linha: &str, espera: Duration) -> Option<String> {
    let fluxo = conectar(porta);
    fluxo.set_read_timeout(Some(espera)).unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    // IP bloqueado: o servidor escreve a recusa e fecha antes de ler.
    let _ = writeln!(escrita, "{linha}");
    let mut r = String::new();
    match leitor.read_line(&mut r) {
        Ok(n) if n > 0 => Some(r),
        _ => None,
    }
}

fn subir(base: &std::path::Path, firewall: Vec<String>) -> u16 {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // O que se mede aqui e a trava da lista negra, nao a cifra do fio.
    c.cifra_fio.exigir = false;
    c.web.ligado = false;
    c.politica.tentativas_ate_bloquear = 3;
    // O ESCAPE ESCRITO do pedido 766 (P9): o soquete chega de 127.0.0.1, e
    // o loopback nasce poupado -- sem esta linha o terceiro token nunca
    // bloquearia, o firewall nunca rodaria, e o teste passaria POR ENGANO
    // (medido em 09/10/2026: verde sem o comando de firewall rodar).
    c.politica.poupar_loopback = false;
    c.politica.firewall = Some(Firewall {
        ligado: true,
        bloquear: firewall,
        desbloquear: vec![],
        listar: vec![],
        timeout_s: 5,
    });
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    comum::porta_real(|| s.porta_dos_dados())
}

/// **Defeito reposto** (voltar o `output()` sob o mutex): o terceiro token
/// errado pendura o comando, `barrado()` espera o mutex e o `ping` do outro
/// cliente nao responde em 2 s.
#[test]
fn firewall_que_pendura_nao_para_o_servidor() {
    let base = DirTemp::novo("fw-pendura");
    let porta = subir(&base, vec!["/bin/sleep".into(), "60".into()]);

    let errado = r#"{"token":"errado","op":"ping"}"#.to_string();
    // Dois tokens errados contam; o terceiro bloqueia e dispara o firewall.
    for _ in 0..2 {
        let r = pedir(porta, &errado, Duration::from_secs(5)).expect("sem resposta");
        assert!(r.contains("\"ok\":false"), "{r}");
    }
    // O terceiro NAO se espera: e ele quem fica preso no comando.
    let terceiro = std::thread::spawn(move || pedir(porta, &errado, Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(300));

    // Outro cliente: responde -- ok ou recusa de bloqueio, tanto faz. O que
    // nao pode e ficar mudo.
    let inicio = Instant::now();
    let r = pedir(
        porta,
        &format!("{{\"token\":\"{TOKEN}\",\"op\":\"ping\"}}"),
        Duration::from_secs(2),
    );
    let gasto = inicio.elapsed();
    assert!(
        r.is_some() && gasto < Duration::from_secs(2),
        "o servidor parou: {gasto:?} sem resposta com o firewall pendurado"
    );

    // E o prazo mata o comando: o terceiro recebe a resposta dele logo depois
    // do `timeout_s` (5 s), nao em 60.
    let t0 = Instant::now();
    let r3 = terceiro.join().unwrap();
    assert!(r3.is_some(), "o terceiro nunca foi respondido");
    assert!(
        t0.elapsed() < Duration::from_secs(9),
        "o prazo do firewall nao valeu: {:?}",
        t0.elapsed()
    );
    // A PREMISSA, conferida: o terceiro bloqueou de verdade, e foi por isso
    // que o firewall rodou. Sem esta linha, uma guarda que poupasse o IP
    // deixava o teste verde sem o comando pendurado nunca ter existido.
    let bl = phxsql_server::Blacklist::abrir(base.join("blacklist.json")).unwrap();
    assert!(
        bl.bloqueado("127.0.0.1", phxsql_server::agora_ms())
            .is_some(),
        "o terceiro token nao bloqueou: o firewall nao rodou e o teste nao mediu nada"
    );
}
