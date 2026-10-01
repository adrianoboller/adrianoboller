//! Indexacao de documentos: `phxclaw indexar DIR` grava o indice e `doc_search` busca
//! nele. O motor (BM25, trechos, formato do indice) e o do `phxclaw-memory-context`, o
//! mesmo crate da memoria; aqui so mora o que e do agente -- onde o indice fica, a
//! ferramenta e a reordenacao opcional por embeddings.
//!
//! A reordenacao (`PHXCLAW_DOCS_EMBED=ollama:all-minilm`) nao substitui o BM25: pega os
//! melhores dele e funde as duas ordens por posicao (RRF, k=60). Ordenar so pelo cosseno
//! deixaria um modelo de 23 MB desfazer o casamento exato de um termo raro (um nome de
//! funcao, um codigo de erro), que e justamente onde o BM25 acerta e o embedding erra.
//! Sem Ollama, ou com ele fora do ar, a busca volta na ordem do BM25 e diz que nao
//! reordenou -- ferramenta de leitura nao falha por causa de um enfeite.
//!
//! E fica DESLIGADA por padrao por medicao, nao por cautela: em 01/10/2026, sobre os 156
//! arquivos de `docs/` com 8 consultas de gabarito (5 em portugues, 3 em ingles), o BM25
//! pos o arquivo certo em 1o lugar em 6 e a fusao com o all-minilm em 3. O all-minilm e
//! treinado em ingles e o acervo e portugues; quem tiver acervo em ingles mede de novo.

use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_memory_context::bm25::{AchadoDoc, Bm25, IndiceDeDocumentos, Relatorio};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Variavel do modelo de embedding da reordenacao; so `ollama:` e aceito.
pub const VAR_EMBED: &str = "PHXCLAW_DOCS_EMBED";
/// Candidatos do BM25 que a reordenacao olha.
pub const CANDIDATOS: usize = 20;
/// Caracteres de cada trecho devolvidos ao modelo.
pub const TRECHO_NO_RESULTADO: usize = 700;

/// `<pasta do agente>/indice-docs/indice.json`.
pub fn arquivo_do_indice(pasta_do_agente: &Path) -> PathBuf {
    pasta_do_agente.join("indice-docs").join("indice.json")
}

/// O que a CLI chama: le o indice que houver, reindexa `dir` e grava.
pub fn indexar(pasta_do_agente: &Path, dir: &Path) -> Result<Relatorio, String> {
    let arq = arquivo_do_indice(pasta_do_agente);
    let mut idx = if arq.exists() {
        IndiceDeDocumentos::carregar(&arq)?
    } else {
        IndiceDeDocumentos::default()
    };
    let r = idx.indexar_pasta(dir)?;
    idx.gravar(&arq)?;
    Ok(r)
}

/// Fundir duas ordens por posicao: `1/(k+pos)` somado. Nao depende da escala de nenhuma
/// das duas pontuacoes, que e o que torna BM25 e cosseno somaveis.
pub fn fundir_por_posicao(ordem_a: &[usize], ordem_b: &[usize], k: f64) -> Vec<usize> {
    let mut pontos: std::collections::BTreeMap<usize, f64> = Default::default();
    for ordem in [ordem_a, ordem_b] {
        for (pos, &d) in ordem.iter().enumerate() {
            *pontos.entry(d).or_default() += 1.0 / (k + pos as f64 + 1.0);
        }
    }
    let mut v: Vec<(usize, f64)> = pontos.into_iter().collect();
    v.sort_by(|a, b| {
        b.1.total_cmp(&a.1).then_with(|| {
            let pa = ordem_a.iter().position(|x| *x == a.0);
            let pb = ordem_a.iter().position(|x| *x == b.0);
            pa.cmp(&pb)
        })
    });
    v.into_iter().map(|(d, _)| d).collect()
}

fn cosseno(a: &[f32], b: &[f32]) -> f32 {
    let (mut p, mut na, mut nb) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(b) {
        p += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        p / (na.sqrt() * nb.sqrt())
    }
}

/// O indice carregado e o motor montado, uma vez por montagem do agente.
pub struct DocSearchTool {
    pub indice: Arc<(IndiceDeDocumentos, Bm25)>,
    pub embed: Option<Arc<phxclaw_llm::OllamaLlm>>,
}

