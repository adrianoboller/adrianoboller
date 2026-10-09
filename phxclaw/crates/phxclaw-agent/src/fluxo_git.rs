//! Git dos fluxos: `phxclaw fluxo exportar --ambiente dev|prod` grava cada fluxo como um
//! arquivo canonico numa pasta de repositorio, e `importar --ambiente` traz de volta. E o
//! «source control» do n8n (push/pull de fluxos entre instancias ligadas a ramos), no
//! tamanho que um arquivo de projeto permite.
//!
//! ```text
//! <repo>/dev/[pasta/]<fluxo>.json     o RASCUNHO de cada fluxo
//! <repo>/prod/[pasta/]<fluxo>.json    a versao PUBLICADA de cada fluxo (o que o gatilho roda)
//! ```
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Nao ha um segundo versionador.** O git e o `git.rs` desta casa: `registrar` chama o
//!   `GitTool` de escrita (mesmo sandbox, mesma varredura de segredos no `add` e no `commit`,
//!   mesmo autor), e a historia, o diff e o ramo sao os dele. Aqui mora so o que o git nao
//!   sabe: o que e um fluxo, e o que e o dev e o prod dele.
//! - **Arquivo canonico = diff estavel.** `fluxo_versoes::canonico`: chaves em ordem em
//!   qualquer profundidade, indentado, quebra final, sem data nem nome de maquina. Exportar
//!   duas vezes o mesmo fluxo gera os mesmos bytes (e nao reescreve o arquivo), entao o
//!   `git diff` mostra so o que o operador mudou. O arquivo e o pacote do `fluxos::exportar`
//!   (formato, sha256 de conferencia, fluxo) mais o `ambiente` e, no prod, a `versao`.
//! - **Ambiente e a versao do fluxo, nao uma pasta de configuracao.** `dev` e o rascunho e
//!   `prod` e a publicada: o prod so contem o que alguem PUBLICOU, e importar no prod
//!   publica (versao nova no historico), o que faz o gatilho passar a rodar o que veio do
//!   git. O `ambiente` mora tambem dentro do arquivo: mover um arquivo do dev para a pasta
//!   do prod e recusado na importacao, nao publicado.
//! - **Credencial nao viaja**: o fluxo nao tem lugar para ela (`fluxos::validar` recusa
//!   variavel e pin com forma de segredo), e o `registrar` ainda passa pela varredura do
//!   `git_write`.
//! - **A importacao confere TUDO antes de gravar qualquer coisa**: um arquivo ruim no meio
//!   nao deixa metade dos fluxos trocados. E nunca sobrescreve rascunho diferente sem o
//!   pedido explicito: trazer o git por cima de uma edicao nao guardada a perderia calado.

use crate::fluxo_versoes::{self, canonico};
use crate::fluxos::{self, Fluxo};
use crate::git::GitTool;
use phxclaw_agent_core::{Tool, ToolContext};
use phxclaw_types::arquivo::gravar_atomico;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A pasta do repositorio de fluxos, sob a pasta do agente, quando nao se diz outra.
pub const PASTA_PADRAO: &str = "fluxos-git";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ambiente {
    Dev,
    Prod,
}

impl Ambiente {
    pub fn ler(s: &str) -> Result<Self, String> {
        match s {
            "dev" => Ok(Self::Dev),
            "prod" => Ok(Self::Prod),
            outro => Err(format!("ambiente {outro:?} nao existe; use dev ou prod")),
        }
    }
    pub fn nome(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Prod => "prod",
        }
    }
}

