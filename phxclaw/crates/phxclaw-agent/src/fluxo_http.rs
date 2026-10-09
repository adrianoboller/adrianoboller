//! O no HTTP generico: a ferramenta `http_request` (capacidade `http.request`) e o passo
//! `http` do motor de fluxo, que e a MESMA ferramenta chamada pelo portao unico
//! (`Agent::call_tool`). Metodo, cabecalhos, query, corpo JSON/form/texto, teto de tempo e
//! de bytes, credencial por NOME, paginacao (cursor ou link `next`) e lotes; a resposta
//! vira itens do fluxo. O gatilho de poll (`gatilho_poll.rs`) faz o pedido por aqui tambem.
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Credencial so por nome, nunca por valor.** O pedido diz `"credencial": "nome"`; a
//!   declaracao (tipo e ORIGENS) mora em `http.json` na raiz do agente e o segredo no
//!   SecretBroker da pasta `credenciais/`. Cabecalho, query ou campo de formulario com nome
//!   de segredo (`Authorization`, `X-Api-Key`, `token`...) e recusado, e todo texto do pedido
//!   passa pelo motor unico de forma (`phxclaw_types::segredo::texto_tem_credencial`): o
//!   fluxo e um JSON no disco e em todo relatorio, e o segredo nele seria o segredo em claro.
//! - **A credencial so vai as origens declaradas com ela.** Um fluxo (escrito por modelo,
//!   importado) que pedisse a credencial `github` para `https://evil.example` levaria o token
//!   embora; aqui a origem do pedido -- de cada pagina, inclusive a do link `next` -- tem de
//!   estar na lista, e o redirecionamento para outra origem tira o cabecalho (o laco do
//!   `phxclaw_egress_broker::request_checked`, o mesmo do EgressBroker).
//! - **SSRF fechado por padrao.** Cada destino, o primeiro e cada redirecionamento, passa
//!   pela politica do navegador (`BrowserPolicy::check_url_resolved`, a mesma lista de IPs
//!   internos: loopback, privada, link-local, 169.254.169.254, CGNAT, IPv4 em IPv6) e a
//!   conexao fica PRESA no IP conferido (`HttpOptions::resolve`) -- sem isso o cliente
//!   resolveria o nome de novo e um DNS que muda de resposta passaria. So `liberar` em
//!   `http.json` abre um destino interno, por origem exata.
//! - **OAuth2 e o do `oauth.rs`.** Client credentials e refresh token sao o MESMO
//!   `AutorizacaoMcp` dos MCP remotos (renovacao serializada, 401 que invalida e repete uma
//!   vez, limpeza dos segredos no erro), num broker e num espaco proprios.
//! - **Teto de bytes lido em pedacos.** O corpo e abortado ao passar do teto
//!   (`http_request_with`), e o teto vale para o no inteiro: dez paginas de 1 MiB nao viram
//!   10 MiB num teto de 2 MiB.
//! - **Erro HTTP aparece, tarjado.** Status >= 400 falha o passo com o status, o metodo, a
//!   URL sem a query e o comeco do corpo; tudo passa pelos segredos conhecidos do pedido e
//!   pelo `scrub_secret_like` antes de sair. `aceitar_erro` devolve o status como item.

use crate::oauth::{self, Alvo, AutorizacaoMcp, ConfigOauth, Tipo};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_browser::BrowserPolicy;
use phxclaw_egress_broker::{EgressError, request_checked};
use phxclaw_http_client::{HttpOptions, HttpRequestSpec, HttpResult};
use phxclaw_mcp_lsp_runtime::AuthorizationSource;
use phxclaw_secret_broker::{SecretBroker, SecretValue, scrub_secret_like, scrub_text};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// O nome da ferramenta no portao; o passo `http` do fluxo vira uma chamada a ela.
pub const FERRAMENTA: &str = "http_request";
/// Rede para fora em nome do operador: fora do padrao, ele concede.
pub const CAPACIDADE: &str = "http.request";
/// A configuracao (destinos liberados e credenciais declaradas), na raiz do agente: e da
/// maquina, nao do projeto, como os brokers.
pub const ARQUIVO: &str = "http.json";
/// Pasta do broker e espaco dos segredos das credenciais nomeadas.
pub const ESPACO: &str = "credenciais";
pub const METODOS: [&str; 6] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"];
pub const TETO_BYTES_PADRAO: usize = 2 * 1024 * 1024;
pub const TETO_BYTES_MAX: usize = 32 * 1024 * 1024;
pub const TETO_MS_PADRAO: u64 = 30_000;
pub const TETO_MS_MAX: u64 = 300_000;
pub const MAX_PAGINAS_PADRAO: usize = 10;
pub const MAX_PAGINAS: usize = 100;
pub const MAX_PAUSA_MS: u64 = 60_000;
/// Teto de itens do no inteiro (todas as paginas e lotes): acima disso falha dizendo
/// quantos, em vez de entregar ao fluxo uma lista que o `por_item` recusaria adiante.
pub const MAX_ITENS: usize = 10_000;
pub const MAX_LOTES: usize = 1_000;
const MAX_REDIRECIONAMENTOS: usize = 5;
/// Quanto do corpo de um erro HTTP entra na mensagem.
const TRECHO_DO_ERRO: usize = 300;

// ------------------------------------------------------------------ configuracao

/// `http.json` na raiz do agente.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigHttp {
    /// Origens internas liberadas (`http://127.0.0.1:9000`): o unico jeito de o no alcancar
    /// loopback, rede privada ou link-local, e tem de ser escrito, nunca implicito.
    #[serde(default)]
    pub liberar: Vec<String>,
    #[serde(default)]
    pub credenciais: BTreeMap<String, DeclCredencial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoCredencial {
    /// `Authorization: Bearer <segredo>`.
    Bearer,
    /// `Authorization: Basic base64(usuario:segredo)`.
    Basico,
    /// `<cabecalho>: <segredo>` (X-Api-Key e afins).
    Cabecalho,
    /// `Authorization: Bearer <acesso>`, o acesso do `oauth.rs` (client credentials ou
    /// refresh token).
    Oauth2,
}

