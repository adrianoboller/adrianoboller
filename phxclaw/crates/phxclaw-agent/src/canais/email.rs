//! E-mail como canal: le a caixa por IMAP (o cursor e o proximo UID) e responde pelo SMTP
//! do `email.rs` (`SmtpConfig::mandar`), o mesmo caminho da ferramenta `send_email`.
//!
//! Tres decisoes:
//! - o cursor e `UIDVALIDITY:proximo UID`. Se o servidor trocar o UIDVALIDITY (a caixa foi
//!   recriada), os UIDs antigos nao valem mais: o canal recomeca do agora em vez de
//!   responder a caixa inteira de novo;
//! - o `From` de um e-mail e texto que qualquer um escreve. A lista de permitidos so vale
//!   se o servidor que recebeu atesta o remetente, entao por padrao so entra mensagem com
//!   `Authentication-Results: ... dmarc=pass` (desligavel com
//!   `PHXCLAW_EMAIL_CANAL_EXIGIR_DMARC=nao`, para servidor que nao confere);
//! - IMAP sem TLS so em loopback (`irc::conectar_sem_tls`): o agente nao tem TLS de soquete
//!   sem crate nova, e a porta 993 de verdade fica para um tunel local. PARCIAL por isso.
//!   A saida SMTP tem TLS (lettre), e a senha dela tambem mora no broker.

use super::http::Credencial;
use super::irc::conectar_sem_tls;
use super::{Entrada, Mensagem, Provedor, Unidade, pausa_se_vazio};
use crate::email::SmtpConfig;
use base64::Engine;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

pub struct Config {
    pub endereco: String,
    pub usuario: String,
    /// Senha do IMAP, no broker.
    pub senha: Credencial,
    /// Senha do SMTP, no broker tambem: a `SmtpConfig` do canal vai sem senha, e ela so
    /// entra na copia que vive o tempo de um envio.
    pub smtp_senha: Option<Credencial>,
    pub pasta: String,
    pub exigir_dmarc: bool,
}

pub struct Email {
    cfg: Config,
    smtp: SmtpConfig,
}

struct Sessao {
    r: BufReader<TcpStream>,
    w: TcpStream,
    n: u32,
}

/// Resposta de um comando: as linhas e os literais (`{N}` seguido de N bytes), separados.
struct Resposta {
    linhas: Vec<String>,
    literais: Vec<Vec<u8>>,
}

fn citar(s: &str) -> Result<String, String> {
    if s.contains(['\r', '\n']) {
        return Err("quebra de linha em argumento IMAP".into());
    }
    Ok(format!(
        "\"{}\"",
        s.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

impl Sessao {
    fn abrir(endereco: &str) -> Result<Self, String> {
        let s = conectar_sem_tls(endereco)?;
        s.set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .ok();
        let w = s.try_clone().map_err(|e| e.to_string())?;
        let mut x = Self {
            r: BufReader::new(s),
            w,
            n: 0,
        };
        let mut l = String::new();
        x.r.read_line(&mut l).map_err(|e| format!("imap: {e}"))?;
        if !l.starts_with("* OK") {
            return Err(format!("imap: saudacao inesperada: {}", l.trim()));
        }
        Ok(x)
    }

    fn comando(&mut self, cmd: &str) -> Result<Resposta, String> {
        self.n += 1;
        let tag = format!("a{}", self.n);
        self.w
            .write_all(format!("{tag} {cmd}\r\n").as_bytes())
            .map_err(|e| format!("imap: {e}"))?;
        let mut r = Resposta {
            linhas: vec![],
            literais: vec![],
        };
        loop {
            let mut l = Vec::new();
            if self
                .r
                .read_until(b'\n', &mut l)
                .map_err(|e| format!("imap: {e}"))?
                == 0
            {
                return Err("imap: o servidor fechou a conexao".into());
            }
            let l = String::from_utf8_lossy(&l).to_string();
            let t = l.trim_end();
            if let Some(n) = t
                .strip_suffix('}')
                .and_then(|x| x.rsplit_once('{'))
                .and_then(|(_, n)| n.parse::<usize>().ok())
            {
                // Literal grande e e-mail de anexo: o teto impede um servidor de mandar o
                // agente alocar o que ele quiser.
                if n > 25 << 20 {
                    return Err(format!("imap: literal de {n} bytes passa do teto"));
                }
                let mut b = vec![0u8; n];
                self.r
                    .read_exact(&mut b)
                    .map_err(|e| format!("imap: {e}"))?;
                r.literais.push(b);
            }
            if let Some(resto) = t.strip_prefix(&format!("{tag} ")) {
                if resto.starts_with("OK") {
                    r.linhas.push(t.to_string());
                    return Ok(r);
                }
                return Err(format!("imap: {}", resto));
            }
            r.linhas.push(t.to_string());
        }
    }
}

/// Cabecalhos desdobrados (linha que comeca com espaco continua a anterior), em minusculas.
fn cabecalhos(bruto: &str) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for l in bruto.lines() {
        if l.starts_with([' ', '\t']) {
            if let Some(u) = v.last_mut() {
                u.1.push(' ');
                u.1.push_str(l.trim());
            }
        } else if let Some((k, x)) = l.split_once(':') {
            v.push((k.trim().to_ascii_lowercase(), x.trim().to_string()));
        }
    }
    v
}

fn separar(bruto: &str) -> (&str, &str) {
    bruto
        .split_once("\r\n\r\n")
        .or_else(|| bruto.split_once("\n\n"))
        .unwrap_or((bruto, ""))
}

fn parametro(valor: &str, nome: &str) -> Option<String> {
    valor.split(';').skip(1).find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k.trim().eq_ignore_ascii_case(nome)).then(|| v.trim().trim_matches('"').to_string())
    })
}

