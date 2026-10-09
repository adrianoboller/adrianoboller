//! Usuarios, projetos e papeis da API: o RBAC do n8n (owner, admin, member) mais o leitor,
//! decidido pelo dono em 09/10/2026.
//!
//! Decisoes que valem saber:
//! - **Guarda pedida, nao imposta.** Sem `<pasta>/usuarios.json` (ou com a lista vazia) o
//!   portao devolve o pedido intacto e o Bearer unico do `api.token` vale exatamente como
//!   antes. O teste que trava isso e o do comportamento VELHO (`sem_usuarios_nada_muda`).
//! - **O `api.token` continua valendo com usuarios, como dono.** Quem le aquele arquivo 0600
//!   ja roda `phxclaw usuario criar` na mesma maquina: recusa-lo nao tiraria poder de
//!   ninguem, e quebraria a ponte, os gatilhos e o SDK que o usam.
//! - **Token de usuario so como hash**: SHA-256(sal || token), sal de 16 bytes por usuario.
//!   O token tem 256 bits do CSPRNG, entao PBKDF2 nao compraria nada -- estiramento existe
//!   para segredo de pouca entropia (senha), e aqui custaria CPU em todo pedido.
//! - **A matriz papel x rota e UMA tabela (`MATRIZ`) lida por UM portao (`portao`)**, o
//!   middleware do `api::router`. Rota fora da tabela e so do dono: esquecer de classificar
//!   uma rota nova FECHA, nao abre.
//! - **O projeto sai de um lugar so por tipo de rota**: da tarefa gravada (rota com `{id}`)
//!   ou do cabecalho `X-PhxClaw-Projeto`. Nunca do corpo: o `criar` recusa corpo com
//!   `projeto`, porque um campo que o portao nao le e a porta dos fundos.
//! - **Quem esconde o token fora do cabecalho** (o websocket do terminal do IDE, que recebe
//!   o token na primeira mensagem) passa pelo portao sem identidade e chama `conferir_rota`
//!   la dentro -- a MESMA `decidir`, com a mesma linha da matriz.

use crate::api::ApiState;
use axum::Json;
use axum::extract::{FromRequestParts, MatchedPath, RawPathParams, Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Arquivo dos usuarios, na pasta do agente (ao lado do `api.token`).
pub const ARQUIVO: &str = "usuarios.json";
/// O UNICO lugar de onde o portao le o projeto de um pedido que nao nomeia tarefa.
pub const CABECALHO_PROJETO: &str = "x-phxclaw-projeto";
/// Prefixo do token de usuario: o detector de segredo do repositorio e dos logs o reconhece.
pub const PREFIXO_TOKEN: &str = "phxu_";
/// Subida maxima pela cadeia de tarefas-mae ao achar o projeto de um subagente.
const PROFUNDIDADE_DA_MAE: usize = 8;

/// Papel do usuario, em ordem de poder: comparar papeis e comparar a posicao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Papel {
    #[serde(rename = "leitor")]
    Leitor,
    #[serde(rename = "member")]
    Membro,
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "owner")]
    Dono,
}

impl Papel {
    pub fn do_texto(t: &str) -> Option<Self> {
        match t.trim().to_ascii_lowercase().as_str() {
            "owner" | "dono" => Some(Self::Dono),
            "admin" => Some(Self::Admin),
            "member" | "membro" => Some(Self::Membro),
            "leitor" | "viewer" => Some(Self::Leitor),
            _ => None,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Dono => "owner",
            Self::Admin => "admin",
            Self::Membro => "member",
            Self::Leitor => "leitor",
        }
    }

    /// Dono e admin enxergam a instancia inteira; membro e leitor, so os projetos deles.
    fn global(self) -> bool {
        self >= Self::Admin
    }
}

/// De onde o portao tira o recurso que o papel precisa alcancar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escopo {
    /// Sem token: a tela, o site publicado, o canvas (sandbox) e o `/health`.
    Publico,
    /// A instancia inteira (configuracao, IDE, agenda, metricas): so o papel conta.
    Instancia,
    /// Colecao de tarefas: o projeto vem do cabecalho `X-PhxClaw-Projeto`.
    Projeto,
    /// Rota com `{id}` de tarefa: o projeto e o da tarefa gravada.
    Tarefa,
    /// O token chega DEPOIS do pedido (primeira mensagem do websocket): o portao deixa
    /// passar e a rota chama `conferir_rota` com o token na mao.
    TokenNaMensagem,
}

