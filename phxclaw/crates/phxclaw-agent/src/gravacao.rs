//! Gravar e repetir uma tarefa: cada pedido e resposta do modelo e cada entrada e saida de
//! ferramenta vao para um JSONL; a repeticao devolve as respostas gravadas no lugar do
//! modelo e acusa, com o passo, onde o agente deixou de fazer a mesma sequencia.
//!
//! O gravador ENVOLVE o modelo e as ferramentas de um `Agent` ja montado, em vez de entrar
//! no motor: o laco continua um so, e o que se grava e exatamente o que o motor chamou.
//! Subagentes (`parallel_research`, equipe) ficam com o modelo de antes e aparecem como UMA
//! chamada de ferramenta com a saida delas -- e e isso que a repeticao precisa devolver.
//!
//! Cada passo leva o que se mediu nele (SP000030): `duracao_ms` pelo relogio de quem
//! chamou; `tokens_entrada`/`tokens_saida` SO quando o provedor os devolveu (o roteiro e o
//! repetidor nao devolvem, e o campo fica ausente -- nunca estimado); `tarefa` e `passo_pai`
//! para a chamada que roda dentro da chamada de outra tarefa (subagente). O primeiro pedido
//! ao modelo grava tambem o `prompt_sha256` do sistema e o sha de cada skill da pasta, para
//! o `avaliar` agrupar execucoes do MESMO prompt.
//!
//! Segredo se tira ANALISANDO: o JSON (argumentos, saida, texto de mensagem que seja JSON) e
//! lido, a chave secreta perde o valor e o resto e reserializado; texto que nao e JSON passa
//! pelo `scrub_secret_like` do broker, o mesmo de toda a base. Recortar texto dependeria de
//! o segredo estar escrito de um jeito; a chave `"password"` dentro de um JSON aninhado num
//! campo string so se acha lendo.

use crate::motor::Agent;
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Tool, ToolContext, ToolError, ToolOutput,
    ToolSpec,
};
use serde_json::{Map, Value, json};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Versao do formato da linha, em toda linha nova. A 2 (SP000030) acrescenta campos
/// opcionais (`duracao_ms`, tokens, `tarefa`, `passo_pai`, a linha `prompt`); o leitor
/// aceita a 1, que so nao os tem. Versao fora de `VERSOES_LIDAS` recusa, em vez de
/// repetir errado.
pub const VERSAO_GRAVACAO: u64 = 2;
pub const VERSOES_LIDAS: [u64; 2] = [1, 2];

const TARJA: &str = "[REDACTED]";

/// Chaves cujo VALOR inteiro e segredo, em qualquer profundidade. Compara-se o nome inteiro
/// ou o sufixo depois de `_` (`x_api_key`, `github_token`), nunca substring: `max_tokens`
/// e `input_tokens` sao contagem, nao credencial.
const CHAVES_SECRETAS: &[&str] = &[
    "password",
    "passwd",
    "senha",
    "secret",
    "segredo",
    "token",
    "api_key",
    "apikey",
    "authorization",
    "cookie",
    "set_cookie",
    "private_key",
    "chave_privada",
    "client_secret",
    "credential",
    "credencial",
    "bearer",
];

/// Publica porque o fluxo recusa variavel com nome de segredo pela MESMA lista: duas
/// listas divergiriam no dia em que alguem acrescentasse um nome numa so.
pub fn chave_secreta(k: &str) -> bool {
    let k = k.to_ascii_lowercase().replace('-', "_");
    CHAVES_SECRETAS
        .iter()
        .any(|s| k == *s || k.ends_with(&format!("_{s}")))
}

/// Tira segredo de um valor JSON analisando-o. Idempotente: a repeticao redige a chamada
/// viva e compara com a gravada, que ja foi redigida.
pub fn redigir(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut out = Map::with_capacity(m.len());
            for (k, x) in m {
                let novo = if chave_secreta(k) && !x.is_null() {
                    Value::String(TARJA.into())
                } else {
                    redigir(x)
                };
                out.insert(k.clone(), novo);
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(redigir).collect()),
        Value::String(s) => Value::String(redigir_texto(s)),
        outro => outro.clone(),
    }
}

/// Texto que e JSON (saida de ferramenta, corpo de API) e analisado e reserializado; o
/// resto passa pela tarja de forma de credencial.
pub fn redigir_texto(s: &str) -> String {
    let t = s.trim_start();
    if (t.starts_with('{') || t.starts_with('['))
        && let Ok(v) = serde_json::from_str::<Value>(s)
    {
        return serde_json::to_string(&redigir(&v)).unwrap_or_else(|_| TARJA.into());
    }
    phxclaw_secret_broker::scrub_secret_like(s)
}

fn redigir_mensagem(m: &Message) -> Value {
    redigir(&serde_json::to_value(m).unwrap_or(Value::Null))
}

fn erro_de_modelo(e: &LlmError) -> Value {
    let tipo = match e {
        LlmError::Transport(_) => "transporte",
        LlmError::Api { .. } => "api",
        LlmError::Parse(_) => "parse",
        LlmError::Denied(_) => "negado",
        LlmError::Credential(_) => "credencial",
    };
    json!({"tipo": tipo, "texto": redigir_texto(&e.to_string())})
}

fn erro_de_ferramenta(e: &ToolError) -> Value {
    let (tipo, texto) = match e {
        ToolError::InvalidArguments(t) => ("argumentos", t.clone()),
        ToolError::Denied(t) => ("negado", t.clone()),
        ToolError::Failed(t) => ("falhou", t.clone()),
        ToolError::Timeout(ms) => ("prazo", ms.to_string()),
    };
    json!({"tipo": tipo, "texto": redigir_texto(&texto)})
}

fn modelo_de_erro(v: &Value) -> LlmError {
    let texto = v["texto"].as_str().unwrap_or_default().to_string();
    // O texto gravado ja e a mensagem inteira; repetido, ele vira o corpo de um erro do
    // mesmo tipo, para o motor tratar como tratou na gravacao.
    match v["tipo"].as_str().unwrap_or_default() {
        "transporte" => LlmError::Transport(texto),
        "negado" => LlmError::Denied(texto),
        "credencial" => LlmError::Credential(texto),
        "api" => LlmError::Api {
            status: 0,
            body: texto,
        },
        _ => LlmError::Parse(texto),
    }
}

