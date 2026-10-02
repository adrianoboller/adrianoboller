//! Fluxos declarativos: um arquivo JSON com passos e dependencias (um DAG), cada passo uma
//! tarefa do agente, uma chamada de ferramenta ou um no de controle do motor.
//!
//! Nada aqui e motor novo. O grafo e a fila sao os do `phxclaw-task-graph` (ordem
//! topologica, ciclo recusado, dependencia desconhecida recusada, tentativas com espera);
//! o passo de agente roda pelo `ferramentas::rodar_filhas`, o MESMO laco do
//! `parallel_research` e do `team_delegate`; e o passo de ferramenta passa pelo
//! `Agent::call_tool`, o portao unico de capacidade, regras, hooks e evidencia. Um fluxo
//! com portao proprio seria a segunda copia da politica, e a que alguem esqueceria.
//!
//! Formato:
//! ```json
//! {"nome": "relatorio", "max_paralelo": 4, "teto_ms": 60000, "fluxo_de_erro": "avisa",
//!  "passos": [
//!   {"id": "coleta", "tarefa": "liste tres fatos sobre Rust"},
//!   {"id": "grava", "depende": ["coleta"], "ferramenta": "write_file",
//!    "args": {"path": "fatos.md", "content": "{{coleta}}"}},
//!   {"id": "tem", "depende": ["grava"], "se": {"caminho": "", "operador": "contem", "valor": "gravado"}},
//!   {"id": "lotes", "depende": ["tem:verdadeiro"], "lote": 2},
//!   {"id": "um_a_um", "depende": ["lotes"], "por_item": true, "ao_errar": "continuar",
//!    "ferramenta": "read_file", "args": {"path": "{{lotes[0]}}"}},
//!   {"id": "avisa", "ferramenta": "write_file", "args": {"path": "erro.txt", "content": "{{erro}}"}}
//! ]}
//! ```
//!
//! A saida de todo passo e uma LISTA DE ITENS JSON. Texto que uma ferramenta ou um
//! subagente devolve vira item: um array JSON vira N itens, um objeto vira um item, e
//! qualquer outro texto vira um item de texto -- e por isso o fluxo antigo, que so sabia
//! texto, continua rodando igual. `{{id}}` em texto de tarefa ou em argumento vira a saida
//! do passo `id`; `{{id.campo.sub[0]}}` entra pelo caminho JSON, sem avaliar codigo. So
//! de passo DECLARADO em `depende`: referencia a passo que pode nao ter terminado seria
//! uma corrida, e por isso e recusada na leitura, nao descoberta na execucao.
//!
//! Inspiracao no n8n (SP000035), nao copia -- onde este motor DIVERGE e qual restricao
//! nossa causou cada divergencia:
//! - Expressao e caminho JSON, nunca JavaScript (`expression.ts` do n8n avalia JS): uma
//!   segunda sandbox para avaliar codigo do operador seria uma segunda politica.
//! - Todo passo de ferramenta passa pelo `Agent::call_tool`; o n8n chama o `execute` do
//!   no direto. Portao unico: fluxo com portao proprio e a segunda copia da politica.
//! - `lote` nao desdobra o grafo em passos novos nem fecha ciclo (`SplitInBatches` do n8n
//!   volta ao proprio no): ele devolve itens-lote e o passo seguinte com `por_item` faz uma
//!   passada por lote. `TaskGraphError::Cycle` fica, porque ciclo e o que impede retomar
//!   com prova.
//! - Sem `pairedItem`: cada chamada pelo portao ja deixa evidencia propria no ledger; a
//!   ligacao item -> origem e o registro de evidencia.
//! - Progresso gravado por onda e `fluxo_sha256` conferido na retomada (o n8n deixa editar
//!   e retomar): saida velha nunca se aplica a definicao nova.
//! - Passo que nao disparou (porta sem itens, ramo morto do `se`) sai como `pulado` no
//!   relatorio em vez de sumir: um relatorio onde o passo some nao prova que ele nao rodou.

use crate::ferramentas::{config_de_subagente, corpo_do_subagente, rodar_filhas};
use crate::motor::Agent;
use crate::tarefa::{Task, TaskStatus};
use chrono::Utc;
use phxclaw_agent_core::{ToolCall, ToolContext};
use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_task_graph::{RetryPolicy, TaskGraph, TaskScheduler, TaskSpec, TaskStatus as Fila};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Teto de passos por fluxo: o arquivo vem do operador, mas um gerado por modelo com mil
/// passos nao pode virar mil subagentes.
pub const MAX_PASSOS: usize = 64;

/// Condicao do no `se`: `caminho` (JSON, relativo a cada item da entrada; vazio = o item
/// inteiro), `operador` e `valor`. Cada item vai para a porta `verdadeiro` ou `falso`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condicao {
    #[serde(default)]
    pub caminho: String,
    /// `igual`, `diferente`, `contem`, `maior`, `menor` ou `existe`.
    pub operador: String,
    #[serde(default)]
    pub valor: Value,
}

/// No `juntar`: `append` (todos os itens, na ordem de `depende`), `chave` (combina os
/// itens das duas dependencias pelo campo `chave`) ou `ramo` (os itens do primeiro ramo
/// que disparou).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Juncao {
    pub modo: String,
    #[serde(default)]
    pub chave: String,
}

/// O que fazer quando o passo esgota as tentativas sem terminar bem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AoErrar {
    /// Falha e bloqueia os dependentes (o comportamento de sempre).
    #[default]
    Parar,
    /// Segue com um item `{"erro": ...}` na saida principal.
    Continuar,
    /// A saida principal fica vazia e o item `{"erro": ...}` sai pela porta `erro`.
    SaidaDeErro,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passo {
    pub id: String,
    /// `id` ou `id:porta` (`cond:verdadeiro`, `cond:falso`, `x:erro`).
    #[serde(default)]
    pub depende: Vec<String>,
    /// Objetivo de um subagente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tarefa: Option<String>,
    /// Nome de ferramenta do agente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ferramenta: Option<String>,
    #[serde(default)]
    pub args: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub se: Option<Condicao>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub juntar: Option<Juncao>,
    /// Tamanho do lote: a entrada vira itens-lote de ate N itens cada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lote: Option<usize>,
    /// Falha o passo (e o fluxo) com esta mensagem; aceita `{{x}}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parar_com_erro: Option<String>,
    /// Tentativas (1 = sem nova tentativa).
    #[serde(default = "uma")]
    pub tentativas: u16,
    /// Roda uma vez por item da entrada; `{{entrada}}` e o item da vez.
    #[serde(default)]
    pub por_item: bool,
    /// Qual dependencia e a entrada (`por_item`, `se`, `lote`); padrao: a primeira.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrada: Option<String>,
    #[serde(default)]
    pub ao_errar: AoErrar,
    /// Teto do passo em milissegundos, por tentativa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teto_ms: Option<u64>,
}

fn uma() -> u16 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fluxo {
    pub nome: String,
    #[serde(default = "quatro")]
    pub max_paralelo: usize,
    pub passos: Vec<Passo>,
    /// Id de um passo FORA do grafo, que roda so quando o fluxo falha, com `{{erro}}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluxo_de_erro: Option<String>,
    /// Teto do fluxo inteiro, em milissegundos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teto_ms: Option<u64>,
}

