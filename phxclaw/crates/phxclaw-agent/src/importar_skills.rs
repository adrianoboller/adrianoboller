//! Importador de skills de outros agentes: `phxclaw skills importar DIR`.
//!
//! Le todo `SKILL.md` abaixo de `DIR` (o formato do Claude Code, do Codex, do OpenClaw e do
//! Hermes: cabecalho YAML entre `---` e corpo em Markdown) e grava cada um na pasta de
//! skills do agente, no formato que o `SkillFolder` le. As decisoes:
//!
//! - **O cabecalho e YAML, lido a mao, so no que importa.** Nao ha crate de YAML no
//!   Cargo.lock, e o que se usa do cabecalho e pouco: `name`, `description` e
//!   `allowed-tools`. O leitor entende escalar com e sem aspas, bloco `|`/`>`, lista em
//!   linha (`[a, b]`) e em bloco (`- a`); mapa aninhado (`metadata:`) e pulado inteiro.
//!   O resto do YAML (ancora, tag, documento multiplo) nao aparece nos 350 do corpus.
//! - **`scripts/` fica DESLIGADO por padrao.** Skill e texto que o modelo le; script e
//!   codigo de terceiro. Importar o texto nao pode virar rodar o codigo: os scripts nao se
//!   copiam, a lista deles vai para `ORIGEM.json`, e o corpo ganha uma nota dizendo que
//!   eles nao vieram. `--com-scripts` copia, e nem assim nada roda sozinho.
//! - **Origem com SHA-256.** `ORIGEM.json` guarda o caminho de origem e o hash do
//!   `SKILL.md` original: reimportar o mesmo arquivo nao duplica, e quem audita sabe de
//!   onde veio cada instrucao que o agente segue.
//! - **Nome de ferramenta conhecido se traduz** (`terminal`, `Bash` -> `shell`;
//!   `web_extract`, `WebFetch` -> `browser_open`...), so onde e nome de ferramenta: entre
//!   crases no corpo e na lista `allowed-tools`. Trocar a palavra solta «Read» no meio
//!   de uma frase seria reescrever a prosa de outra pessoa. O que nao tem par fica como
//!   veio e vai listado como nao traduzido.

use phxclaw_skill_runtime::{SKILL_DESCRIPTION_MAX_CHARS, SKILL_DOC_MAX_BYTES, SkillFolder};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Nomes de ferramenta dos quatro agentes de origem e o nosso equivalente.
pub const TRADUCOES: &[(&str, &str)] = &[
    // Claude Code
    ("Bash", "shell"),
    ("Read", "read_file"),
    ("Write", "write_file"),
    ("Edit", "edit_file"),
    ("MultiEdit", "edit_file"),
    ("Glob", "glob"),
    ("Grep", "grep"),
    ("LS", "list_files"),
    ("WebFetch", "browser_open"),
    ("WebSearch", "web_search"),
    ("Task", "team_delegate"),
    ("NotebookEdit", "notebook_edit"),
    // Hermes
    ("terminal", "shell"),
    ("process", "shell_bg"),
    ("execute_code", "python_repl"),
    ("patch", "edit_file"),
    ("search_files", "grep"),
    ("web_extract", "browser_open"),
    ("browser_navigate", "browser_open"),
    ("delegate_task", "team_delegate"),
    ("vision_analyze", "image"),
    ("clarify", "ask_user"),
    ("skill_view", "skill_load"),
    ("memory", "memory_save"),
    ("send_message", "channel_send"),
    ("message", "channel_send"),
    // Codex e OpenClaw
    ("exec", "shell"),
    ("apply_patch", "edit_file"),
    ("web_fetch", "browser_open"),
    ("browser", "browser_open"),
    ("read", "read_file"),
    ("write", "write_file"),
    ("edit", "edit_file"),
];

/// Os que ja tem o nosso nome passam sem traducao, e nao contam como «nao traduzidos».
const NOSSOS: &[&str] = &[
    "shell",
    "read_file",
    "write_file",
    "edit_file",
    "web_search",
    "image_generate",
    "session_search",
    "list_files",
    "grep",
    "glob",
];

fn traduzir_nome(n: &str) -> Option<&'static str> {
    TRADUCOES
        .iter()
        .find(|(de, _)| *de == n)
        .map(|(_, para)| *para)
}

// ---------------------------------------------------------------- YAML do cabecalho