fn ferramenta_de_erro(v: &Value) -> ToolError {
    let texto = v["texto"].as_str().unwrap_or_default().to_string();
    match v["tipo"].as_str().unwrap_or_default() {
        "argumentos" => ToolError::InvalidArguments(texto),
        "negado" => ToolError::Denied(texto),
        "prazo" => ToolError::Timeout(texto.parse().unwrap_or(0)),
        _ => ToolError::Failed(texto),
    }
}

// ------------------------------------------------------------------ gravador

struct Saida {
    arq: std::fs::File,
    passo: u64,
    erro: Option<String>,
    /// Quantas mensagens o pedido anterior tinha: grava-se so o que entrou depois, senao a
    /// gravacao cresceria com o quadrado dos passos (cada pedido leva o historico inteiro).
    vistas: usize,
    /// Chamadas de ferramenta em curso: (tarefa, passo reservado), na ordem em que
    /// comecaram. E o que diz o `passo_pai` de uma chamada de outra tarefa.
    em_curso: Vec<(String, u64)>,
    /// A tarefa de quem grava, dita por `tarefa()` ou vista na primeira ferramenta.
    raiz: Option<String>,
    prompt_gravado: bool,
}

/// O que se mede num passo, pela mesma regua para modelo e ferramenta.
fn com_medidas(linha: &mut Value, t0: std::time::Instant) {
    linha["duracao_ms"] = json!(t0.elapsed().as_secs_f64() * 1e3);
}

/// Escreve a gravacao, uma linha por evento, com `flush` a cada linha: tarefa que morre no
/// meio deixa a gravacao ate o ultimo passo, e a API nao precisa fechar nada.
pub struct Gravador {
    saida: Mutex<Saida>,
}

impl Gravador {
    /// Cria (ou trunca) o arquivo, 0600: mesmo redigida, a gravacao carrega o conteudo da
    /// tarefa inteira.
    pub fn criar(caminho: &Path, objetivo: &str, modelo: &str) -> std::io::Result<Arc<Self>> {
        if let Some(p) = caminho.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(p)?;
        }
        let mut op = std::fs::OpenOptions::new();
        op.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            op.mode(0o600);
        }
        let arq = op.open(caminho)?;
        let g = Arc::new(Self {
            saida: Mutex::new(Saida {
                arq,
                passo: 0,
                erro: None,
                vistas: 0,
                em_curso: vec![],
                raiz: None,
                prompt_gravado: false,
            }),
        });
        g.linha(json!({
            "tipo": "cabecalho",
            "versao": VERSAO_GRAVACAO,
            "objetivo": redigir_texto(objetivo),
            "modelo": modelo,
            "data": chrono::Utc::now().to_rfc3339(),
        }));
        if let Some(e) = g.saida.lock().unwrap().erro.clone() {
            return Err(std::io::Error::other(e));
        }
        Ok(g)
    }

    /// Reabre uma gravacao que ja tem cabecalho, para acrescentar: a tarefa da API criada
    /// com plano grava a execucao so depois do sim, e a aprovacao chega noutra chamada.
    pub fn continuar(caminho: &Path) -> std::io::Result<Arc<Self>> {
        // A linha que a queda cortou sai do disco antes do `append`: senao o proximo
        // evento nasceria colado nela, e a gravacao inteira deixaria de ser legivel a
        // partir dali. E o passo recomeca contando so as linhas inteiras.
        let bytes = phxclaw_types::arquivo::aparar_cauda_cortada(caminho)?;
        let passo = String::from_utf8_lossy(&bytes)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count() as u64;
        let arq = std::fs::OpenOptions::new().append(true).open(caminho)?;
        Ok(Arc::new(Self {
            saida: Mutex::new(Saida {
                arq,
                passo,
                erro: None,
                vistas: 0,
                em_curso: vec![],
                raiz: None,
                // A gravacao continuada ja teve o primeiro pedido; o prompt de la vale.
                prompt_gravado: true,
            }),
        }))
    }

    /// A tarefa de quem grava. Sem isto, a raiz e a tarefa da primeira ferramenta chamada
    /// -- e as linhas de modelo antes dela saem sem `tarefa`.
    pub fn tarefa(&self, id: &str) {
        self.saida.lock().unwrap().raiz = Some(id.to_string());
    }

    /// Reserva o numero do passo ANTES da chamada: a linha so se escreve depois dela, e
    /// uma chamada de outra tarefa que rode no meio precisa apontar para este numero.
    fn reservar(&self) -> u64 {
        let mut s = self.saida.lock().unwrap();
        let p = s.passo;
        s.passo += 1;
        p
    }

    fn linha(&self, v: Value) {
        let p = self.reservar();
        self.linha_no_passo(v, p);
    }

    fn linha_no_passo(&self, mut v: Value, passo: u64) {
        let mut s = self.saida.lock().unwrap();
        if s.erro.is_some() {
            return;
        }
        v["passo"] = json!(passo);
        v["versao"] = json!(VERSAO_GRAVACAO);
        let mut texto = v.to_string();
        texto.push('\n');
        if let Err(e) = s
            .arq
            .write_all(texto.as_bytes())
            .and_then(|_| s.arq.flush())
        {
            s.erro = Some(format!("gravacao parou no passo {passo}: {e}"));
        }
    }

    /// Passos gravados (o cabecalho conta como o 0), ou o erro que fez a gravacao parar --
    /// gravacao que falhou no meio nao pode sair anunciada como completa.
    pub fn resultado(&self) -> Result<u64, String> {
        let s = self.saida.lock().unwrap();
        match &s.erro {
            Some(e) => Err(e.clone()),
            None => Ok(s.passo),
        }
    }
}

struct GravadorLlm {
    interno: Arc<dyn Llm>,
    g: Arc<Gravador>,
    skills: Option<phxclaw_skill_runtime::SkillFolder>,
}

