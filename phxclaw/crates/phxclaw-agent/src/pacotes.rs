//! Pacotes de plugin no formato do Claude Code (`.claude-plugin/plugin.json`) e do Codex
//! (`.codex-plugin/plugin.json`): uma pasta com skills, hooks, servidores MCP, subagentes
//! e comandos, nos lugares padrao ou nos caminhos que o manifesto disser.
//!
//! Assinatura Ed25519 OBRIGATORIA, sobre a pasta inteira (`phxclaw_plugin_registry::pasta`,
//! o mesmo trust store dos manifestos): pacote sem assinatura, com signatario fora do
//! trust store ou com qualquer arquivo mexido depois de assinado nao carrega nada. Nada de
//! WASM nem biblioteca dinamica: o que executa sao os processos que os proprios
//! subsistemas ja sabem isolar.
//!
//! O que entra no agente, e por qual porta de cada subsistema (nenhuma e nova):
//! - **servidores MCP**, pela MESMA subida do `PHXCLAW_MCP_CONFIG`
//!   (`mcp::carregar_config_de_pacote`), com `${CLAUDE_PLUGIN_ROOT}` trocado pela raiz do
//!   pacote e a capacidade `mcp.<pacote>.<servidor>`, que o operador concede a parte;
//! - **comandos** (`commands/*.md`), que viram comandos de barra (`comandos.rs`), depois
//!   dos do projeto -- nome repetido fica com o projeto;
//! - **skills** (`skills/*/SKILL.md`), pelo importador de skills (`importar_skills`): a
//!   mesma varredura anti-injecao, o mesmo `ORIGEM.json` com o SHA-256, e a mesma porta de
//!   licenca (`licenca`), subindo ate a raiz do pacote: o `license` do `plugin.json` e o
//!   `LICENSE` da raiz entram na conta, copyleft e desconhecida ficam de fora com aviso, e a
//!   compativel entra com o texto em `LICENCA.txt`;
//! - **hooks** (`hooks/hooks.json`), somados aos do projeto (`hooks::somar`) com a raiz do
//!   pacote em `/hooks` -- e so chegam aqui porque a assinatura da pasta inteira ja
//!   passou: hook de pacote sem assinatura valida nao existe para o agente;
//! - **subagentes** (`agents/*.md`), que viram papeis da equipe (`equipe::somar`) com as
//!   capacidades DECLARADAS no cabecalho (`capabilities:`, ou derivadas de `tools:`);
//!   sem declaracao o papel e recusado, porque papel sem capacidade e papel sem limite
//!   que alguem leu.
//!
//! Tudo o que executa (MCP, hook) roda no bwrap do subsistema dono; o pacote nao traz
//! nenhum executavel proprio que o agente rode fora dele.

use crate::comandos::ComandoDeBarra;
use crate::hooks::Hooks;
use crate::mcp::{ConfigMcp, ServidorDeclarado};
use phxclaw_agent_catalog::AgentManifest;
use phxclaw_agent_core::Tool;
use phxclaw_plugin_registry::TrustStore;
use phxclaw_skill_runtime::SkillFolder;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Pacote {
    pub nome: String,
    pub versao: String,
    /// `claude` ou `codex`.
    pub formato: &'static str,
    pub raiz: PathBuf,
    pub skills: Vec<String>,
    pub agentes: Vec<String>,
    pub comandos: Vec<String>,
    /// O `hooks.json` do pacote (eventos do Claude Code); `integrar` o liga.
    pub hooks: Option<Value>,
    pub mcp: ConfigMcp,
    /// A licenca declarada no `plugin.json`, como veio. Quem decide se o texto do pacote pode
    /// ser copiado e a porta de licenca (`licenca::conferir`), que le o mesmo campo.
    pub licenca: Option<String>,
}

/// O que um pacote soma ao agente alem dos servidores MCP, ja conferido.
#[derive(Clone, Default)]
pub struct Componentes {
    pub pacote: String,
    pub comandos: Vec<ComandoDeBarra>,
    pub hooks: Option<Hooks>,
    pub agentes: Vec<AgentManifest>,
    /// Skills importadas para a pasta de skills do agente (nome -> SHA-256).
    pub skills: Vec<(String, String)>,
    pub avisos: Vec<String>,
}

