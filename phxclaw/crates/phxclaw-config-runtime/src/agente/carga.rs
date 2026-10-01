//! A carga do `config.json` do agente, com a precedencia escrita e a origem de cada valor:
//!
//! **ambiente > `.phxclaw/config.json` do projeto > `<pasta>/config.json` > padrao.**
//!
//! O ambiente ganha porque e o que o operador muda para UMA execucao sem tocar arquivo
//! (e o que os testes e os servicos ja usam). O projeto ganha da pasta porque e o mais
//! especifico -- mas so entra projeto CONFIADO, e isso e decisao de quem chama: um
//! repositorio clonado poderia apontar `voz.whisper.bin` para um executavel dele.
//!
//! O que a carga recusa, sempre dizendo a chave:
//! - chave desconhecida (com a sugestao do nome proximo): configuracao que nao e lida mente;
//! - tipo errado (com o tipo esperado);
//! - segredo no arquivo (chave que e segredo no catalogo, ou valor com cara de credencial):
//!   segredo mora no SecretBroker, e o arquivo vai para backup, diff e tela;
//! - chave que so vale no ambiente.
//!
//! O arquivo se escreve em secoes (`{"voz": {"whisper": {"bin": "..."}}}`); `"//"` e
//! comentario em qualquer nivel (o exemplo gerado usa), e `"revisao"` e `"$schema"` sao do
//! proprio arquivo.

use super::catalogo::{catalogo, por_chave, Chave, Natureza, Tipo};
use crate::{distancia, gravar_revisado, ConfigError};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub const CAMPO_REVISAO: &str = "revisao";
pub const CAMPO_COMENTARIO: &str = "//";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Erro {
    pub chave: String,
    pub motivo: String,
}

impl Erro {
    fn novo(chave: impl Into<String>, motivo: impl Into<String>) -> Self {
        Self {
            chave: chave.into(),
            motivo: motivo.into(),
        }
    }
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.chave, self.motivo)
    }
}