/// sha256 do prompt de sistema: todas as mensagens `system` do pedido, na ordem, com
/// `\n` entre elas. E o que o motor montou (base + instrucoes do projeto + memoria +
/// skills listadas), tal como foi ao modelo.
pub fn prompt_sha256(messages: &[Message]) -> String {
    let sistema: Vec<&str> = messages
        .iter()
        .filter(|m| m.role == phxclaw_agent_core::Role::System)
        .map(|m| m.content.as_str())
        .collect();
    crate::motor::sha256_hex(sistema.join("\n").as_bytes())
}

/// nome -> sha256 do `SKILL.md` de cada skill valida da pasta, na ordem do nome. So as
/// validas: e o que o prompt lista e o que o `skill_load` carrega.
pub fn skills_sha256(
    pasta: &phxclaw_skill_runtime::SkillFolder,
) -> std::collections::BTreeMap<String, String> {
    pasta
        .scan()
        .skills
        .iter()
        .filter_map(|k| {
            let bytes = std::fs::read(pasta.root().join(&k.name).join("SKILL.md")).ok()?;
            Some((k.name.clone(), crate::motor::sha256_hex(&bytes)))
        })
        .collect()
}

impl Llm for GravadorLlm {
    fn id(&self) -> String {
        self.interno.id()
    }
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [ToolSpec],
        options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            // O prompt vai ANTES do primeiro pedido: quem le a gravacao sabe sob qual
            // sistema e quais skills ela correu antes de ver qualquer resposta.
            let primeiro = {
                let mut s = self.g.saida.lock().unwrap();
                !std::mem::replace(&mut s.prompt_gravado, true)
            };
            if primeiro {
                self.g.linha(json!({
                    "tipo": "prompt",
                    "prompt_sha256": prompt_sha256(messages),
                    "skills_sha256": self.skills.as_ref().map(skills_sha256).unwrap_or_default(),
                }));
            }
            let (passo, tarefa, passo_pai) = {
                let mut s = self.g.saida.lock().unwrap();
                let p = s.passo;
                s.passo += 1;
                // Pedido ao modelo feito enquanto uma ferramenta esta em curso e de uma
                // tarefa filha (o motor da propria tarefa espera a ferramenta acabar).
                match s.em_curso.first() {
                    Some((_, pai)) => (p, None, Some(*pai)),
                    None => (p, s.raiz.clone(), None),
                }
            };
            let t0 = std::time::Instant::now();
            let r = self.interno.chat(messages, tools, options).await;
            let desde = {
                let mut s = self.g.saida.lock().unwrap();
                let d = if messages.len() >= s.vistas {
                    s.vistas
                } else {
                    0
                };
                s.vistas = messages.len();
                d
            };
            let mut linha = json!({
                "tipo": "modelo",
                "pedido": {
                    "desde": desde,
                    "mensagens": messages[desde..].iter().map(redigir_mensagem).collect::<Vec<_>>(),
                    "ferramentas": tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
                    "opcoes": options,
                },
            });
            com_medidas(&mut linha, t0);
            if let Some(t) = tarefa {
                linha["tarefa"] = json!(t);
            }
            if let Some(p) = passo_pai {
                linha["passo_pai"] = json!(p);
            }
            match &r {
                Ok(resp) => {
                    // Provedor que nao conta tokens devolve 0/0 (roteiro, repeticao): o
                    // campo fica ausente, e a soma de quem le diz «nao informados».
                    if resp.usage.input_tokens + resp.usage.output_tokens > 0 {
                        linha["tokens_entrada"] = json!(resp.usage.input_tokens);
                        linha["tokens_saida"] = json!(resp.usage.output_tokens);
                    }
                    linha["resposta"] = redigir(&serde_json::to_value(resp).unwrap_or(Value::Null))
                }
                Err(e) => linha["erro"] = erro_de_modelo(e),
            }
            self.g.linha_no_passo(linha, passo);
            r
        })
    }
}

struct GravadorTool {
    interno: Arc<dyn Tool>,
    g: Arc<Gravador>,
}

impl Tool for GravadorTool {
    fn spec(&self) -> ToolSpec {
        self.interno.spec()
    }
    fn capability(&self) -> &'static str {
        self.interno.capability()
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let entrada = redigir(&args);
            let (passo, passo_pai) = {
                let mut s = self.g.saida.lock().unwrap();
                let p = s.passo;
                s.passo += 1;
                if s.raiz.is_none() {
                    s.raiz = Some(ctx.task_id.clone());
                }
                // O pai e a chamada em curso MAIS ANTIGA de outra tarefa: subagente nao
                // gera subagente (`agent.spawn` nunca entra no filho), entao a filha so
                // pode estar dentro de uma chamada da raiz -- e as irmas em paralelo
                // apontam todas para ela, nao uma para a outra.
                let pai = s
                    .em_curso
                    .iter()
                    .find(|(t, _)| *t != ctx.task_id)
                    .map(|(_, p)| *p);
                s.em_curso.push((ctx.task_id.clone(), p));
                (p, pai)
            };
            let t0 = std::time::Instant::now();
            let r = self.interno.run(args, ctx).await;
            self.g
                .saida
                .lock()
                .unwrap()
                .em_curso
                .retain(|(_, p)| *p != passo);
            let mut linha = json!({
                "tipo": "ferramenta",
                "nome": self.interno.spec().name,
                "argumentos": entrada,
                "tarefa": ctx.task_id,
            });
            com_medidas(&mut linha, t0);
            if let Some(p) = passo_pai {
                linha["passo_pai"] = json!(p);
            }
            match &r {
                Ok(o) => linha["saida"] = redigir(&serde_json::to_value(o).unwrap_or(Value::Null)),
                Err(e) => linha["erro"] = erro_de_ferramenta(e),
            }
            self.g.linha_no_passo(linha, passo);
            r
        })
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        self.interno.finish(task_id)
    }
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        self.interno.comando_de_shell(args)
    }
}

/// O mesmo agente, com o modelo e cada ferramenta passando pelo gravador. Capacidade,
/// regra de comando e nome continuam os da ferramenta envolvida: o portao do motor nao ve
/// diferenca.
pub fn gravando(mut a: Agent, g: &Arc<Gravador>) -> Agent {
    a.llm = Arc::new(GravadorLlm {
        interno: a.llm,
        g: g.clone(),
        skills: a.config.skills.clone(),
    });
    a.tools = a
        .tools
        .into_iter()
        .map(|t| {
            Arc::new(GravadorTool {
                interno: t,
                g: g.clone(),
            }) as Arc<dyn Tool>
        })
        .collect();
    a
}