/// O que uma credencial nomeada e, SEM o segredo.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclCredencial {
    pub tipo: TipoCredencial,
    /// As origens a que a credencial pode ir. Obrigatoria: credencial sem origem iria a
    /// qualquer URL que um fluxo escrevesse.
    pub origens: Vec<String>,
    #[serde(default)]
    pub usuario: Option<String>,
    #[serde(default)]
    pub cabecalho: Option<String>,
    #[serde(default)]
    pub oauth2: Option<ConfigOauth>,
}

impl ConfigHttp {
    /// `<raiz>/http.json`; ausente e vazio (nada liberado, nenhuma credencial).
    pub fn carregar(raiz_do_agente: &Path) -> Result<Self, String> {
        let arq = raiz_do_agente.join(ARQUIVO);
        let t = match std::fs::read_to_string(&arq) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("{}: {e}", arq.display())),
        };
        let c: Self = serde_json::from_str(&t).map_err(|e| format!("{}: {e}", arq.display()))?;
        for o in &c.liberar {
            origem_canonica(o).map_err(|e| format!("{}: liberar: {e}", arq.display()))?;
        }
        for (nome, d) in &c.credenciais {
            d.validar(nome)
                .map_err(|e| format!("{}: credencial {nome}: {e}", arq.display()))?;
        }
        Ok(c)
    }

    fn politica(&self) -> BrowserPolicy {
        BrowserPolicy {
            allowed_origins: self.liberar.clone(),
            allow_any_public: true,
            block_private_networks: true,
        }
    }
}

impl DeclCredencial {
    fn validar(&self, nome: &str) -> Result<(), String> {
        nome_valido(nome)?;
        if self.origens.is_empty() {
            return Err("'origens' vazia: diga a que origens a credencial pode ir".into());
        }
        for o in &self.origens {
            origem_canonica(o)?;
        }
        match self.tipo {
            TipoCredencial::Basico if self.usuario.as_deref().is_none_or(str::is_empty) => {
                return Err("tipo basico pede 'usuario'".into());
            }
            TipoCredencial::Cabecalho => {
                let c = self.cabecalho.as_deref().unwrap_or("");
                reqwest::header::HeaderName::from_bytes(c.as_bytes())
                    .map_err(|_| format!("tipo cabecalho pede 'cabecalho' valido: {c:?}"))?;
                if c.eq_ignore_ascii_case("host") {
                    return Err("'cabecalho' nao pode ser Host".into());
                }
            }
            TipoCredencial::Oauth2 => self
                .oauth2
                .as_ref()
                .ok_or("tipo oauth2 pede o bloco 'oauth2'")?
                .validar()?,
            _ => {}
        }
        if self.tipo != TipoCredencial::Oauth2 && self.oauth2.is_some() {
            return Err("bloco 'oauth2' so no tipo oauth2".into());
        }
        Ok(())
    }

    /// O texto que prende o segredo ao que a credencial alcanca (ver `Alvo::de_credencial`):
    /// mudar as origens, o endpoint de token ou o cliente pede o segredo de novo.
    fn vinculo(&self) -> String {
        let mut origens: Vec<String> = self
            .origens
            .iter()
            .filter_map(|o| origem_canonica(o).ok())
            .collect();
        origens.sort();
        origens.dedup();
        let mut v = format!("{:?} {}", self.tipo, origens.join(" "));
        if let Some(o) = &self.oauth2 {
            v.push_str(&format!(" token={} cliente={}", o.token, o.cliente_id));
        }
        if let Some(c) = &self.cabecalho {
            v.push_str(&format!(" cabecalho={}", c.to_ascii_lowercase()));
        }
        v
    }

    fn alvo(&self, nome: &str) -> Alvo {
        Alvo::de_credencial(nome, &self.vinculo(), ESPACO)
    }
}

fn nome_valido(nome: &str) -> Result<(), String> {
    if nome.is_empty()
        || nome.len() > 64
        || !nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "nome de credencial invalido: {nome:?} (letras, numeros, _ e -)"
        ));
    }
    Ok(())
}

/// `esquema://host[:porta]` pela mesma normalizacao do `Url` que a politica do navegador
/// usa: `http://h:80/` e `http://h` sao a mesma origem.
fn origem_canonica(o: &str) -> Result<String, String> {
    let u = Url::parse(o).map_err(|e| format!("origem {o:?}: {e}"))?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err(format!("origem {o:?}: so http e https"));
    }
    Ok(u.origin().ascii_serialization())
}

/// O broker das credenciais, so se ja existe: perguntar «tem credencial?» nao pode deixar
/// um cofre vazio para tras (o `broker_em` cria a chave-mestra).
fn broker(raiz_do_agente: &Path, criar: bool) -> Result<Option<Arc<SecretBroker>>, String> {
    let pasta = raiz_do_agente.join(ESPACO);
    if !criar
        && !pasta
            .join(crate::canais::PASTA_DO_BROKER)
            .join(crate::canais::CHAVE_MESTRA)
            .exists()
    {
        return Ok(None);
    }
    crate::canais::broker_em(&pasta).map(Some)
}

