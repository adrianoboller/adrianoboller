//! `code_review` e `phxclaw revisar`: um diff revisado pelo modelo do agente, com saida
//! estruturada (arquivo, linha, severidade, achado).
//!
//! O que esta ferramenta acrescenta ao «pergunte ao modelo» e a CONFERENCIA do que volta:
//!
//! - o diff vai numerado pela linha do arquivo NOVO, que e a linha que o achado cita;
//! - achado em arquivo que o diff nao tem, ou em linha fora de todo hunk, e DESCARTADO e
//!   contado com o motivo. Modelo pequeno inventa linha com facilidade, e achado na linha
//!   errada manda o revisor humano olhar o lugar errado -- pior que achado nenhum;
//! - severidade fora da escala (critica, alta, media, baixa, info; os nomes em ingles
//!   tambem valem) e descartada do mesmo jeito;
//! - resposta que nao e JSON ganha UMA segunda chance, dizendo o erro; a segunda falha e
//!   erro da ferramenta, nunca uma revisao vazia que pareca «nada a apontar».

use crate::git::{ArquivoDiff, analisar_diff, diff_do_repo, limitar_diff};
use phxclaw_agent_core::{
    BoxFut, Llm, LlmOptions, Message, Tool, ToolContext, ToolError, ToolOutput, ToolSpec,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Teto do diff mandado ao modelo: alem disto corta em fronteira de arquivo e diz.
pub const REVISAO_MAX_BYTES: usize = 60 * 1024;

pub const SEVERIDADES: &[&str] = &["critica", "alta", "media", "baixa", "info"];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Achado {
    pub arquivo: String,
    pub linha: Option<u64>,
    pub severidade: String,
    pub achado: String,
    pub sugestao: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Revisao {
    pub resumo: String,
    pub achados: Vec<Achado>,
    /// O que o modelo disse e nao passou na conferencia, com o motivo.
    pub descartados: Vec<Value>,
    pub arquivos: usize,
    pub truncado: bool,
    pub tokens_entrada: u64,
    pub tokens_saida: u64,
}

impl Revisao {
    /// Algum achado igual ou mais grave que `sev`? E o que o `--falhar-em` da CLI pergunta.
    pub fn tem_ao_menos(&self, sev: &str) -> bool {
        let limite = SEVERIDADES.iter().position(|s| *s == sev).unwrap_or(0);
        self.achados.iter().any(|a| {
            SEVERIDADES
                .iter()
                .position(|s| *s == a.severidade)
                .is_some_and(|p| p <= limite)
        })
    }
}

pub fn normalizar_severidade(s: &str) -> Option<&'static str> {
    let s = s.trim().to_lowercase();
    Some(match s.as_str() {
        "critica" | "crítica" | "critical" | "blocker" => "critica",
        "alta" | "high" | "major" => "alta",
        "media" | "média" | "medium" | "moderate" => "media",
        "baixa" | "low" | "minor" => "baixa",
        "info" | "informativa" | "nit" | "suggestion" => "info",
        _ => return None,
    })
}

/// O diff como o modelo le: cada linha do lado novo com o numero dela.
pub fn diff_numerado(arquivos: &[ArquivoDiff]) -> String {
    let mut t = String::new();
    for a in arquivos {
        t.push_str(&format!("=== arquivo: {} ({})\n", a.caminho, a.estado));
        if a.binario {
            t.push_str("(binario)\n");
        }
        for h in &a.hunks {
            t.push_str(&format!("@@ {}\n", h.contexto));
            let mut n = h.novo_inicio;
            for l in &h.linhas {
                match l.chars().next() {
                    Some('+') => {
                        t.push_str(&format!("{n:>6} +{}\n", &l[1..]));
                        n += 1;
                    }
                    Some('-') => t.push_str(&format!("       -{}\n", &l[1..])),
                    Some('\\') => {}
                    _ => {
                        t.push_str(&format!("{n:>6}  {}\n", l.get(1..).unwrap_or("")));
                        n += 1;
                    }
                }
            }
        }
    }
    t
}

const SISTEMA: &str = "Voce revisa codigo. Recebe um diff com cada linha do arquivo NOVO \
numerada a esquerda ('+' adicionada, ' ' contexto, '-' removida, sem numero). Aponte defeitos \
reais: erro de logica, seguranca, dado perdido, concorrencia, recurso que vaza, teste que nao \
prova. Nao aponte estilo. Responda SO com JSON, sem texto fora dele:\n\
{\"resumo\": \"uma frase\", \"achados\": [{\"arquivo\": \"caminho como no diff\", \"linha\": \
numero do arquivo novo, \"severidade\": \"critica|alta|media|baixa|info\", \"achado\": \"o \
defeito\", \"sugestao\": \"como corrigir\"}]}\n\
Sem defeito, \"achados\": [].";

/// Primeiro objeto JSON da resposta (o modelo as vezes cerca com ```json ou com prosa).
fn extrair_json(t: &str) -> Option<Value> {
    let ini = t.find('{')?;
    let fim = t.rfind('}')?;
    (fim > ini)
        .then(|| serde_json::from_str(&t[ini..=fim]).ok())
        .flatten()
}

/// Confere os achados do modelo contra o diff.
pub fn conferir(arquivos: &[ArquivoDiff], resposta: &Value) -> (Vec<Achado>, Vec<Value>) {
    let mut ok = Vec::new();
    let mut fora = Vec::new();
    let lista = resposta
        .get("achados")
        .or_else(|| resposta.get("findings"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for item in lista {
        let s = |k: &str| {
            item.get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
        };
        let caminho = s("arquivo")
            .trim_start_matches("a/")
            .trim_start_matches("b/")
            .to_string();
        let descartar = |motivo: &str| json!({"motivo": motivo, "item": item.clone()});
        let Some(arq) = arquivos.iter().find(|a| a.caminho == caminho) else {
            fora.push(descartar("arquivo fora do diff"));
            continue;
        };
        let linha = item
            .get("linha")
            .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()));
        if let Some(n) = linha
            && !arq.linha_no_diff(n)
        {
            fora.push(descartar("linha fora do diff"));
            continue;
        }
        let Some(sev) = normalizar_severidade(s("severidade")) else {
            fora.push(descartar("severidade fora da escala"));
            continue;
        };
        if s("achado").is_empty() {
            fora.push(descartar("achado vazio"));
            continue;
        }
        ok.push(Achado {
            arquivo: caminho,
            linha,
            severidade: sev.into(),
            achado: s("achado").into(),
            sugestao: Some(s("sugestao").to_string()).filter(|x| !x.is_empty()),
        });
    }
    ok.sort_by_key(|a| SEVERIDADES.iter().position(|s| *s == a.severidade));
    (ok, fora)
}

/// O motor: o mesmo para a ferramenta e para a CLI.
pub async fn revisar(llm: &dyn Llm, diff: &str, foco: Option<&str>) -> Result<Revisao, String> {
    let (diff, truncado) = limitar_diff(diff, REVISAO_MAX_BYTES);
    let arquivos = analisar_diff(&diff);
    if arquivos.is_empty() {
        return Ok(Revisao {
            resumo: "diff vazio: nada a revisar".into(),
            achados: vec![],
            descartados: vec![],
            arquivos: 0,
            truncado,
            tokens_entrada: 0,
            tokens_saida: 0,
        });
    }
    let mut pedido = String::new();
    if let Some(f) = foco {
        pedido.push_str(&format!("Foco pedido: {f}\n\n"));
    }
    if truncado {
        pedido.push_str("(diff cortado no teto; revise o que esta aqui)\n\n");
    }
    pedido.push_str(&diff_numerado(&arquivos));
    let mut msgs = vec![Message::system(SISTEMA), Message::user(pedido)];
    let opts = LlmOptions {
        max_output_tokens: 2048,
        temperature: 0.0,
    };
    let (mut ent, mut sai) = (0u64, 0u64);
    for tentativa in 0..2 {
        let r = llm
            .chat(&msgs, &[], &opts)
            .await
            .map_err(|e| format!("modelo: {e}"))?;
        ent += r.usage.input_tokens;
        sai += r.usage.output_tokens;
        if let Some(v) = extrair_json(&r.content) {
            let (achados, descartados) = conferir(&arquivos, &v);
            return Ok(Revisao {
                resumo: v
                    .get("resumo")
                    .or_else(|| v.get("summary"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                achados,
                descartados,
                arquivos: arquivos.len(),
                truncado,
                tokens_entrada: ent,
                tokens_saida: sai,
            });
        }
        if tentativa == 0 {
            msgs.push(Message::assistant(r.content.clone(), vec![]));
            msgs.push(Message::user(
                "A resposta acima nao e JSON valido. Responda de novo SO com o objeto JSON pedido.",
            ));
        } else {
            return Err(format!(
                "o modelo nao devolveu JSON em duas tentativas: {}",
                r.content.chars().take(200).collect::<String>()
            ));
        }
    }
    unreachable!("o laco devolve nas duas tentativas")
}

/// `code_review` (capacidade `code.review`): revisa o diff do repositorio da pasta (git de
/// verdade, no sandbox) ou um diff dado em texto (o `pr_diff` do GitHub/GitLab).
pub struct CodeReviewTool {
    /// O modelo da montagem, ja envolvido pelo `orcamento::LlmDaTarefa`: as ate duas
    /// chamadas do `revisar` cobram a conta da tarefa. A CLI (`phxclaw revisar`) chama sem
    /// tarefa nenhuma, e ali nao ha conta a cobrar.
    pub llm: Arc<dyn Llm>,
    /// Sem bwrap, so o diff em texto e aceito.
    pub bwrap: Option<PathBuf>,
}

impl Tool for CodeReviewTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "code_review".into(),
            description: "Review a code diff and return structured findings (file, line, \
severity critica|alta|media|baixa|info, finding, suggestion). Either 'diff' (unified diff \
text, e.g. from github/gitlab pr_diff) or a git repo in the task directory: path?, rev? (e.g. \
main..HEAD), cached?. Findings outside the diff are discarded and counted. Optional focus."
                .into(),
            parameters: json!({"type":"object","properties":{
                "diff":{"type":"string"},
                "path":{"type":"string"},
                "rev":{"type":"string"},
                "cached":{"type":"boolean"},
                "focus":{"type":"string"}
            }}),
        }
    }
    fn capability(&self) -> &'static str {
        "code.review"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let diff = match args.get("diff").and_then(Value::as_str) {
                Some(d) if !d.trim().is_empty() => d.to_string(),
                _ => {
                    let bwrap = self.bwrap.as_ref().ok_or_else(|| {
                        ToolError::Failed("sem sandbox para rodar o git; mande 'diff'".into())
                    })?;
                    let repo = crate::sistema::caminho_do_projeto(
                        &ctx.workdir,
                        args.get("path").and_then(Value::as_str).unwrap_or(""),
                    )?;
                    diff_do_repo(
                        bwrap,
                        &ctx.workdir,
                        &repo,
                        args.get("rev")
                            .and_then(Value::as_str)
                            .filter(|s| !s.trim().is_empty()),
                        args.get("cached").and_then(Value::as_bool).unwrap_or(false),
                        &[],
                        ctx.timeout.min(Duration::from_secs(60)),
                    )
                    .await?
                }
            };
            let foco = args.get("focus").and_then(Value::as_str);
            let r = revisar(self.llm.as_ref(), &diff, foco)
                .await
                .map_err(ToolError::Failed)?;
            Ok(ToolOutput::text(
                serde_json::to_string(&r).map_err(|e| ToolError::Failed(e.to_string()))?,
            ))
        })
    }
}

