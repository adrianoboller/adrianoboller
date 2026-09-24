//! O transporte HTTP da casa: o MESMO para o painel (`http.rs`) e para o
//! programa de mesa (`mesa.rs`). As guardas que valem para qualquer porta
//! web moram aqui, uma vez so:
//!
//! 1. teto de conexoes simultaneas (a que passa e fechada na hora);
//! 2. prazo TOTAL de 10 s para o pedido chegar (slowloris);
//! 3. tetos de cabecalho (16 KiB) e corpo (256 KiB) antes de alocar;
//! 4. `Host` conferido contra a lista (DNS rebinding -> 421);
//! 5. POST so com `Content-Type: application/json` (CSRF -> 415);
//! 6. `Transfer-Encoding` recusado; CSP sem script inline.
//!
//! Quem usa so escreve as rotas.

use phxsql_core::json::Json;
use phxsql_core::semaforo::Semaforo;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TETO_CABECALHO: usize = 16 * 1024;
const TETO_CORPO: usize = 256 * 1024;
/// Prazo para o pedido chegar inteiro.
const PRAZO_PEDIDO: Duration = Duration::from_secs(10);

/// O que a rota devolve.
pub struct Resposta {
    pub status: u16,
    pub tipo: &'static str,
    pub corpo: String,
}

impl Resposta {
    pub fn json(status: u16, corpo: String) -> Resposta {
        Resposta {
            status,
            tipo: "application/json; charset=utf-8",
            corpo,
        }
    }

    pub fn html(corpo: &str) -> Resposta {
        Resposta {
            status: 200,
            tipo: "text/html; charset=utf-8",
            corpo: corpo.to_string(),
        }
    }

    pub fn js(corpo: &str) -> Resposta {
        Resposta {
            status: 200,
            tipo: "text/javascript; charset=utf-8",
            corpo: corpo.to_string(),
        }
    }

    pub fn erro(status: u16, m: &str) -> Resposta {
        Resposta::json(
            status,
            Json::objeto(vec![("erro", Json::texto_de(m))]).escrever(),
        )
    }
}

/// Nomes pelos quais o servidor aceita ser chamado: loopback na porta dele,
/// o proprio endereco de escuta e os declarados.
pub fn hosts_de(escuta: &str, nomes: &[String]) -> Vec<String> {
    let porta = escuta.rsplit(':').next().unwrap_or("0");
    let mut h: Vec<String> = ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .map(|n| format!("{n}:{porta}"))
        .collect();
    h.push(escuta.to_string());
    h.extend(nomes.iter().cloned());
    h.into_iter().map(|x| x.to_lowercase()).collect()
}

/// Atende para sempre com as guardas acima; `tratar` so ve pedido que ja
/// passou por todas.
pub fn servir<F>(
    ouvinte: TcpListener,
    hosts: Vec<String>,
    teto: usize,
    tratar: F,
) -> Result<(), String>
where
    F: Fn(&Pedido) -> Resposta + Send + Sync + 'static,
{
    let conexoes = Semaforo::novo(teto);
    let tratar = Arc::new(tratar);
    let hosts = Arc::new(hosts);
    loop {
        let conexao = match ouvinte.accept() {
            Ok((c, _)) => c,
            Err(_) => {
                // Sem descritor (EMFILE) o accept falha na hora; sem a pausa,
                // o laco gira a 100% de CPU.
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        let Some(permissao) = conexoes.tentar() else {
            drop(conexao); // cheio: fecha na hora, sem thread nova
            continue;
        };
        let (tratar, hosts) = (Arc::clone(&tratar), Arc::clone(&hosts));
        std::thread::spawn(move || {
            let _permissao = permissao;
            let _ = atender(conexao, &hosts, &*tratar);
        });
    }
}

fn atender(
    mut fio: TcpStream,
    hosts: &[String],
    tratar: &dyn Fn(&Pedido) -> Resposta,
) -> std::io::Result<()> {
    let r = match ler_pedido(&fio) {
        Err((s, m)) => Resposta::erro(s, &m),
        Ok(p) => {
            let host_ok = p
                .host
                .as_ref()
                .is_some_and(|h| hosts.iter().any(|x| x == h));
            let json_ok = p.metodo != "POST"
                || p.tipo
                    .as_deref()
                    .is_some_and(|t| t.split(';').next() == Some("application/json"));
            if !host_ok {
                Resposta::erro(421, "Host nao reconhecido por este servidor")
            } else if !json_ok {
                Resposta::erro(415, "POST so com Content-Type: application/json")
            } else {
                tratar(&p)
            }
        }
    };
    responder(&mut fio, r.status, r.tipo, &r.corpo)
}

pub struct Pedido {
    pub metodo: String,
    pub caminho: String,
    pub token: Option<String>,
    pub host: Option<String>,
    pub tipo: Option<String>,
    pub ip: IpAddr,
    pub corpo: String,
}

/// Leitor que respeita um prazo TOTAL: a cada leitura, o tempo de espera e o
/// que sobra do prazo, nao um prazo novo.
struct ComPrazo<'a> {
    fio: &'a TcpStream,
    fim: Instant,
}

impl Read for ComPrazo<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let resta = self.fim.saturating_duration_since(Instant::now());
        if resta.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "prazo do pedido",
            ));
        }
        self.fio.set_read_timeout(Some(resta))?;
        (&*self.fio).read(buf)
    }
}

