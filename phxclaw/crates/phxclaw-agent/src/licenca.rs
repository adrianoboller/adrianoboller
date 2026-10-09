//! Porta de licenca: a UNICA resposta para «este conteudo de terceiro pode ser COPIADO para
//! dentro do PhxClaw (Apache-2.0)?». Quem copia texto alheio para a arvore do agente --
//! `importar_skills` (e por ele as skills dos pacotes) e o `phxclaw licenca conferir` que os
//! importadores de fora chamam -- pergunta aqui, e so aqui: duas tabelas de licenca
//! divergiriam no dia em que uma aprendesse um nome que a outra nao sabe.
//!
//! As decisoes:
//!
//! - **Identifica por SPDX quando ha; senao pela ASSINATURA do texto**, as frases canonicas
//!   do cabecalho de cada licenca. Nunca pelo nome do arquivo: `LICENSE-MIT` com texto que
//!   nao se reconhece e licenca desconhecida, nao MIT.
//! - **Sobe da pasta importada ate a raiz do repositorio de origem** (a pasta com `.git`;
//!   sem `.git`, a propria origem). A licenca de um checkout mora na raiz, e a skill mora
//!   tres pastas abaixo: olhar so a pasta da skill deixaria o OpenMontage (AGPL, 157 skills)
//!   passar como «sem licenca» -- e a opcao de aceitar desconhecida o importaria. Nao sobe
//!   alem: um `LICENSE` em `/home` nao diz nada sobre o checkout.
//! - **Vence a declaracao MAIS RESTRITIVA do caminho** (copyleft > desconhecida > compativel),
//!   e nao a mais proxima. O REUSE deixa o arquivo sobrepor a raiz, e isso serve a quem
//!   declara com cuidado; aqui a porta existe para o descuido -- um `license: MIT` copiado de
//!   modelo para dentro de um repositorio AGPL nao pode abrir a porta. O preco: skill
//!   genuinamente MIT dentro de repositorio copyleft fica de fora, e a recusa diz os dois.
//! - **Expressao SPDX se avalia**: `OR` e escolha do licenciado (vale a melhor opcao), `AND`
//!   soma obrigacoes (vale a pior), `WITH` herda a classe da licenca base. Achatar os
//!   parenteses errava para o lado perigoso: `GPL-3.0 AND (MIT OR Apache-2.0)` virava
//!   «Apache-2.0 sozinha».
//! - **Copyleft nao tem opcao.** Desconhecida tem uma, explicita, que grava quem decidiu e
//!   quando. E nao existe opcao de remover aviso: compativel ENTRA COM o texto da licenca,
//!   o `NOTICE` (obrigacao da Apache-2.0, sec. 4d) e as linhas de copyright, no
//!   `LICENCA.txt` ao lado do que se importou.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// Onde o aviso de terceiro fica, ao lado do que se importou.
pub const ARQUIVO_AVISO: &str = "LICENCA.txt";

/// A opcao que aceita licenca desconhecida, com o nome que o operador digita.
pub const OPCAO_ACEITAR: &str = "--aceitar-licenca-desconhecida";

/// As que se combinam com Apache-2.0 sem mudar a licenca da obra: so pedem o aviso.
pub const COMPATIVEIS: &[&str] = &[
    "MIT",
    "MIT-0",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "0BSD",
    "Unlicense",
    "CC0-1.0",
    "Zlib",
];

/// Familias copyleft, pelo prefixo do identificador SPDX. `GPL` nao casa `LGPL` porque o
/// prefixo se confere no comeco do identificador.
const PREFIXOS_COPYLEFT: &[&str] = &[
    "AGPL", "LGPL", "GPL", "MPL", "EPL", "CC-BY-SA", "EUPL", "CDDL", "OSL", "CPL",
];

/// Teto de leitura de um arquivo de licenca: a GPL-3.0 inteira tem ~35 KiB.
const TETO_ARQUIVO: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Classe {
    Compativel,
    Desconhecida,
    Copyleft,
}

impl Classe {
    pub fn nome(self) -> &'static str {
        match self {
            Classe::Compativel => "compativel",
            Classe::Desconhecida => "desconhecida",
            Classe::Copyleft => "copyleft",
        }
    }
}

