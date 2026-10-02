//! `glob` e `grep` na pasta da tarefa, respeitando o `.gitignore`.
//!
//! O `list_files` continua sendo o inventario da pasta (tudo, com tamanho); estes dois
//! respondem outra pergunta -- «onde esta» --, e por isso pulam o que o projeto declarou
//! ignorado: o modelo que acha o mesmo simbolo em `target/` mil vezes gasta o contexto
//! com copia gerada.
//!
//! O `.gitignore` e lido aqui, sem crate nova: o subconjunto que repositorio real usa
//! (`*`, `**`, `?`, `[...]`, `/` ancorado, `/` final so pasta, `!` negando, um arquivo por
//! pasta, mais o `.git/info/exclude`). A ultima regra que casa decide, como no git; pasta
//! ignorada nao e nem visitada, e por isso negar um arquivo dentro dela nao o traz de volta
//! -- o git faz igual.

use crate::tarefa::confine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use regex::RegexBuilder;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Teto de entradas visitadas: uma pasta com um `node_modules` nao ignorado nao trava o
/// passo do agente.
const MAX_VISITAS: usize = 200_000;
/// Arquivo maior que isto nao entra no grep (log, dump): seria lido inteiro por linha.
const MAX_BYTES_GREP: u64 = 5 * 1024 * 1024;
const MAX_CHARS_LINHA: usize = 400;

/// Casa um glob com um texto. `*` e `?` nao atravessam `/`; `**` atravessa, e `**/` tambem
/// casa zero pastas.
pub fn glob_casa(padrao: &str, texto: &str) -> bool {
    expandir_chaves(padrao)
        .iter()
        .any(|p| casa(p.as_bytes(), texto.as_bytes()))
}

/// `{a,b}` vira dois padroes (o `.gitignore` nao tem chaves; o `glob` do modelo usa muito).
fn expandir_chaves(p: &str) -> Vec<String> {
    let Some(ini) = p.find('{') else {
        return vec![p.to_string()];
    };
    let Some(fim) = p[ini..].find('}').map(|f| ini + f) else {
        return vec![p.to_string()];
    };
    p[ini + 1..fim]
        .split(',')
        .flat_map(|alt| expandir_chaves(&format!("{}{alt}{}", &p[..ini], &p[fim + 1..])))
        .collect()
}

fn casa(p: &[u8], t: &[u8]) -> bool {
    if p.is_empty() {
        return t.is_empty();
    }
    if p.starts_with(b"**") {
        let resto = &p[2..];
        // `**/x`: zero ou mais pastas antes de x.
        if let Some(depois) = resto.strip_prefix(b"/") {
            if casa(depois, t) {
                return true;
            }
            return (0..t.len()).any(|i| t[i] == b'/' && casa(depois, &t[i + 1..]));
        }
        return (0..=t.len()).any(|i| casa(resto, &t[i..]));
    }
    match p[0] {
        b'*' => (0..=t.len())
            .take_while(|&i| i == 0 || t[i - 1] != b'/')
            .any(|i| casa(&p[1..], &t[i..])),
        b'?' => !t.is_empty() && t[0] != b'/' && casa(&p[1..], &t[1..]),
        b'[' => {
            let Some(fim) = p.iter().skip(2).position(|&c| c == b']').map(|f| f + 2) else {
                return !t.is_empty() && t[0] == b'[' && casa(&p[1..], &t[1..]);
            };
            if t.is_empty() || t[0] == b'/' {
                return false;
            }
            let classe = &p[1..fim];
            let (negar, classe) = match classe.first() {
                Some(b'!' | b'^') => (true, &classe[1..]),
                _ => (false, classe),
            };
            let mut dentro = false;
            let mut i = 0;
            while i < classe.len() {
                if i + 2 < classe.len() && classe[i + 1] == b'-' {
                    dentro |= (classe[i]..=classe[i + 2]).contains(&t[0]);
                    i += 3;
                } else {
                    dentro |= classe[i] == t[0];
                    i += 1;
                }
            }
            dentro != negar && casa(&p[fim + 1..], &t[1..])
        }
        b'\\' if p.len() > 1 => !t.is_empty() && t[0] == p[1] && casa(&p[2..], &t[1..]),
        c => !t.is_empty() && t[0] == c && casa(&p[1..], &t[1..]),
    }
}