fn quoted_printable(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' {
            if b.get(i + 1) == Some(&b'\r') && b.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            if b.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            if let Some(h) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(h);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// O texto simples de uma parte MIME: em `multipart`, a primeira `text/plain`; o resto
/// (HTML, anexo) nao vira pedido. Charset que nao e UTF-8 cai no Latin-1, que cobre o
/// e-mail ocidental antigo.
fn texto_da_parte(bruto: &str) -> Option<String> {
    let (h, corpo) = separar(bruto);
    let hs = cabecalhos(h);
    let campo = |k: &str| hs.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone());
    let tipo = campo("content-type").unwrap_or_else(|| "text/plain".into());
    let t = tipo.to_ascii_lowercase();
    if t.starts_with("multipart/") {
        let fronteira = parametro(&tipo, "boundary")?;
        return corpo
            .split(&format!("--{fronteira}"))
            .skip(1)
            .filter(|p| !p.starts_with("--"))
            .find_map(|p| texto_da_parte(p.trim_start_matches(['\r', '\n'])));
    }
    if !t.starts_with("text/plain") {
        return None;
    }
    let bytes = match campo("content-transfer-encoding")
        .map(|c| c.to_ascii_lowercase())
        .as_deref()
    {
        Some("quoted-printable") => quoted_printable(corpo),
        Some("base64") => base64::engine::general_purpose::STANDARD
            .decode(corpo.split_whitespace().collect::<String>())
            .ok()?,
        _ => corpo.as_bytes().to_vec(),
    };
    Some(
        match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => e.into_bytes().iter().map(|b| *b as char).collect(),
        }
        .trim()
        .to_string(),
    )
}

/// O endereco de `Nome <a@b>` ou de `a@b`, em minusculas.
pub fn endereco(de: &str) -> String {
    let de = match (de.rfind('<'), de.rfind('>')) {
        (Some(a), Some(b)) if a < b => &de[a + 1..b],
        _ => de,
    };
    de.trim().to_ascii_lowercase()
}

/// Uma mensagem RFC 5322 como `Mensagem`, ou None se o remetente nao foi atestado e o
/// canal exige.
pub fn mensagem_do_email(bruto: &[u8], uid: u32, exigir_dmarc: bool) -> Option<Mensagem> {
    let bruto = String::from_utf8_lossy(bruto);
    let (h, _) = separar(&bruto);
    let hs = cabecalhos(h);
    let de = endereco(&hs.iter().find(|(c, _)| c == "from")?.1);
    let atestado = hs.iter().any(|(c, v)| {
        c == "authentication-results" && v.to_ascii_lowercase().contains("dmarc=pass")
    });
    if exigir_dmarc && !atestado {
        return None;
    }
    Some(Mensagem {
        conversa: de.clone(),
        autor: de,
        id: hs
            .iter()
            .find(|(c, _)| c == "message-id")
            .map_or_else(|| format!("uid{uid}"), |(_, v)| v.clone()),
        texto: texto_da_parte(&bruto),
    })
}

impl Email {
    pub fn novo(cfg: Config, smtp: SmtpConfig) -> Self {
        Self { cfg, smtp }
    }
}