// ------------------------------------------------------------------ leitura

/// O que se mediu num passo. Gravacao da versao 1 nao tem nada disto: tudo `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MedidasDoPasso {
    pub duracao_ms: Option<f64>,
    pub tokens_entrada: Option<u64>,
    pub tokens_saida: Option<u64>,
    pub tarefa: Option<String>,
    pub passo_pai: Option<u64>,
}

impl MedidasDoPasso {
    fn de(v: &Value) -> Self {
        Self {
            duracao_ms: v["duracao_ms"].as_f64(),
            tokens_entrada: v["tokens_entrada"].as_u64(),
            tokens_saida: v["tokens_saida"].as_u64(),
            tarefa: v["tarefa"].as_str().map(str::to_string),
            passo_pai: v["passo_pai"].as_u64(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RespostaGravada {
    pub passo: u64,
    pub resposta: Result<LlmReply, Value>,
    pub medidas: MedidasDoPasso,
}

#[derive(Debug, Clone)]
pub struct ChamadaGravada {
    pub passo: u64,
    pub nome: String,
    pub argumentos: Value,
    pub saida: Result<ToolOutput, Value>,
    pub medidas: MedidasDoPasso,
}

#[derive(Debug, Clone)]
pub struct Gravacao {
    pub versao: u64,
    pub objetivo: String,
    pub modelo: String,
    pub data: String,
    /// sha256 do prompt de sistema do primeiro pedido; ausente na versao 1.
    pub prompt_sha256: Option<String>,
    /// nome -> sha256 do SKILL.md, no primeiro pedido.
    pub skills_sha256: std::collections::BTreeMap<String, String>,
    pub modelo_respostas: Vec<RespostaGravada>,
    pub ferramentas: Vec<ChamadaGravada>,
}

/// A soma de uma tarefa dentro da gravacao (`phxclaw medir`). Tokens so somam quando TODA
/// chamada ao modelo os informou: somar as que informaram diria menos do que a tarefa
/// gastou, com cara de total.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SomaDaTarefa {
    /// `None` e a linha sem tarefa (versao 1, ou modelo antes da primeira ferramenta).
    pub tarefa: Option<String>,
    /// O passo da raiz dentro do qual esta tarefa rodou (subagente); `None` na raiz.
    pub passo_pai: Option<u64>,
    pub chamadas_modelo: usize,
    pub chamadas_ferramenta: usize,
    /// Soma das duracoes; `None` se algum passo nao a tem (versao 1).
    pub duracao_modelo_ms: Option<f64>,
    pub duracao_ferramentas_ms: Option<f64>,
    pub tokens_entrada: Option<u64>,
    pub tokens_saida: Option<u64>,
    /// Chamadas ao modelo sem contagem de tokens, de `chamadas_modelo`.
    pub modelo_sem_tokens: usize,
}

impl Gravacao {
    pub fn ler(caminho: &Path) -> Result<Self, String> {
        let texto =
            std::fs::read_to_string(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
        let mut g: Option<Gravacao> = None;
        // A ultima linha sem quebra no fim e a que a queda cortou: a gravacao vale ate o
        // passo anterior, como o `Gravador` promete («tarefa que morre no meio deixa a
        // gravacao ate o ultimo passo»). Linha ilegivel no MEIO continua sendo erro: ali
        // nao foi queda, foi outra coisa.
        let cauda_cortada = !texto.ends_with('\n');
        let ultima = texto.lines().count();
        for (n, l) in texto
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            let v: Value = match serde_json::from_str(l) {
                Ok(v) => v,
                Err(_) if cauda_cortada && n + 1 == ultima => break,
                Err(e) => {
                    return Err(format!(
                        "{}:{}: linha nao e JSON: {e}",
                        caminho.display(),
                        n + 1
                    ));
                }
            };
            let passo = v["passo"].as_u64().unwrap_or(n as u64);
            let onde = |o: &str| format!("{}:{}: {o}", caminho.display(), n + 1);
            match (v["tipo"].as_str(), g.as_mut()) {
                (Some("cabecalho"), None) => {
                    let versao = v["versao"].as_u64().unwrap_or(0);
                    if !VERSOES_LIDAS.contains(&versao) {
                        return Err(onde(&format!(
                            "versao {} da gravacao; este leitor le as {VERSOES_LIDAS:?}",
                            v["versao"]
                        )));
                    }
                    g = Some(Gravacao {
                        versao,
                        objetivo: v["objetivo"].as_str().unwrap_or_default().into(),
                        modelo: v["modelo"].as_str().unwrap_or_default().into(),
                        data: v["data"].as_str().unwrap_or_default().into(),
                        prompt_sha256: None,
                        skills_sha256: Default::default(),
                        modelo_respostas: vec![],
                        ferramentas: vec![],
                    });
                }
                (Some("prompt"), Some(g)) => {
                    g.prompt_sha256 = v["prompt_sha256"].as_str().map(str::to_string);
                    g.skills_sha256 = v["skills_sha256"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .filter_map(|(k, x)| Some((k.clone(), x.as_str()?.to_string())))
                                .collect()
                        })
                        .unwrap_or_default();
                }
                (Some("modelo"), Some(g)) => {
                    let resposta = match v.get("resposta") {
                        Some(r) => Ok(serde_json::from_value(r.clone())
                            .map_err(|e| onde(&format!("resposta ilegivel: {e}")))?),
                        None => Err(v["erro"].clone()),
                    };
                    g.modelo_respostas.push(RespostaGravada {
                        passo,
                        resposta,
                        medidas: MedidasDoPasso::de(&v),
                    });
                }
                (Some("ferramenta"), Some(g)) => {
                    let saida = match v.get("saida") {
                        Some(s) => Ok(serde_json::from_value(s.clone())
                            .map_err(|e| onde(&format!("saida ilegivel: {e}")))?),
                        None => Err(v["erro"].clone()),
                    };
                    g.ferramentas.push(ChamadaGravada {
                        passo,
                        nome: v["nome"].as_str().unwrap_or_default().into(),
                        argumentos: v["argumentos"].clone(),
                        saida,
                        medidas: MedidasDoPasso::de(&v),
                    });
                }
                (Some("cabecalho"), Some(_)) => return Err(onde("segundo cabecalho")),
                (_, None) => return Err(onde("a gravacao nao comeca pelo cabecalho")),
                (t, _) => return Err(onde(&format!("tipo de linha desconhecido: {t:?}"))),
            }
        }
        g.ok_or_else(|| format!("{}: gravacao vazia", caminho.display()))
    }

    /// Os nomes das ferramentas, na ordem em que o motor as chamou.
    pub fn sequencia(&self) -> Vec<String> {
        self.ferramentas.iter().map(|c| c.nome.clone()).collect()
    }

    /// Um sha so para a lista de skills (nome -> sha), para agrupar por ela.
    pub fn skills_sha256_junto(&self) -> Option<String> {
        // Sem a linha de prompt (versao 1) nao ha lista: o sha de um mapa vazio mentiria.
        self.prompt_sha256.as_ref()?;
        Some(crate::motor::sha256_hex(
            serde_json::to_string(&self.skills_sha256)
                .unwrap_or_default()
                .as_bytes(),
        ))
    }

    /// As somas por tarefa, a raiz primeiro e as filhas na ordem do `passo_pai`. A linha
    /// de modelo sem tarefa e sem pai conta na raiz (e dela: o motor da raiz e o unico que
    /// pede ao modelo fora de uma ferramenta em curso).
    pub fn somas(&self) -> Vec<SomaDaTarefa> {
        use std::collections::BTreeMap;
        let raiz = self
            .ferramentas
            .iter()
            .find(|c| c.medidas.passo_pai.is_none())
            .and_then(|c| c.medidas.tarefa.clone());
        // Chave: (passo_pai, tarefa). A raiz e (None, raiz).
        let mut somas: BTreeMap<(Option<u64>, Option<String>), SomaDaTarefa> = BTreeMap::new();
        let mut soma = |m: &MedidasDoPasso, modelo: bool| {
            let tarefa = match (&m.tarefa, m.passo_pai) {
                (Some(t), _) => Some(t.clone()),
                (None, None) => raiz.clone(),
                (None, Some(_)) => None,
            };
            let s = somas
                .entry((m.passo_pai, tarefa.clone()))
                .or_insert_with(|| SomaDaTarefa {
                    tarefa,
                    passo_pai: m.passo_pai,
                    chamadas_modelo: 0,
                    chamadas_ferramenta: 0,
                    duracao_modelo_ms: Some(0.0),
                    duracao_ferramentas_ms: Some(0.0),
                    tokens_entrada: Some(0),
                    tokens_saida: Some(0),
                    modelo_sem_tokens: 0,
                });
            let dur = if modelo {
                &mut s.duracao_modelo_ms
            } else {
                &mut s.duracao_ferramentas_ms
            };
            *dur = dur.zip(m.duracao_ms).map(|(a, b)| a + b);
            if modelo {
                s.chamadas_modelo += 1;
                s.tokens_entrada = s.tokens_entrada.zip(m.tokens_entrada).map(|(a, b)| a + b);
                s.tokens_saida = s.tokens_saida.zip(m.tokens_saida).map(|(a, b)| a + b);
                if m.tokens_entrada.is_none() {
                    s.modelo_sem_tokens += 1;
                }
            } else {
                s.chamadas_ferramenta += 1;
            }
        };
        for r in &self.modelo_respostas {
            soma(&r.medidas, true);
        }
        for c in &self.ferramentas {
            soma(&c.medidas, false);
        }
        somas.into_values().collect()
    }
}

// ------------------------------------------------------------------ repeticao

/// O que a repeticao faz quando o agente chama uma ferramenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoFerramentas {
    /// Devolve a saida gravada sem rodar nada: sem efeito nenhum. O arquivo que o objetivo
    /// pede nao passa a existir, e o motor recusa o `final_answer` que dependia dele -- e
    /// isso aparece como divergencia, nao some.
    Gravadas,
    /// Roda a ferramenta de verdade, sob o portao de sempre, numa tarefa nova. Saida
    /// diferente da gravada conta, mas nao e divergencia: a pergunta e a SEQUENCIA.
    Reais,
}

#[derive(Default)]
struct EstadoRepeticao {
    prox_modelo: usize,
    prox_ferramenta: usize,
    chamadas: Vec<String>,
    divergencias: Vec<String>,
    saidas_diferentes: usize,
}

pub struct Repetidor {
    gravacao: Gravacao,
    modo: ModoFerramentas,
    estado: Mutex<EstadoRepeticao>,
}

/// O veredito da repeticao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relatorio {
    pub gravada: Vec<String>,
    pub repetida: Vec<String>,
    pub respostas_usadas: usize,
    pub respostas_gravadas: usize,
    /// Em ordem; a primeira e a que importa, as outras costumam ser consequencia dela.
    pub divergencias: Vec<String>,
    pub saidas_diferentes: usize,
}