/// Uma linha da matriz. `metodo` "*" vale para todos; HEAD conta como GET.
#[derive(Debug, Clone, Copy)]
pub struct Regra {
    pub metodo: &'static str,
    pub rota: &'static str,
    pub minimo: Papel,
    pub escopo: Escopo,
}

const fn r(metodo: &'static str, rota: &'static str, minimo: Papel, escopo: Escopo) -> Regra {
    Regra {
        metodo,
        rota,
        minimo,
        escopo,
    }
}

use Escopo::{Instancia as I, Projeto as P, Publico as X, Tarefa as T};
use Papel::{Admin as AD, Dono as DO, Leitor as LE, Membro as ME};

/// A matriz papel x rota, escrita UMA vez. A rota e o molde do axum (`MatchedPath`).
///
/// Por que cada faixa: ler tarefa e metrica e do leitor; mexer em tarefa do proprio projeto
/// e do membro; o que e da instancia inteira (agenda, configuracao lida, IDE, MCP) e do
/// admin -- menos os fluxos, que a tela usa no dia a dia (ver a linha deles); o que da
/// shell no hospedeiro ou reescreve a configuracao e do dono.
pub const MATRIZ: &[Regra] = &[
    r("GET", "/health", LE, X),
    r("GET", "/sites/{id}/{*path}", LE, X),
    r("GET", "/canvas/{id}/{nome}", LE, X),
    r("GET", "/canvas/{id}/{nome}/widget", LE, X),
    r("GET", "/", LE, X),
    r("GET", "/index.html", LE, X),
    r("GET", "/manifest.webmanifest", LE, X),
    r("GET", "/sw.js", LE, X),
    r("GET", "/assets/{*resto}", LE, X),
    r("GET", "/metrics", LE, I),
    r("GET", "/v1/tasks", LE, P),
    r("POST", "/v1/tasks", ME, P),
    r("GET", "/v1/tasks/{id}", LE, T),
    r("GET", "/v1/tasks/{id}/artifacts/{*path}", LE, T),
    r("POST", "/v1/tasks/{id}/plan", ME, T),
    r("POST", "/v1/tasks/{id}/approve", ME, T),
    r("POST", "/v1/tasks/{id}/cancel", ME, T),
    r("POST", "/v1/tasks/{id}/answer", ME, T),
    r("GET", "/v1/schedules", AD, I),
    r("POST", "/v1/schedules", AD, I),
    r("GET", "/v1/config", AD, I),
    r("PUT", "/v1/config", DO, I),
    r("GET", "/v1/config/perfis", AD, I),
    r("PUT", "/v1/config/perfis", DO, I),
    r("GET", "/v1/config/sincronizar", AD, I),
    r("PUT", "/v1/config/sincronizar", DO, I),
    // Os fluxos sao arquivos da pasta do projeto da INSTANCIA, nao de um projeto do RBAC:
    // o nome vem na query ou no corpo e o portao nao precisa dele, porque o escopo e a
    // instancia. Ler e validar e do leitor; gravar e rodar, do membro (orquestrador, 09/10).
    r("GET", "/v1/fluxos", LE, I),
    r("GET", "/v1/fluxos/arquivo", LE, I),
    r("POST", "/v1/fluxos/validar", LE, I),
    r("PUT", "/v1/fluxos/arquivo", ME, I),
    r("POST", "/v1/fluxos/rodar", ME, I),
    r(
        "GET",
        crate::ide::ROTA_TERMINAL,
        DO,
        Escopo::TokenNaMensagem,
    ),
    r("GET", "/v1/ide/simbolos", AD, I),
    r("GET", "/v1/ide/arquivo", AD, I),
    r("POST", "/v1/ide/completar", AD, I),
    r("GET", "/v1/ide/testes", AD, I),
    r("POST", "/v1/ide/testes/rodar", AD, I),
    r("GET", "/v1/plugins/catalogo", AD, I),
    r("POST", "/v1/plugins/instalar", DO, I),
    r("POST", crate::tunel::ROTA_TERMINAL, DO, I),
    r("POST", crate::tunel::ROTA_LSP, DO, I),
    r("*", "/mcp", AD, I),
];

