//! O PhxClaw numa GitHub Action, sem interface: o evento do PR vira `phxclaw revisar` e a
//! revisao vira um comentario no PR.
//!
//! Nada aqui e um segundo revisor nem um segundo cliente do GitHub: o diff e a revisao sao
//! os do `revisao::revisar_da_fonte` (o mesmo do `code_review` e da CLI), e o comentario sai
//! pelo `ForjaCliente::escrever("comment")` da ferramenta `github_write`, com o token lido do
//! broker por concessao curta. A acao composta (`.github/actions/phxclaw/action.yml`) so
//! guarda o token no broker (`phxclaw forja token github`) e chama
//! `phxclaw revisar --evento "$GITHUB_EVENT_PATH" --comentar`.
//!
//! Que eventos revisam: `pull_request` e `pull_request_target` com `opened`, `reopened`,
//! `synchronize` ou `ready_for_review`, e PR que nao e rascunho. O resto (PR fechado,
//! rotulado, push) devolve «nada a revisar» e sai 0: uma Action que falha em evento que
//! nao lhe diz respeito ensina o operador a ignorar o vermelho dela.

use crate::forja::{Forja, ForjaCliente, pasta_da_forja, projeto_valido};
use crate::revisao::{FonteDoDiff, Revisao, revisar_da_fonte};
use phxclaw_agent_core::Llm;
use serde_json::{Value, json};
use std::path::Path;

const ACOES_QUE_REVISAM: &[&str] = &["opened", "reopened", "synchronize", "ready_for_review"];
/// Teto do comentario: o GitHub recusa corpo acima de 65.536 caracteres.
const MAX_COMENTARIO: usize = 60_000;

/// O PR que o evento pede para revisar, ou `None` quando o evento nao e de revisao.
pub fn pr_do_evento(
    evento: &Value,
    nome_do_evento: Option<&str>,
) -> Result<Option<(String, u64)>, String> {
    if let Some(n) = nome_do_evento
        && !matches!(n, "pull_request" | "pull_request_target")
    {
        return Ok(None);
    }
    let Some(pr) = evento.get("pull_request") else {
        return Ok(None);
    };
    let acao = evento.get("action").and_then(Value::as_str).unwrap_or("");
    if !ACOES_QUE_REVISAM.contains(&acao) || pr.get("draft").and_then(Value::as_bool) == Some(true)
    {
        return Ok(None);
    }
    let numero = pr
        .get("number")
        .or_else(|| evento.get("number"))
        .and_then(Value::as_u64)
        .ok_or("evento de PR sem numero")?;
    let repo = evento
        .pointer("/repository/full_name")
        .and_then(Value::as_str)
        .ok_or("evento de PR sem repository.full_name")?;
    let repo = projeto_valido(repo).map_err(|e| e.to_string())?;
    Ok(Some((repo, numero)))
}

/// Celula de tabela Markdown: `|` e quebra de linha quebrariam a tabela.
fn celula(s: &str) -> String {
    s.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// O comentario do PR, em Markdown. Diz quantos achados a conferencia descartou: o leitor
/// precisa saber que o modelo disse mais do que aparece, e por que nao aparece.
pub fn comentario(r: &Revisao) -> String {
    let mut s = String::from("### Revisao do PhxClaw\n\n");
    if !r.resumo.trim().is_empty() {
        s.push_str(r.resumo.trim());
        s.push_str("\n\n");
    }
    if r.achados.is_empty() {
        s.push_str("Nenhum achado.\n");
    } else {
        s.push_str("| Severidade | Onde | Achado | Sugestao |\n|---|---|---|---|\n");
        for a in &r.achados {
            let onde = match a.linha {
                Some(l) => format!("`{}:{l}`", a.arquivo),
                None => format!("`{}`", a.arquivo),
            };
            s.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                a.severidade,
                celula(&onde),
                celula(&a.achado),
                celula(a.sugestao.as_deref().unwrap_or(""))
            ));
        }
    }
    let mut notas = vec![format!("{} arquivo(s)", r.arquivos)];
    if !r.descartados.is_empty() {
        notas.push(format!(
            "{} achado(s) do modelo descartado(s) pela conferencia de arquivo/linha/severidade",
            r.descartados.len()
        ));
    }
    if r.truncado {
        notas.push("diff cortado no teto: so o comeco foi revisado".into());
    }
    s.push_str(&format!("\n<sub>{}</sub>\n", notas.join(" · ")));
    if s.chars().count() > MAX_COMENTARIO {
        s = s.chars().take(MAX_COMENTARIO).collect::<String>() + "\n\n(comentario cortado)\n";
    }
    s
}

/// Publica a revisao como comentario do PR, pela mesma escrita da `github_write`.
pub async fn comentar(
    raiz_do_agente: &Path,
    forja: Forja,
    repo: &str,
    numero: u64,
    r: &Revisao,
) -> Result<Value, String> {
    let c = ForjaCliente::da_pasta(raiz_do_agente, forja).ok_or(format!(
        "sem token de {} em {}: rode `phxclaw forja token {}`",
        forja.nome(),
        pasta_da_forja(raiz_do_agente).display(),
        forja.nome()
    ))?;
    c.escrever(
        "comment",
        &json!({"repo": repo, "number": numero, "on": "pr", "body": comentario(r)}),
    )
    .await
    .map_err(|e| e.to_string())
}

/// O que a Action faz: le o evento, revisa o PR e, com `comentar`, publica. `None` quando o
/// evento nao pede revisao.
pub async fn rodar(
    raiz_do_agente: &Path,
    caminho_do_evento: &Path,
    nome_do_evento: Option<&str>,
    llm: &dyn Llm,
    foco: Option<&str>,
    publicar: bool,
) -> Result<Option<Revisao>, String> {
    let evento: Value = serde_json::from_slice(
        &std::fs::read(caminho_do_evento)
            .map_err(|e| format!("{}: {e}", caminho_do_evento.display()))?,
    )
    .map_err(|e| format!("{}: {e}", caminho_do_evento.display()))?;
    let Some((repo, numero)) = pr_do_evento(&evento, nome_do_evento)? else {
        return Ok(None);
    };
    let r = revisar_da_fonte(
        llm,
        FonteDoDiff::Pr {
            raiz_do_agente: raiz_do_agente.to_path_buf(),
            forja: Forja::Github,
            repo: repo.clone(),
            numero,
        },
        foco,
    )
    .await?;
    if publicar {
        comentar(raiz_do_agente, Forja::Github, &repo, numero, &r).await?;
    }
    Ok(Some(r))
}