const FORMATOS: [(&str, &str); 2] = [(".claude-plugin", "claude"), (".codex-plugin", "codex")];

fn dentro(raiz: &Path, rel: &str) -> Result<PathBuf, String> {
    let p = raiz.join(rel.trim_start_matches("./"));
    let c = std::fs::canonicalize(&p).map_err(|e| format!("{rel}: {e}"))?;
    if !c.starts_with(raiz) {
        return Err(format!("{rel} aponta para fora do pacote"));
    }
    Ok(c)
}

/// Caminho do componente: o que o manifesto disser (texto), senao o padrao, se existir.
fn componente(
    raiz: &Path,
    m: &Value,
    campo: &str,
    padrao: &str,
) -> Result<Option<PathBuf>, String> {
    match m.get(campo).and_then(Value::as_str) {
        Some(rel) => dentro(raiz, rel).map(Some),
        None if raiz.join(padrao).exists() => dentro(raiz, padrao).map(Some),
        None => Ok(None),
    }
}

fn nomes(dir: Option<PathBuf>, so_pastas_com: Option<&str>, ext: Option<&str>) -> Vec<String> {
    let Some(dir) = dir else { return vec![] };
    let mut v: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let n = e.file_name().to_string_lossy().to_string();
            match (so_pastas_com, ext) {
                (Some(f), _) if p.join(f).is_file() => Some(n),
                (None, Some(x)) if p.is_file() && n.ends_with(x) => {
                    Some(n.trim_end_matches(x).to_string())
                }
                _ => None,
            }
        })
        .collect();
    v.sort();
    v
}

fn expandir(t: &str, raiz: &Path) -> String {
    let r = raiz.display().to_string();
    t.replace("${CLAUDE_PLUGIN_ROOT}", &r)
        .replace("${PLUGIN_ROOT}", &r)
}

/// `{"mcpServers": {nome: {command, args, env, url}}}` (ou o mapa direto) para o
/// formato declarado do agente, com a raiz do pacote como pasta de trabalho.
fn traduzir_mcp(v: &Value, raiz: &Path) -> Result<ConfigMcp, String> {
    let mapa = v
        .get("mcpServers")
        .unwrap_or(v)
        .as_object()
        .ok_or("mcpServers precisa ser um objeto")?;
    let mut servidores = Vec::new();
    for (nome, s) in mapa {
        let txt = |c: &str| s.get(c).and_then(Value::as_str).map(|x| expandir(x, raiz));
        let args = s
            .get("args")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|x| expandir(x, raiz))
                    .collect()
            })
            .unwrap_or_default();
        let env: BTreeMap<String, String> = s
            .get("env")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .filter_map(|(k, x)| x.as_str().map(|x| (k.clone(), expandir(x, raiz))))
                    .collect()
            })
            .unwrap_or_default();
        servidores.push(ServidorDeclarado {
            nome: nome.clone(),
            comando: txt("command"),
            args,
            env,
            cwd: Some(raiz.to_path_buf()),
            url: txt("url"),
            prazo_inicio_ms: None,
            auth: None,
            preset: None,
        });
    }
    Ok(ConfigMcp { servidores })
}