fn quatro() -> usize {
    4
}

/// O resultado de um passo, como o fluxo o viu.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resultado {
    pub id: String,
    /// `ok`, `falhou`, `bloqueado` (dependencia que nao terminou bem), `pulado` (porta
    /// que nao disparou) ou `continuou` (falhou e `ao_errar` seguiu).
    pub estado: String,
    /// A saida em texto: o texto cru da ferramenta ou do subagente, ou os itens em JSON.
    pub saida: String,
    /// A saida como itens.
    #[serde(default)]
    pub itens: Vec<Value>,
    /// Portas nomeadas (`verdadeiro`/`falso` do `se`, `erro` do `saida_de_erro`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub portas: BTreeMap<String, Vec<Value>>,
    /// Tarefa filha, quando o passo e de agente.
    pub tarefa: Option<String>,
    pub tentativas: u16,
    /// Veio de uma execucao anterior do mesmo fluxo (retomada), sem rodar de novo.
    #[serde(default)]
    pub reaproveitado: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relatorio {
    pub tarefa: String,
    /// sha256 da definicao: retomar com outra definicao aplicaria saidas velhas a passos
    /// novos, e por isso e recusado.
    pub fluxo_sha256: String,
    pub sucesso: bool,
    /// Na ordem em que os passos TERMINARAM (a ordem topologica, por ondas).
    pub passos: Vec<Resultado>,
}

/// O tipo de um passo, decidido uma vez na validacao.
enum Tipo<'a> {
    Tarefa(&'a str),
    Ferramenta(&'a str),
    Se(&'a Condicao),
    Juntar(&'a Juncao),
    Lote(usize),
    Parar(&'a str),
}

fn tipo(p: &Passo) -> Result<Tipo<'_>, String> {
    let mut tipos = Vec::new();
    if let Some(t) = &p.tarefa {
        tipos.push(Tipo::Tarefa(t));
    }
    if let Some(f) = &p.ferramenta {
        tipos.push(Tipo::Ferramenta(f));
    }
    if let Some(c) = &p.se {
        tipos.push(Tipo::Se(c));
    }
    if let Some(j) = &p.juntar {
        tipos.push(Tipo::Juntar(j));
    }
    if let Some(n) = p.lote {
        tipos.push(Tipo::Lote(n));
    }
    if let Some(m) = &p.parar_com_erro {
        tipos.push(Tipo::Parar(m));
    }
    if tipos.len() != 1 {
        return Err(format!(
            "passo {}: diga 'tarefa', 'ferramenta', 'se', 'juntar', 'lote' OU 'parar_com_erro' \
(exatamente um)",
            p.id
        ));
    }
    Ok(tipos.remove(0))
}

/// `id:porta` -> (`id`, `Some(porta)`).
fn dep_e_porta(d: &str) -> (&str, Option<&str>) {
    match d.split_once(':') {
        Some((id, porta)) => (id, Some(porta)),
        None => (d, None),
    }
}

/// O id de passo de uma referencia `{{id.campo[0]}}`.
fn id_da_referencia(r: &str) -> &str {
    r.split(['.', '[']).next().unwrap_or(r)
}

/// A dependencia que e a entrada do passo: a declarada em `entrada` ou a primeira.
fn entrada_de(p: &Passo) -> Option<&str> {
    p.entrada
        .as_deref()
        .or_else(|| p.depende.first().map(|d| dep_e_porta(d).0))
}

/// Le e valida um fluxo: ids unicos e validos, exatamente um tipo por passo, dependencia
/// conhecida, `{{x}}` so de dependencia declarada e nenhum ciclo (o grafo confere).
pub fn ler(texto: &str) -> Result<Fluxo, String> {
    let f: Fluxo = serde_json::from_str(texto).map_err(|e| format!("fluxo invalido: {e}"))?;
    validar(&f)?;
    Ok(f)
}

pub fn validar(f: &Fluxo) -> Result<(), String> {
    if f.passos.is_empty() || f.passos.len() > MAX_PASSOS {
        return Err(format!("o fluxo precisa de 1 a {MAX_PASSOS} passos"));
    }
    if f.max_paralelo == 0 {
        return Err("max_paralelo precisa ser pelo menos 1".into());
    }
    if f.teto_ms == Some(0) {
        return Err("teto_ms do fluxo precisa ser maior que zero".into());
    }
    if let Some(e) = &f.fluxo_de_erro {
        let p = f
            .passos
            .iter()
            .find(|p| &p.id == e)
            .ok_or_else(|| format!("fluxo_de_erro aponta para '{e}', que nao existe"))?;
        if !p.depende.is_empty() {
            return Err(format!(
                "passo {e}: o fluxo de erro roda fora do grafo e nao pode ter depende"
            ));
        }
        if !matches!(tipo(p)?, Tipo::Tarefa(_) | Tipo::Ferramenta(_)) {
            return Err(format!(
                "passo {e}: o fluxo de erro precisa ser tarefa ou ferramenta"
            ));
        }
        if f.passos
            .iter()
            .any(|p| p.depende.iter().any(|d| dep_e_porta(d).0 == e))
        {
            return Err(format!(
                "passo {e}: o fluxo de erro roda fora do grafo e ninguem pode depender dele"
            ));
        }
    }
    let por_id: BTreeMap<&str, &Passo> = f.passos.iter().map(|p| (p.id.as_str(), p)).collect();
    for p in &f.passos {
        if p.id.is_empty()
            || p.id == "erro"
            || !p
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!("id de passo invalido: {:?}", p.id));
        }
        let t = tipo(p)?;
        if p.tentativas == 0 {
            return Err(format!("passo {}: tentativas comeca em 1", p.id));
        }
        if p.teto_ms == Some(0) {
            return Err(format!(
                "passo {}: teto_ms precisa ser maior que zero",
                p.id
            ));
        }
        for d in &p.depende {
            let (id, porta) = dep_e_porta(d);
            let Some(dep) = por_id.get(id) else {
                return Err(format!("passo {} depende de '{id}', que nao existe", p.id));
            };
            let portas = portas_de(dep);
            match porta {
                Some(pt) if !portas.contains(&pt) => {
                    return Err(format!(
                        "passo {} depende da porta '{pt}' de '{id}', que nao existe (portas: {})",
                        p.id,
                        if portas.is_empty() {
                            "nenhuma".to_string()
                        } else {
                            portas.join(", ")
                        }
                    ));
                }
                None if dep.se.is_some() => {
                    return Err(format!(
                        "passo {} depende de '{id}', que e um 'se': diga a porta ({id}:verdadeiro \
ou {id}:falso)",
                        p.id
                    ));
                }
                _ => {}
            }
        }
        let precisa_entrada = p.por_item || matches!(t, Tipo::Se(_) | Tipo::Lote(_));
        if precisa_entrada && p.depende.is_empty() {
            return Err(format!(
                "passo {}: por_item, se e lote precisam de uma dependencia como entrada",
                p.id
            ));
        }
        if let Some(e) = &p.entrada
            && !p.depende.iter().any(|d| dep_e_porta(d).0 == e)
        {
            return Err(format!(
                "passo {}: entrada '{e}' precisa estar em depende",
                p.id
            ));
        }
        match &t {
            Tipo::Se(c) => {
                if !OPERADORES.contains(&c.operador.as_str()) {
                    return Err(format!(
                        "passo {}: operador '{}' desconhecido (use {})",
                        p.id,
                        c.operador,
                        OPERADORES.join(", ")
                    ));
                }
            }
            Tipo::Juntar(j) => match j.modo.as_str() {
                "append" | "ramo" if p.depende.is_empty() => {
                    return Err(format!("passo {}: juntar precisa de dependencias", p.id));
                }
                "chave" if p.depende.len() != 2 || j.chave.is_empty() => {
                    return Err(format!(
                        "passo {}: juntar por chave pede exatamente duas dependencias e a 'chave'",
                        p.id
                    ));
                }
                "append" | "ramo" | "chave" => {}
                outro => {
                    return Err(format!(
                        "passo {}: modo de juntar '{outro}' desconhecido (append, chave ou ramo)",
                        p.id
                    ));
                }
            },
            Tipo::Lote(0) => return Err(format!("passo {}: lote comeca em 1", p.id)),
            _ => {}
        }
        let mut textos = Vec::new();
        if let Some(t) = &p.tarefa {
            textos.push(t.clone());
        }
        if let Some(m) = &p.parar_com_erro {
            textos.push(m.clone());
        }
        if let Some(c) = &p.se {
            juntar_textos(&c.valor, &mut textos);
        }
        juntar_textos(&p.args, &mut textos);
        let e_fluxo_de_erro = f.fluxo_de_erro.as_deref() == Some(p.id.as_str());
        for t in &textos {
            for r in referencias(t) {
                let id = id_da_referencia(&r);
                if e_fluxo_de_erro && id == "erro" {
                    continue;
                }
                if !p.depende.iter().any(|d| dep_e_porta(d).0 == id) {
                    return Err(format!(
                        "passo {} usa {{{{{r}}}}} sem declarar '{id}' em depende",
                        p.id
                    ));
                }
            }
        }
    }
    grafo(f).map(|_| ())
}

const OPERADORES: [&str; 6] = ["igual", "diferente", "contem", "maior", "menor", "existe"];

/// As portas nomeadas que um passo oferece alem da saida principal.
fn portas_de(p: &Passo) -> Vec<&'static str> {
    let mut v = Vec::new();
    if p.se.is_some() {
        v.extend(["verdadeiro", "falso"]);
    }
    if p.ao_errar == AoErrar::SaidaDeErro {
        v.push("erro");
    }
    v
}

