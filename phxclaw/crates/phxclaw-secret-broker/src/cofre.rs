//! Cofre EXTERNO: a credencial nomeada que nao mora no envelope local, e sim num cofre do
//! operador (HashiCorp Vault, AWS Secrets Manager, Azure Key Vault, GCP Secret Manager).
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Um motor so.** O trait `CofreExterno` e o contrato; as implementacoes (que falam HTTP)
//!   moram no agente, e quem as consome e o `SecretBroker` (`resolver_externo`). Quem pede a
//!   credencial por NOME -- o no HTTP, o fluxo -- nao sabe de onde o valor vem.
//! - **So em memoria.** O valor lido vive no `SecretValue` (zerado no `drop`) e num cache
//!   com prazo curto e teto de entradas. Nada daqui grava em disco: o envelope cifrado do
//!   broker e para o segredo que o operador GUARDOU; o do cofre externo e lido sob demanda,
//!   e grava-lo seria criar uma segunda copia que a rotacao no cofre nao alcanca.
//! - **O erro diz qual cofre e qual nome, nunca o valor.** `ErroDeCofre` carrega o cofre, a
//!   credencial e o motivo; o motivo vem da implementacao (status, codigo de erro do servico)
//!   e passa ainda pela tarja de forma (`scrub_secret_like`) -- o corpo de erro de um cofre e
//!   texto de terceiro.
//! - **Prazo curto de proposito.** O cache existe para um fluxo de mil itens nao fazer mil
//!   pedidos ao cofre, nao para a rotacao demorar a valer: o prazo padrao e 60 s e o teto
//!   aceito e 300 s (`TTL_MAX`).

use crate::{SecretValue, scrub_secret_like};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// O futuro que o trait devolve: o broker nao depende de runtime nenhum, so da `std`.
pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Prazo padrao do cache, e o teto que a configuracao pode pedir.
pub const TTL_PADRAO: Duration = Duration::from_secs(60);
pub const TTL_MAX: Duration = Duration::from_secs(300);
/// Entradas no cache, padrao e teto.
pub const TETO_PADRAO: usize = 64;
pub const TETO_MAX: usize = 1024;

/// Onde a credencial mora no cofre. O significado de cada campo e do tipo de cofre:
/// `caminho` e o caminho do KV no Vault, o `SecretId` na AWS e o nome do segredo no Azure e
/// no GCP; `campo` escolhe uma chave do JSON guardado (obrigatorio no Vault, que guarda um
/// mapa); `versao` fixa a versao (sem ela, a atual).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenciaExterna {
    pub cofre: String,
    pub caminho: String,
    #[serde(default)]
    pub campo: Option<String>,
    #[serde(default)]
    pub versao: Option<String>,
}

impl ReferenciaExterna {
    /// O que se pode conferir sem rede: cofre e caminho presentes, sem controle nem quebra
    /// de linha (o caminho vai para URL e para log).
    pub fn validar(&self) -> Result<(), String> {
        let limpo = |t: &str| !t.is_empty() && t.len() <= 512 && !t.chars().any(char::is_control);
        if !limpo(&self.cofre) {
            return Err("referencia de cofre sem 'cofre'".into());
        }
        if !limpo(&self.caminho) {
            return Err("referencia de cofre sem 'caminho' valido".into());
        }
        for (nome, v) in [("campo", &self.campo), ("versao", &self.versao)] {
            if let Some(v) = v
                && !limpo(v)
            {
                return Err(format!("referencia de cofre com '{nome}' invalido"));
            }
        }
        Ok(())
    }
}

/// Um cofre externo. `ler` devolve o valor ou o MOTIVO da falha -- texto que nao pode
/// conter o valor (quem implementa monta o motivo do status e do codigo de erro do
/// servico, nunca do corpo inteiro).
pub trait CofreExterno: Send + Sync {
    /// `vault`, `aws`, `azure`, `gcp`: o nome que a referencia usa.
    fn tipo(&self) -> &'static str;
    fn ler<'a>(&'a self, r: &'a ReferenciaExterna) -> BoxFut<'a, Result<SecretValue, String>>;
}

/// A falha de resolver uma credencial num cofre externo: qual cofre, qual credencial, e o
/// motivo -- sem valor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroDeCofre {
    pub cofre: String,
    pub credencial: String,
    pub motivo: String,
}

