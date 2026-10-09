//! O gatilho de MCP Events (R6 do radar): um servidor MCP do OPERADOR empurra a notificacao
//! de recurso -- `notifications/resources/updated` de um recurso assinado com
//! `resources/subscribe`, ou `notifications/resources/list_changed` -- e ela dispara a versao
//! PUBLICADA de um fluxo. Armado do mesmo `gatilhos.json` (a lista `mcp`), ao lado dos polls;
//! num modulo proprio porque e o unico gatilho que segura um processo vivo.
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **So servidor do operador, e so com a capacidade.** O `servidor` e um NOME do arquivo de
//!   `PHXCLAW_MCP_CONFIG`; o `gatilhos.json` mora no projeto (pode ter vindo num clone) e nao
//!   declara comando nem URL. E o agente que a fabrica do servidor monta tem de ter
//!   `mcp.<servidor>` concedida -- o MESMO conjunto que o portao confere a cada chamada --,
//!   senao nada sobe: um projeto clonado nao liga processo do operador sem ele ter concedido.
//!   A recusa fica na evidencia do gatilho. Servidor de pacote nao arma gatilho (o nome dele
//!   nao esta no arquivo do operador).
//! - **A notificacao e DADO de fora.** O item vai como `{{entrada}}` do fluxo pelo
//!   `fluxo_versoes::criar_fluxo_publicado`, que passa pelo `fluxos::conferir_entrada` (o
//!   motor unico de credencial) e pelo balde de fichas ANTES de criar a tarefa: notificacao
//!   com forma de segredo e recusada e nao vira tarefa. Chamar a guarda aqui de novo seria a
//!   segunda copia dela.
//! - **Dedupe por janela, com a ULTIMA versao.** A mesma (evento, uri) dentro de `janela_ms`
//!   vira UM disparo, no fim da janela, com os parametros da ultima notificacao e quantas
//!   vieram (`vezes`). O primeiro disparo espera a janela: e o preco de nao disparar tres
//!   vezes pelo salvamento que o editor fez em tres escritas.
//! - **Taxa maxima por gatilho** (`max_por_minuto`): o que passa dela fica pendente
//!   (coalescido) e sai quando abrir vaga -- nunca zero vezes, nunca acima da taxa. O
//!   pendente e limitado pelo numero de recursos assinados (mais um da lista), entao a memoria
//!   nao cresce com a inundacao. O balde de fichas da API continua valendo por cima; recusa
//!   dele (429) devolve o pendente para depois do `retry_after`.
//! - **Inundacao derruba a assinatura, nao o agente.** Mais de `MENSAGENS_MAX_POR_MINUTO`
//!   mensagens do servidor numa janela de um minuto: a sessao cai (o processo morre com ela),
//!   o que estava pendente sai dentro da taxa, e o laco so volta depois de um recuo que dobra
//!   a cada queda (`RECUO_MIN` a `RECUO_MAX`).
//! - **So stdio nesta versao** (ver `mcp::ServidorMcp::assinar`): servidor por `url` e
//!   recusado ao subir, dizendo o motivo.
//! - **Notificacao perdida com o fio caido nao volta.** O MCP nao reenvia, e o pendente acima
//!   da taxa quando a sessao cai se perde junto com ela (contado em `Relato::descartadas` e no
//!   log). Quem precisa de «nada se perde» usa o poll, que tem estado em disco.