impl DocSearchTool {
    /// So existe se ha indice gravado: sem `phxclaw indexar`, a ferramenta nem aparece.
    /// Indice ilegivel vira aviso, nunca agente que nao sobe.
    pub fn da_pasta(pasta_do_agente: &Path) -> Option<Self> {
        let arq = arquivo_do_indice(pasta_do_agente);
        if !arq.exists() {
            return None;
        }
        let idx = IndiceDeDocumentos::carregar(&arq)
            .map_err(|e| eprintln!("aviso: doc_search desligado: {e}"))
            .ok()?;
        let motor = idx.motor();
        Some(Self {
            indice: Arc::new((idx, motor)),
            embed: embed_do_ambiente(),
        })
    }

    /// A busca inteira, para a ferramenta e para os testes: BM25 e, com embedding, a
    /// fusao. Devolve os achados e se reordenou (ou o motivo de nao ter reordenado).
    pub async fn buscar(&self, consulta: &str, n: usize) -> (Vec<AchadoDoc>, Result<(), String>) {
        let (idx, motor) = &*self.indice;
        let Some(embed) = &self.embed else {
            return (idx.buscar(motor, consulta, n), Err("sem modelo".into()));
        };
        let cand = idx.buscar(motor, consulta, CANDIDATOS.max(n));
        if cand.len() < 2 {
            return (cand, Ok(()));
        }
        let mut textos = vec![consulta.to_string()];
        textos.extend(cand.iter().map(|a| a.texto.clone()));
        match embed.incorporar(&textos).await {
            Ok(v) => {
                let q = &v[0];
                let mut por_cos: Vec<(usize, f32)> = v[1..]
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (i, cosseno(q, e)))
                    .collect();
                por_cos.sort_by(|a, b| b.1.total_cmp(&a.1));
                let ordem_cos: Vec<usize> = por_cos.into_iter().map(|(i, _)| i).collect();
                let ordem_bm25: Vec<usize> = (0..cand.len()).collect();
                let ordem = fundir_por_posicao(&ordem_bm25, &ordem_cos, 60.0);
                let r = ordem.into_iter().take(n).map(|i| cand[i].clone()).collect();
                (r, Ok(()))
            }
            Err(e) => {
                let mut c = cand;
                c.truncate(n);
                (c, Err(e.to_string()))
            }
        }
    }
}

fn embed_do_ambiente() -> Option<Arc<phxclaw_llm::OllamaLlm>> {
    let spec = std::env::var(VAR_EMBED)
        .ok()
        .filter(|s| !s.trim().is_empty())?;
    let Some(modelo) = spec.trim().strip_prefix("ollama:") else {
        eprintln!("aviso: {VAR_EMBED}={spec}: so ollama:<modelo> e aceito");
        return None;
    };
    phxclaw_llm::ollama_do_ambiente(modelo)
        .map_err(|e| eprintln!("aviso: {VAR_EMBED}: {e}"))
        .ok()
        .map(Arc::new)
}

impl Tool for DocSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "doc_search".into(),
            description: "Search the operator's indexed document folders (BM25 over paragraphs). \
Returns file:line and the matching passage. Use it before answering questions about those documents."
                .into(),
            parameters: json!({"type":"object","properties":{
                "query":{"type":"string"},
                "max_results":{"type":"integer","minimum":1,"maximum":10}
            },"required":["query"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "doc.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let q = args
                .get("query")
                .and_then(Value::as_str)
                .filter(|q| !q.trim().is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("falta 'query'".into()))?;
            let n = args
                .get("max_results")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .clamp(1, 10) as usize;
            let (achados, reordenou) = self.buscar(q, n).await;
            if achados.is_empty() {
                return Ok(ToolOutput::text("nenhum trecho casa a consulta"));
            }
            let mut s = String::new();
            if let (Some(_), Err(e)) = (&self.embed, &reordenou) {
                s.push_str(&format!("(ordem do BM25; reordenacao falhou: {e})\n"));
            }
            for (i, a) in achados.iter().enumerate() {
                let t: String = a.texto.chars().take(TRECHO_NO_RESULTADO).collect();
                s.push_str(&format!(
                    "{}. {}:{} (bm25 {:.2})\n{}\n\n",
                    i + 1,
                    a.caminho.display(),
                    a.linha,
                    a.pontos,
                    t
                ));
            }
            Ok(ToolOutput::text(s.trim_end().to_string()))
        })
    }
}
