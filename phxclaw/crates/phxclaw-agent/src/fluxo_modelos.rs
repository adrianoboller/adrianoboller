//! A galeria de modelos de fluxo: fluxos prontos, versionados em `modelos/fluxos/*.json` e
//! embutidos no binario (`include_str!`), para `phxclaw fluxo modelos` listar e `phxclaw
//! fluxo usar MODELO DESTINO` copiar para o projeto. E o equivalente do «templates» do n8n.
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Embutido, nao lido de uma pasta.** O binario instalado nao tem o repositorio ao lado,
//!   e uma galeria que depende de um caminho relativo some na maquina do cliente. O teste
//!   `galeria_embutida_e_a_pasta` compara a lista embutida com a pasta: modelo novo que
//!   ninguem registrou aqui reprova, em vez de existir so no disco.
//! - **O arquivo do modelo e um envelope**: `{"modelo": {nome, descricao, etiquetas,
//!   credenciais}, "fluxo": {...}}`. O `fluxo` e o formato de sempre (`fluxos::validar` o
//!   julga igual a qualquer outro); o cabecalho e o que so a galeria precisa e nao pode morar
//!   no fluxo, porque `usar` grava SO o fluxo.
//! - **Credencial entra por NOME, nunca por valor.** `credenciais` lista o que o operador tem
//!   de guardar no broker antes de rodar («smtp», «canal-mensagens»). O arquivo inteiro passa
//!   pelo motor unico `phxclaw_types::segredo` (nome de campo e forma do valor) e e recusado
//!   se trouxer qualquer um dos dois. Alem disso, o modelo que USA uma ferramenta que pede
//!   credencial tem de declarar o nome dela (`credenciais_das_ferramentas`): a lista que
//!   mente por omissao faria o operador descobrir a falta na primeira execucao.
//! - **`usar` nao sobrescreve** (o `gravar_importado` do motor): copiar por cima do fluxo que
//!   o operador ja editou perderia o trabalho dele calado.

use crate::fluxos::{self, Fluxo};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

/// Os modelos, na ordem da listagem. O nome e o do arquivo sem `.json`.
const GALERIA: &[(&str, &str)] = &[
    (
        "aprovacao-humana-antes-de-agir",
        include_str!("../../../modelos/fluxos/aprovacao-humana-antes-de-agir.json"),
    ),
    (
        "ata-de-reuniao",
        include_str!("../../../modelos/fluxos/ata-de-reuniao.json"),
    ),
    (
        "formulario-de-contato",
        include_str!("../../../modelos/fluxos/formulario-de-contato.json"),
    ),
    (
        "monitor-de-testes",
        include_str!("../../../modelos/fluxos/monitor-de-testes.json"),
    ),
    (
        "pesquisa-com-citacoes",
        include_str!("../../../modelos/fluxos/pesquisa-com-citacoes.json"),
    ),
    (
        "ponte-n8n",
        include_str!("../../../modelos/fluxos/ponte-n8n.json"),
    ),
    (
        "previsao-do-tempo-com-aviso",
        include_str!("../../../modelos/fluxos/previsao-do-tempo-com-aviso.json"),
    ),
    (
        "processar-em-lotes-com-erro",
        include_str!("../../../modelos/fluxos/processar-em-lotes-com-erro.json"),
    ),
    (
        "relatorio-do-banco",
        include_str!("../../../modelos/fluxos/relatorio-do-banco.json"),
    ),
    (
        "resumo-diario-por-email",
        include_str!("../../../modelos/fluxos/resumo-diario-por-email.json"),
    ),
    (
        "revisao-de-codigo-com-aviso",
        include_str!("../../../modelos/fluxos/revisao-de-codigo-com-aviso.json"),
    ),
    (
        "triagem-por-prioridade",
        include_str!("../../../modelos/fluxos/triagem-por-prioridade.json"),
    ),
];