/// Uma licenca achada no caminho.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Declaracao {
    /// Arquivo (e campo, `arquivo#license`) onde se achou.
    pub onde: String,
    /// `spdx`, `texto` (assinatura), `cabecalho` (front-matter) ou `manifesto` (plugin.json).
    pub como: &'static str,
    /// O identificador ou a expressao reconhecida; o valor cru quando nao se reconhece.
    pub licenca: String,
    /// `None`: o valor so remete a um arquivo de licenca («terms in LICENSE.txt»), que
    /// entra pela propria declaracao.
    pub classe: Option<Classe>,
}

/// A decisao do operador que aceitou licenca desconhecida, gravada junto do importado.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DecisaoDoOperador {
    pub opcao: String,
    pub operador: String,
    pub quando: String,
    /// A recusa que a opcao atravessou, para quem audita saber o que foi aceito.
    pub recusa_atravessada: String,
}

/// O resultado da porta sobre uma pasta.
#[derive(Debug, Clone, Serialize)]
pub struct Conferencia {
    pub alvo: PathBuf,
    pub limite: PathBuf,
    pub classe: Classe,
    pub declaracoes: Vec<Declaracao>,
    /// Textos de licenca e `NOTICE` achados no caminho: o que vai junto do importado.
    pub textos: Vec<PathBuf>,
    pub copyright: Vec<String>,
}

/// O que fica gravado (no `ORIGEM.json` da skill, por exemplo).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Registro {
    pub classe: Classe,
    pub licencas: Vec<String>,
    pub declaracoes: Vec<Declaracao>,
    pub textos_copiados: Vec<String>,
    pub copyright: Vec<String>,
    pub decisao_do_operador: Option<DecisaoDoOperador>,
}

// ---------------------------------------------------------------- identificador SPDX

/// Classe de UM identificador SPDX (sem operador).
pub fn classe_do_id(id: &str) -> Classe {
    let u = id.trim().to_ascii_uppercase();
    if COMPATIVEIS.iter().any(|c| c.eq_ignore_ascii_case(&u)) {
        return Classe::Compativel;
    }
    if u.ends_with("-OR-LATER") || u.ends_with('+') {
        return Classe::Copyleft;
    }
    let copyleft = PREFIXOS_COPYLEFT.iter().any(|p| {
        u.strip_prefix(p).is_some_and(|resto| {
            resto
                .chars()
                .next()
                .is_none_or(|c| !c.is_ascii_alphabetic() || c == 'V')
        })
    });
    if copyleft {
        Classe::Copyleft
    } else {
        Classe::Desconhecida
    }
}