/// Guarda o segredo de uma credencial declarada: o valor fixo (bearer, a senha do basico, o
/// valor do cabecalho) ou, no oauth2, o segredo do cliente. O refresh token entra pelo
/// `login`.
pub fn guardar_credencial(
    raiz_do_agente: &Path,
    nome: &str,
    valor: SecretValue,
) -> Result<uuid::Uuid, String> {
    let cfg = ConfigHttp::carregar(raiz_do_agente)?;
    let d = cfg
        .credenciais
        .get(nome)
        .ok_or_else(|| format!("credencial {nome} nao esta declarada em {ARQUIVO}"))?;
    if valor.expose().trim().is_empty() {
        return Err("segredo vazio".into());
    }
    let b = broker(raiz_do_agente, true)?.ok_or("sem broker")?;
    let tipo = match d.tipo {
        TipoCredencial::Oauth2 => Tipo::Cliente,
        _ => Tipo::Bearer,
    };
    oauth::guardar(&b, &d.alvo(nome), tipo, valor)
}

/// Guarda o refresh token de uma credencial oauth2 com concessao por codigo (para quem o
/// obteve fora do `login`, ex. no console do provedor).
pub fn guardar_renovacao(
    raiz_do_agente: &Path,
    nome: &str,
    valor: SecretValue,
) -> Result<uuid::Uuid, String> {
    let cfg = ConfigHttp::carregar(raiz_do_agente)?;
    let d = cfg
        .credenciais
        .get(nome)
        .filter(|d| d.tipo == TipoCredencial::Oauth2)
        .ok_or_else(|| format!("credencial oauth2 {nome} nao esta declarada em {ARQUIVO}"))?;
    let b = broker(raiz_do_agente, true)?.ok_or("sem broker")?;
    oauth::guardar(&b, &d.alvo(nome), Tipo::Renovacao, valor)
}

/// `phxclaw credencial guardar|renovacao|login NOME`: o segredo vem da entrada padrao
/// (uma linha) e vai para o broker; nunca por argumento, que fica no historico do shell.
pub async fn cli(
    raiz_do_agente: &Path,
    args: &[String],
    entrada: &mut dyn std::io::BufRead,
) -> Result<String, String> {
    let uso = "uso: phxclaw credencial guardar|renovacao|login NOME [--pasta DIR] (segredo pela \
entrada padrao)";
    let (Some(acao), Some(nome)) = (args.first(), args.get(1)) else {
        return Err(uso.into());
    };
    let mut ler = || -> Result<SecretValue, String> {
        let mut l = String::new();
        entrada.read_line(&mut l).map_err(|e| e.to_string())?;
        let l = l.trim_end_matches(['\r', '\n']).to_string();
        if l.is_empty() {
            return Err("segredo vazio na entrada padrao".into());
        }
        Ok(SecretValue::new(l))
    };
    match acao.as_str() {
        "guardar" => {
            let id = guardar_credencial(raiz_do_agente, nome, ler()?)?;
            Ok(format!("credencial {nome} guardada (segredo {id})"))
        }
        "renovacao" => {
            let id = guardar_renovacao(raiz_do_agente, nome, ler()?)?;
            Ok(format!("refresh token de {nome} guardado (segredo {id})"))
        }
        "login" => {
            let cfg = ConfigHttp::carregar(raiz_do_agente)?;
            let d = cfg
                .credenciais
                .get(nome)
                .filter(|d| d.tipo == TipoCredencial::Oauth2)
                .ok_or_else(|| format!("credencial oauth2 {nome} nao declarada"))?;
            let o = d.oauth2.as_ref().ok_or("sem bloco oauth2")?;
            let cliente = if o.segredo_cliente {
                Some(ler()?)
            } else {
                None
            };
            let b = broker(raiz_do_agente, true)?.ok_or("sem broker")?;
            oauth::login_no(
                &b,
                &d.alvo(nome),
                o,
                cliente,
                Duration::from_secs(300),
                |url| eprintln!("Abra no navegador para autorizar {nome}:\n\n  {url}\n"),
            )
            .await?;
            Ok(format!("{nome} autorizado; refresh token no broker"))
        }
        _ => Err(uso.into()),
    }
}

// ------------------------------------------------------------------ o pedido

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pedido {
    #[serde(default = "get")]
    pub metodo: String,
    pub url: String,
    #[serde(default)]
    pub cabecalhos: BTreeMap<String, String>,
    /// Valores escalares (numero e booleano viram texto).
    #[serde(default)]
    pub query: BTreeMap<String, Value>,
    #[serde(default)]
    pub corpo: Option<Corpo>,
    #[serde(default)]
    pub credencial: Option<String>,
    #[serde(default)]
    pub teto_ms: Option<u64>,
    #[serde(default)]
    pub teto_bytes: Option<usize>,
    /// Caminho JSON do array de itens na resposta (`data.results`); ausente, a resposta.
    #[serde(default)]
    pub itens: Option<String>,
    #[serde(default)]
    pub paginacao: Option<Paginacao>,
    #[serde(default)]
    pub lote: Option<Lote>,
    /// Status >= 400 vira item (`{"status", "corpo"}`) em vez de falhar o passo.
    #[serde(default)]
    pub aceitar_erro: bool,
    /// `corpo` (padrao) ou `completa` (um item com status, cabecalhos e corpo).
    #[serde(default)]
    pub resposta: Option<String>,
}

fn get() -> String {
    "GET".into()
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpo {
    #[serde(default)]
    pub json: Option<Value>,
    #[serde(default)]
    pub form: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub texto: Option<String>,
    /// O Content-Type do `texto` (padrao `text/plain; charset=utf-8`).
    #[serde(default)]
    pub tipo: Option<String>,
}

/// Exatamente um modo: `cursor` (caminho do cursor na resposta, que volta no `parametro`
/// da query), `proximo` (caminho da URL da proxima pagina na resposta) ou `link` (o
/// cabecalho `Link` com `rel="next"`, RFC 8288).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paginacao {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub parametro: Option<String>,
    #[serde(default)]
    pub proximo: Option<String>,
    #[serde(default)]
    pub link: bool,
    #[serde(default = "max_paginas_padrao")]
    pub max_paginas: usize,
}