/// Um fluxo exportado.
#[derive(Debug, Clone)]
pub struct Exportado {
    pub arquivo: PathBuf,
    pub nome: String,
    pub sha256: String,
    /// No prod, o numero da versao publicada (`None`: nunca publicado, o arquivo e o publicado).
    pub versao: Option<u32>,
    /// Falso quando o arquivo ja tinha exatamente estes bytes.
    pub mudou: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Exportacao {
    pub escritos: Vec<Exportado>,
    /// Arquivos de fluxo que o repositorio ainda tinha e o projeto nao tem mais.
    pub removidos: Vec<PathBuf>,
}

/// O arquivo canonico de um fluxo no ambiente. Pure: o mesmo fluxo da os mesmos bytes.
pub fn arquivo_canonico(f: &Fluxo, amb: Ambiente, versao: Option<u32>) -> String {
    let mut pacote = fluxos::exportar(f);
    pacote["ambiente"] = json!(amb.nome());
    if amb == Ambiente::Prod {
        pacote["versao"] = json!(versao);
    }
    canonico(&pacote)
}

fn lido(arq: &Path) -> Option<String> {
    std::fs::read_to_string(arq).ok()
}

/// Grava os fluxos de `fluxos_dir` em `repo/<ambiente>/`. Qualquer fluxo que nao le aborta a
/// exportacao ANTES de gravar: repositorio com metade dos fluxos e pior que sem nenhum.
pub fn exportar(fluxos_dir: &Path, repo: &Path, amb: Ambiente) -> Result<Exportacao, String> {
    let (achados, erros) = fluxos::listar(fluxos_dir, None, None);
    if !erros.is_empty() {
        return Err(format!(
            "{} fluxo(s) nao leem, e nada foi exportado:\n  {}",
            erros.len(),
            erros.join("\n  ")
        ));
    }
    let base = repo.join(amb.nome());
    // Primeiro tudo em memoria: o que falhar em ler a publicada para antes de tocar o disco.
    let mut plano: Vec<(PathBuf, String, Exportado)> = Vec::new();
    for a in &achados {
        let (f, versao) = match amb {
            Ambiente::Dev => (fluxos::ler_arquivo(&a.arquivo)?, None),
            Ambiente::Prod => (
                fluxo_versoes::ler_publicado(&a.arquivo)?,
                fluxo_versoes::ler_indice(&a.arquivo)?.map(|i| i.publicada),
            ),
        };
        let stem = a
            .arquivo
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let destino = if a.pasta.is_empty() {
            base.join(format!("{stem}.json"))
        } else {
            base.join(&a.pasta).join(format!("{stem}.json"))
        };
        let texto = arquivo_canonico(&f, amb, versao);
        let info = Exportado {
            arquivo: destino.clone(),
            nome: f.nome.clone(),
            sha256: fluxos::assinatura(&f),
            versao,
            mudou: lido(&destino).as_deref() != Some(texto.as_str()),
        };
        plano.push((destino, texto, info));
    }
    // O que o repositorio ja tinha, listado ANTES da primeira gravacao: um link no meio dele
    // e recusa, e recusa depois de gravar seria o repositorio pela metade.
    let existentes = arquivos_de_pacote(&base)?;
    let mut feito = Exportacao::default();
    for (destino, texto, info) in plano {
        if info.mudou {
            gravar_atomico(&destino, texto.as_bytes())
                .map_err(|e| format!("{}: {e}", destino.display()))?;
        }
        feito.escritos.push(info);
    }
    // O fluxo que saiu do projeto sai do repositorio, para o `git diff` mostrar a remocao.
    // So apaga arquivo que e pacote de fluxo (tem `phxclaw_fluxo`): README e o que o
    // operador pos ali ficam.
    let vivos: BTreeSet<PathBuf> = feito.escritos.iter().map(|e| e.arquivo.clone()).collect();
    for arq in existentes {
        let e_pacote = lido(&arq)
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| v.get("phxclaw_fluxo").is_some());
        if e_pacote && !vivos.contains(&arq) {
            std::fs::remove_file(&arq).map_err(|e| format!("{}: {e}", arq.display()))?;
            feito.removidos.push(arq);
        }
    }
    Ok(feito)
}

