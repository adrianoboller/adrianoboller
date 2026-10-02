//! A carga do `config.json` do agente, com a precedencia escrita e a origem de cada valor:
//!
//! **ambiente > `.phxclaw/config.json` do projeto > perfil ativo > `<pasta>/config.json`
//! > padrao** (`PRECEDENCIA`, a lista que o codigo percorre e que os textos citam).
//!
//! O ambiente ganha porque e o que o operador muda para UMA execucao sem tocar arquivo
//! (e o que os testes e os servicos ja usam). O projeto ganha da pasta porque e o mais
//! especifico -- mas so entra projeto CONFIADO, e isso e decisao de quem chama: um
//! repositorio clonado poderia apontar `voz.whisper.bin` para um executavel dele.
//!
//! O perfil (`"perfis": {nome: {secoes...}}` e `"perfil_ativo"` no arquivo da PASTA) e uma
//! camada por cima da pasta, como o perfil do VS Code e uma camada por cima das
//! configuracoes do usuario: ele vence a base da pasta (senao ativar um perfil nao mudaria
//! nada que a base ja define) e perde para o projeto (o workspace continua mandando). A
//! variavel `PHXCLAW_PERFIL` escolhe o perfil de UMA execucao sem gravar o arquivo.
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
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// O nome do arquivo, na pasta do agente e em `.phxclaw/` do projeto.
pub const ARQUIVO: &str = "config.json";
/// A chave do catalogo cuja variavel (`PHXCLAW_HOME`) localiza a pasta do agente.
pub const CHAVE_PASTA: &str = "agente.pasta";
/// A pasta do agente quando nem a variavel nem o `--pasta` dizem.
pub const PASTA_PADRAO: &str = "var/agente";

pub const CAMPO_REVISAO: &str = "revisao";
pub const CAMPO_COMENTARIO: &str = "//";
pub const CAMPO_PERFIS: &str = "perfis";
pub const CAMPO_PERFIL_ATIVO: &str = "perfil_ativo";
/// A chave do catalogo cuja variavel (`PHXCLAW_PERFIL`) escolhe o perfil de uma execucao.
pub const CHAVE_PERFIL: &str = "perfil.ativo";

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
    /// O perfil ativo do arquivo da pasta.
    Perfil,
    Pasta,
    Padrao,
    Ausente,
}

/// A ordem de precedencia, do que ganha ao que perde. E A fonte unica: `carregar` percorre
/// esta lista, e todo texto que a descreve (`config exemplo`, a ajuda da CLI, o guia) sai
/// de `precedencia_texto` -- escrita em dois lugares, um envelhece calado (NoGo do
/// integrador na onda 2, quando o exemplo omitia o perfil).
pub const PRECEDENCIA: [Origem; 5] = [
    Origem::Ambiente,
    Origem::Projeto,
    Origem::Perfil,
    Origem::Pasta,
    Origem::Padrao,
];

/// «ambiente > .phxclaw/config.json do projeto confiado > perfil ativo > <pasta>/config.json > padrão».
pub fn precedencia_texto() -> String {
    PRECEDENCIA
        .iter()
        .map(|o| o.rotulo())
        .collect::<Vec<_>>()
        .join(" > ")
}