fn max_paginas_padrao() -> usize {
    MAX_PAGINAS_PADRAO
}

/// `itens` (a lista, em geral `{{passo}}`) vai em pedidos de `tamanho` itens cada, com
/// `pausa_ms` entre um e outro. O corpo de cada pedido e o lote (array JSON), ou o objeto
/// de `corpo.json` com o lote no campo `campo` (padrao `itens`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lote {
    pub itens: Value,
    pub tamanho: usize,
    #[serde(default)]
    pub pausa_ms: u64,
    #[serde(default)]
    pub campo: Option<String>,
}

/// Toda string de um valor JSON, com o caminho, para as conferencias de forma.
fn textos<'a>(v: &'a Value, saida: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => saida.push(s),
        Value::Array(a) => a.iter().for_each(|x| textos(x, saida)),
        Value::Object(o) => o.iter().for_each(|(k, x)| {
            saida.push(k);
            textos(x, saida)
        }),
        _ => {}
    }
}

fn tem_expressao(t: &str) -> bool {
    t.contains("{{")
}

/// A conferencia que vale no fluxo E na ferramenta: segredo pela forma em qualquer texto,
/// nome de segredo em cabecalho, query e formulario, URL com usuario.
fn conferir_segredos(v: &Value) -> Result<(), String> {
    let mut ts = Vec::new();
    textos(v, &mut ts);
    if ts
        .iter()
        .any(|t| phxclaw_types::segredo::texto_tem_credencial(t))
    {
        return Err(
            "o pedido http traz credencial pela forma (Bearer, Basic, chave de \
provedor, JWT, URL com senha): segredo entra so por 'credencial', pelo broker"
                .into(),
        );
    }
    for campo in ["cabecalhos", "query"] {
        if let Some(Value::Object(o)) = v.get(campo) {
            for k in o.keys() {
                if phxclaw_types::segredo::nome_de_segredo(k) {
                    return Err(format!(
                        "{campo}.{k}: nome de segredo; a credencial entra por 'credencial' \
(declarada em {ARQUIVO}, valor no broker)"
                    ));
                }
            }
        }
    }
    if let Some(Value::Object(o)) = v.get("corpo").and_then(|c| c.get("form")) {
        for k in o.keys() {
            if phxclaw_types::segredo::nome_de_segredo(k) {
                return Err(format!(
                    "corpo.form.{k}: nome de segredo; a credencial entra por 'credencial'"
                ));
            }
        }
    }
    if let Some(Value::String(u)) = v.get("url")
        && !tem_expressao(u)
        && let Ok(url) = Url::parse(u)
        && (!url.username().is_empty() || url.password().is_some())
    {
        return Err("url com usuario/senha: a credencial entra por 'credencial'".into());
    }
    Ok(())
}

/// A conferencia na LEITURA do fluxo: o que nao depende de `{{...}}` ja se recusa aqui,
/// antes de qualquer execucao. O resto a ferramenta confere com os valores resolvidos.
pub fn validar_no_fluxo(v: &Value) -> Result<(), String> {
    if !v.is_object() {
        return Err("'http' precisa ser um objeto (o pedido), nao uma expressao".into());
    }
    conferir_segredos(v)?;
    let mut ts = Vec::new();
    textos(v, &mut ts);
    match serde_json::from_value::<Pedido>(v.clone()) {
        Ok(p) => validar(&p, true),
        // Campo numerico vindo de `{{var.x}}` nao desserializa antes da execucao.
        Err(_) if ts.iter().any(|t| tem_expressao(t)) => Ok(()),
        Err(e) => Err(format!("pedido http invalido: {e}")),
    }
}