#[derive(Debug, Clone)]
struct Regra {
    /// Pasta (relativa a raiz da busca) do `.gitignore` que declarou a regra.
    base: String,
    padrao: String,
    negar: bool,
    so_pasta: bool,
    /// Com `/` no meio ou no inicio: casa o caminho a partir da base, nao so o nome.
    ancorada: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Ignorados {
    regras: Vec<Regra>,
}

impl Ignorados {
    /// Acrescenta as linhas de um `.gitignore` que mora em `base`.
    pub fn ler(&mut self, base: &str, texto: &str) {
        for l in texto.lines() {
            let l = l.trim_end();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let (negar, l) = match l.strip_prefix('!') {
                Some(r) => (true, r),
                None => (false, l.strip_prefix('\\').unwrap_or(l)),
            };
            let (so_pasta, l) = match l.strip_suffix('/') {
                Some(r) => (true, r),
                None => (false, l),
            };
            let ancorada = l.contains('/');
            let padrao = l.trim_start_matches('/').to_string();
            if padrao.is_empty() {
                continue;
            }
            self.regras.push(Regra {
                base: base.to_string(),
                padrao,
                negar,
                so_pasta,
                ancorada,
            });
        }
    }

    /// `rel` relativo a raiz da busca, com `/`.
    pub fn ignorado(&self, rel: &str, pasta: bool) -> bool {
        let mut r = false;
        for g in &self.regras {
            if g.so_pasta && !pasta {
                continue;
            }
            let sub = if g.base.is_empty() {
                rel
            } else {
                match rel.strip_prefix(&g.base).and_then(|s| s.strip_prefix('/')) {
                    Some(s) => s,
                    None => continue,
                }
            };
            let alvo = if g.ancorada {
                sub
            } else {
                sub.rsplit('/').next().unwrap_or(sub)
            };
            if glob_casa(&g.padrao, alvo) {
                r = !g.negar;
            }
        }
        r
    }
}

/// Arquivos debaixo de `raiz` (caminhos relativos, com `/`), sem `.git` e sem o que o
/// `.gitignore` manda pular (salvo `incluir_ignorados`). Devolve tambem se bateu no teto.
pub fn percorrer(raiz: &Path, incluir_ignorados: bool) -> (Vec<String>, bool) {
    let mut ign = Ignorados::default();
    if let Ok(t) = std::fs::read_to_string(raiz.join(".git/info/exclude")) {
        ign.ler("", &t);
    }
    let mut saida = Vec::new();
    let mut pilha = vec![(raiz.to_path_buf(), String::new())];
    let mut visitas = 0usize;
    while let Some((dir, rel)) = pilha.pop() {
        if !incluir_ignorados && let Ok(t) = std::fs::read_to_string(dir.join(".gitignore")) {
            ign.ler(&rel, &t);
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entradas: Vec<_> = rd.flatten().collect();
        entradas.sort_by_key(|e| e.file_name());
        for e in entradas {
            visitas += 1;
            if visitas > MAX_VISITAS {
                return (saida, true);
            }
            let nome = e.file_name().to_string_lossy().into_owned();
            if nome == ".git" {
                continue;
            }
            let r = if rel.is_empty() {
                nome
            } else {
                format!("{rel}/{nome}")
            };
            // Link simbolico nao e seguido: apontaria para fora da pasta da tarefa.
            let Ok(tipo) = e.file_type() else { continue };
            if tipo.is_symlink() {
                continue;
            }
            let pasta = tipo.is_dir();
            if !incluir_ignorados && ign.ignorado(&r, pasta) {
                continue;
            }
            if pasta {
                pilha.push((e.path(), r));
            } else {
                saida.push(r);
            }
        }
    }
    saida.sort();
    (saida, false)
}

fn raiz_da_busca(ctx: &ToolContext, args: &Value) -> Result<(PathBuf, String), ToolError> {
    let rel = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "." && *s != "/work")
        .unwrap_or("");
    if rel.is_empty() {
        return Ok((ctx.workdir.clone(), String::new()));
    }
    let p = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
    let rel = rel
        .strip_prefix("/work/")
        .unwrap_or(rel)
        .trim_end_matches('/');
    Ok((p, rel.to_string()))
}

fn com_prefixo(prefixo: &str, r: &str) -> String {
    if prefixo.is_empty() {
        r.to_string()
    } else {
        format!("{prefixo}/{r}")
    }
}

fn limite(args: &Value, padrao: u64) -> usize {
    args.get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(padrao)
        .clamp(1, 2000) as usize
}

/// O filtro de nome do `glob`/`grep`: padrao sem `/` casa o nome do arquivo em qualquer
/// pasta (`*.rs`), com `/` casa o caminho (`src/**/*.rs`).
fn casa_filtro(filtro: &str, rel: &str) -> bool {
    if filtro.contains('/') {
        glob_casa(filtro, rel)
    } else {
        glob_casa(filtro, rel.rsplit('/').next().unwrap_or(rel))
    }
}

pub struct GlobTool;

impl Tool for GlobTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "glob".into(),
            description: "Find files by name pattern in the task directory, skipping what \
.gitignore ignores. pattern: '*.rs' (any folder), 'src/**/*.{ts,tsx}' (path). Newest first. \
Optional path (subfolder), limit, include_ignored."
                .into(),
            parameters: json!({"type":"object","properties":{
                "pattern":{"type":"string"},
                "path":{"type":"string"},
                "limit":{"type":"integer"},
                "include_ignored":{"type":"boolean"}
            },"required":["pattern"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let padrao = args
                .get("pattern")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'pattern'".into()))?
                .trim()
                .trim_start_matches("./")
                .to_string();
            let (raiz, prefixo) = raiz_da_busca(ctx, &args)?;
            let todos = args
                .get("include_ignored")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let max = limite(&args, 200);
            let (lista, teto) = tokio::task::spawn_blocking(move || {
                let (arqs, teto) = percorrer(&raiz, todos);
                let mut l: Vec<(std::time::SystemTime, String)> = arqs
                    .into_iter()
                    .filter(|r| casa_filtro(&padrao, r))
                    .map(|r| {
                        let m = std::fs::metadata(raiz.join(&r))
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::UNIX_EPOCH);
                        (m, r)
                    })
                    .collect();
                // Mais novo primeiro: o arquivo que o agente acabou de mexer e o que ele
                // mais provavelmente procura.
                l.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                (l, teto)
            })
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
            let total = lista.len();
            let arquivos: Vec<String> = lista
                .into_iter()
                .take(max)
                .map(|(_, r)| com_prefixo(&prefixo, &r))
                .collect();
            Ok(ToolOutput::text(
                json!({
                    "arquivos": arquivos,
                    "total": total,
                    "truncado": total > max,
                    "teto_de_visitas": teto,
                })
                .to_string(),
            ))
        })
    }
}

