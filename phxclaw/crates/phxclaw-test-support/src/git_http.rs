//! Um servidor git HTTP FALSO em loopback, para provar push e pull de rede sem rede: o
//! `git http-backend` de verdade (o CGI que o proprio git traz) atras de um servidor HTTP
//! minimo de std, uma conexao por pedido.
//!
//! Por que existe e nao e um `git daemon`: o daemon fala o protocolo `git://`, que nao tem
//! cabecalho nenhum -- a prova da credencial (o `Authorization` chegou ao servidor?) seria
//! impossivel. E nao e https porque um certificado de teste pediria uma CA confiada dentro do
//! sandbox; a regra do produto e https, e o http em loopback e a excecao que ela abre para a
//! prova.
//!
//! O que ele guarda para o teste ler: cada pedido (metodo, caminho) e cada `Authorization`
//! recebido. Com `exigir` definido, pedido sem aquele cabecalho exato leva 401 -- e o git
//! sem credencial falha em vez de passar por engano. Com `vigiar`, a cada pedido -- quando o
//! git do cliente esta VIVO, esperando a resposta -- le o `/proc/*/cmdline` de todo processo
//! e conta os que trazem algum daqueles textos: e a prova de que o segredo nao foi pelo argv,
//! que qualquer usuario da maquina le com `ps`. Vigie todas as formas do segredo (o valor
//! cru e o base64 do Basic): o argv leva o cabecalho pronto, nao o valor.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// O que o servidor viu.
#[derive(Debug, Default, Clone)]
pub struct Visto {
    /// `METODO /caminho?query`, na ordem.
    pub pedidos: Vec<String>,
    /// O valor de cada `Authorization` recebido (vazio quando o pedido nao trouxe).
    pub autorizacoes: Vec<String>,
    /// Processos vistos com o texto vigiado no argv, somados por pedido.
    pub argv_com_segredo: usize,
}

pub struct Servidor {
    /// `http://127.0.0.1:PORTA`
    pub base: String,
    pub visto: Arc<Mutex<Visto>>,
}

impl Servidor {
    /// Serve os repositorios bare de `raiz` (`<base>/<nome>.git`). `exigir`: o valor exato
    /// do `Authorization` que o servidor aceita (`None`: aceita qualquer pedido). `vigiar`: os
    /// textos que nao podem aparecer no argv de processo nenhum.
    pub fn subir(raiz: &Path, exigir: Option<String>, vigiar: Vec<String>) -> Self {
        let l = TcpListener::bind("127.0.0.1:0").expect("porta de loopback");
        let base = format!("http://{}", l.local_addr().unwrap());
        let visto: Arc<Mutex<Visto>> = Default::default();
        let v2 = visto.clone();
        let raiz = raiz.to_path_buf();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let (raiz, v, exigir) = (raiz.clone(), v2.clone(), exigir.clone());
                let n: usize = vigiar.iter().map(|t| argv_com(t)).sum();
                v.lock().unwrap().argv_com_segredo += n;
                std::thread::spawn(move || atender(s, &raiz, &v, exigir.as_deref()));
            }
        });
        Self { base, visto }
    }

    pub fn visto(&self) -> Visto {
        self.visto.lock().unwrap().clone()
    }
}

