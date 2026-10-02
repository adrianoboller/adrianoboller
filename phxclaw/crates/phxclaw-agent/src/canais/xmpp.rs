//! XMPP (Jabber): conexao de cliente com SASL PLAIN, bind de recurso e presenca; as
//! mensagens `type='chat'` viram tarefa e a resposta volta como `<message>` ao JID.
//! Sala multiusuario (MUC, XEP-0045): o agente entra nas `salas` da configuracao com o
//! `apelido` ao abrir a conexao; o `groupchat` vira tarefa com a sala como conversa e o nick
//! como autor, e a resposta volta `groupchat` a sala. O eco da propria fala (nick igual ao
//! apelido) e o historico que a sala manda ao entrar (`<delay/>`) ficam de fora: sem isso o
//! agente responderia a si mesmo e ao passado. Mensagem privada de ocupante (`chat` vindo
//! de `sala/nick`) chega com a conversa `sala/nick` inteira, para a resposta nao sair em
//! publico; so entra se o operador a permitir assim.
//!
//! Mesmo desenho do IRC: a conexao fica aberta, uma thread le o fluxo XML, responde ao ping
//! do servidor (XEP-0199) na hora e poe as mensagens numa fila; o que chega vai para a
//! `Caixa` em disco antes de virar tarefa. TLS por STARTTLS (RFC 6120 §5) pelo
//! `canais::tls`, com o certificado conferido contra o DOMINIO do JID; servidor que nao
//! oferece `<starttls/>` e recusado (nunca se cai para texto claro calado). Sem TLS
//! (`PHXCLAW_XMPP_TLS=false`), so loopback.
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
    /// JIDs das salas (XEP-0045) em que o agente entra ao abrir a conexao; vazio = nenhuma.
    pub salas: Vec<String>,
    /// Nick nas salas; vazio = a parte local do JID.
    pub apelido: String,
}

/// O que uma estrofe `<message>` carrega quando e tarefa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recebida {
    /// O `from` inteiro, com recurso.
    pub de: String,
    /// JID sem recurso: o remetente no `chat`, a sala no `groupchat`.
    pub conversa: String,
    /// O remetente sem recurso no `chat`; o nick (recurso) no `groupchat`.
    pub autor: String,
    pub id: String,
    pub texto: String,
    /// `type='groupchat'`.
    pub sala: bool,
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

/// `<message from='a@b/r' type='chat'><body>oi</body></message>` e o `groupchat` da sala
/// como `Recebida`. Estado de digitacao (`<composing/>` sem `<body>`) nao e mensagem; no
/// `groupchat`, tambem nao sao: a fala da propria sala (sem nick, e assunto ou aviso) e o
/// historico que a sala reenvia ao entrar (`<delay/>`, XEP-0203 ou o antigo `jabber:x:delay`),
/// porque o agente responderia a conversas de antes dele chegar.
pub fn mensagem_da_estrofe(estrofe: &str) -> Option<Recebida> {
    let sala = match atributo(estrofe, "type").as_deref() {
        Some("chat") => false,
        Some("groupchat") => true,
        _ => return None,
    };
    let de = atributo(estrofe, "from")?;
    let (nu, recurso) = match de.split_once('/') {
        Some((n, r)) => (n.to_string(), r.to_string()),
        None => (de.clone(), String::new()),
    };
    if sala
        && (recurso.is_empty()
            || estrofe.contains("urn:xmpp:delay")
            || estrofe.contains("jabber:x:delay"))
    {
        return None;
    }
    let i = estrofe.find("<body")?;
    let abre = i + estrofe[i..].find('>')?;
    if estrofe[..abre].ends_with('/') {
        return None;
    }
    let fecha = estrofe.find("</body>")?;
    let texto = desescapar(&estrofe[abre + 1..fecha]);
    Some(Recebida {
        de,
        autor: if sala { recurso } else { nu.clone() },
        conversa: nu,
        id: atributo(estrofe, "id").unwrap_or_default(),
        texto,
        sala,
    })
}

