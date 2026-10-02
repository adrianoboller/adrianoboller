//! Arquivos e formatos: zip, JSON/XML e PDF como ferramentas do agente.
//!
//! Tudo aqui le e grava so dentro de `ctx.workdir`, pelo mesmo `confine` das outras
//! ferramentas de arquivo. Os programas externos (`pdftotext`, `pdfinfo`, `soffice`)
//! rodam no MESMO sandbox do `shell` (bwrap, sem rede, `/tmp` em tmpfs), montado pela
//! crate do sandbox: um HTML convertido para PDF pode pedir imagem remota, e um PDF hostil
//! e entrada para um parser em C. O prazo e o da chamada (`ctx.timeout`), e o sandbox mata
//! o processo -- e o espaco de PIDs inteiro -- no estouro.

use crate::motor::artifact_for;
use crate::tarefa::confine;
use phxclaw_agent_core::{
    Artifact, BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec, truncate_for_model,
};
use phxclaw_office::zip;
use phxclaw_sandbox::{SandboxError, SandboxExtras, WorkdirCommand, run_in_workdir_com};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Texto devolvido ao modelo: o bastante para um documento de varias paginas sem
/// tomar o contexto inteiro.
pub const MAX_CHARS: usize = 20_000;
/// Teto da SOMA descomprimida ao extrair. O do `phxclaw-office` e por entrada; sozinho
/// ele deixaria passar mil entradas de 200 MiB.
pub const MAX_EXTRAIR_BYTES: usize = 256 * 1024 * 1024;
/// Teto da soma dos arquivos ao criar um zip: o pacote e montado em memoria.
pub const MAX_CRIAR_BYTES: usize = 256 * 1024 * 1024;
/// Tamanho do .zip lido em memoria para listar ou extrair.
pub const MAX_ZIP_BYTES: u64 = 512 * 1024 * 1024;
/// Mais entradas que isso num pacote que o agente recebeu e pacote feito para cansar.
pub const MAX_ENTRADAS: usize = 10_000;
/// Arquivo de dados lido inteiro para validar e formatar.
pub const MAX_DADOS_BYTES: u64 = 16 * 1024 * 1024;
/// Artefatos devolvidos por uma extracao: cada um custa um hash, e mil artefatos so
/// enchem a evidencia sem dizer mais do que a contagem.
const MAX_ARTEFATOS: usize = 100;

fn arg<'a>(a: &'a Value, n: &str) -> Result<&'a str, ToolError> {
    a.get(n)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments(format!("falta o campo texto '{n}'")))
}

fn falha(e: impl std::fmt::Display) -> ToolError {
    ToolError::Failed(e.to_string())
}

/// Caminho confinado e o mesmo caminho relativo a pasta da tarefa, ja sem o `/work/` ou
/// `./` que o `confine` aceita: o artefato e a linha de comando precisam do relativo
/// limpo, e recalcula-lo a partir do absoluto e o que garante que os dois concordam.
fn confinar(ctx: &ToolContext, rel: &str) -> Result<(PathBuf, String), ToolError> {
    let alvo = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
    let limpo = alvo
        .strip_prefix(&ctx.workdir)
        .map_err(|_| ToolError::Denied(format!("caminho fora da pasta da tarefa: {rel}")))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok((alvo, limpo))
}

fn exige_extensao(rel: &str, exts: &[&str]) -> Result<(), ToolError> {
    let baixo = rel.to_ascii_lowercase();
    if exts.iter().any(|e| baixo.ends_with(e)) {
        Ok(())
    } else {
        Err(ToolError::InvalidArguments(format!(
            "{rel}: extensao aceita: {}",
            exts.join(", ")
        )))
    }
}

