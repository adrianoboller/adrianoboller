//! O conselho de integradores: cada integrador registra o seu parecer sobre uma integracao
//! (OK, ou NOGO com os erros) e a decisao sai dos pareceres vigentes -- ordem do dono,
//! 01/10/2026 (papel 111, `config/agents/111-integrador.agent.json`).
//!
//! | pareceres vigentes                         | decisao  |
//! |--------------------------------------------|----------|
//! | algum NOGO                                 | NOGO     |
//! | falta parecer de algum integrador ativo    | AGUARDAR |
//! | todos registrados e todos OK               | GO       |
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Parecer e de quem o assinou.** O vigente de cada integrador e o da revisao mais alta
//!   DELE; ninguem escreve no parecer de outro, entao um NOGO so deixa de valer quando o
//!   mesmo integrador registra parecer novo, e nao existe contagem de votos.
//! - **A lista de integradores ativos so cresce.** Quem registra entra nela, e o registro
//!   pode declarar outros que ainda vao opinar. Tirar um nome da lista seria o jeito de
//!   fazer um Go sem o parecer dele -- maioria pela porta dos fundos.
//! - **O historico fica.** Cada registro e uma linha nova com revisao e carimbo; nada se
//!   sobrescreve, e o que o integrador disse antes continua legivel.
//! - **Gravacao atomica e serializada entre processos.** Um arquivo por integracao em
//!   `<pasta do agente>/gonogo/`, trocado por renomeacao depois do `fsync`, e o
//!   ler-mudar-gravar acontece sob trava exclusiva (`File::lock`) de um `.trava` ao lado:
//!   dois integradores registrando ao mesmo tempo nao perdem parecer (o teste prova com
//!   threads, que no Linux pegam a trava por descritores diferentes como dois processos).
//! - **Identidade e o nome dado, nao autenticada.** O registro guarda tambem a tarefa que o
//!   fez, para auditoria; quem assina como outro deixa rastro, mas nao e barrado aqui.
//!
//! A ferramenta `go_no_go` e a CLI `phxclaw gonogo` chamam as MESMAS funcoes deste modulo.

use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Versao do formato em disco. Arquivo de versao diferente e recusado: ler um formato que
/// este binario nao conhece poderia esquecer um NOGO.
pub const FORMATO: u32 = 1;
/// Subpasta da pasta do agente.
pub const PASTA: &str = "gonogo";

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tarefa: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Integracao {
    pub formato: u32,
    pub integracao: String,
    pub integradores: BTreeSet<String>,
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
    /// Integradores ativos sem parecer.
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
    fn nova(id: &str) -> Self {
        Self {
            formato: FORMATO,
            integracao: id.to_string(),
            integradores: BTreeSet::new(),
            pareceres: vec![],
        }
    }

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
            .integradores
            .iter()
            .filter(|i| !vig.contains_key(i.as_str()))
            .cloned()
            .collect();
        // Integracao sem nenhum integrador ainda nao foi julgada por ninguem.
        if !faltam.is_empty() || vig.is_empty() {
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
    /// Outros integradores ativos desta integracao (somam a lista, nunca tiram).
    pub integradores: Vec<String>,
    pub tarefa: Option<String>,
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

    /// A integracao gravada (`None`: ninguem registrou ainda).
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

    pub fn decidir(&self, id: &str) -> Result<Decisao, String> {
        Ok(self
            .ler(id)?
            .unwrap_or_else(|| Integracao::nova(id.trim()))
            .decidir())
    }

    /// Registra o parecer e devolve a integracao como ficou.
    pub fn registrar(&self, p: &Pedido) -> Result<(Parecer, Integracao), String> {
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
        let outros = p
            .integradores
            .iter()
            .map(|n| validar_nome(n))
            .collect::<Result<Vec<_>, _>>()?;
        std::fs::create_dir_all(&self.pasta)
            .map_err(|e| format!("{}: {e}", self.pasta.display()))?;
        let trava = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.pasta.join(format!("{id}.trava")))
            .map_err(|e| format!("trava de {id}: {e}"))?;
        trava.lock().map_err(|e| format!("trava de {id}: {e}"))?;
        let mut i = self.ler(&id)?.unwrap_or_else(|| Integracao::nova(&id));
        let revisao = i
            .pareceres
            .iter()
            .filter(|x| x.integrador == integrador)
            .map(|x| x.revisao)
            .max()
            .unwrap_or(0)
            + 1;
        let novo = Parecer {
            integrador: integrador.clone(),
            parecer,
            erros,
            carimbo: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            revisao,
            tarefa: p.tarefa.clone(),
        };
        i.integradores.insert(integrador);
        i.integradores.extend(outros);
        i.pareceres.push(novo.clone());
        self.gravar(&i)?;
        // A trava solta quando `trava` sai de escopo, depois da renomeacao.
        drop(trava);
        Ok((novo, i))
    }

    fn gravar(&self, i: &Integracao) -> Result<(), String> {
        let arq = self.arquivo(&i.integracao);
        let tmp = self
            .pasta
            .join(format!(".{}.json.{}.tmp", i.integracao, std::process::id()));
        let texto = serde_json::to_string_pretty(i).map_err(|e| e.to_string())?;
        let escrever = || -> std::io::Result<()> {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(texto.as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            std::fs::rename(&tmp, &arq)?;
            // A renomeacao so e duravel com a pasta sincronizada.
            std::fs::File::open(&self.pasta)?.sync_all()
        };
        escrever().map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", arq.display())
        })
    }
}

