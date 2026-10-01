//! Otimizacao de skill por A/B medido: o modelo local escreve uma variante do `SKILL.md`, o
//! avaliador roda os MESMOS casos (gravacoes ou casos com gabarito) com a original e com a
//! variante, e a variante so substitui a original quando a faixa min-max de acerto dela
//! fica inteira acima da faixa da original. Pior, empatada ou dentro do ruido: nao se
//! promove, e a decisao vai para o registro com os numeros.
//!
//! Hipotese que morreu (pesquisa da onda 5): otimizador de prompt (DSPy/GEPA) promovendo pela
//! media. Nem o OpenJarvis mede o efeito; promover pela media e exatamente o «vencedor
//! dentro do ruido» que esta casa ja publicou uma vez.

use crate::avaliacao::{Caso, Faixa, Lado, LeitorEnergia, avaliar, faixas_decidem};
use crate::motor::Agent;
use phxclaw_agent_core::{Llm, LlmOptions, Message, Tool};
use phxclaw_skill_runtime::{SkillFolder, parse_skill_doc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A skill trocada no agente ja montado: a pasta do prompt e a do `skill_load` passam a
/// ser `pasta`. Sem `skill.read` concedida a skill nunca chega ao modelo e o A/B mediria
/// nada -- recusa em vez de devolver um empate falso.
pub fn com_pasta_de_skills(mut a: Agent, pasta: &Path) -> Result<Agent, String> {
    if !a.config.capabilities.contains("skill.read") {
        return Err(
            "A/B de skill sem a capacidade skill.read: a skill nao chegaria ao modelo".into(),
        );
    }
    let p = SkillFolder::new(pasta);
    a.config.skills = Some(p.clone());
    let mut trocou = false;
    for t in a.tools.iter_mut() {
        if t.spec().name == "skill_load" {
            *t = Arc::new(crate::skills::SkillLoadTool { pasta: p.clone() }) as Arc<dyn Tool>;
            trocou = true;
        }
    }
    if !trocou {
        a.tools
            .push(Arc::new(crate::skills::SkillLoadTool { pasta: p }));
    }
    Ok(a)
}

const PEDIDO_DE_VARIANTE: &str = "You improve instruction files (skills) for an autonomous agent. \
Rewrite the SKILL.md below so the agent follows it more reliably: clearer steps, the exact tool to \
use at each step, and what to check before finishing. Keep the same `name:` in the header. Reply \
with the complete new SKILL.md only, starting with the line ---, and nothing else.";

/// Pede a variante ao modelo e confere: tem de ser um `SKILL.md` valido, com o mesmo nome,
/// e diferente da original (variante igual compararia a skill com ela mesma).
pub async fn gerar_variante(llm: &dyn Llm, nome: &str, original: &str) -> Result<String, String> {
    let msgs = [
        Message::system(PEDIDO_DE_VARIANTE),
        Message::user(original.to_string()),
    ];
    let r = llm
        .chat(&msgs, &[], &LlmOptions::default())
        .await
        .map_err(|e| format!("modelo da variante: {e}"))?;
    let texto = sem_cerca(&r.content);
    // O comeco do que voltou vai junto da recusa: «variante invalida» sozinho nao diz se o
    // modelo esqueceu o cabecalho, conversou antes dele ou devolveu outra coisa.
    let trecho: String = texto.chars().take(160).collect();
    let doc = parse_skill_doc(&texto)
        .map_err(|e| format!("variante invalida: {e}; o modelo devolveu: {trecho:?}"))?;
    if doc.name != nome {
        return Err(format!(
            "variante trocou o nome da skill ({} em vez de {nome})",
            doc.name
        ));
    }
    if texto.trim() == original.trim() {
        return Err("variante identica a original".into());
    }
    Ok(texto)
}

/// Modelo pequeno costuma cercar a resposta com ```markdown; a cerca nao e da skill.
fn sem_cerca(t: &str) -> String {
    let t = t.trim();
    if let Some(r) = t.strip_prefix("```") {
        let r = r.split_once('\n').map_or("", |(_, d)| d);
        return r.trim_end().trim_end_matches("```").trim().to_string() + "\n";
    }
    t.to_string() + "\n"
}

/// A decisao, como vai para o registro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decisao {
    pub data: String,
    pub skill: String,
    pub modelo: String,
    pub casos: usize,
    pub rodadas: usize,
    pub acerto_original: Faixa,
    pub acerto_variante: Faixa,
    pub promovida: bool,
    pub motivo: String,
    pub variante_sha256: String,
    /// Onde a original ficou guardada quando a variante entrou no lugar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_guardada: Option<PathBuf>,
}

pub struct PedidoOtimizacao<'a> {
    pub skill: &'a str,
    /// A pasta de skills de verdade (`<pasta>/<skill>/SKILL.md`): so ela muda, e so na
    /// promocao.
    pub pasta_skills: &'a Path,
    pub modelo: &'a str,
    pub casos: &'a [Caso],
    pub rodadas: usize,
    /// Onde os bracos, as gravacoes de cada execucao e os resultados ficam.
    pub trabalho: &'a Path,
}