impl Origem {
    /// O rotulo de tela da origem na frase da precedencia.
    pub fn rotulo(self) -> &'static str {
        match self {
            Origem::Ambiente => "ambiente",
            Origem::Projeto => ".phxclaw/config.json do projeto confiado",
            Origem::Perfil => "perfil ativo",
            Origem::Pasta => "<pasta>/config.json",
            Origem::Padrao => "padrão",
            Origem::Ausente => "ausente",
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Origem::Ambiente => "ambiente",
            Origem::Projeto => "projeto",
            Origem::Perfil => "perfil",
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
    pub caminho: PathBuf,
    pub revisao: u64,
    /// SHA-256 (hex) dos bytes em disco; arquivo ausente e o hash do vazio. E a parte
    /// deste arquivo no token de concorrencia.
    pub sha256: String,
    /// Chave do catalogo -> valor (so as definidas; `null` no arquivo nao entra).
    pub valores: BTreeMap<String, Value>,
    /// Os perfis do arquivo, cada um com as chaves que ele sobrepoe a base.
    pub perfis: BTreeMap<String, BTreeMap<String, Value>>,
    /// O perfil gravado como ativo (sempre um nome de `perfis`).
    pub perfil_ativo: Option<String>,
}

/// Nome de perfil: ASCII, digitos, `-` e `_`, ate 32 -- ele vira pedaco de rota e de
/// linha de comando.
pub fn nome_de_perfil_valido(nome: &str) -> Result<(), String> {
    if nome.is_empty() || nome.len() > 32 {
        return Err("nome de perfil: 1 a 32 caracteres".into());
    }
    if !nome
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err("nome de perfil: so letras, digitos, `-` e `_`".into());
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Configuracao {
    efetivos: BTreeMap<String, Efetivo>,
    pub pasta: Arquivo,
    pub projeto: Option<Arquivo>,
    /// O perfil em vigor (o da variavel, senao o do arquivo) e de onde veio.
    pub perfil_ativo: Option<(String, Origem)>,
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

    /// O token de concorrencia da API: SHA-256 sobre o SHA-256 de cada um dos dois
    /// arquivos. Era a soma das revisoes, e soma tem ABA: a pasta em 3 com o projeto em 1
    /// da o mesmo 4 que a pasta em 2 com o projeto em 2, e um `If-Match` velho passaria.
    pub fn revisao(&self) -> String {
        token(
            &self.pasta.sha256,
            self.projeto.as_ref().map(|p| p.sha256.as_str()),
        )
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// O SHA-256 do que esta no disco AGORA (ausente: o do vazio).
pub fn sha_em_disco(caminho: &Path) -> Result<String, String> {
    match std::fs::read(caminho) {
        Ok(b) => Ok(sha256_hex(&b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(sha256_hex(b"")),
        Err(e) => Err(format!("{}: {e}", caminho.display())),
    }
}

/// O token de concorrencia a partir dos hashes dos dois arquivos (projeto ausente conta
/// como ausente, nao como vazio: ligar um projeto sem valor tambem muda o token).
pub fn token(pasta: &str, projeto: Option<&str>) -> String {
    sha256_hex(format!("{pasta}\n{}", projeto.unwrap_or("-")).as_bytes())
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

/// Tamanho a partir do qual um texto de classe secreta e tratado como chave.
pub const CHAVE_MINIMA: usize = 20;
/// Entropia de Shannon (bits por caractere) acima da qual um texto de classe secreta nao
/// parece nome de modelo nem versao: `gpt-4o-mini-2024-07-18` fica em 3,6; vinte
/// caracteres aleatorios de uma chave real ficam acima de 4.
const ENTROPIA_MINIMA: f64 = 3.9;

fn entropia(t: &str) -> f64 {
    let mut contagem: BTreeMap<char, usize> = BTreeMap::new();
    for c in t.chars() {
        *contagem.entry(c).or_default() += 1;
    }
    let n = t.chars().count() as f64;
    contagem
        .values()
        .map(|&k| {
            let p = k as f64 / n;
            -p * p.log2()
        })
        .sum()
}

/// Texto de classe secreta: so caracteres de token (base64, hex, `-`, `_`), comprido,
/// com digito e letra dos dois casos (ou hexadecimal longo) e entropia alta. Prefixo
/// conhecido so pega o provedor que esta na lista; a chave de um provedor novo tem de
/// cair pela forma.
fn classe_secreta(t: &str) -> bool {
    let n = t.chars().count();
    if n < CHAVE_MINIMA
        || !t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/' | '='))
    {
        return false;
    }
    let hexa = n >= 32 && t.chars().all(|c| c.is_ascii_hexdigit());
    let misto = t.chars().any(|c| c.is_ascii_digit())
        && t.chars().any(|c| c.is_ascii_lowercase())
        && t.chars().any(|c| c.is_ascii_uppercase());
    (hexa || misto) && entropia(t) >= ENTROPIA_MINIMA
}

/// O nome tem cara de segredo (`*_key`, `*_token`, `*_secret`, `password`, `senha`)?
/// Olha o ultimo segmento da chave pontuada, inteiro ou pelo sufixo depois de `_`:
/// `max_tokens` e contagem, `bot_token` e credencial.
pub fn nome_de_segredo(chave: &str) -> bool {
    let ultimo = chave
        .rsplit('.')
        .next()
        .unwrap_or(chave)
        .to_ascii_lowercase();
    const SUFIXOS: &[&str] = &["key", "token", "secret", "password", "senha", "apikey"];
    SUFIXOS.iter().any(|s| {
        ultimo == *s || ultimo.ends_with(&format!("_{s}")) || ultimo.ends_with(&format!("-{s}"))
    })
}

/// O motivo, se o texto tem cara de credencial: prefixo conhecido, URL com senha
/// (`esquema://usuario:senha@host`) ou a forma de uma chave. O motivo NUNCA carrega o
/// valor, nem um pedaco: o texto do erro vai para a tela, o log e o relatorio da CLI, e
/// e exatamente ali que a credencial nao pode estar. So o tamanho.
/// O prefixo de credencial conhecido com que o texto comeca, se algum.
pub fn prefixo_de_credencial(s: &str) -> Option<&'static str> {
    let t = s.trim();
    PREFIXOS.iter().copied().find(|p| t.starts_with(p))
}

pub fn credencial_aparente(s: &str) -> Option<String> {
    let t = s.trim();
    let n = t.chars().count();
    if prefixo_de_credencial(t).is_some() {
        return Some(format!(
            "valor com cara de credencial (prefixo de credencial conhecido, {n} caracteres)"
        ));
    }
    if let Some((_, resto)) = t.split_once("://") {
        let autoridade = resto.split(['/', '?', '#']).next().unwrap_or("");
        if let Some((info, _)) = autoridade.rsplit_once('@') {
            if info.contains(':') {
                return Some(format!(
                    "URL com senha embutida (usuario:senha@, {n} caracteres)"
                ));
            }
        }
    }
    if classe_secreta(t) {
        return Some(format!(
            "valor com cara de credencial (forma de chave, {n} caracteres)"
        ));
    }
    None
}

/// A descricao de um valor para mensagem de erro: o tipo e o tamanho, nunca o conteudo.
/// Um segredo colado na chave errada (`api.tarefas_por_minuto: "ghp_..."`) voltaria
/// inteiro no «esperado inteiro, veio ...».
fn descricao(v: &Value) -> String {
    match v {
        Value::String(s) => format!("texto de {} caracteres", s.chars().count()),
        Value::Array(a) => format!("lista de {} itens", a.len()),
        Value::Object(m) => format!("objeto de {} chaves", m.len()),
        Value::Number(_) => "numero".into(),
        Value::Bool(_) => "booleano".into(),
        Value::Null => "null".into(),
    }
}

fn eh_falso(s: &str) -> bool {
    matches!(s, "0" | "false" | "no" | "off" | "nao" | "não")
}

/// O valor tipado do texto de uma variavel de ambiente (ou do `config definir`). Lista
/// tambem aceita `[...]` em JSON.
pub fn do_texto(c: &Chave, bruto: &str) -> Result<Value, String> {
    let s = bruto.trim();
    let esperado = || {
        format!(
            "esperado {}, veio texto de {} caracteres",
            c.tipo.nome(),
            s.chars().count()
        )
    };
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
                Err(format!(
                    "esperado um de {}, veio texto de {} caracteres",
                    opcoes.join("|"),
                    s.chars().count()
                ))
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
                _ => Err(format!(
                    "esperado um de {}, veio {}",
                    opcoes.join("|"),
                    descricao(v)
                )),
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
        Err(format!("esperado {tipo}, veio {}", descricao(v)))
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
                } else if let Some(e) = segredo_dentro(&format!("{chave}."), m) {
                    // Secao desconhecida com um segredo dentro: o aviso que importa e o do
                    // segredo, nao «secao desconhecida».
                    erros.push(e);
                } else {
                    erros.push(desconhecida(&chave));
                }
            }
            _ => out.push((chave, v.clone())),
        }
    }
}

/// Chave fora do catalogo com nome de segredo e valor preenchido: a recusa diz que e
/// segredo (e o tamanho), nao «chave desconhecida» com a sugestao -- quem escreveu
/// `github_token` no arquivo precisa ouvir que segredo mora no broker.
fn segredo_pelo_nome(chave: &str, v: &Value) -> Option<String> {
    let preenchido = match v {
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => a
            .iter()
            .any(|x| x.as_str().is_some_and(|s| !s.trim().is_empty())),
        _ => false,
    };
    if nome_de_segredo(chave) && preenchido && por_chave(chave).is_none_or(|c| !c.segredo()) {
        return Some(format!(
            "chave com nome de segredo ({}): segredo mora no SecretBroker, nunca no config.json",
            descricao(v)
        ));
    }
    None
}

/// O primeiro valor com nome de segredo dentro de uma secao, em qualquer profundidade.
fn segredo_dentro(caminho: &str, obj: &Map<String, Value>) -> Option<Erro> {
    for (k, v) in obj {
        let chave = format!("{caminho}{k}");
        if let Some(m) = segredo_pelo_nome(&chave, v) {
            return Some(Erro::novo(chave, m));
        }
        if let Some(m) = v.as_object() {
            if let Some(e) = segredo_dentro(&format!("{chave}."), m) {
                return Some(e);
            }
        }
    }
    None
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
    let mut perfis: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
    let mut perfil_ativo = None;
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
            CAMPO_PERFIS => match v.as_object() {
                Some(m) => {
                    for (nome, corpo_do_perfil) in m {
                        let rotulo = format!("{CAMPO_PERFIS}.{nome}");
                        if let Err(e) = nome_de_perfil_valido(nome) {
                            erros.push(Erro::novo(rotulo, e));
                            continue;
                        }
                        let Some(obj) = corpo_do_perfil.as_object() else {
                            erros.push(Erro::novo(rotulo, "perfil e um objeto de secoes"));
                            continue;
                        };
                        let chaves = valores_de(obj, &mut erros);
                        perfis.insert(nome.clone(), chaves);
                    }
                }
                None => erros.push(Erro::novo(k, "esperado objeto {nome: {secoes}}")),
            },
            CAMPO_PERFIL_ATIVO => match v {
                Value::String(s) => perfil_ativo = Some(s.clone()),
                Value::Null => {}
                _ => erros.push(Erro::novo(k, "esperado o nome de um perfil")),
            },
            _ => {
                corpo.insert(k.clone(), v.clone());
            }
        }
    }
    let valores = valores_de(&corpo, &mut erros);
    if let Some(p) = &perfil_ativo {
        if !perfis.contains_key(p) {
            erros.push(Erro::novo(
                CAMPO_PERFIL_ATIVO,
                format!("perfil `{p}` nao existe em {CAMPO_PERFIS}"),
            ));
        }
    }
    if erros.is_empty() {
        Ok(Arquivo {
            revisao,
            valores,
            perfis,
            perfil_ativo,
            ..Arquivo::default()
        })
    } else {
        Err(erros)
    }
}