fn fichas(expr: &str) -> Vec<String> {
    expr.replace('(', " ( ")
        .replace(')', " ) ")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

struct Leitor {
    t: Vec<String>,
    i: usize,
}

impl Leitor {
    fn olhar(&self) -> Option<&str> {
        self.t.get(self.i).map(String::as_str)
    }
    fn operador(&self, op: &str) -> bool {
        self.olhar().is_some_and(|f| f.eq_ignore_ascii_case(op))
    }
    // ou := e (OR e)*  -- o licenciado escolhe: vale a MELHOR alternativa.
    fn ou(&mut self) -> Option<Classe> {
        let mut c = self.e()?;
        while self.operador("OR") {
            self.i += 1;
            c = c.min(self.e()?);
        }
        Some(c)
    }
    // e := com (AND com)*  -- obrigacoes somadas: vale a PIOR.
    fn e(&mut self) -> Option<Classe> {
        let mut c = self.com()?;
        while self.operador("AND") {
            self.i += 1;
            c = c.max(self.com()?);
        }
        Some(c)
    }
    // com := atomo (WITH excecao)?  -- a excecao nao tira o copyleft da base.
    fn com(&mut self) -> Option<Classe> {
        let c = self.atomo()?;
        if self.operador("WITH") {
            self.i += 1;
            let f = self.olhar()?;
            if !id_valido(f) {
                return None;
            }
            self.i += 1;
        }
        Some(c)
    }
    fn atomo(&mut self) -> Option<Classe> {
        let f = self.olhar()?.to_string();
        self.i += 1;
        if f == "(" {
            let c = self.ou()?;
            if self.olhar() != Some(")") {
                return None;
            }
            self.i += 1;
            return Some(c);
        }
        if !id_valido(&f)
            || ["AND", "OR", "WITH"]
                .iter()
                .any(|o| f.eq_ignore_ascii_case(o))
        {
            return None;
        }
        Some(classe_do_id(&f))
    }
}

fn id_valido(f: &str) -> bool {
    !f.is_empty()
        && f.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-.+:".contains(c))
}

/// Avalia uma expressao SPDX (`MIT OR Apache-2.0`, `GPL-2.0 WITH Classpath-exception-2.0`).
/// `None` quando o texto nao e expressao SPDX (prosa como «MIT License»).
pub fn classe_da_expressao(expr: &str) -> Option<Classe> {
    let mut l = Leitor {
        t: fichas(expr),
        i: 0,
    };
    if l.t.is_empty() {
        return None;
    }
    let c = l.ou()?;
    (l.i == l.t.len()).then_some(c)
}

// ---------------------------------------------------------------- assinatura do texto

fn normalizar(t: &str) -> String {
    t.to_lowercase()
        .chars()
        .map(|c| match c {
            '*' | '#' | '>' | '`' | '_' | '|' => ' ',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            c => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A licenca de um texto pelas frases canonicas do proprio texto (cabecalho ou aviso curto).
/// As copyleft se procuram ANTES: o texto da AGPL e o da LGPL citam a «GNU General Public
/// License», e o da MPL-2.0 tambem -- a ordem e o que da o nome certo.
pub fn identificar_texto(bruto: &str) -> Option<&'static str> {
    let t = normalizar(bruto);
    let tem = |s: &str| t.contains(s);
    if tem("gnu affero general public license") {
        return Some(if tem("version 3") {
            "AGPL-3.0"
        } else {
            "AGPL-1.0"
        });
    }
    if tem("gnu lesser general public license") || tem("gnu library general public license") {
        return Some(if tem("version 3") {
            "LGPL-3.0"
        } else if tem("version 2.1") {
            "LGPL-2.1"
        } else {
            "LGPL-2.0"
        });
    }
    if tem("mozilla public license") {
        return Some(if tem("version 2.0") {
            "MPL-2.0"
        } else {
            "MPL-1.1"
        });
    }
    if tem("eclipse public license") {
        return Some(if tem("version 2.0") || tem("v 2.0") {
            "EPL-2.0"
        } else {
            "EPL-1.0"
        });
    }
    if tem("attribution-sharealike") || tem("attribution share alike") {
        return Some("CC-BY-SA-4.0");
    }
    if tem("gnu general public license") {
        return Some(if tem("version 3") {
            "GPL-3.0"
        } else if tem("version 2") {
            "GPL-2.0"
        } else {
            "GPL-1.0"
        });
    }
    if tem("apache license") && tem("version 2.0") {
        return Some("Apache-2.0");
    }
    if tem("permission is hereby granted, free of charge, to any person obtaining a copy") {
        return Some(
            if tem("the above copyright notice and this permission notice shall be included") {
                "MIT"
            } else {
                "MIT-0"
            },
        );
    }
    if tem("distribute this software for any purpose with or without fee is hereby granted") {
        return Some(
            if tem("provided that the above copyright notice and this permission notice appear") {
                "ISC"
            } else {
                "0BSD"
            },
        );
    }
    if tem(
        "redistribution and use in source and binary forms, with or without modification, are permitted",
    ) {
        return Some(if tem("all advertising materials mentioning") {
            "BSD-4-Clause"
        } else if tem("neither the name of") || tem("may not be used to endorse or promote") {
            "BSD-3-Clause"
        } else {
            "BSD-2-Clause"
        });
    }
    if tem("this is free and unencumbered software released into the public domain") {
        return Some("Unlicense");
    }
    if tem("cc0 1.0 universal") || (tem("creative commons") && tem("cc0")) {
        return Some("CC0-1.0");
    }
    if tem(
        "altered source versions must be plainly marked as such, and must not be misrepresented as being the original software",
    ) {
        return Some("Zlib");
    }
    None
}

/// As linhas `SPDX-License-Identifier:` de um texto, com o comentario em volta tirado.
pub fn spdx_no_texto(texto: &str) -> Vec<String> {
    texto
        .lines()
        .filter_map(|l| l.split_once("SPDX-License-Identifier:").map(|(_, v)| v))
        .map(|v| {
            v.trim()
                .trim_end_matches("-->")
                .trim_end_matches("*/")
                .trim()
                .to_string()
        })
        .filter(|v| !v.is_empty())
        .collect()
}

/// Declaracoes `SPDX-License-Identifier` de um documento (o proprio `SKILL.md`).
pub fn declaracoes_spdx(texto: &str, onde: &str) -> Vec<Declaracao> {
    spdx_no_texto(texto)
        .into_iter()
        .map(|e| Declaracao {
            onde: onde.to_string(),
            como: "spdx",
            classe: Some(classe_da_expressao(&e).unwrap_or(Classe::Desconhecida)),
            licenca: e,
        })
        .collect()
}

fn palavras(t: &str) -> Vec<String> {
    t.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Um valor de licenca declarado por campo (`license:` do cabecalho, `license` do
/// `plugin.json`): expressao SPDX quando for; senao prosa, lida pelo lado seguro -- dica de
/// copyleft basta para recusar, mas compativel so pelos nomes inteiros que nao deixam
/// duvida («MIT License», «Apache 2.0»).
pub fn declaracao_de_valor(valor: &str, onde: &str, como: &'static str) -> Declaracao {
    let valor = valor.trim();
    let mut d = Declaracao {
        onde: onde.to_string(),
        como,
        licenca: valor.to_string(),
        classe: None,
    };
    // Expressao SPDX que decide (compativel ou copyleft) vale como esta; palavra solta que
    // nao e identificador conhecido («Affero», «Proprietaria») ainda passa pela prosa.
    if let Some(c @ (Classe::Compativel | Classe::Copyleft)) = classe_da_expressao(valor) {
        d.classe = Some(c);
        return d;
    }
    let p = palavras(valor);
    let comeca = |pref: &str| p.iter().any(|w| w.starts_with(pref));
    let frase = |f: &str| valor.to_lowercase().contains(f);
    let copyleft = if comeca("agpl") || frase("affero") {
        Some("AGPL")
    } else if comeca("lgpl") || frase("lesser general public") || frase("library general public") {
        Some("LGPL")
    } else if comeca("mpl") || frase("mozilla public") {
        Some("MPL")
    } else if comeca("epl") || frase("eclipse public") {
        Some("EPL")
    } else if frase("sharealike") || frase("share-alike") || frase("by-sa") {
        Some("CC-BY-SA")
    } else if comeca("gpl") || frase("general public license") || frase("copyleft") {
        Some("GPL")
    } else {
        None
    };
    if let Some(c) = copyleft {
        d.licenca = format!("{c} ({valor})");
        d.classe = Some(Classe::Copyleft);
        return d;
    }
    let miolo: Vec<&str> = p
        .iter()
        .map(String::as_str)
        .filter(|w| !["license", "licence", "the", "version", "v"].contains(w))
        .collect();
    let compativel = match miolo.as_slice() {
        ["mit"] | ["expat"] => Some("MIT"),
        ["apache", "2"] | ["apache", "2", "0"] | ["apache2"] => Some("Apache-2.0"),
        ["bsd", "2", "clause"] => Some("BSD-2-Clause"),
        ["bsd", "3", "clause"] => Some("BSD-3-Clause"),
        ["isc"] => Some("ISC"),
        ["unlicense"] => Some("Unlicense"),
        ["cc0"] | ["cc0", "1", "0"] => Some("CC0-1.0"),
        ["zlib"] => Some("Zlib"),
        _ => None,
    };
    if let Some(id) = compativel {
        d.licenca = id.to_string();
        d.classe = Some(Classe::Compativel);
    } else if !(p
        .iter()
        .any(|w| ["license", "licence", "copying"].contains(&w.as_str())))
    {
        // Nem SPDX, nem nome inteiro, nem remissao a um arquivo: nao se reconhece.
        d.classe = Some(Classe::Desconhecida);
    }
    d
}

// ---------------------------------------------------------------- a pasta

/// O teto da subida: a raiz do repositorio (a pasta com `.git`) que contem `alvo`; sem
/// `.git`, o proprio `alvo`.
pub fn limite_padrao(alvo: &Path) -> PathBuf {
    let c = std::fs::canonicalize(alvo).unwrap_or_else(|_| alvo.to_path_buf());
    c.ancestors()
        .find(|a| a.join(".git").exists())
        .map(Path::to_path_buf)
        .unwrap_or(c)
}

const EXTENSOES_DE_CODIGO: &[&str] = &[
    "rs", "py", "js", "mjs", "ts", "json", "go", "java", "c", "h", "sh", "toml", "yaml", "yml",
    "html", "css",
];

/// `LICENSE`, `LICENCE.md`, `COPYING.LESSER`, `LICENSE-MIT`, `UNLICENSE`... -- o nome so
/// diz QUE o arquivo e de licenca; QUAL licenca, quem diz e o texto.
fn e_arquivo_de_licenca(nome: &str) -> bool {
    let n = nome.to_ascii_lowercase();
    let ext = n.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    if EXTENSOES_DE_CODIGO.contains(&ext) {
        return false;
    }
    ["license", "licence", "copying", "unlicense"]
        .iter()
        .any(|s| {
            n.strip_prefix(s)
                .is_some_and(|r| r.is_empty() || r.starts_with(['.', '-', '_']))
        })
}

fn e_notice(nome: &str) -> bool {
    let n = nome.to_ascii_lowercase();
    n == "notice" || n.starts_with("notice.")
}

fn ler_limitado(p: &Path) -> Result<Vec<u8>, String> {
    let m = std::fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if m.len() > TETO_ARQUIVO {
        return Err(format!(
            "{}: {} bytes, acima do teto de {} KiB para arquivo de licenca",
            p.display(),
            m.len(),
            TETO_ARQUIVO / 1024
        ));
    }
    std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// A licenca de um arquivo de licenca: SPDX se o arquivo trouxer, senao a assinatura.
fn declaracao_de_arquivo(p: &Path) -> Declaracao {
    let onde = p.display().to_string();
    let texto = match ler_limitado(p) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => {
            return Declaracao {
                onde,
                como: "texto",
                licenca: e,
                classe: Some(Classe::Desconhecida),
            };
        }
    };
    if let Some(d) = declaracoes_spdx(&texto, &onde)
        .into_iter()
        .max_by_key(|d| d.classe)
    {
        return d;
    }
    match identificar_texto(&texto) {
        Some(id) => Declaracao {
            onde,
            como: "texto",
            licenca: id.to_string(),
            classe: Some(classe_do_id(id)),
        },
        None => Declaracao {
            onde,
            como: "texto",
            licenca: "texto nao reconhecido".into(),
            classe: Some(Classe::Desconhecida),
        },
    }
}

/// Linha que E um aviso de copyright, e nao prosa da licenca sobre copyright: depois de
/// «copyright» vem `(c)`, `©` ou o ano. Medido no `LICENSE` da propria casa (Apache-2.0):
/// sem essa conferencia, «copyright license to reproduce...» e «(c) You must retain...» do
/// corpo da licenca saiam como titulares.
fn e_aviso_de_copyright(minusc: &str) -> bool {
    let ano_ou_c = |r: &str| {
        let r = r.trim_start();
        r.starts_with("(c)")
            || r.starts_with('\u{a9}')
            || r.starts_with(|c: char| c.is_ascii_digit())
    };
    if let Some(r) = minusc.strip_prefix("copyright") {
        return ano_ou_c(r);
    }
    if let Some(r) = minusc
        .strip_prefix("(c)")
        .or_else(|| minusc.strip_prefix('\u{a9}'))
    {
        return r.trim_start().starts_with(|c: char| c.is_ascii_digit());
    }
    false
}

fn linhas_de_copyright(texto: &str, v: &mut Vec<String>) {
    for l in texto.lines() {
        let l = l
            .trim()
            .trim_start_matches(['#', '*', '/', '-', '!', '<', ' '])
            .trim();
        let b = l.to_ascii_lowercase();
        let modelo = [
            "<year>",
            "[yyyy]",
            "{yyyy}",
            "[year]",
            "<copyright holders>",
        ]
        .iter()
        .any(|m| b.contains(m));
        if e_aviso_de_copyright(&b) && !modelo && !v.iter().any(|x| x == l) && v.len() < 50 {
            v.push(l.to_string());
        }
    }
}

/// Confere a licenca de `alvo` (pasta ou arquivo) subindo ate `limite` (`None`:
/// `limite_padrao(alvo)`). `extras`: o que o proprio documento declara (cabecalho, SPDX).
pub fn conferir(alvo: &Path, limite: Option<&Path>, extras: Vec<Declaracao>) -> Conferencia {
    let canon = std::fs::canonicalize(alvo).unwrap_or_else(|_| alvo.to_path_buf());
    let pasta = if canon.is_file() {
        canon
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(canon.clone())
    } else {
        canon.clone()
    };
    let limite = match limite {
        Some(l) => std::fs::canonicalize(l).unwrap_or_else(|_| l.to_path_buf()),
        None => limite_padrao(&pasta),
    };
    let mut c = Conferencia {
        alvo: canon,
        limite: limite.clone(),
        classe: Classe::Desconhecida,
        declaracoes: extras,
        textos: Vec::new(),
        copyright: Vec::new(),
    };
    // Limite que nao e ancestral (pasta fora dele): so a propria pasta.
    let caminho: Vec<&Path> = if pasta.starts_with(&limite) {
        pasta
            .ancestors()
            .take_while(|a| a.starts_with(&limite))
            .collect()
    } else {
        vec![pasta.as_path()]
    };
    for dir in caminho {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut es: Vec<_> = rd.flatten().collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let nome = e.file_name().to_string_lossy().into_owned();
            let p = e.path();
            // Atalho nao se segue: a licenca que vale e a que esta na arvore.
            let Ok(m) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if !m.is_file() {
                continue;
            }
            if e_arquivo_de_licenca(&nome) {
                c.declaracoes.push(declaracao_de_arquivo(&p));
                c.textos.push(p);
            } else if e_notice(&nome) {
                c.textos.push(p);
            }
        }
        for manifesto in [".claude-plugin/plugin.json", ".codex-plugin/plugin.json"] {
            let arq = dir.join(manifesto);
            let Ok(t) = std::fs::read_to_string(&arq) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) else {
                continue;
            };
            if let Some(l) = v.get("license").and_then(serde_json::Value::as_str) {
                c.declaracoes.push(declaracao_de_valor(
                    l,
                    &format!("{}#license", arq.display()),
                    "manifesto",
                ));
            }
        }
    }
    for p in &c.textos {
        if let Ok(b) = ler_limitado(p) {
            linhas_de_copyright(&String::from_utf8_lossy(&b), &mut c.copyright);
        }
    }
    // Remissao («terms in LICENSE.txt») sem arquivo de licenca no caminho nao se sustenta.
    let tem_arquivo = c.declaracoes.iter().any(|d| d.como == "texto");
    for d in &mut c.declaracoes {
        if d.classe.is_none() && !tem_arquivo {
            d.classe = Some(Classe::Desconhecida);
        }
    }
    c.classe = c
        .declaracoes
        .iter()
        .filter_map(|d| d.classe)
        .max()
        .unwrap_or(Classe::Desconhecida);
    c
}

impl Conferencia {
    /// As declaracoes que decidiram a classe.
    pub fn decisivas(&self) -> Vec<&Declaracao> {
        self.declaracoes
            .iter()
            .filter(|d| d.classe == Some(self.classe))
            .collect()
    }

    pub fn licencas(&self) -> Vec<String> {
        let mut v: Vec<String> = self.decisivas().iter().map(|d| d.licenca.clone()).collect();
        v.dedup();
        v
    }

    /// O porque, em uma frase, com a licenca e o lugar.
    pub fn motivo(&self) -> String {
        let onde = |ds: Vec<&Declaracao>| {
            ds.iter()
                .map(|d| format!("{} ({}, {})", d.licenca, d.como, d.onde))
                .collect::<Vec<_>>()
                .join("; ")
        };
        match self.classe {
            Classe::Compativel => format!(
                "licenca compativel com Apache-2.0: {}; entra com o aviso em {ARQUIVO_AVISO}",
                onde(self.decisivas())
            ),
            Classe::Copyleft => format!(
                "licenca copyleft: {}. Copiar este texto para dentro do PhxClaw (Apache-2.0) faria \
a obra combinada herdar o copyleft; recusado, e nenhuma opcao aceita copyleft",
                onde(self.decisivas())
            ),
            Classe::Desconhecida if self.decisivas().is_empty() => format!(
                "nenhuma licenca achada entre {} e {} (LICENSE/COPYING, SPDX-License-Identifier, \
`license:` do cabecalho ou do plugin.json): sem licenca nao ha permissao de copiar. Se a \
licenca mora acima, aponte a raiz do repositorio; {OPCAO_ACEITAR} importa assim mesmo e grava \
a decisao",
                self.alvo.display(),
                self.limite.display()
            ),
            Classe::Desconhecida => format!(
                "licenca nao reconhecida: {}; {OPCAO_ACEITAR} importa assim mesmo e grava a decisao",
                onde(self.decisivas())
            ),
        }
    }
}

/// A politica: compativel entra; copyleft e recusado sempre; desconhecida so com a opcao,
/// e a decisao volta para ser gravada. `Err` traz o motivo.
pub fn decidir(
    c: &Conferencia,
    aceitar_desconhecida: bool,
) -> Result<Option<DecisaoDoOperador>, String> {
    match c.classe {
        Classe::Compativel => Ok(None),
        Classe::Copyleft => Err(c.motivo()),
        Classe::Desconhecida if aceitar_desconhecida => Ok(Some(DecisaoDoOperador {
            opcao: OPCAO_ACEITAR.into(),
            operador: std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "desconhecido".into()),
            quando: chrono::Utc::now().to_rfc3339(),
            recusa_atravessada: c.motivo(),
        })),
        Classe::Desconhecida => Err(c.motivo()),
    }
}

/// Os textos de licenca e `NOTICE` achados, byte a byte, cada um com o nome que tinha na
/// origem (`LICENSE.2` quando o nome se repete em outra pasta do caminho). Para quem guarda
/// o aviso no proprio formato -- o importador de papeis grava `agency/LICENSE` com estes
/// bytes, sem caminho local no conteudo, para a segunda corrida nao mudar byte.
pub fn textos(c: &Conferencia) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut v: Vec<(String, Vec<u8>)> = Vec::new();
    for p in &c.textos {
        let base = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "LICENSE".into());
        let mut nome = base.clone();
        let mut k = 2;
        while v.iter().any(|(n, _)| *n == nome) {
            nome = format!("{base}.{k}");
            k += 1;
        }
        v.push((nome, ler_limitado(p)?));
    }
    Ok(v)
}