/// O pedido como a ferramenta o recebe. `no_fluxo`: a URL ainda pode ter `{{...}}`.
fn validar(p: &Pedido, no_fluxo: bool) -> Result<(), String> {
    let m = p.metodo.to_ascii_uppercase();
    if !METODOS.contains(&m.as_str()) && !(no_fluxo && tem_expressao(&p.metodo)) {
        return Err(format!(
            "metodo {:?} (use {})",
            p.metodo,
            METODOS.join(", ")
        ));
    }
    if !(no_fluxo && tem_expressao(&p.url)) {
        let u = Url::parse(&p.url).map_err(|e| format!("url {:?}: {e}", p.url))?;
        if u.scheme() != "http" && u.scheme() != "https" {
            return Err(format!("url {:?}: so http e https", p.url));
        }
    }
    for (k, v) in &p.cabecalhos {
        reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| format!("cabecalho invalido: {k:?}"))?;
        if k.eq_ignore_ascii_case("host") || k.eq_ignore_ascii_case("content-length") {
            return Err(format!("cabecalho {k} e do cliente HTTP, nao do pedido"));
        }
        if !tem_expressao(v) {
            reqwest::header::HeaderValue::from_str(v)
                .map_err(|_| format!("valor invalido no cabecalho {k}"))?;
        }
    }
    for (k, v) in &p.query {
        if v.is_object() || v.is_array() {
            return Err(format!("query.{k}: so valor escalar"));
        }
    }
    if let Some(n) = &p.credencial {
        nome_valido(n)?;
    }
    if p.teto_ms.is_some_and(|t| t == 0 || t > TETO_MS_MAX) {
        return Err(format!("teto_ms de 1 a {TETO_MS_MAX}"));
    }
    if p.teto_bytes.is_some_and(|t| t == 0 || t > TETO_BYTES_MAX) {
        return Err(format!("teto_bytes de 1 a {TETO_BYTES_MAX}"));
    }
    if let Some(r) = &p.resposta
        && r != "corpo"
        && r != "completa"
    {
        return Err(format!("resposta {r:?} (use corpo ou completa)"));
    }
    let com_corpo = matches!(m.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
    if let Some(c) = &p.corpo {
        let modos = usize::from(c.json.is_some())
            + usize::from(c.form.is_some())
            + usize::from(c.texto.is_some());
        if modos != 1 {
            return Err("corpo: diga 'json', 'form' OU 'texto' (exatamente um)".into());
        }
        if !com_corpo {
            return Err(format!("{m} nao leva corpo"));
        }
        if c.tipo.is_some() && c.texto.is_none() {
            return Err("corpo.tipo so com corpo.texto".into());
        }
        if let Some(f) = &c.form
            && f.values().any(|v| v.is_object() || v.is_array())
        {
            return Err("corpo.form: so valor escalar".into());
        }
    }
    if let Some(pg) = &p.paginacao {
        let modos = usize::from(pg.cursor.is_some())
            + usize::from(pg.proximo.is_some())
            + usize::from(pg.link);
        if modos != 1 {
            return Err(
                "paginacao: diga 'cursor', 'proximo' OU 'link' (exatamente um)".to_string(),
            );
        }
        if pg.cursor.is_some() != pg.parametro.is_some() {
            return Err("paginacao por cursor pede 'cursor' e 'parametro'".into());
        }
        if pg.max_paginas == 0 || pg.max_paginas > MAX_PAGINAS {
            return Err(format!("paginacao.max_paginas de 1 a {MAX_PAGINAS}"));
        }
    }
    if let Some(l) = &p.lote {
        if p.paginacao.is_some() {
            return Err("lote e paginacao nao se combinam".into());
        }
        if !matches!(m.as_str(), "POST" | "PUT" | "PATCH")
            && !(no_fluxo && tem_expressao(&p.metodo))
        {
            return Err("lote manda os itens no corpo: so POST, PUT ou PATCH".into());
        }
        if l.tamanho == 0 {
            return Err("lote.tamanho comeca em 1".into());
        }
        if l.pausa_ms > MAX_PAUSA_MS {
            return Err(format!("lote.pausa_ms ate {MAX_PAUSA_MS}"));
        }
        match &p.corpo {
            None => {}
            Some(Corpo {
                json: Some(Value::Object(_)),
                ..
            }) => {}
            Some(_) => {
                return Err("com lote, o corpo e o lote ou um corpo.json objeto".into());
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ credencial aplicada

enum Autenticacao {
    /// Bearer fixo e OAuth2: o MESMO `AuthorizationSource` dos MCP remotos.
    Fonte(AutorizacaoMcp),
    Basico {
        usuario: String,
        cred: crate::canais::http::Credencial,
    },
    Cabecalho {
        nome: String,
        cred: crate::canais::http::Credencial,
    },
}

struct Credenciada {
    nome: String,
    origens: BTreeSet<String>,
    aut: Autenticacao,
}

impl Credenciada {
    fn resolver(raiz: &Path, cfg: &ConfigHttp, nome: &str) -> Result<Self, String> {
        let d = cfg
            .credenciais
            .get(nome)
            .ok_or_else(|| format!("credencial {nome} nao esta declarada em {ARQUIVO}"))?;
        let falta = || {
            format!(
                "credencial {nome} sem segredo guardado: rode `phxclaw credencial guardar {nome}`"
            )
        };
        let b = broker(raiz, false)?.ok_or_else(falta)?;
        let alvo = d.alvo(nome);
        let aut = match d.tipo {
            TipoCredencial::Bearer | TipoCredencial::Oauth2 => Autenticacao::Fonte(
                AutorizacaoMcp::do_broker(b, &alvo, d.oauth2.as_ref())
                    .map_err(|e| e.unwrap_or_else(falta))?,
            ),
            TipoCredencial::Basico | TipoCredencial::Cabecalho => {
                let cred = oauth::credencial(&b, &alvo, Tipo::Bearer)?.ok_or_else(falta)?;
                if d.tipo == TipoCredencial::Basico {
                    Autenticacao::Basico {
                        usuario: d.usuario.clone().unwrap_or_default(),
                        cred,
                    }
                } else {
                    Autenticacao::Cabecalho {
                        nome: d.cabecalho.clone().unwrap_or_default(),
                        cred,
                    }
                }
            }
        };
        Ok(Self {
            nome: nome.to_string(),
            origens: d
                .origens
                .iter()
                .filter_map(|o| origem_canonica(o).ok())
                .collect(),
            aut,
        })
    }

    fn alcanca(&self, url: &Url) -> bool {
        self.origens.contains(&url.origin().ascii_serialization())
    }

    /// O cabecalho e os segredos que ele carrega (para a tarja de tudo o que sair).
    async fn cabecalho(&self) -> Result<(String, String, Vec<SecretValue>), String> {
        Ok(match &self.aut {
            Autenticacao::Fonte(f) => {
                let v = f.authorization().await?;
                let token = v.strip_prefix("Bearer ").unwrap_or(&v).to_string();
                (
                    "Authorization".into(),
                    v.clone(),
                    vec![SecretValue::new(v), SecretValue::new(token)],
                )
            }
            Autenticacao::Basico { usuario, cred } => cred.com("bearer", |senha| {
                use base64::Engine;
                let b =
                    base64::engine::general_purpose::STANDARD.encode(format!("{usuario}:{senha}"));
                Ok((
                    "Authorization".to_string(),
                    format!("Basic {b}"),
                    vec![SecretValue::new(b), SecretValue::new(senha.to_string())],
                ))
            })?,
            Autenticacao::Cabecalho { nome, cred } => cred.com("bearer", |v| {
                Ok((
                    nome.clone(),
                    v.to_string(),
                    vec![SecretValue::new(v.to_string())],
                ))
            })?,
        })
    }

    /// Depois de um 401: no OAuth2 o acesso deixa de valer e o proximo pedido renova.
    async fn invalidar(&self) -> bool {
        match &self.aut {
            Autenticacao::Fonte(f) => f.invalidar().await,
            _ => false,
        }
    }

    fn limpar(&self, t: String) -> String {
        match &self.aut {
            Autenticacao::Fonte(f) => f.limpar(t),
            Autenticacao::Basico { cred, .. } | Autenticacao::Cabecalho { cred, .. } => {
                match cred.abrir("bearer") {
                    Ok(c) => c.limpar::<()>(Err(t)).unwrap_err(),
                    Err(_) => t,
                }
            }
        }
    }
}

// ------------------------------------------------------------------ a execucao

struct Execucao<'a> {
    p: &'a Pedido,
    metodo: String,
    politica: BrowserPolicy,
    cred: Option<Credenciada>,
    /// Todo segredo que entrou num pedido: a tarja de tudo o que sai.
    conhecidos: std::sync::Mutex<Vec<SecretValue>>,
    teto_bytes: usize,
    gastos: std::sync::atomic::AtomicUsize,
}

impl Execucao<'_> {
    fn limpar(&self, t: String) -> String {
        let t = match &self.cred {
            Some(c) => c.limpar(t),
            None => t,
        };
        let conhecidos = self.conhecidos.lock().unwrap_or_else(|p| p.into_inner());
        scrub_secret_like(&scrub_text(&t, &conhecidos))
    }

    /// Um pedido (com os redirecionamentos), conferido e tarjado. `corpo` ja resolvido.
    async fn enviar(&self, url: &Url, corpo: Option<&CorpoPronto>) -> Result<HttpResult, String> {
        let mut spec = HttpRequestSpec::get(url.to_string());
        spec.method = self.metodo.clone();
        spec.timeout_ms = self.p.teto_ms.unwrap_or(TETO_MS_PADRAO);
        spec.connect_timeout_ms = spec.timeout_ms.min(10_000);
        spec.max_redirects = MAX_REDIRECIONAMENTOS;
        spec.user_agent = Some(concat!("PhxClaw/", env!("CARGO_PKG_VERSION")).into());
        for (k, v) in &self.p.cabecalhos {
            spec.headers.insert(k.clone(), v.clone());
        }
        match corpo {
            Some(CorpoPronto::Json(v)) => spec.json = Some(v.clone()),
            Some(CorpoPronto::Form(f)) => spec.form = f.clone(),
            Some(CorpoPronto::Texto(t, tipo)) => {
                use base64::Engine;
                spec.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(t));
                spec.headers.insert("Content-Type".into(), tipo.clone());
            }
            None => {}
        }
        let mut repetiu = false;
        loop {
            if let Some(c) = &self.cred {
                if !c.alcanca(url) {
                    return Err(format!(
                        "a credencial {} nao vale para {} (origens declaradas: {})",
                        c.nome,
                        url.origin().ascii_serialization(),
                        c.origens.iter().cloned().collect::<Vec<_>>().join(", ")
                    ));
                }
                let (nome, valor, segredos) = c.cabecalho().await?;
                self.conhecidos
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .extend(segredos);
                spec.headers.retain(|k, _| !k.eq_ignore_ascii_case(&nome));
                spec.headers.insert(nome, valor);
            }
            let ja = self.gastos.load(std::sync::atomic::Ordering::SeqCst);
            let resta = self.teto_bytes.saturating_sub(ja);
            if resta == 0 {
                return Err(format!(
                    "o no passou do teto de {} bytes (teto_bytes)",
                    self.teto_bytes
                ));
            }
            let politica = &self.politica;
            let r = request_checked(&spec, |u| async move {
                let (_, enderecos) = politica
                    .check_url_resolved(u.as_str())
                    .await
                    .map_err(|e| EgressError::Refused(e.to_string()))?;
                let resolve = match (u.host_str(), enderecos.is_empty()) {
                    (Some(h), false) => vec![(h.to_string(), enderecos)],
                    _ => vec![],
                };
                Ok(HttpOptions {
                    max_body_bytes: Some(resta),
                    resolve,
                })
            })
            .await;
            let r = match r {
                Ok(r) => r,
                Err(EgressError::Http(phxclaw_http_client::HttpClientError::BodyTooLarge(_))) => {
                    return Err(format!(
                        "a resposta de {} passou do teto de {} bytes (teto_bytes)",
                        url_sem_query(url),
                        self.teto_bytes
                    ));
                }
                Err(e) => {
                    return Err(self.limpar(format!(
                        "{} {}: {e}",
                        self.metodo,
                        url_sem_query(url)
                    )));
                }
            };
            let tamanho = r.bytes().map(|b| b.len()).unwrap_or(0);
            self.gastos
                .fetch_add(tamanho, std::sync::atomic::Ordering::SeqCst);
            if r.status == 401
                && !repetiu
                && let Some(c) = &self.cred
                && c.invalidar().await
            {
                repetiu = true;
                continue;
            }
            if r.status >= 400 && !self.p.aceitar_erro {
                let corpo = r.text().unwrap_or_default();
                let trecho: String = corpo.chars().take(TRECHO_DO_ERRO).collect();
                return Err(self.limpar(format!(
                    "HTTP {} em {} {}: {}",
                    r.status,
                    self.metodo,
                    url_sem_query(url),
                    trecho.trim()
                )));
            }
            return Ok(r);
        }
    }

    /// A resposta em itens, e o JSON dela (para a paginacao ler o cursor ou o link).
    fn itens(&self, r: &HttpResult) -> Result<(Vec<Value>, Option<Value>), String> {
        let bytes = r.bytes().map_err(|e| e.to_string())?;
        let texto = String::from_utf8_lossy(&bytes).into_owned();
        let parece_json = r
            .content_type
            .as_deref()
            .is_some_and(|c| c.to_ascii_lowercase().contains("json"))
            || matches!(texto.trim_start().chars().next(), Some('[' | '{'));
        let json: Option<Value> = if parece_json {
            serde_json::from_str(&texto).ok()
        } else {
            None
        };
        let corpo = match &json {
            Some(j) => j.clone(),
            None if texto.is_empty() => Value::Null,
            None => Value::String(texto),
        };
        if self.p.resposta.as_deref() == Some("completa") || r.status >= 400 {
            // Os cabecalhos de resposta que carregam sessao nao viram item: o item vai para
            // o relatorio e o `task.json`.
            let cabecalhos: BTreeMap<&String, &Vec<String>> = r
                .headers
                .iter()
                .filter(|(k, _)| !phxclaw_types::segredo::nome_de_segredo(k))
                .collect();
            return Ok((
                vec![json!({"status": r.status, "cabecalhos": cabecalhos, "corpo": corpo})],
                json,
            ));
        }
        let alvo = match (&self.p.itens, &json) {
            (Some(c), Some(j)) => crate::fluxos::pelo_caminho(j, c)
                .ok_or_else(|| format!("o caminho '{c}' nao existe na resposta"))?,
            (Some(c), None) => {
                return Err(format!("itens '{c}': a resposta nao e JSON"));
            }
            (None, _) => corpo,
        };
        let itens = match alvo {
            Value::Array(a) => a,
            Value::Null => vec![],
            outro => vec![outro],
        };
        Ok((itens, json))
    }
}

enum CorpoPronto {
    Json(Value),
    Form(Vec<(String, String)>),
    Texto(String, String),
}

fn escalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        outro => outro.to_string(),
    }
}

