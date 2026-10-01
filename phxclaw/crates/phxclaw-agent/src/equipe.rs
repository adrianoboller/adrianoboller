//! A equipe dos 110 papeis da planilha, ativa no agente: listar (`team_list`), delegar
//! (`team_delegate`) e o JSON da interface saem daqui, e a CLI `phxclaw equipe` chama as
//! mesmas funcoes. Os manifestos sao os de `config/agents`, lidos pelo `AgentCatalog`.
//!
//! O subagente de um papel roda pelo MESMO laco do `parallel_research`
//! (`ferramentas::rodar_subagentes`), com o prompt montado do papel e com as ferramentas
//! restritas a interseccao do que o papel autoriza com o que o pai tem: delegar nunca
//! amplia poder.

use crate::ferramentas::{config_de_subagente, corpo_do_subagente, rodar_subagentes};
use crate::motor::Agent;
use crate::tarefa::Task;
use phxclaw_agent_catalog::{AgentCatalog, AgentManifest};
use phxclaw_agent_core::{
    BoxFut, Llm, Tool, ToolContext, ToolError, ToolOutput, ToolSpec, truncate_for_model,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Variavel que aponta a pasta dos manifestos. Quando dada, manda: pasta errada vira aviso
/// em vez de cair calada noutra pasta que o operador nao escolheu.
pub const VAR_PASTA: &str = "PHXCLAW_AGENTES_DIR";
/// Modelo local para os papeis que a planilha roteia para o Ollama.
pub const VAR_MODELO_LOCAL: &str = "PHXCLAW_MODELO_LOCAL";

// O mesmo corte do `parallel_research`, de um lugar so (`config_de_subagente`).
use crate::ferramentas::NUNCA_NO_SUBAGENTE;

/// O catalogo carregado e a frequencia de cada capability nele. A frequencia e o que diz
/// qual capability DISTINGUE um papel: as que vem dos modulos (F15, F18, F22...) aparecem
/// em dezenas de papeis, a propria do papel em um ou dois. Tirar isso do dado poupa uma
/// lista digitada das capabilities de modulo, que envelheceria com a planilha.
pub struct Equipe {
    pub pasta: PathBuf,
    catalogo: AgentCatalog,
    frequencia: BTreeMap<String, usize>,
}

/// O resumo de um papel, o mesmo para a ferramenta, a CLI e a interface.
#[derive(Debug, Clone, Serialize)]
pub struct Ficha {
    pub id: u32,
    pub uuid: String,
    pub nome: String,
    pub macroarea: String,
    pub nucleo: String,
    pub tipo: String,
    pub criticidade: String,
    pub capability_principal: String,
    pub quando_acionar: String,
    pub execucao: String,
    pub modelos: Vec<String>,
    pub humano: bool,
    pub prefere_local: bool,
    pub missao: String,
}

impl Equipe {
    pub fn carregar(pasta: impl AsRef<Path>) -> Result<Self, String> {
        let pasta = pasta.as_ref().to_path_buf();
        let catalogo = AgentCatalog::load_dir(&pasta)
            .map_err(|e| format!("catalogo de agentes em {}: {e}", pasta.display()))?;
        if catalogo.is_empty() {
            return Err(format!("nenhum *.agent.json em {}", pasta.display()));
        }
        let mut frequencia = BTreeMap::new();
        for (_, m) in catalogo.iter() {
            for c in &m.capabilities {
                *frequencia.entry(c.clone()).or_insert(0) += 1;
            }
        }
        Ok(Self {
            pasta,
            catalogo,
            frequencia,
        })
    }

    /// A pasta de `PHXCLAW_AGENTES_DIR`, ou a primeira `config/agents` que existir: na
    /// pasta corrente, subindo a partir do executavel (instalacao e `target/`), e por fim
    /// a do repositorio onde este crate foi compilado.
    pub fn do_ambiente() -> Result<Self, String> {
        Self::carregar(pasta_padrao())
    }

    pub fn len(&self) -> usize {
        self.catalogo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.catalogo.is_empty()
    }

    /// Os papeis na ordem da planilha (pelo ID dela, nao pelo UUID).
    pub fn papeis(&self) -> Vec<&AgentManifest> {
        let mut v: Vec<&AgentManifest> = self.catalogo.iter().map(|(_, m)| m).collect();
        v.sort_by_key(|m| m.agent_id);
        v
    }

    /// Papel pelo ID da planilha, pelo UUID ou pelo nome (exato, sem acento e sem caixa,
    /// ou trecho que case com um so). Ambiguo recusa dizendo os candidatos: delegar ao
    /// papel errado e pior que perguntar de novo.
    pub fn achar(&self, chave: &str) -> Result<&AgentManifest, String> {
        let chave = chave.trim();
        if let Ok(n) = chave.parse::<u32>() {
            return self
                .papeis()
                .into_iter()
                .find(|m| m.agent_id == n)
                .ok_or_else(|| format!("nao ha papel com id {n} (1 a {})", self.len()));
        }
        if let Ok(u) = chave.parse::<uuid::Uuid>() {
            return self
                .catalogo
                .get(&u)
                .ok_or_else(|| format!("nao ha papel com uuid {u}"));
        }
        if let Some(m) = self.catalogo.get_by_name(chave) {
            return Ok(m);
        }
        let alvo = dobrar(chave);
        let papeis = self.papeis();
        if let Some(m) = papeis.iter().find(|m| dobrar(&m.name) == alvo) {
            return Ok(m);
        }
        let parecidos: Vec<&&AgentManifest> = papeis
            .iter()
            .filter(|m| dobrar(&m.name).contains(&alvo))
            .collect();
        match parecidos.as_slice() {
            [m] => Ok(m),
            [] => Err(format!(
                "nenhum papel chamado {chave:?}; veja a lista com team_list ou `phxclaw equipe listar`"
            )),
            muitos => Err(format!(
                "{chave:?} casa com {} papeis: {}; diga o id",
                muitos.len(),
                muitos
                    .iter()
                    .map(|m| format!("{} {}", m.agent_id, m.name))
                    .collect::<Vec<_>>()
                    .join("; ")
            )),
        }
    }

    /// A capability que distingue o papel: a mais rara no catalogo (empate: a primeira).
    pub fn capability_principal<'a>(&self, m: &'a AgentManifest) -> &'a str {
        m.capabilities
            .iter()
            .min_by_key(|c| self.frequencia.get(*c).copied().unwrap_or(0))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn ficha(&self, m: &AgentManifest) -> Ficha {
        Ficha {
            id: m.agent_id,
            uuid: m.uuid.to_string(),
            nome: m.name.clone(),
            macroarea: m.macroarea.clone(),
            nucleo: m.nucleus.clone(),
            tipo: m.role_type.clone(),
            criticidade: m.criticality.clone(),
            capability_principal: self.capability_principal(m).to_string(),
            quando_acionar: m.trigger.clone(),
            execucao: m.execution.clone(),
            modelos: m.models_allowed.clone(),
            humano: e_humano(m),
            prefere_local: prefere_local(m),
            missao: m.mission.clone(),
        }
    }

    /// Filtro por macroarea (trecho) e por texto (nome, missao, tipo, gatilho ou
    /// capability), os dois sem acento e sem caixa.
    pub fn listar(&self, macroarea: Option<&str>, texto: Option<&str>) -> Vec<Ficha> {
        let area = macroarea.map(dobrar).filter(|s| !s.is_empty());
        let texto = texto.map(dobrar).filter(|s| !s.is_empty());
        self.papeis()
            .into_iter()
            .filter(|m| {
                area.as_ref()
                    .is_none_or(|a| dobrar(&m.macroarea).contains(a))
            })
            .filter(|m| {
                texto.as_ref().is_none_or(|t| {
                    [&m.name, &m.mission, &m.role_type, &m.trigger, &m.nucleus]
                        .iter()
                        .any(|c| dobrar(c).contains(t))
                        || m.capabilities.iter().any(|c| dobrar(c).contains(t))
                })
            })
            .map(|m| self.ficha(m))
            .collect()
    }

    /// O arquivo que a interface le: os papeis agrupados por macroarea, na ordem em que a
    /// planilha apresenta cada macroarea. Gerado, nunca digitado (`examples/equipe_json`).
    pub fn json_da_interface(&self) -> Value {
        let mut ordem: Vec<String> = Vec::new();
        let mut grupos: BTreeMap<String, Vec<Ficha>> = BTreeMap::new();
        for m in self.papeis() {
            if !grupos.contains_key(&m.macroarea) {
                ordem.push(m.macroarea.clone());
            }
            grupos
                .entry(m.macroarea.clone())
                .or_default()
                .push(self.ficha(m));
        }
        let fonte = self
            .papeis()
            .first()
            .map(|m| m.source.workbook.clone())
            .unwrap_or_default();
        json!({
            "gerado_por": "cargo run -p phxclaw-agent --example equipe_json",
            "fonte": fonte,
            "total": self.len(),
            "macroareas": ordem.iter().map(|a| json!({
                "nome": a,
                "total": grupos[a].len(),
                "papeis": grupos[a],
            })).collect::<Vec<_>>(),
        })
    }

    /// O conteudo exato de `apps/phxclaw-ui/assets/equipe.json`. O exemplo grava isto e o
    /// teste compara com o arquivo: catalogo mudado sem regerar reprova.
    pub fn arquivo_da_interface(&self) -> String {
        let mut s = serde_json::to_string_pretty(&self.json_da_interface()).unwrap_or_default();
        s.push('\n');
        s
    }
}