/// As chaves definidas num corpo em secoes (a base do arquivo ou um perfil), conferidas
/// uma a uma; os erros se acumulam em `erros`.
fn valores_de(corpo: &Map<String, Value>, erros: &mut Vec<Erro>) -> BTreeMap<String, Value> {
    let mut pares = Vec::new();
    achatar("", corpo, &mut pares, erros);
    let mut valores = BTreeMap::new();
    for (chave, v) in pares {
        if let Some(m) = segredo_pelo_nome(&chave, &v) {
            erros.push(Erro::novo(chave, m));
            continue;
        }
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
    valores
}

/// Le e confere um `config.json`. Arquivo ausente e arquivo vazio de revisao 0.
pub fn ler_arquivo(caminho: &Path) -> Result<Arquivo, Vec<Erro>> {
    let rotulo = caminho.display().to_string();
    let bytes = match std::fs::read(caminho) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Arquivo {
                caminho: caminho.to_path_buf(),
                sha256: sha256_hex(b""),
                ..Arquivo::default()
            });
        }
        Err(e) => return Err(vec![Erro::novo(rotulo, e.to_string())]),
    };
    let doc: Value = serde_json::from_slice(&bytes)
        .map_err(|e| vec![Erro::novo(rotulo.clone(), format!("JSON invalido: {e}"))])?;
    let mut a = validar_documento(&doc).map_err(|es| {
        es.into_iter()
            .map(|e| Erro::novo(e.chave, format!("{} (em {rotulo})", e.motivo)))
            .collect::<Vec<_>>()
    })?;
    a.caminho = caminho.to_path_buf();
    a.sha256 = sha256_hex(&bytes);
    Ok(a)
}

