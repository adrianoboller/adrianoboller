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
//! - JWKS vencido que nao recarrega e recusa (503), e nao a chave velha: falha fechada;
//! - o download do JWKS corre fora da trava do cache, um por vez, e quem chega durante ele
//!   usa o conjunto atual -- um servico de chaves lento nao segura token bom;
//! - a conferencia da assinatura tem teto de vagas em voo por JWKS (429 acima dele): e a
//!   conta cara que qualquer um dispara sem credencial (`teto_em_voo_padrao`);
//! - a recusa sai sem detalhe (`Motivo::recusa`): quem forja um token nao aprende qual
//!   pedaco errou. O `Motivo` existe para o teste provar que recusou PELO motivo certo.

use super::caixa::{PedidoWebhook, Recusa};
use super::http::{Http, politica_para};
use super::rsa::ChavePublica;
use base64::Engine;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// O teto de conferencias de assinatura em voo esta cheio (`Jwks::vaga`).
    Ocupado,
}

impl Motivo {
    /// O que vai para quem bateu na porta: 401 sem dizer o que errou; 403 quando o token e
    /// bom e o pedido nao (a Microsoft pede 403 para o endosso); 503 quando somos nos que
    /// nao temos a chave.
    pub fn recusa(self) -> Recusa {
        match self {
            Motivo::ChavesIndisponiveis => (503, "chaves de assinatura indisponiveis".into()),
            Motivo::NaoVaiAqui => (403, "token nao vale para este pedido".into()),
            Motivo::Ocupado => (429, "conferencias de assinatura demais em andamento".into()),
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
    /// Alguem esta baixando agora. O download corre FORA da trava (o cliente HTTP e
    /// bloqueante e um servico lento segura ate o prazo dele, ~50 s): com a trava presa,
    /// todo token -- inclusive o de `kid` conhecido -- esperava atras do download.
    baixando: bool,
}

/// A vez de baixar o JWKS. Solta a vez ate num panico do download: sem isto, um panico
/// deixaria `baixando` ligado para sempre e o JWKS nunca mais recarregaria.
struct Vez<'a>(&'a Mutex<Cache>);

impl Vez<'_> {
    /// Guarda o resultado e solta a vez no MESMO trecho travado: soltar antes abriria uma
    /// fresta para outro download comecar e o resultado mais velho sobrescrever o novo.
    fn concluir<R>(self, f: impl FnOnce(&mut Cache) -> R) -> R {
        let mut c = self.0.lock().unwrap_or_else(|e| e.into_inner());
        c.baixando = false;
        let r = f(&mut c);
        drop(c);
        // O `Drop` so existe para o caminho do panico; aqui a vez ja foi solta acima.
        std::mem::forget(self);
        r
    }
}

impl Drop for Vez<'_> {
    fn drop(&mut self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).baixando = false;
    }
}

/// Quantas conferencias de assinatura podem correr ao mesmo tempo num JWKS antes de a
/// proxima sair 429. A conferencia e a unica conta cara que um desconhecido dispara sem
/// credencial nenhuma: basta um `kid` publico e lixo do tamanho da chave. Medido (reducao
/// bit a bit, binario otimizado, 4 nucleos): ~1,4 ms por verificacao de 2048 bits e ~4,9 ms
/// de 4096. Com uma em voo, ja sao ~700 por segundo (2048) -- muito acima do que um bot
/// recebe --, entao o teto e baixo de proposito: metade dos nucleos, no minimo 2 (com dois
/// webhooks legitimos chegando juntos, o segundo nao leva 429 a toa). Acima dele RECUSA em
/// vez de enfileirar: fila seguraria uma thread do `spawn_blocking` por pedido do atacante.
pub fn teto_em_voo_padrao() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get() / 2)
        .unwrap_or(1)
        .max(2)
}

/// Uma vaga de conferencia de assinatura; devolve a vaga ao sair, ate em panico.
struct Vaga<'a>(&'a AtomicUsize);

