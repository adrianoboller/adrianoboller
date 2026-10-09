//! Cofres externos: a credencial nomeada do no HTTP resolvida no HashiCorp Vault (KV v2), no
//! AWS Secrets Manager, no Azure Key Vault ou no GCP Secret Manager, lida sob demanda e
//! mantida so em memoria. O contrato (`CofreExterno`) e o cache moram no
//! `phxclaw-secret-broker` (`cofre.rs`), e quem consome e o `SecretBroker` da pasta
//! `credenciais/` (`resolver_externo`): o fluxo pede `"credencial": "github"` e nao sabe se o
//! valor veio do envelope local ou de um cofre.
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **A configuracao e so do operador.** URL, regiao, tenant e projeto sao chaves `cofres.*`
//!   do catalogo, e o prefixo esta em `SO_DO_OPERADOR`: um repositorio confiado que apontasse o
//!   agente para um cofre DELE receberia a credencial base (o token do Vault, a assinatura da
//!   AWS, o segredo do Entra ID, a assercao da conta de servico).
//! - **A credencial base mora no broker LOCAL** (`<pasta>/cofres`), pelo mesmo `Servico` das
//!   outras chaves (`phxclaw cofre vault-token|...`, o valor vem do ambiente do comando).
//! - **A rede e a do no HTTP** (`fluxo_http::PoliticaDeSaida`): lista de IPs internos, IP
//!   conferido preso na conexao, proxy so por opcao escrita. E a `Saida` de cada cofre so
//!   fala com a ORIGEM DECLARADA: pedido para outra origem e recusado antes de sair, e
//!   redirecionamento nao se segue -- a credencial base nao viaja para um `Location`.
//! - **Token derivado so em memoria.** O `client_token` do AppRole, o acesso do Entra ID e o
//!   do Google ficam num `SecretValue` com o prazo, no proprio cofre; nada disso vai ao disco
//!   (diferente do OAuth dos MCP, que guarda o acesso no broker para sobreviver ao reinicio).
//! - **Erro diz cofre, credencial e o motivo pelo status e pelo codigo do servico**, nunca o
//!   corpo inteiro (texto de terceiro), e passa pela tarja do valor exato de toda credencial
//!   base e token que esteve no pedido.

pub mod aws;
pub mod azure;
pub mod gcp;
pub mod sigv4;
pub mod vault;

use crate::chaves::Servico;
use crate::fluxo_http::PoliticaDeSaida;
use phxclaw_browser::BrowserPolicy;
use phxclaw_egress_broker::request_checked;
use phxclaw_http_client::{HttpRequestSpec, HttpResult};
use phxclaw_secret_broker::{CofresExternos, SecretBroker, SecretValue, scrub_text};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Pasta e espaco do broker das credenciais base.
pub const ESPACO: &str = "cofres";
/// Teto do corpo de uma resposta de cofre: o maior segredo da AWS tem 64 KiB, o do Azure 25 KiB.
const TETO_BYTES: usize = 1024 * 1024;
const PRAZO_MS: u64 = 15_000;
/// Onde se liga a saida pelo proxy, para a recusa dizer.
const OPCAO_DO_PROXY: &str = "cofres.usar_proxy_do_ambiente no config.json";

// ------------------------------------------------------------------ credenciais base

pub const VAULT_TOKEN: Servico = Servico {
    espaco: ESPACO,
    nome_do_segredo: "cofre-vault-token",
    chave: "cofres.vault.token",
    aliases: &[],
    rotulo: "o token do Vault",
    comando: "cofre vault-token",
};

pub const VAULT_SECRET_ID: Servico = Servico {
    espaco: ESPACO,
    nome_do_segredo: "cofre-vault-secret-id",
    chave: "cofres.vault.secret_id",
    aliases: &[],
    rotulo: "o secret_id do AppRole do Vault",
    comando: "cofre vault-secret-id",
};

pub const AWS_SEGREDO: Servico = Servico {
    espaco: ESPACO,
    nome_do_segredo: "cofre-aws-segredo",
    chave: "cofres.aws.segredo",
    aliases: &[],
    rotulo: "o secret access key da AWS",
    comando: "cofre aws-segredo",
};

