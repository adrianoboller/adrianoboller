//! Rascunho x publicada: o fluxo em arquivo (`ARQ.json`) e o RASCUNHO, e `publicar` congela
//! uma versao numerada ao lado dele. O gatilho, a agenda e o sub-fluxo rodam a PUBLICADA;
//! so a execucao manual da CLI (`fluxo rodar`) le o rascunho, que e para testar a edicao.
//!
//! Onde mora (formato 1, descrito em `docs/N8N.md` 8g):
//!
//! ```text
//! ARQ.json                    o rascunho (o que o operador edita; o formato de sempre)
//! .ARQ.versoes/indice.json    {formato, publicada, versoes:[{numero, sha256, em, nota, voltou_a}]}
//! .ARQ.versoes/v0001.json     a definicao congelada (pins ja dentro, chaves em ordem)
//! ```
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Fluxo sem pasta de versoes e PUBLICADO IMPLICITO.** Quem nunca publicou continua
//!   rodando o arquivo, exatamente como antes: guarda nova entra pedida, nao imposta. A
//!   pasta comeca no primeiro `publicar`, e desse dia em diante o arquivo deixa de ser o que
//!   o gatilho roda.
//! - **O hash e o do motor**: `fluxos::assinatura`, o mesmo que o `Relatorio.fluxo_sha256`
//!   grava. Por isso o relatorio de uma execucao aponta a versao sem campo novo (`versao_do_sha`),
//!   e a versao e o que se rodou, nao o que se achava que rodava.
//! - **O historico so cresce.** `voltar` nao apaga nem renumera: publica de novo a definicao
//!   de uma versao antiga como versao NOVA (com `voltou_a`). Apagar o meio do historico
//!   faria o numero de um relatorio antigo apontar para outra definicao.
//! - **Nao ha mais de um `publicada` por vez, nem versao sem arquivo.** O arquivo da versao
//!   entra ANTES do indice (queda no meio deixa um arquivo orfao, que o proximo `publicar`
//!   sobrescreve; o contrario deixaria o gatilho apontando para o que nao existe) e a leitura
//!   confere o hash do arquivo contra o do indice: versao adulterada ou truncada FECHA, ela
//!   nunca cai de volta no rascunho (cair no rascunho seria rodar em producao o que ninguem
//!   publicou).
//! - **Diverge do n8n** (que guarda a versao no banco, junto do fluxo) por uma restricao
//!   nossa: o fluxo aqui e arquivo de projeto, e arquivo ao lado do arquivo atravessa o git
//!   (`fluxo_git`) e o `historico.rs` sem tabela nem migracao.

use crate::api::{ApiState, Criada, Recusa};
use crate::fluxos::{self, Fluxo};
use axum::http::StatusCode;
use phxclaw_types::arquivo::{gravar_atomico, trava_de, travar};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Formato do `indice.json`. Futuro e recusado por quem le (nao se roda o que nao se entende).
pub const FORMATO_INDICE: u8 = 1;

/// Teto de versoes por fluxo: o nome do arquivo tem quatro digitos (`v0001`) e o historico
/// nao pode crescer sem fim por um laco de `publicar`.
pub const MAX_VERSOES: usize = 9_999;

/// Teto da nota de uma versao.
pub const MAX_NOTA: usize = 200;

const ARQUIVO_INDICE: &str = "indice.json";

/// Uma versao publicada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versao {
    pub numero: u32,
    /// `fluxos::assinatura` da definicao congelada.
    pub sha256: String,
    /// RFC 3339 do momento em que foi publicada.
    pub em: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nota: String,
    /// Esta versao e a definicao da versao N publicada de novo (`voltar`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voltou_a: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Indice {
    pub formato: u8,
    /// O numero da versao que o gatilho roda.
    pub publicada: u32,
    pub versoes: Vec<Versao>,
}

impl Indice {
    pub fn atual(&self) -> Option<&Versao> {
        self.versoes.iter().find(|v| v.numero == self.publicada)
    }
}

/// `x.json` -> `.x.versoes`: oculta, para `fluxos::listar` (que pula nome com ponto) nao a
/// tomar por uma pasta de fluxos.
pub fn pasta_de(arq: &Path) -> PathBuf {
    let base = arq
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    arq.with_file_name(format!(".{base}.versoes"))
}

pub fn arquivo_da_versao(arq: &Path, numero: u32) -> PathBuf {
    pasta_de(arq).join(format!("v{numero:04}.json"))
}

fn arquivo_do_indice(arq: &Path) -> PathBuf {
    pasta_de(arq).join(ARQUIVO_INDICE)
}