/// Grava o aviso (`LICENCA.txt`) em `dir`: o resumo, as linhas de copyright e o texto
/// INTEIRO de cada licenca e `NOTICE` achados, byte a byte. Chamado so para o que entra.
pub fn guardar_aviso(
    c: &Conferencia,
    decisao: Option<DecisaoDoOperador>,
    dir: &Path,
) -> Result<Registro, String> {
    let mut s: Vec<u8> = Vec::new();
    let mut linha = |t: String| {
        s.extend_from_slice(t.as_bytes());
        s.push(b'\n');
    };
    linha(format!(
        "Aviso de licenca de terceiro, guardado pelo PhxClaw na importacao.\nOrigem: {}\nClasse: {} ({})",
        c.alvo.display(),
        c.classe.nome(),
        c.licencas().join(", ")
    ));
    linha("Declaracoes achadas:".into());
    for d in &c.declaracoes {
        linha(format!("  - {} ({}, {})", d.licenca, d.como, d.onde));
    }
    if !c.copyright.is_empty() {
        linha("Copyright:".into());
        for l in &c.copyright {
            linha(format!("  {l}"));
        }
    }
    if let Some(d) = &decisao {
        linha(format!(
            "Decisao do operador: {} por {} em {}, atravessando: {}",
            d.opcao, d.operador, d.quando, d.recusa_atravessada
        ));
    }
    let mut copiados = Vec::new();
    for (p, b) in c.textos.iter().zip(textos(c)?) {
        s.extend_from_slice(format!("\n==== {} ====\n", p.display()).as_bytes());
        s.extend_from_slice(&b.1);
        if !b.1.ends_with(b"\n") {
            s.push(b'\n');
        }
        copiados.push(p.display().to_string());
    }
    std::fs::write(dir.join(ARQUIVO_AVISO), s)
        .map_err(|e| format!("{}: {e}", dir.join(ARQUIVO_AVISO).display()))?;
    Ok(Registro {
        classe: c.classe,
        licencas: c.licencas(),
        declaracoes: c.declaracoes.clone(),
        textos_copiados: copiados,
        copyright: c.copyright.clone(),
        decisao_do_operador: decisao,
    })
}

