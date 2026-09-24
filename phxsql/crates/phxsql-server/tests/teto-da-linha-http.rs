//! O teto de UMA LINHA da porta HTTP -- pedido 434, provado PELO SOQUETE.
//!
//! # O defeito, e a prova de que era defeito e nao desenho
//!
//! O `http::ler_pedido` tinha dois `read_line` crus: a linha de PEDIDO sem
//! teto nenhum, e cada linha de CABECALHO conferida contra o `MAX_CABECALHO`
//! **depois** de ja estar na memoria -- o acumulado limitado, a linha nunca.
//! Quinze linhas abaixo, no mesmo arquivo, o `Content-Length` e conferido
//! ANTES do `vec![0u8; tamanho]`: quem escreveu a porta sabia a diferenca, e
//! deixou a linha de fora.
//!
//! Com `conexoes_web_max` nascendo em 64, sessenta e quatro navegadores de
//! mentira mandando bytes sem `\n` fazem sessenta e quatro `String` crescerem
//! sem teto, sem log e sem um unico pedido valido.
//!
//! # Por que a prova e «o servidor larga a conexao», e nao «quantos bytes
//! coube escrever»
//!
//! Porque o buffer de recepcao do sistema autotune ate 32 MiB nesta maquina
//! (`/proc/sys/net/ipv4/tcp_rmem`), e um cliente consegue escrever dezenas de
//! MiB antes de qualquer bloqueio: contar bytes mediria o kernel, nao o
//! servidor. O que separa os dois estados sem ambiguidade e o TEMPO ate a
//! conexao acabar. Com o teto, o servidor desiste na hora em que a linha passa
//! de 16 KiB -- responde 400 e fecha. Com o defeito reposto, ele fica preso no
//! `read_line` esperando uma quebra de linha que nunca vem, e so larga quando
//! o prazo DELE vence -- por isso o `timeout_s` desta bateria e 60 s e o
//! prazo do cliente e 3 s: vinte vezes de separacao entre os dois estados.
//!
//! # Por que soquete, e nao teste de unidade
//!
//! Porque o que se prova e que o servidor PARA DE LER -- e parar de ler e um
//! fato do sistema operacional, nao da funcao. E a licao do `BULKINSERT`:
//! teste de unidade nao prova queda de conexao, soquete prova.

mod comum;
use comum::DirTemp;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-do-teto-da-linha-http";

/// Bytes sem `\n` que o cliente despeja.
///
/// 256 KiB: dezesseis vezes o teto de 16 KiB, e pequeno o bastante para caber
/// nos buffers do soquete sem a escrita bloquear -- o que se mede aqui e o
/// servidor largar a conexao, e uma escrita presa mediria outra coisa.
const SEM_FIM: usize = 256 * 1024;

/// Prazo do CLIENTE para o servidor dar sinal de vida.
///
/// Vinte vezes menor que o prazo do servidor, de proposito: e a distancia que
/// separa «desistiu no teto» de «ficou esperando a quebra de linha».
const PRAZO: Duration = Duration::from_secs(3);

fn pasta(nome: &str) -> DirTemp {
    let d = DirTemp::novo(&format!("teto-http-{nome}"));
    std::fs::create_dir_all(d.join("base")).unwrap();
    d
}