fn extensao(rel: &str) -> String {
    Path::new(rel)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn artefato(ctx: &ToolContext, limpo: &str) -> Result<Artifact, ToolError> {
    artifact_for(&ctx.workdir, limpo).map_err(falha)
}

fn ler_limitado(alvo: &Path, teto: u64) -> Result<Vec<u8>, ToolError> {
    let m = std::fs::metadata(alvo).map_err(falha)?;
    if m.len() > teto {
        return Err(ToolError::Failed(format!(
            "arquivo com {} bytes, acima do teto de {teto}",
            m.len()
        )));
    }
    std::fs::read(alvo).map_err(falha)
}

// ---------------------------------------------------------------- processos externos

/// O bwrap que o `shell` usa: o mesmo sandbox, achado num lugar so.
pub fn achar_bwrap() -> Option<PathBuf> {
    ["/usr/bin/bwrap", "/bin/bwrap", "/usr/local/bin/bwrap"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

/// O sandbox ve o `PATH` fixo dele (`/usr/local/bin:/usr/bin:/bin`); procurar no `PATH`
/// do processo pai acharia programa que la dentro nao existe.
fn exige_programa(nome: &str, pacote: &str) -> Result<(), ToolError> {
    exige_programa_em(nome, pacote, &["/usr/local/bin", "/usr/bin", "/bin"])
}

fn exige_programa_em(nome: &str, pacote: &str, dirs: &[&str]) -> Result<(), ToolError> {
    if dirs.iter().any(|d| Path::new(d).join(nome).is_file()) {
        return Ok(());
    }
    Err(ToolError::Failed(format!(
        "{nome} nao encontrado em {}: instale o pacote {pacote} (ex.: apt install {pacote})",
        dirs.join(":")
    )))
}

/// Aspas simples de shell: o caminho vem do modelo e nao pode virar comando.
fn aspas(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[derive(Debug)]
struct Saida {
    stdout: String,
    stderr: String,
}

/// Roda uma linha de shell no sandbox da tarefa com o prazo da chamada. Codigo de saida
/// diferente de zero vira falha com o stderr, que e onde o poppler e o soffice explicam.
async fn no_sandbox(
    ctx: &ToolContext,
    script: String,
    extras: SandboxExtras,
) -> Result<Saida, ToolError> {
    let bwrap = achar_bwrap().ok_or_else(|| {
        ToolError::Failed(
            "bwrap nao encontrado: instale o pacote bubblewrap (ex.: apt install bubblewrap); \
os conversores rodam no mesmo sandbox do shell"
                .into(),
        )
    })?;
    let cmd = WorkdirCommand {
        workdir: ctx.workdir.clone(),
        script,
        timeout: ctx.timeout,
        network: false,
        max_output_bytes: 8 * 1024 * 1024,
    };
    let prazo = ctx.timeout;
    let r = tokio::task::spawn_blocking(move || run_in_workdir_com(&bwrap, &cmd, &extras))
        .await
        .map_err(falha)?
        .map_err(|e| match e {
            SandboxError::Timeout(_) => ToolError::Timeout(prazo.as_millis() as u64),
            outro => falha(outro),
        })?;
    if r.exit_code != Some(0) {
        return Err(ToolError::Failed(format!(
            "saida {}: {}",
            r.exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "por sinal".into()),
            truncate_for_model(r.stderr.trim(), 2_000)
        )));
    }
    Ok(Saida {
        stdout: r.stdout,
        stderr: r.stderr,
    })
}

/// Texto de um PDF da pasta (`limpo` relativo a ela), com `-layout` para tabela e coluna
/// continuarem legiveis. Um so caminho para a ferramenta `pdf` e para o `read_document`.
pub async fn pdf_texto(
    ctx: &ToolContext,
    limpo: &str,
    primeira: Option<u64>,
    ultima: Option<u64>,
) -> Result<String, ToolError> {
    exige_programa("pdftotext", "poppler-utils")?;
    let mut s = String::from("pdftotext -layout -enc UTF-8");
    if let Some(p) = primeira {
        s.push_str(&format!(" -f {p}"));
    }
    if let Some(u) = ultima {
        s.push_str(&format!(" -l {u}"));
    }
    s.push_str(&format!(" {} -", aspas(&format!("/work/{limpo}"))));
    let o = no_sandbox(ctx, s, SandboxExtras::default()).await?;
    Ok(o.stdout)
}

// ---------------------------------------------------------------- zip

/// Nome de entrada que pode virar caminho dentro da pasta de destino. A conferencia e
/// feita no NOME antes do `confine`, porque `a/../../x` normalizado por alguem no meio do
/// caminho deixaria de parecer o que e.
fn nome_seguro(n: &str) -> Result<(), String> {
    let b = n.as_bytes();
    if n.is_empty()
        || n.contains('\0')
        || n.contains('\\')
        || n.starts_with('/')
        || (b.len() >= 2 && b[1] == b':')
    {
        return Err(format!("entrada com caminho absoluto ou invalido: {n:?}"));
    }
    if n.trim_end_matches('/')
        .split('/')
        .any(|s| s == ".." || s == ".")
    {
        return Err(format!("entrada que sai da pasta de destino: {n:?}"));
    }
    Ok(())
}

pub struct ZipListTool;

impl Tool for ZipListTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "zip_list".into(),
            description: "List the entries of a .zip in the task directory (name, size, compressed size) without extracting.".into(),
            parameters: json!({"type":"object","properties":{"path":{"type":"string","description":"relative path of the .zip"}},"required":["path"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let (alvo, limpo) = confinar(ctx, arg(&args, "path")?)?;
            let bytes = ler_limitado(&alvo, MAX_ZIP_BYTES)?;
            let l = zip::list(&bytes).map_err(falha)?;
            let total: u64 = l.iter().map(|e| e.size).sum();
            let mut t = format!(
                "{limpo}: {} entradas, {total} bytes descomprimidos\n",
                l.len()
            );
            for e in &l {
                let marca = match (e.is_dir, e.is_symlink, e.encrypted) {
                    (true, _, _) => " [pasta]",
                    (_, true, _) => " [link]",
                    (_, _, true) => " [cifrada]",
                    _ => "",
                };
                t.push_str(&format!(
                    "{}\t{}\t{}{marca}\n",
                    e.name, e.size, e.compressed
                ));
            }
            Ok(ToolOutput::text(truncate_for_model(
                t.trim_end(),
                MAX_CHARS,
            )))
        })
    }
}

pub struct ZipTool;

impl Tool for ZipTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "zip".into(),
            description: "Extract a .zip into a folder of the task directory (action=extract, path, dest), or create a .zip from files/folders of the task directory (action=create, path, files). Entries with '..', absolute paths or links are refused.".into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["extract","create"]},
                "path":{"type":"string","description":"relative path of the .zip"},
                "dest":{"type":"string","description":"extract: relative folder (default: zip name without extension)"},
                "files":{"type":"array","items":{"type":"string"},"description":"create: relative files or folders"}
            },"required":["action","path"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            match arg(&args, "action")? {
                "extract" => extrair(ctx, &args),
                "create" => criar_zip(ctx, &args),
                outra => Err(ToolError::InvalidArguments(format!(
                    "action desconhecida: {outra} (extract ou create)"
                ))),
            }
        })
    }
}