/// Os `.json` de `base` e das subpastas diretas (a «pasta» do fluxo), em ordem.
///
/// Pelo `symlink_metadata` (o tipo da ENTRADA, nao do alvo) e com link RECUSADO: o
/// repositorio vem de fora (`git pull`), e um `x.json -> /caminho/qualquer.json` ou uma
/// pasta `sub -> /outro/lugar` faria o `importar` trazer para o projeto -- e, no prod,
/// publicar -- um arquivo que nao esta no repositorio. `base` ausente e lista vazia.
fn arquivos_de_pacote(base: &Path) -> Result<Vec<PathBuf>, String> {
    let link = |p: &Path| {
        format!(
            "{}: link simbolico no repositorio de fluxos (so arquivo e pasta de verdade)",
            p.display()
        )
    };
    let tipo = |p: &Path| std::fs::symlink_metadata(p).map(|m| m.file_type()).ok();
    let mut saida = Vec::new();
    let mut pastas = vec![base.to_path_buf()];
    if let Ok(rd) = std::fs::read_dir(base) {
        let mut sub = Vec::new();
        for p in rd.flatten().map(|e| e.path()) {
            let oculta = p
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'));
            match tipo(&p) {
                Some(t) if t.is_symlink() && !oculta => return Err(link(&p)),
                Some(t) if t.is_dir() && !oculta => sub.push(p),
                _ => {}
            }
        }
        sub.sort();
        pastas.extend(sub);
    }
    for d in pastas {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut arqs = Vec::new();
        for p in rd.flatten().map(|e| e.path()) {
            if !p.extension().is_some_and(|x| x == "json") {
                continue;
            }
            match tipo(&p) {
                Some(t) if t.is_symlink() => return Err(link(&p)),
                Some(t) if t.is_file() => arqs.push(p),
                _ => {}
            }
        }
        arqs.sort();
        saida.extend(arqs);
    }
    Ok(saida)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Efeito {
    /// O fluxo nao existia no projeto.
    Novo,
    /// Existia diferente; o rascunho foi trocado (e, no prod, publicado).
    Atualizado,
    /// Ja estava igual: nada foi gravado.
    Igual,
}

#[derive(Debug, Clone)]
pub struct Importado {
    pub destino: PathBuf,
    pub nome: String,
    pub efeito: Efeito,
    /// No prod, a versao publicada depois da importacao.
    pub versao: Option<u32>,
}

struct Plano {
    destino: PathBuf,
    fluxo: Fluxo,
    efeito: Efeito,
    publicar: bool,
}

/// Traz `repo/<ambiente>/` para `fluxos_dir`. Tudo e lido e conferido (formato, ambiente do
/// arquivo, validacao, sha256) antes de a primeira gravacao. No `prod` cada fluxo trocado e
/// PUBLICADO; no `dev` so o rascunho muda. `sobrescrever` e o pedido para trocar um rascunho
/// que difere do que veio.
pub fn importar(
    repo: &Path,
    amb: Ambiente,
    fluxos_dir: &Path,
    sobrescrever: bool,
) -> Result<Vec<Importado>, String> {
    let base = repo.join(amb.nome());
    // O tipo da entrada, nao do alvo: `prod -> /outro/lugar` e recusa, como o link dentro.
    if std::fs::symlink_metadata(&base).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!(
            "{}: link simbolico no repositorio de fluxos (so arquivo e pasta de verdade)",
            base.display()
        ));
    }
    if !base.is_dir() {
        return Err(format!(
            "{} nao existe: exporte o ambiente {} antes (ou aponte --repo)",
            base.display(),
            amb.nome()
        ));
    }
    let arquivos = arquivos_de_pacote(&base)?;
    if arquivos.is_empty() {
        return Err(format!("{}: nenhum fluxo", base.display()));
    }
    let mut planos: Vec<Plano> = Vec::new();
    let mut conflitos: Vec<String> = Vec::new();
    for arq in arquivos {
        let texto = std::fs::read_to_string(&arq).map_err(|e| format!("{}: {e}", arq.display()))?;
        let v: Value = serde_json::from_str(&texto)
            .map_err(|e| format!("{}: pacote invalido: {e}", arq.display()))?;
        match v.get("ambiente").and_then(Value::as_str) {
            Some(a) if a == amb.nome() => {}
            Some(a) => {
                return Err(format!(
                    "{}: o arquivo e do ambiente {a:?} e esta na pasta {} (movido a mao?)",
                    arq.display(),
                    amb.nome()
                ));
            }
            None => {
                return Err(format!(
                    "{}: falta o `ambiente` (nao e um arquivo de fluxos-git)",
                    arq.display()
                ));
            }
        }
        let f = fluxos::importar(&texto).map_err(|e| format!("{}: {e}", arq.display()))?;
        let rel = arq.strip_prefix(&base).map_err(|e| e.to_string())?;
        let destino = fluxos_dir.join(rel);
        let sha = fluxos::assinatura(&f);
        let publicada = fluxo_versoes::ler_indice(&destino)
            .ok()
            .flatten()
            .and_then(|i| i.atual().map(|v| v.sha256.clone()));
        let rascunho = destino
            .exists()
            .then(|| fluxos::ler_arquivo(&destino).map(|d| fluxos::assinatura(&d)));
        let existe = rascunho.is_some();
        let rascunho_igual = matches!(&rascunho, Some(Ok(r)) if *r == sha);
        let publicada_igual = publicada.as_deref() == Some(sha.as_str());
        // Rascunho que existe e difere (ou nao le) so e trocado a pedido.
        let mut exigir_pedido = || {
            if !sobrescrever {
                conflitos.push(format!(
                    "{}: o rascunho difere do que veio do git",
                    destino.display()
                ));
            }
        };
        let (efeito, publicar) = match amb {
            Ambiente::Dev if !existe => (Efeito::Novo, false),
            Ambiente::Dev if rascunho_igual => (Efeito::Igual, false),
            Ambiente::Dev => {
                exigir_pedido();
                (Efeito::Atualizado, false)
            }
            Ambiente::Prod if !existe => (Efeito::Novo, true),
            // O prod ja roda isto; o rascunho com edicao nao guardada nao e do prod.
            Ambiente::Prod if publicada_igual => (Efeito::Igual, false),
            // O arquivo ja e este: so falta publicar.
            Ambiente::Prod if rascunho_igual => (Efeito::Atualizado, true),
            Ambiente::Prod => {
                exigir_pedido();
                (Efeito::Atualizado, true)
            }
        };
        planos.push(Plano {
            destino,
            fluxo: f,
            efeito,
            publicar,
        });
    }
    if !conflitos.is_empty() {
        return Err(format!(
            "{} rascunho(s) seriam perdidos, e nada foi importado (--sobrescrever aceita a troca):\n  {}",
            conflitos.len(),
            conflitos.join("\n  ")
        ));
    }
    let mut feito = Vec::new();
    for p in planos {
        if p.efeito != Efeito::Igual {
            gravar_rascunho(&p)?;
        }
        let mut versao = fluxo_versoes::ler_indice(&p.destino)?.map(|i| i.publicada);
        if p.publicar {
            match fluxo_versoes::publicar(&p.destino, "importado do ambiente prod") {
                Ok(v) => versao = Some(v.numero),
                // Rascunho igual a publicada: o prod ja tem esta versao.
                Err(e) if e.contains("nada a publicar") => {}
                Err(e) => return Err(format!("{}: {e}", p.destino.display())),
            }
        }
        feito.push(Importado {
            destino: p.destino,
            nome: p.fluxo.nome,
            efeito: p.efeito,
            versao,
        });
    }
    Ok(feito)
}