/// Sobe um servidor com a porta web ligada e devolve o endereco dela.
///
/// Pede porta 0 nas duas e devolve a REAL da web, lida do proprio servidor
/// depois do `bind` -- pedido 401.
fn subir(base: &std::path::Path) -> (Arc<Servidor>, SocketAddr) {
    // O `cifra_fio.exigir` falso e o ESCAPE ESCRITO: com ele ligado o
    // `portao_de_rede_http` recusa antes de `ler_pedido` existir, e a prova
    // mediria o portao errado. O `timeout_s` alto e o que da a separacao de
    // tempo explicada no cabecalho.
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:0", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "jobs": {jobs:?},
              "timeout_s": 60,
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": true, "bind": "127.0.0.1:0" }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
        jobs = base.join("jobs.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let web = comum::porta_real(|| s.porta_web());
    let alvo: SocketAddr = format!("127.0.0.1:{web}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(10);
    while Instant::now() < ate {
        if let Some(r) = pedir(alvo, "GET /saude HTTP/1.1\r\nHost: x\r\n\r\n") {
            if r.contains("200") {
                return (s, alvo);
            }
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    panic!("a porta web nao subiu em {alvo}");
}

/// Um pedido HTTP cru, com prazo. `None` = nem conectou.
fn pedir(alvo: SocketAddr, texto: &str) -> Option<String> {
    let mut fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(PRAZO)).ok()?;
    fluxo.set_write_timeout(Some(PRAZO)).ok()?;
    fluxo.write_all(texto.as_bytes()).ok()?;
    fluxo.flush().ok()?;
    let mut resposta = String::new();
    let _ = fluxo.read_to_string(&mut resposta);
    Some(resposta)
}

/// Despeja `SEM_FIM` bytes sem quebra de linha e devolve o que o servidor fez
/// dentro do [`PRAZO`].
///
/// A escrita pode falhar, e falhar e um resultado bom: quer dizer que o
/// servidor ja tinha ido embora. O que nao pode acontecer e o cliente escrever
/// tudo e o servidor continuar calado, guardando.
fn despejar_sem_fim(alvo: SocketAddr, comeco: &str) -> Result<String, String> {
    let mut fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo.set_read_timeout(Some(PRAZO)).unwrap();
    fluxo.set_write_timeout(Some(PRAZO)).unwrap();
    fluxo.write_all(comeco.as_bytes()).unwrap();
    let bloco = vec![b'A'; 8 * 1024];
    let mut escritos = 0usize;
    while escritos < SEM_FIM {
        let n = bloco.len().min(SEM_FIM - escritos);
        match fluxo.write(&bloco[..n]) {
            Ok(0) => break,
            Ok(k) => escritos += k,
            // Cano quebrado: o servidor largou a conexao, que e o que se quer.
            Err(_) => break,
        }
    }
    let _ = fluxo.flush();
    // NAO se manda `\n`: o ponto da prova e que este lado nunca termina a
    // linha. Quem termina a conversa tem de ser o servidor.
    let mut resposta = String::new();
    match fluxo.read_to_string(&mut resposta) {
        // Fim limpo ou resposta: o servidor desistiu da linha sem fim.
        Ok(_) => Ok(resposta),
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => Ok(resposta),
        // Prazo estourado: ninguem fechou nada -- o servidor continua lendo, e
        // o que ele leu esta na memoria dele.
        Err(e) => Err(format!("{e}")),
    }
}

/// **A linha de PEDIDO sem fim nao e guardada: o servidor desiste no teto.**
///
/// O defeito que este teste trava: `read_line` cru na linha de pedido, com o
/// tamanho escolhido por quem ainda nao pediu nada.
#[test]
fn a_linha_de_pedido_sem_fim_faz_o_servidor_desistir() {
    let d = pasta("pedido");
    let (_s, alvo) = subir(&d);
    let marca = Instant::now();
    match despejar_sem_fim(alvo, "GET /saude?") {
        Ok(_) => {}
        Err(e) => panic!(
            "o servidor continuou LENDO a linha de pedido sem fim depois de \
             {SEM_FIM} bytes e {:?}: {e}. Sem teto, quem escolhe quanta \
             memoria esta porta reserva e quem ainda nao pediu nada.",
            marca.elapsed()
        ),
    }
}

/// **A linha de CABECALHO sem fim tambem nao e guardada.**
///
/// E o IRMAO da de cima, e ele fica: as duas chamam a mesma funcao na mesma
/// ordem. O `MAX_CABECALHO` ja existia para o acumulado, e o acumulado so e
/// conferido depois de a linha caber na memoria -- uma linha de cabecalho sem
/// fim nunca chegava a soma nenhuma.
#[test]
fn a_linha_de_cabecalho_sem_fim_faz_o_servidor_desistir() {
    let d = pasta("cabecalho");
    let (_s, alvo) = subir(&d);
    let marca = Instant::now();
    match despejar_sem_fim(alvo, "GET /saude HTTP/1.1\r\nX-Grande: ") {
        Ok(_) => {}
        Err(e) => panic!(
            "o servidor continuou LENDO a linha de cabecalho sem fim depois de \
             {SEM_FIM} bytes e {:?}: {e}",
            marca.elapsed()
        ),
    }
}

/// **O COMPORTAMENTO VELHO: pedido longo e LEGITIMO continua passando.**
///
/// Este e o teste que mais importa. Uma linha de pedido HTTP de verdade nao
/// chega perto de 8 KiB -- as maiores desta interface sao `GET` com filtro na
/// query --, e o teto escolhido e o `MAX_CABECALHO` de 16 KiB que a porta ja
/// declarava. Por isso o conjunto do que e ACEITO nao mudou em nada: toda
/// linha de um pedido aceito ja cabia nos 16 KiB, porque o acumulado inteiro
/// tinha de caber. O que mudou foi so QUANDO a recusa acontece.
#[test]
fn o_pedido_longo_e_legitimo_continua_passando() {
    let d = pasta("legitimo");
    let (_s, alvo) = subir(&d);
    // 8 KiB de query -- o dobro do maior pedido que esta interface monta, e
    // metade do teto.
    let query = "x=".to_string() + &"a".repeat(8 * 1024);
    let r = pedir(
        alvo,
        &format!("GET /saude?{query} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"),
    )
    .expect("o pedido longo e legitimo nem conectou");
    assert!(
        r.contains("200"),
        "o pedido longo e legitimo deixou de ser atendido: {}",
        &r[..r.len().min(200)]
    );
}

/// **E o pedido de todo dia, curto, continua igual.**
#[test]
fn o_pedido_de_sempre_continua_passando() {
    let d = pasta("curto");
    let (_s, alvo) = subir(&d);
    let r = pedir(
        alvo,
        "GET /saude HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    )
    .expect("o pedido curto nem conectou");
    assert!(r.contains("200"), "{}", &r[..r.len().min(200)]);
}