/// Confere o pacote INTEIRO antes de gravar o primeiro byte: recusar na decima entrada
/// depois de gravar nove deixaria meia extracao que ninguem pediu.
fn extrair(ctx: &ToolContext, args: &Value) -> Result<ToolOutput, ToolError> {
    let rel = arg(args, "path")?;
    exige_extensao(rel, &[".zip"])?;
    let (alvo, limpo) = confinar(ctx, rel)?;
    let bytes = ler_limitado(&alvo, MAX_ZIP_BYTES)?;
    let infos = zip::list(&bytes).map_err(falha)?;
    if infos.len() > MAX_ENTRADAS {
        return Err(ToolError::Denied(format!(
            "{} entradas, acima do teto de {MAX_ENTRADAS}",
            infos.len()
        )));
    }
    let padrao = Path::new(&limpo)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "extraido".into());
    let dest_rel = args
        .get("dest")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            Path::new(&limpo)
                .parent()
                .map(|p| p.join(&padrao).to_string_lossy().into_owned())
                .unwrap_or(padrao)
        });
    let (_, dest) = confinar(ctx, &dest_rel)?;
    let mut total = 0usize;
    let mut planos: Vec<(String, PathBuf, String, bool)> = Vec::with_capacity(infos.len());
    for e in &infos {
        if e.is_symlink {
            return Err(ToolError::Denied(format!(
                "entrada {:?} e link simbolico; extrair link e escrever onde ele aponta",
                e.name
            )));
        }
        if e.encrypted {
            return Err(ToolError::Failed(format!(
                "entrada {:?} cifrada nao suportada",
                e.name
            )));
        }
        nome_seguro(&e.name).map_err(ToolError::Denied)?;
        total = total.saturating_add(e.size as usize);
        if total > MAX_EXTRAIR_BYTES {
            return Err(ToolError::Denied(format!(
                "pacote descomprime para mais de {MAX_EXTRAIR_BYTES} bytes"
            )));
        }
        let nome = e.name.trim_end_matches('/');
        let rel_dentro = if dest.is_empty() {
            nome.to_string()
        } else {
            format!("{dest}/{nome}")
        };
        let (abs, limpo_e) = confinar(ctx, &rel_dentro)?;
        planos.push((e.name.clone(), abs, limpo_e, e.is_dir));
    }
    let ar = zip::read_limited(&bytes, MAX_EXTRAIR_BYTES).map_err(falha)?;
    let conteudo: std::collections::HashMap<&str, &[u8]> = ar.entries().collect();
    let mut artifacts = Vec::new();
    let mut arquivos = 0usize;
    for (nome, abs, limpo_e, is_dir) in &planos {
        if *is_dir {
            std::fs::create_dir_all(abs).map_err(falha)?;
            continue;
        }
        let dados = conteudo
            .get(nome.as_str())
            .ok_or_else(|| ToolError::Failed(format!("entrada {nome:?} sumiu na leitura")))?;
        if let Some(d) = abs.parent() {
            std::fs::create_dir_all(d).map_err(falha)?;
        }
        std::fs::write(abs, dados).map_err(falha)?;
        arquivos += 1;
        if artifacts.len() < MAX_ARTEFATOS {
            artifacts.push(artefato(ctx, limpo_e)?);
        }
    }
    let mostrar = if dest.is_empty() { "." } else { &dest };
    Ok(ToolOutput {
        content: format!("extraidos {arquivos} arquivos ({total} bytes) de {limpo} em {mostrar}"),
        artifacts,
    })
}

/// Arquivos de uma pasta, em ordem de nome (o pacote sai igual a cada geracao). Link e
/// pulado, nao seguido: seguir um link da pasta poria no pacote um arquivo de fora dela.
fn coletar(
    dir: &Path,
    base: &Path,
    out: &mut Vec<(String, PathBuf)>,
    pulados: &mut Vec<String>,
) -> Result<(), ToolError> {
    let mut filhos: Vec<_> = std::fs::read_dir(dir)
        .map_err(falha)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    filhos.sort();
    for p in filhos {
        let m = std::fs::symlink_metadata(&p).map_err(falha)?;
        let rel = p
            .strip_prefix(base)
            .map_err(falha)?
            .to_string_lossy()
            .replace('\\', "/");
        if m.file_type().is_symlink() {
            pulados.push(rel);
        } else if m.is_dir() {
            coletar(&p, base, out, pulados)?;
        } else if m.is_file() {
            out.push((rel, p));
        }
    }
    Ok(())
}