/// Motivo legivel de uma `<presence type='error'>` da sala (XEP-0045 §7.2), pelo filho da
/// condicao; sem condicao conhecida, o `code` cru. `None` se nao e erro.
pub fn erro_da_presenca(estrofe: &str) -> Option<String> {
    if atributo(estrofe, "type").as_deref() != Some("error") {
        return None;
    }
    let motivo = [
        ("<conflict", "o apelido ja esta em uso na sala"),
        ("<item-not-found", "a sala nao existe"),
        ("<not-authorized", "a sala pede senha"),
        ("<forbidden", "o agente esta banido da sala"),
        ("<registration-required", "a sala so aceita membros"),
        ("<service-unavailable", "a sala esta lotada"),
        ("<not-acceptable", "a sala nao aceita esse apelido"),
        (
            "<jid-malformed",
            "o JID da sala ou o apelido esta mal formado",
        ),
    ]
    .iter()
    .find(|(marca, _)| estrofe.contains(marca))
    .map(|(_, m)| (*m).to_string());
    Some(motivo.unwrap_or_else(|| {
        let i = estrofe.find("<error").unwrap_or(0);
        format!(
            "erro {}",
            atributo(&estrofe[i..], "code").unwrap_or_else(|| "sem condicao".into())
        )
    }))
}

/// A proxima estrofe inteira de um dos `nomes` a partir de `desde`: `Some(Ok((inicio, fim)))`;
/// `Some(Err(inicio))` se comecou e ainda nao terminou; `None` se nao ha nenhuma.
fn achar_estrofe(buf: &str, desde: usize, nomes: &[&str]) -> Option<Result<(usize, usize), usize>> {
    let (i, nome) = nomes
        .iter()
        .filter_map(|n| buf[desde..].find(&format!("<{n}")).map(|i| (desde + i, *n)))
        .min_by_key(|(i, _)| *i)?;
    let Some(gt) = buf[i..].find('>').map(|g| i + g) else {
        return Some(Err(i));
    };
    if buf[..gt].ends_with('/') {
        return Some(Ok((i, gt + 1)));
    }
    Some(match buf[gt..].find(&format!("</{nome}>")) {
        Some(f) => Ok((i, gt + f + nome.len() + 3)),
        None => Err(i),
    })
}

/// As estrofes inteiras de `presence` no buffer, sem tirar nada: o que vier junto
/// (mensagens da sala) segue para a leitura em fundo.
fn presencas(buf: &str) -> Vec<&str> {
    let mut saida = Vec::new();
    let mut desde = 0;
    while let Some(Ok((i, f))) = achar_estrofe(buf, desde, &["presence"]) {
        saida.push(&buf[i..f]);
        desde = f;
    }
    saida
}

