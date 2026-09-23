//! O painel pela rede: um servidor HTTP minimo (so `std`) com a API JSON e a
//! tela embutida.
//!
//! # As guardas, na ordem em que o pedido passa por elas
//!
//! 1. **Teto de conexoes** (`Semaforo` do nucleo): a conexao 257 e fechada na
//!    hora, em vez de virar mais uma thread (achado M1).
//! 2. **Prazo total por pedido**: 10 s para chegar inteiro. O prazo por
//!    leitura sozinho deixava um byte a cada 14 s segurar a thread por horas.
//! 3. **Tetos antes de alocar**: cabecalho 16 KiB, corpo 256 KiB -- a licao
//!    do pedido 434 do PhxSql.
//! 4. **`Host` conferido** contra a lista (loopback + nomes declarados):
//!    DNS rebinding chega com o nome do atacante e morre com 421 (A5).
//! 5. **POST so com `Content-Type: application/json`**: um site qualquer nao
//!    manda isso sem preflight de CORS, e o painel nao responde preflight --
//!    o `fetch` em `no-cors` da pagina maliciosa morre com 415 (A5).
//! 6. **Instalacao so com o codigo de uso unico** impresso no terminal do
//!    painel: quem so alcanca a porta nao instala com admin e senha dele (A5).
//! 7. **Tentativas de senha limitadas** por login, por IP e por rede, e a
//!    conta cara (PBKDF2) FORA da trava do painel, com no maximo
//!    `CONFERENCIAS` ao mesmo tempo (A2, A3).
//!
//! # Texto claro
//!
//! O painel fala HTTP sem TLS (a petrea de zero dependencia ainda nao tem TLS
//! escrito aqui). Por isso escuta em 127.0.0.1, e so escuta fora disso com
//! `--aceito-sem-tls` escrito (A4, versao minima).

use crate::guarda::{frase_de_bloqueio, Limitador};
use crate::painel::{conferir_login, conferir_rede, Instalacao, Painel, Usuario};
use crate::supervisor::Supervisor;
use phxsql_core::hash::{iguais_em_tempo_constante, para_hex};
use phxsql_core::json::Json;
use phxsql_core::semaforo::Semaforo;
use phxsql_core::senha::bytes_aleatorios;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const TETO_CABECALHO: usize = 16 * 1024;
const TETO_CORPO: usize = 256 * 1024;
const VIDA_SESSAO: Duration = Duration::from_secs(8 * 3600);
/// Conexoes atendidas ao mesmo tempo.
pub const TETO_CONEXOES: usize = 256;
/// Prazo para o pedido chegar inteiro.
const PRAZO_PEDIDO: Duration = Duration::from_secs(10);
/// Conferencias de senha (PBKDF2, ~430 ms cada) ao mesmo tempo.
const CONFERENCIAS: usize = 4;
const TELA: &str = include_str!("tela.html");
const TELA_JS: &str = include_str!("tela.js");

pub struct Estado {
    painel: Mutex<Painel>,
    sessoes: Mutex<HashMap<String, (Usuario, Instant)>>,
    pub supervisor: Option<Supervisor>,
    tentativas: Limitador,
    conferencias: Semaforo,
    conexoes: Semaforo,
    /// Valores aceitos no cabecalho `Host`, em minusculas.
    hosts: Vec<String>,
    /// Codigo de uso unico da instalacao; some depois de usado.
    codigo_instalacao: Mutex<Option<String>>,
}

impl Estado {
    pub fn novo(painel: Painel, supervisor: Option<Supervisor>) -> Estado {
        Estado {
            painel: Mutex::new(painel),
            sessoes: Mutex::new(HashMap::new()),
            supervisor,
            tentativas: Limitador::default(),
            conferencias: Semaforo::novo(CONFERENCIAS),
            conexoes: Semaforo::novo(TETO_CONEXOES),
            hosts: Vec::new(),
            codigo_instalacao: Mutex::new(None),
        }
    }

    /// Nomes pelos quais o painel aceita ser chamado: os de loopback na porta
    /// dele, o proprio endereco de escuta e os declarados em `--nome`.
    pub fn com_hosts(mut self, escuta: &str, nomes: &[String]) -> Estado {
        let porta = escuta.rsplit(':').next().unwrap_or("8470");
        let mut h: Vec<String> = ["127.0.0.1", "localhost", "[::1]"]
            .iter()
            .map(|n| format!("{n}:{porta}"))
            .collect();
        h.push(escuta.to_string());
        h.extend(nomes.iter().cloned());
        self.hosts = h.into_iter().map(|x| x.to_lowercase()).collect();
        self
    }

    /// Gera o codigo de instalacao (so quando ainda nao esta instalado) e o
    /// devolve para o terminal do painel mostrar.
    pub fn gerar_codigo_instalacao(&self) -> String {
        let codigo = para_hex(&bytes_aleatorios(6));
        *self
            .codigo_instalacao
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(codigo.clone());
        codigo
    }