impl Drop for Vaga<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// O JWKS de um servico, com cache. A busca passa pela politica de destinos como todo
/// pedido dos canais (`Http`), que recusa `http://` fora de loopback.
pub struct Jwks {
    http: Http,
    recarga: Duration,
    intervalo: Duration,
    cache: Mutex<Cache>,
    em_voo: AtomicUsize,
    teto_em_voo: usize,
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
            em_voo: AtomicUsize::new(0),
            teto_em_voo: teto_em_voo_padrao(),
        })
    }

    /// O JWKS que a configuracao pediu, ou o `oficial` sem configuracao. A URL do JWKS e a
    /// ancora de confianca do canal -- quem a troca escolhe quais chaves assinam por nos --:
    /// nao e segredo, mas nao pode ser trocada calada, entao a troca sai como aviso ao subir.
    pub fn configurado(canal: &str, url: Option<&str>, oficial: &str) -> Result<Self, String> {
        let url = url
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .unwrap_or(oficial);
        if let Some(a) = aviso_jwks(canal, url, oficial) {
            eprintln!("{a}");
        }
        Self::novo(url)
    }

    /// O teste fixa o teto para provar a recusa sem depender de quantos nucleos tem.
    pub fn com_teto_em_voo(mut self, teto: usize) -> Self {
        self.teto_em_voo = teto;
        self
    }

    /// A vaga, ou `None` com o teto cheio. Decide ANTES da conta: o portao que corta o
    /// trabalho caro tem de vir antes dele.
    fn vaga(&self) -> Option<Vaga<'_>> {
        self.em_voo
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < self.teto_em_voo).then_some(n + 1)
            })
            .ok()
            .map(|_| Vaga(&self.em_voo))
    }

    fn baixar(&self) -> Result<Chaves, String> {
        let corpo = self
            .http
            .binario(self.http.cliente()?.get(self.http.base()), TETO_JWKS)?;
        let v: Value =
            serde_json::from_slice(&corpo).map_err(|e| format!("JWKS nao e JSON: {e}"))?;
        chaves_do_jwks(&v)
    }

    /// A chave do `kid`, recarregando se o cache venceu ou se o `kid` e novo. Um so baixa
    /// por vez, e sem a trava: quem chega durante o download responde com o conjunto atual
    /// -- a chave conhecida vale (inclusive a de um cache que acabou de vencer: o vencido so
    /// vira recusa quando a recarga FALHA), o `kid` desconhecido e 401, e sem conjunto
    /// nenhum ainda e 503. Ninguem espera o download alheio, entao um servico de chaves
    /// lento nao segura token bom.
    pub fn chave(&self, kid: &str) -> Result<Arc<ChaveJwks>, Motivo> {
        let agora = Instant::now();
        let vez;
        let vencido;
        {
            let mut c = self.cache.lock().map_err(|_| Motivo::ChavesIndisponiveis)?;
            vencido = c
                .lido
                .is_none_or(|t| agora.duration_since(t) >= self.recarga);
            let conhecida = c.chaves.get(kid).cloned();
            if !vencido && let Some(k) = conhecida {
                return Ok(k);
            }
            if c.baixando {
                return match (conhecida, c.lido) {
                    (Some(k), _) => Ok(k),
                    (None, None) => Err(Motivo::ChavesIndisponiveis),
                    (None, Some(_)) => Err(Motivo::KidDesconhecido),
                };
            }
            let pode = c
                .tentado
                .is_none_or(|t| agora.duration_since(t) >= self.intervalo);
            if !pode {
                return Err(if vencido {
                    Motivo::ChavesIndisponiveis
                } else {
                    Motivo::KidDesconhecido
                });
            }
            c.tentado = Some(agora);
            c.baixando = true;
            vez = Vez(&self.cache);
        }
        let baixado = self.baixar();
        vez.concluir(|c| {
            match baixado {
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
            c.chaves.get(kid).cloned().ok_or(Motivo::KidDesconhecido)
        })
    }
}

/// O aviso de JWKS trocado, ou `None` quando e o oficial. Separado do `eprintln!` para o
/// teste provar o texto sem capturar a saida de erro.
pub fn aviso_jwks(canal: &str, url: &str, oficial: &str) -> Option<String> {
    (url.trim() != oficial).then(|| {
        format!(
            "aviso: {canal}: JWKS trocado para {url} (o oficial e {oficial}); as chaves \
             dessa URL decidem quem assina os webhooks deste canal"
        )
    })
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
    let vaga = jwks.vaga().ok_or(Motivo::Ocupado)?;
    if !chave.chave.verificar(assinado.as_bytes(), &assinatura) {
        return Err(Motivo::Assinatura);
    }
    drop(vaga);
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