pub struct GrepTool;

impl Tool for GrepTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".into(),
            description: "Search file contents with a regex in the task directory, skipping \
.gitignore'd and binary files. output_mode: content (default; file, line, text, with \
'context' lines around), files (only names), count. Optional glob ('*.rs'), path, \
case_insensitive, limit."
                .into(),
            parameters: json!({"type":"object","properties":{
                "pattern":{"type":"string"},
                "path":{"type":"string"},
                "glob":{"type":"string"},
                "output_mode":{"type":"string","enum":["content","files","count"]},
                "context":{"type":"integer"},
                "case_insensitive":{"type":"boolean"},
                "limit":{"type":"integer"},
                "include_ignored":{"type":"boolean"}
            },"required":["pattern"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let padrao = args
                .get("pattern")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'pattern'".into()))?;
            let re = RegexBuilder::new(padrao)
                .case_insensitive(
                    args.get("case_insensitive")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                // Teto do automato: regex do modelo nao vira consumo de memoria sem fim.
                .size_limit(10 * 1024 * 1024)
                .build()
                .map_err(|e| ToolError::InvalidArguments(format!("regex invalida: {e}")))?;
            let modo = args
                .get("output_mode")
                .and_then(Value::as_str)
                .unwrap_or("content")
                .to_string();
            if !matches!(modo.as_str(), "content" | "files" | "count") {
                return Err(ToolError::InvalidArguments(format!(
                    "output_mode desconhecido: {modo} (content, files, count)"
                )));
            }
            let ctxl = args
                .get("context")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(10) as usize;
            let filtro = args.get("glob").and_then(Value::as_str).map(str::to_string);
            let todos = args
                .get("include_ignored")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let max = limite(&args, 200);
            let (raiz, prefixo) = raiz_da_busca(ctx, &args)?;
            let v = tokio::task::spawn_blocking(move || {
                let (arqs, teto) = percorrer(&raiz, todos);
                let mut achados = Vec::new();
                let mut arquivos = Vec::new();
                let mut contagem = Vec::new();
                let mut total = 0usize;
                for r in arqs {
                    if filtro.as_deref().is_some_and(|f| !casa_filtro(f, &r)) {
                        continue;
                    }
                    let p = raiz.join(&r);
                    if std::fs::metadata(&p).map(|m| m.len() > MAX_BYTES_GREP).unwrap_or(true) {
                        continue;
                    }
                    let Ok(b) = std::fs::read(&p) else { continue };
                    // Binario (NUL nos primeiros 8 KiB, o criterio do git): fora.
                    if b.iter().take(8192).any(|&c| c == 0) {
                        continue;
                    }
                    let t = String::from_utf8_lossy(&b);
                    let linhas: Vec<&str> = t.lines().collect();
                    let casadas: Vec<usize> = linhas
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| re.is_match(l))
                        .map(|(i, _)| i)
                        .collect();
                    if casadas.is_empty() {
                        continue;
                    }
                    let nome = com_prefixo(&prefixo, &r);
                    total += casadas.len();
                    match modo.as_str() {
                        "files" => arquivos.push(nome),
                        "count" => contagem.push(json!({"arquivo": nome, "ocorrencias": casadas.len()})),
                        _ => {
                            for i in casadas {
                                if achados.len() >= max {
                                    break;
                                }
                                let corta = |s: &str| s.chars().take(MAX_CHARS_LINHA).collect::<String>();
                                let mut a = json!({"arquivo": nome, "linha": i + 1, "texto": corta(linhas[i])});
                                if ctxl > 0 {
                                    let ini = i.saturating_sub(ctxl);
                                    let fim = (i + ctxl + 1).min(linhas.len());
                                    a["antes"] = json!(linhas[ini..i].iter().map(|l| corta(l)).collect::<Vec<_>>());
                                    a["depois"] = json!(linhas[i + 1..fim].iter().map(|l| corta(l)).collect::<Vec<_>>());
                                }
                                achados.push(a);
                            }
                        }
                    }
                }
                let corpo = match modo.as_str() {
                    "files" => {
                        let n = arquivos.len();
                        arquivos.truncate(max);
                        json!({"arquivos": arquivos, "total_arquivos": n, "truncado": n > max})
                    }
                    "count" => {
                        let n = contagem.len();
                        contagem.truncate(max);
                        json!({"contagem": contagem, "total_arquivos": n, "truncado": n > max})
                    }
                    _ => json!({"achados": achados, "truncado": total > achados.len()}),
                };
                let mut corpo = corpo;
                corpo["total_ocorrencias"] = json!(total);
                corpo["teto_de_visitas"] = json!(teto);
                corpo
            })
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
            Ok(ToolOutput::text(v.to_string()))
        })
    }
}