/// O DAG do `phxclaw-task-graph`, com o mapa id -> uuid de volta. O passo do fluxo de
/// erro fica FORA: ele nao e dependencia de ninguem e so roda quando o grafo falha.
fn grafo(f: &Fluxo) -> Result<(TaskGraph, BTreeMap<Uuid, usize>), String> {
    let mut uuids: BTreeMap<&str, Uuid> = BTreeMap::new();
    for p in &f.passos {
        if uuids
            .insert(p.id.as_str(), phxclaw_types::new_uuid_v7())
            .is_some()
        {
            return Err(format!("id de passo repetido: {}", p.id));
        }
    }
    let mut specs = Vec::with_capacity(f.passos.len());
    let mut indice = BTreeMap::new();
    for (i, p) in f.passos.iter().enumerate() {
        if f.fluxo_de_erro.as_deref() == Some(p.id.as_str()) {
            continue;
        }
        let mut s = TaskSpec::new(
            p.id.clone(),
            if p.tarefa.is_some() {
                "agente"
            } else {
                "ferramenta"
            },
            Value::Null,
            p.id.clone(),
        );
        s.uuid = uuids[p.id.as_str()];
        s.dependencies = p
            .depende
            .iter()
            .map(|d| {
                let id = dep_e_porta(d).0;
                uuids
                    .get(id)
                    .copied()
                    .ok_or_else(|| format!("passo {} depende de '{id}', que nao existe", p.id))
            })
            .collect::<Result<_, _>>()?;
        // Espera curta entre tentativas: o fluxo e interativo, nao uma fila de fundo.
        s.retry = RetryPolicy {
            max_attempts: p.tentativas,
            base_delay_ms: 50,
            max_delay_ms: 2_000,
        };
        indice.insert(s.uuid, i);
        specs.push(s);
    }
    if specs.is_empty() {
        return Err("o fluxo so tem o passo de erro".into());
    }
    let g = TaskGraph::new(specs).map_err(|e| match e {
        phxclaw_task_graph::TaskGraphError::Cycle => "o fluxo tem ciclo".to_string(),
        outro => outro.to_string(),
    })?;
    Ok((g, indice))
}

fn juntar_textos(v: &Value, saida: &mut Vec<String>) {
    match v {
        Value::String(s) => saida.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| juntar_textos(x, saida)),
        Value::Object(o) => o.values().for_each(|x| juntar_textos(x, saida)),
        _ => {}
    }
}

/// Os `x` de cada `{{x}}`.
fn referencias(t: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut resto = t;
    while let Some(i) = resto.find("{{") {
        let depois = &resto[i + 2..];
        let Some(j) = depois.find("}}") else { break };
        v.push(depois[..j].trim().to_string());
        resto = &depois[j + 2..];
    }
    v
}

// ------------------------------------------------------------------ itens e expressoes

/// A saida de um passo como o motor a guarda: o texto cru (o que o fluxo antigo via) e os
/// itens (o que o fluxo novo enxerga).
#[derive(Debug, Clone, Default)]
struct Saida {
    texto: String,
    itens: Vec<Value>,
}

impl Saida {
    fn de_texto(t: String) -> Self {
        let itens = itens_de_texto(&t);
        Self { texto: t, itens }
    }
    fn de_itens(itens: Vec<Value>) -> Self {
        Self {
            texto: texto_de_itens(&itens),
            itens,
        }
    }
    /// O valor que uma expressao enxerga: um item e o item; varios sao a lista.
    fn valor(&self) -> Value {
        match self.itens.as_slice() {
            [um] => um.clone(),
            _ => Value::Array(self.itens.clone()),
        }
    }
}

/// Texto -> itens: array JSON vira N itens, objeto vira um, o resto e um item de texto.
/// Numero ou booleano em texto continuam texto: `read_file` de um arquivo com "42" nao
/// pode mudar de tipo pelas costas do fluxo antigo.
fn itens_de_texto(t: &str) -> Vec<Value> {
    let aparado = t.trim();
    if aparado.starts_with('[') || aparado.starts_with('{') {
        match serde_json::from_str::<Value>(aparado) {
            Ok(Value::Array(a)) => return a,
            Ok(o @ Value::Object(_)) => return vec![o],
            _ => {}
        }
    }
    vec![Value::String(t.to_string())]
}

fn texto_de_itens(itens: &[Value]) -> String {
    match itens {
        [Value::String(s)] => s.clone(),
        [um] => um.to_string(),
        _ => Value::Array(itens.to_vec()).to_string(),
    }
}

