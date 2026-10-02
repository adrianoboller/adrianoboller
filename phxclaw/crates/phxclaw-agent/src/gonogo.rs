//! O conselho de integradores: cada integrador registra o seu parecer sobre uma integracao
//! (OK, ou NOGO com os erros) e a decisao sai dos pareceres vigentes -- ordem do dono,
//! 01/10/2026 (papel 111, `config/agents/111-integrador.agent.json`).
//!
//! | pareceres vigentes                         | decisao  |
//! |--------------------------------------------|----------|
//! | algum NOGO                                 | NOGO     |
//! | falta parecer de algum membro do conselho  | AGUARDAR |
//! | todos registrados e todos OK               | GO       |
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **O conselho nasce declarado e nao muda.** Quem abre a integracao (`abrir`) diz quem
//!   sao os integradores; registro de nome fora da lista e recusado, integracao nao aberta
//!   nao aceita parecer, e conselho vazio nunca da GO. Antes, quem registrava entrava na
//!   lista: numa integracao nova, «Qualquer OK» sozinho dava GO.
//! - **Parecer e de quem o assinou, e a assinatura e conferida.** O primeiro registro de
//!   cada integrador cria a identidade dele, e os seguintes tem de apresenta-la:
//!   - pela CLI, um token aleatorio de 256 bits, guardado no SecretBroker da pasta e
//!     mostrado UMA vez; a conferencia compara o SHA-256 do token apresentado com o do
//!     envelope, sem abrir o cofre;
//!   - pela ferramenta do agente, a tarefa que registrou: so ela registra de novo com
//!     aquele nome (o modelo nao escolhe o `task_id`, o motor o da).
//!
//!   Antes o nome vinha do argumento e qualquer um gravava «Ana OK» por cima do NOGO de
//!   Ana. O vigente de cada integrador continua sendo o da revisao mais alta DELE, entao um
//!   NOGO so deixa de valer quando o mesmo integrador registra parecer novo, e nao existe
//!   contagem de votos.
//! - **O que continua dependendo de confianca**: quem abre a integracao escolhe o conselho,
//!   e o primeiro a registrar um nome fica com ele. As duas coisas ficam gravadas
//!   (`aberta_por`, a tarefa ou «cli» de cada parecer).
//! - **O historico fica.** Cada registro e uma linha nova com revisao e carimbo.
//! - **Gravacao atomica e serializada entre processos.** Um arquivo por integracao em
//!   `<pasta do agente>/gonogo/`, trocado por renomeacao depois do `fsync`, e o
//!   ler-mudar-gravar acontece sob trava exclusiva (`File::lock`) de um `.trava` ao lado.
//!   As identidades moram em `integradores.json`, com trava propria, tomada sempre DEPOIS
//!   da trava da integracao (ordem fixa: duas travas nunca se esperam em cruz).
//!
//! A ferramenta `go_no_go` e a CLI `phxclaw gonogo` chamam as MESMAS funcoes deste modulo.

use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Versao do formato em disco. Arquivo de versao diferente e recusado: ler um formato que
/// este binario nao conhece poderia esquecer um NOGO. O 1 nao tinha conselho declarado.
pub const FORMATO: u32 = 2;
/// Subpasta da pasta do agente.
pub const PASTA: &str = "gonogo";
/// O registro das identidades, na mesma pasta.
const IDENTIDADES: &str = "integradores.json";
/// Espaco dos tokens no SecretBroker da pasta.
const ESPACO: &str = "gonogo";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Veredito {
    #[serde(rename = "OK")]
    Ok,
    #[serde(rename = "NOGO")]
    NoGo,
}