/// O nome de pacote vira a capacidade `mcp.<pacote>.<servidor>` pela `normalize_mcp_name`,
/// que troca tudo fora de `[A-Za-z0-9_-]` por `_`. Se o nome aceitasse `.`, `foo.bar` e
/// `foo_bar` dariam a MESMA capacidade e o operador que concedeu `mcp.foo_bar.x` a um
/// estaria concedendo ao outro. O conserto e tirar do alfabeto do nome o que a normalizacao
/// destroi: nesse alfabeto ela e a identidade, logo injetiva por construcao.
pub fn nome_de_pacote_valido(n: &str) -> bool {
    !n.is_empty()
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// Reserva a chave de capacidade de um pacote, ou recusa dizendo com quem ela colide.
/// Defesa de segunda linha: mesmo com o alfabeto fechado, DUAS pastas podem declarar o
/// mesmo `name` no `plugin.json` (a pasta nao e o nome), e a segunda herdaria a concessao
/// da primeira. A chave e a que a capacidade usa de verdade, nao o nome cru -- se o
/// alfabeto um dia abrir, esta checagem continua cobrindo a colisao.
fn reservar_nome(
    vistos: &mut BTreeMap<String, String>,
    nome: &str,
    origem: &str,
) -> Result<(), String> {
    let chave = phxclaw_mcp_lsp_runtime::normalize_mcp_name(nome.trim());
    if let Some(outro) = vistos.get(&chave) {
        return Err(format!(
            "o nome {nome:?} da a mesma capacidade mcp.{chave}.* de {outro}: dois pacotes nao \
dividem concessao; remova um dos dois"
        ));
    }
    vistos.insert(chave, origem.to_string());
    Ok(())
}

fn ler_json(p: &Path) -> Result<Value, String> {
    serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?)
        .map_err(|e| format!("{}: {e}", p.display()))
}

/// Le e CONFERE um pacote. A assinatura vem antes de qualquer componente: nada de um
/// pacote adulterado e lido alem do nome.
pub fn ler(dir: &Path, trust: &TrustStore) -> Result<Pacote, String> {
    let raiz = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (pasta, formato) = FORMATOS
        .iter()
        .find(|(p, _)| raiz.join(p).join("plugin.json").is_file())
        .ok_or("sem .claude-plugin/plugin.json nem .codex-plugin/plugin.json")?;
    let m = ler_json(&raiz.join(pasta).join("plugin.json"))?;
    let nome = m
        .get("name")
        .and_then(Value::as_str)
        .filter(|n| nome_de_pacote_valido(n))
        .ok_or("plugin.json sem 'name' valido (so letras ASCII, digitos, '-' e '_')")?
        .to_string();
    phxclaw_plugin_registry::pasta::verificar_pasta(&raiz, &nome, trust)
        .map_err(|e| format!("assinatura do pacote {nome}: {e}"))?;
    let hooks = match m.get("hooks") {
        Some(Value::Object(_)) => m.get("hooks").cloned(),
        _ => componente(&raiz, &m, "hooks", "hooks/hooks.json")?
            .map(|p| ler_json(&p))
            .transpose()?,
    };
    let mcp = match m.get("mcpServers") {
        Some(v @ Value::Object(_)) => traduzir_mcp(v, &raiz)?,
        _ => match componente(&raiz, &m, "mcpServers", ".mcp.json")? {
            Some(p) => traduzir_mcp(&ler_json(&p)?, &raiz)?,
            None => ConfigMcp { servidores: vec![] },
        },
    };
    Ok(Pacote {
        versao: m
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0")
            .into(),
        licenca: m.get("license").and_then(Value::as_str).map(str::to_string),
        formato,
        skills: nomes(
            componente(&raiz, &m, "skills", "skills")?,
            Some("SKILL.md"),
            None,
        ),
        agentes: nomes(
            componente(&raiz, &m, "agents", "agents")?,
            None,
            Some(".md"),
        ),
        comandos: nomes(
            componente(&raiz, &m, "commands", "commands")?,
            None,
            Some(".md"),
        ),
        hooks,
        mcp,
        nome,
        raiz,
    })
}

/// As ferramentas MCP do pacote, pela mesma subida do `PHXCLAW_MCP_CONFIG`, com a
/// capacidade no espaco do pacote (`mcp.<pacote>.<servidor>`): servidor de pacote com o nome
/// de um do operador nao herda a concessao dele.
pub fn ferramentas(p: &Pacote) -> (Vec<Arc<dyn Tool>>, Vec<String>) {
    crate::mcp::carregar_config_de_pacote(&p.mcp, &p.raiz, &p.nome)
}

/// O primeiro paragrafo nao vazio de um texto, para a missao de um subagente sem
/// `description`.
fn primeiro_paragrafo(corpo: &str) -> String {
    corpo
        .split("\n\n")
        .map(str::trim)
        .find(|p| !p.is_empty())
        .unwrap_or("")
        .trim_start_matches('#')
        .trim()
        .chars()
        .take(300)
        .collect()
}