fn gravar_rascunho(p: &Plano) -> Result<(), String> {
    if !p.destino.exists() {
        // O fluxo novo ganha o formato de sempre (pins no arquivo ao lado).
        return fluxos::gravar_importado(&p.fluxo, &p.destino);
    }
    // Por cima de um rascunho: UM arquivo gravado de uma vez, com os pins dentro; o arquivo
    // de pins do rascunho velho sairia em duplicidade (`ler_arquivo` recusa) e vai embora.
    let corpo = canonico(&serde_json::to_value(&p.fluxo).map_err(|e| e.to_string())?);
    gravar_atomico(&p.destino, corpo.as_bytes())
        .map_err(|e| format!("{}: {e}", p.destino.display()))?;
    let pins = fluxos::arquivo_de_pins(&p.destino);
    match std::fs::remove_file(&pins) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {e}", pins.display()))
        }
        _ => Ok(()),
    }
}

/// Registra o repositorio de fluxos no git (`init` se faltar, `add`, `commit`), pelo
/// `GitTool` de escrita: o mesmo sandbox e a mesma varredura de segredos de qualquer `git_write`.
/// Devolve `{"commit": ..., "varredura_de_segredos": ...}`, ou `{"commit": null,
/// "nada_a_registrar": true}` quando nada mudou.
pub async fn registrar(g: &GitTool, repo: &Path, mensagem: &str) -> Result<Value, String> {
    let ctx = ToolContext {
        task_id: "fluxo-git".into(),
        workdir: repo.to_path_buf(),
        timeout: Duration::from_secs(60),
    };
    let rodar = |args: Value| {
        let ctx = &ctx;
        async move {
            let r = g.run(args, ctx).await.map_err(|e| e.to_string())?;
            serde_json::from_str::<Value>(&r.content).map_err(|e| e.to_string())
        }
    };
    if !repo.join(".git").exists() {
        rodar(json!({"action": "init", "branch": "main"})).await?;
    }
    let st = rodar(json!({"action": "add", "paths": ["."]})).await?;
    let preparado = st["arquivos"]
        .as_array()
        .is_some_and(|l| l.iter().any(|a| a["preparado"] == true));
    if !preparado {
        return Ok(json!({"commit": null, "nada_a_registrar": true}));
    }
    rodar(json!({"action": "commit", "message": mensagem})).await
}
