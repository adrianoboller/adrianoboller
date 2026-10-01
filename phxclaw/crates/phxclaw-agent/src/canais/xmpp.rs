//! XMPP (Jabber): conexao de cliente com SASL PLAIN, bind de recurso e presenca; as
//! mensagens `type='chat'` viram tarefa e a resposta volta como `<message>` ao JID.
//!
//! Mesmo desenho do IRC: a conexao fica aberta, uma thread le o fluxo XML, responde ao ping
//! do servidor (XEP-0199) na hora e poe as mensagens numa fila; o que chega vai para a
//! `Caixa` em disco antes de virar tarefa. TLS por STARTTLS (RFC 6120 §5) pelo
//! `canais::tls`, com o certificado conferido contra o DOMINIO do JID; servidor que nao
//! oferece `<starttls/>` e recusado (nunca se cai para texto claro calado). Sem TLS
//! (`PHXCLAW_XMPP_TLS=false`), so loopback. Sala multiusuario (MUC, `groupchat`) fica de fora.
//!
//! O fluxo XML e lido por recorte de estrofe (`<message ...>...</message>`) e nao por um
//! analisador de fluxo: as estrofes que importam nao se aninham, e o analisador do
//! quick-xml quer o documento inteiro, que num fluxo XMPP so termina quando a conexao cai.

use super::caixa::Caixa;
use super::cripto::base64;
use super::http::Credencial;
use super::tls::{Fio, Tls, conectar, conectar_para_starttls, ler_em_fundo};
use super::{Entrada, Mensagem, Provedor, Unidade};
use serde_json::Value;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Config {
    pub endereco: String,
    /// `agente@dominio`.
    pub jid: String,
    pub senha: Credencial,
    /// `None` = texto claro, so em loopback; com TLS, STARTTLS obrigatorio.
    pub tls: Option<Tls>,
}

struct Conexao {
    escrita: Arc<Mutex<Fio>>,
    estrofes: Mutex<Receiver<String>>,
}

pub struct Xmpp {
    cfg: Config,
    caixa: Arc<Caixa>,
    conexao: Mutex<Option<Arc<Conexao>>>,
}

pub fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
        .replace('"', "&quot;")
}

pub fn desescapar(s: &str) -> String {
    let mut saida = String::new();
    let mut resto = s;
    while let Some(i) = resto.find('&') {
        saida.push_str(&resto[..i]);
        let fim = resto[i..].find(';').map(|f| i + f);
        let Some(f) = fim else {
            saida.push_str(&resto[i..]);
            return saida;
        };
        let ent = &resto[i + 1..f];
        let c = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match c {
            Some(c) => saida.push(c),
            None => saida.push_str(&resto[i..=f]),
        }
        resto = &resto[f + 1..];
    }
    saida.push_str(resto);
    saida
}

/// Valor de um atributo na marca de abertura da estrofe.
pub fn atributo(estrofe: &str, nome: &str) -> Option<String> {
    let abertura = &estrofe[..estrofe.find('>')?];
    for aspas in ['\'', '"'] {
        let chave = format!(" {nome}={aspas}");
        if let Some(i) = abertura.find(&chave) {
            let resto = &abertura[i + chave.len()..];
            return Some(desescapar(&resto[..resto.find(aspas)?]));
        }
    }
    None
}

/// `<message from='a@b/r' type='chat'><body>oi</body></message>` como (JID sem recurso,
/// id, texto). Estado de digitacao (`<composing/>` sem `<body>`) nao e mensagem.
pub fn mensagem_da_estrofe(estrofe: &str) -> Option<(String, String, String)> {
    if atributo(estrofe, "type").as_deref() != Some("chat") {
        return None;
    }
    let de = atributo(estrofe, "from")?;
    let nu = de.split('/').next()?.to_string();
    let i = estrofe.find("<body")?;
    let abre = i + estrofe[i..].find('>')?;
    if estrofe[..abre].ends_with('/') {
        return None;
    }
    let fecha = estrofe.find("</body>")?;
    let texto = desescapar(&estrofe[abre + 1..fecha]);
    Some((nu, atributo(estrofe, "id").unwrap_or_default(), texto))
}

/// Tira do buffer as estrofes inteiras de `message` e `iq`; o resto fica para a proxima
/// leitura.
fn recortar(buf: &mut String) -> Vec<String> {
    let mut saida = Vec::new();
    loop {
        let inicio = [buf.find("<message"), buf.find("<iq")]
            .into_iter()
            .flatten()
            .min();
        let Some(i) = inicio else {
            // Nada aproveitavel: guarda so o fim, que pode ser o comeco de uma estrofe.
            if buf.len() > 64 {
                let corte = buf.len() - 64;
                let corte = (corte..buf.len())
                    .find(|c| buf.is_char_boundary(*c))
                    .unwrap_or(0);
                buf.drain(..corte);
            }
            return saida;
        };
        let nome = if buf[i..].starts_with("<message") {
            "message"
        } else {
            "iq"
        };
        let Some(gt) = buf[i..].find('>').map(|g| i + g) else {
            return saida;
        };
        let fim = if buf[..gt].ends_with('/') {
            gt + 1
        } else {
            match buf[gt..].find(&format!("</{nome}>")) {
                Some(f) => gt + f + nome.len() + 3,
                None => {
                    buf.drain(..i);
                    return saida;
                }
            }
        };
        saida.push(buf[i..fim].to_string());
        buf.drain(..fim);
    }
}