/// Onde a interface le a equipe, relativo a raiz do PhxClaw.
pub const ARQUIVO_DA_INTERFACE: &str = "apps/phxclaw-ui/assets/equipe.json";

/// Pasta dos manifestos: a variavel, ou a primeira candidata que existir.
pub fn pasta_padrao() -> PathBuf {
    if let Ok(p) = std::env::var(VAR_PASTA) {
        return PathBuf::from(p);
    }
    let mut candidatas = vec![PathBuf::from("config/agents")];
    if let Ok(exe) = std::env::current_exe() {
        candidatas.extend(
            exe.ancestors()
                .skip(1)
                .take(5)
                .map(|d| d.join("config/agents")),
        );
    }
    candidatas.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/agents"));
    candidatas
        .iter()
        .find(|p| p.is_dir())
        .cloned()
        .unwrap_or_else(|| candidatas.remove(0))
}

/// Papel que a planilha marca como humano nao roda como subagente: o que ele decide e
/// autoridade de gente, e um modelo respondendo no lugar dele seria decisao forjada.
pub fn e_humano(m: &AgentManifest) -> bool {
    dobrar(&m.role_type) == "papel humano" || m.models_allowed.iter().any(|x| x == "humano")
}

/// A planilha roteia o papel para o Ollama (sozinho ou ao lado de outro modelo).
pub fn prefere_local(m: &AgentManifest) -> bool {
    m.models_allowed.iter().any(|x| x == "ollama")
}