/// O JSON que a ferramenta e a CLI mostram.
pub fn vista(i: &Integracao) -> Value {
    let d = i.decidir();
    json!({
        "integracao": i.integracao,
        "decisao": d,
        "integradores": i.integradores,
        "vigentes": i.vigentes().values().collect::<Vec<_>>(),
        "historico": i.pareceres.len(),
    })
}

pub fn texto_da_vista(i: &Integracao) -> String {
    let d = i.decidir();
    let mut s = format!("{} -- {}\n", i.integracao, d.nome());
    let vig = i.vigentes();
    for n in &i.integradores {
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

pub const USO: &str = "uso: phxclaw gonogo registrar INTEGRACAO INTEGRADOR OK|NOGO [--erro \"...\"]... \
[--integradores a,b] [--pasta DIR] | ver INTEGRACAO | decidir INTEGRACAO";

/// `phxclaw gonogo ...`: devolve o texto e o codigo de saida (0 GO, 2 NOGO, 3 AGUARDAR).
pub fn cli(args: &[String], pasta_do_agente: &Path) -> Result<(String, i32), String> {
    let mut pos: Vec<&str> = vec![];
    let mut erros = vec![];
    let mut integradores = vec![];
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
            "--pasta" => {
                it.next();
            }
            _ => pos.push(a),
        }
    }
    let c = Conselho::da_pasta_do_agente(pasta_do_agente);
    match pos.as_slice() {
        ["registrar" | "record", id, integrador, parecer] => {
            let (_, i) = c.registrar(&Pedido {
                integracao: id.to_string(),
                integrador: integrador.to_string(),
                parecer: Some(Veredito::do_texto(parecer)?),
                erros,
                integradores,
                tarefa: None,
            })?;
            Ok((texto_da_vista(&i), i.decidir().codigo()))
        }
        ["ver" | "show", id] => match c.ler(id)? {
            Some(i) => Ok((texto_da_vista(&i), i.decidir().codigo())),
            None => Ok((format!("{id} -- AGUARDAR (nenhum parecer registrado)\n"), 3)),
        },
        ["decidir" | "decide", id] => {
            let d = c.decidir(id)?;
            Ok((format!("{}\n", d.nome()), d.codigo()))
        }
        _ => Err(USO.into()),
    }
}

// ---------------------------------------------------------------- ferramenta

