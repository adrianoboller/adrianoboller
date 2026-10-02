//! `github`/`github_write` e `gitlab`/`gitlab_write`: issues e PRs (MRs no GitLab) pela API
//! REST -- listar, ler, comentar, criar, e o diff do PR para o `code_review`.
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **O token mora so no SecretBroker** (envelope cifrado na pasta `forja/`), como o do
//!   Telegram, e e lido por concessao de 30 s a cada chamada, revogada logo depois. Nenhum
//!   campo daqui o guarda, e nenhuma mensagem de erro o recebe. `GITHUB_TOKEN`/`GH_TOKEN`
//!   do ambiente do processo NAO sao lidos: so o `phxclaw forja token` guarda um token, e
//!   guardar e um ato do operador, nao um efeito colateral do ambiente.
//! - **O modelo nunca escolhe o servidor.** A base sai de `PHXCLAW_GITHUB_API` /
//!   `PHXCLAW_GITLAB_API` (padrao: as publicas); o modelo da so `dono/projeto` e numero.
//! - **Sem seguir redirecionamento.** Redirecionar para outra origem levando o cabecalho
//!   do token e o jeito classico de vazar credencial.
//! - **Sem token, a ferramenta nem existe**, como o `channel_send` sem canal.
//! - Capacidades `github.read`/`github.write`/`gitlab.read`/`gitlab.write` fora do padrao:
//!   e rede para fora em nome do operador, ele concede.

use crate::canais::http::Credencial;
use crate::git::{analisar_diff, limitar_diff};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_http_client::{HttpRequestSpec, HttpResult, http_request};
use phxclaw_secret_broker::{SecretBroker, SecretValue};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

const NAMESPACE: &str = "forjas";
const MAX_CHARS_CORPO: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forja {
    Github,
    Gitlab,
}

impl Forja {
    pub fn nome(self) -> &'static str {
        match self {
            Forja::Github => "github",
            Forja::Gitlab => "gitlab",
        }
    }
    pub fn de_nome(s: &str) -> Option<Self> {
        match s {
            "github" => Some(Forja::Github),
            "gitlab" => Some(Forja::Gitlab),
            _ => None,
        }
    }
    fn segredo(self) -> String {
        format!("{}-token", self.nome())
    }
    fn escopo(self) -> String {
        format!("forge:{}:api", self.nome())
    }
    /// A chave do catalogo de `campo` desta forja (`forja.github.api`, `forja.gitlab.token`).
    fn chave(self, campo: &str) -> String {
        format!("forja.{}.{campo}", self.nome())
    }
    /// Base da API: so o operador escolhe, pela configuracao.
    pub fn base_do_ambiente(self) -> String {
        let padrao = match self {
            Forja::Github => "https://api.github.com",
            Forja::Gitlab => "https://gitlab.com/api/v4",
        };
        crate::config::texto_de(&self.chave("api"))
            .unwrap_or_else(|| padrao.into())
            .trim_end_matches('/')
            .to_string()
    }
}

/// A pasta do broker das forjas, ao lado das tarefas.
pub fn pasta_da_forja(raiz_do_agente: &Path) -> std::path::PathBuf {
    raiz_do_agente.join("forja")
}

/// Guarda o token da forja e devolve o id do segredo: mesmo valor reaproveita o envelope,
/// valor novo rotaciona. A regra e a de `canais::guardar_segredo`, a mesma dos canais e do
/// MCP; aqui so se diz o nome, o espaco e o escopo da forja.
pub fn guardar_token(
    broker: &SecretBroker,
    forja: Forja,
    token: SecretValue,
) -> Result<Uuid, String> {
    crate::canais::guardar_segredo(
        broker,
        &forja.segredo(),
        NAMESPACE,
        &[&forja.escopo()],
        token,
    )
}

fn token_guardado(
    broker: &SecretBroker,
    forja: Forja,
) -> Result<Option<phxclaw_secret_broker::SecretDescriptor>, String> {
    crate::canais::segredo_guardado(broker, &forja.segredo(), NAMESPACE)
}

/// Cliente REST de uma forja. Guarda o broker e o id do segredo, nunca o token.
pub struct ForjaCliente {
    pub forja: Forja,
    pub base: String,
    /// A mesma `Credencial` dos canais: concessao de 30 s por chamada e limpeza do erro.
    token: Credencial,
}

fn falha(m: impl Into<String>) -> ToolError {
    ToolError::Failed(m.into())
}