/// Capacidades de FERRAMENTA do agente que uma capability da planilha autoriza. As duas
/// listas nao falam a mesma lingua (169 capabilities de dominio contra ~20 de ferramenta),
/// e sem esta traducao a interseccao com o pai daria a quase todo papel so memoria e
/// skill. A traducao e conservadora: ler o repositorio vira ler arquivo, escrever codigo
/// ou documento vira escrever arquivo, build e teste viram shell, pesquisa vira web, e o
/// resto (canal, segredo, plugin, MCP) nao vira nada -- quem pede isso pede ao operador.
pub fn ferramentas_da_capability(cap: &str) -> &'static [&'static str] {
    const LER: &[&str] = &["fs.read"];
    const ESCREVER: &[&str] = &["fs.write", "doc.write"];
    const RODAR: &[&str] = &["shell.exec"];
    const PESQUISAR: &[&str] = &["web.search", "web.browse"];
    match cap {
        "memory.read" => &["memory.read"],
        "skill.read" => &["skill.read"],
        "database.postgresql.operate" => &["db.read"],
        "workspace.command.run" | "workspace.gate.run" | "compiler.build" => RODAR,
        "workspace.git.read"
        | "workspace.diff"
        | "tree_sitter.analyze"
        | "lsp.read"
        | "evidence.read" => LER,
        "workspace.file.write" => ESCREVER,
        c if c.starts_with("repo.") => LER,
        c if c.starts_with("knowledge.") && c.ends_with(".read") => LER,
        c if c.starts_with("research.") => PESQUISAR,
        c if c.starts_with("build.") || c.starts_with("quality.") => RODAR,
        c if c.starts_with("code.")
            || c.starts_with("documentation.")
            || c.ends_with(".implement") =>
        {
            ESCREVER
        }
        _ => &[],
    }
}

