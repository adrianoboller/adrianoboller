//! O painel pela rede: um servidor HTTP minimo (so `std`) com a API JSON e a
//! tela embutida.
//!
//! # Tetos ANTES de alocar
//!
//! Cabecalho ate 16 KiB e corpo ate 256 KiB, conferidos antes de ler -- a
//! licao do pedido 434 do PhxSql: leitura de linha sem teto num socket e alto
//! de memoria antes de qualquer credencial.
//!
//! # Texto claro
//!
//! O painel fala HTTP sem TLS (a petrea de zero dependencia ainda nao tem TLS
//! escrito aqui). Por isso o padrao e escutar em 127.0.0.1: para expor, ponha
//! um proxy com TLS na frente ou use o painel pela propria VPN. Ver
//! `docs/PHOENIX-VPN.md`, secao «Limites».

use crate::painel::{Instalacao, Painel, Usuario};
use crate::supervisor::Supervisor;
use phxsql_core::hash::para_hex;
use phxsql_core::json::Json;
use phxsql_core::senha::bytes_aleatorios;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TETO_CABECALHO: usize = 16 * 1024;
const TETO_CORPO: usize = 256 * 1024;
const VIDA_SESSAO: Duration = Duration::from_secs(8 * 3600);
const TELA: &str = include_str!("tela.html");

pub struct Estado {
    pub painel: Mutex<Painel>,
    sessoes: Mutex<HashMap<String, (Usuario, Instant)>>,
    pub supervisor: Option<Supervisor>,
}

impl Estado {
    pub fn novo(painel: Painel, supervisor: Option<Supervisor>) -> Estado {
        Estado {
            painel: Mutex::new(painel),
            sessoes: Mutex::new(HashMap::new()),
            supervisor,
        }
    }
}

pub fn servir(escuta: &str, estado: Arc<Estado>) -> Result<(), String> {
    let ouvinte = TcpListener::bind(escuta).map_err(|e| format!("escutar em {escuta}: {e}"))?;
    for conexao in ouvinte.incoming().flatten() {
        let estado = Arc::clone(&estado);
        std::thread::spawn(move || {
            let _ = atender(conexao, &estado);
        });
    }
    Ok(())
}

struct Pedido {
    metodo: String,
    caminho: String,
    token: Option<String>,
    corpo: String,
}