/// Teto de arquivos que uma substituicao muda de uma vez: acima disso o pedido e amplo
/// demais para ter sido lido na previa.
const MAX_ARQUIVOS_SUBSTITUIR: usize = 500;

/// Os arquivos que um padrao casa e quantas ocorrencias em cada um: a previa e a gravacao
/// saem DESTA lista, para que o que se grava seja exatamente o que a previa mostrou.
fn ocorrencias_no_projeto(
    raiz: &Path,
    re: &regex::Regex,
    filtro: Option<&str>,
    todos: bool,
) -> (Vec<(String, String, usize)>, bool) {
    let (arqs, teto) = percorrer(raiz, todos);
    let mut v = Vec::new();
    for r in arqs {
        if filtro.is_some_and(|f| !casa_filtro(f, &r)) {
            continue;
        }
        let p = raiz.join(&r);
        if std::fs::metadata(&p)
            .map(|m| m.len() > MAX_BYTES_GREP)
            .unwrap_or(true)
        {
            continue;
        }
        let Ok(b) = std::fs::read(&p) else { continue };
        if b.iter().take(8192).any(|&c| c == 0) {
            continue;
        }
        // So UTF-8 valido se reescreve: `from_utf8_lossy` gravaria `U+FFFD` onde o byte
        // original estava, mudando o que o padrao nem tocou.
        let Ok(t) = String::from_utf8(b) else {
            continue;
        };
        let n = re.find_iter(&t).count();
        if n > 0 {
            v.push((r, t, n));
        }
    }
    (v, teto)
}