/// O que o papel autoriza em capacidades de ferramenta (antes da interseccao com o pai).
pub fn capacidades_do_papel(m: &AgentManifest) -> BTreeSet<String> {
    m.capabilities
        .iter()
        .flat_map(|c| {
            ferramentas_da_capability(c)
                .iter()
                .map(|s| s.to_string())
                .chain(std::iter::once(c.clone()))
        })
        .collect()
}

/// O que o subagente recebe: o que o papel autoriza E o pai tem, nunca mais que o pai, e
/// nunca o poder de delegar de novo.
pub fn capacidades_do_subagente(m: &AgentManifest, pai: &BTreeSet<String>) -> BTreeSet<String> {
    capacidades_do_papel(m)
        .intersection(pai)
        .filter(|c| !NUNCA_NO_SUBAGENTE.contains(&c.as_str()))
        .cloned()
        .collect()
}

/// Instrucao do papel, somada ao prompt base do motor no subagente.
pub fn prompt_do_papel(m: &AgentManifest) -> String {
    let mut s = format!(
        "You are acting as the PhxClaw team role \"{}\" (id {}, {} / {}, {}). \
Stay strictly inside this role: do only what its mission and responsibilities cover, \
respect its limits, and deliver what it must deliver. Answer in the user's language.\n",
        m.name, m.agent_id, m.macroarea, m.nucleus, m.role_type
    );
    for (rotulo, valor) in [
        ("Missao", &m.mission),
        ("Responsabilidades", &m.responsibilities),
        ("Acoes autorizadas", &m.authorized_actions),
        ("Limites (nao pode)", &m.limits),
        ("Entregaveis", &m.deliverables),
        ("Politica de execucao", &m.execution_policy),
        ("Proximo gate", &m.next_gate),
    ] {
        if !valor.trim().is_empty() {
            s.push_str(&format!("{rotulo}: {}\n", valor.trim()));
        }
    }
    s
}

/// O resultado de uma delegacao, para a ferramenta e para a CLI.
pub enum Delegacao {
    /// Papel humano: nada rodou, e o pedido volta para uma pessoa decidir.
    Humano { papel: String, pedido: String },
    Rodou {
        papel: String,
        modelo: String,
        motivo_modelo: String,
        capacidades: Vec<String>,
        tarefa: Box<Task>,
    },
}

/// Escolhe o modelo do subagente pela planilha: papel roteado para o Ollama usa o modelo
/// local configurado (ou o do pai, quando o do pai ja e local); os demais, e o local sem
/// configuracao, caem no do pai. O motivo volta escrito, porque modelo escolhido em
/// silencio e custo que ninguem explica.
pub fn escolher_modelo(
    m: &AgentManifest,
    pai: &Arc<dyn Llm>,
    local: Option<&Arc<dyn Llm>>,
) -> (Arc<dyn Llm>, String) {
    if !prefere_local(m) {
        return (
            pai.clone(),
            format!(
                "modelo do agente pai (a planilha pede {:?}, nao Ollama)",
                m.models_allowed
            ),
        );
    }
    match local {
        Some(l) => (
            l.clone(),
            format!("modelo local de {VAR_MODELO_LOCAL} (a planilha roteia para Ollama)"),
        ),
        None if pai.id().starts_with("ollama:") => (
            pai.clone(),
            "modelo do agente pai, que ja e local (a planilha roteia para Ollama)".into(),
        ),
        None => (
            pai.clone(),
            format!(
                "modelo do agente pai: a planilha roteia para Ollama, mas {VAR_MODELO_LOCAL} nao esta configurado"
            ),
        ),
    }
}