/// O relatorio de `phxclaw licenca conferir` em JSON: o que o importador de papeis (Python)
/// le. `entra` ja aplica a politica; `decisao_do_operador` vem quando a opcao foi usada e a
/// licenca era desconhecida -- quem chama grava junto do que importar.
pub fn relatorio_json(c: &Conferencia, aceitar_desconhecida: bool) -> serde_json::Value {
    let decisao = decidir(c, aceitar_desconhecida);
    serde_json::json!({
        "alvo": c.alvo,
        "limite": c.limite,
        "classe": c.classe,
        "licencas": c.licencas(),
        "entra": decisao.is_ok(),
        "motivo": c.motivo(),
        "declaracoes": c.declaracoes,
        "textos": c.textos,
        "copyright": c.copyright,
        "decisao_do_operador": decisao.ok().flatten(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressao_spdx_avalia_ou_e_com() {
        use Classe::*;
        let c = |e: &str| classe_da_expressao(e);
        assert_eq!(c("MIT"), Some(Compativel));
        assert_eq!(c("mit or apache-2.0"), Some(Compativel));
        assert_eq!(c("MIT OR GPL-3.0-only"), Some(Compativel), "OR e escolha");
        assert_eq!(c("MIT AND GPL-3.0-only"), Some(Copyleft), "AND soma");
        // O caso que achatar os parenteses errava para o lado perigoso.
        assert_eq!(c("GPL-3.0 AND (MIT OR Apache-2.0)"), Some(Copyleft));
        assert_eq!(c("(GPL-3.0 AND MIT) OR Apache-2.0"), Some(Compativel));
        assert_eq!(
            c("GPL-2.0-only WITH Classpath-exception-2.0"),
            Some(Copyleft)
        );
        assert_eq!(c("AGPL-3.0-or-later"), Some(Copyleft));
        assert_eq!(c("Foo-1.0-or-later"), Some(Copyleft), "qualquer -or-later");
        assert_eq!(c("GPL-2.0+"), Some(Copyleft));
        assert_eq!(c("LicenseRef-Proprietaria"), Some(Desconhecida));
        assert_eq!(c("MIT License"), None, "prosa nao e expressao");
        assert_eq!(c("(MIT"), None);
        assert_eq!(classe_do_id("LGPL-2.1"), Copyleft);
        assert_eq!(classe_do_id("CC-BY-4.0"), Desconhecida);
        assert_eq!(classe_do_id("CC-BY-SA-4.0"), Copyleft);
    }

    #[test]
    fn prosa_do_cabecalho_le_pelo_lado_seguro() {
        let c = |v: &str| declaracao_de_valor(v, "x", "cabecalho").classe;
        assert_eq!(c("MIT License"), Some(Classe::Compativel));
        assert_eq!(c("Apache License 2.0"), Some(Classe::Compativel));
        assert_eq!(c("GNU GPL v3"), Some(Classe::Copyleft));
        assert_eq!(c("Affero"), Some(Classe::Copyleft));
        assert_eq!(c("AGPLv3"), Some(Classe::Copyleft));
        // «example» contem «mpl»: so palavra inteira conta.
        assert_eq!(c("see example"), Some(Classe::Desconhecida));
        // Remete ao arquivo: a classe vem dele.
        assert_eq!(c("Complete terms in LICENSE.txt"), None);
        assert_eq!(c("Proprietaria"), Some(Classe::Desconhecida));
    }

    #[test]
    fn assinatura_pelo_texto_e_nao_pelo_nome() {
        assert_eq!(
            identificar_texto(
                "GNU AFFERO GENERAL PUBLIC LICENSE\n  Version 3, 19 November 2007\n...GNU General Public License"
            ),
            Some("AGPL-3.0")
        );
        assert_eq!(
            identificar_texto(
                "GNU LESSER GENERAL PUBLIC LICENSE\nVersion 3, 29 June 2007\nversion 3 of the GNU General Public License"
            ),
            Some("LGPL-3.0")
        );
        assert_eq!(
            identificar_texto(
                "Redistribution and use in source and binary forms, with or\nwithout modification, are permitted provided that ... Neither the name of"
            ),
            Some("BSD-3-Clause")
        );
        assert_eq!(identificar_texto("Licenca: veja o site."), None);
        assert!(e_arquivo_de_licenca("LICENSE-MIT"));
        assert!(e_arquivo_de_licenca("COPYING.LESSER"));
        assert!(e_arquivo_de_licenca("licence.md"));
        assert!(!e_arquivo_de_licenca("license_check.py"));
        assert!(!e_arquivo_de_licenca("licenses-overview"));
    }

    #[test]
    fn copyright_e_o_aviso_e_nao_a_prosa_da_licenca() {
        let mut v = Vec::new();
        linhas_de_copyright(
            "      copyright notice that is included in or attached to the work\n\
             copyright license to reproduce, prepare Derivative Works of,\n\
             (c) You must retain, in the Source form of any Derivative Works\n\
             Copyright [yyyy] [name of copyright owner]\n\
             Copyright 2026 PhxClaw contributors\n\
             Copyright (c) 2025 AgentLand Contributors\n\
             \u{a9} 2024 Fulana\n",
            &mut v,
        );
        assert_eq!(
            v,
            [
                "Copyright 2026 PhxClaw contributors",
                "Copyright (c) 2025 AgentLand Contributors",
                "\u{a9} 2024 Fulana"
            ]
        );
    }
}