fn ler_pedido(fio: &TcpStream) -> Result<Pedido, (u16, String)> {
    let mut leitor = BufReader::new(fio.take((TETO_CABECALHO + TETO_CORPO) as u64));
    let mut linhas = Vec::new();
    let mut lidos = 0;
    loop {
        let mut l = String::new();
        let n = leitor
            .read_line(&mut l)
            .map_err(|_| (400, "pedido ilegivel".to_string()))?;
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
    let mut tamanho = 0usize;
    let mut token = None;
    for l in &linhas[1..] {
        if let Some((k, v)) = l.split_once(':') {
            let v = v.trim();
            match k.to_ascii_lowercase().as_str() {
                "content-length" => {
                    tamanho = v
                        .parse()
                        .map_err(|_| (400, "Content-Length invalido".to_string()))?
                }
                "authorization" => token = v.strip_prefix("Bearer ").map(str::to_string),
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
        .map_err(|_| (400, "corpo cortado".to_string()))?;
    let corpo = String::from_utf8(corpo).map_err(|_| (400, "corpo nao e UTF-8".to_string()))?;
    Ok(Pedido {
        metodo,
        caminho,
        token,
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
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Error",
    };
    write!(
        fio,
        "HTTP/1.1 {status} {frase}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\n\
Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\n\
Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'\r\n\
Connection: close\r\n\r\n{corpo}",
        corpo.len()
    )
}

fn atender(mut fio: TcpStream, estado: &Estado) -> std::io::Result<()> {
    fio.set_read_timeout(Some(Duration::from_secs(15)))?;
    let pedido = match ler_pedido(&fio) {
        Ok(p) => p,
        Err((s, m)) => return responder(&mut fio, s, "application/json", &erro_json(&m)),
    };
    if pedido.metodo == "GET" && (pedido.caminho == "/" || pedido.caminho == "/index.html") {
        return responder(&mut fio, 200, "text/html; charset=utf-8", TELA);
    }
    let (status, corpo) = match rotear(&pedido, estado) {
        Ok(j) => (200, j.escrever()),
        Err((s, m)) => (s, erro_json(&m)),
    };
    responder(&mut fio, status, "application/json; charset=utf-8", &corpo)
}

fn erro_json(m: &str) -> String {
    Json::objeto(vec![("erro", Json::texto_de(m))]).escrever()
}

type Saida = Result<Json, (u16, String)>;

fn ruim(e: String) -> (u16, String) {
    (400, e)
}

fn rotear(p: &Pedido, e: &Estado) -> Saida {
    let corpo = if p.corpo.trim().is_empty() {
        Json::Objeto(vec![])
    } else {
        Json::analisar(&p.corpo).map_err(|x| (400, format!("JSON invalido: {x}")))?
    };
    let t = |c: &str| corpo.texto_ou(c, "").to_string();
    let caminho = p.caminho.split('?').next().unwrap_or_default();

    match (p.metodo.as_str(), caminho) {
        ("GET", "/api/estado") => {
            let mut painel = e.painel.lock().expect("painel");
            let instalado = painel.instalado().map_err(ruim)?;
            // Sem login, so o nome: responsavel, e-mail e telefone sao dado
            // pessoal e nao se entregam a quem apenas alcanca a porta.
            let empresa = if instalado {
                let completa = painel.empresa().map_err(ruim)?;
                if usuario(p, e).is_ok() {
                    completa
                } else {
                    Json::objeto(vec![(
                        "nome",
                        Json::texto_de(completa.texto_ou("nome", "")),
                    )])
                }
            } else {
                Json::Nulo
            };
            Ok(Json::objeto(vec![
                ("instalado", Json::de_bool(instalado)),
                ("destrancado", Json::de_bool(painel.destrancado())),
                ("empresa", empresa),
                ("openvpn", Json::de_bool(e.supervisor.is_some())),
            ]))
        }
        ("POST", "/api/instalar") => {
            let i = Instalacao {
                empresa: t("empresa"),
                finalidade: t("finalidade"),
                responsavel: t("responsavel"),
                email: t("email"),
                telefone: t("telefone"),
                admin_usuario: t("admin_usuario"),
                admin_senha: t("admin_senha"),
                senha_mestre: t("senha_mestre"),
                servidor_nome: t("servidor_nome"),
                servidor_ip: t("servidor_ip"),
                servidor_dns: t("servidor_dns"),
                certificado_pem: t("certificado_pem"),
            };
            e.painel
                .lock()
                .expect("painel")
                .instalar(&i)
                .map_err(ruim)?;
            ok()
        }
        ("POST", "/api/login") => {
            let u = e
                .painel
                .lock()
                .expect("painel")
                .login(&t("usuario"), &t("senha"))
                .map_err(|m| (401, m))?;
            let token = para_hex(&bytes_aleatorios(32));
            let mut s = e.sessoes.lock().expect("sessoes");
            s.retain(|_, (_, quando)| quando.elapsed() < VIDA_SESSAO);
            s.insert(token.clone(), (u.clone(), Instant::now()));
            Ok(Json::objeto(vec![
                ("token", Json::texto_de(token)),
                ("login", Json::texto_de(u.login)),
                ("admin", Json::de_bool(u.admin)),
            ]))
        }
        ("POST", "/api/sair") => {
            if let Some(tk) = &p.token {
                e.sessoes.lock().expect("sessoes").remove(tk);
            }
            ok()
        }
        ("POST", "/api/destrancar") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel
                .lock()
                .expect("painel")
                .destrancar(&t("senha_mestre"))
                .map_err(ruim)?;
            materializar_e_subir(e).map_err(ruim)?;
            ok()
        }
        ("GET", "/api/redes") => {
            let u = usuario(p, e)?;
            e.painel.lock().expect("painel").redes(&u).map_err(ruim)
        }
        ("POST", "/api/redes") => {
            let u = usuario(p, e)?;
            let servidor = corpo.campo("servidor_id").and_then(Json::inteiro);
            let perfil = {
                let mut painel = e.painel.lock().expect("painel");
                painel
                    .criar_rede(&u, &t("nome"), &t("senha"), &t("finalidade"), servidor)
                    .map_err(ruim)?
            };
            materializar_e_subir(e).map_err(ruim)?;
            perfil_json(&t("nome"), perfil)
        }
        ("POST", "/api/redes/entrar") => {
            let u = usuario(p, e)?;
            let perfil = e
                .painel
                .lock()
                .expect("painel")
                .entrar_na_rede(&u, &t("nome"), &t("senha"))
                .map_err(ruim)?;
            perfil_json(&t("nome"), perfil)
        }
        ("POST", "/api/redes/sair") => {
            let u = usuario(p, e)?;
            let id = corpo
                .campo("rede_id")
                .and_then(Json::inteiro)
                .ok_or((400, "rede_id".into()))?;
            e.painel
                .lock()
                .expect("painel")
                .sair_da_rede(&u, id)
                .map_err(ruim)?;
            ok()
        }
        ("POST", "/api/redes/membros") => {
            let u = usuario(p, e)?;
            let id = corpo
                .campo("rede_id")
                .and_then(Json::inteiro)
                .ok_or((400, "rede_id".into()))?;
            e.painel
                .lock()
                .expect("painel")
                .membros(&u, id)
                .map_err(|m| (403, m))
        }
        ("GET", "/api/usuarios") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel.lock().expect("painel").usuarios().map_err(ruim)
        }
        ("POST", "/api/usuarios") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel
                .lock()
                .expect("painel")
                .criar_usuario(
                    &t("login"),
                    &t("senha"),
                    &t("email"),
                    corpo.booleano_ou("admin", false),
                )
                .map_err(ruim)?;
            ok()
        }
        ("GET", "/api/servidores") => {
            usuario(p, e)?;
            e.painel.lock().expect("painel").servidores().map_err(ruim)
        }
        ("POST", "/api/servidores") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel
                .lock()
                .expect("painel")
                .criar_servidor(&t("nome"), &t("ip"), &t("dns"))
                .map_err(ruim)?;
            ok()
        }
        _ => Err((404, format!("rota desconhecida: {} {caminho}", p.metodo))),
    }
}

fn ok() -> Saida {
    Ok(Json::objeto(vec![("ok", Json::de_bool(true))]))
}

fn perfil_json(rede: &str, perfil: String) -> Saida {
    let arquivo: String = rede
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(Json::objeto(vec![
        ("arquivo", Json::texto_de(format!("phoenix-{arquivo}.ovpn"))),
        ("perfil", Json::texto_de(perfil)),
    ]))
}

fn usuario(p: &Pedido, e: &Estado) -> Result<Usuario, (u16, String)> {
    let tk = p.token.as_ref().ok_or((401, "faca login".to_string()))?;
    let s = e.sessoes.lock().expect("sessoes");
    match s.get(tk) {
        Some((u, quando)) if quando.elapsed() < VIDA_SESSAO => Ok(u.clone()),
        _ => Err((401, "sessao expirada: faca login de novo".into())),
    }
}

fn exigir_admin(u: &Usuario) -> Result<(), (u16, String)> {
    if u.admin {
        Ok(())
    } else {
        Err((403, "so o administrador faz isso".into()))
    }
}

/// Reescreve os arquivos de todas as redes e, se o supervisor esta ligado,
/// sobe o OpenVPN das que ainda nao estao no ar.
pub fn materializar_e_subir(e: &Estado) -> Result<(), String> {
    let redes = e.painel.lock().expect("painel").materializar_todas()?;
    if let Some(s) = &e.supervisor {
        for (nome, dir) in redes {
            s.garantir(&nome, &dir)?;
        }
    }
    Ok(())
}