/// Capacidades de um subagente a partir dos nomes de ferramenta do Claude Code
/// (`tools: Read, Write, Bash`): so o que a casa sabe mapear; o resto e aviso. O importador
/// do agency-agents (`importar_papeis`) usa esta mesma tabela: duas divergiriam.
pub(crate) fn capacidades_das_ferramentas(
    tools: &[String],
    avisos: &mut Vec<String>,
) -> Vec<String> {
    let mut caps = std::collections::BTreeSet::new();
    for t in tools {
        let cap = match t.trim().to_ascii_lowercase().as_str() {
            "read" | "read_file" | "glob" | "grep" | "list_files" | "search_files" | "lsp" => {
                "fs.read"
            }
            "write" | "edit" | "multiedit" | "write_file" | "edit_file" => "fs.write",
            "bash" | "shell" => "shell.exec",
            "websearch" | "web_search" => "web.search",
            "webfetch" | "browse" | "web_fetch" => "web.browse",
            outro => {
                avisos.push(format!(
                    "ferramenta {outro:?} do subagente sem capacidade conhecida"
                ));
                continue;
            }
        };
        caps.insert(cap.to_string());
    }
    caps.into_iter().collect()
}

/// Um papel da equipe a partir de `agents/<nome>.md` (cabecalho YAML + instrucao).
fn papel_de(
    pacote: &Pacote,
    nome_arquivo: &str,
    texto: &str,
    indice: u32,
) -> Result<(AgentManifest, Vec<String>), String> {
    use crate::importar_skills::{Valor, ler_cabecalho, separar};
    // O corpo vira `responsibilities` e o `prompt_do_papel` o imprime: e texto de terceiro
    // como o dos papeis importados, e passa pela MESMA varredura (antes do cabecalho) e
    // pelo MESMO teto. Reprovado, o papel nao entra -- fica so o aviso de bloqueio.
    let achados = crate::instrucoes::varrer(texto);
    if !achados.is_empty() {
        return Err(format!(
            "[BLOCKED: {}/agents/{nome_arquivo}.md contained potential prompt injection ({}). Content not loaded.]",
            pacote.nome,
            achados.join(", ")
        ));
    }
    let (cab, corpo) = separar(texto)?;
    let cab = cab.map(ler_cabecalho).unwrap_or_default();
    let texto_de = |k: &str| match cab.get(k) {
        Some(Valor::Texto(t)) => Some(t.clone()),
        _ => None,
    };
    let lista_de = |k: &str| -> Vec<String> {
        match cab.get(k) {
            Some(Valor::Lista(l)) => l.clone(),
            Some(Valor::Texto(t)) => t
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            _ => vec![],
        }
    };
    let nome = texto_de("name").unwrap_or_else(|| nome_arquivo.to_string());
    let nome = crate::importar_skills::nome_valido(&nome);
    let mut avisos = Vec::new();
    let mut capacidades = lista_de("capabilities");
    if capacidades.is_empty() {
        capacidades = capacidades_das_ferramentas(&lista_de("tools"), &mut avisos);
    }
    if capacidades.is_empty() {
        return Err(format!(
            "subagente {nome}: sem `capabilities:` nem `tools:` conhecidas no cabecalho; papel sem capacidade declarada nao entra"
        ));
    }
    let missao = texto_de("description").unwrap_or_else(|| primeiro_paragrafo(corpo));
    if missao.trim().is_empty() {
        return Err(format!("subagente {nome}: sem description nem corpo"));
    }
    let m: AgentManifest = serde_json::from_value(json!({
        "manifest_version": "1.0.0",
        "uuid": phxclaw_types::new_uuid_v7(),
        "agent_id": 100_000 + indice,
        "name": format!("{}/{nome}", pacote.nome),
        "macroarea": "plugin",
        "nucleus": pacote.nome,
        "role_type": "subagente de pacote",
        "mission": missao,
        "responsibilities": phxclaw_agent_core::truncate_for_model(
            corpo.trim(),
            crate::equipe::TETO_DESCRICAO_DO_PAPEL,
        ),
        "execution": "local",
        "models_allowed": texto_de("model").map(|m| vec![m]).unwrap_or_default(),
        "capabilities": capacidades,
        "execution_policy": "subagent",
        "source": {"workbook": format!("pacote:{}", pacote.nome), "sheet": pacote.formato, "row": indice},
    }))
    .map_err(|e| format!("subagente {nome}: {e}"))?;
    Ok((m, avisos))
}