/// Tira do buffer as estrofes inteiras de `message` e `iq`; o resto fica para a proxima
/// leitura.
fn recortar(buf: &mut String) -> Vec<String> {
    let mut saida = Vec::new();
    loop {
        match achar_estrofe(buf, 0, &["message", "iq"]) {
            None => {
                // Nada aproveitavel: guarda so o fim, que pode ser o comeco de uma estrofe.
                if buf.len() > 64 {
                    let corte = buf.len() - 64;
                    let corte = (corte..buf.len())
                        .find(|c| buf.is_char_boundary(*c))
                        .unwrap_or(0);
                    buf.drain(..corte);
                }
                return saida;
            }
            Some(Err(i)) => {
                buf.drain(..i);
                return saida;
            }
            Some(Ok((i, fim))) => {
                saida.push(buf[i..fim].to_string());
                buf.drain(..fim);
            }
        }
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
    ler_ate_que(s, buf, &marcas.join(" | "), |b| {
        marcas
            .iter()
            .find(|m| b.contains(**m))
            .map(|m| (*m).to_string())
    })
}

/// Le ate `decide` dar resposta sobre o buffer, ou o prazo vencer (`espera` vai no erro).
fn ler_ate_que<T>(
    s: &mut Fio,
    buf: &mut String,
    espera: &str,
    mut decide: impl FnMut(&str) -> Option<T>,
) -> Result<T, String> {
    let fim = Instant::now() + Duration::from_secs(15);
    let mut bloco = [0u8; 4096];
    loop {
        if let Some(t) = decide(buf) {
            return Ok(t);
        }
        if Instant::now() >= fim {
            return Err(format!("xmpp: o servidor nao respondeu ({espera})"));
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

/// Entra na sala (XEP-0045 §7.2) e espera a sala confirmar: a presenca de volta com o
/// proprio nick e o `status 110`, ou a de erro, que vira motivo legivel. Esperar e preciso:
/// sem a confirmacao, um 409 chegaria depois, misturado ao fluxo, e ninguem o veria.
fn entrar_na_sala(s: &mut Fio, buf: &mut String, sala: &str, apelido: &str) -> Result<(), String> {
    escrever_em(
        s,
        &format!(
            "<presence to='{}/{}'><x xmlns='http://jabber.org/protocol/muc'/></presence>",
            escapar(sala),
            escapar(apelido)
        ),
    )?;
    let minha = format!("{sala}/{apelido}").to_ascii_lowercase();
    ler_ate_que(s, buf, &format!("presenca da sala {sala}"), |b| {
        presencas(b).into_iter().find_map(|p| {
            let de = atributo(p, "from")?.to_ascii_lowercase();
            if let Some(motivo) = erro_da_presenca(p) {
                // O erro vem de `sala/apelido`; o de outra sala nao decide esta.
                return (de == minha).then(|| {
                    Err(format!(
                        "xmpp: nao entrou na sala {sala} como {apelido}: {motivo}"
                    ))
                });
            }
            // A sala confirma refletindo a nossa presenca (`sala/apelido`); o `status 110` cobre
            // o servico que trocou o nick (XEP-0045 §7.2.9, status 210).
            (de == minha || p.contains("code='110'") || p.contains("code=\"110\""))
                .then_some(Ok(()))
        })
    })?
}

/// A propria fala de volta: no `chat`, o que sai do proprio JID (outro recurso); na sala, o
/// que a sala reflete com o nosso nick (XEP-0045 §7.4 manda refletir a todos, inclusive a quem
/// falou).
pub fn eco(r: &Recebida, jid: &str, apelido: &str) -> bool {
    if r.sala {
        r.autor.eq_ignore_ascii_case(apelido)
    } else {
        r.conversa.eq_ignore_ascii_case(jid)
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

    fn apelido(&self) -> String {
        if self.cfg.apelido.trim().is_empty() {
            self.cfg
                .jid
                .split('@')
                .next()
                .unwrap_or("phxclaw")
                .to_string()
        } else {
            self.cfg.apelido.trim().to_string()
        }
    }

    fn e_sala(&self, conversa: &str) -> bool {
        self.cfg
            .salas
            .iter()
            .any(|s| s.eq_ignore_ascii_case(conversa))
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
        let apelido = self.apelido();
        for sala in &self.cfg.salas {
            entrar_na_sala(&mut s, &mut buf, sala, &apelido)?;
        }
        s.set_read_timeout(None).ok();
        let escrita = Arc::new(Mutex::new(s));
        let (tx, rx) = channel();
        // Fraco, como no IRC: a leitura em fundo nao pode manter o fio vivo sozinha.
        let pong = Arc::downgrade(&escrita);
        let mut despachar = move |bytes: &[u8]| {
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
        };
        // O que sobrou da entrada nas salas (mensagens que a sala ja mandou) nao pode esperar
        // a proxima leitura: num fluxo parado, ela nunca vem.
        despachar(b"");
        ler_em_fundo(escrita.clone(), despachar);
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
        let apelido = self.apelido();
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
                        if let Some(r) = mensagem_da_estrofe(&e)
                            && !eco(&r, &eu, &apelido)
                        {
                            let (conversa, autor) = if !r.sala && self.e_sala(&r.conversa) {
                                // Privada de ocupante: a conversa e `sala/nick`, senao a
                                // resposta sairia `groupchat` para a sala inteira.
                                (r.de.clone(), r.autor.clone())
                            } else {
                                (r.conversa, r.autor)
                            };
                            novas.push((
                                Mensagem {
                                    conversa,
                                    autor,
                                    id: r.id,
                                    texto: Some(r.texto),
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
        // A sala recebe `groupchat` no JID sem recurso (XEP-0045 §7.4); o resto e `chat`.
        let tipo = if self.e_sala(conversa) {
            "groupchat"
        } else {
            "chat"
        };
        self.com_conexao(|c| {
            escrever(
                &c.escrita,
                &format!(
                    "<message to='{}' type='{tipo}' id='{id}'><body>{}</body></message>",
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
        let r = mensagem_da_estrofe(e).unwrap();
        assert_eq!(
            (
                r.conversa.as_str(),
                r.autor.as_str(),
                r.id.as_str(),
                r.texto.as_str(),
                r.sala
            ),
            ("ana@x.org", "ana@x.org", "m1", "oi & \u{e9}", false)
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

    /// Prova real: tirar `r.autor.eq_ignore_ascii_case(apelido)` do `eco` (devolver `false` na
    /// sala) reprova o eco; tirar o `urn:xmpp:delay` do `mensagem_da_estrofe` reprova o
    /// historico; tirar `recurso.is_empty()` reprova o aviso da sala.
    #[test]
    fn groupchat_vira_entrada_com_nick_e_eco_historico_e_aviso_nao() {
        let e = "<message from='sala@conf.x.org/ana' type='groupchat' id='g1'><body>oi</body></message>";
        let r = mensagem_da_estrofe(e).unwrap();
        assert_eq!(
            (
                r.conversa.as_str(),
                r.autor.as_str(),
                r.id.as_str(),
                r.texto.as_str(),
                r.sala
            ),
            ("sala@conf.x.org", "ana", "g1", "oi", true)
        );
        assert!(!eco(&r, "agente@x.org", "claw"));
        let proprio = mensagem_da_estrofe(
            "<message from='sala@conf.x.org/Claw' type='groupchat'><body>eco</body></message>",
        )
        .unwrap();
        assert!(
            eco(&proprio, "agente@x.org", "claw"),
            "o nick refletido e o nosso"
        );
        assert_eq!(
            mensagem_da_estrofe(
                "<message from='sala@conf.x.org/ana' type='groupchat'><body>antes</body><delay xmlns='urn:xmpp:delay' stamp='2026-01-01T00:00:00Z'/></message>"
            ),
            None,
            "historico da sala"
        );
        assert_eq!(
            mensagem_da_estrofe(
                "<message from='sala@conf.x.org' type='groupchat'><subject>tema</subject></message>"
            ),
            None,
            "fala da propria sala"
        );
        // Privada de ocupante: `chat`, e o `de` guarda o nick para a resposta nao ir a sala.
        let priv_ = mensagem_da_estrofe(
            "<message from='sala@conf.x.org/ana' type='chat'><body>psiu</body></message>",
        )
        .unwrap();
        assert_eq!(
            (priv_.sala, priv_.de.as_str()),
            (false, "sala@conf.x.org/ana")
        );
    }

    /// Prova real: tirar o `<conflict` da tabela faz o motivo cair em `erro 409`, e o teste
    /// pede o texto legivel.
    #[test]
    fn presenca_de_erro_409_vira_motivo_legivel_e_a_de_entrada_nao() {
        let e = "<presence from='sala@conf.x.org/claw' to='agente@x.org/phxclaw' type='error'><x xmlns='http://jabber.org/protocol/muc'/><error by='sala@conf.x.org' type='cancel' code='409'><conflict xmlns='urn:ietf:params:xml:ns:xmpp-stanzas'/></error></presence>";
        assert_eq!(
            erro_da_presenca(e).as_deref(),
            Some("o apelido ja esta em uso na sala")
        );
        assert_eq!(
            erro_da_presenca("<presence from='s@c/x' type='error'><error code='500'/></presence>")
                .as_deref(),
            Some("erro 500")
        );
        let ok = "<presence from='sala@conf.x.org/claw'><x xmlns='http://jabber.org/protocol/muc#user'><item affiliation='none' role='participant'/><status code='110'/></x></presence>";
        assert_eq!(erro_da_presenca(ok), None);
        // O recorte de presencas nao tira a mensagem que veio junto.
        let buf = format!(
            "{ok}<message from='sala@conf.x.org/ana' type='groupchat'><body>1</body></message><presence from='sala@conf.x.org/bia'/><presence from='s@c/"
        );
        let ps = presencas(&buf);
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0], ok);
    }
}