impl Veredito {
    pub fn do_texto(s: &str) -> Result<Self, String> {
        match s
            .trim()
            .to_ascii_uppercase()
            .replace(['-', '_', ' '], "")
            .as_str()
        {
            "OK" => Ok(Veredito::Ok),
            "NOGO" => Ok(Veredito::NoGo),
            _ => Err(format!("parecer desconhecido: {s} (OK ou NOGO)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parecer {
    pub integrador: String,
    pub parecer: Veredito,
    #[serde(default)]
    pub erros: Vec<String>,
    /// RFC 3339, UTC.
    pub carimbo: String,
    /// 1, 2, 3... por integrador.
    pub revisao: u64,
    /// A tarefa que registrou, ou `None` pela CLI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tarefa: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Integracao {
    pub formato: u32,
    pub integracao: String,
    /// Os integradores declarados na abertura. Nao muda.
    pub conselho: BTreeSet<String>,
    /// A tarefa que abriu, ou `cli`.
    pub aberta_por: String,
    pub aberta_em: String,
    pub pareceres: Vec<Parecer>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "decisao")]
pub enum Decisao {
    #[serde(rename = "GO")]
    Go,
    /// Quem deu NOGO e com quais erros.
    #[serde(rename = "NOGO")]
    NoGo { por: BTreeMap<String, Vec<String>> },
    /// Membros do conselho sem parecer.
    #[serde(rename = "AGUARDAR")]
    Aguardar { faltam: Vec<String> },
}

impl Decisao {
    pub fn nome(&self) -> &'static str {
        match self {
            Decisao::Go => "GO",
            Decisao::NoGo { .. } => "NOGO",
            Decisao::Aguardar { .. } => "AGUARDAR",
        }
    }

    /// Codigo de saida da CLI: script que espera o Go distingue os tres sem ler texto.
    pub fn codigo(&self) -> i32 {
        match self {
            Decisao::Go => 0,
            Decisao::NoGo { .. } => 2,
            Decisao::Aguardar { .. } => 3,
        }
    }
}

impl Integracao {
    /// O parecer vigente de cada integrador: o da revisao mais alta dele.
    pub fn vigentes(&self) -> BTreeMap<&str, &Parecer> {
        let mut v: BTreeMap<&str, &Parecer> = BTreeMap::new();
        for p in &self.pareceres {
            let e = v.entry(p.integrador.as_str()).or_insert(p);
            if p.revisao >= e.revisao {
                *e = p;
            }
        }
        v
    }

    pub fn decidir(&self) -> Decisao {
        let vig = self.vigentes();
        let por: BTreeMap<String, Vec<String>> = vig
            .values()
            .filter(|p| p.parecer == Veredito::NoGo)
            .map(|p| (p.integrador.clone(), p.erros.clone()))
            .collect();
        if !por.is_empty() {
            return Decisao::NoGo { por };
        }
        let faltam: Vec<String> = self
            .conselho
            .iter()
            .filter(|i| !vig.contains_key(i.as_str()))
            .cloned()
            .collect();
        // Conselho vazio (arquivo editado a mao) nunca e unanimidade de ninguem.
        if !faltam.is_empty() || self.conselho.is_empty() {
            return Decisao::Aguardar { faltam };
        }
        Decisao::Go
    }
}

/// Id de integracao vira nome de arquivo: so o que nao escapa da pasta.
fn validar_id(id: &str) -> Result<&str, String> {
    let id = id.trim();
    let ok = !id.is_empty()
        && id.len() <= 100
        && !id.starts_with('.')
        && id != "integradores"
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if ok {
        Ok(id)
    } else {
        Err(format!(
            "id de integracao invalido: {id:?} (letras, digitos, '.', '-', '_'; ate 100)"
        ))
    }
}

fn validar_nome(n: &str) -> Result<String, String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 100 || n.chars().any(char::is_control) {
        return Err(format!(
            "nome de integrador invalido: {n:?} (1 a 100 caracteres)"
        ));
    }
    Ok(n.to_string())
}

/// Quem esta registrando: a tarefa do agente, ou a CLI com (ou sem, no primeiro registro)
/// a credencial do integrador.
#[derive(Debug, Clone, PartialEq)]
pub enum Identidade {
    Tarefa(String),
    Credencial(Option<String>),
}

impl Identidade {
    fn rotulo(&self) -> String {
        match self {
            Identidade::Tarefa(t) => t.clone(),
            Identidade::Credencial(_) => "cli".into(),
        }
    }
}

/// Como um integrador prova quem e.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo")]
enum Prova {
    #[serde(rename = "tarefa")]
    Tarefa { tarefa: String },
    #[serde(rename = "credencial")]
    Credencial,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Identidades {
    formato: u32,
    integradores: BTreeMap<String, Prova>,
}

/// O registro de pareceres de uma pasta de agente.
#[derive(Debug, Clone)]
pub struct Conselho {
    pub pasta: PathBuf,
}

/// O que um registro pede.
#[derive(Debug, Clone, Default)]
pub struct Pedido {
    pub integracao: String,
    pub integrador: String,
    pub parecer: Option<Veredito>,
    pub erros: Vec<String>,
}

/// O resultado de um registro. `credencial` so vem no PRIMEIRO registro de um integrador
/// pela CLI: e a unica vez que ela aparece.
#[derive(Debug, Clone)]
pub struct Registro {
    pub parecer: Parecer,
    pub integracao: Integracao,
    pub credencial: Option<String>,
}

fn agora() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn sha256_hex(t: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(t.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Trava exclusiva entre processos, solta quando o arquivo fecha. O motor e o de toda a
/// base (`phxclaw_types::arquivo`); aqui so o erro ganha o caminho.
fn travar(arq: &Path) -> Result<std::fs::File, String> {
    phxclaw_types::arquivo::travar(arq).map_err(|e| format!("{}: {e}", arq.display()))
}

/// Troca atomica (temporario por pid, fsync, renomeacao, fsync da pasta) pelo mesmo
/// motor dos outros seis arquivos que se trocam assim. Este foi o modelo dele.
fn gravar_atomico(arq: &Path, texto: &str) -> Result<(), String> {
    let mut bytes = texto.as_bytes().to_vec();
    bytes.push(b'\n');
    phxclaw_types::arquivo::gravar_atomico(arq, &bytes)
        .map_err(|e| format!("{}: {e}", arq.display()))
}

impl Conselho {
    pub fn da_pasta_do_agente(agente: &Path) -> Self {
        Self {
            pasta: agente.join(PASTA),
        }
    }

    fn arquivo(&self, id: &str) -> PathBuf {
        self.pasta.join(format!("{id}.json"))
    }

    /// A integracao gravada (`None`: ninguem abriu ainda).
    pub fn ler(&self, id: &str) -> Result<Option<Integracao>, String> {
        let id = validar_id(id)?;
        let arq = self.arquivo(id);
        let t = match std::fs::read_to_string(&arq) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", arq.display())),
        };
        let v: Value = serde_json::from_str(&t).map_err(|e| format!("{}: {e}", arq.display()))?;
        let f = v["formato"].as_u64().unwrap_or(0);
        if f != u64::from(FORMATO) {
            return Err(format!(
                "{}: formato {f}, este binario le o {FORMATO}",
                arq.display()
            ));
        }
        let i: Integracao =
            serde_json::from_value(v).map_err(|e| format!("{}: {e}", arq.display()))?;
        if i.integracao != id {
            return Err(format!(
                "{}: o arquivo diz ser da integracao {}",
                arq.display(),
                i.integracao
            ));
        }
        Ok(Some(i))
    }

    /// Integracao nao aberta e AGUARDAR: ninguem declarou o conselho ainda.
    pub fn decidir(&self, id: &str) -> Result<Decisao, String> {
        Ok(match self.ler(id)? {
            Some(i) => i.decidir(),
            None => Decisao::Aguardar { faltam: vec![] },
        })
    }

    /// Abre a integracao com o conselho declarado. Abrir de novo e recusado: o conselho nao
    /// muda depois do primeiro parecer.
    pub fn abrir(
        &self,
        id: &str,
        conselho: &[String],
        quem: &Identidade,
    ) -> Result<Integracao, String> {
        let id = validar_id(id)?.to_string();
        let conselho = conselho
            .iter()
            .map(|n| validar_nome(n))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if conselho.is_empty() {
            return Err("conselho vazio: declare os integradores desta integracao".into());
        }
        std::fs::create_dir_all(&self.pasta)
            .map_err(|e| format!("{}: {e}", self.pasta.display()))?;
        let _t = travar(&self.pasta.join(format!("{id}.trava")))?;
        if self.ler(&id)?.is_some() {
            return Err(format!(
                "integracao {id} ja aberta; o conselho dela nao muda"
            ));
        }
        let i = Integracao {
            formato: FORMATO,
            integracao: id.clone(),
            conselho,
            aberta_por: quem.rotulo(),
            aberta_em: agora(),
            pareceres: vec![],
        };
        self.gravar(&i)?;
        Ok(i)
    }

    /// Registra o parecer, conferindo que o nome e do conselho e que quem registra e ele.
    pub fn registrar(&self, p: &Pedido, quem: &Identidade) -> Result<Registro, String> {
        let id = validar_id(&p.integracao)?.to_string();
        let integrador = validar_nome(&p.integrador)?;
        let parecer = p.parecer.ok_or("falta o parecer (OK ou NOGO)")?;
        let erros: Vec<String> = p
            .erros
            .iter()
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty())
            .collect();
        match parecer {
            Veredito::NoGo if erros.is_empty() => {
                return Err("NOGO sem erros: diga o que a frente tem de consertar".into());
            }
            Veredito::Ok if !erros.is_empty() => {
                return Err("OK com erros: se ha erro a consertar, o parecer e NOGO".into());
            }
            _ => {}
        }
        std::fs::create_dir_all(&self.pasta)
            .map_err(|e| format!("{}: {e}", self.pasta.display()))?;
        let _t = travar(&self.pasta.join(format!("{id}.trava")))?;
        let mut i = self.ler(&id)?.ok_or_else(|| {
            format!(
                "integracao {id} nao aberta: quem a abre declara o conselho \
                 (`phxclaw gonogo abrir {id} --integradores a,b`)"
            )
        })?;
        if !i.conselho.contains(&integrador) {
            return Err(format!(
                "{integrador} nao e do conselho de {id} ({})",
                i.conselho.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        let credencial = self.conferir_identidade(&integrador, quem)?;
        let revisao = i
            .pareceres
            .iter()
            .filter(|x| x.integrador == integrador)
            .map(|x| x.revisao)
            .max()
            .unwrap_or(0)
            + 1;
        let novo = Parecer {
            integrador,
            parecer,
            erros,
            carimbo: agora(),
            revisao,
            tarefa: match quem {
                Identidade::Tarefa(t) => Some(t.clone()),
                Identidade::Credencial(_) => None,
            },
        };
        i.pareceres.push(novo.clone());
        self.gravar(&i)?;
        Ok(Registro {
            parecer: novo,
            integracao: i,
            credencial,
        })
    }

    /// Confere quem registra contra a prova gravada do integrador, ou cria a prova no
    /// primeiro registro dele. Devolve o token novo, quando criou um.
    fn conferir_identidade(&self, nome: &str, quem: &Identidade) -> Result<Option<String>, String> {
        let _t = travar(&self.pasta.join("integradores.trava"))?;
        let arq = self.pasta.join(IDENTIDADES);
        let mut ids: Identidades = match std::fs::read_to_string(&arq) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", arq.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Identidades {
                formato: 1,
                ..Identidades::default()
            },
            Err(e) => return Err(format!("{}: {e}", arq.display())),
        };
        let recusa = |como: &str| {
            format!("{nome} ja registrou parecer {como}; so quem prova ser {nome} registra outro")
        };
        match (ids.integradores.get(nome), quem) {
            (Some(Prova::Tarefa { tarefa }), Identidade::Tarefa(t)) if tarefa == t => Ok(None),
            (Some(Prova::Tarefa { tarefa }), _) => Err(recusa(&format!("pela tarefa {tarefa}"))),
            (Some(Prova::Credencial), Identidade::Credencial(Some(tok))) => {
                let broker = crate::canais::broker_em(&self.pasta)?;
                let d = crate::canais::segredo_guardado(&broker, &nome_do_segredo(nome), ESPACO)?;
                match d {
                    Some(d) if d.sha256 == sha256_hex(tok.trim()) => Ok(None),
                    _ => Err(format!("credencial de {nome} nao confere")),
                }
            }
            (Some(Prova::Credencial), _) => Err(recusa("com credencial (--credencial)")),
            (None, Identidade::Credencial(Some(_))) => Err(format!(
                "{nome} ainda nao tem credencial: o primeiro registro, sem --credencial, a cria"
            )),
            (None, Identidade::Tarefa(t)) => {
                ids.integradores
                    .insert(nome.to_string(), Prova::Tarefa { tarefa: t.clone() });
                self.gravar_identidades(&arq, &ids)?;
                Ok(None)
            }
            (None, Identidade::Credencial(None)) => {
                let mut b = [0u8; 32];
                getrandom::fill(&mut b).map_err(|e| format!("sem fonte aleatoria: {e}"))?;
                let token: String = b.iter().map(|x| format!("{x:02x}")).collect();
                let broker = crate::canais::broker_em(&self.pasta)?;
                crate::canais::guardar_segredo(
                    &broker,
                    &nome_do_segredo(nome),
                    ESPACO,
                    &["gonogo:registrar"],
                    phxclaw_secret_broker::SecretValue::new(token.clone()),
                )?;
                ids.integradores.insert(nome.to_string(), Prova::Credencial);
                self.gravar_identidades(&arq, &ids)?;
                Ok(Some(token))
            }
        }
    }

    fn gravar_identidades(&self, arq: &Path, ids: &Identidades) -> Result<(), String> {
        gravar_atomico(
            arq,
            &serde_json::to_string_pretty(ids).map_err(|e| e.to_string())?,
        )
    }

    fn gravar(&self, i: &Integracao) -> Result<(), String> {
        gravar_atomico(
            &self.arquivo(&i.integracao),
            &serde_json::to_string_pretty(i).map_err(|e| e.to_string())?,
        )
    }
}

/// O nome do envelope: o nome do integrador pode ter qualquer caractere, o hash nao.
fn nome_do_segredo(nome: &str) -> String {
    format!("integrador-{}", &sha256_hex(nome)[..32])
}

/// O JSON que a ferramenta e a CLI mostram.
pub fn vista(i: &Integracao) -> Value {
    let d = i.decidir();
    json!({
        "integracao": i.integracao,
        "decisao": d,
        "conselho": i.conselho,
        "aberta_por": i.aberta_por,
        "vigentes": i.vigentes().values().collect::<Vec<_>>(),
        "historico": i.pareceres.len(),
    })
}

pub fn texto_da_vista(i: &Integracao) -> String {
    let d = i.decidir();
    let mut s = format!("{} -- {}\n", i.integracao, d.nome());
    let vig = i.vigentes();
    for n in &i.conselho {
        match vig.get(n.as_str()) {
            Some(p) => {
                s.push_str(&format!(
                    "  {n}: {} (revisao {}, {})\n",
                    if p.parecer == Veredito::Ok {
                        "OK"
                    } else {
                        "NOGO"
                    },
                    p.revisao,
                    p.carimbo
                ));
                for e in &p.erros {
                    s.push_str(&format!("      - {e}\n"));
                }
            }
            None => s.push_str(&format!("  {n}: sem parecer\n")),
        }
    }
    s
}

// ---------------------------------------------------------------- CLI

pub const USO: &str = "uso: phxclaw gonogo abrir INTEGRACAO --integradores a,b | registrar \
INTEGRACAO INTEGRADOR OK|NOGO [--erro \"...\"]... [--credencial TOKEN|-] | ver INTEGRACAO | \
decidir INTEGRACAO [--pasta DIR]";

/// `phxclaw gonogo ...`: devolve o texto e o codigo de saida (0 GO, 2 NOGO, 3 AGUARDAR).
/// `--credencial -` le o token do stdin, para ele nao aparecer na lista de processos.
pub fn cli(
    args: &[String],
    pasta_do_agente: &Path,
    stdin: &mut dyn std::io::BufRead,
) -> Result<(String, i32), String> {
    let mut pos: Vec<&str> = vec![];
    let mut erros = vec![];
    let mut integradores = vec![];
    let mut credencial = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--erro" => erros.push(it.next().ok_or(USO)?.clone()),
            "--integradores" => integradores.extend(
                it.next()
                    .ok_or(USO)?
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from),
            ),
            "--credencial" => {
                let c = it.next().ok_or(USO)?;
                credencial = Some(if c == "-" {
                    let mut l = String::new();
                    stdin.read_line(&mut l).map_err(|e| e.to_string())?;
                    l.trim().to_string()
                } else {
                    c.clone()
                });
            }
            "--pasta" => {
                it.next();
            }
            _ => pos.push(a),
        }
    }
    let c = Conselho::da_pasta_do_agente(pasta_do_agente);
    match pos.as_slice() {
        ["abrir" | "open", id] => {
            let i = c.abrir(id, &integradores, &Identidade::Credencial(None))?;
            Ok((texto_da_vista(&i), i.decidir().codigo()))
        }
        ["registrar" | "record", id, integrador, parecer] => {
            let r = c.registrar(
                &Pedido {
                    integracao: id.to_string(),
                    integrador: integrador.to_string(),
                    parecer: Some(Veredito::do_texto(parecer)?),
                    erros,
                },
                &Identidade::Credencial(credencial),
            )?;
            let mut t = texto_da_vista(&r.integracao);
            if let Some(tok) = r.credencial {
                t.push_str(&format!(
                    "credencial de {integrador} (guarde: nao sera mostrada de novo, e o proximo \
                     parecer de {integrador} a exige com --credencial): {tok}\n"
                ));
            }
            Ok((t, r.integracao.decidir().codigo()))
        }
        ["ver" | "show", id] => match c.ler(id)? {
            Some(i) => Ok((texto_da_vista(&i), i.decidir().codigo())),
            None => Ok((format!("{id} -- AGUARDAR (integracao nao aberta)\n"), 3)),
        },
        ["decidir" | "decide", id] => {
            let d = c.decidir(id)?;
            Ok((format!("{}\n", d.nome()), d.codigo()))
        }
        _ => Err(USO.into()),
    }
}