fn url_sem_query(u: &Url) -> String {
    let mut u = u.clone();
    u.set_query(None);
    u.set_fragment(None);
    let _ = u.set_username("");
    let _ = u.set_password(None);
    u.to_string()
}

/// `Link: <https://x/?p=2>; rel="next", <...>; rel="last"` -> a URL do `next`.
pub fn link_next(cabecalho: &str) -> Option<String> {
    let mut resto = cabecalho;
    while let Some(i) = resto.find('<') {
        let depois = &resto[i + 1..];
        let j = depois.find('>')?;
        let url = &depois[..j];
        let params = &depois[j + 1..];
        let fim = params.find('<').unwrap_or(params.len());
        let e_next = params[..fim].split(';').any(|p| {
            let p = p.trim().trim_end_matches(',').trim();
            p.split_once('=').is_some_and(|(k, v)| {
                k.trim().eq_ignore_ascii_case("rel")
                    && v.trim()
                        .trim_matches('"')
                        .split_whitespace()
                        .any(|r| r.eq_ignore_ascii_case("next"))
            })
        });
        if e_next {
            return Some(url.trim().to_string());
        }
        resto = &params[fim..];
    }
    None
}

/// O pedido inteiro: paginas ou lotes, e os itens de todas as respostas.
pub async fn executar(raiz_do_agente: &Path, args: &Value) -> Result<Vec<Value>, String> {
    conferir_segredos(args)?;
    let p: Pedido =
        serde_json::from_value(args.clone()).map_err(|e| format!("pedido http invalido: {e}"))?;
    validar(&p, false)?;
    let cfg = ConfigHttp::carregar(raiz_do_agente)?;
    let cred = match &p.credencial {
        Some(n) => Some(Credenciada::resolver(raiz_do_agente, &cfg, n)?),
        None => None,
    };
    let ex = Execucao {
        p: &p,
        metodo: p.metodo.to_ascii_uppercase(),
        politica: cfg.politica(),
        cred,
        conhecidos: Default::default(),
        teto_bytes: p.teto_bytes.unwrap_or(TETO_BYTES_PADRAO),
        gastos: Default::default(),
    };
    let mut base = Url::parse(&p.url).map_err(|e| format!("url: {e}"))?;
    if !p.query.is_empty() {
        let mut q = base.query_pairs_mut();
        for (k, v) in &p.query {
            q.append_pair(k, &escalar(v));
        }
    }
    let corpo = p.corpo.as_ref().map(|c| match c {
        Corpo { json: Some(j), .. } => CorpoPronto::Json(j.clone()),
        Corpo { form: Some(f), .. } => {
            CorpoPronto::Form(f.iter().map(|(k, v)| (k.clone(), escalar(v))).collect())
        }
        Corpo { texto, tipo, .. } => CorpoPronto::Texto(
            texto.clone().unwrap_or_default(),
            tipo.clone()
                .unwrap_or_else(|| "text/plain; charset=utf-8".into()),
        ),
    });
    let mut itens = Vec::new();
    let juntar = |novos: Vec<Value>, itens: &mut Vec<Value>| -> Result<(), String> {
        if itens.len() + novos.len() > MAX_ITENS {
            return Err(format!(
                "a resposta passou do teto de {MAX_ITENS} itens do no http"
            ));
        }
        itens.extend(novos);
        Ok(())
    };
    if let Some(l) = &p.lote {
        let lista = match &l.itens {
            Value::Array(a) => a.clone(),
            Value::Null => vec![],
            outro => vec![outro.clone()],
        };
        let lotes: Vec<&[Value]> = lista.chunks(l.tamanho).collect();
        if lotes.len() > MAX_LOTES {
            return Err(format!(
                "{} lotes passam do teto de {MAX_LOTES} pedidos",
                lotes.len()
            ));
        }
        for (i, pedaco) in lotes.into_iter().enumerate() {
            if i > 0 && l.pausa_ms > 0 {
                tokio::time::sleep(Duration::from_millis(l.pausa_ms)).await;
            }
            let lote = Value::Array(pedaco.to_vec());
            let corpo = match &corpo {
                Some(CorpoPronto::Json(Value::Object(o))) => {
                    let mut o = o.clone();
                    o.insert(l.campo.clone().unwrap_or_else(|| "itens".into()), lote);
                    CorpoPronto::Json(Value::Object(o))
                }
                _ => CorpoPronto::Json(lote),
            };
            let r = ex.enviar(&base, Some(&corpo)).await?;
            juntar(ex.itens(&r)?.0, &mut itens)?;
        }
        return Ok(itens);
    }
    let mut url = base.clone();
    let mut visitadas = BTreeSet::new();
    for pagina in 1.. {
        visitadas.insert(url.to_string());
        let r = ex.enviar(&url, corpo.as_ref()).await?;
        let (novos, json) = ex.itens(&r)?;
        juntar(novos, &mut itens)?;
        let Some(pg) = &p.paginacao else { break };
        if pagina >= pg.max_paginas {
            break;
        }
        let atual = Url::parse(&r.final_url).unwrap_or_else(|_| url.clone());
        let proximo: Option<Url> = if let (Some(campo), Some(param)) = (&pg.cursor, &pg.parametro) {
            json.as_ref()
                .and_then(|j| crate::fluxos::pelo_caminho(j, campo))
                .map(|c| escalar(&c))
                .filter(|c| !c.is_empty())
                .map(|c| {
                    let pares: Vec<(String, String)> = base
                        .query_pairs()
                        .filter(|(k, _)| k != param.as_str())
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect();
                    let mut u = base.clone();
                    u.set_query(None);
                    {
                        let mut q = u.query_pairs_mut();
                        for (k, v) in &pares {
                            q.append_pair(k, v);
                        }
                        q.append_pair(param, &c);
                    }
                    u
                })
        } else if let Some(campo) = &pg.proximo {
            json.as_ref()
                .and_then(|j| crate::fluxos::pelo_caminho(j, campo))
                .and_then(|v| v.as_str().map(str::to_string))
                .filter(|s| !s.is_empty())
                .and_then(|s| atual.join(&s).ok())
        } else {
            r.headers
                .get("link")
                .and_then(|v| v.iter().find_map(|l| link_next(l)))
                .and_then(|s| atual.join(&s).ok())
        };
        match proximo {
            // A mesma pagina de novo e um laco do servidor, nao mais dados.
            Some(u) if !visitadas.contains(u.as_str()) => url = u,
            _ => break,
        }
    }
    // O corpo POST de uma pagina vai igual em todas: e o que a API paginada por POST
    // (busca com cursor) espera.
    Ok(itens)
}