fn ler_pedido(fio: &TcpStream) -> Result<Pedido, (u16, String)> {
    let ip = fio
        .peer_addr()
        .map(|a| a.ip())
        .map_err(|_| (400, "sem endereco".to_string()))?;
    let prazo = ComPrazo {
        fio,
        fim: Instant::now() + PRAZO_PEDIDO,
    };
    let mut leitor = BufReader::new(prazo.take((TETO_CABECALHO + TETO_CORPO) as u64));
    let mut linhas = Vec::new();
    let mut lidos = 0;
    loop {
        let mut l = String::new();
        let n = leitor
            .read_line(&mut l)
            .map_err(|_| (408, "pedido ilegivel ou lento demais".to_string()))?;
        lidos += n;
        if n == 0 || lidos > TETO_CABECALHO {
            return Err((431, "cabecalho grande demais ou cortado".into()));
        }
        let l = l.trim_end().to_string();
        if l.is_empty() {
            break;
        }
        linhas.push(l);
    }
    let primeira = linhas.first().ok_or((400, "pedido vazio".to_string()))?;
    let mut p = primeira.split(' ');
    let metodo = p.next().unwrap_or_default().to_string();
    let caminho = p.next().unwrap_or("/").to_string();
    let (mut tamanho, mut token, mut host, mut tipo) = (0usize, None, None, None);
    for l in &linhas[1..] {
        if let Some((k, v)) = l.split_once(':') {
            let v = v.trim();
            match k.to_ascii_lowercase().as_str() {
                "content-length" => {
                    tamanho = v
                        .parse()
                        .map_err(|_| (400, "Content-Length invalido".to_string()))?
                }
                // Com proxy na frente, corpo em pedacos passaria pelo teto
                // sem ser contado: recusa em vez de ler errado.
                "transfer-encoding" => return Err((501, "Transfer-Encoding nao suportado".into())),
                "authorization" => token = v.strip_prefix("Bearer ").map(str::to_string),
                "host" => host = Some(v.to_lowercase()),
                "content-type" => tipo = Some(v.to_lowercase()),
                _ => {}
            }
        }
    }
    if tamanho > TETO_CORPO {
        return Err((413, "corpo grande demais".into()));
    }
    let mut corpo = vec![0u8; tamanho];
    leitor
        .read_exact(&mut corpo)
        .map_err(|_| (408, "corpo cortado ou lento demais".to_string()))?;
    let corpo = String::from_utf8(corpo).map_err(|_| (400, "corpo nao e UTF-8".to_string()))?;
    Ok(Pedido {
        metodo,
        caminho,
        token,
        host,
        tipo,
        ip,
        corpo,
    })
}

fn responder(fio: &mut TcpStream, status: u16, tipo: &str, corpo: &str) -> std::io::Result<()> {
    let frase = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        421 => "Misdirected Request",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        _ => "Error",
    };
    write!(
        fio,
        "HTTP/1.1 {status} {frase}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\n\
Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\n\
Referrer-Policy: no-referrer\r\n\
Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; frame-ancestors 'none'\r\n\
Connection: close\r\n\r\n{corpo}",
        corpo.len()
    )
}
