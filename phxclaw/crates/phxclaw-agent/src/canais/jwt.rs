//! O JWT RS256 que o Bot Framework (Teams) e o Google Chat poem no `Authorization` do
//! webhook: assinatura pela chave do JWKS do servico (`rsa.rs`), emissor, audiencia e
//! validade. O que e de cada servico (o `serviceUrl` e os endossos do Teams, o `email` do
//! ID token do Google) fica no canal; o que e de todo JWT RS256 fica aqui, uma vez so.
//!
//! Decisoes, e de onde vieram:
//! - `alg` tem de ser `RS256`, e so: aceitar o `alg` que o token diz e a porta do `none` e
//!   do HS256 assinado com a chave publica como segredo. Nao ha interruptor para desligar a
//!   conferencia -- a Microsoft escreve «implementers shouldn't expose a way to disable
//!   validation», e a casa nao tem motivo para discordar;
//! - tolerancia de relogio de 5 min nos dois lados (`exp` e `nbf`), o numero da Microsoft;
//! - o JWKS se recarrega a cada 24 h (o «ao menos a cada 24 h» da Microsoft) e na hora em
//!   que chega um `kid` desconhecido -- e a troca de chave do servico --, mas no maximo uma
//!   vez por minuto: sem esse teto, cada pedido com `kid` inventado viraria um pedido nosso
//!   ao servico, e o agente seria o amplificador de quem bate na porta;
//! - JWKS vencido que nao recarrega e recusa (503), e nao a chave velha: falha fechada.
//! - a recusa sai sem detalhe (`Motivo::recusa`): quem forja um token nao aprende qual
//!   pedaco errou. O `Motivo` existe para o teste provar que recusou PELO motivo certo.

use super::caixa::{PedidoWebhook, Recusa};
use super::http::{Http, politica_para};
use super::rsa::ChavePublica;
use base64::Engine;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const TOLERANCIA_SEG: i64 = 300;
pub const RECARGA: Duration = Duration::from_secs(24 * 3600);
pub const INTERVALO_MINIMO: Duration = Duration::from_secs(60);
/// O JWKS do Bot Framework tinha 225 chaves com `x5c` (medido pelo pesquisador); 4 MiB e
/// folga de varias vezes isso, e o teto vem antes da leitura.
const TETO_JWKS: usize = 4 << 20;
/// Token de verdade tem 1 a 2 KB; o teto impede decodificar meio megabyte de cabecalho.
const TETO_TOKEN: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    SemToken,
    Formato,
    Algoritmo,
    SemKid,
    KidDesconhecido,
    ChavesIndisponiveis,
    Assinatura,
    Emissor,
    Audiencia,
    SemValidade,
    Vencido,
    AindaNaoVale,
    /// Token bom, mas que nao vale para ESTE pedido (o `serviceUrl` ou o endosso do Teams,
    /// o `email` do ID token do Google).
    NaoVaiAqui,
}

impl Motivo {
    /// O que vai para quem bateu na porta: 401 sem dizer o que errou; 403 quando o token e
    /// bom e o pedido nao (a Microsoft pede 403 para o endosso); 503 quando somos nos que
    /// nao temos a chave.
    pub fn recusa(self) -> Recusa {
        match self {
            Motivo::ChavesIndisponiveis => (503, "chaves de assinatura indisponiveis".into()),
            Motivo::NaoVaiAqui => (403, "token nao vale para este pedido".into()),
            _ => (401, "token nao confere".into()),
        }
    }
}

/// Uma chave do JWKS e os canais que a endossam (so o Bot Framework publica endosso).
#[derive(Debug)]
pub struct ChaveJwks {
    pub chave: ChavePublica,
    pub endossos: Vec<String>,
}

type Chaves = BTreeMap<String, Arc<ChaveJwks>>;