/// De onde vem o diff da `phxclaw revisar`.
pub enum FonteDoDiff {
    /// Repositorio git (pasta do hospedeiro), pelo mesmo `diff_do_repo` da ferramenta.
    Repo {
        pasta: PathBuf,
        rev: Option<String>,
        cached: bool,
    },
    /// Arquivo com diff unificado (`-` le a entrada padrao).
    Arquivo(PathBuf),
    /// PR/MR da forja com token guardado em `<raiz>/forja`.
    Pr {
        raiz_do_agente: PathBuf,
        forja: crate::forja::Forja,
        repo: String,
        numero: u64,
    },
}

/// `github:dono/proj#7` ou `gitlab:grupo/proj#3`.
pub fn analisar_pr(s: &str) -> Result<(crate::forja::Forja, String, u64), String> {
    let (f, resto) = s
        .split_once(':')
        .ok_or("formato: github:dono/projeto#numero")?;
    let forja = crate::forja::Forja::de_nome(f).ok_or(format!("forja desconhecida: {f}"))?;
    let (repo, n) = resto
        .rsplit_once('#')
        .ok_or("falta #numero (github:dono/projeto#7)")?;
    let n = n.parse().map_err(|_| format!("numero invalido: {n}"))?;
    let repo = crate::forja::projeto_valido(repo).map_err(|e| e.to_string())?;
    Ok((forja, repo, n))
}