    /// A trava do painel. Envenenada (panico de quem a segurava), ela nao
    /// derruba o painel para sempre: a conexao com o banco e dada por
    /// quebrada e refeita na proxima chamada (achado M7).
    pub fn painel(&self) -> MutexGuard<'_, Painel> {
        self.painel.lock().unwrap_or_else(|envenenada| {
            let mut p = envenenada.into_inner();
            p.depois_de_panico();
            p
        })
    }

    fn sessoes(&self) -> MutexGuard<'_, HashMap<String, (Usuario, Instant)>> {
        self.sessoes.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn servir(escuta: &str, estado: Arc<Estado>) -> Result<(), String> {
    let ouvinte = TcpListener::bind(escuta).map_err(|e| format!("escutar em {escuta}: {e}"))?;
    loop {
        let conexao = match ouvinte.accept() {
            Ok((c, _)) => c,
            Err(_) => {
                // Sem descritor (EMFILE) o accept falha na hora; sem a pausa,
                // o laco gira a 100% de CPU (achado M1).
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        let Some(permissao) = estado.conexoes.tentar() else {
            drop(conexao); // cheio: fecha na hora, sem thread nova
            continue;
        };
        let estado = Arc::clone(&estado);
        std::thread::spawn(move || {
            let _permissao = permissao;
            let _ = atender(conexao, &estado);
        });
    }
}

struct Pedido {
    metodo: String,
    caminho: String,
    token: Option<String>,
    host: Option<String>,
    tipo: Option<String>,
    ip: IpAddr,
    corpo: String,
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

fn atender(mut fio: TcpStream, estado: &Estado) -> std::io::Result<()> {
    let pedido = match ler_pedido(&fio) {
        Ok(p) => p,
        Err((s, m)) => return responder(&mut fio, s, "application/json", &erro_json(&m)),
    };
    // Host fora da lista: DNS rebinding ou endereco errado. Vale para tudo,
    // inclusive a tela -- e a tela na origem do atacante que roubaria dado.
    let host_ok = pedido
        .host
        .as_ref()
        .is_some_and(|h| estado.hosts.iter().any(|x| x == h));
    if !host_ok {
        return responder(
            &mut fio,
            421,
            "application/json",
            &erro_json("Host nao reconhecido por este painel (use --nome para declarar)"),
        );
    }
    let caminho = pedido.caminho.split('?').next().unwrap_or_default();
    if pedido.metodo == "GET" && (caminho == "/" || caminho == "/index.html") {
        return responder(&mut fio, 200, "text/html; charset=utf-8", TELA);
    }
    if pedido.metodo == "GET" && caminho == "/tela.js" {
        return responder(&mut fio, 200, "text/javascript; charset=utf-8", TELA_JS);
    }
    if pedido.metodo == "POST"
        && !pedido
            .tipo
            .as_deref()
            .is_some_and(|t| t.split(';').next() == Some("application/json"))
    {
        return responder(
            &mut fio,
            415,
            "application/json",
            &erro_json("POST so com Content-Type: application/json"),
        );
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

/// Erro de regra volta como esta; erro cru do PostgreSQL (que revela o
/// esquema) vai para o log do painel e o cliente ve uma frase generica.
fn ruim(e: String) -> (u16, String) {
    if e.starts_with("PostgreSQL ") || e.starts_with("conexao com o PostgreSQL") {
        eprintln!("phxvpn: {e}");
        return (
            500,
            "erro interno do banco (detalhe no log do painel)".into(),
        );
    }
    (400, e)
}

fn bloqueado(falta: Duration) -> (u16, String) {
    (429, frase_de_bloqueio(falta))
}

fn rotear(p: &Pedido, e: &Estado) -> Saida {
    let corpo = if p.corpo.trim().is_empty() {
        Json::Objeto(vec![])
    } else {
        Json::analisar(&p.corpo).map_err(|x| (400, format!("JSON invalido: {x}")))?
    };
    let t = |c: &str| corpo.texto_ou(c, "").to_string();
    let caminho = p.caminho.split('?').next().unwrap_or_default();
    let chave_ip = format!("ip:{}", p.ip);

    match (p.metodo.as_str(), caminho) {
        ("GET", "/api/estado") => {
            let mut painel = e.painel();
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
            e.tentativas.antes(&chave_ip).map_err(bloqueado)?;
            {
                let codigo = e
                    .codigo_instalacao
                    .lock()
                    .unwrap_or_else(|x| x.into_inner());
                let bate = codigo.as_deref().is_some_and(|c| {
                    iguais_em_tempo_constante(
                        c.as_bytes(),
                        t("codigo_instalacao").trim().as_bytes(),
                    )
                });
                if !bate {
                    drop(codigo);
                    e.tentativas.falhou(&chave_ip);
                    return Err((
                        403,
                        "codigo de instalacao nao confere (ele aparece no terminal do painel)"
                            .into(),
                    ));
                }
            }
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
            e.painel().instalar(&i).map_err(ruim)?;
            // Uso unico: instalado, o codigo morre.
            *e.codigo_instalacao
                .lock()
                .unwrap_or_else(|x| x.into_inner()) = None;
            ok()
        }
        ("POST", "/api/login") => {
            let login = t("usuario");
            let chave_login = format!("login:{login}");
            e.tentativas
                .antes_de_todas(&[&chave_login, &chave_ip])
                .map_err(bloqueado)?;
            // Banco com a trava; PBKDF2 sem ela, e no maximo CONFERENCIAS de
            // uma vez -- o resto espera a vez em vez de parar o painel.
            let (u, hash) = e.painel().hash_do_login(&login).map_err(ruim)?;
            let resultado = {
                let _vez = e.conferencias.adquirir();
                conferir_login(u, &hash, &t("senha"))
            };
            let u = match resultado {
                Ok(u) => {
                    e.tentativas.acertou(&chave_login);
                    u
                }
                Err(m) => {
                    e.tentativas.falhou(&chave_login);
                    e.tentativas.falhou(&chave_ip);
                    return Err((401, m));
                }
            };
            let token = para_hex(&bytes_aleatorios(32));
            let mut s = e.sessoes();
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
                e.sessoes().remove(tk);
            }
            ok()
        }
        ("POST", "/api/destrancar") => {
            let u = usuario(p, e)?;
            exigir_admin(&u)?;
            let chave = format!("mestre:{}", u.id);
            e.tentativas.antes(&chave).map_err(bloqueado)?;
            if let Err(m) = e.painel().destrancar(&t("senha_mestre")) {
                e.tentativas.falhou(&chave);
                return Err(ruim(m));
            }
            e.tentativas.acertou(&chave);
            materializar_e_subir(e).map_err(ruim)?;
            ok()
        }
        ("GET", "/api/redes") => {
            let u = usuario(p, e)?;
            e.painel().redes(&u).map_err(ruim)
        }
        ("POST", "/api/redes") => {
            let u = usuario(p, e)?;
            let servidor = corpo.campo("servidor_id").and_then(Json::inteiro);
            let perfil = e
                .painel()
                .criar_rede(&u, &t("nome"), &t("senha"), &t("finalidade"), servidor)
                .map_err(ruim)?;
            materializar_e_subir(e).map_err(ruim)?;
            perfil_json(&t("nome"), perfil)
        }
        ("POST", "/api/redes/entrar") => {
            let u = usuario(p, e)?;
            let nome = t("nome");
            // Por usuario E rede: quem erra a senha de uma rede nao trava as
            // outras; e o IP segura quem troca de conta.
            let chave_rede = format!("rede:{}:{nome}", u.id);
            e.tentativas
                .antes_de_todas(&[&chave_rede, &chave_ip])
                .map_err(bloqueado)?;
            let hash = e.painel().hash_da_rede(&nome).map_err(ruim)?;
            let conferido = {
                let _vez = e.conferencias.adquirir();
                conferir_rede(&hash, &t("senha"))
            };
            if let Err(m) = conferido {
                e.tentativas.falhou(&chave_rede);
                e.tentativas.falhou(&chave_ip);
                return Err((400, m));
            }
            e.tentativas.acertou(&chave_rede);
            let perfil = e.painel().entrar_ja_conferido(&u, &nome).map_err(ruim)?;
            perfil_json(&nome, perfil)
        }
        ("POST", "/api/redes/sair") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel().sair_da_rede(&u, id).map_err(ruim)?;
            ok()
        }
        ("POST", "/api/redes/remover") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel()
                .remover_membro(&u, id, &t("login"))
                .map_err(|m| (403, m))?;
            ok()
        }
        ("POST", "/api/redes/membros") => {
            let u = usuario(p, e)?;
            let id = rede_id(&corpo)?;
            e.painel().membros(&u, id).map_err(|m| (403, m))
        }
        ("GET", "/api/usuarios") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel().usuarios().map_err(ruim)
        }
        ("POST", "/api/usuarios") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel()
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
            e.painel().servidores().map_err(ruim)
        }
        ("POST", "/api/servidores") => {
            exigir_admin(&usuario(p, e)?)?;
            e.painel()
                .criar_servidor(&t("nome"), &t("ip"), &t("dns"))
                .map_err(ruim)?;
            ok()
        }
        _ => Err((404, format!("rota desconhecida: {} {caminho}", p.metodo))),
    }
}

fn rede_id(corpo: &Json) -> Result<i64, (u16, String)> {
    corpo
        .campo("rede_id")
        .and_then(Json::inteiro)
        .ok_or((400, "rede_id".into()))
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
        ("arquivo", Json::texto_de(format!("phxvpn-{arquivo}.ovpn"))),
        ("perfil", Json::texto_de(perfil)),
    ]))
}

fn usuario(p: &Pedido, e: &Estado) -> Result<Usuario, (u16, String)> {
    let tk = p.token.as_ref().ok_or((401, "faca login".to_string()))?;
    let s = e.sessoes();
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
    let redes = e.painel().materializar_todas()?;
    if let Some(s) = &e.supervisor {
        for (nome, dir) in redes {
            s.garantir(&nome, &dir)?;
        }
    }
    Ok(())
}