impl Relatorio {
    pub fn igual(&self) -> bool {
        self.divergencias.is_empty()
    }
}

impl Repetidor {
    pub fn novo(gravacao: Gravacao, modo: ModoFerramentas) -> Arc<Self> {
        Arc::new(Self {
            gravacao,
            modo,
            estado: Mutex::new(EstadoRepeticao::default()),
        })
    }

    pub fn gravacao(&self) -> &Gravacao {
        &self.gravacao
    }

    /// O agente montado, com o modelo trocado pelas respostas gravadas e cada ferramenta
    /// conferida contra a gravacao. A montagem (e portanto o portao) e a de quem repete.
    pub fn agente(self: &Arc<Self>, mut a: Agent) -> Agent {
        a.llm = Arc::new(RepetidorLlm(self.clone()));
        a.tools = a
            .tools
            .into_iter()
            .map(|t| {
                Arc::new(RepetidorTool {
                    interno: t,
                    r: self.clone(),
                }) as Arc<dyn Tool>
            })
            .collect();
        a
    }

    fn divergir(&self, e: &mut EstadoRepeticao, texto: String) -> String {
        e.divergencias.push(texto.clone());
        texto
    }

    /// O relatorio, com o que sobrou da gravacao sem ser pedido tambem como divergencia:
    /// repeticao que parou antes nao fez a mesma sequencia.
    pub fn relatorio(&self) -> Relatorio {
        let e = self.estado.lock().unwrap();
        let mut divergencias = e.divergencias.clone();
        if let Some(c) = self.gravacao.ferramentas.get(e.prox_ferramenta) {
            divergencias.push(format!(
                "passo {}: a gravacao chamou {} e a repeticao terminou sem chama-la ({} chamada(s) gravada(s) sem repetir)",
                c.passo,
                c.nome,
                self.gravacao.ferramentas.len() - e.prox_ferramenta
            ));
        }
        if let Some(r) = self.gravacao.modelo_respostas.get(e.prox_modelo) {
            divergencias.push(format!(
                "passo {}: a repeticao terminou com {} resposta(s) do modelo sem usar",
                r.passo,
                self.gravacao.modelo_respostas.len() - e.prox_modelo
            ));
        }
        Relatorio {
            gravada: self.gravacao.sequencia(),
            repetida: e.chamadas.clone(),
            respostas_usadas: e.prox_modelo,
            respostas_gravadas: self.gravacao.modelo_respostas.len(),
            divergencias,
            saidas_diferentes: e.saidas_diferentes,
        }
    }
}