// ------------------------------------------------------------------ a ferramenta

/// A ferramenta `http_request`, sobre a raiz do agente (onde moram `http.json` e o broker
/// das credenciais). A configuracao e lida a cada chamada: liberar um destino ou declarar
/// uma credencial vale sem reiniciar.
pub struct HttpTool {
    pub raiz: PathBuf,
}

impl Tool for HttpTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: FERRAMENTA.into(),
            description: "Generic HTTP request (GET/POST/PUT/PATCH/DELETE/HEAD) with headers, \
query, JSON/form/text body, timeout and size limit. Authentication only by the NAME of a \
credential declared by the operator ('credencial'); never put a secret in headers, query or \
body. Supports pagination (cursor field, next URL field, or Link rel=next) and batching. \
Returns the response items as a JSON array. Internal network addresses are refused unless \
the operator allowed them."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "metodo": {"type": "string", "enum": METODOS},
                    "url": {"type": "string"},
                    "cabecalhos": {"type": "object"},
                    "query": {"type": "object"},
                    "corpo": {"type": "object"},
                    "credencial": {"type": "string"},
                    "teto_ms": {"type": "integer"},
                    "teto_bytes": {"type": "integer"},
                    "itens": {"type": "string"},
                    "paginacao": {"type": "object"},
                    "lote": {"type": "object"},
                    "aceitar_erro": {"type": "boolean"},
                    "resposta": {"type": "string", "enum": ["corpo", "completa"]}
                },
                "required": ["url"]
            }),
        }
    }

    fn capability(&self) -> &'static str {
        CAPACIDADE
    }

    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let itens = executar(&self.raiz, &args)
                .await
                .map_err(ToolError::Failed)?;
            Ok(ToolOutput::text(Value::Array(itens).to_string()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_next_le_a_rfc_8288() {
        assert_eq!(
            link_next(r#"<https://a/x?p=2>; rel="next", <https://a/x?p=9>; rel="last""#).as_deref(),
            Some("https://a/x?p=2")
        );
        assert_eq!(
            link_next(r#"<https://a/x?p=1>; rel="prev", <https://a/x?p=3>; rel="next""#).as_deref(),
            Some("https://a/x?p=3")
        );
        assert_eq!(link_next(r#"<https://a/x?p=9>; rel="last""#), None);
    }

    #[test]
    fn segredo_no_pedido_e_recusado_na_leitura() {
        for v in [
            json!({"url": "https://a/x", "cabecalhos": {"Authorization": "abc"}}),
            json!({"url": "https://a/x", "cabecalhos": {"X-Api-Key": "abc"}}),
            json!({"url": "https://a/x", "query": {"api_key": "abc"}}),
            json!({"metodo": "POST", "url": "https://a/x", "corpo": {"form": {"password": "x"}}}),
            json!({"url": "https://usuario:senha@a/x"}),
            json!({"metodo": "POST", "url": "https://a/x",
                   "corpo": {"json": {"k": "ghp_0123456789abcdefghij"}}}),
        ] {
            assert!(validar_no_fluxo(&v).is_err(), "{v}");
        }
        assert!(validar_no_fluxo(&json!({"url": "{{a.url}}", "credencial": "x"})).is_ok());
        assert!(validar_no_fluxo(&json!({"url": "https://a/x", "teto_ms": "{{var.t}}"})).is_ok());
        assert!(validar_no_fluxo(&json!({"url": "https://a/x", "metodo": "TRACE"})).is_err());
    }
}