/// Monta o agente de um braco: o modelo e a pasta de skills daquele braco.
pub type FabricaDeBraco<'a> = &'a (dyn Fn(&str, &Path) -> Result<Agent, String> + Sync);

/// A/B da `variante` contra a skill atual e a decisao, ja registrada em
/// `<pasta_skills>/<skill>/otimizacao.jsonl`. So a promocao toca a pasta de verdade.
pub async fn ab(
    fabrica: FabricaDeBraco<'_>,
    p: &PedidoOtimizacao<'_>,
    variante: &str,
) -> Result<Decisao, String> {
    let dir_skill = p.pasta_skills.join(p.skill);
    let arq_skill = dir_skill.join("SKILL.md");
    let original =
        std::fs::read_to_string(&arq_skill).map_err(|e| format!("{}: {e}", arq_skill.display()))?;
    let mut acertos = Vec::new();
    for (braco, texto) in [("original", original.as_str()), ("variante", variante)] {
        // Cada braco tem uma pasta so com a skill em teste: as outras skills ficam de fora
        // para a diferenca medida ser so a dela.
        let pasta = p.trabalho.join(braco).join("skills");
        let d = pasta.join(p.skill);
        std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        std::fs::write(d.join("SKILL.md"), texto).map_err(|e| e.to_string())?;
        let fab = |m: &str| fabrica(m, &pasta);
        let r = avaliar(
            &fab,
            &[p.modelo.to_string()],
            p.casos,
            p.rodadas,
            &p.trabalho.join(braco),
            &LeitorEnergia::do_sistema(),
        )
        .await?;
        acertos.push(r.modelos[0].acerto_por_rodada);
    }
    let (a, b) = (acertos[0], acertos[1]);
    let promovida = faixas_decidem(&a, &b, true) == Some(Lado::B);
    let motivo = if promovida {
        format!(
            "faixa da variante [{:.2}–{:.2}] inteira acima da original [{:.2}–{:.2}]",
            b.min, b.max, a.min, a.max
        )
    } else if faixas_decidem(&a, &b, true) == Some(Lado::A) {
        format!(
            "variante PIOR: [{:.2}–{:.2}] abaixo da original [{:.2}–{:.2}]",
            b.min, b.max, a.min, a.max
        )
    } else {
        format!(
            "faixas se cruzam (variante [{:.2}–{:.2}], original [{:.2}–{:.2}]): empate nao promove",
            b.min, b.max, a.min, a.max
        )
    };
    let mut d = Decisao {
        data: chrono::Utc::now().to_rfc3339(),
        skill: p.skill.to_string(),
        modelo: p.modelo.to_string(),
        casos: p.casos.len(),
        rodadas: p.rodadas,
        acerto_original: a,
        acerto_variante: b,
        promovida,
        motivo,
        variante_sha256: crate::motor::sha256_hex(variante.as_bytes()),
        original_guardada: None,
    };
    if promovida {
        // A original nao se perde: fica ao lado, com a data, e o registro diz onde.
        let guardada = dir_skill.join(format!(
            "SKILL.md.{}.anterior",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
        ));
        std::fs::rename(&arq_skill, &guardada).map_err(|e| e.to_string())?;
        std::fs::write(&arq_skill, variante).map_err(|e| e.to_string())?;
        d.original_guardada = Some(guardada);
    }
    registrar(&dir_skill, &d)?;
    Ok(d)
}

/// A recusa que acontece ANTES do A/B (variante invalida, igual, com outro nome) tambem e
/// decisao, e vai para o mesmo registro: variante que nem chegou a ser medida nao pode
/// sumir do historico como se ninguem tivesse tentado.
pub fn registrar_recusa(
    pasta_skills: &Path,
    skill: &str,
    modelo: &str,
    motivo: &str,
) -> Result<(), String> {
    let linha = serde_json::json!({
        "data": chrono::Utc::now().to_rfc3339(),
        "skill": skill,
        "modelo": modelo,
        "promovida": false,
        "medida": false,
        "motivo": motivo,
    });
    escrever_registro(&pasta_skills.join(skill), &linha)
}

fn registrar(dir_skill: &Path, d: &Decisao) -> Result<(), String> {
    escrever_registro(
        dir_skill,
        &serde_json::to_value(d).map_err(|e| e.to_string())?,
    )
}

fn escrever_registro(dir_skill: &Path, d: &serde_json::Value) -> Result<(), String> {
    use std::io::Write;
    let arq = dir_skill.join("otimizacao.jsonl");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&arq)
        .map_err(|e| format!("{}: {e}", arq.display()))?;
    writeln!(f, "{d}").map_err(|e| format!("{}: {e}", arq.display()))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cerca_de_markdown_sai_da_variante() {
        assert_eq!(
            sem_cerca("```markdown\n---\nname: a\n---\ncorpo\n```"),
            "---\nname: a\n---\ncorpo\n"
        );
        assert_eq!(sem_cerca("---\nx\n"), "---\nx\n");
    }
}