fn cortar(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!(
            "{}[... {} caracteres]",
            s.chars().take(n).collect::<String>(),
            s.chars().count()
        )
    }
}

/// `dono/projeto` (GitLab aceita grupos aninhados). Nada de `..`, de barra no comeco ou de
/// caractere que mude a rota da URL.
pub fn projeto_valido(s: &str) -> Result<String, ToolError> {
    let s = s.trim().trim_matches('/');
    let ok = s.contains('/')
        && !s.split('/').any(|p| p.is_empty() || p == "." || p == "..")
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c));
    if !ok {
        return Err(ToolError::InvalidArguments(format!(
            "repo invalido: {s:?} (formato dono/projeto)"
        )));
    }
    Ok(s.to_string())
}

impl ForjaCliente {
    pub fn novo(forja: Forja, base: String, broker: Arc<SecretBroker>, segredo: Uuid) -> Self {
        Self {
            forja,
            base: base.trim_end_matches('/').to_string(),
            token: Credencial::de_escopo(
                broker,
                segredo,
                "phxclaw.agent.forja",
                format!("forge:{}", forja.nome()),
            ),
        }
    }

    /// Cliente com o token guardado na pasta `forja/` do agente; `None` sem broker ou sem
    /// token. Nao cria a pasta: so quem guardou um token tem forja.
    pub fn da_pasta(raiz_do_agente: &Path, forja: Forja) -> Option<Self> {
        let pasta = pasta_da_forja(raiz_do_agente);
        if !pasta.join("segredos/master.key").exists() {
            return None;
        }
        let broker = crate::canais::broker_em(&pasta)
            .map_err(|e| eprintln!("aviso: forja sem broker: {e}"))
            .ok()?;
        let d = token_guardado(&broker, forja).ok()??;
        Some(Self::novo(forja, forja.base_do_ambiente(), broker, d.uuid))
    }

    fn rota_do_projeto(&self, repo: &str) -> String {
        match self.forja {
            Forja::Github => format!("/repos/{repo}"),
            Forja::Gitlab => format!("/projects/{}", repo.replace('/', "%2F")),
        }
    }