/// O texto canonico de um valor: chaves em ordem em qualquer profundidade, indentado, com
/// quebra final. Estavel para o diff do git e para o hash do arquivo.
pub fn canonico(v: &Value) -> String {
    let mut t = serde_json::to_string_pretty(&fluxos::ordenado(v)).unwrap_or_default();
    t.push('\n');
    t
}

/// O indice do fluxo, ou `None` quando ele nunca foi publicado (o publicado implicito).
/// A pasta SEM indice, formato futuro ou numeracao quebrada e erro: nao se adivinha.
pub fn ler_indice(arq: &Path) -> Result<Option<Indice>, String> {
    let pasta = pasta_de(arq);
    if !pasta.exists() {
        return Ok(None);
    }
    let ind = arquivo_do_indice(arq);
    let texto = std::fs::read_to_string(&ind).map_err(|e| {
        format!(
            "{}: {e} (a pasta de versoes existe e o indice nao le: o fluxo nao roda o rascunho no lugar)",
            ind.display()
        )
    })?;
    let i: Indice = serde_json::from_str(&texto)
        .map_err(|e| format!("{}: indice invalido: {e}", ind.display()))?;
    if i.formato > FORMATO_INDICE {
        return Err(format!(
            "{}: indice no formato {}, e este binario le ate o {FORMATO_INDICE}",
            ind.display(),
            i.formato
        ));
    }
    if i.versoes.is_empty() {
        return Err(format!("{}: indice sem nenhuma versao", ind.display()));
    }
    for (n, v) in i.versoes.iter().enumerate() {
        if v.numero as usize != n + 1 {
            return Err(format!(
                "{}: a versao na posicao {} esta numerada {} (o historico so cresce, de 1 em 1)",
                ind.display(),
                n + 1,
                v.numero
            ));
        }
    }
    if i.atual().is_none() {
        return Err(format!(
            "{}: a publicada ({}) nao esta entre as versoes",
            ind.display(),
            i.publicada
        ));
    }
    Ok(Some(i))
}

fn gravar_indice(arq: &Path, i: &Indice) -> Result<(), String> {
    let corpo = serde_json::to_string_pretty(i).map_err(|e| e.to_string())? + "\n";
    let ind = arquivo_do_indice(arq);
    gravar_atomico(&ind, corpo.as_bytes()).map_err(|e| format!("{}: {e}", ind.display()))
}