/// As chaves que interessam de um JWKS: RSA, de assinatura, que passam pela politica do
/// `rsa.rs`. Chave fora disso e pulada, nao fatal -- uma chave estranha no meio de 225 nao
/// pode derrubar as outras 224. JWKS sem nenhuma aproveitavel e erro.
pub fn chaves_do_jwks(v: &Value) -> Result<Chaves, String> {
    let lista = v["keys"].as_array().ok_or("JWKS sem `keys`")?;
    let mut r = Chaves::new();
    for k in lista {
        let texto = |c: &str| k[c].as_str();
        if texto("kty") != Some("RSA")
            || texto("use").is_some_and(|u| u != "sig")
            || texto("alg").is_some_and(|a| a != "RS256")
        {
            continue;
        }
        let (Some(kid), Some(n), Some(e)) = (texto("kid"), texto("n"), texto("e")) else {
            continue;
        };
        let Ok(chave) = ChavePublica::de_jwk(n, e) else {
            continue;
        };
        let endossos = k["endorsements"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        r.insert(kid.to_string(), Arc::new(ChaveJwks { chave, endossos }));
    }
    if r.is_empty() {
        return Err("JWKS sem chave RSA de assinatura aproveitavel".into());
    }
    Ok(r)
}

#[derive(Default)]
struct Cache {
    chaves: Chaves,
    lido: Option<Instant>,
    tentado: Option<Instant>,
}

/// O JWKS de um servico, com cache. A busca passa pela politica de destinos como todo
/// pedido dos canais (`Http`).
pub struct Jwks {
    http: Http,
    recarga: Duration,
    intervalo: Duration,
    cache: Mutex<Cache>,
}

impl Jwks {
    pub fn novo(url: &str) -> Result<Self, String> {
        Self::com_prazos(url, RECARGA, INTERVALO_MINIMO)
    }

    /// Os prazos explicitos existem para o teste provar a recarga sem esperar 24 h.
    pub fn com_prazos(url: &str, recarga: Duration, intervalo: Duration) -> Result<Self, String> {
        Ok(Self {
            http: Http::novo(url, politica_para(url)?)?,
            recarga,
            intervalo,
            cache: Mutex::new(Cache::default()),
        })
    }

    fn baixar(&self) -> Result<Chaves, String> {
        let corpo = self
            .http
            .binario(self.http.cliente()?.get(self.http.base()), TETO_JWKS)?;
        let v: Value =
            serde_json::from_slice(&corpo).map_err(|e| format!("JWKS nao e JSON: {e}"))?;
        chaves_do_jwks(&v)
    }

    /// A chave do `kid`, recarregando se o cache venceu ou se o `kid` e novo.
    pub fn chave(&self, kid: &str) -> Result<Arc<ChaveJwks>, Motivo> {
        let mut c = self.cache.lock().map_err(|_| Motivo::ChavesIndisponiveis)?;
        let agora = Instant::now();
        let vencido = c
            .lido
            .is_none_or(|t| agora.duration_since(t) >= self.recarga);
        if vencido || !c.chaves.contains_key(kid) {
            let pode = c
                .tentado
                .is_none_or(|t| agora.duration_since(t) >= self.intervalo);
            if pode {
                c.tentado = Some(agora);
                match self.baixar() {
                    Ok(m) => {
                        c.chaves = m;
                        c.lido = Some(agora);
                    }
                    Err(_) if vencido => {
                        c.chaves.clear();
                        c.lido = None;
                        return Err(Motivo::ChavesIndisponiveis);
                    }
                    Err(_) => {}
                }
            } else if vencido {
                return Err(Motivo::ChavesIndisponiveis);
            }
        }
        c.chaves.get(kid).cloned().ok_or(Motivo::KidDesconhecido)
    }
}

/// O que o servico tem de ter escrito no token.
pub struct Exigido<'a> {
    pub emissores: &'a [&'a str],
    pub audiencia: &'a str,
}

/// O token que passou: as declaracoes e os endossos da chave que o assinou.
#[derive(Debug)]
pub struct Conferido {
    pub claims: Value,
    pub endossos: Vec<String>,
}

/// Segundos desde a epoca, pelo relogio do sistema.
pub fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// O token do `Authorization: Bearer` (o esquema nao distingue caixa, RFC 7235 §2.1).
pub fn bearer(p: &PedidoWebhook) -> Option<&str> {
    let v = p.cabecalho("authorization")?.trim();
    let (esquema, token) = v.split_once(' ')?;
    esquema
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim())
        .filter(|t| !t.is_empty())
}

fn parte_json(b64: &str) -> Result<Value, Motivo> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64)
        .map_err(|_| Motivo::Formato)?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| Motivo::Formato)?;
    if v.is_object() {
        Ok(v)
    } else {
        Err(Motivo::Formato)
    }
}

fn numero(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
}

/// Confere um JWT RS256. A assinatura vem ANTES de qualquer leitura das declaracoes: nada
/// do corpo e levado em conta ate se saber quem o escreveu.
pub fn conferir(
    token: &str,
    jwks: &Jwks,
    exigido: &Exigido,
    agora: i64,
) -> Result<Conferido, Motivo> {
    if token.len() > TETO_TOKEN {
        return Err(Motivo::Formato);
    }
    let partes: Vec<&str> = token.split('.').collect();
    let [cab, corpo, ass] = partes[..] else {
        return Err(Motivo::Formato);
    };
    let cabecalho = parte_json(cab)?;
    if cabecalho["alg"] != "RS256" {
        return Err(Motivo::Algoritmo);
    }
    // `crit` pede que se entenda uma extensao (RFC 7515 §4.1.11); nao entendemos nenhuma.
    if !cabecalho["crit"].is_null() {
        return Err(Motivo::Formato);
    }
    let kid = cabecalho["kid"].as_str().ok_or(Motivo::SemKid)?;
    let assinatura = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(ass)
        .map_err(|_| Motivo::Formato)?;
    let chave = jwks.chave(kid)?;
    let assinado = &token[..cab.len() + 1 + corpo.len()];
    if !chave.chave.verificar(assinado.as_bytes(), &assinatura) {
        return Err(Motivo::Assinatura);
    }
    let claims = parte_json(corpo)?;
    let iss = claims["iss"].as_str().unwrap_or("");
    if !exigido.emissores.contains(&iss) {
        return Err(Motivo::Emissor);
    }
    let aud_ok = match &claims["aud"] {
        Value::String(a) => a == exigido.audiencia,
        Value::Array(l) => l.iter().any(|a| a == exigido.audiencia),
        _ => false,
    };
    if !aud_ok || exigido.audiencia.is_empty() {
        return Err(Motivo::Audiencia);
    }
    let exp = numero(&claims["exp"]).ok_or(Motivo::SemValidade)?;
    if agora > exp.saturating_add(TOLERANCIA_SEG) {
        return Err(Motivo::Vencido);
    }
    if let Some(nbf) = numero(&claims["nbf"])
        && agora < nbf.saturating_sub(TOLERANCIA_SEG)
    {
        return Err(Motivo::AindaNaoVale);
    }
    Ok(Conferido {
        claims,
        endossos: chave.endossos.clone(),
    })
}