/// Substituicao em todos os arquivos do projeto, pelo mesmo motor do `grep` (a mesma
/// varredura, o mesmo `.gitignore`, o mesmo criterio de binario) e gravada pelo mesmo
/// caminho do `edit_file` (`confine` + `std::fs::write`, artefato com hash). Sem
/// `confirm: true` e so previa: a contagem por arquivo, e nada muda. A capacidade e
/// `fs.write`, entao o portao do motor tira o ponto de restauracao antes da gravacao.
pub struct ReplaceInProjectTool;

impl Tool for ReplaceInProjectTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "replace_in_project".into(),
            description: "Replace a pattern in every matching file of the task directory \
(skips .gitignore'd and binary files). pattern is literal unless regex=true (then \
replacement may use $1 groups). Optional glob ('*.rs'), path (subfolder), \
case_insensitive. confirm=false (default) only previews: files and count per file, \
nothing is written. confirm=true writes; a checkpoint is taken before."
                .into(),
            parameters: json!({"type":"object","properties":{
                "pattern":{"type":"string"},
                "replacement":{"type":"string"},
                "regex":{"type":"boolean"},
                "glob":{"type":"string"},
                "path":{"type":"string"},
                "case_insensitive":{"type":"boolean"},
                "confirm":{"type":"boolean"},
                "include_ignored":{"type":"boolean"}
            },"required":["pattern","replacement"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let padrao = args
                .get("pattern")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("falta 'pattern'".into()))?;
            let troca = args
                .get("replacement")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'replacement'".into()))?
                .to_string();
            let eh_regex = args.get("regex").and_then(Value::as_bool).unwrap_or(false);
            let confirmar = args
                .get("confirm")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let fonte = if eh_regex {
                padrao.to_string()
            } else {
                regex::escape(padrao)
            };
            let re = RegexBuilder::new(&fonte)
                .case_insensitive(
                    args.get("case_insensitive")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                .size_limit(10 * 1024 * 1024)
                .build()
                .map_err(|e| ToolError::InvalidArguments(format!("regex invalida: {e}")))?;
            // Literal: o `$` do texto de troca e texto, nao grupo.
            let troca = if eh_regex {
                troca
            } else {
                troca.replace('$', "$$")
            };
            let filtro = args.get("glob").and_then(Value::as_str).map(str::to_string);
            let todos = args
                .get("include_ignored")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let (raiz, prefixo) = raiz_da_busca(ctx, &args)?;
            let workdir = ctx.workdir.clone();
            let v = tokio::task::spawn_blocking(move || -> Result<(Value, Vec<phxclaw_agent_core::Artifact>), ToolError> {
                let (lista, teto) = ocorrencias_no_projeto(&raiz, &re, filtro.as_deref(), todos);
                let total: usize = lista.iter().map(|(_, _, n)| n).sum();
                let arquivos: Vec<Value> = lista
                    .iter()
                    .map(|(r, _, n)| json!({"arquivo": com_prefixo(&prefixo, r), "ocorrencias": n}))
                    .collect();
                if !confirmar {
                    return Ok((
                        json!({"previa": true, "arquivos": arquivos, "total": total,
                               "teto_de_visitas": teto,
                               "aviso": "nada foi gravado; repita com confirm=true para gravar"}),
                        vec![],
                    ));
                }
                if lista.len() > MAX_ARQUIVOS_SUBSTITUIR {
                    return Err(ToolError::InvalidArguments(format!(
                        "o padrao casa em {} arquivos, acima do teto de {MAX_ARQUIVOS_SUBSTITUIR}; \
                         restrinja com glob ou path",
                        lista.len()
                    )));
                }
                let mut artefatos = Vec::new();
                for (r, texto, _) in &lista {
                    let rel = com_prefixo(&prefixo, r);
                    let alvo = confine(&workdir, &rel).map_err(ToolError::Denied)?;
                    let novo = re.replace_all(texto, troca.as_str());
                    std::fs::write(&alvo, novo.as_bytes())
                        .map_err(|e| ToolError::Failed(format!("{rel}: {e}")))?;
                    artefatos.push(
                        crate::motor::artifact_for(&workdir, &rel)
                            .map_err(|e| ToolError::Failed(e.to_string()))?,
                    );
                }
                Ok((
                    json!({"previa": false, "arquivos": arquivos, "total": total,
                           "gravados": lista.len(), "teto_de_visitas": teto}),
                    artefatos,
                ))
            })
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))??;
            Ok(ToolOutput {
                content: v.0.to_string(),
                artifacts: v.1,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_estrela_nao_atravessa_pasta_e_duas_estrelas_sim() {
        assert!(glob_casa("*.rs", "a.rs"));
        assert!(!glob_casa("*.rs", "src/a.rs"));
        assert!(glob_casa("src/**/*.rs", "src/a.rs"));
        assert!(glob_casa("src/**/*.rs", "src/x/y/a.rs"));
        assert!(glob_casa("**/*.{rs,toml}", "Cargo.toml"));
        assert!(glob_casa("a?[0-9].txt", "ab7.txt"));
        assert!(!glob_casa("a?[!0-9].txt", "ab7.txt"));
        assert!(!glob_casa("src/*", "src/x/a.rs"));
    }

    #[test]
    fn gitignore_ultima_regra_ganha_e_ancoragem_respeitada() {
        let mut g = Ignorados::default();
        g.ler("", "target/\n*.log\n!importante.log\n/raiz.txt\n");
        g.ler("sub", "local.txt\n");
        assert!(g.ignorado("target", true));
        assert!(
            !g.ignorado("target", false),
            "so_pasta: arquivo chamado target fica"
        );
        assert!(g.ignorado("x/y.log", false));
        assert!(!g.ignorado("x/importante.log", false));
        assert!(g.ignorado("raiz.txt", false));
        assert!(!g.ignorado("x/raiz.txt", false), "ancorado na raiz");
        assert!(g.ignorado("sub/local.txt", false));
        assert!(
            !g.ignorado("local.txt", false),
            "regra de sub nao vale fora dela"
        );
    }

    /// Prova real (papel F, 01/10/2026): a regra de um `.gitignore` aninhado vale so
    /// debaixo da pasta dele. Dois mutantes que a bateria anterior deixava vivos:
    /// (a) o `percorrer` ler o `.gitignore` de `z/` com base vazia -- a regra vaza para as
    /// irmas; (b) o prefixo da base sem a fronteira `/` -- a regra de `sub` alcanca `subway`.
    /// A ordem importa: a pilha visita a ULTIMA irma primeiro, entao o `.gitignore` mora em
    /// `z/`, que e lida antes de `a/` e `m/`. Com ele em `a/`, as irmas seriam visitadas
    /// antes da regra existir e o mutante (a) passaria.
    #[test]
    fn gitignore_aninhado_nao_vaza_para_as_irmas() {
        let d =
            std::env::temp_dir().join(format!("phx-busca-irmas-{}", phxclaw_types::new_uuid_v7()));
        for (p, t) in [
            ("z/.gitignore", "gerado.rs\n"),
            ("z/gerado.rs", ""),
            ("z/q/gerado.rs", ""),
            ("a/gerado.rs", ""),
            ("m/gerado.rs", ""),
        ] {
            let f = d.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, t).unwrap();
        }
        let (v, cortou) = percorrer(&d, false);
        assert!(!cortou);
        assert_eq!(v, ["a/gerado.rs", "m/gerado.rs", "z/.gitignore"]);
        let _ = std::fs::remove_dir_all(&d);

        let mut g = Ignorados::default();
        g.ler("sub", "local.txt\n");
        assert!(g.ignorado("sub/local.txt", false));
        assert!(
            !g.ignorado("subway/local.txt", false),
            "regra de sub alcancou subway"
        );
    }
}