/// Teto do arquivo de um modelo: galeria e texto lido por gente.
pub const MAX_BYTES_MODELO: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arquivo {
    modelo: Cabecalho,
    fluxo: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cabecalho {
    nome: String,
    descricao: String,
    #[serde(default)]
    etiquetas: Vec<String>,
    #[serde(default)]
    credenciais: Vec<String>,
}

/// Um modelo lido e conferido.
#[derive(Debug, Clone)]
pub struct Modelo {
    pub nome: String,
    pub descricao: String,
    pub etiquetas: Vec<String>,
    /// Nomes dos segredos que o operador guarda no broker antes de rodar.
    pub credenciais: Vec<String>,
    /// O fluxo que `usar` grava, ja validado pelo motor.
    pub fluxo: Fluxo,
}

/// Ferramenta -> nome da credencial que ela exige. A fonte e o que cada ferramenta le do
/// broker/config (`email.rs`, `canais`, `n8n.rs`, `sistema.rs`): se uma ferramenta nova
/// passar a exigir credencial, o modelo que a usa so entra declarando.
const CREDENCIAL_DA_FERRAMENTA: &[(&str, &str)] = &[
    ("send_email", "smtp"),
    ("channel_send", "canal-mensagens"),
    ("n8n_workflow", "n8n"),
    ("postgres", "postgres"),
    ("postgres_write", "postgres"),
];

/// Os nomes de credencial que as ferramentas do fluxo exigem.
pub fn credenciais_das_ferramentas(f: &Fluxo) -> BTreeSet<&'static str> {
    f.passos
        .iter()
        .filter_map(|p| p.ferramenta.as_deref())
        .filter_map(|t| {
            CREDENCIAL_DA_FERRAMENTA
                .iter()
                .find(|(ferr, _)| *ferr == t)
                .map(|(_, c)| *c)
        })
        .collect()
}

fn nome_de_modelo_valido(n: &str) -> bool {
    (3..=64).contains(&n.len())
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !n.starts_with('-')
        && !n.ends_with('-')
}