fn criar_zip(ctx: &ToolContext, args: &Value) -> Result<ToolOutput, ToolError> {
    let rel = arg(args, "path")?;
    exige_extensao(rel, &[".zip"])?;
    let (saida, limpo) = confinar(ctx, rel)?;
    let pedidos: Vec<&str> = args
        .get("files")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if pedidos.is_empty() {
        return Err(ToolError::InvalidArguments(
            "files vazio: diga quais arquivos ou pastas entram".into(),
        ));
    }
    let base = &ctx.workdir;
    let mut achados = Vec::new();
    let mut pulados = Vec::new();
    for p in pedidos {
        let (abs, limpo_p) = confinar(ctx, p)?;
        let m = std::fs::symlink_metadata(&abs)
            .map_err(|e| ToolError::InvalidArguments(format!("{p}: {e}")))?;
        if m.file_type().is_symlink() {
            pulados.push(limpo_p);
        } else if m.is_dir() {
            coletar(&abs, base, &mut achados, &mut pulados)?;
        } else {
            achados.push((limpo_p, abs));
        }
    }
    let mut vistos = HashSet::new();
    let mut entradas = Vec::new();
    let mut total = 0usize;
    for (nome, abs) in achados {
        // O proprio pacote nao entra nele: zipar a pasta onde ele sera gravado o
        // incluiria na geracao seguinte.
        if nome == limpo || !vistos.insert(nome.clone()) {
            continue;
        }
        // O tamanho vem do metadado ANTES de ler: ler primeiro e conferir depois poria um
        // arquivo de 10 GB inteiro na memoria so para recusa-lo.
        total += std::fs::metadata(&abs).map_err(falha)?.len() as usize;
        if total > MAX_CRIAR_BYTES {
            return Err(ToolError::Denied(format!(
                "arquivos somam mais de {MAX_CRIAR_BYTES} bytes"
            )));
        }
        entradas.push((nome, std::fs::read(&abs).map_err(falha)?));
    }
    if entradas.is_empty() {
        return Err(ToolError::InvalidArguments(
            "nenhum arquivo regular para empacotar".into(),
        ));
    }
    let z = zip::write_store(&entradas).map_err(falha)?;
    if let Some(d) = saida.parent() {
        std::fs::create_dir_all(d).map_err(falha)?;
    }
    std::fs::write(&saida, &z).map_err(falha)?;
    let a = artefato(ctx, &limpo)?;
    let mut t = format!(
        "criado {limpo} com {} arquivos ({} bytes)",
        entradas.len(),
        a.bytes
    );
    if !pulados.is_empty() {
        t.push_str(&format!(
            "; links pulados (nao se segue link): {}",
            pulados.join(", ")
        ));
    }
    Ok(ToolOutput {
        content: t,
        artifacts: vec![a],
    })
}

// ---------------------------------------------------------------- json e xml

fn linha_coluna(src: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(src.len());
    let mut corte = pos;
    while !src.is_char_boundary(corte) {
        corte -= 1;
    }
    let antes = &src[..corte];
    let linha = antes.matches('\n').count() + 1;
    let coluna = antes.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (linha, coluna)
}

/// Reindenta um JSON ja validado sem passar pelo `Value`: o `Value` perde a ordem das
/// chaves quando o `serde_json` nao tem `preserve_order` e arredonda numero que nao cabe
/// em `f64`. Reformatar texto mantem as duas coisas como o autor escreveu.
pub fn json_bonito(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len() * 2);
    let mut nivel = 0usize;
    let (mut em_texto, mut escape) = (false, false);
    let quebra = |out: &mut String, nivel: usize| {
        out.push('\n');
        out.push_str(&"  ".repeat(nivel));
    };
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        i += 1;
        if em_texto {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                em_texto = false;
            }
            continue;
        }
        match ch {
            '"' => {
                em_texto = true;
                out.push(ch);
            }
            '{' | '[' => {
                let fecha = if ch == '{' { '}' } else { ']' };
                let mut j = i;
                while j < c.len() && c[j].is_whitespace() {
                    j += 1;
                }
                if j < c.len() && c[j] == fecha {
                    out.push(ch);
                    out.push(fecha);
                    i = j + 1;
                } else {
                    out.push(ch);
                    nivel += 1;
                    quebra(&mut out, nivel);
                }
            }
            '}' | ']' => {
                nivel = nivel.saturating_sub(1);
                quebra(&mut out, nivel);
                out.push(ch);
            }
            ',' => {
                out.push(',');
                quebra(&mut out, nivel);
            }
            ':' => out.push_str(": "),
            w if w.is_whitespace() => {}
            outro => out.push(outro),
        }
    }
    out
}

fn json_validar(src: &str) -> Result<Value, ToolError> {
    serde_json::from_str::<Value>(src).map_err(|e| {
        ToolError::Failed(format!(
            "JSON invalido na linha {}, coluna {}: {e}",
            e.line(),
            e.column()
        ))
    })
}

fn resumo_json(v: &Value) -> String {
    match v {
        Value::Object(m) => format!("objeto com {} chaves", m.len()),
        Value::Array(a) => format!("lista com {} itens", a.len()),
        Value::String(_) => "texto".into(),
        Value::Number(_) => "numero".into(),
        Value::Bool(_) => "booleano".into(),
        Value::Null => "null".into(),
    }
}

/// Ponteiro RFC 6901. Quando nao acha, diz ate onde achou e o que havia ali: o modelo
/// acerta o caminho na tentativa seguinte em vez de chutar de novo.
fn json_consultar(v: &Value, ponteiro: &str) -> Result<String, ToolError> {
    if !ponteiro.is_empty() && !ponteiro.starts_with('/') {
        return Err(ToolError::InvalidArguments(format!(
            "ponteiro JSON comeca com '/' (RFC 6901): {ponteiro:?}"
        )));
    }
    if let Some(achado) = v.pointer(ponteiro) {
        return Ok(serde_json::to_string_pretty(achado).unwrap_or_default());
    }
    let partes: Vec<&str> = ponteiro.split('/').skip(1).collect();
    let mut prefixo = String::new();
    let mut atual = v;
    for p in &partes {
        let prox = format!("{prefixo}/{p}");
        match v.pointer(&prox) {
            Some(x) => {
                prefixo = prox;
                atual = x;
            }
            None => break,
        }
    }
    let ha = match atual {
        Value::Object(m) => format!(
            "chaves: {}",
            m.keys().take(30).cloned().collect::<Vec<_>>().join(", ")
        ),
        Value::Array(a) => format!("lista de {} itens (indices 0..{})", a.len(), a.len()),
        outro => format!("valor {}", resumo_json(outro)),
    };
    Err(ToolError::Failed(format!(
        "nada em {ponteiro:?}; o caminho existe ate {:?}, onde ha {ha}",
        if prefixo.is_empty() { "" } else { &prefixo }
    )))
}