/// Delegacao de uma subtarefa a um papel. `base` e o agente pai (modelo, ferramentas,
/// politica e armazenamento); o subagente sai dele pelo mesmo laco do `parallel_research`.
pub async fn delegar(
    equipe: &Equipe,
    base: &Agent,
    papel: &str,
    tarefa: &str,
    pai: Option<&str>,
    local: Option<&Arc<dyn Llm>>,
) -> Result<Delegacao, String> {
    let m = equipe.achar(papel)?;
    if tarefa.trim().is_empty() {
        return Err("falta a tarefa a delegar".into());
    }
    if e_humano(m) {
        return Ok(Delegacao::Humano {
            papel: m.name.clone(),
            pedido: tarefa.to_string(),
        });
    }
    let caps = capacidades_do_subagente(m, &base.config.capabilities);
    let (llm, motivo_modelo) = escolher_modelo(m, &base.llm, local);
    let mut config = config_de_subagente(&base.config);
    config.capabilities = caps.clone();
    let papel_txt = prompt_do_papel(m);
    config.extra_instructions = Some(match &base.config.extra_instructions {
        Some(x) => format!("{x}\n\n{papel_txt}"),
        None => papel_txt,
    });
    // So entram as ferramentas que a politica do filho deixa rodar: o portao do motor ja
    // negaria as outras, mas lista-las seria convidar o modelo a tentar.
    let tools = base
        .tools
        .iter()
        .filter(|t| caps.contains(t.capability()))
        .cloned()
        .collect();
    let sub = Agent::new(llm, tools, config, base.store.clone());
    let modelo = sub.llm.id();
    let tarefa = rodar_subagentes(&sub, &[tarefa.to_string()], pai)
        .await
        .into_iter()
        .next()
        .ok_or("o subagente nao devolveu tarefa")?;
    Ok(Delegacao::Rodou {
        papel: m.name.clone(),
        modelo,
        motivo_modelo,
        capacidades: caps.into_iter().collect(),
        tarefa: Box::new(tarefa),
    })
}

/// O texto de uma delegacao, o mesmo na ferramenta e na CLI.
pub fn texto_da_delegacao(d: &Delegacao) -> String {
    match d {
        Delegacao::Humano { papel, pedido } => format!(
            "DECISAO HUMANA NECESSARIA: \"{papel}\" e um papel humano e nao roda como subagente. \
Leve a uma pessoa com essa autoridade: {pedido}"
        ),
        Delegacao::Rodou {
            papel,
            modelo,
            motivo_modelo,
            capacidades,
            tarefa,
        } => format!(
            "papel: {papel}\nsubtarefa: {} ({:?})\nmodelo: {modelo} -- {motivo_modelo}\ncapacidades: {}\nresposta:\n{}",
            tarefa.id,
            tarefa.status,
            if capacidades.is_empty() {
                "(nenhuma ferramenta: o papel nao autoriza nada que o pai tenha)".to_string()
            } else {
                capacidades.join(", ")
            },
            corpo_do_subagente(tarefa)
        ),
    }
}

/// Uma linha por papel, o mesmo formato na ferramenta e na CLI.
pub fn texto_da_lista(fichas: &[Ficha], total: usize) -> String {
    let mut s = format!("{} de {total} papeis\n", fichas.len());
    for f in fichas {
        let gatilho: String = f.quando_acionar.chars().take(110).collect();
        s.push_str(&format!(
            "{:>3} | {} | {} | {} | {} | {}{} | {}\n",
            f.id,
            f.nome,
            f.macroarea,
            f.tipo,
            f.criticidade,
            f.capability_principal,
            if f.humano { " | HUMANO" } else { "" },
            gatilho
        ));
    }
    s
}

/// O papel inteiro, para `team_list` com id e para `phxclaw equipe mostrar`.
pub fn texto_do_papel(equipe: &Equipe, m: &AgentManifest) -> String {
    let f = equipe.ficha(m);
    let mut s = format!(
        "{} {} ({})\nuuid: {}\nmacroarea: {} / {}\ntipo: {}\ncriticidade: {}\nexecucao: {}\nmodelos: {}{}\ncapability principal: {}\ncapabilities: {}\nferramentas que o papel autoriza: {}\n",
        f.id,
        f.nome,
        if f.humano { "humano" } else { "agente" },
        f.uuid,
        f.macroarea,
        f.nucleo,
        f.tipo,
        f.criticidade,
        f.execucao,
        f.modelos.join(", "),
        if f.prefere_local {
            " (roteado para local)"
        } else {
            ""
        },
        f.capability_principal,
        m.capabilities.join(", "),
        m.capabilities
            .iter()
            .flat_map(|c| ferramentas_da_capability(c).iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", "),
    );
    for (rotulo, valor) in [
        ("quando acionar", &m.trigger),
        ("missao", &m.mission),
        ("responsabilidades", &m.responsibilities),
        ("limites", &m.limits),
        ("entregaveis", &m.deliverables),
        ("proximo gate", &m.next_gate),
    ] {
        s.push_str(&format!("{rotulo}: {}\n", valor.trim()));
    }
    s
}