fn nome_de_credencial_valido(n: &str) -> bool {
    (1..=64).contains(&n.len())
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
        && n.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn sem_repeticao(lista: &[String], o_que: &str) -> Result<(), String> {
    let mut vistos = BTreeSet::new();
    match lista.iter().find(|x| !vistos.insert(x.as_str())) {
        Some(x) => Err(format!("{o_que} repetido: {x:?}")),
        None => Ok(()),
    }
}

/// Le e confere o texto de um modelo. Recusa: arquivo grande, campo desconhecido, nome fora
/// do padrao ou diferente do nome do fluxo, credencial por VALOR (nome de campo ou forma),
/// credencial exigida pelas ferramentas e nao declarada, e fluxo que o motor recusa.
pub fn ler_modelo(texto: &str) -> Result<Modelo, String> {
    if texto.len() > MAX_BYTES_MODELO {
        return Err(format!("modelo passa de {MAX_BYTES_MODELO} bytes"));
    }
    // O arquivo INTEIRO, antes de qualquer interpretacao: a guarda de forma nao pode
    // depender de o campo estar onde se esperava.
    if phxclaw_types::segredo::texto_tem_credencial(texto) {
        return Err(
            "o modelo traz uma credencial pela forma (chave de provedor, JWT, PEM, URL com senha \
ou Basic/Bearer): modelo guarda o NOME da credencial, nunca o valor"
                .into(),
        );
    }
    let a: Arquivo = serde_json::from_str(texto).map_err(|e| format!("modelo invalido: {e}"))?;
    if fluxos::variavel_parece_segredo("fluxo", &a.fluxo) {
        return Err(
            "o fluxo do modelo tem campo com nome de segredo ou valor com forma de segredo: \
modelo guarda o NOME da credencial em `credenciais`, nunca o valor"
                .into(),
        );
    }
    let c = a.modelo;
    if !nome_de_modelo_valido(&c.nome) {
        return Err(format!(
            "nome de modelo invalido: {:?} (minusculas, digitos e '-', de 3 a 64)",
            c.nome
        ));
    }
    if c.descricao.trim().is_empty() || c.descricao.chars().count() > 400 {
        return Err(format!(
            "modelo {}: descricao de 1 a 400 caracteres",
            c.nome
        ));
    }
    sem_repeticao(&c.etiquetas, "etiqueta")?;
    sem_repeticao(&c.credenciais, "credencial")?;
    if let Some(x) = c.credenciais.iter().find(|x| !nome_de_credencial_valido(x)) {
        return Err(format!(
            "modelo {}: nome de credencial invalido: {x:?} (minusculas, digitos, '-' e '_')",
            c.nome
        ));
    }
    // As etiquetas do cabecalho viram as do fluxo (e a `listar` as filtra); as que o proprio
    // fluxo ja trouxer ficam, sem repeticao.
    let mut fluxo = a.fluxo;
    let obj = fluxo
        .as_object_mut()
        .ok_or("modelo: `fluxo` precisa ser um objeto")?;
    let mut etiquetas: Vec<String> = obj
        .get("etiquetas")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    for e in &c.etiquetas {
        if !etiquetas.contains(e) {
            etiquetas.push(e.clone());
        }
    }
    obj.insert("etiquetas".into(), serde_json::json!(etiquetas));
    let f = fluxos::ler(&fluxo.to_string()).map_err(|e| format!("modelo {}: {e}", c.nome))?;
    if f.nome != c.nome {
        return Err(format!(
            "modelo {}: o nome do fluxo ({:?}) precisa ser o do modelo",
            c.nome, f.nome
        ));
    }
    let faltam: Vec<&str> = credenciais_das_ferramentas(&f)
        .into_iter()
        .filter(|n| !c.credenciais.iter().any(|d| d == n))
        .collect();
    if !faltam.is_empty() {
        return Err(format!(
            "modelo {}: as ferramentas pedem credencial que ele nao declara: {}",
            c.nome,
            faltam.join(", ")
        ));
    }
    Ok(Modelo {
        nome: c.nome,
        descricao: c.descricao,
        etiquetas: f.etiquetas.clone(),
        credenciais: c.credenciais,
        fluxo: f,
    })
}

/// Toda a galeria, lida e conferida. Um modelo que nao le para a listagem inteira: galeria
/// com modelo quebrado e bug de quem a publicou, e o teste o acusa antes do cliente.
pub fn galeria() -> Result<Vec<Modelo>, String> {
    GALERIA
        .iter()
        .map(|(nome, texto)| {
            let m = ler_modelo(texto).map_err(|e| format!("{nome}.json: {e}"))?;
            if m.nome != *nome {
                return Err(format!(
                    "{nome}.json: o arquivo traz o modelo {:?} (o nome do arquivo e o do modelo)",
                    m.nome
                ));
            }
            Ok(m)
        })
        .collect()
}

/// Os nomes embutidos, sem ler nada.
pub fn nomes() -> Vec<&'static str> {
    GALERIA.iter().map(|(n, _)| *n).collect()
}

pub fn achar(nome: &str) -> Result<Modelo, String> {
    let (_, texto) = GALERIA.iter().find(|(n, _)| *n == nome).ok_or_else(|| {
        format!(
            "modelo {nome:?} nao existe; os que existem: {}",
            nomes().join(", ")
        )
    })?;
    ler_modelo(texto)
}

/// Copia o modelo para `destino` como um fluxo do projeto (rascunho; sem versao publicada o
/// arquivo e o publicado implicito). Nao sobrescreve.
pub fn usar(nome: &str, destino: &Path) -> Result<Modelo, String> {
    let m = achar(nome)?;
    fluxos::gravar_importado(&m.fluxo, destino)?;
    Ok(m)
}