    /// Uma chamada, com o token da concessao curta.
    pub async fn chamar(
        &self,
        metodo: &str,
        rota: &str,
        query: Vec<(String, String)>,
        corpo: Option<Value>,
        aceitar: &str,
    ) -> Result<HttpResult, ToolError> {
        // A concessao se revoga no `drop`, inclusive num `?` no meio do caminho.
        let token = self.token.abrir("api").map_err(|e| {
            falha(format!(
                "{}: sem concessao do token: {e}",
                self.forja.nome()
            ))
        })?;
        let mut spec = HttpRequestSpec::get(format!("{}{rota}", self.base));
        spec.method = metodo.into();
        spec.query = query;
        spec.json = corpo;
        spec.follow_redirects = false;
        spec.user_agent = Some("PhxClaw-agente".into());
        spec.headers.insert("Accept".into(), aceitar.into());
        match self.forja {
            Forja::Github => {
                spec.headers
                    .insert("X-GitHub-Api-Version".into(), "2022-11-28".into());
                spec.auth.bearer = Some(token.expor().to_string());
            }
            Forja::Gitlab => {
                spec.headers
                    .insert("PRIVATE-TOKEN".into(), token.expor().to_string());
            }
        }
        let r = http_request(&spec).await;
        drop(spec);
        // Todo texto que sai daqui para o modelo, a evidencia ou o JSON passa pela MESMA
        // limpeza do canal (`canais::http::limpar`): servidor que ecoa o cabecalho num erro
        // nao pode devolver o token ao modelo.
        let limpo = |m: String| -> ToolError { falha(token.limpar::<()>(Err(m)).unwrap_err()) };
        let r = r.map_err(|e| limpo(format!("{}: {e}", self.forja.nome())))?;
        // 3xx e erro: o redirecionamento nao e seguido (o token iria junto), e aceita-lo
        // como sucesso devolvia corpo vazio -- o `pr_diff` de repositorio renomeado (o GitHub
        // responde 301) virava «diff vazio, nada a revisar».
        if (300..400).contains(&r.status) {
            return Err(falha(format!(
                "{} respondeu {} (redirecionamento nao seguido: confira o dono/projeto ou a base da API)",
                self.forja.nome(),
                r.status
            )));
        }
        if r.status >= 400 {
            let corpo = r.text().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&corpo)
                .ok()
                .and_then(|v| {
                    v.get("message").or_else(|| v.get("error")).map(|m| {
                        m.as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| m.to_string())
                    })
                })
                .unwrap_or(corpo);
            // Limpa ANTES de cortar: o corte poderia partir o token e deixar meio segredo
            // que a limpeza ja nao reconhece.
            let msg = token.limpar::<()>(Err(msg)).unwrap_err();
            return Err(falha(format!(
                "{} respondeu {}: {}",
                self.forja.nome(),
                r.status,
                cortar(&msg, 300)
            )));
        }
        // Resposta de sucesso que traz o token de volta nao vai inteira ao modelo: limpar
        // um JSON por dentro mudaria o dado; recusar diz o que houve.
        let corpo = r.bytes().unwrap_or_default();
        let t = token.expor().as_bytes();
        if !t.is_empty() && corpo.windows(t.len()).any(|j| j == t) {
            return Err(limpo(format!(
                "{} devolveu a credencial no corpo da resposta; resposta descartada",
                self.forja.nome()
            )));
        }
        Ok(r)
    }

    async fn json(
        &self,
        metodo: &str,
        rota: &str,
        query: Vec<(String, String)>,
        corpo: Option<Value>,
    ) -> Result<Value, ToolError> {
        let aceitar = match self.forja {
            Forja::Github => "application/vnd.github+json",
            Forja::Gitlab => "application/json",
        };
        self.chamar(metodo, rota, query, corpo, aceitar)
            .await?
            .json()
            .map_err(|e| falha(format!("{}: resposta nao e JSON: {e}", self.forja.nome())))
    }

    /// O diff unificado de um PR/MR: o que o `pr_diff` devolve e o `phxclaw revisar --pr`
    /// le. No GitLab ele e montado dos pedacos de `/diffs`, com os cabecalhos que o
    /// analisador do git espera.
    pub async fn diff_de_pr(&self, repo: &str, numero: u64) -> Result<String, ToolError> {
        let p = self.rota_do_projeto(repo);
        match self.forja {
            Forja::Github => self
                .chamar(
                    "GET",
                    &format!("{p}/pulls/{numero}"),
                    vec![],
                    None,
                    "application/vnd.github.diff",
                )
                .await?
                .text()
                .map_err(|e| falha(e.to_string())),
            Forja::Gitlab => {
                let v = self
                    .json(
                        "GET",
                        &format!("{p}/merge_requests/{numero}/diffs"),
                        vec![("per_page".into(), "100".into())],
                        None,
                    )
                    .await?;
                let mut t = String::new();
                for d in v.as_array().into_iter().flatten() {
                    let s = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or("");
                    let b = |k: &str| d.get(k).and_then(Value::as_bool).unwrap_or(false);
                    t.push_str(&format!(
                        "diff --git a/{} b/{}\n",
                        s("old_path"),
                        s("new_path")
                    ));
                    if b("new_file") {
                        t.push_str(&format!("new file mode {}\n", s("b_mode")));
                    }
                    if b("deleted_file") {
                        t.push_str(&format!("deleted file mode {}\n", s("a_mode")));
                    }
                    if b("renamed_file") {
                        t.push_str(&format!(
                            "rename from {}\nrename to {}\n",
                            s("old_path"),
                            s("new_path")
                        ));
                    }
                    let antigo = if b("new_file") {
                        "/dev/null".to_string()
                    } else {
                        format!("a/{}", s("old_path"))
                    };
                    let novo = if b("deleted_file") {
                        "/dev/null".to_string()
                    } else {
                        format!("b/{}", s("new_path"))
                    };
                    if !s("diff").is_empty() {
                        t.push_str(&format!("--- {antigo}\n+++ {novo}\n"));
                        t.push_str(s("diff"));
                        if !t.ends_with('\n') {
                            t.push('\n');
                        }
                    }
                }
                Ok(t)
            }
        }
    }

    fn issue(&self, v: &Value) -> Value {
        let s = |k: &str| v.get(k).cloned().unwrap_or(Value::Null);
        let corpo = |k: &str| {
            json!(cortar(
                v.get(k).and_then(Value::as_str).unwrap_or(""),
                MAX_CHARS_CORPO
            ))
        };
        match self.forja {
            Forja::Github => json!({
                "numero": s("number"),
                "titulo": s("title"),
                "estado": s("state"),
                "autor": v.pointer("/user/login"),
                "url": s("html_url"),
                "corpo": corpo("body"),
                "rotulos": v.get("labels").and_then(Value::as_array).map(|l| l.iter().filter_map(|x| x.get("name")).cloned().collect::<Vec<_>>()),
                "comentarios": s("comments"),
            }),
            Forja::Gitlab => json!({
                "numero": s("iid"),
                "titulo": s("title"),
                "estado": s("state"),
                "autor": v.pointer("/author/username"),
                "url": s("web_url"),
                "corpo": corpo("description"),
                "rotulos": s("labels"),
                "comentarios": s("user_notes_count"),
            }),
        }
    }

    fn pr(&self, v: &Value) -> Value {
        let mut i = self.issue(v);
        let s = |k: &str| v.get(k).cloned().unwrap_or(Value::Null);
        match self.forja {
            Forja::Github => {
                i["de"] = v.pointer("/head/ref").cloned().unwrap_or(Value::Null);
                i["para"] = v.pointer("/base/ref").cloned().unwrap_or(Value::Null);
                i["rascunho"] = s("draft");
                i["mesclado"] = s("merged");
            }
            Forja::Gitlab => {
                i["de"] = s("source_branch");
                i["para"] = s("target_branch");
                i["rascunho"] = s("draft");
                i["mesclado"] = json!(v.get("state").and_then(Value::as_str) == Some("merged"));
            }
        }
        i
    }

    fn comentario(&self, v: &Value) -> Value {
        match self.forja {
            Forja::Github => {
                json!({"autor": v.pointer("/user/login"), "data": v.get("created_at"), "corpo": cortar(v.get("body").and_then(Value::as_str).unwrap_or(""), MAX_CHARS_CORPO)})
            }
            Forja::Gitlab => {
                json!({"autor": v.pointer("/author/username"), "data": v.get("created_at"), "corpo": cortar(v.get("body").and_then(Value::as_str).unwrap_or(""), MAX_CHARS_CORPO)})
            }
        }
    }

    /// As acoes que so leem (capacidade `<forja>.read`).
    pub async fn ler(&self, acao: &str, args: &Value) -> Result<Value, ToolError> {
        let repo = projeto_valido(texto(args, "repo")?)?;
        let p = self.rota_do_projeto(&repo);
        let gh = self.forja == Forja::Github;
        let limite = args
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .clamp(1, 100)
            .to_string();
        let estado = args.get("state").and_then(Value::as_str).unwrap_or("open");
        let estado = match (gh, estado) {
            (true, "open" | "closed" | "all") => estado.to_string(),
            (false, "open") => "opened".into(),
            (false, "closed" | "merged" | "all" | "opened") => estado.to_string(),
            (_, e) => {
                return Err(ToolError::InvalidArguments(format!(
                    "state invalido: {e} (open, closed, all)"
                )));
            }
        };
        let lista = vec![
            ("state".to_string(), estado),
            ("per_page".to_string(), limite),
        ];
        match acao {
            "list_issues" => {
                let v = self
                    .json("GET", &format!("{p}/issues"), lista, None)
                    .await?;
                // A lista de issues do GitHub inclui os PRs; quem pediu issue nao quer PR.
                let l: Vec<Value> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|i| !(gh && i.get("pull_request").is_some()))
                    .map(|i| self.issue(i))
                    .collect();
                Ok(json!({"issues": l}))
            }
            "list_prs" => {
                let rota = if gh {
                    format!("{p}/pulls")
                } else {
                    format!("{p}/merge_requests")
                };
                let v = self.json("GET", &rota, lista, None).await?;
                let l: Vec<Value> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|i| self.pr(i))
                    .collect();
                Ok(json!({"prs": l}))
            }
            "get_issue" | "get_pr" => {
                let n = numero(args)?;
                let (rota, notas) = match (gh, acao) {
                    (true, "get_issue") => (
                        format!("{p}/issues/{n}"),
                        format!("{p}/issues/{n}/comments"),
                    ),
                    (true, _) => (format!("{p}/pulls/{n}"), format!("{p}/issues/{n}/comments")),
                    (false, "get_issue") => {
                        (format!("{p}/issues/{n}"), format!("{p}/issues/{n}/notes"))
                    }
                    (false, _) => (
                        format!("{p}/merge_requests/{n}"),
                        format!("{p}/merge_requests/{n}/notes"),
                    ),
                };
                let v = self.json("GET", &rota, vec![], None).await?;
                let c = self
                    .json("GET", &notas, vec![("per_page".into(), "50".into())], None)
                    .await?;
                let mut item = if acao == "get_pr" {
                    self.pr(&v)
                } else {
                    self.issue(&v)
                };
                item["lista_de_comentarios"] = json!(
                    c.as_array()
                        .into_iter()
                        .flatten()
                        .map(|x| self.comentario(x))
                        .collect::<Vec<_>>()
                );
                Ok(item)
            }
            "pr_diff" => {
                let n = numero(args)?;
                let d = self.diff_de_pr(&repo, n).await?;
                let (d, cortou) = limitar_diff(&d, crate::git::DIFF_MAX_BYTES);
                let arquivos: Vec<Value> = analisar_diff(&d)
                    .iter()
                    .map(|a| json!({"caminho": a.caminho, "estado": a.estado, "adicoes": a.adicoes, "remocoes": a.remocoes}))
                    .collect();
                Ok(json!({"arquivos": arquivos, "diff": d, "truncado": cortou}))
            }
            outra => Err(ToolError::InvalidArguments(format!(
                "action desconhecida em {}: {outra} (list_issues, get_issue, list_prs, get_pr, pr_diff)",
                self.forja.nome()
            ))),
        }
    }

    /// As acoes que mudam algo na forja (capacidade `<forja>.write`).
    pub async fn escrever(&self, acao: &str, args: &Value) -> Result<Value, ToolError> {
        let repo = projeto_valido(texto(args, "repo")?)?;
        let p = self.rota_do_projeto(&repo);
        let gh = self.forja == Forja::Github;
        let corpo_txt = args.get("body").and_then(Value::as_str).unwrap_or("");
        match acao {
            "comment" => {
                let n = numero(args)?;
                if corpo_txt.trim().is_empty() {
                    return Err(ToolError::InvalidArguments("falta 'body'".into()));
                }
                let em_pr = args.get("on").and_then(Value::as_str) == Some("pr");
                let rota = match (gh, em_pr) {
                    // No GitHub o comentario geral de PR e o de issue: mesma rota.
                    (true, _) => format!("{p}/issues/{n}/comments"),
                    (false, false) => format!("{p}/issues/{n}/notes"),
                    (false, true) => format!("{p}/merge_requests/{n}/notes"),
                };
                let v = self
                    .json("POST", &rota, vec![], Some(json!({"body": corpo_txt})))
                    .await?;
                Ok(json!({"comentado": n, "id": v.get("id"), "url": v.get("html_url")}))
            }
            "create_issue" => {
                let titulo = texto(args, "title")?;
                let corpo = if gh {
                    json!({"title": titulo, "body": corpo_txt})
                } else {
                    json!({"title": titulo, "description": corpo_txt})
                };
                let v = self
                    .json("POST", &format!("{p}/issues"), vec![], Some(corpo))
                    .await?;
                Ok(self.issue(&v))
            }
            "create_pr" => {
                let titulo = texto(args, "title")?;
                let de = crate::git::referencia(texto(args, "head")?)?;
                let para = crate::git::referencia(texto(args, "base")?)?;
                let rascunho = args.get("draft").and_then(Value::as_bool).unwrap_or(false);
                let (rota, corpo) = if gh {
                    (
                        format!("{p}/pulls"),
                        json!({"title": titulo, "head": de, "base": para, "body": corpo_txt, "draft": rascunho}),
                    )
                } else {
                    let titulo = if rascunho {
                        format!("Draft: {titulo}")
                    } else {
                        titulo.to_string()
                    };
                    (
                        format!("{p}/merge_requests"),
                        json!({"title": titulo, "source_branch": de, "target_branch": para, "description": corpo_txt}),
                    )
                };
                let v = self.json("POST", &rota, vec![], Some(corpo)).await?;
                Ok(self.pr(&v))
            }
            outra => Err(ToolError::InvalidArguments(format!(
                "action desconhecida em {}_write: {outra} (comment, create_issue, create_pr)",
                self.forja.nome()
            ))),
        }
    }
}

