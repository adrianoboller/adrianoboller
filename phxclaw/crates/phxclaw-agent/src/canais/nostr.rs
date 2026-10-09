//! Nostr: quem fala com o agente o faz por mensagem direta cifrada (NIP-17: rumor kind 14,
//! selado em kind 13 e embrulhado em kind 1059, NIP-59, os dois cifrados pelo NIP-44 v2)
//! ou por nota publica que menciona a chave do bot (kind 1 com a tag `p`, NIP-01).
//!
//! O que faz a lista de permitidos valer aqui e a assinatura: a conversa e a chave publica
//! de quem escreveu, e qualquer um pode pôr qualquer chave num evento. Nota: o id e
//! recalculado e a assinatura Schnorr conferida (`bip340`). Mensagem direta: a assinatura
//! do embrulho e a do selo sao conferidas, e a chave do rumor TEM de ser a do selo -- e so
//! o selo que o remetente assina; sem essa igualdade, qualquer um se passaria por qualquer
//! um so trocando o `pubkey` do rumor (o NIP-17 manda conferir).
//!
//! A resposta volta pelo caminho por onde a pessoa falou, e o caminho privado e pegajoso:
//! quem ja mandou DM nunca mais recebe resposta em nota publica neste processo, e conversa
//! sem historico (a `channel_send` do agente) sai cifrada. Errar para o privado custa uma
//! notificacao num lugar diferente; errar para o publico publica o que era segredo.
//!
//! A DM sai para os relays que o destinatario lista no kind 10050 dele (NIP-17: SO para
//! eles, ate tres); sem lista, para o relay do canal. O bot anuncia o proprio 10050 com o
//! relay do canal, senao cliente nenhum sabe onde entregar. Relay que pede NIP-42 recebe o
//! AUTH assinado pela chave do bot.
//!
//! Cursor: o `created_at` (do rumor, na DM) do ultimo tratado; a consulta pede `since`
//! nele, e o embrulho dois dias antes, porque o NIP-17 manda sortear o `created_at` do
//! embrulho ate dois dias para tras. O mesmo segundo volta na consulta seguinte e o
//! gateway descarta o id repetido -- repetir e o lado certo de errar; perder nao. E o
//! cursor nunca passa do relogio: um `created_at` no futuro (de qualquer chave, inclusive
//! de fora da lista) empurraria o cursor e faria o canal pular tudo ate la.
//!
//! A chave secreta do bot so existe dentro de `Credencial::com`: assinar, abrir e fechar
//! embrulho acontecem ali, e a chave de conversa morre no fim da chamada.

use super::bip340;
use super::http::Credencial;
use super::nip44;
use super::{Entrada, Mensagem, Provedor, Unidade};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message as Ws;

pub const KIND_NOTA: i64 = 1;
pub const KIND_SELO: i64 = 13;
pub const KIND_CHAT: i64 = 14;
pub const KIND_ARQUIVO: i64 = 15;
pub const KIND_EMBRULHO: i64 = 1059;
pub const KIND_CAIXA_DM: i64 = 10050;
pub const KIND_AUTH: i64 = 22242;
/// O NIP-17 sorteia o `created_at` do selo e do embrulho em ate dois dias para tras.
pub const DOIS_DIAS: i64 = 2 * 24 * 3600;
/// Relays da caixa de entrada do destinatario que se usam: o NIP-17 pede listas de 1 a 3,
/// e o teto impede uma lista de cem relays de virar cem conexoes por resposta.
const TETO_CAIXA: usize = 3;
/// Embrulhos lembrados (id -> `created_at` do rumor). Passou disso, esquece e reabre.
const TETO_ABERTOS: usize = 10_000;

type Soquete =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Por onde a conversa falou. Ver o topo: `Direta` e pegajosa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Nota,
    Direta,
}