fn escrever(s: &Mutex<Fio>, x: &str) -> Result<(), String> {
    let mut g = s.lock().map_err(|_| "soquete envenenado".to_string())?;
    escrever_em(&mut g, x)
}

fn escrever_em(s: &mut Fio, x: &str) -> Result<(), String> {
    s.write_all(x.as_bytes())
        .and_then(|_| s.flush())
        .map_err(|e| format!("xmpp: escrita: {e}"))
}

/// Le ate o buffer conter um dos marcadores, ou o prazo vencer.
fn ler_ate(s: &mut Fio, buf: &mut String, marcas: &[&str]) -> Result<String, String> {
    let fim = Instant::now() + Duration::from_secs(15);
    let mut bloco = [0u8; 4096];
    loop {
        if let Some(m) = marcas.iter().find(|m| buf.contains(**m)) {
            return Ok((*m).to_string());
        }
        if Instant::now() >= fim {
            return Err(format!(
                "xmpp: o servidor nao respondeu ({})",
                marcas.join(" | ")
            ));
        }
        s.set_read_timeout(Some(Duration::from_millis(500))).ok();
        match s.read(&mut bloco) {
            Ok(0) => return Err("xmpp: o servidor fechou a conexao".into()),
            Ok(n) => buf.push_str(&String::from_utf8_lossy(&bloco[..n])),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(format!("xmpp: leitura: {e}")),
        }
    }
}

impl Xmpp {
    pub fn novo(cfg: Config, caixa: Arc<Caixa>) -> Self {
        Self {
            cfg,
            caixa,
            conexao: Mutex::new(None),
        }
    }

    fn cabecalho(&self) -> Result<String, String> {
        let dominio = self.cfg.jid.split('@').nth(1).ok_or("jid sem dominio")?;
        Ok(format!(
            "<?xml version='1.0'?><stream:stream to='{}' xmlns='jabber:client' \
xmlns:stream='http://etherx.jabber.org/streams' version='1.0'>",
            escapar(dominio)
        ))
    }

    fn abrir(&self) -> Result<Conexao, String> {
        let dominio = self.cfg.jid.split('@').nth(1).ok_or("jid sem dominio")?;
        let mut s = match &self.cfg.tls {
            None => conectar(&self.cfg.endereco, None)?,
            Some(_) => conectar_para_starttls(&self.cfg.endereco)?,
        };
        let usuario = self.cfg.jid.split('@').next().unwrap_or("").to_string();
        let mut buf = String::new();
        escrever_em(&mut s, &self.cabecalho()?)?;
        ler_ate(&mut s, &mut buf, &["</stream:features>"])?;
        if let Some(tls) = &self.cfg.tls {
            // RFC 6120 §5.4: o servidor anuncia, o cliente pede, o servidor diz `proceed`, e
            // dali em diante o fluxo recomeca por dentro do TLS. Sem o anuncio, recusa: seguir
            // em texto claro mandaria a senha do SASL aberta.
            if !buf.contains("urn:ietf:params:xml:ns:xmpp-tls") {
                return Err("xmpp: o servidor nao oferece STARTTLS".into());
            }
            buf.clear();
            escrever_em(
                &mut s,
                "<starttls xmlns='urn:ietf:params:xml:ns:xmpp-tls'/>",
            )?;
            if ler_ate(&mut s, &mut buf, &["<proceed", "<failure"])? == "<failure" {
                return Err("xmpp: o servidor recusou o STARTTLS".into());
            }
            buf.clear();
            s = s.subir(dominio, tls)?;
            escrever_em(&mut s, &self.cabecalho()?)?;
            ler_ate(&mut s, &mut buf, &["</stream:features>"])?;
        }
        if !buf.contains("PLAIN") {
            return Err("xmpp: o servidor nao oferece SASL PLAIN".into());
        }
        buf.clear();
        self.cfg.senha.com("receive", |senha| {
            let credencial = base64(format!("\0{usuario}\0{senha}").as_bytes());
            escrever_em(
                &mut s,
                &format!("<auth xmlns='urn:ietf:params:xml:ns:xmpp-sasl' mechanism='PLAIN'>{credencial}</auth>"),
            )
        })?;
        if ler_ate(&mut s, &mut buf, &["<success", "<failure"])? == "<failure" {
            return Err("xmpp: usuario ou senha recusados".into());
        }
        buf.clear();
        // Depois do SASL o fluxo recomeca do zero (RFC 6120 §6.4.6).
        escrever_em(&mut s, &self.cabecalho()?)?;
        ler_ate(&mut s, &mut buf, &["</stream:features>"])?;
        buf.clear();
        escrever_em(
            &mut s,
            "<iq type='set' id='bind1'><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'><resource>phxclaw</resource></bind></iq>",
        )?;
        ler_ate(&mut s, &mut buf, &["</iq>", "/>"])?;
        if buf.contains("type='error'") || buf.contains("type=\"error\"") {
            return Err("xmpp: o servidor recusou o bind".into());
        }
        buf.clear();
        escrever_em(&mut s, "<presence/>")?;
        s.set_read_timeout(None).ok();
        let escrita = Arc::new(Mutex::new(s));
        let (tx, rx) = channel();
        // Fraco, como no IRC: a leitura em fundo nao pode manter o fio vivo sozinha.
        let pong = Arc::downgrade(&escrita);
        let mut buf = String::new();
        ler_em_fundo(escrita.clone(), move |bytes| {
            buf.push_str(&String::from_utf8_lossy(bytes));
            for e in recortar(&mut buf) {
                if e.starts_with("<iq") {
                    if e.contains("urn:xmpp:ping")
                        && atributo(&e, "type").as_deref() == Some("get")
                        && let Some(p) = pong.upgrade()
                    {
                        let id = escapar(&atributo(&e, "id").unwrap_or_default());
                        let para = escapar(&atributo(&e, "from").unwrap_or_default());
                        let _ = escrever(&p, &format!("<iq type='result' id='{id}' to='{para}'/>"));
                    }
                } else if tx.send(e).is_err() {
                    return false;
                }
            }
            true
        });
        Ok(Conexao {
            escrita,
            estrofes: Mutex::new(rx),
        })
    }