impl Provedor for Email {
    fn nome(&self) -> &str {
        "email"
    }

    fn limite(&self) -> (usize, Unidade) {
        (100_000, Unidade::Byte)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let mut s = Sessao::abrir(&self.cfg.endereco)?;
        self.cfg.senha.com("receive", |senha| {
            s.comando(&format!(
                "LOGIN {} {}",
                citar(&self.cfg.usuario)?,
                citar(senha)?
            ))
            .map(|_| ())
        })?;
        let sel = s.comando(&format!("SELECT {}", citar(&self.cfg.pasta)?))?;
        let validade = sel
            .linhas
            .iter()
            .find_map(|l| {
                let i = l.find("UIDVALIDITY ")?;
                l[i + 12..].split(']').next()?.trim().parse::<u64>().ok()
            })
            .unwrap_or(0);
        let (val_antiga, proximo) = cursor
            .and_then(|c| c.split_once(':'))
            .and_then(|(v, u)| Some((v.parse::<u64>().ok()?, u.parse::<u32>().ok()?)))
            .unwrap_or((u64::MAX, 0));
        let uids = |r: &Resposta| -> Vec<u32> {
            r.linhas
                .iter()
                .filter_map(|l| l.strip_prefix("* SEARCH"))
                .flat_map(|l| {
                    l.split_whitespace()
                        .filter_map(|x| x.parse().ok())
                        .collect::<Vec<u32>>()
                })
                .collect()
        };
        let mut lote = Vec::new();
        if val_antiga != validade {
            // Primeira vez, ou caixa recriada: comeca do agora.
            let r = s.comando("UID SEARCH ALL")?;
            let max = uids(&r).into_iter().max().unwrap_or(0);
            lote.push(Entrada {
                cursor: Some(format!("{validade}:{}", max + 1)),
                mensagem: None,
            });
        } else {
            let r = s.comando(&format!("UID SEARCH UID {proximo}:*"))?;
            // `n:*` devolve o ultimo UID mesmo quando ele e menor que n (RFC 3501 §6.4.8).
            let mut novos: Vec<u32> = uids(&r).into_iter().filter(|u| *u >= proximo).collect();
            novos.sort_unstable();
            for uid in novos {
                let f = s.comando(&format!("UID FETCH {uid} (BODY.PEEK[])"))?;
                let m = f
                    .literais
                    .first()
                    .and_then(|b| mensagem_do_email(b, uid, self.cfg.exigir_dmarc));
                lote.push(Entrada {
                    cursor: Some(format!("{validade}:{}", uid + 1)),
                    mensagem: m,
                });
            }
        }
        let _ = s.comando("LOGOUT");
        pausa_se_vazio(&lote, espera_seg);
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        use lettre::Message;
        let msg = Message::builder()
            .from(
                self.smtp
                    .from
                    .parse()
                    .map_err(|e| format!("remetente: {e}"))?,
            )
            .to(conversa
                .parse()
                .map_err(|e| format!("destino {conversa}: {e}"))?)
            .subject("Resposta do agente")
            .body(texto.to_string())
            .map_err(|e| e.to_string())?;
        let mandar = |c: &SmtpConfig| {
            tokio::runtime::Handle::current()
                .block_on(c.mandar(msg.clone()))
                .map_err(|e| e.to_string())
        };
        let r = match &self.cfg.smtp_senha {
            Some(cred) => cred.com("send", |senha| {
                let mut c = self.smtp.clone();
                c.password = Some(senha.to_string());
                mandar(&c)
            })?,
            None => mandar(&self.smtp)?,
        };
        Ok(r.code().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_pega_o_texto_simples_e_decodifica() {
        let e = "From: Ana <Ana@X.org>\r\nMessage-ID: <1@x>\r\nAuthentication-Results: mx; dmarc=pass\r\nContent-Type: multipart/alternative; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/html\r\n\r\n<p>oi</p>\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nol=C3=A1 =\r\nmundo\r\n--b--\r\n";
        let m = mensagem_do_email(e.as_bytes(), 7, true).unwrap();
        assert_eq!(m.conversa, "ana@x.org");
        assert_eq!(m.id, "<1@x>");
        assert_eq!(m.texto.as_deref(), Some("ol\u{e1} mundo"));
        let sem = e.replace("dmarc=pass", "dmarc=fail");
        assert!(mensagem_do_email(sem.as_bytes(), 7, true).is_none());
        assert!(mensagem_do_email(sem.as_bytes(), 7, false).is_some());
    }
}