fn xml_erro(src: &str, pos: usize, msg: impl std::fmt::Display) -> ToolError {
    let (l, c) = linha_coluna(src, pos);
    ToolError::Failed(format!("XML invalido na linha {l}, coluna {c}: {msg}"))
}

const ENTIDADES: [(&str, char); 5] = [
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
];

/// Uma passada de leitura que serve as tres acoes: confere a boa formacao (uma raiz,
/// fechamentos casados, nada fora da raiz, entidade declarada) e entrega cada evento a
/// quem quiser. Entidade externa nunca e resolvida: o leitor nao busca nada.
fn xml_percorrer(
    src: &str,
    mut cada: impl FnMut(&quick_xml::events::Event) -> Result<(), ToolError>,
) -> Result<(), ToolError> {
    use quick_xml::events::Event;
    let mut r = quick_xml::Reader::from_str(src);
    let mut abertos: Vec<String> = Vec::new();
    let (mut raiz_vista, mut tem_dtd) = (false, false);
    loop {
        let ev = r
            .read_event()
            .map_err(|e| xml_erro(src, r.error_position() as usize, e))?;
        let pos = r.buffer_position() as usize;
        let fora = abertos.is_empty();
        match &ev {
            Event::Eof => {
                if let Some(n) = abertos.last() {
                    return Err(xml_erro(
                        src,
                        src.len(),
                        format!("elemento <{n}> sem fechamento"),
                    ));
                }
                if !raiz_vista {
                    return Err(xml_erro(src, src.len(), "documento sem elemento raiz"));
                }
                return Ok(());
            }
            Event::Start(_) | Event::Empty(_) if fora && raiz_vista => {
                return Err(xml_erro(src, pos, "mais de um elemento raiz"));
            }
            Event::Start(e) => {
                raiz_vista = true;
                abertos.push(e.name().as_ref().to_string());
            }
            Event::Empty(_) => raiz_vista = true,
            Event::End(_) => {
                abertos.pop();
            }
            Event::Text(t) if fora && !t.trim().is_empty() => {
                return Err(xml_erro(src, pos, "texto fora do elemento raiz"));
            }
            Event::GeneralRef(g) => {
                if fora {
                    return Err(xml_erro(src, pos, "referencia fora do elemento raiz"));
                }
                let conhecida = g.is_char_ref() || ENTIDADES.iter().any(|(n, _)| **g == **n);
                if g.is_char_ref() {
                    g.resolve_char_ref()
                        .map_err(|e| xml_erro(src, pos, e))?
                        .ok_or_else(|| xml_erro(src, pos, "referencia de caractere invalida"))?;
                }
                if !conhecida && !tem_dtd {
                    return Err(xml_erro(
                        src,
                        pos,
                        format!("entidade nao declarada &{};", &**g),
                    ));
                }
            }
            Event::DocType(_) => tem_dtd = true,
            _ => {}
        }
        cada(&ev)?;
    }
}

fn xml_validar(src: &str) -> Result<(), ToolError> {
    xml_percorrer(src, |_| Ok(()))
}

/// Reindenta descartando so o texto que e todo espaco (a formatacao antiga). Texto com
/// conteudo sai como veio, sem aparar: aparar juntaria «a &amp; b» em «a&b».
fn xml_bonito(src: &str) -> Result<String, ToolError> {
    use quick_xml::events::{BytesText, Event};
    let mut w = quick_xml::Writer::new_with_indent(Vec::new(), b' ', 2);
    xml_percorrer(src, |ev| {
        let r = match ev {
            Event::Text(t) if t.trim().is_empty() => Ok(()),
            // Referencia vai como texto: escrita como evento proprio ela ganharia quebra
            // de linha e recuo na frente, que viram conteudo do elemento.
            Event::GeneralRef(g) => {
                w.write_event(Event::Text(BytesText::from_escaped(format!("&{};", &**g))))
            }
            Event::Eof => Ok(()),
            outro => w.write_event(outro.borrow()),
        };
        r.map_err(falha)
    })?;
    String::from_utf8(w.into_inner()).map_err(falha)
}

fn nome_casa(seg: &str, nome: &str) -> bool {
    seg == "*" || seg == nome || nome.rsplit(':').next() == Some(seg)
}