pub struct Nostr {
    relay: String,
    segredo: Credencial,
    /// Chave publica do bot (hex), calculada da secreta na montagem.
    eu: String,
    vias: Mutex<HashMap<String, Via>>,
    /// Embrulho ja aberto: id -> `created_at` do rumor, ou `None` se nao abriu. Cada abertura
    /// sao dois ECDH e duas conferencias de assinatura, e a janela de dois dias traz os
    /// mesmos embrulhos a cada volta; o lixo de quem inunda a caixa so se paga uma vez.
    abertos: Mutex<HashMap<String, Option<i64>>>,
    caixa_anunciada: AtomicBool,
}

/// O id do evento: sha256 da serializacao canonica `[0, pubkey, created_at, kind, tags,
/// content]` (NIP-01). O `serde_json` escapa como a NIP pede (sem espaco, UTF-8 cru).
pub fn id_do_evento(ev: &Value) -> String {
    use sha2::{Digest, Sha256};
    let ser = json!([
        0,
        ev["pubkey"],
        ev["created_at"],
        ev["kind"],
        ev["tags"],
        ev["content"]
    ]);
    bip340::hex(&Sha256::digest(ser.to_string().as_bytes()))
}

/// Id recalculado bate e a assinatura confere pela chave do proprio evento.
pub fn evento_valido(ev: &Value) -> bool {
    let (Some(id), Some(pk), Some(sig)) =
        (ev["id"].as_str(), ev["pubkey"].as_str(), ev["sig"].as_str())
    else {
        return false;
    };
    if id_do_evento(ev) != id {
        return false;
    }
    match (
        bip340::de_hex::<32>(id),
        bip340::de_hex::<32>(pk),
        bip340::de_hex::<64>(sig),
    ) {
        (Ok(m), Ok(p), Ok(s)) => bip340::conferir(&p, &m, &s),
        _ => false,
    }
}

fn aleatorio<const T: usize>() -> Result<[u8; T], String> {
    let mut b = [0u8; T];
    getrandom::fill(&mut b).map_err(|e| format!("CSPRNG do sistema indisponivel: {e}"))?;
    Ok(b)
}

/// Um instante sorteado em ate dois dias antes de `agora` (selo e embrulho).
fn instante_sorteado(agora: i64) -> Result<i64, String> {
    let r = u64::from_le_bytes(aleatorio::<8>()?);
    Ok(agora - (r % DOIS_DIAS as u64) as i64)
}

/// Poe `pubkey`, `id` e `sig` num evento (kind, tags, content, created_at ja nele).
pub fn assinar_evento(segredo: &[u8; 32], mut ev: Value) -> Result<Value, String> {
    ev["pubkey"] = json!(bip340::hex(&bip340::chave_publica(segredo)?));
    let id = id_do_evento(&ev);
    // aux do BIP-340: nao precisa ser secreto, so diferente a cada assinatura.
    let sig = bip340::assinar(segredo, &bip340::de_hex(&id)?, &aleatorio::<32>()?)?;
    ev["id"] = json!(id);
    ev["sig"] = json!(bip340::hex(&sig));
    Ok(ev)
}

/// Monta e assina um evento kind 1.
pub fn nota(
    segredo: &[u8; 32],
    conteudo: &str,
    tags: Value,
    criado_em: i64,
) -> Result<Value, String> {
    assinar_evento(
        segredo,
        json!({"created_at": criado_em, "kind": KIND_NOTA, "tags": tags, "content": conteudo}),
    )
}

/// O rumor kind 14 (sem assinatura, NIP-59) de `segredo` para `para`.
pub fn rumor_de_chat(
    segredo: &[u8; 32],
    para: &str,
    texto: &str,
    criado_em: i64,
) -> Result<Value, String> {
    let mut r = json!({
        "pubkey": bip340::hex(&bip340::chave_publica(segredo)?),
        "created_at": criado_em,
        "kind": KIND_CHAT,
        "tags": [["p", para]],
        "content": texto,
    });
    r["id"] = json!(id_do_evento(&r));
    Ok(r)
}

fn chave_de(pk: &str) -> Result<[u8; 32], String> {
    bip340::de_hex::<32>(pk).map_err(|e| format!("chave publica: {e}"))
}