/// O documento do arquivo, de volta em secoes (o que se grava).
pub fn documento(revisao: u64, valores: &BTreeMap<String, Value>) -> Value {
    documento_de(&Arquivo {
        revisao,
        valores: valores.clone(),
        ..Arquivo::default()
    })
}

fn em_secoes(valores: &BTreeMap<String, Value>) -> Map<String, Value> {
    let mut raiz = Map::new();
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
    raiz
}

/// O documento inteiro de um `Arquivo`: revisao, secoes, perfis e perfil ativo.
pub fn documento_de(a: &Arquivo) -> Value {
    let mut raiz = Map::new();
    raiz.insert(CAMPO_REVISAO.into(), Value::from(a.revisao));
    raiz.extend(em_secoes(&a.valores));
    if !a.perfis.is_empty() {
        let mut perfis = Map::new();
        for (nome, chaves) in &a.perfis {
            perfis.insert(nome.clone(), Value::Object(em_secoes(chaves)));
        }
        raiz.insert(CAMPO_PERFIS.into(), Value::Object(perfis));
    }
    if let Some(p) = &a.perfil_ativo {
        raiz.insert(CAMPO_PERFIL_ATIVO.into(), Value::String(p.clone()));
    }
    Value::Object(raiz)
}

/// O documento SEM a revisao nem nada da maquina: o que atravessa a sincronizacao entre
/// pastas. Cada valor passa de novo pela varredura de credencial -- o arquivo ja foi
/// recusado na leitura se trouxesse segredo, mas o que sai para outra maquina se confere
/// na saida, nao se confia na entrada.
pub fn exportar(a: &Arquivo) -> Result<Value, Vec<Erro>> {
    let mut erros = Vec::new();
    let conferir = |prefixo: &str, chaves: &BTreeMap<String, Value>, erros: &mut Vec<Erro>| {
        for (chave, v) in chaves {
            let rotulo = format!("{prefixo}{chave}");
            if let Some(m) = segredo_pelo_nome(chave, v) {
                erros.push(Erro::novo(rotulo, m));
                continue;
            }
            match por_chave(chave) {
                Some(c) => {
                    if let Err(m) = conferir_valor(c, v) {
                        erros.push(Erro::novo(rotulo, m));
                    }
                }
                None => erros.push(desconhecida(&rotulo)),
            }
        }
    };
    conferir("", &a.valores, &mut erros);
    for (nome, chaves) in &a.perfis {
        conferir(&format!("{CAMPO_PERFIS}.{nome}."), chaves, &mut erros);
    }
    if !erros.is_empty() {
        return Err(erros);
    }
    let mut doc = documento_de(a);
    if let Some(o) = doc.as_object_mut() {
        o.remove(CAMPO_REVISAO);
    }
    Ok(doc)
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
    // O perfil em vigor: a variavel (uma execucao) vence o campo do arquivo. Perfil que
    // nao existe e erro, nao «vale a base»: seguir calado faria uma restricao do perfil
    // deixar de valer sem ninguem ver -- a mesma regra do arquivo invalido.
    let var_perfil = por_chave(CHAVE_PERFIL)
        .map(|c| c.variavel.clone())
        .unwrap_or_default();
    let perfil_ativo = match ambiente(&var_perfil).filter(|v| !v.trim().is_empty()) {
        Some(nome) => Some((nome.trim().to_string(), Origem::Ambiente)),
        None => arq_pasta.perfil_ativo.clone().map(|n| (n, Origem::Pasta)),
    };
    let perfil: Option<&BTreeMap<String, Value>> = match &perfil_ativo {
        Some((nome, origem)) => match arq_pasta.perfis.get(nome) {
            Some(p) => Some(p),
            None => {
                erros.push(Erro::novo(
                    CHAVE_PERFIL,
                    format!(
                        "perfil `{nome}` ({}) nao existe em {} de {}",
                        origem.nome(),
                        CAMPO_PERFIS,
                        arq_pasta.caminho.display()
                    ),
                ));
                None
            }
        },
        None => None,
    };
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
        } else {
            // As fontes de arquivo, na ordem da constante: a primeira que define a chave ganha.
            let de = |o: Origem| -> Option<Value> {
                match o {
                    Origem::Projeto => arq_projeto.as_ref()?.valores.get(&c.chave).cloned(),
                    Origem::Perfil => perfil?.get(&c.chave).cloned(),
                    Origem::Pasta => arq_pasta.valores.get(&c.chave).cloned(),
                    Origem::Padrao => c
                        .padrao
                        .map(|p| do_texto(c, p).expect("padrao do catalogo conferido no teste")),
                    Origem::Ambiente | Origem::Ausente => None,
                }
            };
            PRECEDENCIA
                .iter()
                .skip(1)
                .find_map(|&o| {
                    de(o).map(|v| Efetivo {
                        valor: Some(v),
                        origem: o,
                    })
                })
                .unwrap_or(Efetivo {
                    valor: None,
                    origem: Origem::Ausente,
                })
        };
        efetivos.insert(c.chave.clone(), efetivo);
    }
    if erros.is_empty() {
        Ok(Configuracao {
            efetivos,
            pasta: arq_pasta,
            projeto: arq_projeto,
            perfil_ativo,
        })
    } else {
        Err(erros)
    }
}