/// Caminho de elementos `a/b/c` a partir da raiz, com `*` para qualquer nome e `@attr`
/// no fim para ler atributo. Devolve o texto de cada elemento casado (descendentes
/// inclusive), na ordem do documento.
fn xml_consultar(src: &str, caminho: &str) -> Result<Vec<String>, ToolError> {
    use quick_xml::events::Event;
    let mut segs: Vec<&str> = caminho
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let atributo = match segs.last() {
        Some(s) if s.starts_with('@') => Some(segs.pop().unwrap_or_default()[1..].to_string()),
        _ => None,
    };
    if segs.is_empty() {
        return Err(ToolError::InvalidArguments(
            "caminho XML vazio: use a/b/c a partir da raiz".into(),
        ));
    }
    let mut pilha: Vec<String> = Vec::new();
    let mut achados = Vec::new();
    // (profundidade em que o casamento abriu, texto acumulado)
    let mut captura: Option<(usize, String)> = None;
    let casa = |pilha: &[String]| {
        pilha.len() == segs.len() && segs.iter().zip(pilha).all(|(s, n)| nome_casa(s, n))
    };
    xml_percorrer(src, |ev| {
        match ev {
            Event::Start(e) | Event::Empty(e) => {
                pilha.push(e.name().as_ref().to_string());
                if casa(&pilha) {
                    if let Some(a) = &atributo {
                        for at in e.attributes().flatten() {
                            let k = at.key.as_ref().to_string();
                            if nome_casa(a, &k) {
                                achados.push(
                                    at.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                        .map_err(falha)?
                                        .into_owned(),
                                );
                            }
                        }
                    } else if captura.is_none() {
                        captura = Some((pilha.len(), String::new()));
                    }
                }
                if matches!(ev, Event::Empty(_)) {
                    if let Some((_, t)) = captura.take_if(|(d, _)| *d == pilha.len()) {
                        achados.push(t);
                    }
                    pilha.pop();
                }
            }
            Event::End(_) => {
                if let Some((_, t)) = captura.take_if(|(d, _)| *d == pilha.len()) {
                    achados.push(t.trim().to_string());
                }
                pilha.pop();
            }
            Event::Text(t) => {
                if let Some((_, s)) = &mut captura {
                    s.push_str(&t.xml10_content());
                }
            }
            Event::CData(t) => {
                if let Some((_, s)) = &mut captura {
                    s.push_str(t);
                }
            }
            Event::GeneralRef(g) => {
                if let Some((_, s)) = &mut captura {
                    let c = if g.is_char_ref() {
                        g.resolve_char_ref().ok().flatten()
                    } else {
                        ENTIDADES.iter().find(|(n, _)| **g == **n).map(|(_, c)| *c)
                    };
                    match c {
                        Some(c) => s.push(c),
                        None => s.push_str(&format!("&{};", &**g)),
                    }
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(achados)
}

#[derive(Clone, Copy, PartialEq)]
enum Formato {
    Json,
    Xml,
}

fn formato(rel: &str) -> Result<Formato, ToolError> {
    match extensao(rel).as_str() {
        "json" => Ok(Formato::Json),
        "xml" | "svg" | "xsd" | "xsl" | "xslt" | "rss" | "atom" => Ok(Formato::Xml),
        _ => Err(ToolError::InvalidArguments(format!(
            "{rel}: so .json ou .xml (e .svg/.xsd/.xsl/.rss/.atom)"
        ))),
    }
}

fn ler_texto_dados(ctx: &ToolContext, rel: &str) -> Result<(Formato, String, String), ToolError> {
    let f = formato(rel)?;
    let (alvo, limpo) = confinar(ctx, rel)?;
    let b = ler_limitado(&alvo, MAX_DADOS_BYTES)?;
    // BOM de UTF-8 e comum em arquivo salvo pelo Bloco de Notas e nao e erro de sintaxe.
    let b = b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&b).to_vec();
    let s = String::from_utf8(b).map_err(|e| {
        ToolError::Failed(format!(
            "{limpo} nao e UTF-8 valido (byte {})",
            e.utf8_error().valid_up_to()
        ))
    })?;
    Ok((f, limpo, s))
}

fn formatado(f: Formato, s: &str) -> Result<String, ToolError> {
    match f {
        Formato::Json => {
            json_validar(s)?;
            Ok(json_bonito(s))
        }
        Formato::Xml => xml_bonito(s),
    }
}

pub struct DataFileTool;

impl Tool for DataFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "data_file".into(),
            description: "Work with a .json or .xml file of the task directory. action=validate reports errors with line/column; action=format returns it pretty-printed (use data_file_format to save); action=query returns the value at 'query': a JSON Pointer (RFC 6901, e.g. /items/0/name) for JSON, or an element path from the root (e.g. catalog/book/title, '*' matches any name, a final '@attr' reads an attribute) for XML.".into(),
            parameters: json!({"type":"object","properties":{
                "path":{"type":"string"},
                "action":{"type":"string","enum":["validate","format","query"]},
                "query":{"type":"string"}
            },"required":["path","action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let (f, limpo, s) = ler_texto_dados(ctx, arg(&args, "path")?)?;
            let t = match (arg(&args, "action")?, f) {
                ("validate", Formato::Json) => {
                    format!("{limpo}: JSON valido, {}", resumo_json(&json_validar(&s)?))
                }
                ("validate", Formato::Xml) => {
                    xml_validar(&s)?;
                    format!("{limpo}: XML bem formado")
                }
                ("format", _) => formatado(f, &s)?,
                ("query", Formato::Json) => {
                    json_consultar(&json_validar(&s)?, arg(&args, "query")?)?
                }
                ("query", Formato::Xml) => {
                    let q = arg(&args, "query")?;
                    let achados = xml_consultar(&s, q)?;
                    if achados.is_empty() {
                        return Err(ToolError::Failed(format!("nada casou com {q:?}")));
                    }
                    let n = achados.len();
                    let mut t: Vec<String> = achados
                        .into_iter()
                        .take(200)
                        .enumerate()
                        .map(|(i, a)| format!("[{i}] {a}"))
                        .collect();
                    if n > 200 {
                        t.push(format!("[... {n} no total]"));
                    }
                    t.join("\n")
                }
                (outra, _) => {
                    return Err(ToolError::InvalidArguments(format!(
                        "action desconhecida: {outra} (validate, format ou query)"
                    )));
                }
            };
            Ok(ToolOutput::text(truncate_for_model(&t, MAX_CHARS)))
        })
    }
}

/// Formatar gravando e outra capacidade (`fs.write`) e precisa ser feito aqui, e nao pelo
/// modelo copiando a saida do `data_file`: a saida volta truncada, e regravar o truncado
/// apagaria o resto do arquivo.
pub struct DataFileFormatTool;

impl Tool for DataFileFormatTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "data_file_format".into(),
            description: "Validate and pretty-print a .json or .xml file of the task directory and save it (in place, or to 'output'). Key order and numbers are kept as written.".into(),
            parameters: json!({"type":"object","properties":{"path":{"type":"string"},"output":{"type":"string"}},"required":["path"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let rel = arg(&args, "path")?;
            let (f, limpo, s) = ler_texto_dados(ctx, rel)?;
            let saida_rel = args.get("output").and_then(Value::as_str).unwrap_or(rel);
            if formato(saida_rel)? != f {
                return Err(ToolError::InvalidArguments(
                    "output tem de ter a mesma extensao do arquivo".into(),
                ));
            }
            let (saida, saida_limpo) = confinar(ctx, saida_rel)?;
            let mut t = formatado(f, &s)?;
            t.push('\n');
            if let Some(d) = saida.parent() {
                std::fs::create_dir_all(d).map_err(falha)?;
            }
            std::fs::write(&saida, t).map_err(falha)?;
            let a = artefato(ctx, &saida_limpo)?;
            Ok(ToolOutput {
                content: format!("{limpo} formatado em {saida_limpo} ({} bytes)", a.bytes),
                artifacts: vec![a],
            })
        })
    }
}

// ---------------------------------------------------------------- pdf

fn pagina(args: &Value, n: &str) -> Result<Option<u64>, ToolError> {
    match args.get(n) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_u64() {
            Some(p) if p >= 1 => Ok(Some(p)),
            _ => Err(ToolError::InvalidArguments(format!(
                "{n} e numero de pagina a partir de 1"
            ))),
        },
    }
}

pub struct PdfTool;

impl Tool for PdfTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "pdf".into(),
            description: "Read a .pdf of the task directory. action=info returns page count, title and metadata; action=text returns the text (layout kept), optionally only pages first_page..last_page.".into(),
            parameters: json!({"type":"object","properties":{
                "path":{"type":"string"},
                "action":{"type":"string","enum":["info","text"]},
                "first_page":{"type":"integer","minimum":1},
                "last_page":{"type":"integer","minimum":1}
            },"required":["path","action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    /// Conversor no bwrap (processo): a regra de comando a alcanca por `pdf <action>`.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        Some(crate::motor::linha_sintetica(
            "pdf",
            args,
            &["action", "path"],
        ))
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let rel = arg(&args, "path")?;
            exige_extensao(rel, &[".pdf"])?;
            let (alvo, limpo) = confinar(ctx, rel)?;
            if !alvo.is_file() {
                return Err(ToolError::InvalidArguments(format!("{limpo} nao existe")));
            }
            match arg(&args, "action")? {
                "info" => {
                    exige_programa("pdfinfo", "poppler-utils")?;
                    let o = no_sandbox(
                        ctx,
                        format!("pdfinfo -enc UTF-8 {}", aspas(&format!("/work/{limpo}"))),
                        SandboxExtras::default(),
                    )
                    .await?;
                    let campo = |k: &str| {
                        o.stdout
                            .lines()
                            .find_map(|l| l.strip_prefix(k).map(|v| v.trim().to_string()))
                            .unwrap_or_default()
                    };
                    Ok(ToolOutput::text(format!(
                        "paginas: {}\ntitulo: {}\n\n{}",
                        campo("Pages:"),
                        campo("Title:"),
                        truncate_for_model(o.stdout.trim(), MAX_CHARS)
                    )))
                }
                "text" => {
                    let (p, u) = (pagina(&args, "first_page")?, pagina(&args, "last_page")?);
                    if let (Some(p), Some(u)) = (p, u)
                        && p > u
                    {
                        return Err(ToolError::InvalidArguments(format!(
                            "faixa vazia: {p}..{u}"
                        )));
                    }
                    let t = pdf_texto(ctx, &limpo, p, u).await?;
                    Ok(ToolOutput::text(truncate_for_model(&t, MAX_CHARS)))
                }
                outra => Err(ToolError::InvalidArguments(format!(
                    "action desconhecida: {outra} (info ou text)"
                ))),
            }
        })
    }
}

