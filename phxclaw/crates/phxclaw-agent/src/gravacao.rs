//! Gravar e repetir uma tarefa: cada pedido e resposta do modelo e cada entrada e saida de
//! ferramenta vao para um JSONL; a repeticao devolve as respostas gravadas no lugar do
//! modelo e acusa, com o passo, onde o agente deixou de fazer a mesma sequencia.
//!
//! O gravador ENVOLVE o modelo e as ferramentas de um `Agent` ja montado, em vez de entrar
//! no motor: o laco continua um so, e o que se grava e exatamente o que o motor chamou.
//! Subagentes (`parallel_research`, equipe) ficam com o modelo de antes e aparecem como UMA
//! chamada de ferramenta com a saida delas -- e e isso que a repeticao precisa devolver.
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

/// Versao do formato da linha. Leitor que ve outra recusa, em vez de repetir errado.
pub const VERSAO_GRAVACAO: u64 = 1;

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

fn chave_secreta(k: &str) -> bool {
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
            }),
        }))
    }

    fn linha(&self, mut v: Value) {
        let mut s = self.saida.lock().unwrap();
        if s.erro.is_some() {
            return;
        }
        v["passo"] = json!(s.passo);
        s.passo += 1;
        let mut texto = v.to_string();
        texto.push('\n');
        if let Err(e) = s
            .arq
            .write_all(texto.as_bytes())
            .and_then(|_| s.arq.flush())
        {
            s.erro = Some(format!("gravacao parou no passo {}: {e}", s.passo - 1));
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
            match &r {
                Ok(resp) => {
                    linha["resposta"] = redigir(&serde_json::to_value(resp).unwrap_or(Value::Null))
                }
                Err(e) => linha["erro"] = erro_de_modelo(e),
            }
            self.g.linha(linha);
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
            let r = self.interno.run(args, ctx).await;
            let mut linha = json!({
                "tipo": "ferramenta",
                "nome": self.interno.spec().name,
                "argumentos": entrada,
            });
            match &r {
                Ok(o) => linha["saida"] = redigir(&serde_json::to_value(o).unwrap_or(Value::Null)),
                Err(e) => linha["erro"] = erro_de_ferramenta(e),
            }
            self.g.linha(linha);
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

#[derive(Debug, Clone)]
pub struct RespostaGravada {
    pub passo: u64,
    pub resposta: Result<LlmReply, Value>,
}

#[derive(Debug, Clone)]
pub struct ChamadaGravada {
    pub passo: u64,
    pub nome: String,
    pub argumentos: Value,
    pub saida: Result<ToolOutput, Value>,
}

#[derive(Debug, Clone)]
pub struct Gravacao {
    pub objetivo: String,
    pub modelo: String,
    pub data: String,
    pub modelo_respostas: Vec<RespostaGravada>,
    pub ferramentas: Vec<ChamadaGravada>,
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
                    if v["versao"].as_u64() != Some(VERSAO_GRAVACAO) {
                        return Err(onde(&format!(
                            "versao {} da gravacao; este leitor e da {VERSAO_GRAVACAO}",
                            v["versao"]
                        )));
                    }
                    g = Some(Gravacao {
                        objetivo: v["objetivo"].as_str().unwrap_or_default().into(),
                        modelo: v["modelo"].as_str().unwrap_or_default().into(),
                        data: v["data"].as_str().unwrap_or_default().into(),
                        modelo_respostas: vec![],
                        ferramentas: vec![],
                    });
                }
                (Some("modelo"), Some(g)) => {
                    let resposta = match v.get("resposta") {
                        Some(r) => Ok(serde_json::from_value(r.clone())
                            .map_err(|e| onde(&format!("resposta ilegivel: {e}")))?),
                        None => Err(v["erro"].clone()),
                    };
                    g.modelo_respostas.push(RespostaGravada { passo, resposta });
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
}