/// Texto de varios erros, um por linha (para a CLI e para `Result<_, String>`).
pub fn em_texto(erros: &[Erro]) -> String {
    erros
        .iter()
        .map(Erro::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origem {
    Ambiente,
    Projeto,
    Pasta,
    Padrao,
    Ausente,
}

impl Origem {
    pub fn nome(self) -> &'static str {
        match self {
            Origem::Ambiente => "ambiente",
            Origem::Projeto => "projeto",
            Origem::Pasta => "pasta",
            Origem::Padrao => "padrao",
            Origem::Ausente => "ausente",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Efetivo {
    /// Sempre `None` para segredo: o valor nunca passa por aqui.
    pub valor: Option<Value>,
    pub origem: Origem,
}

/// Um `config.json` lido e conferido.
#[derive(Clone, Debug, Default)]
pub struct Arquivo {
    pub revisao: u64,
    /// Chave do catalogo -> valor (so as definidas; `null` no arquivo nao entra).
    pub valores: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct Configuracao {
    efetivos: BTreeMap<String, Efetivo>,
    pub pasta: Arquivo,
    pub projeto: Option<Arquivo>,
}

impl Configuracao {
    /// O valor efetivo. Chave fora do catalogo e erro de programacao: em depuracao para.
    pub fn valor(&self, chave: &str) -> Option<&Value> {
        debug_assert!(
            por_chave(chave).is_some(),
            "chave fora do catalogo: {chave}"
        );
        self.efetivos.get(chave).and_then(|e| e.valor.as_ref())
    }

    pub fn efetivo(&self, chave: &str) -> Option<&Efetivo> {
        self.efetivos.get(chave)
    }

    pub fn origem(&self, chave: &str) -> Origem {
        self.efetivos
            .get(chave)
            .map(|e| e.origem)
            .unwrap_or(Origem::Ausente)
    }

    pub fn texto(&self, chave: &str) -> Option<String> {
        match self.valor(chave)? {
            Value::String(s) => Some(s.clone()),
            outro => Some(outro.to_string()),
        }
    }

    pub fn inteiro(&self, chave: &str) -> Option<i64> {
        self.valor(chave)?.as_i64()
    }

    pub fn booleano(&self, chave: &str) -> Option<bool> {
        self.valor(chave)?.as_bool()
    }

    pub fn lista(&self, chave: &str) -> Option<Vec<String>> {
        Some(
            self.valor(chave)?
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        )
    }

    /// A soma das revisoes dos dois arquivos: o token de concorrencia da API. Qualquer
    /// gravacao, em qualquer dos dois, o faz subir de um.
    pub fn revisao(&self) -> u64 {
        self.pasta.revisao + self.projeto.as_ref().map_or(0, |p| p.revisao)
    }
}

/// Prefixos de credencial conhecidos. Valor que comeca assim nao e configuracao.
const PREFIXOS: &[&str] = &[
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxs-",
    "xapp-",
    "sk-",
    "sk_live_",
    "rk_live_",
    "AIza",
    "AKIA",
    "ya29.",
    "xai-",
    "-----BEGIN",
];

/// O motivo, se o texto tem cara de credencial: prefixo conhecido, ou URL com senha
/// (`esquema://usuario:senha@host`).
pub fn credencial_aparente(s: &str) -> Option<String> {
    let t = s.trim();
    if let Some(p) = PREFIXOS.iter().find(|p| t.starts_with(**p)) {
        return Some(format!("valor com cara de credencial (comeca com {p})"));
    }
    if let Some((_, resto)) = t.split_once("://") {
        let autoridade = resto.split(['/', '?', '#']).next().unwrap_or("");
        if let Some((info, _)) = autoridade.rsplit_once('@') {
            if info.contains(':') {
                return Some("URL com senha embutida (usuario:senha@)".into());
            }
        }
    }
    None
}

fn eh_falso(s: &str) -> bool {
    matches!(s, "0" | "false" | "no" | "off" | "nao" | "não")
}

/// O valor tipado do texto de uma variavel de ambiente (ou do `config definir`). Lista
/// tambem aceita `[...]` em JSON.
pub fn do_texto(c: &Chave, bruto: &str) -> Result<Value, String> {
    let s = bruto.trim();
    let esperado = || format!("esperado {}, veio {s:?}", c.tipo.nome());
    match c.tipo {
        Tipo::Texto | Tipo::Caminho => Ok(Value::String(s.to_string())),
        Tipo::Inteiro => s.parse::<i64>().map(Value::from).map_err(|_| esperado()),
        Tipo::Real => s
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(esperado),
        Tipo::Booleano => {
            let l = s.to_lowercase();
            if matches!(l.as_str(), "1" | "true" | "yes" | "on" | "sim") {
                Ok(Value::Bool(true))
            } else if eh_falso(&l) {
                Ok(Value::Bool(false))
            } else {
                Err(format!("{} (1/0, true/false, sim/nao)", esperado()))
            }
        }
        Tipo::Lista(sep) => {
            if s.starts_with('[') {
                let v: Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
                conferir_tipo(c, &v)?;
                return Ok(v);
            }
            Ok(Value::Array(
                s.split(sep)
                    .map(str::trim)
                    .filter(|x| !x.is_empty())
                    .map(|x| Value::String(x.to_string()))
                    .collect(),
            ))
        }
        Tipo::Enum(opcoes) => {
            if opcoes.contains(&s) {
                Ok(Value::String(s.to_string()))
            } else {
                Err(format!("esperado um de {}, veio {s:?}", opcoes.join("|")))
            }
        }
    }
}

/// O valor JSON tem o tipo da chave?
pub fn conferir_tipo(c: &Chave, v: &Value) -> Result<(), String> {
    let ok = match c.tipo {
        Tipo::Texto | Tipo::Caminho => v.is_string(),
        Tipo::Inteiro => v.is_i64() || v.is_u64(),
        Tipo::Real => v.is_number(),
        Tipo::Booleano => v.is_boolean(),
        Tipo::Lista(_) => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
        Tipo::Enum(opcoes) => {
            return match v.as_str() {
                Some(s) if opcoes.contains(&s) => Ok(()),
                _ => Err(format!("esperado um de {}, veio {v}", opcoes.join("|"))),
            };
        }
    };
    if ok {
        Ok(())
    } else {
        let tipo = match c.tipo {
            Tipo::Lista(_) => "lista de textos".to_string(),
            t => t.nome().to_string(),
        };
        Err(format!("esperado {tipo}, veio {v}"))
    }
}

/// O nome conhecido mais proximo, se for perto.
pub fn sugestao(digitada: &str) -> Option<&'static str> {
    catalogo()
        .iter()
        .map(|c| (distancia(digitada, &c.chave), c.chave.as_str()))
        .filter(|(d, _)| *d <= (digitada.chars().count() / 4).max(2))
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

fn desconhecida(chave: &str) -> Erro {
    let motivo = match sugestao(chave) {
        Some(s) => format!("chave desconhecida; quis dizer `{s}`?"),
        None => "chave desconhecida (veja `phxclaw config exemplo`)".to_string(),
    };
    Erro::novo(chave, motivo)
}

/// A recusa de uma chave que nao mora no arquivo (segredo ou so do ambiente).
pub fn recusa_fora_do_arquivo(c: &Chave) -> Option<String> {
    match &c.natureza {
        Natureza::Config => None,
        Natureza::Segredo { comando, .. } => Some(format!(
            "e segredo e nao mora no config.json: guarde com `{comando}`"
        )),
        Natureza::Ambiente { motivo } => Some(format!(
            "so vale como variavel de ambiente ({}): {motivo}",
            c.variavel
        )),
    }
}

/// Confere UM valor para o arquivo: natureza, tipo e cara de credencial.
pub fn conferir_valor(c: &Chave, v: &Value) -> Result<(), String> {
    if let Some(m) = recusa_fora_do_arquivo(c) {
        return Err(m);
    }
    conferir_tipo(c, v)?;
    let textos: Vec<&str> = match v {
        Value::String(s) => vec![s.as_str()],
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    };
    for t in textos {
        if let Some(m) = credencial_aparente(t) {
            return Err(format!(
                "{m}: segredo mora no SecretBroker, nunca no config.json"
            ));
        }
    }
    Ok(())
}

fn eh_prefixo_de_secao(p: &str) -> bool {
    let com_ponto = format!("{p}.");
    catalogo().iter().any(|c| c.chave.starts_with(&com_ponto))
}

fn achatar(
    caminho: &str,
    obj: &Map<String, Value>,
    out: &mut Vec<(String, Value)>,
    erros: &mut Vec<Erro>,
) {
    for (k, v) in obj {
        if k == CAMPO_COMENTARIO {
            let ok = v.is_string() || v.as_array().is_some_and(|a| a.iter().all(Value::is_string));
            if !ok {
                erros.push(Erro::novo(
                    format!("{caminho}{k}"),
                    "comentario e texto ou lista de textos",
                ));
            }
            continue;
        }
        let chave = format!("{caminho}{k}");
        match v {
            Value::Object(m) if por_chave(&chave).is_none() => {
                if eh_prefixo_de_secao(&chave) {
                    achatar(&format!("{chave}."), m, out, erros);
                } else {
                    erros.push(desconhecida(&chave));
                }
            }
            _ => out.push((chave, v.clone())),
        }
    }
}

/// Confere um documento inteiro e devolve a revisao e os valores definidos. Junta TODOS os
/// erros: a tela e a CLI mostram a lista, e consertar um por vez seria uma ida por erro.
pub fn validar_documento(doc: &Value) -> Result<Arquivo, Vec<Erro>> {
    let Some(raiz) = doc.as_object() else {
        return Err(vec![Erro::novo("(raiz)", "o config.json e um objeto JSON")]);
    };
    let mut erros = Vec::new();
    let mut revisao = 0;
    let mut corpo = Map::new();
    for (k, v) in raiz {
        match k.as_str() {
            CAMPO_REVISAO => match v.as_u64() {
                Some(r) => revisao = r,
                None => erros.push(Erro::novo(k, "esperado inteiro >= 0")),
            },
            "$schema" => {
                if !v.is_string() {
                    erros.push(Erro::novo(k, "esperado texto"));
                }
            }
            _ => {
                corpo.insert(k.clone(), v.clone());
            }
        }
    }
    let mut pares = Vec::new();
    achatar("", &corpo, &mut pares, &mut erros);
    let mut valores = BTreeMap::new();
    for (chave, v) in pares {
        let Some(c) = por_chave(&chave) else {
            erros.push(desconhecida(&chave));
            continue;
        };
        if v.is_null() {
            // `null` e «nao definido aqui» (o exemplo gerado usa); a recusa de segredo vale
            // mesmo assim, para o arquivo nao ensinar a pôr segredo nele.
            if let Some(m) = recusa_fora_do_arquivo(c) {
                erros.push(Erro::novo(chave, m));
            }
            continue;
        }
        match conferir_valor(c, &v) {
            Ok(()) => {
                valores.insert(chave, v);
            }
            Err(m) => erros.push(Erro::novo(chave, m)),
        }
    }
    if erros.is_empty() {
        Ok(Arquivo { revisao, valores })
    } else {
        Err(erros)
    }
}

/// Le e confere um `config.json`. Arquivo ausente e arquivo vazio de revisao 0.
pub fn ler_arquivo(caminho: &Path) -> Result<Arquivo, Vec<Erro>> {
    let rotulo = caminho.display().to_string();
    let bytes = match std::fs::read(caminho) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Arquivo::default()),
        Err(e) => return Err(vec![Erro::novo(rotulo, e.to_string())]),
    };
    let doc: Value = serde_json::from_slice(&bytes)
        .map_err(|e| vec![Erro::novo(rotulo.clone(), format!("JSON invalido: {e}"))])?;
    validar_documento(&doc).map_err(|es| {
        es.into_iter()
            .map(|e| Erro::novo(e.chave, format!("{} (em {rotulo})", e.motivo)))
            .collect()
    })
}

/// O documento do arquivo, de volta em secoes (o que se grava).
pub fn documento(revisao: u64, valores: &BTreeMap<String, Value>) -> Value {
    let mut raiz = Map::new();
    raiz.insert(CAMPO_REVISAO.into(), Value::from(revisao));
    for (chave, v) in valores {
        let partes: Vec<&str> = chave.split('.').collect();
        let mut atual = &mut raiz;
        for p in &partes[..partes.len() - 1] {
            atual = atual
                .entry(p.to_string())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .expect("secao e objeto");
        }
        atual.insert(partes[partes.len() - 1].to_string(), v.clone());
    }
    Value::Object(raiz)
}

pub type Ambiente<'a> = &'a dyn Fn(&str) -> Option<String>;

/// A configuracao efetiva. `projeto`: o arquivo do projeto ja CONFIADO (quem chama decide;
/// `None` = sem projeto ou nao confiado).
pub fn carregar(
    ambiente: Ambiente<'_>,
    pasta: &Path,
    projeto: Option<&Path>,
) -> Result<Configuracao, Vec<Erro>> {
    let mut erros = Vec::new();
    let arq_pasta = ler_arquivo(pasta).unwrap_or_else(|e| {
        erros.extend(e);
        Arquivo::default()
    });
    let arq_projeto = projeto.map(|p| {
        ler_arquivo(p).unwrap_or_else(|e| {
            erros.extend(e);
            Arquivo::default()
        })
    });
    let mut efetivos = BTreeMap::new();
    for c in catalogo() {
        let bruto = ambiente(&c.variavel).filter(|v| !v.trim().is_empty());
        let efetivo = if c.segredo() {
            Efetivo {
                valor: None,
                origem: if bruto.is_some() {
                    Origem::Ambiente
                } else {
                    Origem::Ausente
                },
            }
        } else if let Some(b) = bruto {
            match do_texto(c, &b) {
                Ok(v) => Efetivo {
                    valor: Some(v),
                    origem: Origem::Ambiente,
                },
                Err(m) => {
                    erros.push(Erro::novo(&c.chave, format!("{} (em {})", m, c.variavel)));
                    continue;
                }
            }
        } else if let Some(v) = arq_projeto.as_ref().and_then(|a| a.valores.get(&c.chave)) {
            Efetivo {
                valor: Some(v.clone()),
                origem: Origem::Projeto,
            }
        } else if let Some(v) = arq_pasta.valores.get(&c.chave) {
            Efetivo {
                valor: Some(v.clone()),
                origem: Origem::Pasta,
            }
        } else if let Some(p) = c.padrao {
            Efetivo {
                valor: Some(do_texto(c, p).expect("padrao do catalogo conferido no teste")),
                origem: Origem::Padrao,
            }
        } else {
            Efetivo {
                valor: None,
                origem: Origem::Ausente,
            }
        };
        efetivos.insert(c.chave.clone(), efetivo);
    }
    if erros.is_empty() {
        Ok(Configuracao {
            efetivos,
            pasta: arq_pasta,
            projeto: arq_projeto,
        })
    } else {
        Err(erros)
    }
}

/// Recusa de uma mudanca: conflito de revisao, ou a lista de erros por chave.
#[derive(Debug)]
pub enum Recusa {
    Conflito { atual: u64 },
    Invalida(Vec<Erro>),
    Falha(String),
}

impl fmt::Display for Recusa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Recusa::Conflito { atual } => write!(f, "conflito de revisao (atual: {atual})"),
            Recusa::Invalida(e) => write!(f, "{}", em_texto(e)),
            Recusa::Falha(m) => write!(f, "{m}"),
        }
    }
}