/// A definicao congelada de uma versao, conferida contra o hash do indice.
pub fn ler_versao(arq: &Path, v: &Versao) -> Result<Fluxo, String> {
    let caminho = arquivo_da_versao(arq, v.numero);
    let texto =
        std::fs::read_to_string(&caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    let f: Fluxo = serde_json::from_str(&texto)
        .map_err(|e| format!("{}: versao invalida: {e}", caminho.display()))?;
    fluxos::validar(&f).map_err(|e| format!("{}: {e}", caminho.display()))?;
    if fluxos::assinatura(&f) != v.sha256 {
        return Err(format!(
            "{}: o conteudo nao bate com o hash da versao {} (editada a mao?); republique ou volte a outra versao",
            caminho.display(),
            v.numero
        ));
    }
    Ok(f)
}

/// Onde o gatilho le o fluxo: o proprio arquivo (publicado implicito) ou o arquivo da versao
/// publicada, ja conferido. O caminho devolvido serve a quem so sabe receber caminho
/// (`api::criar_fluxo_com`), sem que o rascunho possa entrar no meio.
pub fn caminho_publicado(arq: &Path) -> Result<PathBuf, String> {
    match ler_indice(arq)? {
        None => Ok(arq.to_path_buf()),
        Some(i) => {
            let v = i.atual().ok_or("indice sem a publicada")?;
            ler_versao(arq, v)?;
            Ok(arquivo_da_versao(arq, v.numero))
        }
    }
}

/// O fluxo que roda em producao: a publicada, ou o arquivo se nunca foi publicado.
pub fn ler_publicado(arq: &Path) -> Result<Fluxo, String> {
    match ler_indice(arq)? {
        None => fluxos::ler_arquivo(arq),
        Some(i) => ler_versao(arq, i.atual().ok_or("indice sem a publicada")?),
    }
}

/// O numero da versao cujo hash e `sha` (o `Relatorio.fluxo_sha256` de uma execucao).
pub fn versao_do_sha(arq: &Path, sha: &str) -> Result<Option<u32>, String> {
    Ok(ler_indice(arq)?.and_then(|i| {
        i.versoes
            .iter()
            .rev()
            .find(|v| v.sha256 == sha)
            .map(|v| v.numero)
    }))
}

fn conferir_nota(nota: &str) -> Result<String, String> {
    let n = nota.trim();
    if n.chars().count() > MAX_NOTA {
        return Err(format!("a nota passa de {MAX_NOTA} caracteres"));
    }
    if phxclaw_types::segredo::texto_tem_credencial(n) {
        return Err("a nota parece trazer uma credencial: segredo nao entra em historico".into());
    }
    Ok(n.to_string())
}

/// Congela o rascunho como a proxima versao e a torna a publicada. O rascunho e lido pelo
/// leitor de sempre (`fluxos::ler_arquivo`: pins ao lado, validacao): fluxo invalido nao e
/// publicado. Recusa quando o rascunho e igual a publicada (versao vazia nao e historico).
pub fn publicar(arq: &Path, nota: &str) -> Result<Versao, String> {
    let nota = conferir_nota(nota)?;
    let f = fluxos::ler_arquivo(arq)?;
    let sha = fluxos::assinatura(&f);
    let corpo = canonico(&serde_json::to_value(&f).map_err(|e| e.to_string())?);
    acrescentar(arq, sha, corpo, nota, None, true)
}

/// A publicada passa a ser a definicao da versao `n`, como versao NOVA no fim do historico.
pub fn voltar(arq: &Path, n: u32) -> Result<Versao, String> {
    let i = ler_indice(arq)?.ok_or("o fluxo nunca foi publicado: nao ha versao para voltar")?;
    let alvo = i
        .versoes
        .get((n as usize).wrapping_sub(1))
        .ok_or_else(|| format!("a versao {n} nao existe (ha {})", i.versoes.len()))?;
    let f = ler_versao(arq, alvo)?;
    let corpo = canonico(&serde_json::to_value(&f).map_err(|e| e.to_string())?);
    acrescentar(
        arq,
        alvo.sha256.clone(),
        corpo,
        format!("voltou a v{n}"),
        Some(n),
        false,
    )
}

/// O trabalho comum de `publicar` e `voltar`, sob a trava do fluxo: dois processos
/// publicando ao mesmo tempo nao podem pegar o mesmo numero. A trava mora AO LADO do
/// arquivo do fluxo e nao dentro da pasta de versoes: a pasta so passa a existir inteira
/// (ver `criar_pasta_com_a_primeira`), e um leitor nunca a ve sem indice.
fn acrescentar(
    arq: &Path,
    sha: String,
    corpo: String,
    nota: String,
    voltou_a: Option<u32>,
    exigir_diferente: bool,
) -> Result<Versao, String> {
    let trava = trava_de(arq);
    let _trava = travar(&trava).map_err(|e| format!("{}: {e}", trava.display()))?;
    // Lido DEPOIS da trava: o que outro processo publicou ate agora ja conta.
    let existente = ler_indice(arq)?;
    let primeira = existente.is_none();
    let mut i = existente.unwrap_or(Indice {
        formato: FORMATO_INDICE,
        publicada: 0,
        versoes: vec![],
    });
    if let Some(atual) = i.atual()
        && atual.sha256 == sha
    {
        return Err(if exigir_diferente {
            format!(
                "o rascunho e igual a versao publicada (v{}); nada a publicar",
                atual.numero
            )
        } else {
            format!("a versao {} ja e a publicada", atual.numero)
        });
    }
    if i.versoes.len() >= MAX_VERSOES {
        return Err(format!("o fluxo ja tem {MAX_VERSOES} versoes"));
    }
    let numero = i.versoes.len() as u32 + 1;
    let v = Versao {
        numero,
        sha256: sha,
        em: fluxos::agora().to_rfc3339(),
        nota,
        voltou_a,
    };
    i.versoes.push(v.clone());
    i.publicada = numero;
    if primeira {
        criar_pasta_com_a_primeira(arq, &corpo, &i)?;
    } else {
        // O arquivo da versao ANTES do indice: queda no meio deixa um arquivo orfao que a
        // proxima publicacao sobrescreve, e o indice velho continua valendo inteiro.
        let arquivo = arquivo_da_versao(arq, numero);
        gravar_atomico(&arquivo, corpo.as_bytes())
            .map_err(|e| format!("{}: {e}", arquivo.display()))?;
        gravar_indice(arq, &i)?;
    }
    Ok(v)
}

/// A primeira publicacao monta a pasta INTEIRA (versao 1 e indice) ao lado e a poe no lugar
/// com um `rename`: o gatilho que dispara no meio do `publicar` ve o fluxo sem pasta (o
/// publicado implicito) ou com a pasta completa, nunca uma pasta sem indice -- que
/// `ler_indice` recusa de proposito, porque pasta sem indice e o sinal de indice apagado.
fn criar_pasta_com_a_primeira(arq: &Path, corpo: &str, i: &Indice) -> Result<(), String> {
    let pasta = pasta_de(arq);
    let montagem = pasta.with_file_name(format!(
        "{}.novo",
        pasta.file_name().unwrap_or_default().to_string_lossy()
    ));
    // Sobra de uma queda anterior: so este processo, sob a trava, mexe aqui.
    let _ = std::fs::remove_dir_all(&montagem);
    let montar = || -> Result<(), String> {
        let v1 = montagem.join("v0001.json");
        gravar_atomico(&v1, corpo.as_bytes()).map_err(|e| format!("{}: {e}", v1.display()))?;
        let ind = montagem.join(ARQUIVO_INDICE);
        let texto = serde_json::to_string_pretty(i).map_err(|e| e.to_string())? + "\n";
        gravar_atomico(&ind, texto.as_bytes()).map_err(|e| format!("{}: {e}", ind.display()))?;
        std::fs::rename(&montagem, &pasta).map_err(|e| format!("{}: {e}", pasta.display()))?;
        let pai = pasta.parent().filter(|p| !p.as_os_str().is_empty());
        std::fs::File::open(pai.unwrap_or(Path::new(".")))
            .and_then(|d| d.sync_all())
            .map_err(|e| format!("{}: {e}", pasta.display()))
    };
    montar().inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&montagem);
    })
}