pub const AWS_TOKEN_SESSAO: Servico = Servico {
    espaco: ESPACO,
    nome_do_segredo: "cofre-aws-token-sessao",
    chave: "cofres.aws.token_sessao",
    aliases: &[],
    rotulo: "o token de sessao da AWS",
    comando: "cofre aws-token-sessao",
};

pub const AZURE_SEGREDO: Servico = Servico {
    espaco: ESPACO,
    nome_do_segredo: "cofre-azure-segredo",
    chave: "cofres.azure.segredo",
    aliases: &[],
    rotulo: "o segredo do cliente do Entra ID",
    comando: "cofre azure-segredo",
};

pub const GCP_CONTA: Servico = Servico {
    espaco: ESPACO,
    nome_do_segredo: "cofre-gcp-conta",
    chave: "cofres.gcp.conta",
    aliases: &[],
    rotulo: "o JSON da conta de servico do GCP",
    comando: "cofre gcp-conta",
};

/// Os comandos `phxclaw cofre <qual>` e o `Servico` de cada um.
pub const BASES: [(&str, &Servico); 6] = [
    ("vault-token", &VAULT_TOKEN),
    ("vault-secret-id", &VAULT_SECRET_ID),
    ("aws-segredo", &AWS_SEGREDO),
    ("aws-token-sessao", &AWS_TOKEN_SESSAO),
    ("azure-segredo", &AZURE_SEGREDO),
    ("gcp-conta", &GCP_CONTA),
];

/// O valor da credencial base, por concessao curta, como `SecretValue` que morre no fim da
/// chamada. Sem ela, o erro diz o comando que a guarda.
pub(crate) fn base(raiz: &Path, s: &Servico) -> Result<SecretValue, String> {
    s.credencial(raiz)?
        .com("send", |v| Ok(SecretValue::new(v.to_string())))
}

/// A base esta guardada? Sem abrir broker que nao existe (abrir cria a chave-mestra).
fn guardada(raiz: &Path, s: &Servico) -> Result<bool, String> {
    if !s.pasta(raiz).join("segredos/master.key").exists() {
        return Ok(false);
    }
    let b = crate::canais::broker_em(&s.pasta(raiz))?;
    Ok(crate::canais::segredo_guardado(&b, s.nome_do_segredo, s.espaco)?.is_some())
}

/// A base opcional (o token de sessao da AWS): ausente nao e erro.
pub(crate) fn base_opcional(raiz: &Path, s: &Servico) -> Result<Option<SecretValue>, String> {
    if !guardada(raiz, s)? {
        return Ok(None);
    }
    base(raiz, s).map(Some)
}

/// `phxclaw cofre <qual> [--pasta DIR]`: a variavel do catalogo (no ambiente do comando)
/// vai para o broker de `<pasta>/cofres`.
pub fn cli(raiz: &Path, args: &[String]) -> Result<String, String> {
    let qual = args.first().map(String::as_str).unwrap_or("");
    let Some((_, s)) = BASES.iter().find(|(n, _)| *n == qual) else {
        return Err(format!(
            "uso: phxclaw cofre {} [--pasta DIR]",
            BASES.map(|(n, _)| n).join("|")
        ));
    };
    let id = s.guardar_do_ambiente(raiz)?;
    Ok(format!(
        "{} guardado no broker de {} (segredo {id})",
        s.rotulo,
        s.pasta(raiz).display()
    ))
}