/// O alvo de uma gravacao.
pub struct Alvo<'a> {
    pub arquivo: &'a Path,
    /// A configuracao efetiva de agora: diz se a chave vem do ambiente.
    pub atual: &'a Configuracao,
    /// Revisao de concorrencia esperada (a soma de `Configuracao::revisao`); `None`: a de
    /// agora (a CLI, que nao tem If-Match).
    pub esperada: Option<u64>,
}

/// Aplica `mudancas` (`chave -> valor`, `null` remove) ao arquivo do alvo e grava pela
/// mesma `gravar_revisado` do config-runtime. Devolve a nova revisao de concorrencia.
pub fn definir(alvo: Alvo<'_>, mudancas: &Map<String, Value>) -> Result<u64, Recusa> {
    let total = alvo.atual.revisao();
    if let Some(e) = alvo.esperada {
        if e != total {
            return Err(Recusa::Conflito { atual: total });
        }
    }
    let em_disco = ler_arquivo(alvo.arquivo).map_err(Recusa::Invalida)?;
    let mut valores = em_disco.valores.clone();
    let mut erros = Vec::new();
    for (chave, v) in mudancas {
        let Some(c) = por_chave(chave) else {
            erros.push(desconhecida(chave));
            continue;
        };
        if let Some(m) = recusa_fora_do_arquivo(c) {
            erros.push(Erro::novo(chave, m));
            continue;
        }
        if alvo.atual.origem(chave) == Origem::Ambiente {
            erros.push(Erro::novo(
                chave,
                format!(
                    "vem do ambiente ({}): o arquivo nao teria efeito enquanto a variavel existir",
                    c.variavel
                ),
            ));
            continue;
        }
        if v.is_null() {
            valores.remove(chave);
            continue;
        }
        match conferir_valor(c, v) {
            Ok(()) => {
                valores.insert(chave.clone(), v.clone());
            }
            Err(m) => erros.push(Erro::novo(chave, m)),
        }
    }
    if !erros.is_empty() {
        return Err(Recusa::Invalida(erros));
    }
    let atual_doc = std::fs::read(alvo.arquivo)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let proximo = documento(em_disco.revisao, &valores);
    let validar = |v: &Value| {
        validar_documento(v)
            .map(|_| ())
            .map_err(|e| ConfigError::Invalid(em_texto(&e)))
    };
    let historico = historico_de(alvo.arquivo);
    gravar_revisado(
        alvo.arquivo,
        &historico,
        atual_doc.as_ref().map(|d| (em_disco.revisao, d)),
        em_disco.revisao,
        proximo,
        CAMPO_REVISAO,
        &validar,
    )
    .map_err(|e| match e {
        ConfigError::RevisionConflict { actual, .. } => Recusa::Conflito { atual: actual },
        outro => Recusa::Falha(outro.to_string()),
    })?;
    Ok(total + 1)
}

/// O historico das revisoes, ao lado do arquivo.
pub fn historico_de(arquivo: &Path) -> PathBuf {
    arquivo
        .parent()
        .unwrap_or(Path::new("."))
        .join("config-historico")
}
