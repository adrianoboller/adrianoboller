//! Estilos de saida (o `output style` do Claude Code): um arquivo `.md` por estilo, com
//! `name` e `description` no cabecalho e as instrucoes no corpo, somadas ao prompt de
//! sistema da tarefa.
//!
//! Tres vem embutidos (`conciso`, `explicativo`, `aprendizado`, em `estilos/` da crate) e o
//! projeto acrescenta ou sobrepoe os seus em `.phxclaw/estilos/<nome>.md`. Sobrepor pelo
//! nome e de proposito: o operador ajusta o `conciso` da casa sem inventar um `conciso2`.

use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct EstiloDeSaida {
    pub nome: String,
    pub descricao: String,
    pub instrucoes: String,
}

const EMBUTIDOS: &[(&str, &str)] = &[
    ("conciso", include_str!("../estilos/conciso.md")),
    ("explicativo", include_str!("../estilos/explicativo.md")),
    ("aprendizado", include_str!("../estilos/aprendizado.md")),
];

/// Cabecalho `---` com `name:` e `description:`, e o corpo. Sem cabecalho, o nome vem do
/// arquivo e o texto inteiro e a instrucao.
pub fn ler(nome_do_arquivo: &str, texto: &str) -> Result<EstiloDeSaida, String> {
    let mut nome = nome_do_arquivo.to_string();
    let mut descricao = String::new();
    let corpo = match texto.trim_start().strip_prefix("---") {
        Some(resto) => {
            let (cab, corpo) = resto
                .split_once("\n---")
                .ok_or("cabecalho '---' sem fecho")?;
            for l in cab.lines() {
                if let Some((k, v)) = l.split_once(':') {
                    match k.trim() {
                        "name" => nome = v.trim().to_string(),
                        "description" => descricao = v.trim().to_string(),
                        _ => {}
                    }
                }
            }
            corpo.trim_start_matches(['-', '\r']).to_string()
        }
        None => texto.to_string(),
    };
    let instrucoes = corpo.trim().to_string();
    if instrucoes.is_empty() {
        return Err(format!("estilo {nome} sem instrucoes"));
    }
    Ok(EstiloDeSaida {
        nome,
        descricao,
        instrucoes,
    })
}

/// O estilo pelo nome: o do projeto ganha do embutido. Nome so com letra, digito, `-` e
/// `_`, porque vira caminho de arquivo.
pub fn carregar(nome: &str, pasta_do_projeto: Option<&Path>) -> Result<EstiloDeSaida, String> {
    if nome.is_empty()
        || !nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(format!("nome de estilo invalido: {nome:?}"));
    }
    if let Some(p) = pasta_do_projeto {
        let arq = p.join("estilos").join(format!("{nome}.md"));
        if let Ok(t) = std::fs::read_to_string(&arq) {
            return ler(nome, &t).map_err(|e| format!("{}: {e}", arq.display()));
        }
    }
    EMBUTIDOS
        .iter()
        .find(|(n, _)| *n == nome)
        .map(|(n, t)| ler(n, t))
        .unwrap_or_else(|| {
            Err(format!(
                "estilo desconhecido: {nome} (disponiveis: {})",
                listar(pasta_do_projeto)
                    .iter()
                    .map(|e| e.nome.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })
}

/// Todos os estilos visiveis, embutidos e do projeto, sem repetir nome.
pub fn listar(pasta_do_projeto: Option<&Path>) -> Vec<EstiloDeSaida> {
    let mut v: Vec<EstiloDeSaida> = Vec::new();
    if let Some(p) = pasta_do_projeto {
        let mut arqs: Vec<_> = std::fs::read_dir(p.join("estilos"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        arqs.sort();
        for a in arqs {
            let n = a
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if let Ok(e) = std::fs::read_to_string(&a)
                .map_err(|e| e.to_string())
                .and_then(|t| ler(&n, &t))
            {
                v.push(e);
            }
        }
    }
    for (n, t) in EMBUTIDOS {
        if !v.iter().any(|e| e.nome == *n)
            && let Ok(e) = ler(n, t)
        {
            v.push(e);
        }
    }
    v
}

/// O bloco que o motor poe no prompt de sistema.
pub fn bloco_para_o_prompt(e: &EstiloDeSaida) -> String {
    format!(
        "Output style \"{}\" (how to write every answer to the user in this task):\n{}",
        e.nome, e.instrucoes
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embutidos_carregam_e_projeto_sobrepoe() {
        for (n, _) in EMBUTIDOS {
            let e = carregar(n, None).unwrap();
            assert_eq!(&e.nome, n);
            assert!(!e.descricao.is_empty() && !e.instrucoes.contains("---"));
        }
        let d = std::env::temp_dir().join(format!("phx-estilos-{}", std::process::id()));
        std::fs::create_dir_all(d.join("estilos")).unwrap();
        std::fs::write(
            d.join("estilos/conciso.md"),
            "---\nname: conciso\ndescription: meu\n---\nSo numeros.",
        )
        .unwrap();
        std::fs::write(d.join("estilos/pirata.md"), "Fale como pirata.").unwrap();
        assert_eq!(
            carregar("conciso", Some(&d)).unwrap().instrucoes,
            "So numeros."
        );
        assert_eq!(
            carregar("pirata", Some(&d)).unwrap().instrucoes,
            "Fale como pirata."
        );
        assert_eq!(listar(Some(&d)).len(), 4);
        assert!(carregar("../segredo", Some(&d)).is_err());
        assert!(
            carregar("inexistente", Some(&d))
                .unwrap_err()
                .contains("pirata")
        );
        let _ = std::fs::remove_dir_all(d);
    }
}