// ------------------------------------------------------------------ configuracao

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetodoVault {
    #[default]
    Token,
    Approle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfgVault {
    pub url: String,
    pub namespace: Option<String>,
    pub montagem: String,
    pub metodo: MetodoVault,
    pub approle_montagem: String,
    pub role_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfgAws {
    pub regiao: String,
    pub url: Option<String>,
    pub chave_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfgAzure {
    pub url: String,
    pub tenant: String,
    pub cliente_id: String,
    pub token_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfgGcp {
    pub projeto: Option<String>,
    pub url: Option<String>,
    pub token_url: Option<String>,
}

/// As chaves `cofres.*` do catalogo, ja lidas. Um cofre existe quando a chave que o liga
/// (a URL do Vault e do Azure, a regiao da AWS, a conta guardada do GCP) esta definida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigCofres {
    pub vault: Option<CfgVault>,
    pub aws: Option<CfgAws>,
    pub azure: Option<CfgAzure>,
    pub gcp: Option<CfgGcp>,
    pub cache_segundos: u64,
    pub cache_max: usize,
    pub liberar: Vec<String>,
    pub usar_proxy_do_ambiente: bool,
}

impl Default for ConfigCofres {
    fn default() -> Self {
        Self {
            vault: None,
            aws: None,
            azure: None,
            gcp: None,
            cache_segundos: phxclaw_secret_broker::cofre::TTL_PADRAO.as_secs(),
            cache_max: phxclaw_secret_broker::cofre::TETO_PADRAO,
            liberar: vec![],
            usar_proxy_do_ambiente: false,
        }
    }
}

impl ConfigCofres {
    /// Do `config.json` (e do ambiente), pelo ponto unico. O GCP liga quando a conta esta
    /// guardada no broker de `raiz`: e ela que diz o projeto, o resto tem padrao.
    pub fn do_config(raiz: &Path) -> Self {
        use crate::config::{booleano_de, inteiro_de, lista_de, texto_de};
        let vault = texto_de("cofres.vault.url").map(|url| CfgVault {
            url,
            namespace: texto_de("cofres.vault.namespace"),
            montagem: texto_de("cofres.vault.montagem").unwrap_or_else(|| "secret".into()),
            metodo: match texto_de("cofres.vault.metodo").as_deref() {
                Some("approle") => MetodoVault::Approle,
                _ => MetodoVault::Token,
            },
            approle_montagem: texto_de("cofres.vault.approle_montagem")
                .unwrap_or_else(|| "approle".into()),
            role_id: texto_de("cofres.vault.role_id"),
        });
        let aws = texto_de("cofres.aws.regiao").map(|regiao| CfgAws {
            regiao,
            url: texto_de("cofres.aws.url"),
            chave_id: texto_de("cofres.aws.chave_id").unwrap_or_default(),
        });
        let azure = texto_de("cofres.azure.url").map(|url| CfgAzure {
            url,
            tenant: texto_de("cofres.azure.tenant").unwrap_or_default(),
            cliente_id: texto_de("cofres.azure.cliente_id").unwrap_or_default(),
            token_url: texto_de("cofres.azure.token_url"),
        });
        let gcp_declarado = texto_de("cofres.gcp.projeto").is_some()
            || texto_de("cofres.gcp.url").is_some()
            || guardada(raiz, &GCP_CONTA).unwrap_or(false);
        let gcp = gcp_declarado.then(|| CfgGcp {
            projeto: texto_de("cofres.gcp.projeto"),
            url: texto_de("cofres.gcp.url"),
            token_url: texto_de("cofres.gcp.token_url"),
        });
        let padrao = Self::default();
        Self {
            vault,
            aws,
            azure,
            gcp,
            cache_segundos: inteiro_de("cofres.cache_segundos")
                .and_then(|n| u64::try_from(n).ok())
                .unwrap_or(padrao.cache_segundos),
            cache_max: inteiro_de("cofres.cache_max")
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(padrao.cache_max),
            liberar: lista_de("cofres.liberar").unwrap_or_default(),
            usar_proxy_do_ambiente: booleano_de("cofres.usar_proxy_do_ambiente").unwrap_or(false),
        }
    }

    fn marca(&self, raiz: &Path) -> String {
        format!(
            "{}|{}",
            raiz.display(),
            serde_json::to_string(self).unwrap_or_default()
        )
    }
}

/// Os cofres de `cfg`, com as credenciais base em `raiz`. Configuracao invalida (URL sem
/// HTTPS fora do loopback, regiao com caractere estranho) e erro aqui, antes de qualquer
/// pedido.
pub fn montar(raiz: &Path, cfg: &ConfigCofres) -> Result<CofresExternos, String> {
    let mut c = CofresExternos::novo(
        Duration::from_secs(cfg.cache_segundos),
        cfg.cache_max,
        cfg.marca(raiz),
    );
    if let Some(v) = &cfg.vault {
        c = c.com(Arc::new(vault::Vault::novo(raiz, v, cfg)?));
    }
    if let Some(a) = &cfg.aws {
        c = c.com(Arc::new(aws::Aws::novo(raiz, a, cfg)?));
    }
    if let Some(a) = &cfg.azure {
        c = c.com(Arc::new(azure::Azure::novo(raiz, a, cfg)?));
    }
    if let Some(g) = &cfg.gcp {
        c = c.com(Arc::new(gcp::Gcp::novo(raiz, g, cfg)?));
    }
    Ok(c)
}

/// Liga ao `broker` os cofres de `cfg`, remontando so quando a configuracao mudou (o cache
/// sobrevive entre pedidos; a configuracao nova vale sem reiniciar).
pub fn ligar_com(broker: &SecretBroker, raiz: &Path, cfg: &ConfigCofres) -> Result<(), String> {
    if broker
        .cofres()
        .is_some_and(|c| c.marca() == cfg.marca(raiz))
    {
        return Ok(());
    }
    broker
        .ligar_cofres(Arc::new(montar(raiz, cfg)?))
        .map_err(|e| e.to_string())
}

/// `ligar_com` com a configuracao do `config.json`.
pub fn ligar(broker: &SecretBroker, raiz: &Path) -> Result<(), String> {
    ligar_com(broker, raiz, &ConfigCofres::do_config(raiz))
}

// ------------------------------------------------------------------ a saida

/// O cliente de UM cofre: so fala com a origem declarada, pela politica de saida da casa,
/// sem seguir redirecionamento.
pub(crate) struct Saida {
    origem: String,
    politica: BrowserPolicy,
    usar_proxy: bool,
}

impl Saida {
    /// `url` e a declarada pelo operador: HTTPS, ou HTTP so em loopback (o Vault de
    /// desenvolvimento, o servidor falso dos testes) -- a credencial base nao anda em claro
    /// pela rede.
    pub(crate) fn nova(url: &str, cfg: &ConfigCofres) -> Result<Self, String> {
        let u = Url::parse(url.trim()).map_err(|e| format!("url do cofre {url:?}: {e}"))?;
        let loopback = matches!(
            u.host_str(),
            Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
        );
        if u.scheme() != "https" && !(u.scheme() == "http" && loopback) {
            return Err(format!(
                "url do cofre {url:?}: so HTTPS (HTTP so em loopback)"
            ));
        }
        if !u.username().is_empty() || u.password().is_some() {
            return Err("url do cofre com usuario/senha: a credencial vai pelo broker".into());
        }
        Ok(Self {
            origem: u.origin().ascii_serialization(),
            politica: BrowserPolicy {
                allowed_origins: cfg.liberar.clone(),
                allow_any_public: true,
                block_private_networks: true,
            },
            usar_proxy: cfg.usar_proxy_do_ambiente,
        })
    }

    /// Manda `spec` se ele vai a origem declarada; status 3xx e erro.
    pub(crate) async fn enviar(&self, mut spec: HttpRequestSpec) -> Result<HttpResult, String> {
        let u = Url::parse(&spec.url).map_err(|e| format!("url: {e}"))?;
        if u.origin().ascii_serialization() != self.origem {
            return Err(format!(
                "destino {} fora da origem declarada do cofre ({})",
                u.origin().ascii_serialization(),
                self.origem
            ));
        }
        spec.follow_redirects = false;
        spec.max_redirects = 0;
        spec.timeout_ms = PRAZO_MS;
        spec.connect_timeout_ms = PRAZO_MS.min(10_000);
        spec.user_agent = Some(concat!("PhxClaw/", env!("CARGO_PKG_VERSION")).into());
        let gastos = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let avisos = std::sync::Mutex::new(BTreeSet::new());
        let saida = PoliticaDeSaida {
            politica: &self.politica,
            usar_proxy: self.usar_proxy,
            opcao_do_proxy: OPCAO_DO_PROXY,
            teto: TETO_BYTES,
            gastos: &gastos,
            avisos: &avisos,
        };
        let r = request_checked(&spec, |u| saida.opcoes(u))
            .await
            .map_err(|e| e.to_string())?;
        for a in avisos.into_inner().unwrap_or_default() {
            eprintln!("aviso: cofre: {a}");
        }
        if (300..400).contains(&r.status) {
            return Err(format!(
                "o cofre respondeu redirecionamento ({}): a credencial nao segue para outro destino",
                r.status
            ));
        }
        Ok(r)
    }
}

// ------------------------------------------------------------------ ajudas comuns

/// Um token derivado em memoria (AppRole, Entra ID, Google), com o vencimento.
#[derive(Default)]
pub(crate) struct TokenEmMemoria(std::sync::Mutex<Option<(SecretValue, Instant)>>);

impl TokenEmMemoria {
    /// Folga antes do vencimento: o relogio do servico e o nosso nao batem ao segundo.
    const FOLGA: Duration = Duration::from_secs(60);

    pub(crate) fn valido(&self) -> Option<SecretValue> {
        let g = self.0.lock().unwrap_or_else(|p| p.into_inner());
        g.as_ref()
            .filter(|(_, vence)| Instant::now() + Self::FOLGA < *vence)
            .map(|(v, _)| v.clone())
    }

    pub(crate) fn guardar(&self, v: SecretValue, validade: Duration) {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = Some((v, Instant::now() + validade));
    }

    pub(crate) fn esquecer(&self) {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
}

/// Tira do texto o valor exato de cada segredo que esteve no pedido.
pub(crate) fn limpar(t: String, segredos: &[&SecretValue]) -> String {
    let v: Vec<SecretValue> = segredos.iter().map(|s| (*s).clone()).collect();
    scrub_text(&t, &v)
}

/// O `campo` de um valor JSON (o segredo guardado como objeto), ou o valor inteiro sem
/// `campo`. O erro cita as CHAVES que existem, nunca valor.
pub(crate) fn escolher_campo(texto: &str, campo: Option<&str>) -> Result<SecretValue, String> {
    let Some(campo) = campo else {
        return Ok(SecretValue::new(texto.to_string()));
    };
    let v: serde_json::Value = serde_json::from_str(texto)
        .map_err(|_| format!("'campo' {campo:?} pedido, mas o valor guardado nao e JSON"))?;
    campo_do_objeto(&v, campo)
}

pub(crate) fn campo_do_objeto(v: &serde_json::Value, campo: &str) -> Result<SecretValue, String> {
    let o = v
        .as_object()
        .ok_or_else(|| format!("'campo' {campo:?} pedido, mas o valor nao e objeto JSON"))?;
    match o.get(campo) {
        Some(serde_json::Value::String(s)) => Ok(SecretValue::new(s.clone())),
        Some(serde_json::Value::Null) | None => Err(format!(
            "campo {campo:?} nao existe (campos: {})",
            o.keys().cloned().collect::<Vec<_>>().join(", ")
        )),
        Some(outro) => Ok(SecretValue::new(outro.to_string())),
    }
}

/// Um segmento de caminho para a URL: sem `.`/`..`, sem barra, codificado.
pub(crate) fn segmento(s: &str, o_que: &str) -> Result<String, String> {
    if s.is_empty() || s == "." || s == ".." || s.contains('/') || s.chars().any(char::is_control) {
        return Err(format!("{o_que} invalido: {s:?}"));
    }
    Ok(sigv4::codificar(s))
}

/// O motivo de um status de erro: o status e o codigo que o servico poe no corpo (nunca a
/// mensagem livre, que pode ecoar o pedido).
pub(crate) fn motivo_do_status(status: u16, codigo: Option<String>) -> String {
    let base = match status {
        401 => "credencial base recusada (401)".to_string(),
        403 => "permissao negada (403)".to_string(),
        404 => "nao encontrado (404)".to_string(),
        429 => "limite de pedidos do cofre (429)".to_string(),
        s => format!("HTTP {s}"),
    };
    match codigo.filter(|c| {
        !c.is_empty()
            && c.len() <= 80
            && c.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "._-#: ".contains(ch))
    }) {
        Some(c) => format!("{base}: {c}"),
        None => base,
    }
}

/// O corpo como JSON (o cofre responde JSON; corpo que nao e vira `Null`).
pub(crate) fn json_de(r: &HttpResult) -> serde_json::Value {
    r.bytes()
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::Value::Null)
}