#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Texto(String),
    Lista(Vec<String>),
    /// Mapa aninhado: o importador nao usa, so registra que existia.
    Mapa,
}

fn sem_aspas(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        // Escapes do YAML de aspas duplas que aparecem em descricao: \" \\ \n \t.
        let mut s = String::new();
        let mut it = v[1..v.len() - 1].chars();
        while let Some(c) = it.next() {
            if c == '\\' {
                match it.next() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some(o) => s.push(o),
                    None => {}
                }
            } else {
                s.push(c);
            }
        }
        s
    } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        v[1..v.len() - 1].replace("''", "'")
    } else {
        // Comentario no fim de escalar sem aspas.
        v.split(" #").next().unwrap_or(v).trim().to_string()
    }
}

fn lista_em_linha(v: &str) -> Vec<String> {
    v.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(sem_aspas)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Separa cabecalho e corpo. Sem `---` na primeira linha, nao ha cabecalho (o corpo e o
/// texto inteiro); com `---` sem fecho, e erro.
pub fn separar(texto: &str) -> Result<(Option<&str>, &str), String> {
    let t = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let Some(resto) = t
        .strip_prefix("---\n")
        .or_else(|| t.strip_prefix("---\r\n"))
    else {
        return Ok((None, t));
    };
    let mut pos = 0;
    for linha in resto.split_inclusive('\n') {
        if linha.trim_end() == "---" {
            return Ok((Some(&resto[..pos]), &resto[pos + linha.len()..]));
        }
        pos += linha.len();
    }
    Err("cabecalho --- sem fecho".into())
}

/// As chaves de primeiro nivel do cabecalho.
pub fn ler_cabecalho(cab: &str) -> BTreeMap<String, Valor> {
    let linhas: Vec<&str> = cab.lines().collect();
    let mut m = BTreeMap::new();
    let mut i = 0;
    let indentada = |l: &str| l.starts_with(' ') || l.starts_with('\t');
    while i < linhas.len() {
        let l = linhas[i];
        i += 1;
        if l.trim().is_empty() || l.trim_start().starts_with('#') || indentada(l) {
            continue;
        }
        let Some((k, v)) = l.split_once(':') else {
            continue;
        };
        let k = k.trim().to_string();
        let v = v.trim();
        // As linhas indentadas que seguem pertencem a esta chave.
        let ini = i;
        while i < linhas.len() && (linhas[i].trim().is_empty() || indentada(linhas[i])) {
            i += 1;
        }
        let filhas: Vec<&str> = linhas[ini..i].to_vec();
        let valor = if v.starts_with('|') || v.starts_with('>') {
            let dobrar = v.starts_with('>');
            let corpo: Vec<&str> = filhas.iter().map(|l| l.trim()).collect();
            let s = if dobrar {
                // `>` junta as linhas com espaco e mantem a linha em branco como quebra.
                corpo
                    .split(|l| l.is_empty())
                    .map(|p| p.join(" "))
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                corpo.join("\n").trim().to_string()
            };
            Valor::Texto(s)
        } else if v.starts_with('[') {
            // Lista em linha que pode continuar nas linhas seguintes ate o `]`.
            let mut s = v.to_string();
            for f in &filhas {
                s.push(' ');
                s.push_str(f.trim());
            }
            Valor::Lista(lista_em_linha(&s))
        } else if !v.is_empty() {
            // Escalar com continuacao indentada (texto longo partido) vira uma linha so.
            let mut s = sem_aspas(v);
            for f in filhas.iter().filter(|f| !f.trim().is_empty()) {
                s.push(' ');
                s.push_str(&sem_aspas(f));
            }
            Valor::Texto(s)
        } else if filhas.iter().any(|f| f.trim_start().starts_with("- ")) {
            Valor::Lista(
                filhas
                    .iter()
                    .filter_map(|f| f.trim_start().strip_prefix("- "))
                    .map(sem_aspas)
                    .collect(),
            )
        } else if filhas.iter().any(|f| !f.trim().is_empty()) {
            Valor::Mapa
        } else {
            Valor::Texto(String::new())
        };
        m.insert(k, valor);
    }
    m
}

// ---------------------------------------------------------------- importacao

#[derive(Debug, Clone, Default)]
pub struct Opcoes {
    /// Copia `scripts/` para a pasta da skill (desligado por padrao).
    pub com_scripts: bool,
}

/// O que vai para `ORIGEM.json` e para o relatorio.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Importada {
    pub nome: String,
    pub origem: PathBuf,
    pub sha256: String,
    /// O nome do cabecalho foi ajustado ao que o `SkillFolder` aceita (ou nao havia).
    pub nome_ajustado: bool,
    /// Sem cabecalho ou sem `description`: saiu do primeiro paragrafo do corpo.
    pub descricao_derivada: bool,
    /// A descricao passou de 300 caracteres e foi cortada (com `...`).
    pub descricao_cortada: bool,
    /// O corpo passou do teto de 64 KiB e foi cortado num titulo (`ORIGINAL.md` inteiro ao lado).
    pub corpo_cortado: bool,
    pub traducoes: BTreeMap<String, usize>,
    pub nao_traduzidas: Vec<String>,
    pub scripts_desligados: Vec<String>,
    pub scripts_copiados: bool,
}

#[derive(Debug, Default)]
pub struct Relatorio {
    pub achados: usize,
    pub importadas: Vec<Importada>,
    /// Mesmo SHA-256 ja importado: nao se duplica.
    pub repetidas: usize,
    pub recusadas: Vec<(PathBuf, String)>,
}

fn skill_mds(dir: &Path, v: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut es: Vec<_> = rd.flatten().collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        let Ok(m) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if m.is_dir() && e.file_name() != ".git" {
            skill_mds(&p, v);
        } else if m.is_file() && e.file_name() == "SKILL.md" {
            v.push(p);
        }
    }
}

