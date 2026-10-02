//! Comandos de barra: `/nome argumentos` no objetivo vira o corpo de `commands/nome.md`,
//! no formato do Claude Code (`$ARGUMENTS` recebe o resto da linha). Duas fontes, na
//! mesma tabela: o `.phxclaw/commands/` do projeto e os pacotes de plugin assinados
//! (`pacotes.rs`). Nome repetido fica com o primeiro que entrou -- o do projeto, que e o
//! operador -- e o segundo vira aviso, nunca sobreposicao calada.
//!
//! O objetivo gravado na tarefa continua sendo o que o usuario digitou (`/revisar src`);
//! o que muda e a mensagem que o modelo le. Assim o historico mostra o comando, e nao
//! um paragrafo que ninguem digitou.

use std::collections::BTreeMap;
use std::path::Path;

/// Teto do corpo de um comando: vai inteiro na mensagem do usuario a cada tarefa.
pub const CORPO_MAX: usize = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoDeBarra {
    pub nome: String,
    pub descricao: String,
    pub corpo: String,
    /// `projeto` ou `pacote:<nome>`.
    pub origem: String,
}

#[derive(Debug, Clone, Default)]
pub struct ComandosDeBarra {
    itens: BTreeMap<String, ComandoDeBarra>,
}

/// Nome de comando: o mesmo alfabeto das skills (ASCII, digitos, `-`, `_`, ate 64).
pub fn nome_valido(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 64
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

impl ComandosDeBarra {
    pub fn len(&self) -> usize {
        self.itens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.itens.is_empty()
    }

    pub fn nomes(&self) -> Vec<&str> {
        self.itens.keys().map(String::as_str).collect()
    }

    pub fn achar(&self, nome: &str) -> Option<&ComandoDeBarra> {
        self.itens.get(nome)
    }

    /// Soma um comando; nome repetido e recusado com quem ja o tem.
    pub fn somar(&mut self, c: ComandoDeBarra) -> Result<(), String> {
        if let Some(j) = self.itens.get(&c.nome) {
            return Err(format!(
                "comando /{} de {} ignorado: ja existe (de {})",
                c.nome, c.origem, j.origem
            ));
        }
        self.itens.insert(c.nome.clone(), c);
        Ok(())
    }

    /// Soma todos, devolvendo os avisos dos repetidos.
    pub fn somar_todos(&mut self, lista: Vec<ComandoDeBarra>) -> Vec<String> {
        lista
            .into_iter()
            .filter_map(|c| self.somar(c).err())
            .collect()
    }

    /// `/nome resto` -> o corpo com `$ARGUMENTS` trocado pelo resto. `None` quando o
    /// objetivo nao comeca por um comando conhecido: `/tmp/x` e caminho, nao comando.
    pub fn expandir(&self, objetivo: &str) -> Option<String> {
        let t = objetivo.trim_start();
        let sem_barra = t.strip_prefix('/')?;
        let (nome, resto) = match sem_barra.find(char::is_whitespace) {
            Some(i) => (&sem_barra[..i], sem_barra[i..].trim()),
            None => (sem_barra, ""),
        };
        let c = self.itens.get(nome)?;
        Some(if c.corpo.contains("$ARGUMENTS") {
            c.corpo.replace("$ARGUMENTS", resto)
        } else if resto.is_empty() {
            c.corpo.clone()
        } else {
            format!("{}\n\nArguments: {resto}", c.corpo.trim_end())
        })
    }

    /// O bloco do prompt de sistema: nome e descricao, para o modelo saber o que o
    /// usuario quis dizer com `/nome` e sugerir comandos existentes.
    pub fn bloco_para_o_prompt(&self) -> Option<String> {
        if self.itens.is_empty() {
            return None;
        }
        let mut s =
            String::from("Slash commands available to the user (already expanded when used):\n");
        for c in self.itens.values() {
            s.push_str(&format!("- /{}: {}\n", c.nome, c.descricao));
        }
        Some(s)
    }
}

/// Le `*.md` de uma pasta: o nome e o do arquivo, a descricao vem do cabecalho YAML
/// (`description:`) ou da primeira linha nao vazia, o corpo e o resto. Devolve tambem os
/// avisos (nome invalido, corpo grande demais, cabecalho ilegivel).
pub fn ler_pasta(dir: &Path, origem: &str) -> (Vec<ComandoDeBarra>, Vec<String>) {
    let mut lista = Vec::new();
    let mut avisos = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return (lista, avisos);
    };
    let mut entradas: Vec<_> = rd.flatten().collect();
    entradas.sort_by_key(|e| e.file_name());
    for e in entradas {
        let p = e.path();
        let Some(nome) = p
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".md"))
        else {
            continue;
        };
        if !nome_valido(nome) {
            avisos.push(format!("{origem}: comando {nome:?} com nome invalido"));
            continue;
        }
        let Ok(texto) = std::fs::read_to_string(&p) else {
            avisos.push(format!("{origem}: {} ilegivel", p.display()));
            continue;
        };
        match de_texto(nome, &texto, origem) {
            Ok(c) => lista.push(c),
            Err(m) => avisos.push(format!("{origem}: /{nome}: {m}")),
        }
    }
    (lista, avisos)
}