/// `go_no_go` (gonogo.write): registrar o parecer e consultar a decisao.
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
            description: "Integrators' council Go/NoGo. action: record {integration, \
integrator, verdict: OK|NOGO, errors? (required for NOGO), integrators? (other active \
integrators)}; status {integration}. Decision: any current NOGO -> NOGO; an active integrator \
without verdict -> WAIT; all OK -> GO. Only the same integrator can replace own NOGO."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["record","status"]},
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
            let v = match txt("action").as_str() {
                "record" | "registrar" => {
                    let pedido = Pedido {
                        integracao: id,
                        integrador: txt("integrator"),
                        parecer: Some(
                            Veredito::do_texto(&txt("verdict"))
                                .map_err(ToolError::InvalidArguments)?,
                        ),
                        erros: lista(&args, "errors"),
                        integradores: lista(&args, "integrators"),
                        tarefa: Some(ctx.task_id.clone()),
                    };
                    // Trava e fsync sao bloqueantes: fora do executor.
                    let (p, i) = tokio::task::spawn_blocking(move || c.registrar(&pedido))
                        .await
                        .map_err(|e| ToolError::Failed(e.to_string()))?
                        .map_err(ToolError::InvalidArguments)?;
                    let mut v = vista(&i);
                    v["registrado"] = json!(p);
                    v
                }
                "status" | "consultar" => match c.ler(&id).map_err(ToolError::InvalidArguments)? {
                    Some(i) => vista(&i),
                    None => {
                        json!({"integracao": id, "decisao": Decisao::Aguardar { faltam: vec![] },
                                   "integradores": [], "vigentes": [], "historico": 0})
                    }
                },
                outra => {
                    return Err(ToolError::InvalidArguments(format!(
                        "action desconhecida em go_no_go: {outra} (record, status)"
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

    fn reg(c: &Conselho, quem: &str, v: Veredito, erros: &[&str], outros: &[&str]) {
        c.registrar(&Pedido {
            integracao: "onda-6".into(),
            integrador: quem.into(),
            parecer: Some(v),
            erros: erros.iter().map(|s| s.to_string()).collect(),
            integradores: outros.iter().map(|s| s.to_string()).collect(),
            tarefa: None,
        })
        .unwrap();
    }

    use Veredito::{NoGo, Ok as OK};

    /// A tabela da ordem do dono, caso a caso, cada um numa pasta propria.
    #[test]
    fn tabela_do_conselho() {
        type Passo<'a> = (&'a str, Veredito, &'a [&'a str], &'a [&'a str]);
        let casos: &[(&str, &[Passo], &str)] = &[
            ("um OK", &[("A", OK, &[], &[])], "GO"),
            ("dois, um ausente", &[("A", OK, &[], &["B"])], "AGUARDAR"),
            (
                "um NOGO e um OK",
                &[("A", NoGo, &["fmt"], &["B"]), ("B", OK, &[], &[])],
                "NOGO",
            ),
            (
                "NOGO revisado pelo mesmo",
                &[("A", NoGo, &["clippy"], &[]), ("A", OK, &[], &[])],
                "GO",
            ),
            (
                "NOGO de A revisado por B",
                &[
                    ("A", NoGo, &["teste"], &["B"]),
                    ("B", OK, &[], &[]),
                    ("B", OK, &[], &["A"]),
                ],
                "NOGO",
            ),
            (
                "NOGO de B ainda aguarda C: NOGO manda",
                &[("A", OK, &[], &["B", "C"]), ("B", NoGo, &["x"], &[])],
                "NOGO",
            ),
        ];
        for (nome, passos, esperado) in casos {
            let c = Conselho::da_pasta_do_agente(&pasta("tabela"));
            for (quem, v, e, o) in passos.iter() {
                reg(&c, quem, *v, e, o);
            }
            assert_eq!(c.decidir("onda-6").unwrap().nome(), *esperado, "{nome}");
            let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
        }
        let vazio = Conselho::da_pasta_do_agente(&pasta("vazio"));
        assert_eq!(vazio.decidir("onda-6").unwrap().nome(), "AGUARDAR");
        let _ = std::fs::remove_dir_all(vazio.pasta.parent().unwrap());
    }

    #[test]
    fn revisao_conta_por_integrador_e_historico_fica() {
        let c = Conselho::da_pasta_do_agente(&pasta("revisao"));
        reg(&c, "A", NoGo, &["fmt"], &["B"]);
        reg(&c, "B", OK, &[], &[]);
        reg(&c, "A", OK, &[], &[]);
        let i = c.ler("onda-6").unwrap().unwrap();
        assert_eq!(i.pareceres.len(), 3);
        assert_eq!(i.vigentes()["A"].revisao, 2);
        assert_eq!(i.vigentes()["B"].revisao, 1);
        assert_eq!(i.decidir(), Decisao::Go);
        let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
    }

    #[test]
    fn recusas_de_pedido_e_de_formato() {
        let c = Conselho::da_pasta_do_agente(&pasta("recusa"));
        let base = Pedido {
            integracao: "x".into(),
            integrador: "A".into(),
            parecer: Some(NoGo),
            ..Pedido::default()
        };
        assert!(c.registrar(&base).unwrap_err().contains("NOGO sem erros"));
        let ok_com_erro = Pedido {
            parecer: Some(OK),
            erros: vec!["e".into()],
            ..base.clone()
        };
        assert!(c.registrar(&ok_com_erro).is_err());
        let fuga = Pedido {
            integracao: "../fora".into(),
            erros: vec!["e".into()],
            ..base.clone()
        };
        assert!(c.registrar(&fuga).unwrap_err().contains("invalido"));
        std::fs::create_dir_all(&c.pasta).unwrap();
        std::fs::write(
            c.pasta.join("futuro.json"),
            r#"{"formato":2,"integracao":"futuro"}"#,
        )
        .unwrap();
        assert!(c.ler("futuro").unwrap_err().contains("formato 2"));
        let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
    }

    #[test]
    fn registros_simultaneos_nao_perdem_parecer() {
        let c = Conselho::da_pasta_do_agente(&pasta("concorrencia"));
        let n = 16;
        let fios: Vec<_> = (0..n)
            .map(|k| {
                let c = c.clone();
                std::thread::spawn(move || {
                    c.registrar(&Pedido {
                        integracao: "onda-6".into(),
                        integrador: format!("I{k}"),
                        parecer: Some(OK),
                        ..Pedido::default()
                    })
                    .unwrap();
                })
            })
            .collect();
        for f in fios {
            f.join().unwrap();
        }
        let i = c.ler("onda-6").unwrap().unwrap();
        assert_eq!(i.pareceres.len(), n, "parecer perdido");
        assert_eq!(i.integradores.len(), n);
        assert_eq!(i.decidir(), Decisao::Go);
        let _ = std::fs::remove_dir_all(c.pasta.parent().unwrap());
    }

    #[tokio::test]
    async fn ferramenta_registra_e_consulta_pelo_mesmo_motor() {
        let d = pasta("ferramenta");
        let t = GoNoGoTool {
            conselho: Conselho::da_pasta_do_agente(&d),
        };
        assert_eq!(t.capability(), "gonogo.write");
        let ctx = ToolContext {
            task_id: "tarefa-1".into(),
            workdir: d.clone(),
            timeout: std::time::Duration::from_secs(5),
        };
        let r = |a: Value| {
            let t = &t;
            let ctx = &ctx;
            async move { t.run(a, ctx).await }
        };
        let v: Value = serde_json::from_str(
            &r(
                json!({"action":"record","integration":"w6","integrator":"A",
                       "verdict":"NOGO","errors":["fmt"],"integrators":["B"]}),
            )
            .await
            .unwrap()
            .content,
        )
        .unwrap();
        assert_eq!(v["decisao"]["decisao"], "NOGO", "{v}");
        assert_eq!(v["registrado"]["tarefa"], "tarefa-1");
        assert!(
            r(json!({"action":"record","integration":"w6","integrator":"A","verdict":"NOGO"}))
                .await
                .is_err(),
            "NOGO sem erros"
        );
        let v: Value = serde_json::from_str(
            &r(json!({"action":"status","integration":"w6"}))
                .await
                .unwrap()
                .content,
        )
        .unwrap();
        assert_eq!(v["decisao"]["decisao"], "NOGO");
        // A CLI le o MESMO arquivo que a ferramenta gravou.
        assert_eq!(cli(&["decidir".into(), "w6".into()], &d).unwrap().1, 2);
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
    fn cli_registra_ve_e_decide_com_codigo() {
        let d = pasta("cli");
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (t, cod) = cli(
            &a(&[
                "registrar",
                "w6",
                "Ana",
                "nogo",
                "--erro",
                "clippy 2 avisos",
                "--integradores",
                "Bia",
            ]),
            &d,
        )
        .unwrap();
        assert_eq!(cod, 2, "{t}");
        assert!(
            t.contains("clippy 2 avisos") && t.contains("Bia: sem parecer"),
            "{t}"
        );
        assert_eq!(cli(&a(&["decidir", "w6"]), &d).unwrap().1, 2);
        cli(&a(&["registrar", "w6", "Ana", "OK"]), &d).unwrap();
        assert_eq!(
            cli(&a(&["decidir", "w6"]), &d).unwrap(),
            ("AGUARDAR\n".into(), 3)
        );
        cli(&a(&["registrar", "w6", "Bia", "OK"]), &d).unwrap();
        assert_eq!(cli(&a(&["decidir", "w6"]), &d).unwrap(), ("GO\n".into(), 0));
        assert!(cli(&a(&["apagar", "w6"]), &d).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