impl std::fmt::Display for ErroDeCofre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cofre {}: credencial {}: {}",
            self.cofre, self.credencial, self.motivo
        )
    }
}

impl std::error::Error for ErroDeCofre {}

struct Entrada {
    valor: SecretValue,
    vence: Instant,
    nasceu: Instant,
}

/// Os cofres configurados e o cache em memoria dos valores lidos.
pub struct CofresExternos {
    cofres: BTreeMap<String, Arc<dyn CofreExterno>>,
    cache: Mutex<BTreeMap<ReferenciaExterna, Entrada>>,
    ttl: Duration,
    teto: usize,
    /// A marca da configuracao que montou estes cofres: quem os liga ao broker compara e
    /// remonta quando a configuracao muda (ver `SecretBroker::ligar_cofres`).
    marca: String,
}

impl CofresExternos {
    /// `ttl` e `teto` sao presos aos limites (`TTL_MAX`, `TETO_MAX`); zero desliga o cache.
    pub fn novo(ttl: Duration, teto: usize, marca: impl Into<String>) -> Self {
        Self {
            cofres: BTreeMap::new(),
            cache: Mutex::new(BTreeMap::new()),
            ttl: ttl.min(TTL_MAX),
            teto: teto.min(TETO_MAX),
            marca: marca.into(),
        }
    }

    #[must_use]
    pub fn com(mut self, cofre: Arc<dyn CofreExterno>) -> Self {
        self.cofres.insert(cofre.tipo().to_string(), cofre);
        self
    }

    pub fn marca(&self) -> &str {
        &self.marca
    }

    pub fn tipos(&self) -> Vec<&str> {
        self.cofres.keys().map(String::as_str).collect()
    }