/// Um comando a partir do texto do `.md` (cabecalho YAML opcional).
pub fn de_texto(nome: &str, texto: &str, origem: &str) -> Result<ComandoDeBarra, String> {
    if !nome_valido(nome) {
        return Err("nome invalido".into());
    }
    let (cabecalho, corpo) = crate::importar_skills::separar(texto)?;
    let corpo = corpo.trim().to_string();
    if corpo.is_empty() {
        return Err("corpo vazio".into());
    }
    if corpo.len() > CORPO_MAX {
        return Err(format!("corpo acima de {CORPO_MAX} bytes"));
    }
    let descricao = cabecalho
        .map(crate::importar_skills::ler_cabecalho)
        .and_then(|m| match m.get("description") {
            Some(crate::importar_skills::Valor::Texto(t)) => Some(t.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            corpo
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim_start_matches('#')
                .trim()
                .chars()
                .take(160)
                .collect()
        });
    Ok(ComandoDeBarra {
        nome: nome.to_string(),
        descricao,
        corpo,
        origem: origem.to_string(),
    })
}

/// Os comandos do projeto: `<raiz>/.phxclaw/commands/*.md` (a pasta do operador, fora do
/// `work/` que o modelo enxerga -- a mesma regra dos hooks).
pub fn do_projeto() -> (Vec<ComandoDeBarra>, Vec<String>) {
    match crate::montagem::pasta_do_projeto() {
        Some(p) => ler_pasta(&p.join("commands"), "projeto"),
        None => (vec![], vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(nome: &str, corpo: &str) -> ComandoDeBarra {
        ComandoDeBarra {
            nome: nome.into(),
            descricao: "d".into(),
            corpo: corpo.into(),
            origem: "teste".into(),
        }
    }

    #[test]
    fn expande_so_comando_conhecido_e_troca_os_argumentos() {
        let mut c = ComandosDeBarra::default();
        c.somar(cmd("revisar", "Revise $ARGUMENTS com cuidado"))
            .unwrap();
        c.somar(cmd("oi", "diga oi")).unwrap();
        assert_eq!(
            c.expandir("/revisar src/main.rs").as_deref(),
            Some("Revise src/main.rs com cuidado")
        );
        assert_eq!(c.expandir("/oi").as_deref(), Some("diga oi"));
        assert_eq!(
            c.expandir("/oi para todos").as_deref(),
            Some("diga oi\n\nArguments: para todos")
        );
        assert_eq!(c.expandir("/tmp/x"), None, "caminho nao e comando");
        assert_eq!(c.expandir("revisar"), None);
        // repetido: o primeiro fica
        let e = c.somar(cmd("oi", "outro")).unwrap_err();
        assert!(e.contains("ja existe"), "{e}");
        assert_eq!(c.achar("oi").unwrap().corpo, "diga oi");
    }

    #[test]
    fn le_cabecalho_e_recusa_nome_invalido() {
        let c = de_texto("x", "---\ndescription: faz x\n---\ncorpo $ARGUMENTS\n", "p").unwrap();
        assert_eq!(
            (c.descricao.as_str(), c.corpo.as_str()),
            ("faz x", "corpo $ARGUMENTS")
        );
        let c = de_texto("y", "# Titulo\n\ncorpo\n", "p").unwrap();
        assert_eq!(c.descricao, "Titulo");
        assert!(de_texto("a b", "corpo", "p").is_err());
        assert!(de_texto("z", "   \n", "p").is_err());
    }
}