/// Recusa de uma mudanca: conflito de revisao, ou a lista de erros por chave.
#[derive(Debug)]
pub enum Recusa {
    Conflito { atual: String },
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
    /// O token de concorrencia que o cliente viu (`Configuracao::revisao`); `None`: o de
    /// agora (a CLI, que nao tem If-Match).
    pub esperada: Option<&'a str>,
    /// `Some(nome)`: as mudancas vao para o perfil, nao para a base do arquivo.
    pub perfil: Option<&'a str>,
}

/// Aplica `mudancas` (`chave -> valor`, `null` remove) ao arquivo do alvo e grava pela
/// mesma `gravar_revisado` do config-runtime. Devolve o novo token de concorrencia.
///
/// A conferencia do token e a leitura do arquivo acontecem DENTRO da trava e contra o
/// disco, nao contra o `alvo.atual` que o processo carregou antes: o servidor e a CLI sao
/// processos diferentes, e um `If-Match` conferido contra o cache do servidor passaria
/// mesmo depois de a CLI ter gravado.
pub fn definir(alvo: Alvo<'_>, mudancas: &Map<String, Value>) -> Result<String, Recusa> {
    let atual = alvo.atual;
    let perfil = alvo.perfil.map(str::to_string);
    gravar_com(alvo, &mut |arq: &mut Arquivo| {
        let mut erros = Vec::new();
        let valores = match &perfil {
            Some(nome) => match arq.perfis.get_mut(nome) {
                Some(p) => p,
                None => {
                    return Err(vec![Erro::novo(
                        CAMPO_PERFIS,
                        format!("perfil `{nome}` nao existe (crie com `config perfil criar`)"),
                    )]);
                }
            },
            None => &mut arq.valores,
        };
        for (chave, v) in mudancas {
            if let Some(m) = segredo_pelo_nome(chave, v) {
                erros.push(Erro::novo(chave, m));
                continue;
            }
            let Some(c) = por_chave(chave) else {
                erros.push(desconhecida(chave));
                continue;
            };
            if let Some(m) = recusa_fora_do_arquivo(c) {
                erros.push(Erro::novo(chave, m));
                continue;
            }
            if atual.origem(chave) == Origem::Ambiente {
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
        if erros.is_empty() {
            Ok(())
        } else {
            Err(erros)
        }
    })
}

/// Cria um perfil vazio (ou copia das chaves da base, com `copiar_base`). Nome repetido
/// e recusa: sobrescrever um perfil calado apagaria o que ele tinha.
pub fn perfil_criar(alvo: Alvo<'_>, nome: &str, copiar_base: bool) -> Result<String, Recusa> {
    let nome = nome.trim().to_string();
    gravar_com(alvo, &mut |arq: &mut Arquivo| {
        nome_de_perfil_valido(&nome).map_err(|m| vec![Erro::novo(CAMPO_PERFIS, m)])?;
        if arq.perfis.contains_key(&nome) {
            return Err(vec![Erro::novo(
                format!("{CAMPO_PERFIS}.{nome}"),
                "perfil ja existe",
            )]);
        }
        let chaves = if copiar_base {
            arq.valores.clone()
        } else {
            BTreeMap::new()
        };
        arq.perfis.insert(nome.clone(), chaves);
        Ok(())
    })
}

/// Torna `nome` o perfil ativo do arquivo (`None`: nenhum). Perfil que nao existe e
/// recusa com a lista dos que existem.
pub fn perfil_usar(alvo: Alvo<'_>, nome: Option<&str>) -> Result<String, Recusa> {
    let nome = nome.map(|n| n.trim().to_string());
    gravar_com(alvo, &mut |arq: &mut Arquivo| {
        if let Some(n) = &nome {
            if !arq.perfis.contains_key(n) {
                let existem: Vec<&String> = arq.perfis.keys().collect();
                return Err(vec![Erro::novo(
                    CAMPO_PERFIL_ATIVO,
                    format!(
                        "perfil `{n}` nao existe; existem: {}",
                        if existem.is_empty() {
                            "(nenhum)".to_string()
                        } else {
                            existem
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    ),
                )]);
            }
        }
        arq.perfil_ativo = nome.clone();
        Ok(())
    })
}

/// Substitui o conteudo do arquivo pelo documento vindo de outra maquina (o que
/// `exportar` produziu la), conferindo ANTES, sob a trava, que o disco esta no `sha_base`
/// que quem sincroniza viu: se mudou, e conflito, com o SHA de agora. A revisao local
/// continua contando -- e deste arquivo, nao do outro.
pub fn importar(alvo: Alvo<'_>, doc: &Value, sha_base: &str) -> Result<String, Recusa> {
    let vindo = validar_documento(doc).map_err(Recusa::Invalida)?;
    let alvo = Alvo {
        esperada: None,
        ..alvo
    };
    let sha_agora = sha_em_disco(alvo.arquivo).map_err(Recusa::Falha)?;
    if sha_agora != sha_base {
        return Err(Recusa::Conflito { atual: sha_agora });
    }
    gravar_com(alvo, &mut |arq: &mut Arquivo| {
        arq.valores = vindo.valores.clone();
        arq.perfis = vindo.perfis.clone();
        arq.perfil_ativo = vindo.perfil_ativo.clone();
        Ok(())
    })
}

/// A gravacao comum a toda mudanca do arquivo: le o disco sob a trava, confere o token,
/// deixa `aplicar` mexer no `Arquivo`, e grava pela `gravar_revisado`. Devolve o token.
fn gravar_com(
    alvo: Alvo<'_>,
    aplicar: &mut dyn FnMut(&mut Arquivo) -> Result<(), Vec<Erro>>,
) -> Result<String, Recusa> {
    // Os outros arquivos que entram no token (o que nao e o alvo), lidos do disco na hora.
    let outros: Vec<(bool, PathBuf)> = std::iter::once((true, alvo.atual.pasta.caminho.clone()))
        .chain(
            alvo.atual
                .projeto
                .iter()
                .map(|p| (false, p.caminho.clone())),
        )
        .collect();
    let token_do_disco = |sha_do_alvo: &str| -> Result<String, Recusa> {
        let mut pasta = String::new();
        let mut projeto = None;
        for (eh_pasta, caminho) in &outros {
            let sha = if caminho == alvo.arquivo {
                sha_do_alvo.to_string()
            } else {
                sha_em_disco(caminho).map_err(Recusa::Falha)?
            };
            if *eh_pasta {
                pasta = sha;
            } else {
                projeto = Some(sha);
            }
        }
        Ok(token(&pasta, projeto.as_deref()))
    };
    let validar = |v: &Value| {
        validar_documento(v)
            .map(|_| ())
            .map_err(|e| ConfigError::Invalid(em_texto(&e)))
    };
    let historico = historico_de(alvo.arquivo);
    let mut invalida: Option<Vec<Erro>> = None;
    let mut montar = |em_disco: Option<&Value>| -> Result<Value, ConfigError> {
        // O hash e dos bytes do arquivo, nao do JSON reserializado: le-se de novo, ja
        // com a trava tomada.
        let sha_alvo = sha_em_disco(alvo.arquivo).map_err(ConfigError::Invalid)?;
        let atual = token_do_disco(&sha_alvo).map_err(|e| ConfigError::Invalid(e.to_string()))?;
        if let Some(e) = alvo.esperada {
            if e != atual.as_str() {
                return Err(ConfigError::ConflitoDeToken { atual });
            }
        }
        let mut arq = match em_disco {
            Some(d) => validar_documento(d).map_err(|e| ConfigError::Invalid(em_texto(&e)))?,
            None => Arquivo::default(),
        };
        if let Err(erros) = aplicar(&mut arq) {
            invalida = Some(erros);
            return Err(ConfigError::Invalid("recusada".into()));
        }
        Ok(documento_de(&arq))
    };
    let r = gravar_revisado(
        alvo.arquivo,
        &historico,
        CAMPO_REVISAO,
        &validar,
        &mut montar,
    );
    match r {
        Ok(_) => {}
        Err(ConfigError::ConflitoDeToken { atual }) => return Err(Recusa::Conflito { atual }),
        Err(ConfigError::Invalid(_)) if invalida.is_some() => {
            return Err(Recusa::Invalida(invalida.unwrap_or_default()));
        }
        Err(outro) => return Err(Recusa::Falha(outro.to_string())),
    }
    let sha_alvo = sha_em_disco(alvo.arquivo).map_err(Recusa::Falha)?;
    token_do_disco(&sha_alvo)
}

/// O historico das revisoes, ao lado do arquivo.
pub fn historico_de(arquivo: &Path) -> PathBuf {
    arquivo
        .parent()
        .unwrap_or(Path::new("."))
        .join("config-historico")
}

// --- o leitor do PROCESSO, para quem nao e o agente ----------------------------------
//
// As pontes de plugin, o SDK, o no de dispositivo, as CLIs de missao e o desktop nao tem o
// `phxclaw-agent` (nem o `--pasta` dele, nem a lista de projetos confiados). Leem por aqui:
// a MESMA carga, da pasta FIXADA pelo processo (o agente fixa a dele ao arrancar) ou de
// `PHXCLAW_HOME`/`var/agente`, SEM o arquivo do projeto -- confiar num projeto e
// julgamento do agente, e um processo que nao o tem nao le o `.phxclaw/config.json` de um
// repositorio qualquer. Arquivo invalido vira UM aviso no stderr por processo e `None`:
// quem pode recusar na porta (a CLI do agente) recusa la.

fn ambiente_do_processo(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

fn pasta_fixada() -> &'static Mutex<Option<PathBuf>> {
    static P: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

/// Fixa a pasta do processo (o `--pasta` da CLI): a partir daqui `do_processo` le dela.
pub fn fixar_pasta_do_processo(pasta: &Path) {
    *pasta_fixada().lock().unwrap_or_else(|p| p.into_inner()) = Some(pasta.to_path_buf());
}

/// A pasta do agente deste processo: a fixada, senao `PHXCLAW_HOME`, senao `var/agente`.
/// Le a variavel direto porque e ela que LOCALIZA o arquivo: passar pela carga seria
/// carregar a configuracao para saber de onde carrega-la.
pub fn pasta_do_processo() -> PathBuf {
    if let Some(p) = pasta_fixada()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
    {
        return p;
    }
    let var = por_chave(CHAVE_PASTA)
        .map(|c| c.variavel.clone())
        .unwrap_or_default();
    ambiente_do_processo(&var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(PASTA_PADRAO))
}

/// A configuracao do processo (ambiente > perfil > pasta > padrao; sem projeto).
pub fn do_processo() -> Result<Configuracao, Vec<Erro>> {
    carregar(
        &ambiente_do_processo,
        &pasta_do_processo().join(ARQUIVO),
        None,
    )
}

fn do_processo_tolerante() -> Option<Configuracao> {
    match do_processo() {
        Ok(c) => Some(c),
        Err(e) => {
            static AVISADO: OnceLock<()> = OnceLock::new();
            AVISADO.get_or_init(|| eprintln!("aviso: config.json: {}", em_texto(&e)));
            None
        }
    }
}

pub fn texto_do_processo(chave: &str) -> Option<String> {
    do_processo_tolerante()?
        .texto(chave)
        .filter(|v| !v.trim().is_empty())
}

pub fn caminho_do_processo(chave: &str) -> Option<PathBuf> {
    texto_do_processo(chave).map(PathBuf::from)
}

pub fn inteiro_do_processo(chave: &str) -> Option<i64> {
    do_processo_tolerante()?.inteiro(chave)
}

pub fn booleano_do_processo(chave: &str) -> Option<bool> {
    do_processo_tolerante()?.booleano(chave)
}

pub fn lista_do_processo(chave: &str) -> Option<Vec<String>> {
    do_processo_tolerante()?.lista(chave)
}

/// Um segredo do catalogo, SO do ambiente (o broker e do agente). `valor` devolve `None`
/// para segredo de proposito; quem precisa do texto le por aqui.
pub fn segredo_do_processo(chave: &str) -> Option<String> {
    let c = por_chave(chave).unwrap_or_else(|| panic!("chave fora do catalogo: {chave}"));
    debug_assert!(c.segredo(), "{chave} nao e segredo no catalogo");
    ambiente_do_processo(&c.variavel).map(|v| v.trim().to_string())
}

/// O nome da variavel de ambiente da chave, para a mensagem ao operador. Do catalogo.
pub fn variavel(chave: &str) -> &'static str {
    por_chave(chave)
        .map(|c| c.variavel.as_str())
        .unwrap_or_else(|| panic!("chave fora do catalogo: {chave}"))
}