/// Sela o rumor com a chave do remetente e embrulha o selo com uma chave efemera, para
/// `destino` (NIP-59). A chave efemera nasce aqui do CSPRNG e morre no fim: ninguem a
/// guarda, e e ela que impede de ligar o embrulho ao remetente.
pub fn embrulhar(
    segredo: &[u8; 32],
    rumor: &Value,
    destino: &str,
    agora: i64,
) -> Result<Value, String> {
    let pk_destino = chave_de(destino)?;
    let conv = nip44::chave_de_conversa(segredo, &pk_destino)?;
    let selo = assinar_evento(
        segredo,
        json!({
            "created_at": instante_sorteado(agora)?,
            "kind": KIND_SELO,
            "tags": [],
            "content": nip44::cifrar_aleatorio(&rumor.to_string(), &conv)?,
        }),
    )?;
    let efemera = loop {
        let k = aleatorio::<32>()?;
        if bip340::chave_publica(&k).is_ok() {
            break k;
        }
    };
    let conv = nip44::chave_de_conversa(&efemera, &pk_destino)?;
    assinar_evento(
        &efemera,
        json!({
            "created_at": instante_sorteado(agora)?,
            "kind": KIND_EMBRULHO,
            "tags": [["p", destino]],
            "content": nip44::cifrar_aleatorio(&selo.to_string(), &conv)?,
        }),
    )
}

/// Abre um embrulho endereçado a `segredo` e devolve o rumor, com o id RECALCULADO.
/// Recusa: embrulho ou selo com assinatura que nao confere, kind errado, selo com tags,
/// carga NIP-44 recusada (versao, MAC, padding, tamanho) e rumor cuja chave nao e a de
/// quem assinou o selo.
pub fn desembrulhar(segredo: &[u8; 32], embrulho: &Value) -> Result<Value, String> {
    if embrulho["kind"].as_i64() != Some(KIND_EMBRULHO) {
        return Err("nao e embrulho (kind 1059)".into());
    }
    // O NIP-44 manda conferir a assinatura de fora ANTES de decifrar.
    if !evento_valido(embrulho) {
        return Err("embrulho com assinatura que nao confere".into());
    }
    let abrir = |ev: &Value| -> Result<Value, String> {
        let conv =
            nip44::chave_de_conversa(segredo, &chave_de(ev["pubkey"].as_str().unwrap_or(""))?)?;
        let claro = nip44::decifrar(ev["content"].as_str().unwrap_or(""), &conv)?;
        serde_json::from_str(&claro).map_err(|e| format!("conteudo nao e evento: {e}"))
    };
    let selo = abrir(embrulho)?;
    if selo["kind"].as_i64() != Some(KIND_SELO) {
        return Err("o embrulho nao traz um selo (kind 13)".into());
    }
    if selo["tags"] != json!([]) {
        return Err("selo com tags (o NIP-59 manda vazio)".into());
    }
    if !evento_valido(&selo) {
        return Err("selo com assinatura que nao confere".into());
    }
    let mut rumor = abrir(&selo)?;
    if rumor["pubkey"] != selo["pubkey"] {
        return Err("rumor com chave diferente da do selo (remetente forjado)".into());
    }
    if !rumor["content"].is_string() || rumor["created_at"].as_i64().is_none() {
        return Err("rumor malformado".into());
    }
    let id = id_do_evento(&rumor);
    if rumor
        .get("id")
        .is_some_and(|i| i.as_str() != Some(id.as_str()))
    {
        return Err("rumor com id que nao bate".into());
    }
    rumor["id"] = json!(id);
    Ok(rumor)
}