fn atender(s: TcpStream, raiz: &PathBuf, visto: &Mutex<Visto>, exigir: Option<&str>) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut primeira = String::new();
    if r.read_line(&mut primeira).unwrap_or(0) == 0 {
        return;
    }
    let mut cab: Vec<(String, String)> = Vec::new();
    loop {
        let mut linha = String::new();
        if r.read_line(&mut linha).unwrap_or(0) == 0 || linha == "\r\n" {
            break;
        }
        if let Some((k, v)) = linha.split_once(':') {
            cab.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let valor = |k: &str| cab.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let mut partes = primeira.split_whitespace();
    let metodo = partes.next().unwrap_or("").to_string();
    let alvo = partes.next().unwrap_or("").to_string();
    let aut = valor("authorization").unwrap_or_default();
    {
        let mut g = visto.lock().unwrap();
        g.pedidos.push(format!("{metodo} {alvo}"));
        g.autorizacoes.push(aut.clone());
    }
    let corpo = ler_corpo(&mut r, &valor);
    let mut s = s;
    if let Some(e) = exigir
        && aut != e
    {
        let _ = s.write_all(
            b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"falso\"\r\n\
Content-Length: 0\r\nConnection: close\r\n\r\n",
        );
        return;
    }
    let (caminho, query) = alvo.split_once('?').unwrap_or((&alvo, ""));
    let mut c = Command::new("git");
    c.arg("http-backend")
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("GIT_PROJECT_ROOT", raiz)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        // O receive-pack do http-backend so roda para usuario autenticado.
        .env("REMOTE_USER", "prova")
        .env("REQUEST_METHOD", &metodo)
        .env("PATH_INFO", caminho)
        .env("QUERY_STRING", query)
        .env("CONTENT_LENGTH", corpo.len().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(t) = valor("content-type") {
        c.env("CONTENT_TYPE", t);
    }
    if let Some(t) = valor("content-encoding") {
        c.env("HTTP_CONTENT_ENCODING", t);
    }
    if let Some(t) = valor("git-protocol") {
        c.env("GIT_PROTOCOL", t);
    }
    let Ok(mut filho) = c.spawn() else {
        let _ = s.write_all(b"HTTP/1.1 500 X\r\nConnection: close\r\n\r\n");
        return;
    };
    let mut entrada = filho.stdin.take().unwrap();
    std::thread::spawn(move || {
        let _ = entrada.write_all(&corpo);
    });
    let saida = filho
        .wait_with_output()
        .map(|o| o.stdout)
        .unwrap_or_default();
    // A saida do CGI: cabecalhos (com `Status:` opcional), linha vazia, corpo.
    let fim = saida
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| (p, 4))
        .or_else(|| saida.windows(2).position(|w| w == b"\n\n").map(|p| (p, 2)));
    let Some((p, n)) = fim else {
        let _ = s.write_all(b"HTTP/1.1 500 X\r\nConnection: close\r\n\r\n");
        return;
    };
    let cabecalhos = String::from_utf8_lossy(&saida[..p]).to_string();
    let mut status = "200 OK".to_string();
    let mut resposta = String::new();
    for l in cabecalhos.lines() {
        match l.split_once(':') {
            Some((k, v)) if k.eq_ignore_ascii_case("status") => status = v.trim().to_string(),
            Some(_) => resposta.push_str(&format!("{}\r\n", l.trim_end())),
            None => {}
        }
    }
    let _ =
        s.write_all(format!("HTTP/1.1 {status}\r\n{resposta}Connection: close\r\n\r\n").as_bytes());
    let _ = s.write_all(&saida[p + n..]);
}

/// Quantos processos (fora este) tem `texto` no argv.
fn argv_com(texto: &str) -> usize {
    let eu = std::process::id().to_string();
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return 0;
    };
    rd.flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.chars().all(|c| c.is_ascii_digit()) && n != eu
        })
        .filter(|e| {
            std::fs::read(e.path().join("cmdline"))
                .is_ok_and(|b| b.windows(texto.len()).any(|w| w == texto.as_bytes()))
        })
        .count()
}

/// O corpo pelo `Content-Length` ou pelo `Transfer-Encoding: chunked` (o git usa o segundo
/// quando o pacote passa do `http.postBuffer`).
fn ler_corpo(r: &mut impl BufRead, valor: &dyn Fn(&str) -> Option<String>) -> Vec<u8> {
    if valor("transfer-encoding").is_some_and(|t| t.eq_ignore_ascii_case("chunked")) {
        let mut corpo = Vec::new();
        loop {
            let mut tam = String::new();
            if r.read_line(&mut tam).unwrap_or(0) == 0 {
                break;
            }
            let n =
                usize::from_str_radix(tam.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
            let mut pedaco = vec![0; n + 2];
            if r.read_exact(&mut pedaco).is_err() || n == 0 {
                break;
            }
            corpo.extend_from_slice(&pedaco[..n]);
        }
        return corpo;
    }
    let n: usize = valor("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut corpo = vec![0; n];
    let _ = r.read_exact(&mut corpo);
    corpo
}

/// Um servidor que so CONTA conexoes e nunca responde git: o destino que um `.git/config`
/// escrito pelo modelo apontaria. Qualquer conexao nele e a prova do furo.
pub fn armadilha() -> (String, Arc<Mutex<usize>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("porta de loopback");
    let base = format!("http://{}", l.local_addr().unwrap());
    let n: Arc<Mutex<usize>> = Default::default();
    let n2 = n.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            *n2.lock().unwrap() += 1;
            let mut s = s;
            let _ =
                s.write_all(b"HTTP/1.1 404 X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    (base, n)
}