/// Minusculas, sem acento e com travessao virando hifen: quem digita "Product Owner -
/// Humano" no terminal acha o papel escrito com en-dash na planilha.
pub fn dobrar(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            '–' | '—' => '-',
            outro => outro,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn texto_opcional<'a>(args: &'a Value, nome: &str) -> Option<&'a str> {
    args.get(nome).and_then(Value::as_str)
}

/// `team_list`: a lista dos papeis, filtrada, ou um papel inteiro pelo id.
pub struct TeamListTool {
    pub equipe: Arc<Equipe>,
}

impl Tool for TeamListTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "team_list".into(),
            description: format!(
                "List the {} PhxClaw team roles (id | name | macro-area | type | criticality | main capability | when to call). \
Filter with 'macroarea' and/or 'text'; pass 'id' (number or name) to get one role in full. \
Use before team_delegate to pick the right role.",
                self.equipe.len()
            ),
            parameters: json!({"type":"object","properties":{
                "macroarea":{"type":"string","description":"part of the macro-area name, e.g. Qualidade"},
                "text":{"type":"string","description":"search in name, mission, type, trigger and capabilities"},
                "id":{"type":"string","description":"role id or name for the full description"}
            }}),
        }
    }
    fn capability(&self) -> &'static str {
        "team.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            if let Some(id) = texto_opcional(&args, "id").filter(|s| !s.trim().is_empty()) {
                let m = self.equipe.achar(id).map_err(ToolError::InvalidArguments)?;
                return Ok(ToolOutput::text(texto_do_papel(&self.equipe, m)));
            }
            let fichas = self.equipe.listar(
                texto_opcional(&args, "macroarea"),
                texto_opcional(&args, "text"),
            );
            Ok(ToolOutput::text(texto_da_lista(&fichas, self.equipe.len())))
        })
    }
}

/// `team_delegate`: uma subtarefa para um papel, rodando como subagente filho.
pub struct TeamDelegateTool {
    pub equipe: Arc<Equipe>,
    /// O agente pai sem as ferramentas de subagente (as mesmas que o `parallel_research`
    /// recebe): modelo, ferramentas, politica e armazenamento.
    pub base: Agent,
    pub local: Option<Arc<dyn Llm>>,
}

impl Tool for TeamDelegateTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "team_delegate".into(),
            description: "Delegate one self-contained sub-task to a PhxClaw team role (by id or name, see team_list). \
The role runs as a sub-agent with its own mission and limits and only the tools both it and you are allowed; \
returns its answer. Human roles do not run: they come back asking for a human decision."
                .into(),
            parameters: json!({"type":"object","properties":{
                "role":{"type":"string","description":"role id (number) or name"},
                "task":{"type":"string","description":"the sub-task, self-contained"}
            },"required":["role","task"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "team.delegate"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let papel = texto_opcional(&args, "role")
                .ok_or_else(|| ToolError::InvalidArguments("falta 'role' (id ou nome)".into()))?;
            let tarefa = texto_opcional(&args, "task")
                .ok_or_else(|| ToolError::InvalidArguments("falta 'task'".into()))?;
            let d = delegar(
                &self.equipe,
                &self.base,
                papel,
                tarefa,
                Some(&ctx.task_id),
                self.local.as_ref(),
            )
            .await
            .map_err(ToolError::InvalidArguments)?;
            Ok(ToolOutput::text(truncate_for_model(
                &texto_da_delegacao(&d),
                self.base.config.max_tool_output_chars,
            )))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dobrar_tira_acento_caixa_e_travessao() {
        assert_eq!(
            dobrar("Product Owner – Humano"),
            dobrar("product owner - humano")
        );
        assert_eq!(dobrar("Governança"), "governanca");
    }

    #[test]
    fn traducao_nunca_entrega_poder_de_delegar() {
        for c in [
            "team.dispatch",
            "task.orchestrate",
            "mission.run",
            "agent.spawn",
        ] {
            assert!(
                ferramentas_da_capability(c)
                    .iter()
                    .all(|x| !NUNCA_NO_SUBAGENTE.contains(x)),
                "{c}"
            );
        }
    }
}