/// Copia a versao `n` para o rascunho. Recusa quando o rascunho tem edicao que nenhuma
/// versao guarda (`forcar` aceita perdê-la): voltar o rascunho e a unica operacao daqui que
/// destroi o que o operador digitou.
pub fn restaurar_rascunho(arq: &Path, n: u32, forcar: bool) -> Result<(), String> {
    let i = ler_indice(arq)?.ok_or("o fluxo nunca foi publicado: nao ha versao para restaurar")?;
    let alvo = i
        .versoes
        .get((n as usize).wrapping_sub(1))
        .ok_or_else(|| format!("a versao {n} nao existe (ha {})", i.versoes.len()))?;
    let f = ler_versao(arq, alvo)?;
    if !forcar && arq.exists() {
        let guardado = fluxos::ler_arquivo(arq)
            .map(|d| i.versoes.iter().any(|v| v.sha256 == fluxos::assinatura(&d)));
        // Rascunho que nao le (quebrado) tambem nao esta guardado em versao nenhuma.
        if guardado != Ok(true) {
            return Err(
                "o rascunho tem edicao que nenhuma versao guarda; publique antes ou use --forcar"
                    .into(),
            );
        }
    }
    // Os pins ja estao DENTRO da versao: o arquivo de pins ao lado sairia em duplicidade
    // (`ler_arquivo` recusa o mesmo passo pinado nos dois), entao ele vai embora junto.
    let corpo = canonico(&serde_json::to_value(&f).map_err(|e| e.to_string())?);
    gravar_atomico(arq, corpo.as_bytes()).map_err(|e| format!("{}: {e}", arq.display()))?;
    let pins = fluxos::arquivo_de_pins(arq);
    match std::fs::remove_file(&pins) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {e}", pins.display()))
        }
        _ => Ok(()),
    }
}

/// Onde o fluxo esta: o indice (se publicou) e se o rascunho andou desde a publicada.
#[derive(Debug, Clone)]
pub struct Situacao {
    pub indice: Option<Indice>,
    /// O hash do rascunho, ou o motivo de ele nao ler.
    pub rascunho: Result<String, String>,
}

impl Situacao {
    /// Rascunho que difere da publicada (ou que nao le). `false` sem publicada: o arquivo
    /// e o publicado implicito.
    pub fn rascunho_alterado(&self) -> bool {
        match (&self.indice, &self.rascunho) {
            (Some(i), Ok(sha)) => i.atual().is_none_or(|v| &v.sha256 != sha),
            (Some(_), Err(_)) => true,
            (None, _) => false,
        }
    }
}

pub fn situacao(arq: &Path) -> Result<Situacao, String> {
    Ok(Situacao {
        indice: ler_indice(arq)?,
        rascunho: fluxos::ler_arquivo(arq).map(|f| fluxos::assinatura(&f)),
    })
}

/// `api::criar_fluxo_com` sobre a versao PUBLICADA do fluxo: e por aqui que o gatilho de
/// webhook, o de arquivo e o formulario disparam. O caminho do rascunho nao chega ao
/// `criar_fluxo_com`: se a publicada nao le, a recusa e esta, nunca o rascunho no lugar.
pub fn criar_fluxo_publicado(
    s: &ApiState,
    caminho: &str,
    entrada: Vec<Value>,
    preparo: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<Criada, Recusa> {
    let alvo = caminho_publicado(Path::new(caminho)).map_err(|e| Recusa {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        erro: format!("fluxo: {e}"),
        retry_after: None,
    })?;
    crate::api::criar_fluxo_com(s, &alvo.to_string_lossy(), entrada, preparo)
}
