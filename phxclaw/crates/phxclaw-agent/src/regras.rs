//! Regras por comando de shell: `permitir`, `negar` ou `perguntar`, no molde do
//! `execpolicy` do Codex (regra por prefixo de palavras, a decisao mais estrita vence).
//!
//! Tres decisoes que valem saber antes de mexer:
//!
//! - **A regra nao e a fronteira; o sandbox e.** Shell e linguagem inteira, e nenhum
//!   casador de texto a fecha. A regra existe para o operador dizer «isto nao» ou «isto so
//!   com alguem olhando» sobre o que o sandbox deixaria rodar. Por isso a leitura e
//!   conservadora: o que nao se analisa cai em `perguntar`, nunca em `permitir`.
//! - **Cada pedaco da linha se confere sozinho.** `ls && git push` sao dois comandos; um
//!   casador so do comeco da linha deixaria o `git push` passar escondido atras do `ls`.
//!   Pelo mesmo motivo `$(...)`, crase, `sh -c '...'` e `env`/`sudo` na frente sao abertos.
//! - **Sem arquivo de regras, nada muda.** O padrao e `permitir`, que e o comportamento de
//!   antes desta guarda: regra nova entra pedida, nao imposta.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decisao {
    // A ordem e a da severidade: `max` escolhe a mais estrita.
    Permitir,
    Perguntar,
    Negar,
}

#[derive(Debug, Clone, Deserialize)]
struct RegraBruta {
    padrao: String,
    decisao: Decisao,
    #[serde(default)]
    motivo: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArquivoBruto {
    #[serde(default = "permitir")]
    padrao: Decisao,
    #[serde(default)]
    regras: Vec<RegraBruta>,
}

fn permitir() -> Decisao {
    Decisao::Permitir
}

#[derive(Debug, Clone, PartialEq)]
pub struct Regra {
    /// Palavras do prefixo; `*` casa uma palavra qualquer.
    pub palavras: Vec<String>,
    pub decisao: Decisao,
    pub motivo: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegrasDeComando {
    pub padrao: Decisao,
    pub regras: Vec<Regra>,
}

/// O veredito de uma linha: a decisao e o que a causou, para a recusa dizer qual regra.
#[derive(Debug, Clone, PartialEq)]
pub struct Veredito {
    pub decisao: Decisao,
    pub explicacao: String,
}

/// Prefixos que nao sao o comando, so o embrulham: a regra confere o que vem depois.
const EMBRULHOS: &[&str] = &[
    "sudo", "env", "nohup", "time", "nice", "command", "exec", "builtin", "doas", "timeout",
    "stdbuf", "xargs",
];

impl RegrasDeComando {
    /// `{"padrao": "permitir", "regras": [{"padrao": "git push", "decisao": "perguntar"}]}`.
    pub fn de_json(texto: &str) -> Result<Self, String> {
        let b: ArquivoBruto = serde_json::from_str(texto).map_err(|e| e.to_string())?;
        let mut regras = Vec::new();
        for r in b.regras {
            let palavras = palavras_da_linha(&r.padrao)
                .ok_or_else(|| format!("padrao com aspas abertas: {}", r.padrao))?;
            if palavras.is_empty() {
                return Err("padrao vazio".into());
            }
            regras.push(Regra {
                palavras,
                decisao: r.decisao,
                motivo: r.motivo,
            });
        }
        Ok(Self {
            padrao: b.padrao,
            regras,
        })
    }