struct RepetidorLlm(Arc<Repetidor>);

impl Llm for RepetidorLlm {
    fn id(&self) -> String {
        format!("repeticao:{}", self.0.gravacao.modelo)
    }
    fn chat<'a>(
        &'a self,
        _messages: &'a [Message],
        _tools: &'a [ToolSpec],
        _options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            let r = &self.0;
            let mut e = r.estado.lock().unwrap();
            let i = e.prox_modelo;
            let Some(g) = r.gravacao.modelo_respostas.get(i) else {
                let passo = r
                    .gravacao
                    .modelo_respostas
                    .last()
                    .map_or(0, |x| x.passo + 1);
                let t = r.divergir(
                    &mut e,
                    format!(
                        "passo {passo}: o agente pediu a resposta {} do modelo e a gravacao tem {}",
                        i + 1,
                        r.gravacao.modelo_respostas.len()
                    ),
                );
                return Err(LlmError::Denied(format!("repeticao divergiu: {t}")));
            };
            e.prox_modelo += 1;
            g.resposta.clone().map_err(|v| modelo_de_erro(&v))
        })
    }
}

struct RepetidorTool {
    interno: Arc<dyn Tool>,
    r: Arc<Repetidor>,
}

impl Tool for RepetidorTool {
    fn spec(&self) -> ToolSpec {
        self.interno.spec()
    }
    fn capability(&self) -> &'static str {
        self.interno.capability()
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let nome = self.interno.spec().name;
            let entrada = redigir(&args);
            let gravada = {
                let mut e = self.r.estado.lock().unwrap();
                e.chamadas.push(nome.clone());
                let i = e.prox_ferramenta;
                match self.r.gravacao.ferramentas.get(i) {
                    None => {
                        let t = self.r.divergir(
                            &mut e,
                            format!(
                                "chamada {}: o agente chamou {nome} e a gravacao acabou em {} chamada(s)",
                                i + 1,
                                self.r.gravacao.ferramentas.len()
                            ),
                        );
                        return Err(ToolError::Failed(format!("repeticao divergiu: {t}")));
                    }
                    Some(c) => {
                        e.prox_ferramenta += 1;
                        if c.nome != nome || c.argumentos != entrada {
                            let t = self.r.divergir(
                                &mut e,
                                format!(
                                    "passo {}: gravado {}({}), repetido {nome}({})",
                                    c.passo, c.nome, c.argumentos, entrada
                                ),
                            );
                            if self.r.modo == ModoFerramentas::Gravadas {
                                return Err(ToolError::Failed(format!("repeticao divergiu: {t}")));
                            }
                        }
                        c.clone()
                    }
                }
            };
            match self.r.modo {
                ModoFerramentas::Gravadas => gravada.saida.map_err(|v| ferramenta_de_erro(&v)),
                ModoFerramentas::Reais => {
                    let r = self.interno.run(args, ctx).await;
                    let agora = match &r {
                        Ok(o) => Ok(redigir(&serde_json::to_value(o).unwrap_or(Value::Null))),
                        Err(e) => Err(erro_de_ferramenta(e)),
                    };
                    let antes = gravada
                        .saida
                        .as_ref()
                        .map(|o| redigir(&serde_json::to_value(o).unwrap_or(Value::Null)))
                        .map_err(Clone::clone);
                    if agora != antes {
                        self.r.estado.lock().unwrap().saidas_diferentes += 1;
                    }
                    r
                }
            }
        })
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        self.interno.finish(task_id)
    }
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        self.interno.comando_de_shell(args)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Parecer do DBA (01/10/2026), defeito 3: o cabecalho prometia «tarefa que morre no
    /// meio deixa a gravacao ate o ultimo passo», e o leitor recusava o arquivo inteiro na
    /// linha cortada. Reposto: `Gravacao::ler` devolvia Err «linha nao e JSON» e o
    /// `continuar` colava o evento novo na metade da linha velha.
    #[test]
    fn cauda_cortada_e_tolerada_na_leitura_e_sai_do_disco_ao_continuar() {
        let d = std::env::temp_dir().join(format!("phx-grav-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).unwrap();
        let arq = d.join("t.jsonl");
        let g = Gravador::criar(&arq, "objetivo", "modelo").unwrap();
        g.linha(
            json!({"tipo": "ferramenta", "nome": "ls", "argumentos": {}, "saida": {"content": "a"}}),
        );
        g.linha(
            json!({"tipo": "ferramenta", "nome": "cat", "argumentos": {}, "saida": {"content": "b"}}),
        );
        drop(g);
        let inteiro = std::fs::read(&arq).unwrap();
        // A queda: a terceira ferramenta pela metade, sem quebra no fim.
        let mut cortado = inteiro.clone();
        cortado.extend_from_slice(b"{\"tipo\":\"ferramenta\",\"nome\":\"rm\",\"argu");
        std::fs::write(&arq, &cortado).unwrap();
        let lida = Gravacao::ler(&arq).unwrap();
        assert_eq!(lida.sequencia(), ["ls", "cat"]);
        // Continuar apara a cauda e o proximo evento nasce numa linha limpa, no passo certo.
        let g = Gravador::continuar(&arq).unwrap();
        g.linha(
            json!({"tipo": "ferramenta", "nome": "rm", "argumentos": {}, "saida": {"content": "c"}}),
        );
        assert_eq!(g.resultado().unwrap(), 4);
        drop(g);
        let lida = Gravacao::ler(&arq).unwrap();
        assert_eq!(lida.sequencia(), ["ls", "cat", "rm"]);
        assert_eq!(lida.ferramentas[2].passo, 3);
        // Linha ilegivel no MEIO nao e queda: continua erro.
        let mut meio = inteiro.clone();
        meio.extend_from_slice(b"{isto nao e json}\n");
        meio.extend_from_slice(b"{\"tipo\":\"ferramenta\",\"nome\":\"x\",\"argumentos\":{},\"saida\":{\"content\":\"\"},\"passo\":4}\n");
        std::fs::write(&arq, &meio).unwrap();
        let e = Gravacao::ler(&arq).unwrap_err();
        assert!(e.contains("linha nao e JSON"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn segredo_sai_analisando_inclusive_json_dentro_de_texto() {
        let v = json!({
            "url": "https://x/api?token=abc123def",
            "headers": {"Authorization": "Bearer sk-proj-ABCdef0123456789xyz"},
            "corpo": "{\"usuario\":\"ana\",\"password\":\"hunter2\",\"n\":[1,2]}",
            "max_tokens": 50,
            "github_token": "qualquer",
            "texto": "chave ghp_0123456789abcdefABCDEF no meio"
        });
        let r = redigir(&v);
        let s = r.to_string();
        for segredo in ["abc123def", "sk-proj", "hunter2", "qualquer", "ghp_0123"] {
            assert!(!s.contains(segredo), "{segredo} vazou: {s}");
        }
        // Contagem nao e credencial, e o resto do JSON aninhado sobrevive reserializado.
        assert_eq!(r["max_tokens"], 50);
        let corpo: Value = serde_json::from_str(r["corpo"].as_str().unwrap()).unwrap();
        assert_eq!(corpo["usuario"], "ana");
        assert_eq!(corpo["n"], json!([1, 2]));
        assert_eq!(redigir(&r), r, "redigir tem de ser idempotente");
    }

    #[test]
    fn segredo_em_json_com_espaco_estranho_nao_escapa_pelo_recorte() {
        // Um recorte por `"password":"` nao veria este; a analise ve.
        let t = "{ \"cfg\" : { \"Password\"\n :\t \"segredo-sem-forma\" } }";
        let r = redigir_texto(t);
        assert!(!r.contains("segredo-sem-forma"), "{r}");
    }

    /// SP000030: a gravacao da versao 1 (sem medidas, sem linha de prompt) continua lendo,
    /// com as medidas ausentes -- e nao inventadas. Reposto o defeito (leitor so da versao
    /// atual), `ler` recusava o arquivo inteiro.
    #[test]
    fn gravacao_da_versao_1_continua_lendo_sem_medidas() {
        let d = std::env::temp_dir().join(format!("phx-grav-v1-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).unwrap();
        let arq = d.join("v1.jsonl");
        std::fs::write(
            &arq,
            concat!(
                "{\"tipo\":\"cabecalho\",\"versao\":1,\"objetivo\":\"o\",\"modelo\":\"m\",\"data\":\"d\",\"passo\":0}\n",
                "{\"tipo\":\"ferramenta\",\"nome\":\"ls\",\"argumentos\":{},\"saida\":{\"content\":\"a\"},\"passo\":1}\n",
            ),
        )
        .unwrap();
        let g = Gravacao::ler(&arq).unwrap();
        assert_eq!(g.versao, 1);
        assert_eq!(g.sequencia(), ["ls"]);
        assert_eq!(g.prompt_sha256, None);
        assert_eq!(g.ferramentas[0].medidas, MedidasDoPasso::default());
        let somas = g.somas();
        assert_eq!(somas.len(), 1);
        assert_eq!(
            somas[0].duracao_ferramentas_ms, None,
            "versao 1 nao mediu: nao se soma"
        );
        // Versao fora das lidas continua recusa.
        std::fs::write(&arq, "{\"tipo\":\"cabecalho\",\"versao\":9,\"passo\":0}\n").unwrap();
        assert!(Gravacao::ler(&arq).unwrap_err().contains("versao 9"));
        let _ = std::fs::remove_dir_all(&d);
    }

    struct Soma;
    impl Tool for Soma {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: String::from("soma"),
                description: String::new(),
                parameters: json!({"type":"object"}),
            }
        }
        fn capability(&self) -> &'static str {
            "x"
        }
        fn run<'a>(
            &'a self,
            _: Value,
            _: &'a ToolContext,
        ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
            Box::pin(async { Ok(ToolOutput::text("3")) })
        }
    }

    /// Uma ferramenta que, como o `parallel_research`, roda outra ferramenta numa tarefa
    /// filha enquanto esta em curso.
    struct Pai(Arc<dyn Tool>);
    impl Tool for Pai {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: String::from("pai"),
                description: String::new(),
                parameters: json!({"type":"object"}),
            }
        }
        fn capability(&self) -> &'static str {
            "x"
        }
        fn run<'a>(
            &'a self,
            _: Value,
            ctx: &'a ToolContext,
        ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
            Box::pin(async move {
                let filha = ToolContext {
                    task_id: "filha".into(),
                    workdir: ctx.workdir.clone(),
                    timeout: ctx.timeout,
                };
                self.0.run(json!({}), &filha).await?;
                self.0.run(json!({}), &filha).await?;
                Ok(ToolOutput::text("ok"))
            })
        }
    }

    struct ModeloContado;
    impl Llm for ModeloContado {
        fn id(&self) -> String {
            "contado".into()
        }
        fn chat<'a>(
            &'a self,
            _: &'a [Message],
            _: &'a [ToolSpec],
            _: &'a LlmOptions,
        ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
            Box::pin(async {
                Ok(LlmReply {
                    content: "oi".into(),
                    tool_calls: vec![],
                    usage: phxclaw_agent_core::Usage {
                        input_tokens: 10,
                        output_tokens: 5,
                        duracao_geracao_ns: None,
                    },
                    model: "contado".into(),
                })
            })
        }
    }

    /// SP000030: cada passo leva duracao, tokens (so os que o provedor devolveu) e o
    /// `passo_pai` da chamada de outra tarefa; a linha `prompt` vem antes do primeiro
    /// pedido; e as somas saem por tarefa. Reposto o defeito (linha sem medidas), as somas
    /// davam `None` e o `passo_pai` nao existia.
    #[tokio::test]
    async fn cada_passo_leva_duracao_tokens_e_passo_pai_e_as_somas_saem_por_tarefa() {
        let d = std::env::temp_dir().join(format!("phx-grav-med-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).unwrap();
        // Uma skill na pasta, para o sha dela ir na linha de prompt.
        let skills = d.join("skills");
        std::fs::create_dir_all(skills.join("relatorio")).unwrap();
        std::fs::write(
            skills.join("relatorio/SKILL.md"),
            "---\nname: relatorio\ndescription: faz relatorio\n---\npasso 1\n",
        )
        .unwrap();
        let arq = d.join("t.jsonl");
        let g = Gravador::criar(&arq, "objetivo", "contado").unwrap();
        g.tarefa("raiz");
        let soma: Arc<dyn Tool> = Arc::new(GravadorTool {
            interno: Arc::new(Soma),
            g: g.clone(),
        });
        let pai = GravadorTool {
            interno: Arc::new(Pai(soma)),
            g: g.clone(),
        };
        let llm_sem = GravadorLlm {
            interno: Arc::new(crate::ScriptedLlm::new(vec![LlmReply {
                content: "x".into(),
                tool_calls: vec![],
                usage: Default::default(),
                model: "roteiro".into(),
            }])),
            g: g.clone(),
            skills: Some(phxclaw_skill_runtime::SkillFolder::new(&skills)),
        };
        let llm_com = GravadorLlm {
            interno: Arc::new(ModeloContado),
            g: g.clone(),
            skills: None,
        };
        let msgs = [Message::system("sistema"), Message::user("oi")];
        llm_sem
            .chat(&msgs, &[], &LlmOptions::default())
            .await
            .unwrap();
        let ctx = ToolContext {
            task_id: "raiz".into(),
            workdir: d.clone(),
            timeout: std::time::Duration::from_secs(1),
        };
        pai.run(json!({}), &ctx).await.unwrap();
        llm_com
            .chat(&msgs, &[], &LlmOptions::default())
            .await
            .unwrap();
        drop((pai, llm_sem, llm_com));
        assert_eq!(
            g.resultado().unwrap(),
            7,
            "cabecalho, prompt, 2 modelo, pai e 2 filhas"
        );

        let lida = Gravacao::ler(&arq).unwrap();
        assert_eq!(lida.versao, VERSAO_GRAVACAO);
        assert_eq!(
            lida.prompt_sha256.as_deref(),
            Some(prompt_sha256(&msgs).as_str())
        );
        assert_eq!(lida.skills_sha256.len(), 1);
        assert!(lida.skills_sha256.contains_key("relatorio"));
        // Toda linha nova leva a versao.
        let texto = std::fs::read_to_string(&arq).unwrap();
        assert!(texto.lines().all(|l| l.contains("\"versao\":2")), "{texto}");

        // Modelo sem contagem: tokens ausentes, duracao presente.
        let m0 = &lida.modelo_respostas[0].medidas;
        assert!(m0.duracao_ms.is_some() && m0.tokens_entrada.is_none());
        assert_eq!(m0.tarefa.as_deref(), Some("raiz"));
        let m1 = &lida.modelo_respostas[1].medidas;
        assert_eq!((m1.tokens_entrada, m1.tokens_saida), (Some(10), Some(5)));

        // A chamada `pai` reservou o passo 3 antes de rodar; as filhas (4 e 5) apontam
        // para ele, e a linha dele sai depois delas no arquivo.
        let por_nome = |n: &str| {
            lida.ferramentas
                .iter()
                .filter(|c| c.nome == n)
                .collect::<Vec<_>>()
        };
        let p = por_nome("pai")[0];
        assert_eq!(p.passo, 3);
        assert_eq!(p.medidas.passo_pai, None);
        assert_eq!(p.medidas.tarefa.as_deref(), Some("raiz"));
        let filhas = por_nome("soma");
        assert_eq!(filhas.len(), 2);
        for f in &filhas {
            assert_eq!(f.medidas.passo_pai, Some(3));
            assert_eq!(f.medidas.tarefa.as_deref(), Some("filha"));
            assert!(f.medidas.duracao_ms.is_some());
        }
        assert!(p.medidas.duracao_ms.unwrap() >= filhas[0].medidas.duracao_ms.unwrap());

        let somas = lida.somas();
        assert_eq!(somas.len(), 2);
        let raiz = somas.iter().find(|s| s.passo_pai.is_none()).unwrap();
        assert_eq!(raiz.tarefa.as_deref(), Some("raiz"));
        assert_eq!((raiz.chamadas_modelo, raiz.chamadas_ferramenta), (2, 1));
        assert_eq!(raiz.modelo_sem_tokens, 1);
        assert_eq!(
            raiz.tokens_entrada, None,
            "uma chamada sem contagem: nao se soma como total"
        );
        assert!(raiz.duracao_modelo_ms.is_some());
        let filha = somas.iter().find(|s| s.passo_pai == Some(3)).unwrap();
        assert_eq!(filha.tarefa.as_deref(), Some("filha"));
        assert_eq!(filha.chamadas_ferramenta, 2);
        let _ = std::fs::remove_dir_all(&d);
    }
}