/// O que a CLI chama: le o diff da fonte e passa pelo MESMO `revisar` da ferramenta.
pub async fn revisar_da_fonte(
    llm: &dyn Llm,
    fonte: FonteDoDiff,
    foco: Option<&str>,
) -> Result<Revisao, String> {
    let diff = match fonte {
        FonteDoDiff::Arquivo(p) if p.as_os_str() == "-" => {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
                .map_err(|e| e.to_string())?;
            s
        }
        FonteDoDiff::Arquivo(p) => {
            std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?
        }
        FonteDoDiff::Repo { pasta, rev, cached } => {
            let bwrap = crate::arquivos::achar_bwrap()
                .ok_or("sem bwrap: o git do agente so roda no sandbox; use --diff ARQ")?;
            diff_do_repo(
                &bwrap,
                &pasta,
                "",
                rev.as_deref(),
                cached,
                &[],
                Duration::from_secs(120),
            )
            .await
            .map_err(|e| e.to_string())?
        }
        FonteDoDiff::Pr {
            raiz_do_agente,
            forja,
            repo,
            numero,
        } => {
            let c = crate::forja::ForjaCliente::da_pasta(&raiz_do_agente, forja).ok_or(format!(
                "sem token de {} em {}: rode `phxclaw forja token {}`",
                forja.nome(),
                crate::forja::pasta_da_forja(&raiz_do_agente).display(),
                forja.nome()
            ))?;
            c.diff_de_pr(&repo, numero)
                .await
                .map_err(|e| e.to_string())?
        }
    };
    revisar(llm, &diff, foco).await
}

/// O modelo da CLI, pelo mesmo `chaves::modelo` da montagem: a chave sai do broker da
/// raiz do agente.
pub fn modelo(spec: &str, raiz_do_agente: &std::path::Path) -> Result<Arc<dyn Llm>, String> {
    crate::chaves::modelo(spec, raiz_do_agente)
}