fn texto_de(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        outro => outro.to_string(),
    }
}

/// Resolve `campo.sub[0].x` dentro de `v`. Caminho vazio e o proprio valor.
fn pelo_caminho(v: &Value, caminho: &str) -> Option<Value> {
    let mut atual = v.clone();
    if caminho.trim().is_empty() {
        return Some(atual);
    }
    for parte in caminho.split('.') {
        let (nome, indices) = match parte.find('[') {
            Some(i) => (&parte[..i], &parte[i..]),
            None => (parte, ""),
        };
        if !nome.is_empty() {
            atual = atual.get(nome)?.clone();
        }
        let mut resto = indices;
        while let Some(r) = resto.strip_prefix('[') {
            let fim = r.find(']')?;
            let n: usize = r[..fim].trim().parse().ok()?;
            atual = atual.get(n)?.clone();
            resto = &r[fim + 1..];
        }
    }
    Some(atual)
}

/// O que um passo enxerga ao rodar: a saida de cada dependencia pelo id (ja na porta
/// pedida), e `erro` no fluxo de erro.
type Visao = BTreeMap<String, Saida>;

/// Resolve uma referencia `id` ou `id.caminho` na visao do passo.
fn resolver(r: &str, visao: &Visao) -> Result<Value, String> {
    let id = id_da_referencia(r);
    let s = visao
        .get(id)
        .ok_or_else(|| format!("expressao {{{{{r}}}}}: '{id}' nao esta na entrada do passo"))?;
    if r.len() == id.len() {
        return Ok(s.valor());
    }
    let caminho = r[id.len()..].trim_start_matches('.');
    pelo_caminho(&s.valor(), caminho).ok_or_else(|| {
        format!("expressao {{{{{r}}}}}: o caminho '{caminho}' nao existe na saida de '{id}'")
    })
}

fn substituir(t: &str, visao: &Visao) -> Result<String, String> {
    let mut r = String::with_capacity(t.len());
    let mut resto = t;
    while let Some(i) = resto.find("{{") {
        r.push_str(&resto[..i]);
        let depois = &resto[i + 2..];
        let Some(j) = depois.find("}}") else {
            r.push_str(&resto[i..]);
            return Ok(r);
        };
        let ref_ = depois[..j].trim();
        let id = id_da_referencia(ref_);
        // `{{id}}` inteiro em texto e o texto cru da saida: e o que o fluxo antigo recebia.
        if ref_ == id {
            r.push_str(&visao.get(id).map(|s| s.texto.clone()).ok_or_else(|| {
                format!("expressao {{{{{ref_}}}}}: '{id}' nao esta na entrada do passo")
            })?);
        } else {
            r.push_str(&texto_de(&resolver(ref_, visao)?));
        }
        resto = &depois[j + 2..];
    }
    r.push_str(resto);
    Ok(r)
}

/// Como `substituir`, em cada texto de um valor JSON. Um texto que e SO uma expressao vira
/// o proprio valor (lista, objeto, numero) -- e assim um passo passa itens a uma
/// ferramenta sem os achatar em texto; se o valor e texto, nada muda.
fn substituir_valor(v: &Value, visao: &Visao) -> Result<Value, String> {
    Ok(match v {
        Value::String(s) => {
            let aparado = s.trim();
            if let Some(ref_) = aparado
                .strip_prefix("{{")
                .and_then(|x| x.strip_suffix("}}"))
                .filter(|x| !x.contains("{{") && !x.contains("}}"))
            {
                let ref_ = ref_.trim();
                match resolver(ref_, visao)? {
                    Value::String(_) => Value::String(substituir(s, visao)?),
                    outro => outro,
                }
            } else {
                Value::String(substituir(s, visao)?)
            }
        }
        Value::Array(a) => Value::Array(
            a.iter()
                .map(|x| substituir_valor(x, visao))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| substituir_valor(x, visao).map(|x| (k.clone(), x)))
                .collect::<Result<_, _>>()?,
        ),
        outro => outro.clone(),
    })
}

// ------------------------------------------------------------------ nos de controle