/// Liga ao agente o que o pacote (ja conferido por `ler`) traz alem do MCP. `skills`:
/// a pasta de skills do agente, onde as do pacote sao importadas (`None` pula as skills).
pub fn integrar(p: &Pacote, skills: Option<&SkillFolder>, bwrap: Option<PathBuf>) -> Componentes {
    let mut c = Componentes {
        pacote: p.nome.clone(),
        ..Componentes::default()
    };
    let origem = format!("pacote:{}", p.nome);
    // comandos
    if let Ok(Some(dir)) = componente(&p.raiz, &Value::Null, "commands", "commands") {
        let (lista, avisos) = crate::comandos::ler_pasta(&dir, &origem);
        c.comandos = lista;
        c.avisos.extend(avisos);
    }
    // skills, pelo importador (varredura anti-injecao, ORIGEM.json com o SHA-256)
    if let (Some(destino), Ok(Some(dir))) = (
        skills,
        componente(&p.raiz, &Value::Null, "skills", "skills"),
    ) {
        // A porta sobe ate a raiz do pacote (e nao ate um `.git` acima da pasta de pacotes):
        // e la que moram o `plugin.json` e o `LICENSE` de quem escreveu. Desconhecida nao se
        // aceita aqui -- carga de pacote nao tem operador presente para decidir.
        let r = crate::importar_skills::importar(
            &dir,
            destino,
            &crate::importar_skills::Opcoes {
                com_scripts: false,
                limite_licenca: Some(p.raiz.clone()),
                aceitar_licenca_desconhecida: false,
            },
        );
        for i in &r.importadas {
            c.skills.push((i.nome.clone(), i.sha256.clone()));
        }
        for (arq, m) in &r.recusadas {
            c.avisos
                .push(format!("skill {} recusada: {m}", arq.display()));
        }
    }
    // hooks: o `hooks.json` ja veio sob a assinatura da pasta
    if let Some(h) = &p.hooks {
        match Hooks::de_json(&h.to_string(), p.raiz.clone(), bwrap) {
            Ok(h) => c.hooks = Some(h),
            Err(e) => c.avisos.push(format!("hooks do pacote ignorados: {e}")),
        }
    }
    // subagentes
    if let Ok(Some(dir)) = componente(&p.raiz, &Value::Null, "agents", "agents") {
        for (i, nome) in p.agentes.iter().enumerate() {
            let arq = dir.join(format!("{nome}.md"));
            let texto = match std::fs::read_to_string(&arq) {
                Ok(t) => t,
                Err(e) => {
                    c.avisos.push(format!("{}: {e}", arq.display()));
                    continue;
                }
            };
            match papel_de(p, nome, &texto, i as u32) {
                Ok((m, avisos)) => {
                    c.agentes.push(m);
                    c.avisos.extend(avisos);
                }
                Err(e) => c.avisos.push(e),
            }
        }
    }
    c
}

/// `PHXCLAW_PACOTES_DIR`: cada subpasta e um pacote; o trust store e o dos plugins
/// (`PHXCLAW_PLUGIN_SIGNERS`). Sem trust store, nenhum pacote carrega -- assinatura e
/// obrigatoria, nao opcional quando der.
pub fn do_ambiente() -> Vec<Arc<dyn Tool>> {
    do_ambiente_completo(None, None).0
}