use crate::api::ApiState;
use crate::gatilho_poll::PASTA;
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome};
use phxclaw_mcp_lsp_runtime::RuntimeAudit;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const JANELA_MIN_MS: u64 = 100;
pub const JANELA_MAX_MS: u64 = 3_600_000;
pub const JANELA_PADRAO_MS: u64 = 2_000;
pub const TAXA_MAX_POR_MINUTO: u32 = 60;
pub const TAXA_PADRAO_POR_MINUTO: u32 = 6;
pub const RECURSOS_MAX: usize = 64;
/// Mensagens do servidor por minuto acima das quais a assinatura cai. Dez por segundo e
/// folga larga para recurso de verdade (um arquivo, uma tabela) e pouco para um laco.
pub const MENSAGENS_MAX_POR_MINUTO: u64 = 600;
pub const RECUO_MIN: Duration = Duration::from_secs(5);
pub const RECUO_MAX: Duration = Duration::from_secs(15 * 60);
/// Ids de tarefa lembrados no `Relato` de uma sessao: ela pode durar dias.
const LEMBRADOS: usize = 100;
const MINUTO: Duration = Duration::from_secs(60);
const UPDATED: &str = "notifications/resources/updated";
const LIST_CHANGED: &str = "notifications/resources/list_changed";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatilhoMcp {
    pub nome: String,
    /// Nome do servidor no arquivo do operador (`PHXCLAW_MCP_CONFIG`).
    pub servidor: String,
    /// URIs assinadas com `resources/subscribe`; a notificacao de outra uri e ignorada.
    #[serde(default)]
    pub recursos: Vec<String>,
    /// Dispara tambem com `notifications/resources/list_changed`.
    #[serde(default)]
    pub lista: bool,
    /// O fluxo (relativo a raiz do projeto); roda a versao publicada.
    pub fluxo: String,
    #[serde(default = "janela_padrao")]
    pub janela_ms: u64,
    #[serde(default = "taxa_padrao")]
    pub max_por_minuto: u32,
}

fn janela_padrao() -> u64 {
    JANELA_PADRAO_MS
}

fn taxa_padrao() -> u32 {
    TAXA_PADRAO_POR_MINUTO
}

#[derive(Deserialize, Default)]
struct Arquivo {
    #[serde(default)]
    mcp: Vec<GatilhoMcp>,
}

impl GatilhoMcp {
    fn validar(&self) -> Result<(), String> {
        let n = &self.nome;
        if n.is_empty()
            || n.len() > 64
            || !n
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!("nome de gatilho mcp invalido: {n:?}"));
        }
        if self.servidor.trim().is_empty() {
            return Err(format!("gatilho mcp {n}: falta 'servidor'"));
        }
        if self.recursos.is_empty() && !self.lista {
            return Err(format!(
                "gatilho mcp {n}: nada a escutar (informe 'recursos' e/ou 'lista': true)"
            ));
        }
        if self.recursos.len() > RECURSOS_MAX {
            return Err(format!(
                "gatilho mcp {n}: {} recursos, acima de {RECURSOS_MAX}",
                self.recursos.len()
            ));
        }
        let mut vistos = BTreeSet::new();
        for r in &self.recursos {
            if r.trim().is_empty() || r.len() > 2048 {
                return Err(format!("gatilho mcp {n}: recurso vazio ou longo demais"));
            }
            if !vistos.insert(r) {
                return Err(format!("gatilho mcp {n}: recurso repetido: {r}"));
            }
        }
        if !(JANELA_MIN_MS..=JANELA_MAX_MS).contains(&self.janela_ms) {
            return Err(format!(
                "gatilho mcp {n}: janela_ms de {JANELA_MIN_MS} a {JANELA_MAX_MS}"
            ));
        }
        if !(1..=TAXA_MAX_POR_MINUTO).contains(&self.max_por_minuto) {
            return Err(format!(
                "gatilho mcp {n}: max_por_minuto de 1 a {TAXA_MAX_POR_MINUTO}"
            ));
        }
        Ok(())
    }
}

/// Os gatilhos `mcp` de `<pasta>/gatilhos.json`, com o fluxo resolvido contra a raiz do
/// projeto e a versao publicada lida ja na carga: fluxo invalido para aqui, nao na primeira
/// notificacao.
pub fn carregar(pasta: &Path) -> Result<Vec<GatilhoMcp>, String> {
    let arq = pasta.join("gatilhos.json");
    let t = match std::fs::read_to_string(&arq) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(format!("{}: {e}", arq.display())),
    };
    let a: Arquivo = serde_json::from_str(&t).map_err(|e| format!("{}: {e}", arq.display()))?;
    let raiz = pasta.parent().unwrap_or(pasta);
    let mut nomes = BTreeSet::new();
    let mut gs = a.mcp;
    for g in &mut gs {
        g.validar().map_err(|e| format!("{}: {e}", arq.display()))?;
        if !nomes.insert(g.nome.clone()) {
            return Err(format!(
                "{}: gatilho mcp {} repetido",
                arq.display(),
                g.nome
            ));
        }
        let c = if Path::new(&g.fluxo).is_relative() {
            raiz.join(&g.fluxo)
        } else {
            PathBuf::from(&g.fluxo)
        };
        crate::fluxo_versoes::ler_publicado(&c).map_err(|e| {
            format!(
                "{}: gatilho mcp {}: fluxo {}: {e}",
                arq.display(),
                g.nome,
                g.fluxo
            )
        })?;
        g.fluxo = c.to_string_lossy().into_owned();
    }
    Ok(gs)
}