    fn com_conexao<T>(&self, f: impl FnOnce(&Conexao) -> Result<T, String>) -> Result<T, String> {
        let c = {
            let mut g = self
                .conexao
                .lock()
                .map_err(|_| "conexao envenenada".to_string())?;
            if g.is_none() {
                *g = Some(Arc::new(self.abrir()?));
            }
            g.as_ref().expect("acabou de abrir").clone()
        };
        let r = f(&c);
        if r.is_err()
            && let Ok(mut g) = self.conexao.lock()
            && g.as_ref().is_some_and(|x| Arc::ptr_eq(x, &c))
        {
            *g = None;
        }
        r
    }
}

impl Provedor for Xmpp {
    fn nome(&self) -> &str {
        "xmpp"
    }

    /// Servidores costumam limitar a estrofe a 10 mil bytes ou mais; 8 mil de corpo deixam
    /// folga ao envelope e ao escape.
    fn limite(&self) -> (usize, Unidade) {
        (8000, Unidade::Byte)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let eu = self.cfg.jid.to_ascii_lowercase();
        let novas = self.com_conexao(|c| {
            let fila = c
                .estrofes
                .lock()
                .map_err(|_| "fila envenenada".to_string())?;
            let mut novas = Vec::new();
            let mut espera = Duration::from_secs(espera_seg);
            loop {
                match fila.recv_timeout(espera) {
                    Ok(e) => {
                        if let Some((de, id, texto)) = mensagem_da_estrofe(&e)
                            && de.to_ascii_lowercase() != eu
                        {
                            novas.push((
                                Mensagem {
                                    conversa: de.clone(),
                                    autor: de,
                                    id,
                                    texto: Some(texto),
                                },
                                Value::Null,
                            ));
                        }
                        espera = Duration::from_millis(50);
                    }
                    Err(RecvTimeoutError::Timeout) => return Ok(novas),
                    Err(RecvTimeoutError::Disconnected) if !novas.is_empty() => return Ok(novas),
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err("xmpp: o servidor fechou a conexao".into());
                    }
                }
            }
        })?;
        if !novas.is_empty() {
            self.caixa.anexar(novas)?;
        }
        self.caixa.ler_desde(cursor, Duration::ZERO)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let id = phxclaw_types::new_uuid_v7().to_string();
        self.com_conexao(|c| {
            escrever(
                &c.escrita,
                &format!(
                    "<message to='{}' type='chat' id='{id}'><body>{}</body></message>",
                    escapar(conversa),
                    escapar(texto)
                ),
            )
        })?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estrofe_de_chat_vira_mensagem_e_digitacao_nao() {
        let e = "<message from='ana@x.org/celular' type='chat' id='m1'><body>oi &amp; &#233;</body></message>";
        assert_eq!(
            mensagem_da_estrofe(e),
            Some(("ana@x.org".into(), "m1".into(), "oi & \u{e9}".into()))
        );
        assert_eq!(
            mensagem_da_estrofe("<message from='a@x' type='chat'><composing xmlns='x'/></message>"),
            None
        );
        let mut b = "lixo<message type='chat' from='a@x'><body>1</body></message><iq type='get' id='p'/><mess".to_string();
        let r = recortar(&mut b);
        assert_eq!(r.len(), 2);
        assert_eq!(b, "<mess");
    }
}