fn como_numero(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn iguais(a: &Value, b: &Value) -> bool {
    a == b || texto_de(a) == texto_de(b)
}

/// Avalia a condicao num item. `existe` e o caminho presente e nao nulo; os outros
/// comparam o valor no caminho com `valor` (numero com numero, senao texto com texto).
fn condicao_vale(c: &Condicao, item: &Value, valor: &Value) -> bool {
    let achado = pelo_caminho(item, &c.caminho).filter(|v| !v.is_null());
    match c.operador.as_str() {
        "existe" => achado.is_some(),
        op => {
            let Some(a) = achado else {
                return op == "diferente";
            };
            match op {
                "igual" => iguais(&a, valor),
                "diferente" => !iguais(&a, valor),
                "contem" => match &a {
                    Value::String(s) => s.contains(&texto_de(valor)),
                    Value::Array(l) => l.iter().any(|x| iguais(x, valor)),
                    Value::Object(o) => o.contains_key(&texto_de(valor)),
                    _ => false,
                },
                "maior" | "menor" => {
                    let ordem = match (como_numero(&a), como_numero(valor)) {
                        (Some(x), Some(y)) => x.partial_cmp(&y),
                        _ => Some(texto_de(&a).cmp(&texto_de(valor))),
                    };
                    matches!(
                        (op, ordem),
                        ("maior", Some(std::cmp::Ordering::Greater))
                            | ("menor", Some(std::cmp::Ordering::Less))
                    )
                }
                _ => false,
            }
        }
    }
}

fn lotes(itens: &[Value], n: usize) -> Vec<Value> {
    itens
        .chunks(n.max(1))
        .map(|c| Value::Array(c.to_vec()))
        .collect()
}

/// Combina por chave: cada item da primeira entrada que acha par na segunda sai com os
/// campos dos dois (os da segunda por cima); sem par, o item fica de fora.
fn combinar_por_chave(a: &[Value], b: &[Value], chave: &str) -> Vec<Value> {
    let mut saida = Vec::new();
    for x in a {
        let Some(k) = pelo_caminho(x, chave) else {
            continue;
        };
        for y in b {
            if pelo_caminho(y, chave).is_some_and(|ky| iguais(&ky, &k)) {
                let mut m = x.as_object().cloned().unwrap_or_default();
                if let Some(o) = y.as_object() {
                    m.extend(o.clone());
                }
                saida.push(Value::Object(m));
            }
        }
    }
    saida
}

// ------------------------------------------------------------------ execucao

/// Uma passada agendada de um passo: (run, tentativa, id, o que roda, prazo).
type Passada<T> = (Uuid, u16, String, T, Option<(Duration, String)>);
/// As passadas terminadas de um passo: (run, tentativa, [(ok, texto)], tarefa filha).
type Passadas = (Uuid, u16, Vec<(bool, String)>, Option<String>);

/// O desfecho de um passo numa onda, antes de passar pela fila.
struct Desfecho {
    run: Uuid,
    tentativa: u16,
    id: String,
    ok: bool,
    saida: Saida,
    portas: BTreeMap<String, Vec<Value>>,
    tarefa: Option<String>,
    /// Erro de definicao (expressao sem caminho, `parar_com_erro`): tentar de novo nao
    /// muda nada.
    repetivel: bool,
}

/// Roda o fluxo inteiro. O fluxo e ele mesmo uma tarefa (pasta, evidencia, `task.json`):
/// os passos de ferramenta rodam na pasta dela, e os de agente sao tarefas filhas dela.
///
/// Por ondas: cada volta pega da fila tudo o que esta pronto (ate `max_paralelo`) e roda
/// junto. Passo que falha bloqueia os que dependem dele, e eles aparecem como `bloqueado`
/// no relatorio -- nunca somem.
pub async fn rodar(agente: &Agent, fluxo: &Fluxo) -> Result<Relatorio, String> {
    executar(agente, fluxo, None).await
}

/// Retoma um fluxo que parou no meio (passo que falhou, processo que caiu): o progresso
/// gravado a cada onda no `task.json` da tarefa do fluxo e o ponto de retomada. O que
/// terminou bem nao roda de novo -- a saida gravada volta para a fila como sucesso, e os
/// `{{id}}` dos passos seguintes a recebem igual.
pub async fn retomar(agente: &Agent, fluxo: &Fluxo, tarefa: &str) -> Result<Relatorio, String> {
    executar(agente, fluxo, Some(tarefa)).await
}

fn sha256_do_fluxo(f: &Fluxo) -> String {
    use sha2::Digest;
    let t = serde_json::to_string(f).unwrap_or_default();
    sha2::Sha256::digest(t.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// O estado que atravessa as ondas: saidas, portas e o que nao disparou.
#[derive(Default)]
struct Memoria {
    saidas: BTreeMap<String, Saida>,
    portas: BTreeMap<String, BTreeMap<String, Vec<Value>>>,
    /// `id` ou `id:porta` que nao disparou: dependente deles e `pulado`.
    mortos: BTreeSet<String>,
}

impl Memoria {
    fn guardar(&mut self, id: &str, saida: Saida, portas: BTreeMap<String, Vec<Value>>) {
        for (porta, itens) in &portas {
            if itens.is_empty() {
                self.mortos.insert(format!("{id}:{porta}"));
            }
        }
        self.saidas.insert(id.to_string(), saida);
        self.portas.insert(id.to_string(), portas);
    }

    fn matar(&mut self, p: &Passo) {
        self.mortos.insert(p.id.clone());
        for porta in portas_de(p) {
            self.mortos.insert(format!("{}:{porta}", p.id));
        }
    }

    /// A visao do passo e quais dependencias estao mortas.
    fn visao(&self, p: &Passo) -> (Visao, Vec<String>) {
        let mut visao = Visao::new();
        let mut mortas = Vec::new();
        for d in &p.depende {
            let (id, porta) = dep_e_porta(d);
            // A porta nomeada morre so por ela mesma: a saida principal vazia de um
            // `saida_de_erro` e justamente o que faz a porta `erro` disparar.
            if self.mortos.contains(d) {
                mortas.push(id.to_string());
            }
            let saida = match porta {
                Some(pt) => Saida::de_itens(
                    self.portas
                        .get(id)
                        .and_then(|m| m.get(pt))
                        .cloned()
                        .unwrap_or_default(),
                ),
                None => self.saidas.get(id).cloned().unwrap_or_default(),
            };
            visao.insert(id.to_string(), saida);
        }
        (visao, mortas)
    }
}

/// O passo pula quando uma dependencia nao disparou; `juntar` em `append`/`ramo` so pula
/// quando TODAS morreram, porque juntar ramos e justamente esperar o que sobreviveu.
fn pula(p: &Passo, mortas: &[String]) -> bool {
    match &p.juntar {
        Some(j) if j.modo != "chave" => mortas.len() >= p.depende.len(),
        _ => !mortas.is_empty(),
    }
}

/// O no de controle, resolvido na hora: nenhum deles chama ferramenta, e por isso nenhum
/// passa pelo portao -- eles so reorganizam itens que o portao ja deixou entrar.
fn controle(
    p: &Passo,
    t: &Tipo<'_>,
    visao: &Visao,
) -> Result<(Saida, BTreeMap<String, Vec<Value>>), String> {
    let entrada = |visao: &Visao| -> Vec<Value> {
        entrada_de(p)
            .and_then(|e| visao.get(e))
            .map(|s| s.itens.clone())
            .unwrap_or_default()
    };
    Ok(match t {
        Tipo::Se(c) => {
            let valor = substituir_valor(&c.valor, visao)?;
            let (sim, nao): (Vec<Value>, Vec<Value>) = entrada(visao)
                .into_iter()
                .partition(|item| condicao_vale(c, item, &valor));
            let mut portas = BTreeMap::new();
            portas.insert("verdadeiro".to_string(), sim.clone());
            portas.insert("falso".to_string(), nao.clone());
            let mut todos = sim;
            todos.extend(nao);
            (Saida::de_itens(todos), portas)
        }
        Tipo::Lote(n) => (Saida::de_itens(lotes(&entrada(visao), *n)), BTreeMap::new()),
        Tipo::Juntar(j) => {
            let por_dep = |d: &String| {
                visao
                    .get(dep_e_porta(d).0)
                    .map(|s| s.itens.clone())
                    .unwrap_or_default()
            };
            let itens = match j.modo.as_str() {
                "append" => p.depende.iter().flat_map(&por_dep).collect(),
                "ramo" => p
                    .depende
                    .iter()
                    .map(&por_dep)
                    .find(|v| !v.is_empty())
                    .unwrap_or_default(),
                _ => combinar_por_chave(&por_dep(&p.depende[0]), &por_dep(&p.depende[1]), &j.chave),
            };
            (Saida::de_itens(itens), BTreeMap::new())
        }
        Tipo::Parar(m) => return Err(substituir(m, visao)?),
        Tipo::Tarefa(_) | Tipo::Ferramenta(_) => unreachable!("nao e no de controle"),
    })
}

/// Quanto tempo o passo ainda pode levar: o menor entre o teto dele e o que resta do
/// fluxo. Devolve o prazo e o nome de quem o impoe, para a mensagem dizer qual estourou.
fn prazo(p: &Passo, fim_do_fluxo: Option<(Instant, u64)>) -> Option<(Duration, String)> {
    let do_passo = p.teto_ms.map(|ms| {
        (
            Duration::from_millis(ms),
            format!("teto do passo ({ms} ms)"),
        )
    });
    let do_fluxo = fim_do_fluxo.map(|(fim, ms)| {
        (
            fim.saturating_duration_since(Instant::now()),
            format!("teto do fluxo ({ms} ms)"),
        )
    });
    match (do_passo, do_fluxo) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (a, b) => a.or(b),
    }
}

async fn com_prazo<F, T>(prazo: Option<(Duration, String)>, fut: F) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    match prazo {
        None => Ok(fut.await),
        Some((d, nome)) => tokio::time::timeout(d, fut)
            .await
            .map_err(|_| format!("{nome} estourou")),
    }
}

async fn executar(
    agente: &Agent,
    fluxo: &Fluxo,
    retomada: Option<&str>,
) -> Result<Relatorio, String> {
    validar(fluxo)?;
    let (g, indice) = grafo(fluxo)?;
    let mut fila = TaskScheduler::new(g);
    let hash = sha256_do_fluxo(fluxo);
    // passo id -> saida, do que ja terminou bem na execucao anterior
    let mut anteriores: BTreeMap<String, Resultado> = BTreeMap::new();
    let mut mae = match retomada {
        None => Task::new(format!("fluxo: {}", fluxo.nome), agente.llm.id()),
        Some(id) => {
            let t = agente
                .store
                .load(id)
                .map_err(|e| format!("tarefa do fluxo {id}: {e}"))?;
            let r: Relatorio = serde_json::from_str(t.answer.as_deref().unwrap_or(""))
                .map_err(|_| format!("tarefa {id} nao tem progresso de fluxo gravado"))?;
            if r.fluxo_sha256 != hash {
                return Err(format!(
                    "a definicao do fluxo mudou desde a tarefa {id}: retomar aplicaria saidas \
velhas a passos novos; rode de novo"
                ));
            }
            for p in r
                .passos
                .into_iter()
                .filter(|p| p.estado == "ok" || p.estado == "continuou")
            {
                anteriores.insert(p.id.clone(), p);
            }
            t
        }
    };
    mae.status = TaskStatus::Running;
    mae.error = None;
    agente
        .store
        .save(&mae)
        .map_err(|e| format!("tarefa do fluxo: {e}"))?;
    let ledger = EvidenceLedger::open(agente.store.evidence_path(&mae.id))
        .map_err(|e| format!("evidencia do fluxo: {e}"))?;
    let ctx = ToolContext {
        task_id: mae.id.clone(),
        workdir: agente.store.workdir(&mae.id),
        timeout: agente.config.tool_timeout,
    };
    let _ = std::fs::create_dir_all(&ctx.workdir);
    // O subagente do passo e o do `parallel_research`: menos passos, sem perguntar.
    let sub = Agent::new(
        agente.llm.clone(),
        agente.tools.clone(),
        config_de_subagente(&agente.config),
        agente.store.clone(),
    );
    let fim_do_fluxo = fluxo
        .teto_ms
        .map(|ms| (Instant::now() + Duration::from_millis(ms), ms));
    let mut mem = Memoria::default();
    let mut feitos: Vec<Resultado> = Vec::new();
    let mut tentativas: BTreeMap<String, u16> = BTreeMap::new();
    let mut estourou = false;
    loop {
        if fim_do_fluxo.is_some_and(|(fim, _)| Instant::now() >= fim) {
            estourou = true;
            break;
        }
        let runs = fila.claim_ready(fluxo.max_paralelo, Utc::now());
        if runs.is_empty() {
            // Nada pronto: ou acabou, ou ha passo esperando a proxima tentativa.
            let esperando = fila
                .graph()
                .tasks()
                .any(|t| fila.state(&t.uuid).map(|s| s.status) == Some(Fila::Pending));
            if esperando {
                tokio::time::sleep(Duration::from_millis(25)).await;
                continue;
            }
            break;
        }
        // Separa a onda: agentes vao juntos pelo laco unico; ferramentas, pelo portao; nos
        // de controle se resolvem aqui mesmo.
        let mut prontos: Vec<Desfecho> = Vec::new();
        // (run, tentativa, id, visoes por item, tarefas filhas)
        let mut grupos_de_filhas: Vec<Passada<Vec<Task>>> = Vec::new();
        let mut chamadas: Vec<Passada<ToolCall>> = Vec::new();
        for r in &runs {
            let p = &fluxo.passos[indice[&r.task_uuid]];
            let t = tipo(p)?;
            if let Some(antes) = anteriores.remove(&p.id) {
                fila.succeed(r.uuid, json!({"saida": antes.saida}), Utc::now())
                    .map_err(|e| e.to_string())?;
                let saida = Saida {
                    texto: antes.saida.clone(),
                    // Progresso gravado antes dos itens existirem: o texto vira itens.
                    itens: if antes.itens.is_empty() {
                        itens_de_texto(&antes.saida)
                    } else {
                        antes.itens.clone()
                    },
                };
                mem.guardar(&p.id, saida, antes.portas.clone());
                if antes.estado == "continuou" && p.ao_errar == AoErrar::SaidaDeErro {
                    mem.mortos.insert(p.id.clone());
                }
                feitos.push(Resultado {
                    reaproveitado: true,
                    ..antes
                });
                continue;
            }
            let (visao, mortas) = mem.visao(p);
            if pula(p, &mortas) {
                fila.succeed(r.uuid, json!({"pulado": true}), Utc::now())
                    .map_err(|e| e.to_string())?;
                mem.matar(p);
                feitos.push(Resultado {
                    id: p.id.clone(),
                    estado: "pulado".into(),
                    saida: String::new(),
                    itens: vec![],
                    portas: BTreeMap::new(),
                    tarefa: None,
                    tentativas: 0,
                    reaproveitado: false,
                });
                continue;
            }
            *tentativas.entry(p.id.clone()).or_default() += 1;
            let desfecho_de_erro = |ok: bool, msg: String, repetivel: bool| Desfecho {
                run: r.uuid,
                tentativa: r.attempt,
                id: p.id.clone(),
                ok,
                saida: Saida::de_texto(msg),
                portas: BTreeMap::new(),
                tarefa: None,
                repetivel,
            };
            match &t {
                Tipo::Tarefa(_) | Tipo::Ferramenta(_) => {}
                outro => {
                    prontos.push(match controle(p, outro, &visao) {
                        Ok((saida, portas)) => Desfecho {
                            run: r.uuid,
                            tentativa: r.attempt,
                            id: p.id.clone(),
                            ok: true,
                            saida,
                            portas,
                            tarefa: None,
                            repetivel: false,
                        },
                        Err(e) => desfecho_de_erro(false, e, false),
                    });
                    continue;
                }
            }
            // Uma visao por passada: a inteira, ou uma por item da entrada.
            let visoes: Vec<Visao> = if p.por_item {
                let e = entrada_de(p).unwrap_or_default().to_string();
                visao
                    .get(&e)
                    .map(|s| s.itens.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|item| {
                        let mut v = visao.clone();
                        v.insert(e.clone(), Saida::de_itens(vec![item]));
                        v
                    })
                    .collect()
            } else {
                vec![visao]
            };
            if visoes.is_empty() {
                // Entrada sem itens: nao ha o que rodar, e o passo termina vazio.
                prontos.push(Desfecho {
                    run: r.uuid,
                    tentativa: r.attempt,
                    id: p.id.clone(),
                    ok: true,
                    saida: Saida::de_itens(vec![]),
                    portas: BTreeMap::new(),
                    tarefa: None,
                    repetivel: false,
                });
                continue;
            }
            let pz = prazo(p, fim_do_fluxo);
            match &t {
                Tipo::Tarefa(obj) => {
                    let mut filhas = Vec::new();
                    let mut erro = None;
                    for v in &visoes {
                        match substituir(obj, v) {
                            Ok(objetivo) => {
                                let mut t = Task::new(objetivo, sub.llm.id());
                                t.parent = Some(mae.id.clone());
                                filhas.push(t);
                            }
                            Err(e) => {
                                erro = Some(e);
                                break;
                            }
                        }
                    }
                    match erro {
                        Some(e) => prontos.push(desfecho_de_erro(false, e, false)),
                        None => {
                            grupos_de_filhas.push((r.uuid, r.attempt, p.id.clone(), filhas, pz))
                        }
                    }
                }
                Tipo::Ferramenta(nome) => {
                    for (i, v) in visoes.iter().enumerate() {
                        match substituir_valor(&p.args, v) {
                            Ok(arguments) => chamadas.push((
                                r.uuid,
                                r.attempt,
                                p.id.clone(),
                                ToolCall {
                                    id: format!("{}-{}-{i}", p.id, r.attempt),
                                    name: nome.to_string(),
                                    arguments,
                                },
                                pz.clone(),
                            )),
                            Err(e) => {
                                prontos.push(desfecho_de_erro(false, e, false));
                                chamadas.retain(|c| c.2 != p.id);
                                break;
                            }
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
        let fut_ferramentas =
            futures_util::future::join_all(chamadas.iter().map(|(_, _, _, c, pz)| {
                com_prazo(pz.clone(), agente.call_tool(c, &ctx, &ledger, &mae.id))
            }));
        let fut_filhas =
            futures_util::future::join_all(grupos_de_filhas.iter().map(|(_, _, _, filhas, pz)| {
                com_prazo(pz.clone(), rodar_filhas(&sub, filhas.clone()))
            }));
        let (terminadas, respostas) = tokio::join!(fut_filhas, fut_ferramentas);
        // Junta as passadas de cada passo, na ordem dos itens: qualquer passada que falhou
        // falha o passo com o motivo dela.
        let mut por_passo: BTreeMap<String, Passadas> = BTreeMap::new();
        for ((run, tentativa, id, filhas, _), r) in grupos_de_filhas.into_iter().zip(terminadas) {
            let entrada = por_passo
                .entry(id)
                .or_insert((run, tentativa, vec![], None));
            match r {
                Ok(ts) => {
                    entrada.3 = ts.first().map(|t| t.id.clone());
                    for t in ts {
                        entrada
                            .2
                            .push((t.status == TaskStatus::Completed, corpo_do_subagente(&t)));
                    }
                }
                Err(e) => {
                    entrada.3 = filhas.first().map(|t| t.id.clone());
                    entrada.2.push((false, e));
                }
            }
        }
        for ((run, tentativa, id, _, _), r) in chamadas.into_iter().zip(respostas) {
            let entrada = por_passo
                .entry(id)
                .or_insert((run, tentativa, vec![], None));
            entrada.2.push(match r {
                Ok((texto, desfecho, _)) => (desfecho == "ok", texto),
                Err(e) => (false, e),
            });
        }
        for (id, (run, tentativa, passadas, tarefa)) in por_passo {
            let falha = passadas.iter().find(|(ok, _)| !ok).map(|(_, t)| t.clone());
            prontos.push(match falha {
                Some(motivo) => Desfecho {
                    run,
                    tentativa,
                    id,
                    ok: false,
                    saida: Saida::de_texto(motivo),
                    portas: BTreeMap::new(),
                    tarefa,
                    repetivel: true,
                },
                None => {
                    let saida = match passadas.as_slice() {
                        [(_, t)] => Saida::de_texto(t.clone()),
                        muitos => Saida::de_itens(
                            muitos.iter().flat_map(|(_, t)| itens_de_texto(t)).collect(),
                        ),
                    };
                    Desfecho {
                        run,
                        tentativa,
                        id,
                        ok: true,
                        saida,
                        portas: BTreeMap::new(),
                        tarefa,
                        repetivel: false,
                    }
                }
            });
        }
        let agora = Utc::now();
        for mut d in prontos {
            let p = fluxo
                .passos
                .iter()
                .find(|p| p.id == d.id)
                .expect("passo da onda");
            let n = tentativas[&d.id];
            if d.ok {
                if p.ao_errar == AoErrar::SaidaDeErro {
                    d.portas.insert("erro".into(), vec![]);
                }
                fila.succeed(d.run, json!({"saida": d.saida.texto}), agora)
                    .map_err(|e| e.to_string())?;
                mem.guardar(&d.id, d.saida.clone(), d.portas.clone());
                feitos.push(Resultado {
                    id: d.id,
                    estado: "ok".into(),
                    saida: d.saida.texto,
                    itens: d.saida.itens,
                    portas: d.portas,
                    tarefa: d.tarefa,
                    tentativas: n,
                    reaproveitado: false,
                });
                continue;
            }
            let motivo = d.saida.texto.clone();
            let ultima = !d.repetivel || d.tentativa >= p.tentativas;
            if !ultima {
                fila.fail(d.run, motivo, true, agora)
                    .map_err(|e| e.to_string())?;
                continue;
            }
            let item_de_erro = vec![json!({"erro": motivo, "passo": d.id})];
            let (estado, saida, portas) = match p.ao_errar {
                AoErrar::Parar => {
                    fila.fail(d.run, motivo.clone(), false, agora)
                        .map_err(|e| e.to_string())?;
                    ("falhou", Saida::de_texto(motivo), BTreeMap::new())
                }
                AoErrar::Continuar => {
                    fila.succeed(d.run, json!({"erro": motivo}), agora)
                        .map_err(|e| e.to_string())?;
                    let s = Saida {
                        texto: motivo,
                        itens: item_de_erro,
                    };
                    mem.guardar(&d.id, s.clone(), BTreeMap::new());
                    ("continuou", s, BTreeMap::new())
                }
                AoErrar::SaidaDeErro => {
                    fila.succeed(d.run, json!({"erro": motivo}), agora)
                        .map_err(|e| e.to_string())?;
                    let mut portas = BTreeMap::new();
                    portas.insert("erro".to_string(), item_de_erro);
                    let s = Saida {
                        texto: motivo,
                        itens: vec![],
                    };
                    mem.guardar(&d.id, s.clone(), portas.clone());
                    // A saida principal nao disparou: quem depende dela pula.
                    mem.mortos.insert(d.id.clone());
                    ("continuou", s, portas)
                }
            };
            feitos.push(Resultado {
                id: d.id,
                estado: estado.into(),
                saida: saida.texto,
                itens: saida.itens,
                portas,
                tarefa: d.tarefa,
                tentativas: n,
                reaproveitado: false,
            });
        }
        // O ponto de retomada: o progresso vai para o disco a cada onda, e um processo
        // que cair no meio deixa o que ja terminou bem gravado.
        mae.answer = serde_json::to_string_pretty(&Relatorio {
            tarefa: mae.id.clone(),
            fluxo_sha256: hash.clone(),
            sucesso: false,
            passos: feitos.clone(),
        })
        .ok();
        mae.updated_at = Utc::now();
        let _ = agente.store.save(&mae);
    }
    // O que nunca rodou: ficou bloqueado por uma dependencia que nao terminou bem, ou o
    // teto do fluxo estourou antes da vez dele -- e o relatorio diz qual dos dois.
    for t in fila.graph().tasks() {
        let p = &fluxo.passos[indice[&t.uuid]];
        if !feitos.iter().any(|r| r.id == p.id) {
            feitos.push(Resultado {
                id: p.id.clone(),
                estado: if estourou { "falhou" } else { "bloqueado" }.into(),
                saida: if estourou {
                    format!(
                        "teto do fluxo ({} ms) estourou antes de rodar",
                        fluxo.teto_ms.unwrap_or_default()
                    )
                } else {
                    String::new()
                },
                itens: vec![],
                portas: BTreeMap::new(),
                tarefa: None,
                tentativas: 0,
                reaproveitado: false,
            });
        }
    }
    let sucesso = feitos
        .iter()
        .all(|r| matches!(r.estado.as_str(), "ok" | "continuou" | "pulado"));
    // O fluxo de erro: roda pelo MESMO portao e laco dos outros passos, com `{{erro}}` =
    // os passos que falharam e o motivo de cada um. O fluxo continua falho.
    if !sucesso && let Some(id) = &fluxo.fluxo_de_erro {
        let p = fluxo.passos.iter().find(|p| &p.id == id).expect("validado");
        let falhos: Vec<Value> = feitos
            .iter()
            .filter(|r| r.estado == "falhou")
            .map(|r| json!({"passo": r.id, "motivo": r.saida}))
            .collect();
        let mut visao = Visao::new();
        visao.insert(
            "erro".into(),
            Saida::de_itens(vec![json!({"fluxo": fluxo.nome, "passos": falhos})]),
        );
        let pz = prazo(p, fim_do_fluxo);
        let (ok, texto, tarefa) = match tipo(p)? {
            Tipo::Tarefa(obj) => match substituir(obj, &visao) {
                Ok(objetivo) => {
                    let mut t = Task::new(objetivo, sub.llm.id());
                    t.parent = Some(mae.id.clone());
                    let tid = t.id.clone();
                    match com_prazo(pz, rodar_filhas(&sub, vec![t])).await {
                        Ok(ts) => {
                            let t = &ts[0];
                            (
                                t.status == TaskStatus::Completed,
                                corpo_do_subagente(t),
                                Some(tid),
                            )
                        }
                        Err(e) => (false, e, Some(tid)),
                    }
                }
                Err(e) => (false, e, None),
            },
            Tipo::Ferramenta(nome) => match substituir_valor(&p.args, &visao) {
                Ok(arguments) => {
                    let c = ToolCall {
                        id: format!("{}-erro", p.id),
                        name: nome.to_string(),
                        arguments,
                    };
                    match com_prazo(pz, agente.call_tool(&c, &ctx, &ledger, &mae.id)).await {
                        Ok((texto, desfecho, _)) => (desfecho == "ok", texto, None),
                        Err(e) => (false, e, None),
                    }
                }
                Err(e) => (false, e, None),
            },
            _ => unreachable!("validado"),
        };
        let s = Saida::de_texto(texto);
        feitos.push(Resultado {
            id: id.clone(),
            estado: if ok { "ok" } else { "falhou" }.into(),
            saida: s.texto,
            itens: s.itens,
            portas: BTreeMap::new(),
            tarefa,
            tentativas: 1,
            reaproveitado: false,
        });
    }
    let relatorio = Relatorio {
        tarefa: mae.id.clone(),
        fluxo_sha256: hash,
        sucesso,
        passos: feitos,
    };
    mae.status = if sucesso {
        TaskStatus::Completed
    } else {
        TaskStatus::Failed
    };
    mae.answer = serde_json::to_string_pretty(&relatorio).ok();
    if !sucesso {
        mae.error = Some(if estourou {
            format!(
                "teto do fluxo ({} ms) estourou",
                fluxo.teto_ms.unwrap_or_default()
            )
        } else {
            "passo falhou ou ficou bloqueado".into()
        });
    }
    mae.updated_at = Utc::now();
    let _ = agente.store.save(&mae);
    Ok(relatorio)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminho_json_resolve_campo_indice_e_vazio() {
        let v = json!({"a": {"b": [10, {"c": "x"}]}, "n": 3});
        assert_eq!(pelo_caminho(&v, "a.b[1].c"), Some(json!("x")));
        assert_eq!(pelo_caminho(&v, "a.b[0]"), Some(json!(10)));
        assert_eq!(pelo_caminho(&v, ""), Some(v.clone()));
        assert_eq!(pelo_caminho(&v, "a.z"), None);
        assert_eq!(pelo_caminho(&v, "a.b[9]"), None);
        assert_eq!(id_da_referencia("a.b[1].c"), "a");
        assert_eq!(id_da_referencia("a[0]"), "a");
        assert_eq!(id_da_referencia("a"), "a");
    }

    #[test]
    fn texto_vira_itens_sem_mudar_de_tipo_pelas_costas() {
        assert_eq!(itens_de_texto("[1, 2]"), vec![json!(1), json!(2)]);
        assert_eq!(itens_de_texto("{\"a\":1}"), vec![json!({"a":1})]);
        assert_eq!(itens_de_texto("42"), vec![json!("42")]);
        assert_eq!(itens_de_texto("[nao e json"), vec![json!("[nao e json")]);
        assert_eq!(texto_de_itens(&[json!("x")]), "x");
        assert_eq!(texto_de_itens(&[json!(1), json!(2)]), "[1,2]");
    }

    #[test]
    fn operadores_da_condicao() {
        let c = |op: &str, caminho: &str| Condicao {
            caminho: caminho.into(),
            operador: op.into(),
            valor: Value::Null,
        };
        let item = json!({"n": 5, "s": "abc", "l": [1, 2], "o": {"k": 1}});
        assert!(condicao_vale(&c("igual", "n"), &item, &json!(5)));
        assert!(condicao_vale(&c("igual", "n"), &item, &json!("5")));
        assert!(condicao_vale(&c("diferente", "n"), &item, &json!(6)));
        assert!(condicao_vale(&c("diferente", "nada"), &item, &json!(6)));
        assert!(condicao_vale(&c("contem", "s"), &item, &json!("b")));
        assert!(condicao_vale(&c("contem", "l"), &item, &json!(2)));
        assert!(condicao_vale(&c("contem", "o"), &item, &json!("k")));
        assert!(condicao_vale(&c("maior", "n"), &item, &json!(4)));
        assert!(!condicao_vale(&c("maior", "n"), &item, &json!("10")));
        assert!(condicao_vale(&c("menor", "n"), &item, &json!("10")));
        assert!(condicao_vale(&c("existe", "o.k"), &item, &Value::Null));
        assert!(!condicao_vale(&c("existe", "o.z"), &item, &Value::Null));
    }

    #[test]
    fn combinar_por_chave_e_juncao_interna() {
        let a = vec![json!({"id": 1, "x": "a"}), json!({"id": 2, "x": "b"})];
        let b = vec![json!({"id": 2, "y": "B"}), json!({"id": 3, "y": "C"})];
        assert_eq!(
            combinar_por_chave(&a, &b, "id"),
            vec![json!({"id": 2, "x": "b", "y": "B"})]
        );
        assert_eq!(lotes(&[json!(1), json!(2), json!(3)], 2).len(), 2);
    }
}