fn texto<'a>(args: &'a Value, nome: &str) -> Result<&'a str, ToolError> {
    args.get(nome)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::InvalidArguments(format!("falta o campo texto '{nome}'")))
}

fn numero(args: &Value) -> Result<u64, ToolError> {
    args.get("number")
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str()?.trim().trim_start_matches('#').parse().ok())
        })
        .filter(|n| *n > 0)
        .ok_or_else(|| ToolError::InvalidArguments("falta 'number' (inteiro)".into()))
}

/// A ferramenta: leitura e escrita da mesma forja sao a mesma struct registrada duas vezes,
/// uma por capacidade.
pub struct ForjaTool {
    pub cliente: Arc<ForjaCliente>,
    pub escrita: bool,
}

impl Tool for ForjaTool {
    fn spec(&self) -> ToolSpec {
        let f = self.cliente.forja.nome();
        let pr = if self.cliente.forja == Forja::Github {
            "pull request"
        } else {
            "merge request"
        };
        if self.escrita {
            ToolSpec {
                name: format!("{f}_write"),
                description: format!(
                    "Act on {f} ({pr}s are 'pr'). action: comment {{repo, number, body, on: issue|pr}}; \
create_issue {{repo, title, body?}}; create_pr {{repo, title, head, base, body?, draft?}}. repo is owner/name."
                ),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["comment","create_issue","create_pr"]},
                    "repo":{"type":"string"},
                    "number":{"type":"integer"},
                    "body":{"type":"string"},
                    "on":{"type":"string","enum":["issue","pr"]},
                    "title":{"type":"string"},
                    "head":{"type":"string"},
                    "base":{"type":"string"},
                    "draft":{"type":"boolean"}
                },"required":["action","repo"]}),
            }
        } else {
            ToolSpec {
                name: f.into(),
                description: format!(
                    "Read {f} ({pr}s are 'pr'). action: list_issues {{repo, state?, limit?}}; get_issue \
{{repo, number}} (with comments); list_prs; get_pr {{repo, number}}; pr_diff {{repo, number}} (files and \
unified diff, feed it to code_review). repo is owner/name."
                ),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["list_issues","get_issue","list_prs","get_pr","pr_diff"]},
                    "repo":{"type":"string"},
                    "number":{"type":"integer"},
                    "state":{"type":"string"},
                    "limit":{"type":"integer"}
                },"required":["action","repo"]}),
            }
        }
    }
    fn capability(&self) -> &'static str {
        match (self.cliente.forja, self.escrita) {
            (Forja::Github, false) => "github.read",
            (Forja::Github, true) => "github.write",
            (Forja::Gitlab, false) => "gitlab.read",
            (Forja::Gitlab, true) => "gitlab.write",
        }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = texto(&args, "action")?.to_string();
            let v = if self.escrita {
                self.cliente.escrever(&acao, &args).await?
            } else {
                self.cliente.ler(&acao, &args).await?
            };
            Ok(ToolOutput::text(v.to_string()))
        })
    }
}