// ---------------------------------------------------------------- ferramenta

/// `go_no_go` (gonogo.write): abrir, registrar o parecer e consultar a decisao. A
/// identidade e a tarefa que chama: o modelo nao a escolhe.
pub struct GoNoGoTool {
    pub conselho: Conselho,
}

fn lista(args: &Value, nome: &str) -> Vec<String> {
    match &args[nome] {
        Value::Array(a) => a
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        Value::String(s) if !s.trim().is_empty() => vec![s.clone()],
        _ => vec![],
    }
}

impl Tool for GoNoGoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "go_no_go".into(),
            description: "Integrators' council Go/NoGo. action: open {integration, integrators} \
declares the council (fixed afterwards); record {integration, integrator, verdict: OK|NOGO, \
errors? (required for NOGO)} -- the integrator must be in the council, and once a name is \
recorded only the same task can record for it again; status {integration}. Decision: any \
current NOGO -> NOGO; a council member without verdict -> WAIT; all OK -> GO."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["open","record","status"]},
                "integration":{"type":"string"},
                "integrator":{"type":"string"},
                "verdict":{"type":"string","enum":["OK","NOGO"]},
                "errors":{"type":"array","items":{"type":"string"}},
                "integrators":{"type":"array","items":{"type":"string"}}
            },"required":["action","integration"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "gonogo.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let txt = |n: &str| args[n].as_str().unwrap_or("").to_string();
            let id = txt("integration");
            let c = self.conselho.clone();
            let quem = Identidade::Tarefa(ctx.task_id.clone());
            // Trava e fsync sao bloqueantes: fora do executor.
            let fora = |f: Box<dyn FnOnce() -> Result<Value, String> + Send>| async move {
                tokio::task::spawn_blocking(f)
                    .await
                    .map_err(|e| ToolError::Failed(e.to_string()))?
                    .map_err(ToolError::InvalidArguments)
            };
            let v = match txt("action").as_str() {
                "open" | "abrir" => {
                    let conselho = lista(&args, "integrators");
                    fora(Box::new(move || {
                        Ok(vista(&c.abrir(&id, &conselho, &quem)?))
                    }))
                    .await?
                }
                "record" | "registrar" => {
                    let pedido = Pedido {
                        integracao: id,
                        integrador: txt("integrator"),
                        parecer: Some(
                            Veredito::do_texto(&txt("verdict"))
                                .map_err(ToolError::InvalidArguments)?,
                        ),
                        erros: lista(&args, "errors"),
                    };
                    fora(Box::new(move || {
                        let r = c.registrar(&pedido, &quem)?;
                        let mut v = vista(&r.integracao);
                        v["registrado"] = json!(r.parecer);
                        Ok(v)
                    }))
                    .await?
                }
                "status" | "consultar" => match c.ler(&id).map_err(ToolError::InvalidArguments)? {
                    Some(i) => vista(&i),
                    None => {
                        json!({"integracao": id, "decisao": Decisao::Aguardar { faltam: vec![] },
                                   "conselho": [], "vigentes": [], "historico": 0,
                                   "aviso": "integracao nao aberta"})
                    }
                },
                outra => {
                    return Err(ToolError::InvalidArguments(format!(
                        "action desconhecida em go_no_go: {outra} (open, record, status)"
                    )));
                }
            };
            Ok(ToolOutput::text(v.to_string()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "phxclaw-gonogo-{nome}-{}",
            phxclaw_types::new_uuid_v7()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn t(nome: &str) -> Identidade {
        Identidade::Tarefa(format!("tarefa-{nome}"))
    }

    fn reg(c: &Conselho, quem: &str, v: Veredito, erros: &[&str]) -> Result<Registro, String> {
        c.registrar(
            &Pedido {
                integracao: "onda-6".into(),
                integrador: quem.into(),
                parecer: Some(v),
                erros: erros.iter().map(|s| s.to_string()).collect(),
            },
            &t(quem),
        )
    }

    fn abrir(c: &Conselho, conselho: &[&str]) {
        c.abrir(
            "onda-6",
            &conselho.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &t("orquestrador"),
        )
        .unwrap();
    }

    use Veredito::{NoGo, Ok as OK};

    /// A tabela da ordem do dono, caso a caso, cada um numa pasta propria.
    #[test]
    fn tabela_do_conselho() {
        type Passo<'a> = (&'a str, Veredito, &'a [&'a str]);
        let casos: &[(&str, &[&str], &[Passo], &str)] = &[
            ("um OK", &["A"], &[("A", OK, &[])], "GO"),
            (
                "dois, um ausente",
                &["A", "B"],
                &[("A", OK, &[])],
                "AGUARDAR",
            ),
            (
                "um NOGO e um OK",
                &["A", "B"],
                &[("A", NoGo, &["fmt"]), ("B", OK, &[])],
                "NOGO",
            ),
            (
                "NOGO revisado pelo mesmo",
                &["A"],
                &[("A", NoGo, &["clippy"]), ("A", OK, &[])],
                "GO",
            ),
            (
                "NOGO de B ainda aguarda C: NOGO manda",
                &["A", "B", "C"],
                &[("A", OK, &[]), ("B", NoGo, &["x"])],
                "NOGO",
            ),
        ];
        for (nome, conselho, passos, esperado) in casos {
            let c = Conselho::da_pasta_do_agente(&pasta("tabela"));
            abrir(&c, conselho);
            for (quem, v, e) in passos.iter() {
                reg(&c, quem, *v, e).unwrap();
            }
            assert_eq!(c.decidir("onda-6").unwrap().nome(), *esperado, "{nome}");
            let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
        }
    }

    /// Os ataques que a revisao de seguranca reproduziu, cada um recusado.
    #[test]
    fn ninguem_registra_por_outro_nem_fora_do_conselho() {
        let c = Conselho::da_pasta_do_agente(&pasta("adverso"));
        // Integracao nao aberta: nem registra, nem da GO.
        assert!(
            reg(&c, "Qualquer", OK, &[])
                .unwrap_err()
                .contains("nao aberta")
        );
        assert_eq!(c.decidir("onda-6").unwrap().nome(), "AGUARDAR");
        assert!(
            c.abrir("onda-6", &[], &t("o"))
                .unwrap_err()
                .contains("conselho vazio")
        );
        abrir(&c, &["Ana", "Bia"]);
        assert!(
            c.abrir("onda-6", &["Eu".into()], &t("o")).is_err(),
            "reabrir"
        );
        assert!(
            reg(&c, "Qualquer", OK, &[])
                .unwrap_err()
                .contains("nao e do conselho")
        );
        reg(&c, "Ana", NoGo, &["teste vermelho"]).unwrap();
        reg(&c, "Bia", OK, &[]).unwrap();
        // «NOGO de A revisado por B»: Bia nao fala por Ana.
        reg(&c, "Bia", OK, &[]).unwrap();
        // Outra tarefa dizendo ser Ana.
        let impostor = c.registrar(
            &Pedido {
                integracao: "onda-6".into(),
                integrador: "Ana".into(),
                parecer: Some(OK),
                erros: vec![],
            },
            &t("Bia"),
        );
        assert!(
            impostor.unwrap_err().contains("pela tarefa"),
            "outra tarefa"
        );
        // A CLI sem credencial dizendo ser Ana.
        let cli_sem = c.registrar(
            &Pedido {
                integracao: "onda-6".into(),
                integrador: "Ana".into(),
                parecer: Some(OK),
                erros: vec![],
            },
            &Identidade::Credencial(None),
        );
        assert!(cli_sem.is_err(), "sem credencial");
        assert_eq!(c.decidir("onda-6").unwrap().nome(), "NOGO");
        // So a propria Ana tira o NOGO dela.
        reg(&c, "Ana", OK, &[]).unwrap();
        assert_eq!(c.decidir("onda-6").unwrap().nome(), "GO");
        let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
    }

    /// Arquivo editado a mao com o conselho vazio: quem registrou nao vira unanimidade.
    #[test]
    fn conselho_vazio_no_arquivo_nunca_da_go() {
        let i = Integracao {
            formato: FORMATO,
            integracao: "w".into(),
            conselho: BTreeSet::new(),
            aberta_por: "cli".into(),
            aberta_em: agora(),
            pareceres: vec![Parecer {
                integrador: "Qualquer".into(),
                parecer: OK,
                erros: vec![],
                carimbo: agora(),
                revisao: 1,
                tarefa: None,
            }],
        };
        assert_eq!(i.decidir().nome(), "AGUARDAR");
    }

    #[test]
    fn revisao_conta_por_integrador_e_historico_fica() {
        let c = Conselho::da_pasta_do_agente(&pasta("revisao"));
        abrir(&c, &["A", "B"]);
        reg(&c, "A", NoGo, &["fmt"]).unwrap();
        reg(&c, "B", OK, &[]).unwrap();
        reg(&c, "A", OK, &[]).unwrap();
        let i = c.ler("onda-6").unwrap().unwrap();
        assert_eq!(i.pareceres.len(), 3);
        assert_eq!(i.vigentes()["A"].revisao, 2);
        assert_eq!(i.vigentes()["B"].revisao, 1);
        assert_eq!(i.aberta_por, "tarefa-orquestrador");
        assert_eq!(i.decidir(), Decisao::Go);
        let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
    }

    #[test]
    fn recusas_de_pedido_e_de_formato() {
        let c = Conselho::da_pasta_do_agente(&pasta("recusa"));
        abrir(&c, &["A"]);
        let base = Pedido {
            integracao: "onda-6".into(),
            integrador: "A".into(),
            parecer: Some(NoGo),
            ..Pedido::default()
        };
        assert!(
            c.registrar(&base, &t("A"))
                .unwrap_err()
                .contains("NOGO sem erros")
        );
        let ok_com_erro = Pedido {
            parecer: Some(OK),
            erros: vec!["e".into()],
            ..base.clone()
        };
        assert!(c.registrar(&ok_com_erro, &t("A")).is_err());
        assert!(
            c.abrir("../fora", &["A".into()], &t("o"))
                .unwrap_err()
                .contains("invalido")
        );
        std::fs::write(
            c.pasta.join("velho.json"),
            r#"{"formato":1,"integracao":"velho"}"#,
        )
        .unwrap();
        assert!(c.ler("velho").unwrap_err().contains("formato 1"));
        let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
    }

    #[test]
    fn registros_simultaneos_nao_perdem_parecer() {
        let c = Conselho::da_pasta_do_agente(&pasta("concorrencia"));
        let n = 16;
        let nomes: Vec<String> = (0..n).map(|k| format!("I{k}")).collect();
        c.abrir("onda-6", &nomes, &t("o")).unwrap();
        let fios: Vec<_> = (0..n)
            .map(|k| {
                let c = c.clone();
                std::thread::spawn(move || {
                    reg(&c, &format!("I{k}"), OK, &[]).unwrap();
                })
            })
            .collect();
        for f in fios {
            f.join().unwrap();
        }
        let i = c.ler("onda-6").unwrap().unwrap();
        assert_eq!(i.pareceres.len(), n, "parecer perdido");
        assert_eq!(i.decidir(), Decisao::Go);
        let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
    }

    #[tokio::test]
    async fn ferramenta_identifica_pela_tarefa() {
        let d = pasta("ferramenta");
        let t = GoNoGoTool {
            conselho: Conselho::da_pasta_do_agente(&d),
        };
        assert_eq!(t.capability(), "gonogo.write");
        let ctx = |id: &str| ToolContext {
            task_id: id.into(),
            workdir: d.clone(),
            timeout: std::time::Duration::from_secs(5),
        };
        let r = |a: Value, c: ToolContext| {
            let t = &t;
            async move { t.run(a, &c).await }
        };
        let json_de = |o: ToolOutput| serde_json::from_str::<Value>(&o.content).unwrap();
        r(
            json!({"action":"open","integration":"w6","integrators":["A","B"]}),
            ctx("orq"),
        )
        .await
        .unwrap();
        let v = json_de(
            r(
                json!({"action":"record","integration":"w6","integrator":"A",
                       "verdict":"NOGO","errors":["fmt"]}),
                ctx("tarefa-a"),
            )
            .await
            .unwrap(),
        );
        assert_eq!(v["decisao"]["decisao"], "NOGO", "{v}");
        assert_eq!(v["registrado"]["tarefa"], "tarefa-a");
        let impostor = r(
            json!({"action":"record","integration":"w6","integrator":"A","verdict":"OK"}),
            ctx("tarefa-b"),
        )
        .await;
        assert!(impostor.is_err(), "outra tarefa trocando o NOGO de A");
        let v = json_de(
            r(json!({"action":"status","integration":"w6"}), ctx("x"))
                .await
                .unwrap(),
        );
        assert_eq!(v["decisao"]["decisao"], "NOGO");
        // A CLI le o MESMO arquivo que a ferramenta gravou.
        assert_eq!(
            cli(&["decidir".into(), "w6".into()], &d, &mut std::io::empty())
                .unwrap()
                .1,
            2
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn o_papel_111_ganha_a_capacidade() {
        let e = crate::equipe::Equipe::do_ambiente().unwrap();
        let m = e.achar("111").unwrap();
        assert!(crate::equipe::capacidades_do_papel(m).contains("gonogo.write"));
        assert_eq!(e.capability_principal(m), "release.go_no_go.decide");
    }

    #[test]
    fn cli_credencial_aparece_uma_vez_e_e_exigida_depois() {
        let d = pasta("cli");
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut nada = std::io::empty();
        cli(
            &a(&["abrir", "w6", "--integradores", "Ana,Bia"]),
            &d,
            &mut nada,
        )
        .unwrap();
        let (t, cod) = cli(
            &a(&[
                "registrar",
                "w6",
                "Ana",
                "nogo",
                "--erro",
                "clippy 2 avisos",
            ]),
            &d,
            &mut nada,
        )
        .unwrap();
        assert_eq!(cod, 2, "{t}");
        assert!(
            t.contains("clippy 2 avisos") && t.contains("Bia: sem parecer"),
            "{t}"
        );
        let token = t
            .lines()
            .find(|l| l.starts_with("credencial de Ana"))
            .and_then(|l| l.rsplit(' ').next())
            .unwrap()
            .to_string();
        assert_eq!(token.len(), 64);
        // Sem credencial e com a errada: recusado, o NOGO fica.
        assert!(cli(&a(&["registrar", "w6", "Ana", "OK"]), &d, &mut nada).is_err());
        let errada = "0".repeat(64);
        assert!(
            cli(
                &a(&["registrar", "w6", "Ana", "OK", "--credencial", &errada]),
                &d,
                &mut nada
            )
            .unwrap_err()
            .contains("nao confere")
        );
        assert_eq!(cli(&a(&["decidir", "w6"]), &d, &mut nada).unwrap().1, 2);
        // Com a credencial pelo stdin: vale, e nao gera outra.
        let mut entrada = std::io::Cursor::new(format!("{token}\n"));
        let (t, _) = cli(
            &a(&["registrar", "w6", "Ana", "OK", "--credencial", "-"]),
            &d,
            &mut entrada,
        )
        .unwrap();
        assert!(!t.contains("credencial de Ana"), "{t}");
        assert_eq!(
            cli(&a(&["decidir", "w6"]), &d, &mut nada).unwrap(),
            ("AGUARDAR\n".into(), 3)
        );
        cli(&a(&["registrar", "w6", "Bia", "OK"]), &d, &mut nada).unwrap();
        assert_eq!(
            cli(&a(&["decidir", "w6"]), &d, &mut nada).unwrap(),
            ("GO\n".into(), 0)
        );
        // O token nao fica em arquivo nenhum da pasta fora do cofre cifrado.
        for e in walk(&d) {
            let b = std::fs::read(&e).unwrap_or_default();
            assert!(
                !String::from_utf8_lossy(&b).contains(&token),
                "token em claro em {e:?}"
            );
        }
        assert!(cli(&a(&["apagar", "w6"]), &d, &mut nada).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    fn walk(d: &Path) -> Vec<PathBuf> {
        let mut v = vec![];
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                v.extend(walk(&p));
            } else {
                v.push(p);
            }
        }
        v
    }
}