    /// Quantas entradas o cache tem agora (as vencidas inclusive, ate a proxima limpeza).
    pub fn em_cache(&self) -> usize {
        self.cache.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// Tira a referencia do cache: o 401/403 de quem USOU o valor diz que ele mudou no cofre.
    pub fn esquecer(&self, r: &ReferenciaExterna) {
        if let Ok(mut c) = self.cache.lock() {
            c.remove(r);
        }
    }

    fn do_cache(&self, r: &ReferenciaExterna) -> Option<SecretValue> {
        let c = self.cache.lock().ok()?;
        c.get(r)
            .filter(|e| Instant::now() < e.vence)
            .map(|e| e.valor.clone())
    }

    fn guardar(&self, r: &ReferenciaExterna, valor: &SecretValue) {
        if self.ttl.is_zero() || self.teto == 0 {
            return;
        }
        let Ok(mut c) = self.cache.lock() else {
            return;
        };
        let agora = Instant::now();
        c.retain(|_, e| agora < e.vence);
        // Cheio mesmo sem as vencidas: sai a mais velha. O teto e de memoria, nao de
        // acerto -- perder uma entrada custa um pedido ao cofre, nada mais.
        while c.len() >= self.teto {
            let Some(velha) = c
                .iter()
                .min_by_key(|(_, e)| e.nasceu)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            c.remove(&velha);
        }
        c.insert(
            r.clone(),
            Entrada {
                valor: valor.clone(),
                vence: agora + self.ttl,
                nasceu: agora,
            },
        );
    }

    /// O valor de `r` para a credencial `credencial`: do cache se ainda vale, senao do cofre.
    /// O `bool` diz se veio do cache (vai para a evidencia, nunca o valor).
    pub async fn resolver(
        &self,
        credencial: &str,
        r: &ReferenciaExterna,
    ) -> Result<(SecretValue, bool), ErroDeCofre> {
        let erro = |motivo: String| ErroDeCofre {
            cofre: r.cofre.clone(),
            credencial: credencial.to_string(),
            motivo: scrub_secret_like(&motivo),
        };
        r.validar().map_err(erro)?;
        if let Some(v) = self.do_cache(r) {
            return Ok((v, true));
        }
        let cofre = self.cofres.get(&r.cofre).ok_or_else(|| {
            erro(format!(
                "cofre nao configurado (configurados: {})",
                if self.cofres.is_empty() {
                    "nenhum".to_string()
                } else {
                    self.tipos().join(", ")
                }
            ))
        })?;
        let valor = cofre.ler(r).await.map_err(erro)?;
        if valor.expose().is_empty() {
            return Err(erro("o cofre devolveu valor vazio".into()));
        }
        self.guardar(r, &valor);
        Ok((valor, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Executor minimo: os cofres falsos daqui respondem sem esperar nada.
    fn rodar<T>(f: impl Future<Output = T>) -> T {
        let mut f = std::pin::pin!(f);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        loop {
            if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    struct Falso {
        lidas: AtomicUsize,
    }

    impl CofreExterno for Falso {
        fn tipo(&self) -> &'static str {
            "falso"
        }
        fn ler<'a>(&'a self, r: &'a ReferenciaExterna) -> BoxFut<'a, Result<SecretValue, String>> {
            Box::pin(async move {
                let n = self.lidas.fetch_add(1, Ordering::SeqCst);
                match r.caminho.as_str() {
                    "falha" => Err("HTTP 403: permission denied".into()),
                    "eco" => Err("o servico disse: Bearer abcdefghijklmnopqrstuvwx".into()),
                    "vazio" => Ok(SecretValue::new(String::new())),
                    c => Ok(SecretValue::new(format!("valor-{c}-{n}"))),
                }
            })
        }
    }

    fn refe(c: &str) -> ReferenciaExterna {
        ReferenciaExterna {
            cofre: "falso".into(),
            caminho: c.into(),
            campo: None,
            versao: None,
        }
    }

    fn cofres(ttl: u64, teto: usize) -> (CofresExternos, Arc<Falso>) {
        let f = Arc::new(Falso {
            lidas: AtomicUsize::new(0),
        });
        (
            CofresExternos::novo(Duration::from_secs(ttl), teto, "t").com(f.clone()),
            f,
        )
    }

    #[test]
    fn cache_serve_a_segunda_leitura_e_respeita_o_teto() {
        let (c, f) = cofres(60, 2);
        let (v1, cache1) = rodar(c.resolver("a", &refe("x"))).unwrap();
        let (v2, cache2) = rodar(c.resolver("a", &refe("x"))).unwrap();
        assert_eq!((cache1, cache2), (false, true));
        assert_eq!(v1.expose(), v2.expose());
        assert_eq!(f.lidas.load(Ordering::SeqCst), 1);
        rodar(c.resolver("a", &refe("y"))).unwrap();
        rodar(c.resolver("a", &refe("z"))).unwrap();
        assert_eq!(c.em_cache(), 2, "o teto de 2 entradas vale");
        // A mais velha (x) saiu: ler de novo vai ao cofre.
        assert!(!rodar(c.resolver("a", &refe("x"))).unwrap().1);
        c.esquecer(&refe("z"));
        assert!(!rodar(c.resolver("a", &refe("z"))).unwrap().1);
    }

    #[test]
    fn ttl_zero_desliga_o_cache_e_ttl_alto_e_preso_ao_teto() {
        let (c, f) = cofres(0, 8);
        rodar(c.resolver("a", &refe("x"))).unwrap();
        rodar(c.resolver("a", &refe("x"))).unwrap();
        assert_eq!(f.lidas.load(Ordering::SeqCst), 2);
        assert_eq!(c.em_cache(), 0);
        let (alto, _) = cofres(86_400, 1_000_000);
        assert_eq!((alto.ttl, alto.teto), (TTL_MAX, TETO_MAX));
    }

    #[test]
    fn erro_diz_cofre_e_credencial_e_tarja_o_que_tem_forma_de_segredo() {
        let (c, _) = cofres(60, 8);
        let e = rodar(c.resolver("github", &refe("falha"))).unwrap_err();
        assert_eq!(
            e.to_string(),
            "cofre falso: credencial github: HTTP 403: permission denied"
        );
        let e = rodar(c.resolver("github", &refe("eco"))).unwrap_err();
        assert!(!e.to_string().contains("abcdefghijklmnop"), "{e}");
        let e = rodar(c.resolver("github", &refe("vazio"))).unwrap_err();
        assert!(e.motivo.contains("vazio"), "{e}");
        let mut outro = refe("x");
        outro.cofre = "vault".into();
        let e = rodar(c.resolver("github", &outro)).unwrap_err();
        assert_eq!(
            e.to_string(),
            "cofre vault: credencial github: cofre nao configurado (configurados: falso)"
        );
        let mut quebrado = refe("a\nb");
        quebrado.cofre = "falso".into();
        assert!(rodar(c.resolver("github", &quebrado)).is_err());
    }
}