pub fn descrever(gs: &[GatilhoMcp]) -> String {
    gs.iter()
        .map(|g| {
            format!(
                "mcp '{}' do servidor {} ({} recurso(s){})",
                g.nome,
                g.servidor,
                g.recursos.len(),
                if g.lista { " e a lista" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

// ---------------------------------------------------------------- dedupe e taxa

/// O evento e a uri (`list_changed` nao tem uri).
type Chave = (&'static str, Option<String>);

struct Pendente {
    prazo: Instant,
    item: Value,
    vezes: u64,
}

/// O freio de um gatilho: coalesce a mesma chave dentro da janela e segura a taxa. Sem IO,
/// para o teste conferir o tempo sem servidor nenhum.
struct Freio {
    janela: Duration,
    max_por_minuto: usize,
    pendentes: BTreeMap<Chave, Pendente>,
    disparos: VecDeque<Instant>,
}

impl Freio {
    fn novo(g: &GatilhoMcp) -> Self {
        Self {
            janela: Duration::from_millis(g.janela_ms),
            max_por_minuto: g.max_por_minuto as usize,
            pendentes: BTreeMap::new(),
            disparos: VecDeque::new(),
        }
    }

    fn anotar(&mut self, chave: Chave, item: Value, agora: Instant) {
        let p = self.pendentes.entry(chave).or_insert(Pendente {
            prazo: agora + self.janela,
            item: Value::Null,
            vezes: 0,
        });
        p.item = item;
        p.vezes += 1;
    }

    fn esquecer_velhos(&mut self, agora: Instant) {
        while self
            .disparos
            .front()
            .is_some_and(|t| agora.duration_since(*t) >= MINUTO)
        {
            self.disparos.pop_front();
        }
    }

    /// Quando o proximo disparo pode sair: o prazo mais cedo, ou a vaga da taxa se ela
    /// estiver cheia. `None`: nada pendente.
    fn proximo(&mut self, agora: Instant) -> Option<Instant> {
        self.esquecer_velhos(agora);
        let prazo = self.pendentes.values().map(|p| p.prazo).min()?;
        let vaga = if self.disparos.len() >= self.max_por_minuto {
            self.disparos.front().map(|t| *t + MINUTO).unwrap_or(agora)
        } else {
            agora
        };
        Some(prazo.max(vaga))
    }

    /// Os itens vencidos, se a taxa deixa um disparo agora. Saem do pendente.
    fn vencidos(&mut self, agora: Instant) -> Option<Vec<Value>> {
        self.esquecer_velhos(agora);
        if self.disparos.len() >= self.max_por_minuto {
            return None;
        }
        let chaves: Vec<Chave> = self
            .pendentes
            .iter()
            .filter(|(_, p)| p.prazo <= agora)
            .map(|(k, _)| k.clone())
            .collect();
        if chaves.is_empty() {
            return None;
        }
        Some(
            chaves
                .into_iter()
                .filter_map(|k| self.pendentes.remove(&k))
                .map(|p| {
                    let mut item = p.item;
                    item["vezes"] = json!(p.vezes);
                    item
                })
                .collect(),
        )
    }

    fn disparou(&mut self, agora: Instant) {
        self.disparos.push_back(agora);
    }

    /// Recusa passageira (o balde da API): os itens voltam ao pendente para `quando`.
    fn devolver(&mut self, itens: Vec<Value>, quando: Instant) {
        for item in itens {
            let evento = if item["evento"] == "resources/list_changed" {
                LIST_CHANGED
            } else {
                UPDATED
            };
            let chave = (evento, item["uri"].as_str().map(str::to_string));
            let vezes = item["vezes"].as_u64().unwrap_or(1);
            let p = self.pendentes.entry(chave).or_insert(Pendente {
                prazo: quando,
                item: Value::Null,
                vezes: 0,
            });
            // A que chegou enquanto o disparo era recusado e mais nova: fica a dela.
            if p.item.is_null() {
                p.item = item;
            }
            p.vezes += vezes;
            p.prazo = p.prazo.min(quando);
        }
    }

    /// Fim da sessao: o pendente sai sem esperar a janela, mas dentro da taxa.
    fn encerrar(&mut self, agora: Instant) {
        for p in self.pendentes.values_mut() {
            p.prazo = p.prazo.min(agora);
        }
    }
}

// ---------------------------------------------------------------- a sessao

/// Por que uma sessao terminou.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum Fim {
    /// O prazo pedido pelo chamador (os testes); o laco do servidor nao pede prazo.
    #[default]
    Prazo,
    /// O fio caiu (o servidor saiu, linha grande demais, JSON invalido).
    Fio(String),
    /// Mais de `MENSAGENS_MAX_POR_MINUTO` mensagens num minuto.
    Inundacao(u64),
}

/// O que uma sessao fez.
#[derive(Debug, Default)]
pub struct Relato {
    /// Ids das tarefas disparadas (os ultimos `LEMBRADOS`).
    pub disparos: VecDeque<String>,
    pub n_disparos: u64,
    /// Disparos recusados de vez (credencial na entrada, fluxo ilegivel), com o motivo.
    pub recusas: VecDeque<String>,
    pub mensagens: u64,
    /// Mensagens que nao eram evento deste gatilho (outra uri, outro metodo, pedido).
    pub ignoradas: u64,
    /// Avisos que ficaram pendentes acima da taxa quando a sessao acabou: o freio morre com
    /// ela, e eles se perdem -- contados aqui e no log, nunca calados.
    pub descartadas: u64,
    pub fim: Fim,
}

fn lembrar(v: &mut VecDeque<String>, s: String) {
    v.push_back(s);
    while v.len() > LEMBRADOS {
        v.pop_front();
    }
}

/// Um gatilho armado: a configuracao, o arquivo do operador e a raiz do agente.
pub struct Assinante {
    pub gatilho: GatilhoMcp,
    /// O arquivo de `PHXCLAW_MCP_CONFIG`, de onde sai o servidor.
    pub config_mcp: PathBuf,
    /// A raiz do agente: o broker das credenciais e `gatilhos/` (a evidencia).
    pub raiz: PathBuf,
}

impl Assinante {
    pub fn new(gatilho: GatilhoMcp, config_mcp: &Path, raiz_do_agente: &Path) -> Self {
        Self {
            gatilho,
            config_mcp: config_mcp.to_path_buf(),
            raiz: raiz_do_agente.to_path_buf(),
        }
    }

    /// O item do fluxo, se a mensagem e um evento que este gatilho escuta.
    fn evento(&self, v: &Value) -> Option<(Chave, Value)> {
        // Com `id` e pedido do servidor (ja respondido pelo leitor) ou resposta: nao e evento.
        if v.get("id").is_some() {
            return None;
        }
        let metodo = v.get("method")?.as_str()?;
        let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
        let g = &self.gatilho;
        let (evento, uri) = match metodo {
            UPDATED => {
                let uri = params.get("uri")?.as_str()?;
                // So o que se assinou: o servidor nao escolhe o que dispara o fluxo.
                if !g.recursos.iter().any(|r| r == uri) {
                    return None;
                }
                (UPDATED, Some(uri.to_string()))
            }
            LIST_CHANGED if g.lista => (LIST_CHANGED, None),
            _ => return None,
        };
        let item = json!({
            "gatilho": g.nome,
            "servidor": g.servidor,
            "evento": evento.trim_start_matches("notifications/"),
            "uri": uri,
            "parametros": params,
        });
        Some(((evento, uri), item))
    }

    fn ledger(&self) -> Result<EvidenceLedger, String> {
        let pasta = self.raiz.join(PASTA);
        std::fs::create_dir_all(&pasta).map_err(|e| format!("{}: {e}", pasta.display()))?;
        EvidenceLedger::open(pasta.join(format!("mcp-{}.evidence.jsonl", self.gatilho.nome)))
            .map_err(|e| format!("evidencia do gatilho mcp: {e}"))
    }

    /// Dispara o que venceu, pela versao publicada do fluxo.
    fn disparar_vencidos(&self, s: &ApiState, freio: &mut Freio, r: &mut Relato, agora: Instant) {
        let Some(itens) = freio.vencidos(agora) else {
            return;
        };
        let g = &self.gatilho;
        match crate::fluxo_versoes::criar_fluxo_publicado(s, &g.fluxo, itens.clone(), |_| Ok(())) {
            Ok(c) => {
                freio.disparou(agora);
                r.n_disparos += 1;
                println!(
                    "gatilho mcp {}: {} evento(s), tarefa {}",
                    g.nome,
                    itens.len(),
                    c.id
                );
                lembrar(&mut r.disparos, c.id);
            }
            Err(e) if e.status == axum::http::StatusCode::TOO_MANY_REQUESTS => {
                let espera = Duration::from_secs(e.retry_after.unwrap_or(1).max(1));
                freio.devolver(itens, agora + espera);
            }
            Err(e) => {
                let m = format!("gatilho mcp {}: disparo recusado: {}", g.nome, e.erro);
                eprintln!("{m}");
                lembrar(&mut r.recusas, m);
            }
        }
    }

    /// Uma sessao: confere a capacidade no agente da fabrica, sobe o servidor, assina e
    /// escuta ate o fio cair, a inundacao ou o `prazo` (os testes; o laco nao pede prazo).
    /// `Err` e a sessao que nem comecou (capacidade, servidor, assinatura recusada).
    pub async fn sessao(&self, s: &ApiState, prazo: Option<Duration>) -> Result<Relato, String> {
        let g = &self.gatilho;
        let servidor =
            crate::mcp::servidor_do_operador(&self.config_mcp, &g.servidor, Some(&self.raiz))?;
        let ledger = self.ledger()?;
        let agente = (s.factory)(&s.default_model).map_err(|e| format!("agente: {e}"))?;
        if !agente.config.capabilities.contains(servidor.capacidade()) {
            let _ = ledger.append(EvidenceDraft {
                action_uuid: phxclaw_types::new_uuid_v7(),
                correlation_uuid: None,
                actor: "phxclaw-agent".into(),
                capability: servidor.capacidade().into(),
                action: format!("gatilho mcp {}: resources/subscribe", g.nome),
                outcome: EvidenceOutcome::Denied,
                request_summary: json!({"servidor": g.servidor, "recursos": g.recursos,
                                        "lista": g.lista}),
                result_summary: json!({"motivo": "capacidade nao concedida"}),
                artifact_uris: vec![],
            });
            return Err(format!(
                "capacidade {} nao concedida ao agente do servidor: o gatilho mcp {} nao sobe o \
                 servidor {}",
                servidor.capacidade(),
                g.nome,
                g.servidor
            ));
        }
        let auditoria = RuntimeAudit::new("phxclaw-agent", Some(ledger));
        let mut a = servidor.assinar(&g.recursos, g.lista, auditoria).await?;
        let mut freio = Freio::novo(g);
        let mut r = Relato::default();
        let fim_pedido = prazo.map(|p| Instant::now() + p);
        let mut minuto = (Instant::now(), 0u64);
        loop {
            self.disparar_vencidos(s, &mut freio, &mut r, Instant::now());
            let acordar = [freio.proximo(Instant::now()), fim_pedido]
                .into_iter()
                .flatten()
                .min();
            // `recv` do mpsc e seguro de cortar: o prazo so descarta a espera, nunca meia
            // mensagem (a linha do fio e lida inteira pela tarefa do leitor).
            let msg = match acordar {
                Some(t) => tokio::time::timeout_at(t.into(), a.proxima()).await.ok(),
                None => Some(a.proxima().await),
            };
            let v = match msg {
                None => {
                    if fim_pedido.is_some_and(|f| Instant::now() >= f) {
                        r.fim = Fim::Prazo;
                        break;
                    }
                    continue;
                }
                Some(None) => {
                    r.fim = Fim::Fio("o leitor do servidor terminou".into());
                    break;
                }
                Some(Some(Err(e))) => {
                    r.fim = Fim::Fio(e);
                    break;
                }
                Some(Some(Ok(v))) => v,
            };
            r.mensagens += 1;
            let agora = Instant::now();
            if agora.duration_since(minuto.0) >= MINUTO {
                minuto = (agora, 0);
            }
            minuto.1 += 1;
            if minuto.1 > MENSAGENS_MAX_POR_MINUTO {
                r.fim = Fim::Inundacao(minuto.1);
                break;
            }
            match self.evento(&v) {
                Some((chave, item)) => freio.anotar(chave, item, agora),
                None => r.ignoradas += 1,
            }
        }
        // O processo morre aqui, antes de qualquer disparo final: a inundacao para ja.
        drop(a);
        freio.encerrar(Instant::now());
        self.disparar_vencidos(s, &mut freio, &mut r, Instant::now());
        r.descartadas = freio.pendentes.values().map(|p| p.vezes).sum();
        if r.descartadas > 0 {
            eprintln!(
                "gatilho mcp {}: {} aviso(s) acima da taxa descartado(s) no fim da sessao",
                g.nome, r.descartadas
            );
        }
        Ok(r)
    }
}

/// O laco de um gatilho no servidor: uma sessao atras da outra, com recuo que dobra a cada
/// queda e volta ao minimo depois de uma sessao longa.
pub async fn laco(s: ApiState, a: Assinante) {
    let mut recuo = RECUO_MIN;
    loop {
        let inicio = Instant::now();
        match a.sessao(&s, None).await {
            Ok(r) => {
                let motivo = match &r.fim {
                    Fim::Prazo => "prazo".to_string(),
                    Fim::Fio(e) => format!("fio caiu: {e}"),
                    Fim::Inundacao(n) => format!("inundacao ({n} mensagens em um minuto)"),
                };
                eprintln!(
                    "gatilho mcp {}: sessao terminou ({motivo}); {} disparo(s), {} recusa(s)",
                    a.gatilho.nome,
                    r.n_disparos,
                    r.recusas.len()
                );
            }
            Err(e) => eprintln!("gatilho mcp {}: {e}", a.gatilho.nome),
        }
        recuo = if inicio.elapsed() >= RECUO_MAX {
            RECUO_MIN
        } else {
            (recuo * 2).min(RECUO_MAX)
        };
        tokio::time::sleep(recuo).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(janela_ms: u64, max: u32) -> GatilhoMcp {
        GatilhoMcp {
            nome: "t".into(),
            servidor: "s".into(),
            recursos: vec!["a".into(), "b".into()],
            lista: true,
            fluxo: "f.json".into(),
            janela_ms,
            max_por_minuto: max,
        }
    }

    fn chave(uri: &str) -> Chave {
        (UPDATED, Some(uri.to_string()))
    }

    /// Tres avisos da mesma uri na janela viram UM item, com a ultima versao e `vezes: 3`;
    /// antes do fim da janela nada sai.
    #[test]
    fn a_mesma_chave_na_janela_vira_um_disparo_com_a_ultima_versao() {
        let mut f = Freio::novo(&g(1_000, 10));
        let t0 = Instant::now();
        for n in 1..=3 {
            f.anotar(chave("a"), json!({"uri": "a", "n": n}), t0);
        }
        assert!(f.vencidos(t0 + Duration::from_millis(999)).is_none());
        let itens = f.vencidos(t0 + Duration::from_millis(1_000)).unwrap();
        assert_eq!(itens, vec![json!({"uri": "a", "n": 3, "vezes": 3})]);
        assert!(f.pendentes.is_empty());
    }

    /// Acima da taxa, o pendente espera a vaga: com 2 por minuto, o terceiro disparo so sai
    /// um minuto depois do primeiro -- e sai (nunca zero vezes).
    #[test]
    fn acima_da_taxa_o_pendente_espera_a_vaga() {
        let mut f = Freio::novo(&g(100, 2));
        let t0 = Instant::now();
        for i in 0..3u64 {
            let t = t0 + Duration::from_millis(200 * i);
            f.anotar(chave("a"), json!({"uri": "a"}), t);
            let quando = t + Duration::from_millis(100);
            assert_eq!(
                f.proximo(t),
                Some(if i < 2 {
                    quando
                } else {
                    t0 + MINUTO + Duration::from_millis(100)
                })
            );
            if i < 2 {
                assert!(f.vencidos(quando).is_some());
                f.disparou(quando);
            } else {
                assert!(f.vencidos(quando).is_none(), "passou da taxa");
            }
        }
        let vaga = t0 + MINUTO + Duration::from_millis(100);
        assert_eq!(f.vencidos(vaga).map(|v| v.len()), Some(1));
    }
}