pub struct PdfCreateTool;

/// Pastas do hospedeiro que o LibreOffice precisa ler alem do sandbox do shell. Medido em
/// 01/10/2026: sem `/etc/libreoffice` o `soffice` aborta (sinal 6, `RuntimeException`), e
/// sem `/etc/fonts` nao ha fonte para desenhar. O `/etc` inteiro nao entra: ele traria o
/// resto da configuracao do hospedeiro para dentro.
const SOFFICE_ETC: [&str; 2] = ["/etc/fonts", "/etc/libreoffice"];

impl Tool for PdfCreateTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "pdf_create".into(),
            description: "Convert a .docx, .odt, .html or .txt of the task directory to PDF with LibreOffice (no network: remote images in HTML are not fetched).".into(),
            parameters: json!({"type":"object","properties":{
                "source":{"type":"string"},
                "output":{"type":"string","description":"relative .pdf path (default: source with .pdf)"}
            },"required":["source"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.write"
    }
    /// Conversor no bwrap (processo): a regra de comando a alcanca por `pdf_create <source>`.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        Some(crate::motor::linha_sintetica(
            "pdf_create",
            args,
            &["source"],
        ))
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let src = arg(&args, "source")?;
            exige_extensao(src, &[".docx", ".odt", ".html", ".htm", ".txt", ".rtf"])?;
            let (src_abs, src_limpo) = confinar(ctx, src)?;
            if !src_abs.is_file() {
                return Err(ToolError::InvalidArguments(format!(
                    "{src_limpo} nao existe"
                )));
            }
            let padrao = Path::new(&src_limpo)
                .with_extension("pdf")
                .to_string_lossy()
                .into_owned();
            let out = args
                .get("output")
                .and_then(Value::as_str)
                .unwrap_or(&padrao)
                .to_string();
            exige_extensao(&out, &[".pdf"])?;
            let (out_abs, out_limpo) = confinar(ctx, &out)?;
            exige_programa("soffice", "libreoffice-writer")?;
            if let Some(d) = out_abs.parent() {
                std::fs::create_dir_all(d).map_err(falha)?;
            }
            // Sem apagar a saida antiga antes: se a conversao falha o `cp` do glob vazio
            // falha junto, e o PDF que ja existia continua la.
            // Perfil e saida no /tmp do sandbox (tmpfs): somem com ele, e dois soffice em
            // paralelo nao disputam a trava de um perfil comum.
            let script = format!(
                "soffice --headless --norestore --nolockcheck \
-env:UserInstallation=file:///tmp/perfil --convert-to pdf --outdir /tmp/saida {} \
&& cp /tmp/saida/*.pdf {}",
                aspas(&format!("/work/{src_limpo}")),
                aspas(&format!("/work/{out_limpo}"))
            );
            let extras = SandboxExtras {
                ro_binds: SOFFICE_ETC
                    .iter()
                    .filter(|p| Path::new(p).exists())
                    .map(|p| (PathBuf::from(p), p.to_string()))
                    .collect(),
                env: vec![("HOME".into(), "/tmp".into())],
            };
            let o = no_sandbox(ctx, script, extras).await?;
            let ok = std::fs::read(&out_abs)
                .map(|b| b.starts_with(b"%PDF"))
                .unwrap_or(false);
            if !ok {
                return Err(ToolError::Failed(format!(
                    "o LibreOffice nao gerou {out_limpo}: {}",
                    truncate_for_model(format!("{}\n{}", o.stdout, o.stderr).trim(), 2_000)
                )));
            }
            let a = artefato(ctx, &out_limpo)?;
            Ok(ToolOutput {
                content: format!(
                    "criado {out_limpo} a partir de {src_limpo} ({} bytes)",
                    a.bytes
                ),
                artifacts: vec![a],
            })
        })
    }
}