/// Rota que ninguem classificou: so o dono. Fechar e o erro barato.
const SEM_LINHA: Regra = r("*", "", DO, I);

/// A linha da matriz para o pedido (ou `SEM_LINHA`).
pub fn regra_de(metodo: &Method, rota: Option<&str>) -> Regra {
    let m = if metodo == Method::HEAD {
        "GET"
    } else {
        metodo.as_str()
    };
    rota.and_then(|rota| {
        MATRIZ
            .iter()
            .find(|x| x.rota == rota && (x.metodo == "*" || x.metodo == m))
    })
    .copied()
    .unwrap_or(SEM_LINHA)
}

/// O que o arquivo guarda de cada usuario. O token nunca: so o sal e o hash.
#[derive(Clone, Serialize, Deserialize)]
pub struct Usuario {
    pub nome: String,
    pub papel: Papel,
    #[serde(default)]
    pub projetos: Vec<String>,
    sal: String,
    hash: String,
    pub criado_em: DateTime<Utc>,
}

// A mao, e nao derivado: `{:?}` de um usuario num log ou num erro nao pode levar o hash.
impl std::fmt::Debug for Usuario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Usuario")
            .field("nome", &self.nome)
            .field("papel", &self.papel)
            .field("projetos", &self.projetos)
            .field("hash", &"<redigido>")
            .finish()
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Arquivo {
    #[serde(default)]
    usuarios: Vec<Usuario>,
}

/// Usuario pronto para conferir: sal e hash ja em bytes, para o pedido nao decodificar hex.
#[derive(Clone)]
pub struct Carregado {
    nome: String,
    papel: Papel,
    projetos: Vec<String>,
    sal: [u8; 16],
    hash: [u8; 32],
}

/// Assinatura do arquivo no disco: a gravacao troca o inode (temporario + rename), entao
/// revogar vale no pedido seguinte, sem reiniciar o servidor.
type Marca = (Option<std::time::SystemTime>, u64, u64);

struct Fonte {
    arquivo: PathBuf,
    cache: Mutex<(Option<Marca>, Arc<Vec<Carregado>>)>,
}

/// Os usuarios da API, relidos quando o arquivo muda. `Default` = nenhum (os testes e o
/// canal, que nao abrem a API).
#[derive(Clone, Default)]
pub struct Usuarios(Option<Arc<Fonte>>);

impl Usuarios {
    pub fn nenhum() -> Self {
        Self(None)
    }

    pub fn da_pasta(raiz: &Path) -> Self {
        Self(Some(Arc::new(Fonte {
            arquivo: raiz.join(ARQUIVO),
            cache: Mutex::new((None, Arc::new(Vec::new()))),
        })))
    }

    /// A lista em vigor. Vazia = RBAC desligado. Arquivo ilegivel tambem da vazia: so o
    /// `api.token` vale, que e o mesmo que nao ter usuarios -- nunca abre para um token
    /// de usuario que nao se conseguiu conferir.
    pub fn ativos(&self) -> Arc<Vec<Carregado>> {
        let Some(f) = &self.0 else {
            return Arc::new(Vec::new());
        };
        let Ok(m) = std::fs::metadata(&f.arquivo) else {
            return Arc::new(Vec::new());
        };
        let marca = marca_de(&m);
        let mut c = f.cache.lock().unwrap_or_else(|p| p.into_inner());
        if c.0 != Some(marca) {
            let lista = match ler_arquivo(&f.arquivo) {
                Ok(a) => carregar(&a.usuarios),
                Err(e) => {
                    eprintln!("{ARQUIVO} ilegivel ({e}): so o api.token vale");
                    Vec::new()
                }
            };
            *c = (Some(marca), Arc::new(lista));
        }
        c.1.clone()
    }

    /// O RBAC esta ligado (ha ao menos um usuario).
    pub fn ligado(&self) -> bool {
        !self.ativos().is_empty()
    }
}

fn marca_de(m: &std::fs::Metadata) -> Marca {
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(m);
    #[cfg(not(unix))]
    let ino = 0;
    (m.modified().ok(), m.len(), ino)
}