/// As ferramentas das forjas com token guardado na pasta do agente (leitura e escrita de
/// cada uma). Sem token, nenhuma.
pub fn ferramentas_da_pasta(raiz_do_agente: &Path) -> Vec<Arc<dyn Tool>> {
    let mut v: Vec<Arc<dyn Tool>> = Vec::new();
    for f in [Forja::Github, Forja::Gitlab] {
        if let Some(c) = ForjaCliente::da_pasta(raiz_do_agente, f) {
            let c = Arc::new(c);
            v.push(Arc::new(ForjaTool {
                cliente: c.clone(),
                escrita: false,
            }));
            v.push(Arc::new(ForjaTool {
                cliente: c,
                escrita: true,
            }));
        }
    }
    v
}

/// `phxclaw forja token <forja>`: o token sai de `PHXCLAW_GITHUB_TOKEN` /
/// `PHXCLAW_GITLAB_TOKEN` direto para o broker da pasta. Variavel propria, e nao o
/// `GITHUB_TOKEN` que outras ferramentas deixam no ambiente: guardar e escolha do operador.
pub fn guardar_do_ambiente(raiz_do_agente: &Path, forja: Forja) -> Result<Uuid, String> {
    let chave = forja.chave("token");
    let token = crate::config::segredo_do_ambiente(&chave)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            format!(
                "defina {} com o token de {}",
                crate::config::variavel(&chave),
                forja.nome()
            )
        })?;
    let broker = crate::canais::broker_em(&pasta_da_forja(raiz_do_agente))?;
    guardar_token(&broker, forja, SecretValue::new(token.trim().to_string()))
}