/// Os pacotes do ambiente inteiros: as ferramentas MCP e, por pacote, os componentes
/// (comandos, skills importadas em `skills`, hooks, subagentes).
pub fn do_ambiente_completo(
    skills: Option<&SkillFolder>,
    bwrap: Option<PathBuf>,
) -> (Vec<Arc<dyn Tool>>, Vec<Componentes>) {
    let mut componentes = Vec::new();
    let Some(dir) = crate::config::caminho_de("pacotes.dir") else {
        return (vec![], componentes);
    };
    let trust = crate::config::caminho_de("plugins.assinantes")
        .ok_or_else(|| format!("{} ausente", crate::config::variavel("plugins.assinantes")))
        .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
        .and_then(|t| TrustStore::from_json(&t).map_err(|e| e.to_string()));
    let trust = match trust {
        Ok(t) => t,
        Err(e) => {
            eprintln!("aviso: pacotes de plugin nao carregados (sem trust store): {e}");
            return (vec![], componentes);
        }
    };
    let mut tools = Vec::new();
    let mut pastas: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    pastas.sort();
    // chave de capacidade -> pasta que a ocupou primeiro (ordem alfabetica das pastas)
    let mut vistos: BTreeMap<String, String> = BTreeMap::new();
    for p in pastas {
        if p.file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        {
            // `.instalando-*` da loja: pacote pela metade nao e pacote.
            continue;
        }
        match ler(&p, &trust).and_then(|pac| {
            reservar_nome(&mut vistos, &pac.nome, &p.display().to_string()).map(|()| pac)
        }) {
            Ok(pac) => {
                let (t, avisos) = ferramentas(&pac);
                for a in avisos {
                    eprintln!("aviso: pacote {}: {a}", pac.nome);
                }
                tools.extend(t);
                let c = integrar(&pac, skills, bwrap.clone());
                for a in &c.avisos {
                    eprintln!("aviso: pacote {}: {a}", pac.nome);
                }
                componentes.push(c);
            }
            Err(e) => eprintln!("aviso: pacote {} recusado: {e}", p.display()),
        }
    }
    (tools, componentes)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_que_a_normalizacao_funde_nao_e_nome() {
        use phxclaw_mcp_lsp_runtime::normalize_mcp_name as norm;
        // o defeito: estes dois eram nomes validos e davam a mesma capacidade
        assert_eq!(norm("foo.bar"), norm("foo_bar"));
        assert!(!nome_de_pacote_valido("foo.bar"));
        assert!(nome_de_pacote_valido("foo_bar"));
        for ruim in ["", "a b", "a/b", "a.b", "..", "ç", "a:b"] {
            assert!(!nome_de_pacote_valido(ruim), "{ruim:?}");
        }
        // o irmao: o que o catalogo usa hoje segue valendo, e a normalizacao e identidade nele
        for bom in ["pacote-teste", "foo_bar", "Foo-Bar_2"] {
            assert!(nome_de_pacote_valido(bom), "{bom}");
            assert_eq!(norm(bom), bom);
        }
    }

    #[test]
    fn ler_recusa_o_nome_com_ponto_antes_de_qualquer_outra_coisa() {
        let d = std::env::temp_dir().join(format!("pacotes-nome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".claude-plugin")).unwrap();
        std::fs::write(
            d.join(".claude-plugin/plugin.json"),
            r#"{"name":"foo.bar","version":"1.0.0"}"#,
        )
        .unwrap();
        let trust = TrustStore::from_json(r#"{"version":"1","signers":[]}"#).unwrap();
        let e = ler(&d, &trust).unwrap_err();
        let _ = std::fs::remove_dir_all(&d);
        // a recusa e a do NOME, nao a da assinatura que viria depois
        assert!(e.contains("'name' valido"), "{e}");
    }

    #[test]
    fn dois_pacotes_na_mesma_capacidade_a_segunda_e_recusada() {
        let mut vistos = BTreeMap::new();
        reservar_nome(&mut vistos, "foo_bar", "/p/a").unwrap();
        let e = reservar_nome(&mut vistos, "foo_bar", "/p/b").unwrap_err();
        assert!(e.contains("/p/a") && e.contains("mcp.foo_bar.*"), "{e}");
        // se o alfabeto do nome um dia abrir, a colisao pela normalizacao tambem cai aqui
        let e = reservar_nome(&mut vistos, "foo.bar", "/p/c").unwrap_err();
        assert!(e.contains("/p/a"), "{e}");
        // o irmao: nome diferente reserva normalmente
        reservar_nome(&mut vistos, "foo_baz", "/p/d").unwrap();
        reservar_nome(&mut vistos, "foo-bar", "/p/e").unwrap();
    }
}