fn carregar(v: &[Usuario]) -> Vec<Carregado> {
    v.iter()
        .filter_map(|u| {
            Some(Carregado {
                nome: u.nome.clone(),
                papel: u.papel,
                projetos: u.projetos.clone(),
                sal: de_hex(&u.sal)?,
                hash: de_hex(&u.hash)?,
            })
        })
        .collect()
}

fn ler_arquivo(p: &Path) -> Result<Arquivo, String> {
    match std::fs::read(p) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Arquivo::default()),
        Err(e) => Err(e.to_string()),
    }
}

fn gravar_arquivo(p: &Path, a: &Arquivo) -> Result<(), String> {
    let t = serde_json::to_vec_pretty(a).map_err(|e| e.to_string())?;
    phxclaw_secret_broker::write_private_file(p, &t).map_err(|e| e.to_string())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn de_hex<const N: usize>(t: &str) -> Option<[u8; N]> {
    let t = t.as_bytes();
    if t.len() != 2 * N {
        return None;
    }
    let mut o = [0u8; N];
    for (i, par) in t.chunks(2).enumerate() {
        let s = std::str::from_utf8(par).ok()?;
        o[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(o)
}

fn resumo_do_token(sal: &[u8; 16], token: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(sal);
    h.update(token.as_bytes());
    h.finalize().into()
}

/// Compara dois resumos sem sair no primeiro byte diferente, e diz quantos bytes visitou:
/// e a contagem que o teste confere (medir relogio num teste seria ruido, nao prova).
fn comparar_contando(a: &[u8; 32], b: &[u8; 32]) -> (bool, usize) {
    let mut dif = 0u8;
    let mut visitados = 0usize;
    for i in 0..32 {
        dif |= a[i] ^ b[i];
        visitados += 1;
    }
    (std::hint::black_box(dif) == 0, visitados)
}

/// Acha o dono do token conferindo TODOS os usuarios, sem parar no que casou: o tempo da
/// resposta nao diz em que posicao da lista o token mora. Devolve tambem quantas
/// comparacoes fez, para o teste.
fn achar_contando<'a>(lista: &'a [Carregado], token: &str) -> (Option<&'a Carregado>, usize) {
    let mut achado = None;
    let mut comparacoes = 0;
    for u in lista {
        let (igual, _) = comparar_contando(&resumo_do_token(&u.sal, token), &u.hash);
        comparacoes += 1;
        if igual {
            achado = Some(u);
        }
    }
    (achado, comparacoes)
}

fn bearer(h: &HeaderMap) -> Option<&str> {
    h.get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Quem fez o pedido e o que ele alcanca, posto pelo portao na extensao do pedido: e o que o
/// `criar` e o `listar` leem para carimbar e filtrar pelo projeto, sem decidir de novo.
#[derive(Debug, Clone)]
pub struct Acesso {
    pub quem: String,
    pub papel: Papel,
    projetos: Vec<String>,
    /// O projeto resolvido para este pedido (cabecalho, unico do usuario, ou da tarefa).
    pub projeto: Option<String>,
}

impl Acesso {
    fn legado() -> Self {
        Self {
            quem: "api.token".into(),
            papel: Papel::Dono,
            projetos: vec![],
            projeto: None,
        }
    }

    fn alcanca(&self, projeto: Option<&str>) -> bool {
        self.papel.global() || projeto.is_some_and(|p| self.projetos.iter().any(|x| x == p))
    }
}

/// Identifica o token: o `api.token` (dono) ou um usuario da lista.
fn identificar(s: &ApiState, lista: &[Carregado], h: &HeaderMap) -> Option<Acesso> {
    if phxclaw_api_gateway::authorized(h, &s.token) {
        return Some(Acesso::legado());
    }
    let (u, _) = achar_contando(lista, bearer(h)?);
    u.map(|u| Acesso {
        quem: u.nome.clone(),
        papel: u.papel,
        projetos: u.projetos.clone(),
        projeto: None,
    })
}

/// Autenticacao SO (sem papel nem projeto) para a `api::auth` das rotas: o portao ja
/// decidiu o papel; a rota so confirma que o token e de alguem. Sem usuarios, falso.
pub fn e_usuario(s: &ApiState, h: &HeaderMap) -> bool {
    let lista = s.usuarios.ativos();
    !lista.is_empty()
        && bearer(h)
            .map(|t| achar_contando(&lista, t).0.is_some())
            .unwrap_or(false)
}

type Recusa = (StatusCode, Json<Value>);

fn recusa(code: StatusCode, msg: impl Into<String>) -> Recusa {
    (code, Json(json!({"error": msg.into()})))
}

fn nome_valido(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 64
        && n.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn projeto_do_cabecalho(a: &Acesso, h: &HeaderMap) -> Result<Option<String>, Recusa> {
    match h.get(CABECALHO_PROJETO) {
        Some(v) => {
            let p = v.to_str().unwrap_or("").trim();
            if !nome_valido(p) {
                return Err(recusa(
                    StatusCode::BAD_REQUEST,
                    "X-PhxClaw-Projeto invalido (letras, digitos, - _ . ate 64)",
                ));
            }
            Ok(Some(p.to_string()))
        }
        None if a.papel.global() => Ok(None),
        None => match a.projetos.as_slice() {
            [unico] => Ok(Some(unico.clone())),
            _ => Err(recusa(
                StatusCode::BAD_REQUEST,
                "informe o projeto no cabecalho X-PhxClaw-Projeto",
            )),
        },
    }
}

/// O projeto da tarefa gravada; o subagente herda o da mae (ate `PROFUNDIDADE_DA_MAE`).
fn projeto_da_tarefa(s: &ApiState, id: &str) -> Option<Option<String>> {
    let mut t = s.store.load(id).ok()?;
    for _ in 0..PROFUNDIDADE_DA_MAE {
        if t.projeto.is_some() {
            break;
        }
        match t.parent.as_deref().and_then(|m| s.store.load(m).ok()) {
            Some(m) => t = m,
            None => break,
        }
    }
    Some(t.projeto)
}

/// A decisao, UMA so: o portao chama para todo pedido; o websocket do IDE, com o token da
/// primeira mensagem. `Ok(None)` = RBAC desligado ou rota publica (nada a carimbar).
fn decidir(
    s: &ApiState,
    metodo: &Method,
    rota: Option<&str>,
    h: &HeaderMap,
    id_da_tarefa: Option<&str>,
    token_na_mao: bool,
) -> Result<Option<Acesso>, Recusa> {
    let lista = s.usuarios.ativos();
    if lista.is_empty() {
        return Ok(None);
    }
    let regra = regra_de(metodo, rota);
    match regra.escopo {
        Escopo::Publico => return Ok(None),
        Escopo::TokenNaMensagem if !token_na_mao => return Ok(None),
        _ => {}
    }
    let mut a = identificar(s, &lista, h)
        .ok_or_else(|| recusa(StatusCode::UNAUTHORIZED, "token ausente ou invalido"))?;
    if a.papel < regra.minimo {
        return Err(recusa(
            StatusCode::FORBIDDEN,
            format!(
                "papel {} nao alcanca esta rota (pede {})",
                a.papel.nome(),
                regra.minimo.nome()
            ),
        ));
    }
    let sem_projeto = || recusa(StatusCode::FORBIDDEN, "token sem acesso a este projeto");
    match regra.escopo {
        Escopo::Projeto => {
            let p = projeto_do_cabecalho(&a, h)?;
            if !a.alcanca(p.as_deref()) {
                return Err(sem_projeto());
            }
            a.projeto = p;
        }
        Escopo::Tarefa => {
            let id = id_da_tarefa
                .ok_or_else(|| recusa(StatusCode::BAD_REQUEST, "rota de tarefa sem id"))?;
            let p = projeto_da_tarefa(s, id)
                .ok_or_else(|| recusa(StatusCode::NOT_FOUND, "tarefa inexistente"))?;
            if !a.alcanca(p.as_deref()) {
                return Err(sem_projeto());
            }
            a.projeto = p;
        }
        Escopo::Instancia | Escopo::TokenNaMensagem | Escopo::Publico => {}
    }
    Ok(Some(a))
}

/// O portao: middleware do `api::router` (`route_layer`), para TODA rota dele. Sem usuarios
/// devolve o pedido intacto -- o primeiro teste da funcao e esse.
pub async fn portao(State(s): State<ApiState>, req: Request, next: Next) -> Response {
    if !s.usuarios.ligado() {
        return next.run(req).await;
    }
    let rota = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string());
    let (mut partes, corpo) = req.into_parts();
    let id = RawPathParams::from_request_parts(&mut partes, &())
        .await
        .ok()
        .and_then(|p| {
            p.iter()
                .find(|(k, _)| *k == "id")
                .map(|(_, v)| v.to_string())
        });
    match decidir(
        &s,
        &partes.method,
        rota.as_deref(),
        &partes.headers,
        id.as_deref(),
        false,
    ) {
        Err(e) => e.into_response(),
        Ok(a) => {
            let mut req = Request::from_parts(partes, corpo);
            if let Some(a) = a {
                req.extensions_mut().insert(a);
            }
            next.run(req).await
        }
    }
}

/// Para a rota que recebe o token fora do cabecalho (websocket): a mesma `decidir`, com a
/// linha da matriz dela. Sem usuarios, cai na conferencia do Bearer unico de sempre.
pub fn conferir_rota(
    s: &ApiState,
    metodo: Method,
    rota: &str,
    h: &HeaderMap,
) -> Result<(), Recusa> {
    if !s.usuarios.ligado() {
        return crate::api::auth(s, h);
    }
    decidir(s, &metodo, Some(rota), h, None, true).map(|_| ())
}

/// As tarefas que o pedido enxerga: com o projeto resolvido pelo portao, so as dele (o
/// subagente pelo projeto da mae); sem acesso carimbado (RBAC desligado) ou sem projeto
/// (dono/admin sem cabecalho), todas.
pub fn visiveis(
    acesso: Option<&Acesso>,
    tarefas: Vec<crate::tarefa::Task>,
) -> Vec<crate::tarefa::Task> {
    let Some(p) = acesso.and_then(|a| a.projeto.as_deref()) else {
        return tarefas;
    };
    let por_id: std::collections::HashMap<&str, &crate::tarefa::Task> =
        tarefas.iter().map(|t| (t.id.as_str(), t)).collect();
    let projeto_de = |t: &crate::tarefa::Task| -> Option<String> {
        let mut t = t;
        for _ in 0..PROFUNDIDADE_DA_MAE {
            if t.projeto.is_some() {
                break;
            }
            match t.parent.as_deref().and_then(|m| por_id.get(m)) {
                Some(m) => t = m,
                None => break,
            }
        }
        t.projeto.clone()
    };
    let manter: Vec<bool> = tarefas
        .iter()
        .map(|t| projeto_de(t).as_deref() == Some(p))
        .collect();
    tarefas
        .into_iter()
        .zip(manter)
        .filter_map(|(t, m)| m.then_some(t))
        .collect()
}

// ---------------------------------------------------------------- comando local (CLI)

fn novo_token() -> String {
    format!(
        "{PREFIXO_TOKEN}{}",
        phxclaw_api_gateway::generate_bearer_token()
    )
}

fn novo_sal() -> Result<[u8; 16], String> {
    let mut s = [0u8; 16];
    getrandom::fill(&mut s).map_err(|e| format!("CSPRNG do sistema indisponivel: {e}"))?;
    Ok(s)
}

/// Gera o token e devolve (token, sal hex, hash hex). O token so existe no retorno: quem
/// chama o mostra UMA vez e o descarta.
fn emitir() -> Result<(String, String, String), String> {
    let token = novo_token();
    let sal = novo_sal()?;
    let hash = resumo_do_token(&sal, &token);
    Ok((token, hex(&sal), hex(&hash)))
}

struct Opcoes {
    posicionais: Vec<String>,
    papel: Option<String>,
    projetos: Vec<String>,
}

fn opcoes(args: &[String]) -> Result<Opcoes, String> {
    let mut o = Opcoes {
        posicionais: vec![],
        papel: None,
        projetos: vec![],
    };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let valor = || {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{a} pede um valor"))
        };
        match a {
            "--papel" | "--role" => {
                o.papel = Some(valor()?);
                i += 2;
            }
            "--projeto" | "--project" => {
                o.projetos.push(valor()?);
                i += 2;
            }
            "--pasta" => i += 2,
            _ if a.starts_with("--") => return Err(format!("opcao desconhecida: {a}")),
            _ => {
                o.posicionais.push(args[i].clone());
                i += 1;
            }
        }
    }
    Ok(o)
}

pub const USO: &str = "uso: phxclaw usuario criar NOME --papel owner|admin|member|leitor \
[--projeto P]... | listar | chave NOME | remover NOME  [--pasta DIR]";

/// `phxclaw usuario ...` na pasta `raiz`. Criar e trocar a chave imprimem o token UMA vez;
/// o disco guarda so o sal e o hash. Nao existe rota HTTP para isto: criar usuario pela
/// rede seria a porta para o primeiro que chegar se fazer dono.
pub fn comando(raiz: &Path, args: &[String]) -> Result<String, String> {
    let o = opcoes(args)?;
    let arq = raiz.join(ARQUIVO);
    let mut a = ler_arquivo(&arq)?;
    let nome = o.posicionais.get(1).map(String::as_str).unwrap_or("");
    match o.posicionais.first().map(String::as_str) {
        Some("criar" | "create") => {
            if !nome_valido(nome) {
                return Err(format!(
                    "nome invalido (letras, digitos, - _ . ate 64)\n{USO}"
                ));
            }
            if a.usuarios.iter().any(|u| u.nome == nome) {
                return Err(format!(
                    "usuario {nome} ja existe (troque a chave com `chave`)"
                ));
            }
            let papel = o
                .papel
                .as_deref()
                .and_then(Papel::do_texto)
                .ok_or_else(|| format!("--papel obrigatorio: owner|admin|member|leitor\n{USO}"))?;
            if let Some(p) = o.projetos.iter().find(|p| !nome_valido(p)) {
                return Err(format!("projeto invalido: {p}"));
            }
            if !papel.global() && o.projetos.is_empty() {
                return Err(format!(
                    "{} sem --projeto nao alcanca tarefa nenhuma",
                    papel.nome()
                ));
            }
            let (token, sal, hash) = emitir()?;
            a.usuarios.push(Usuario {
                nome: nome.to_string(),
                papel,
                projetos: o.projetos.clone(),
                sal,
                hash,
                criado_em: Utc::now(),
            });
            gravar_arquivo(&arq, &a)?;
            Ok(format!(
                "usuario {nome} criado ({}; projetos: {})\n\
                 token (aparece SO agora; o disco guarda so o hash):\n{token}\n\
                 uso: Authorization: Bearer <token>  e, com mais de um projeto, \
                 X-PhxClaw-Projeto: <projeto>",
                papel.nome(),
                projetos_de(papel, &o.projetos)
            ))
        }
        Some("chave" | "key") => {
            let u = a
                .usuarios
                .iter_mut()
                .find(|u| u.nome == nome)
                .ok_or_else(|| format!("usuario inexistente: {nome}"))?;
            let (token, sal, hash) = emitir()?;
            u.sal = sal;
            u.hash = hash;
            gravar_arquivo(&arq, &a)?;
            Ok(format!(
                "chave de {nome} trocada; a anterior deixou de valer\n\
                 token (aparece SO agora; o disco guarda so o hash):\n{token}"
            ))
        }
        Some("remover" | "remove") => {
            let antes = a.usuarios.len();
            a.usuarios.retain(|u| u.nome != nome);
            if a.usuarios.len() == antes {
                return Err(format!("usuario inexistente: {nome}"));
            }
            gravar_arquivo(&arq, &a)?;
            Ok(format!(
                "usuario {nome} removido{}",
                if a.usuarios.is_empty() {
                    "; sem usuarios, a API volta ao Bearer unico do api.token"
                } else {
                    ""
                }
            ))
        }
        Some("listar" | "list") => {
            if a.usuarios.is_empty() {
                return Ok("nenhum usuario: a API usa so o Bearer do api.token".into());
            }
            Ok(a.usuarios
                .iter()
                .map(|u| {
                    format!(
                        "{}  {}  projetos: {}  criado em {}",
                        u.nome,
                        u.papel.nome(),
                        projetos_de(u.papel, &u.projetos),
                        u.criado_em.format("%Y-%m-%d %H:%M UTC")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        _ => Err(USO.into()),
    }
}

fn projetos_de(papel: Papel, p: &[String]) -> String {
    if papel.global() {
        "todos".into()
    } else {
        p.join(", ")
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("phx-rbac-{nome}-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn token_da_saida(s: &str) -> String {
        s.lines()
            .find(|l| l.starts_with(PREFIXO_TOKEN))
            .expect("token na saida")
            .to_string()
    }

    #[test]
    fn a_comparacao_visita_os_32_bytes_onde_quer_que_esteja_a_diferenca() {
        let a = [7u8; 32];
        let mut cedo = a;
        cedo[0] ^= 1;
        let mut tarde = a;
        tarde[31] ^= 1;
        assert_eq!(comparar_contando(&a, &a), (true, 32));
        assert_eq!(comparar_contando(&a, &cedo), (false, 32));
        assert_eq!(comparar_contando(&a, &tarde), (false, 32));
    }

    #[test]
    fn achar_confere_todos_os_usuarios_mesmo_depois_de_casar() {
        let d = pasta("todos");
        for n in ["ana", "bia", "caio"] {
            comando(&d, &args(&["criar", n, "--papel", "admin"])).unwrap();
        }
        let primeiro = token_da_saida(&comando(&d, &args(&["chave", "ana"])).unwrap());
        let lista = carregar(&ler_arquivo(&d.join(ARQUIVO)).unwrap().usuarios);
        let (u, n) = achar_contando(&lista, &primeiro);
        assert_eq!(u.map(|u| u.nome.as_str()), Some("ana"));
        assert_eq!(n, 3, "parou no primeiro que casou");
        let (u, n) = achar_contando(&lista, "phxu_errado");
        assert!(u.is_none());
        assert_eq!(n, 3);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn o_hash_nao_vaza_no_disco_no_debug_nem_na_listagem() {
        let d = pasta("vaza");
        let saida = comando(
            &d,
            &args(&["criar", "ana", "--papel", "member", "--projeto", "vendas"]),
        )
        .unwrap();
        let token = token_da_saida(&saida);
        let disco = std::fs::read_to_string(d.join(ARQUIVO)).unwrap();
        assert!(!disco.contains(&token), "token em texto puro no disco");
        let a = ler_arquivo(&d.join(ARQUIVO)).unwrap();
        let u = &a.usuarios[0];
        assert_eq!(u.hash.len(), 64);
        assert!(disco.contains(&u.hash), "o disco guarda o hash");
        let dbg = format!("{u:?}");
        assert!(!dbg.contains(&u.hash) && !dbg.contains(&u.sal), "{dbg}");
        let l = comando(&d, &args(&["listar"])).unwrap();
        assert!(!l.contains(&u.hash) && !l.contains(&token), "{l}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let m = std::fs::metadata(d.join(ARQUIVO))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(m & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn membro_sem_projeto_e_papel_desconhecido_sao_recusados_na_criacao() {
        let d = pasta("cria");
        assert!(comando(&d, &args(&["criar", "ana", "--papel", "member"])).is_err());
        assert!(comando(&d, &args(&["criar", "ana", "--papel", "chefe"])).is_err());
        assert!(comando(&d, &args(&["criar", "a b", "--papel", "admin"])).is_err());
        assert!(!d.join(ARQUIVO).exists(), "recusa nao grava");
        comando(&d, &args(&["criar", "ana", "--papel", "owner"])).unwrap();
        assert!(comando(&d, &args(&["criar", "ana", "--papel", "owner"])).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn rota_sem_linha_na_matriz_e_so_do_dono() {
        let r = regra_de(&Method::GET, Some("/v1/rota-que-ninguem-classificou"));
        assert_eq!((r.minimo, r.escopo), (Papel::Dono, Escopo::Instancia));
        let r = regra_de(&Method::HEAD, Some("/v1/tasks/{id}"));
        assert_eq!((r.minimo, r.escopo), (Papel::Leitor, Escopo::Tarefa));
        // Uma linha por (metodo, rota): duas seriam duas decisoes para a mesma pergunta.
        for (i, a) in MATRIZ.iter().enumerate() {
            for b in &MATRIZ[i + 1..] {
                assert!(
                    !(a.rota == b.rota
                        && (a.metodo == b.metodo || a.metodo == "*" || b.metodo == "*")),
                    "linha repetida: {} {}",
                    a.metodo,
                    a.rota
                );
            }
        }
    }
}