    pub fn carregar(arquivo: &Path) -> Result<Option<Self>, String> {
        match std::fs::read_to_string(arquivo) {
            Ok(t) => Self::de_json(&t)
                .map(Some)
                .map_err(|e| format!("{}: {e}", arquivo.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", arquivo.display())),
        }
    }

    /// Arquivo de regras que existe e nao se le: a falha fecha. O operador escreveu regra
    /// para restringir; ignora-la calado deixaria rodar justamente o que ele quis barrar.
    pub fn negar_tudo(motivo: &str) -> Self {
        Self {
            padrao: Decisao::Negar,
            regras: vec![Regra {
                palavras: vec!["*".into()],
                decisao: Decisao::Negar,
                motivo: Some(motivo.into()),
            }],
        }
    }

    pub fn avaliar(&self, linha: &str) -> Veredito {
        let Some(pedacos) = pedacos(linha, 0) else {
            let d = self.padrao.max(Decisao::Perguntar);
            return Veredito {
                decisao: d,
                explicacao: "linha que nao se analisa (aspas abertas ou aninhamento fundo)".into(),
            };
        };
        let mut pior = Veredito {
            decisao: Decisao::Permitir,
            explicacao: String::new(),
        };
        let mut algum = false;
        for p in pedacos {
            if p.is_empty() {
                continue;
            }
            algum = true;
            let v = self.avaliar_pedaco(&p);
            if v.decisao > pior.decisao || pior.explicacao.is_empty() {
                pior = v;
            }
        }
        if !algum {
            return Veredito {
                decisao: self.padrao,
                explicacao: "linha vazia: padrao".into(),
            };
        }
        pior
    }

    fn avaliar_pedaco(&self, palavras: &[String]) -> Veredito {
        let mut achada: Option<&Regra> = None;
        for r in &self.regras {
            if casa(&r.palavras, palavras) && achada.is_none_or(|a| r.decisao > a.decisao) {
                achada = Some(r);
            }
        }
        match achada {
            Some(r) => Veredito {
                decisao: r.decisao,
                explicacao: format!(
                    "regra '{}'{}",
                    r.palavras.join(" "),
                    r.motivo
                        .as_deref()
                        .map(|m| format!(": {m}"))
                        .unwrap_or_default()
                ),
            },
            None => Veredito {
                decisao: self.padrao,
                explicacao: format!("nenhuma regra para '{}': padrao", palavras.join(" ")),
            },
        }
    }
}

fn casa(regra: &[String], palavras: &[String]) -> bool {
    regra.len() <= palavras.len()
        && regra.iter().zip(palavras).enumerate().all(|(i, (r, p))| {
            r == "*" || r == p || (i == 0 && p.rsplit('/').next() == Some(r.as_str()))
        })
}

/// Palavras de uma linha simples (aspas tiradas), ou `None` com aspas abertas.
fn palavras_da_linha(s: &str) -> Option<Vec<String>> {
    let p = pedacos(s, 0)?;
    Some(p.into_iter().flatten().collect())
}

/// Parte a linha em comandos: separadores de controle (`;`, `&&`, `||`, `|`, `&`, quebra
/// de linha, parenteses) e substituicao de comando (`$(`, crase) viram fronteira. Aspas
/// simples sao literais; dentro de aspas duplas `$(` e crase continuam abrindo comando,
/// como no shell. `sh -c '...'` e `eval` tem o argumento aberto de novo.
fn pedacos(s: &str, fundo: u32) -> Option<Vec<Vec<String>>> {
    if fundo > 4 {
        return None;
    }
    let mut saida: Vec<Vec<String>> = Vec::new();
    let mut atual: Vec<String> = Vec::new();
    let mut palavra = String::new();
    let mut tem_palavra = false;
    let mut aninhados: Vec<String> = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let fecha_palavra = |palavra: &mut String, tem: &mut bool, atual: &mut Vec<String>| {
        if *tem {
            atual.push(std::mem::take(palavra));
            *tem = false;
        }
    };
    while i < c.len() {
        let ch = c[i];
        match ch {
            '\'' => {
                let fim = c[i + 1..].iter().position(|&x| x == '\'')? + i + 1;
                palavra.extend(&c[i + 1..fim]);
                tem_palavra = true;
                i = fim + 1;
                continue;
            }
            '"' => {
                let mut j = i + 1;
                loop {
                    let x = *c.get(j)?;
                    if x == '\\' {
                        if let Some(n) = c.get(j + 1) {
                            palavra.push(*n);
                        }
                        j += 2;
                        continue;
                    }
                    if x == '"' {
                        break;
                    }
                    if x == '`' || (x == '$' && c.get(j + 1) == Some(&'(')) {
                        let (dentro, fim) = substituicao(&c, j)?;
                        aninhados.push(dentro);
                        j = fim;
                        continue;
                    }
                    palavra.push(x);
                    j += 1;
                }
                tem_palavra = true;
                i = j + 1;
                continue;
            }
            '\\' => {
                if let Some(n) = c.get(i + 1) {
                    palavra.push(*n);
                    tem_palavra = true;
                }
                i += 2;
                continue;
            }
            '`' => {
                let (dentro, fim) = substituicao(&c, i)?;
                aninhados.push(dentro);
                i = fim;
                continue;
            }
            '$' if c.get(i + 1) == Some(&'(') => {
                let (dentro, fim) = substituicao(&c, i)?;
                aninhados.push(dentro);
                i = fim;
                continue;
            }
            ';' | '&' | '|' | '\n' | '(' | ')' | '{' | '}' => {
                fecha_palavra(&mut palavra, &mut tem_palavra, &mut atual);
                saida.push(std::mem::take(&mut atual));
            }
            x if x.is_whitespace() => fecha_palavra(&mut palavra, &mut tem_palavra, &mut atual),
            x => {
                palavra.push(x);
                tem_palavra = true;
            }
        }
        i += 1;
    }
    fecha_palavra(&mut palavra, &mut tem_palavra, &mut atual);
    saida.push(atual);
    let mut final_: Vec<Vec<String>> = Vec::new();
    for p in saida {
        let p = sem_embrulho(p);
        // `sh -c 'linha'`, `bash -c`, `eval 'linha'`: o texto e outro comando inteiro.
        let interno = match p.first().map(|x| x.rsplit('/').next().unwrap_or(x)) {
            Some("sh" | "bash" | "dash" | "zsh" | "ksh") => p
                .iter()
                .position(|x| x == "-c" || (x.starts_with('-') && x.ends_with('c')))
                .and_then(|k| p.get(k + 1))
                .cloned(),
            Some("eval") => Some(p[1..].join(" ")),
            _ => None,
        };
        if let Some(linha) = interno {
            final_.extend(pedacos(&linha, fundo + 1)?);
        }
        final_.push(p);
    }
    for a in aninhados {
        final_.extend(pedacos(&a, fundo + 1)?);
    }
    Some(final_)
}

/// `$(...)` ou crase a partir de `i`: o texto de dentro e o indice depois do fecho.
fn substituicao(c: &[char], i: usize) -> Option<(String, usize)> {
    if c[i] == '`' {
        let fim = c[i + 1..].iter().position(|&x| x == '`')? + i + 1;
        return Some((c[i + 1..fim].iter().collect(), fim + 1));
    }
    let mut nivel = 0i32;
    let mut j = i + 1;
    while j < c.len() {
        match c[j] {
            '(' => nivel += 1,
            ')' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some((c[i + 2..j].iter().collect(), j + 1));
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

fn sem_embrulho(mut p: Vec<String>) -> Vec<String> {
    loop {
        let Some(primeira) = p.first().cloned() else {
            return p;
        };
        let base = primeira.rsplit('/').next().unwrap_or(&primeira).to_string();
        let base = base.as_str();
        let atribuicao = primeira.contains('=')
            && primeira.split('=').next().is_some_and(|n| {
                !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_')
            });
        if atribuicao {
            p.remove(0);
        } else if EMBRULHOS.contains(&base) {
            let com_valor = opcoes_com_valor(base);
            p.remove(0);
            // Opcoes do embrulho (`sudo -u x`, `timeout 5`, `nice -n 5`) tambem saem: o
            // comando de verdade e a primeira palavra que nao e opcao, valor de opcao nem
            // duracao.
            while let Some(x) = p.first().cloned() {
                if com_valor.contains(&x.as_str()) {
                    p.drain(..2.min(p.len()));
                } else if x.starts_with('-') || (base == "timeout" && duracao(&x)) {
                    p.remove(0);
                } else {
                    break;
                }
            }
        } else {
            return p;
        }
    }
}

/// Opcoes de embrulho que levam o valor na palavra seguinte: sem isto, `sudo -u root rm`
/// tomaria `root` pelo comando e o `rm` passaria sem regra.
fn opcoes_com_valor(embrulho: &str) -> &'static [&'static str] {
    match embrulho {
        "sudo" | "doas" => &[
            "-u", "-g", "-C", "-D", "-p", "-h", "-U", "--user", "--group",
        ],
        "env" => &["-u", "-C", "-S", "--unset", "--chdir"],
        "nice" => &["-n", "--adjustment"],
        "timeout" => &["-s", "-k", "--signal", "--kill-after"],
        "xargs" => &["-n", "-I", "-P", "-L", "-d", "-s", "-E", "-a"],
        _ => &[],
    }
}

fn duracao(x: &str) -> bool {
    let n = x.trim_end_matches(['s', 'm', 'h', 'd']);
    !n.is_empty() && n.chars().all(|c| c.is_ascii_digit() || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> RegrasDeComando {
        RegrasDeComando::de_json(
            r#"{"padrao":"permitir","regras":[
                {"padrao":"git push","decisao":"perguntar"},
                {"padrao":"rm -rf","decisao":"negar","motivo":"apagar arvore"},
                {"padrao":"curl","decisao":"negar"},
                {"padrao":"git *","decisao":"permitir"}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn decisao_mais_estrita_vence_entre_pedacos_e_regras() {
        let r = r();
        assert_eq!(r.avaliar("ls -la").decisao, Decisao::Permitir);
        assert_eq!(r.avaliar("git status").decisao, Decisao::Permitir);
        assert_eq!(
            r.avaliar("git push origin main").decisao,
            Decisao::Perguntar
        );
        let v = r.avaliar("ls && rm -rf /work/x");
        assert_eq!(v.decisao, Decisao::Negar);
        assert!(v.explicacao.contains("apagar arvore"), "{v:?}");
    }

    #[test]
    fn embrulho_e_substituicao_nao_escondem_o_comando() {
        let r = r();
        for linha in [
            "echo $(curl http://x)",
            "echo \"$(curl x)\"",
            "echo `curl x`",
            "sudo -u root rm -rf /",
            "FOO=1 env curl x",
            "sh -c 'curl x'",
            "bash -lc \"git push\"",
            "/usr/bin/curl x",
            "timeout 5 curl x",
            "(cd a; curl x)",
            "true | curl x",
        ] {
            assert_ne!(r.avaliar(linha).decisao, Decisao::Permitir, "{linha}");
        }
        assert_eq!(r.avaliar("echo 'curl x'").decisao, Decisao::Permitir);
    }

    #[test]
    fn linha_ilegivel_pergunta_e_arquivo_vazio_permite() {
        assert_eq!(r().avaliar("echo 'aberta").decisao, Decisao::Perguntar);
        let vazio = RegrasDeComando::de_json("{}").unwrap();
        assert_eq!(vazio.avaliar("rm -rf /").decisao, Decisao::Permitir);
        assert!(
            RegrasDeComando::de_json(r#"{"regras":[{"padrao":"","decisao":"negar"}]}"#).is_err()
        );
        assert_eq!(
            RegrasDeComando::negar_tudo("x").avaliar("ls").decisao,
            Decisao::Negar
        );
    }
}