/// `wss://` sempre; `ws://` so em loopback, pela mesma regra do HTTP sem TLS.
pub fn conferir_relay(url: &str) -> Result<(), String> {
    let u = reqwest::Url::parse(url).map_err(|e| format!("{url}: {e}"))?;
    let loopback = matches!(
        u.host_str(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    );
    match u.scheme() {
        "wss" => Ok(()),
        "ws" if loopback => Ok(()),
        _ => Err(format!("{url}: relay sem TLS so em loopback")),
    }
}

async fn conectar(relay: &str) -> Result<Soquete, String> {
    conferir_relay(relay)?;
    let (ws, _) = tokio_tungstenite::connect_async(relay)
        .await
        .map_err(|e| format!("relay {relay}: {e}"))?;
    Ok(ws)
}

async fn mandar(ws: &mut Soquete, v: &Value) -> Result<(), String> {
    ws.send(Ws::text(v.to_string()))
        .await
        .map_err(|e| format!("relay: {e}"))
}

/// A proxima mensagem de texto do relay, ja como JSON; `None` no prazo ou no fim.
async fn proxima(ws: &mut Soquete, fim: tokio::time::Instant) -> Option<Result<Value, String>> {
    loop {
        let Ok(m) = tokio::time::timeout_at(fim, ws.next()).await else {
            return None;
        };
        match m {
            None => return None,
            Some(Err(e)) => return Some(Err(format!("relay: {e}"))),
            Some(Ok(Ws::Text(t))) => {
                return Some(Ok(serde_json::from_str(t.as_str()).unwrap_or(Value::Null)));
            }
            Some(Ok(_)) => continue,
        }
    }
}

fn recusa_por_auth(motivo: &Value) -> bool {
    motivo
        .as_str()
        .is_some_and(|m| m.starts_with("auth-required"))
}

impl Nostr {
    pub fn novo(relay: String, segredo: Credencial) -> Result<Self, String> {
        conferir_relay(&relay)?;
        let eu = segredo.com("probe", |s| {
            Ok(bip340::hex(&bip340::chave_publica(&bip340::de_hex(s)?)?))
        })?;
        Ok(Self {
            relay,
            segredo,
            eu,
            vias: Mutex::default(),
            abertos: Mutex::default(),
            caixa_anunciada: AtomicBool::new(false),
        })
    }

    pub fn chave_publica(&self) -> &str {
        &self.eu
    }

    /// Por onde a resposta a `conversa` sairia agora.
    pub fn via(&self, conversa: &str) -> Via {
        self.vias
            .lock()
            .ok()
            .and_then(|v| v.get(conversa).copied())
            .unwrap_or(Via::Direta)
    }

    fn bloquear<T>(
        &self,
        f: impl std::future::Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        tokio::runtime::Handle::current().block_on(f)
    }

    /// Com a chave secreta do bot, so pelo broker.
    fn com_chave<T>(
        &self,
        uso: &str,
        f: impl FnOnce(&[u8; 32]) -> Result<T, String>,
    ) -> Result<T, String> {
        self.segredo.com(uso, |s| f(&bip340::de_hex(s)?))
    }

    /// O evento kind 22242 do NIP-42 para o desafio de `relay`.
    fn evento_auth(&self, relay: &str, desafio: &str) -> Result<Value, String> {
        let agora = chrono::Utc::now().timestamp();
        self.com_chave("verify", |s| {
            assinar_evento(
                s,
                json!({"created_at": agora, "kind": KIND_AUTH, "content": "",
                    "tags": [["relay", relay], ["challenge", desafio]]}),
            )
        })
    }

    /// REQ com `filtros` e os EVENT que vierem ate o EOSE. Com `ao_vivo`, se nada veio
    /// ate o EOSE, espera o que chegar ate o prazo. Relay que pede AUTH recebe o evento
    /// assinado e o REQ uma segunda vez (uma so).
    async fn consultar(
        &self,
        filtros: &[Value],
        espera: Duration,
        ao_vivo: bool,
    ) -> Result<Vec<Value>, String> {
        let mut ws = conectar(&self.relay).await?;
        let sub = "phxclaw";
        let mut req = vec![json!("REQ"), json!(sub)];
        req.extend(filtros.iter().cloned());
        let req = Value::Array(req);
        mandar(&mut ws, &req).await?;
        let (mut autenticou, mut pendente, mut reenviou) = (false, false, false);
        let mut eventos = Vec::new();
        let mut eose = false;
        let fim = tokio::time::Instant::now() + espera;
        loop {
            // Depois do EOSE, o que ja veio basta; sem nada, espera o ao vivo ate o prazo.
            if eose && (!eventos.is_empty() || !ao_vivo) {
                break;
            }
            let Some(v) = proxima(&mut ws, fim).await else {
                break;
            };
            let v = v?;
            match v[0].as_str() {
                Some("EVENT") if v[1] == sub => eventos.push(v[2].clone()),
                Some("EOSE") if v[1] == sub => eose = true,
                Some("AUTH") => {
                    if let Some(d) = v[1].as_str() {
                        let ev = self.evento_auth(&self.relay, d)?;
                        mandar(&mut ws, &json!(["AUTH", ev])).await?;
                        autenticou = true;
                        if pendente && !reenviou {
                            mandar(&mut ws, &req).await?;
                            reenviou = true;
                        }
                    }
                }
                Some("CLOSED") if v[1] == sub => {
                    if recusa_por_auth(&v[2]) && !reenviou {
                        if autenticou {
                            mandar(&mut ws, &req).await?;
                            reenviou = true;
                        } else {
                            pendente = true;
                        }
                    } else {
                        break;
                    }
                }
                _ => {}
            }
        }
        let _ = mandar(&mut ws, &json!(["CLOSE", sub])).await;
        let _ = ws.close(None).await;
        Ok(eventos)
    }

    /// Publica `ev` em `relay` e espera o OK. AUTH pedido: assina, e reenvia uma vez.
    async fn publicar(&self, relay: &str, ev: &Value) -> Result<String, String> {
        let id = ev["id"].as_str().unwrap_or("").to_string();
        let mut ws = conectar(relay).await?;
        let msg = json!(["EVENT", ev]);
        mandar(&mut ws, &msg).await?;
        let (mut autenticou, mut pendente, mut reenviou) = (false, false, false);
        let fim = tokio::time::Instant::now() + Duration::from_secs(10);
        let r = loop {
            let Some(v) = proxima(&mut ws, fim).await else {
                break Err(format!("relay {relay} nao confirmou o evento (OK)"));
            };
            let v = v?;
            match v[0].as_str() {
                Some("AUTH") => {
                    if let Some(d) = v[1].as_str() {
                        let a = self.evento_auth(relay, d)?;
                        mandar(&mut ws, &json!(["AUTH", a])).await?;
                        autenticou = true;
                        if pendente && !reenviou {
                            mandar(&mut ws, &msg).await?;
                            reenviou = true;
                        }
                    }
                }
                Some("OK") if v[1] == id.as_str() => {
                    if v[2] == true {
                        break Ok(id.clone());
                    }
                    if recusa_por_auth(&v[3]) && !reenviou {
                        if autenticou {
                            mandar(&mut ws, &msg).await?;
                            reenviou = true;
                        } else {
                            pendente = true;
                        }
                        continue;
                    }
                    break Err(format!("relay {relay} recusou: {}", v[3]));
                }
                _ => {}
            }
        };
        let _ = ws.close(None).await;
        r
    }

    /// O kind 10050 do bot, uma vez por processo: sem ele, cliente que segue o NIP-17 nao
    /// sabe onde entregar a DM. Falhou, tenta na proxima volta.
    fn anunciar_caixa(&self) {
        if self.caixa_anunciada.load(Ordering::Relaxed) {
            return;
        }
        let agora = chrono::Utc::now().timestamp();
        let feito = self
            .com_chave("send", |s| {
                assinar_evento(
                    s,
                    json!({"created_at": agora, "kind": KIND_CAIXA_DM, "content": "",
                        "tags": [["relay", self.relay]]}),
                )
            })
            .and_then(|ev| self.bloquear(self.publicar(&self.relay, &ev)));
        if feito.is_ok() {
            self.caixa_anunciada.store(true, Ordering::Relaxed);
        }
    }

    /// Os relays da caixa de entrada de `pk` (o 10050 mais novo, com assinatura que
    /// confere), ate tres e pela regra do `conferir_relay`; sem lista, o relay do canal.
    pub fn caixa_de(&self, pk: &str) -> Result<Vec<String>, String> {
        let eventos = self.bloquear(self.consultar(
            &[json!({"kinds": [KIND_CAIXA_DM], "authors": [pk], "limit": 1})],
            Duration::from_secs(5),
            false,
        ))?;
        let lista = eventos
            .into_iter()
            .filter(|e| {
                e["kind"].as_i64() == Some(KIND_CAIXA_DM) && e["pubkey"] == pk && evento_valido(e)
            })
            .max_by_key(|e| e["created_at"].as_i64().unwrap_or(0));
        let mut relays: Vec<String> = Vec::new();
        for t in lista
            .as_ref()
            .and_then(|e| e["tags"].as_array())
            .into_iter()
            .flatten()
        {
            if t[0] == "relay"
                && let Some(r) = t[1].as_str()
                && conferir_relay(r).is_ok()
                && !relays.iter().any(|x| x == r)
                && relays.len() < TETO_CAIXA
            {
                relays.push(r.to_string());
            }
        }
        if relays.is_empty() {
            relays.push(self.relay.clone());
        }
        Ok(relays)
    }

    fn lembrar_via(&self, conversa: &str, via: Via) {
        if let Ok(mut v) = self.vias.lock() {
            match via {
                Via::Direta => {
                    v.insert(conversa.to_string(), Via::Direta);
                }
                Via::Nota => {
                    v.entry(conversa.to_string()).or_insert(Via::Nota);
                }
            }
        }
    }

    /// Abre os embrulhos que ainda nao foram abertos (ou cujo rumor ainda pode valer),
    /// todos numa so concessao da chave. Devolve (rumor) dos que abriram e valem.
    fn abrir_embrulhos(&self, embrulhos: &[Value], desde: i64) -> Result<Vec<Value>, String> {
        let pular = |id: &str| -> bool {
            self.abertos
                .lock()
                .ok()
                .and_then(|a| a.get(id).copied())
                .is_some_and(|t| t.is_none_or(|t| t < desde))
        };
        let novos: Vec<&Value> = embrulhos
            .iter()
            .filter(|e| !pular(e["id"].as_str().unwrap_or("")))
            .collect();
        if novos.is_empty() {
            return Ok(vec![]);
        }
        let abertos: Vec<(String, Option<Value>)> = self.com_chave("receive", |s| {
            Ok(novos
                .iter()
                .map(|e| {
                    let id = e["id"].as_str().unwrap_or("").to_string();
                    (id, desembrulhar(s, e).ok())
                })
                .collect())
        })?;
        let mut lembrar = self.abertos.lock().map_err(|e| e.to_string())?;
        if lembrar.len() > TETO_ABERTOS {
            lembrar.clear();
        }
        let mut rumores = Vec::new();
        for (id, r) in abertos {
            let t = r.as_ref().and_then(|r| r["created_at"].as_i64());
            lembrar.insert(id, t);
            if let Some(r) = r
                && t.is_some_and(|t| t >= desde)
            {
                rumores.push(r);
            }
        }
        Ok(rumores)
    }

    /// Manda `texto` cifrado a `conversa` (NIP-17): o embrulho dela vai aos relays da
    /// caixa dela, e a copia do bot (o NIP-17 manda embrulhar tambem ao remetente) ao relay
    /// do canal. Devolve o id do rumor, o mesmo nas duas copias.
    fn enviar_direta(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let destinos = self.caixa_de(conversa)?;
        let agora = chrono::Utc::now().timestamp();
        let (para_ela, para_mim, id) = self.com_chave("send", |s| {
            let r = rumor_de_chat(s, conversa, texto, agora)?;
            let id = r["id"].as_str().unwrap_or("").to_string();
            Ok((
                embrulhar(s, &r, conversa, agora)?,
                embrulhar(s, &r, &self.eu, agora)?,
                id,
            ))
        })?;
        let mut erros = Vec::new();
        let mut entregue = false;
        for d in &destinos {
            match self.bloquear(self.publicar(d, &para_ela)) {
                Ok(_) => entregue = true,
                Err(e) => erros.push(e),
            }
        }
        if !entregue {
            return Err(erros.join("; "));
        }
        // A copia e para o historico de outro cliente do bot; perder nao perde a resposta.
        let _ = self.bloquear(self.publicar(&self.relay, &para_mim));
        Ok(id)
    }
}

impl Provedor for Nostr {
    fn nome(&self) -> &str {
        "nostr"
    }

    fn limite(&self) -> (usize, Unidade) {
        (2000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let agora = chrono::Utc::now().timestamp();
        let Some(desde) = cursor.and_then(|c| c.parse::<i64>().ok()) else {
            // Primeira vez: comeca do agora.
            return Ok(vec![Entrada {
                cursor: Some(agora.to_string()),
                mensagem: None,
            }]);
        };
        self.anunciar_caixa();
        let eu = self.eu.clone();
        let eventos = self.bloquear(self.consultar(
            &[
                json!({"kinds": [KIND_NOTA], "#p": [eu], "since": desde, "limit": 200}),
                json!({"kinds": [KIND_EMBRULHO], "#p": [eu], "since": desde - DOIS_DIAS,
                    "limit": 200}),
            ],
            Duration::from_secs(espera_seg.max(1)),
            true,
        ))?;
        let (notas, embrulhos): (Vec<Value>, Vec<Value>) = eventos
            .into_iter()
            .filter(|e| matches!(e["kind"].as_i64(), Some(KIND_NOTA | KIND_EMBRULHO)))
            .partition(|e| e["kind"].as_i64() == Some(KIND_NOTA));
        let mut itens: Vec<(i64, Option<Mensagem>, Via)> = Vec::new();
        for e in notas.into_iter().filter(evento_valido) {
            let de = e["pubkey"].as_str().unwrap_or("").to_string();
            let t = e["created_at"].as_i64().unwrap_or(desde);
            itens.push((
                t,
                (de != self.eu).then(|| Mensagem {
                    conversa: de.clone(),
                    autor: de,
                    id: e["id"].as_str().unwrap_or("").to_string(),
                    texto: e["content"].as_str().map(str::to_string),
                }),
                Via::Nota,
            ));
        }
        for r in self.abrir_embrulhos(&embrulhos, desde)? {
            let de = r["pubkey"].as_str().unwrap_or("").to_string();
            let t = r["created_at"].as_i64().unwrap_or(desde);
            // 14 e texto; 15 e arquivo (a mensagem existe, o agente nao a le); reacao e o
            // resto so andam o cursor.
            let texto = match r["kind"].as_i64() {
                Some(KIND_CHAT) => Some(r["content"].as_str().map(str::to_string)),
                Some(KIND_ARQUIVO) => Some(None),
                _ => None,
            };
            itens.push((
                t,
                texto.filter(|_| de != self.eu).map(|texto| Mensagem {
                    conversa: de.clone(),
                    autor: de,
                    id: r["id"].as_str().unwrap_or("").to_string(),
                    texto,
                }),
                Via::Direta,
            ));
        }
        itens.sort_by_key(|(t, _, _)| *t);
        let mut lote: Vec<Entrada> = itens
            .into_iter()
            .map(|(t, m, via)| {
                if let Some(m) = &m {
                    self.lembrar_via(&m.conversa, via);
                }
                Entrada {
                    cursor: Some(t.min(agora).max(desde).to_string()),
                    mensagem: m,
                }
            })
            .collect();
        if lote.is_empty() {
            lote.push(Entrada {
                cursor: Some(desde.to_string()),
                mensagem: None,
            });
        }
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        bip340::de_hex::<32>(conversa).map_err(|e| format!("conversa nao e chave: {e}"))?;
        if self.via(conversa) == Via::Direta {
            return self.enviar_direta(conversa, texto);
        }
        let ev = self.com_chave("send", |s| {
            nota(
                s,
                texto,
                json!([["p", conversa]]),
                chrono::Utc::now().timestamp(),
            )
        })?;
        self.bloquear(self.publicar(&self.relay, &ev))
    }
}