// ---------------------------------------------------------------- leitura de documento

/// Extensoes lidas como texto UTF-8 pelo `read_document`.
pub const TEXTO_PURO: &[&str] = &[
    "txt", "md", "csv", "html", "htm", "css", "js", "json", "xml", "svg",
];

/// O corpo do `read_document`: um so lugar decide como cada formato vira texto, e a
/// ferramenta `pdf` le pelo mesmo `pdf_texto`.
pub async fn ler_documento(ctx: &ToolContext, rel: &str) -> Result<String, ToolError> {
    let (alvo, limpo) = confinar(ctx, rel)?;
    let of = |e: phxclaw_office::OfficeError| falha(e);
    let ext = extensao(&limpo);
    let texto = match ext.as_str() {
        "docx" => phxclaw_office::read_docx_text(&alvo).map_err(of)?,
        "xlsx" => {
            let wb = phxclaw_office::read_xlsx_values(&alvo).map_err(of)?;
            serde_json::to_string_pretty(&wb).unwrap_or_default()
        }
        "pptx" => phxclaw_office::read_pptx_text(&alvo).map_err(of)?,
        "pdf" => {
            if !alvo.is_file() {
                return Err(ToolError::InvalidArguments(format!("{limpo} nao existe")));
            }
            pdf_texto(ctx, &limpo, None, None).await?
        }
        e if TEXTO_PURO.contains(&e) => {
            let b = ler_limitado(&alvo, MAX_DADOS_BYTES)?;
            let (s, aviso) = match String::from_utf8(b) {
                Ok(s) => (s, ""),
                Err(e) => (
                    String::from_utf8_lossy(e.as_bytes()).into_owned(),
                    "[aviso: bytes invalidos em UTF-8 trocados por \u{FFFD}]\n",
                ),
            };
            if e == "html" || e == "htm" {
                // Metade do orcamento para cada vista: o texto limpo e o que o modelo
                // le; a marcacao e o que ele precisa para editar o arquivo.
                let r = phxclaw_web_search::readable(&s, None);
                return Ok(format!(
                    "{aviso}--- texto sem marcacao (titulo: {}) ---\n{}\n\n--- HTML ---\n{}",
                    r.title,
                    truncate_for_model(&r.text, MAX_CHARS / 2),
                    truncate_for_model(&s, MAX_CHARS / 2)
                ));
            }
            format!("{aviso}{s}")
        }
        _ => {
            return Err(ToolError::InvalidArguments(format!(
                "{limpo}: formatos lidos: docx, xlsx, pptx, pdf, {}",
                TEXTO_PURO.join(", ")
            )));
        }
    };
    Ok(truncate_for_model(&texto, MAX_CHARS))
}

/// As ferramentas desta frente, para a montagem registrar de uma vez.
pub fn arquivos_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ZipListTool),
        Arc::new(ZipTool),
        Arc::new(DataFileTool),
        Arc::new(DataFileFormatTool),
        Arc::new(PdfTool),
        Arc::new(PdfCreateTool),
    ]
}

#[cfg(test)]
mod tests;