/// Nome que o `SkillFolder` aceita: ASCII, digitos, `-` e `_`, ate 64.
pub fn nome_valido(bruto: &str) -> String {
    let mut s: String = bruto
        .trim()
        .chars()
        .map(phxclaw_memory_context::bm25::dobrar_acento)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-');
    let s: String = s.chars().take(64).collect();
    if s.is_empty() { "skill".into() } else { s }
}

/// Primeiro paragrafo de prosa do corpo (pula titulo, tabela, codigo).
fn primeiro_paragrafo(corpo: &str) -> Option<String> {
    corpo
        .split("\n\n")
        .map(str::trim)
        .find(|p| {
            !p.is_empty()
                && !p.starts_with('#')
                && !p.starts_with('|')
                && !p.starts_with("```")
                && !p.starts_with('<')
        })
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Troca `` `nome` `` conhecido pelo nosso; conta o que trocou e o que ficou.
pub fn traduzir_corpo(corpo: &str, contagem: &mut BTreeMap<String, usize>) -> String {
    let mut saida = String::with_capacity(corpo.len());
    let mut resto = corpo;
    while let Some(i) = resto.find('`') {
        saida.push_str(&resto[..i]);
        let depois = &resto[i + 1..];
        match depois.find('`') {
            Some(j)
                if j > 0
                    && depois[..j]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
            {
                let nome = &depois[..j];
                match traduzir_nome(nome) {
                    Some(nosso) if nosso != nome => {
                        *contagem.entry(format!("{nome}->{nosso}")).or_default() += 1;
                        saida.push('`');
                        saida.push_str(nosso);
                        saida.push('`');
                    }
                    _ => {
                        saida.push('`');
                        saida.push_str(nome);
                        saida.push('`');
                    }
                }
                resto = &depois[j + 1..];
            }
            _ => {
                saida.push('`');
                resto = depois;
            }
        }
    }
    saida.push_str(resto);
    saida
}

/// Corta no ultimo titulo (`\n#`) antes do teto: o corpo cortado termina numa secao
/// inteira, nao no meio de uma frase.
fn cortar_no_titulo(corpo: &str, teto: usize) -> &str {
    if corpo.len() <= teto {
        return corpo;
    }
    let mut fim = teto;
    while !corpo.is_char_boundary(fim) {
        fim -= 1;
    }
    match corpo[..fim].rfind("\n#") {
        Some(i) if i > teto / 2 => &corpo[..i],
        _ => &corpo[..fim],
    }
}

/// Importa todas as skills de `origem` para `destino`.
pub fn importar(origem: &Path, destino: &SkillFolder, op: &Opcoes) -> Relatorio {
    let mut rel = Relatorio::default();
    let mut arquivos = Vec::new();
    skill_mds(origem, &mut arquivos);
    rel.achados = arquivos.len();
    if let Err(e) = std::fs::create_dir_all(destino.root()) {
        rel.recusadas
            .push((destino.root().to_path_buf(), e.to_string()));
        return rel;
    }
    // O que ja esta importado, por hash: reimportar a mesma pasta nao duplica nada.
    let mut ja: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(destino.root()) {
        for e in rd.flatten() {
            if let Ok(t) = std::fs::read_to_string(e.path().join("ORIGEM.json"))
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(&t)
                && let Some(h) = v["sha256"].as_str()
            {
                ja.insert(h.to_string(), e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    for arq in arquivos {
        match importar_um(&arq, destino, op, &mut ja) {
            Ok(Some(i)) => rel.importadas.push(i),
            Ok(None) => rel.repetidas += 1,
            Err(e) => rel.recusadas.push((arq, e)),
        }
    }
    rel
}

fn importar_um(
    arq: &Path,
    destino: &SkillFolder,
    op: &Opcoes,
    ja: &mut BTreeMap<String, String>,
) -> Result<Option<Importada>, String> {
    let bytes = std::fs::read(arq).map_err(|e| e.to_string())?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    if ja.contains_key(&sha256) {
        return Ok(None);
    }
    let texto = String::from_utf8(bytes).map_err(|_| "SKILL.md nao e UTF-8".to_string())?;
    let (cab, corpo) = separar(&texto)?;
    let cab = cab.map(ler_cabecalho).unwrap_or_default();
    let texto_de = |k: &str| match cab.get(k) {
        Some(Valor::Texto(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    };
    let pasta_origem = arq.parent().unwrap_or(Path::new("."));
    let nome_da_pasta = pasta_origem
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bruto = texto_de("name").unwrap_or(nome_da_pasta);
    let mut nome = nome_valido(&bruto);
    let nome_ajustado = nome != bruto;
    // Nome ja usado por OUTRA skill (outro hash): sufixo, nunca sobrescrever.
    let base = nome.clone();
    let mut k = 2;
    while destino.root().join(&nome).exists() {
        let sufixo = format!("-{k}");
        nome = format!("{}{sufixo}", &base[..base.len().min(64 - sufixo.len())]);
        k += 1;
    }

    let mut imp = Importada {
        nome: nome.clone(),
        origem: arq.to_path_buf(),
        sha256: sha256.clone(),
        nome_ajustado,
        ..Default::default()
    };
    let descricao = match texto_de("description") {
        Some(d) => d,
        None => {
            imp.descricao_derivada = true;
            primeiro_paragrafo(corpo).ok_or("sem description e sem paragrafo no corpo")?
        }
    };
    let mut descricao = descricao.split_whitespace().collect::<Vec<_>>().join(" ");
    if descricao.chars().count() > SKILL_DESCRIPTION_MAX_CHARS {
        descricao = descricao
            .chars()
            .take(SKILL_DESCRIPTION_MAX_CHARS - 3)
            .collect::<String>()
            + "...";
        imp.descricao_cortada = true;
    }

    // Ferramentas do cabecalho: lista ou texto com virgula; `Bash(git:*)` vale `Bash`.
    let ferramentas: Vec<String> = match cab.get("allowed-tools") {
        Some(Valor::Lista(l)) => l.clone(),
        Some(Valor::Texto(t)) => t.split(',').map(|s| s.trim().to_string()).collect(),
        _ => vec![],
    };
    let mut permitidas = Vec::new();
    for f in ferramentas.iter().filter(|f| !f.is_empty()) {
        let base = f.split('(').next().unwrap_or(f).trim();
        match traduzir_nome(base) {
            Some(nosso) => {
                *imp.traducoes.entry(format!("{base}->{nosso}")).or_default() += 1;
                permitidas.push(nosso.to_string());
            }
            None if NOSSOS.contains(&base) => permitidas.push(base.to_string()),
            None => imp.nao_traduzidas.push(f.clone()),
        }
    }
    permitidas.sort();
    permitidas.dedup();

    let mut corpo = traduzir_corpo(corpo.trim(), &mut imp.traducoes);
    let scripts = pasta_origem.join("scripts");
    if scripts.is_dir() {
        let mut v = Vec::new();
        listar_relativo(&scripts, &scripts, &mut v);
        imp.scripts_desligados = v.iter().map(|p| format!("scripts/{p}")).collect();
    }
    if !imp.scripts_desligados.is_empty() && !op.com_scripts {
        corpo.push_str(&format!(
            "\n\n> Imported by PhxClaw: this skill shipped {} script(s) under `scripts/` that were \
NOT imported (scripts are disabled by default). Do not assume they exist.",
            imp.scripts_desligados.len()
        ));
    }

    let cabecalho = format!(
        "---\nname: {nome}\ndescription: {}\n{}origin-sha256: {sha256}\n---\n\n",
        descricao.replace('\n', " "),
        if permitidas.is_empty() {
            String::new()
        } else {
            format!("allowed-tools: {}\n", permitidas.join(", "))
        },
    );
    // Teto do `SkillFolder`: o corpo vai inteiro para o contexto quando a skill carrega.
    let teto = SKILL_DOC_MAX_BYTES as usize - cabecalho.len() - 200;
    let corpo_final = if corpo.len() > teto {
        imp.corpo_cortado = true;
        format!(
            "{}\n\n> Imported by PhxClaw: the original has {} bytes, above the {} KiB skill limit; \
it was cut at a heading. The full text is ORIGINAL.md in this skill's folder.",
            cortar_no_titulo(&corpo, teto),
            corpo.len(),
            SKILL_DOC_MAX_BYTES / 1024
        )
    } else {
        corpo
    };

    let dir = destino.root().join(&nome);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let cortado = imp.corpo_cortado;
    let gravar = || -> Result<(), String> {
        std::fs::write(dir.join("SKILL.md"), format!("{cabecalho}{corpo_final}\n"))
            .map_err(|e| e.to_string())?;
        if cortado {
            std::fs::write(dir.join("ORIGINAL.md"), &texto).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    let r = gravar().and_then(|_| {
        if op.com_scripts && !imp.scripts_desligados.is_empty() {
            copiar_pasta(&scripts, &dir.join("scripts"))?;
            imp.scripts_copiados = true;
        }
        // A prova de que importou e o MESMO leitor do agente carregar a skill.
        destino.load(&nome).map(|_| ()).map_err(|e| e.to_string())
    });
    if let Err(e) = r {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(e);
    }
    let origem_json = serde_json::to_vec_pretty(&imp).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("ORIGEM.json"), origem_json).map_err(|e| e.to_string())?;
    ja.insert(sha256, nome);
    Ok(Some(imp))
}

fn listar_relativo(raiz: &Path, d: &Path, v: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(d) else {
        return;
    };
    let mut es: Vec<_> = rd.flatten().collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        if p.is_dir() {
            listar_relativo(raiz, &p, v);
        } else if let Ok(r) = p.strip_prefix(raiz) {
            v.push(r.display().to_string());
        }
    }
}

/// Copia sem seguir symlink: script importado nao vira atalho para fora da pasta.
fn copiar_pasta(de: &Path, para: &Path) -> Result<(), String> {
    std::fs::create_dir_all(para).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(de).map_err(|e| e.to_string())?.flatten() {
        let m = std::fs::symlink_metadata(e.path()).map_err(|e| e.to_string())?;
        if m.is_dir() {
            copiar_pasta(&e.path(), &para.join(e.file_name()))?;
        } else if m.is_file() {
            std::fs::copy(e.path(), para.join(e.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabecalho_yaml_nas_formas_do_corpus() {
        let cab = "name: x\ndescription: >\n  linha um\n  linha dois\nallowed-tools: [\"Read\", \"Bash\"]\n\
metadata:\n  hermes:\n    tags: [a]\ntags:\n  - um\n  - 'dois'\nlonga: \"com \\\"aspas\\\"\"\n";
        let m = ler_cabecalho(cab);
        assert_eq!(m["description"], Valor::Texto("linha um linha dois".into()));
        assert_eq!(
            m["allowed-tools"],
            Valor::Lista(vec!["Read".into(), "Bash".into()])
        );
        assert_eq!(m["metadata"], Valor::Mapa);
        assert_eq!(m["tags"], Valor::Lista(vec!["um".into(), "dois".into()]));
        assert_eq!(m["longa"], Valor::Texto("com \"aspas\"".into()));
    }

    #[test]
    fn traduz_so_nome_entre_crases() {
        let mut c = BTreeMap::new();
        let t = traduzir_corpo("Use `terminal` e `web_extract`; Read the `README`.", &mut c);
        assert_eq!(t, "Use `shell` e `browser_open`; Read the `README`.");
        assert_eq!(c.len(), 2);
        assert_eq!(nome_valido("Minha Skill: v2.0"), "minha-skill-v2-0");
    }
}
