//! A loja de plugins: o indice do `phxclaw-community-registry` dentro do agente, como
//! ferramenta (`plugin_catalog`) e como CLI (`phxclaw plugins catalogo|instalar`), pela
//! MESMA `Loja`. Antes a crate era so biblioteca: ninguem a chamava.
//!
//! O que a instalacao exige, nesta ordem, e nada se pula:
//! 1. o indice se valida (`CommunityRegistryIndex::validate`): entrada invalida derruba o
//!    catalogo inteiro, nao so a entrada;
//! 2. a versao escolhida e a mais nova nao retirada e compativel com `core_api`;
//! 3. os bytes baixados batem com `package_sha256` -- antes de abrir qualquer coisa;
//! 4. o pacote e a pasta assinada do `pacotes.rs` (`.claude-plugin`/`.codex-plugin` com
//!    `.phxclaw-assinatura.json`), conferida contra o MESMO trust store dos plugins; o que
//!    nao passa e apagado, nunca fica «instalado mas desligado».
//!
//! O pacote viaja como `.tar` (ustar, sem compressao), lido e escrito aqui -- sem crate
//! nova. Link simbolico, caminho absoluto e `..` sao recusados na leitura: o arquivo so
//! pode escrever dentro da pasta de destino.

use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_community_registry::{CommunityPluginRelease, CommunityRegistryIndex, Version};
use phxclaw_plugin_registry::TrustStore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A capacidade da ferramenta; fora do padrao: instalar codigo e decisao do operador.
pub const CAPACIDADE: &str = "plugin.catalog";
/// Teto do pacote baixado.
pub const PACOTE_MAX: usize = 64 * 1024 * 1024;

pub struct Loja {
    /// URL `https://` ou caminho de arquivo do indice JSON.
    pub catalogo: String,
    /// Onde os pacotes instalados ficam (`pacotes.dir`): cada subpasta e um pacote.
    pub destino: PathBuf,
    pub trust: TrustStore,
    /// A versao deste agente, para `core_api`.
    pub nucleo: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instalado {
    pub nome: String,
    pub versao: String,
    pub caminho: PathBuf,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// O SHA-256 em hexadecimal, o mesmo que o catalogo guarda em `package_sha256`.
pub fn sha256_hex(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

impl Loja {
    /// A loja da configuracao: `pacotes.catalogo` (sem ele, `None`), `pacotes.dir` e o
    /// trust store de `plugins.assinantes` (senao `<plugins.raiz>/config/trust/...`).
    /// Trust store ausente e erro: sem ele nada se instalaria, e a loja mentiria.
    pub fn da_configuracao() -> Result<Option<Loja>, String> {
        let Some(catalogo) = crate::config::texto("pacotes.catalogo")? else {
            return Ok(None);
        };
        let destino = crate::config::texto("pacotes.dir")?
            .map(PathBuf::from)
            .ok_or("pacotes.dir (PHXCLAW_PACOTES_DIR) nao definido: onde instalar?")?;
        let signers = match crate::config::texto("plugins.assinantes")? {
            Some(p) => PathBuf::from(p),
            None => crate::config::texto("plugins.raiz")?
                .map(|r| PathBuf::from(r).join("config/trust/plugin-signers.json"))
                .ok_or("plugins.assinantes (PHXCLAW_PLUGIN_SIGNERS) nao definido")?,
        };
        let trust = TrustStore::from_json(
            &std::fs::read_to_string(&signers)
                .map_err(|e| format!("{}: {e}", signers.display()))?,
        )
        .map_err(|e| e.to_string())?;
        Ok(Some(Loja {
            catalogo,
            destino,
            trust,
            nucleo: Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?,
        }))
    }

    async fn baixar(&self, origem: &str) -> Result<Vec<u8>, String> {
        if let Some(p) = origem.strip_prefix("file://") {
            return std::fs::read(p).map_err(|e| format!("{p}: {e}"));
        }
        if !origem.starts_with("https://") {
            // O indice local e um caminho; qualquer outro esquema nao entra.
            if !origem.contains("://") {
                return std::fs::read(origem).map_err(|e| format!("{origem}: {e}"));
            }
            return Err(format!("origem {origem}: so https:// ou arquivo local"));
        }
        let r = reqwest::get(origem).await.map_err(|e| e.to_string())?;
        if !r.status().is_success() {
            return Err(format!("{origem}: {}", r.status()));
        }
        let b = r.bytes().await.map_err(|e| e.to_string())?;
        if b.len() > PACOTE_MAX {
            return Err(format!("{origem}: acima de {PACOTE_MAX} bytes"));
        }
        Ok(b.to_vec())
    }

    /// O indice, lido e validado.
    pub async fn indice(&self) -> Result<CommunityRegistryIndex, String> {
        let bytes = self.baixar(&self.catalogo).await?;
        let texto = String::from_utf8(bytes).map_err(|e| e.to_string())?;
        let idx = CommunityRegistryIndex::from_json(&texto).map_err(|e| e.to_string())?;
        let rel = idx.validate();
        if !rel.valid {
            return Err(format!("catalogo invalido: {}", rel.errors.join("; ")));
        }
        Ok(idx)
    }

    /// As versoes mais novas compativeis, uma por nome, filtradas por `busca` (nome,
    /// categoria ou capacidade, sem caixa).
    pub fn listar<'a>(
        &self,
        idx: &'a CommunityRegistryIndex,
        busca: Option<&str>,
    ) -> Vec<&'a CommunityPluginRelease> {
        let b = busca
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        let mut vistos = std::collections::BTreeSet::new();
        idx.compatible_releases(&self.nucleo)
            .into_iter()
            .filter(|r| vistos.insert(r.name.clone()))
            .filter(|r| match &b {
                None => true,
                Some(b) => {
                    r.name.to_ascii_lowercase().contains(b.as_str())
                        || r.categories
                            .iter()
                            .any(|c| c.to_ascii_lowercase().contains(b.as_str()))
                        || r.capabilities
                            .iter()
                            .any(|c| c.to_ascii_lowercase().contains(b.as_str()))
                }
            })
            .collect()
    }

    pub fn ficha(r: &CommunityPluginRelease) -> Value {
        json!({
            "nome": r.name, "versao": r.version, "publicador": r.publisher_id,
            "licenca": r.license, "categorias": r.categories, "capacidades": r.capabilities,
            "pontos_de_extensao": r.extension_points, "fonte": r.source_repository,
            "instalado": false,
        })
    }

    /// As fichas das versoes compativeis, com `instalado` conferido na pasta de destino:
    /// a ferramenta `plugin_catalog` e a rota `/v1/plugins/catalogo` leem daqui.
    pub async fn fichas(&self, busca: Option<&str>) -> Result<Vec<Value>, String> {
        let idx = self.indice().await?;
        Ok(self
            .listar(&idx, busca)
            .into_iter()
            .map(|r| {
                let mut f = Loja::ficha(r);
                f["instalado"] = json!(self.destino.join(&r.name).exists());
                f
            })
            .collect())
    }

    /// Instala `nome` (a versao mais nova compativel) em `<destino>/<nome>`.
    pub async fn instalar(&self, nome: &str) -> Result<Instalado, String> {
        let idx = self.indice().await?;
        let r = self
            .listar(&idx, None)
            .into_iter()
            .find(|r| r.name == nome)
            .ok_or_else(|| {
                format!(
                    "pacote {nome} nao esta no catalogo (ou nenhuma versao e compativel com {})",
                    self.nucleo
                )
            })?
            .clone();
        let alvo = self.destino.join(&r.name);
        if alvo.exists() {
            return Err(format!(
                "{} ja existe: remova a pasta antes de reinstalar",
                alvo.display()
            ));
        }
        let bytes = self.baixar(&r.package_url).await?;
        let visto = sha256_hex(&bytes);
        if visto != r.package_sha256.to_ascii_lowercase() {
            return Err(format!(
                "pacote {}: sha256 {visto} difere do catalogo ({}); nada foi aberto",
                r.name, r.package_sha256
            ));
        }
        std::fs::create_dir_all(&self.destino).map_err(|e| e.to_string())?;
        let tmp = self
            .destino
            .join(format!(".instalando-{}", phxclaw_types::new_uuid_v7()));
        let feito = (|| -> Result<(), String> {
            std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
            desempacotar(&bytes, &tmp)?;
            let p = crate::pacotes::ler(&tmp, &self.trust)?;
            if p.nome != r.name {
                return Err(format!(
                    "o pacote se chama {}, o catalogo diz {}",
                    p.nome, r.name
                ));
            }
            std::fs::rename(&tmp, &alvo).map_err(|e| e.to_string())
        })();
        if let Err(e) = feito {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(format!("pacote {} recusado: {e}", r.name));
        }
        Ok(Instalado {
            nome: r.name.clone(),
            versao: r.version.clone(),
            caminho: alvo,
        })
    }
}

// ------------------------------------------------------------------ ustar

const BLOCO: usize = 512;

fn campo(h: &mut [u8], ini: usize, tam: usize, v: &str) {
    let b = v.as_bytes();
    let n = b.len().min(tam);
    h[ini..ini + n].copy_from_slice(&b[..n]);
}

fn octal(h: &mut [u8], ini: usize, tam: usize, v: u64) {
    campo(h, ini, tam, &format!("{:0width$o}", v, width = tam - 1));
}

fn cabecalho(nome: &str, tamanho: u64, tipo: u8) -> Result<[u8; BLOCO], String> {
    let mut h = [0u8; BLOCO];
    let (prefixo, base) = if nome.len() <= 100 {
        ("", nome)
    } else {
        let i = nome[..nome.len().min(156)]
            .rfind('/')
            .ok_or_else(|| format!("{nome}: nome longo demais para o ustar"))?;
        (&nome[..i], &nome[i + 1..])
    };
    if base.len() > 100 || prefixo.len() > 155 {
        return Err(format!("{nome}: nome longo demais para o ustar"));
    }
    campo(&mut h, 0, 100, base);
    octal(&mut h, 100, 8, 0o644);
    octal(&mut h, 108, 8, 0);
    octal(&mut h, 116, 8, 0);
    octal(&mut h, 124, 12, tamanho);
    octal(&mut h, 136, 12, 0);
    h[156] = tipo;
    campo(&mut h, 257, 6, "ustar\0");
    campo(&mut h, 263, 2, "00");
    campo(&mut h, 345, 155, prefixo);
    // Soma de verificacao: o campo conta como espacos.
    h[148..156].fill(b' ');
    let soma: u64 = h.iter().map(|&b| u64::from(b)).sum();
    campo(&mut h, 148, 8, &format!("{soma:06o}\0 "));
    Ok(h)
}

fn arquivos_de(raiz: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut es: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        let t = e.file_type().map_err(|x| x.to_string())?;
        if t.is_symlink() {
            return Err(format!(
                "{}: link simbolico nao entra no pacote",
                p.strip_prefix(raiz).unwrap_or(&p).display()
            ));
        }
        if t.is_dir() {
            if e.file_name() == ".git" {
                continue;
            }
            arquivos_de(raiz, &p, out)?;
        } else {
            out.push(p);
        }
    }
    Ok(())
}

/// A pasta inteira num `.tar` ustar (arquivos em ordem de caminho; pastas implicitas).
pub fn empacotar(dir: &Path) -> Result<Vec<u8>, String> {
    let mut lista = Vec::new();
    arquivos_de(dir, dir, &mut lista)?;
    let mut out = Vec::new();
    for p in lista {
        let rel = p
            .strip_prefix(dir)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(&p).map_err(|e| format!("{rel}: {e}"))?;
        out.extend_from_slice(&cabecalho(&rel, bytes.len() as u64, b'0')?);
        out.extend_from_slice(&bytes);
        let resto = (BLOCO - bytes.len() % BLOCO) % BLOCO;
        out.extend(std::iter::repeat_n(0u8, resto));
    }
    out.extend(std::iter::repeat_n(0u8, 2 * BLOCO));
    Ok(out)
}

fn texto_do_campo(h: &[u8], ini: usize, tam: usize) -> String {
    let c = &h[ini..ini + tam];
    let fim = c.iter().position(|&b| b == 0).unwrap_or(c.len());
    String::from_utf8_lossy(&c[..fim]).to_string()
}

fn caminho_seguro(rel: &str) -> Result<PathBuf, String> {
    let r = rel.trim_end_matches('/');
    if r.is_empty()
        || r.starts_with('/')
        || r.contains('\\')
        || r.split('/').any(|s| s.is_empty() || s == "." || s == "..")
    {
        return Err(format!("{rel:?}: caminho recusado no pacote"));
    }
    Ok(PathBuf::from(r))
}

/// Abre o `.tar` em `destino`. Devolve quantos arquivos gravou. Link, dispositivo e
/// caminho fora da pasta sao erro -- e o chamador apaga o que ja saiu.
pub fn desempacotar(bytes: &[u8], destino: &Path) -> Result<usize, String> {
    let mut i = 0;
    let mut n = 0;
    while i + BLOCO <= bytes.len() {
        let h = &bytes[i..i + BLOCO];
        if h.iter().all(|&b| b == 0) {
            break;
        }
        let nome = texto_do_campo(h, 0, 100);
        let prefixo = texto_do_campo(h, 345, 155);
        let completo = if prefixo.is_empty() {
            nome
        } else {
            format!("{prefixo}/{nome}")
        };
        let tamanho = u64::from_str_radix(texto_do_campo(h, 124, 12).trim(), 8)
            .map_err(|_| format!("{completo}: tamanho ilegivel"))? as usize;
        let tipo = h[156];
        i += BLOCO;
        let dados = bytes
            .get(i..i + tamanho)
            .ok_or_else(|| format!("{completo}: pacote truncado"))?;
        i += tamanho.div_ceil(BLOCO) * BLOCO;
        let rel = caminho_seguro(&completo)?;
        match tipo {
            b'0' | 0 => {
                let p = destino.join(&rel);
                if let Some(pai) = p.parent() {
                    std::fs::create_dir_all(pai).map_err(|e| e.to_string())?;
                }
                std::fs::write(&p, dados).map_err(|e| format!("{completo}: {e}"))?;
                n += 1;
            }
            b'5' => {
                std::fs::create_dir_all(destino.join(&rel)).map_err(|e| e.to_string())?;
            }
            outro => {
                return Err(format!(
                    "{completo}: tipo {:?} nao entra no pacote (so arquivo e pasta)",
                    outro as char
                ));
            }
        }
    }
    Ok(n)
}

// ------------------------------------------------------------------ a ferramenta

pub struct PluginCatalogTool {
    pub loja: Arc<Loja>,
}

/// A loja da configuracao como ferramenta; sem `pacotes.catalogo`, nenhuma.
pub fn ferramenta_da_configuracao() -> Option<Arc<dyn Tool>> {
    match Loja::da_configuracao() {
        Ok(Some(l)) => Some(Arc::new(PluginCatalogTool { loja: Arc::new(l) })),
        Ok(None) => None,
        Err(e) => {
            eprintln!("aviso: loja de plugins desligada: {e}");
            None
        }
    }
}

impl Tool for PluginCatalogTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "plugin_catalog".into(),
            description: "Plugin store: action=list shows the signed packages available for this \
agent; action=search with query filters by name, category or capability; action=install with \
name downloads a package, checks its sha256 against the catalog and its Ed25519 signature \
against the trust store, and installs it (the agent loads it on the next start)."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["list","search","install"]},
                "query":{"type":"string"},
                "name":{"type":"string"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        CAPACIDADE
    }
    /// Instalar e trazer codigo de fora: a linha `plugin_catalog install <nome>` deixa as
    /// regras de comando do operador alcancarem a instalacao no mesmo portao do shell.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        match args.get("action").and_then(Value::as_str) {
            Some("install") => Some(format!(
                "plugin_catalog install {}",
                args.get("name").and_then(Value::as_str).unwrap_or("")
            )),
            _ => None,
        }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = args
                .get("action")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'action'".into()))?;
            match acao {
                "list" | "search" => {
                    let busca = args.get("query").and_then(Value::as_str);
                    let fichas = self.loja.fichas(busca).await.map_err(ToolError::Failed)?;
                    Ok(ToolOutput::text(
                        serde_json::to_string_pretty(&fichas).unwrap_or_default(),
                    ))
                }
                "install" => {
                    let nome = args
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| ToolError::InvalidArguments("falta 'name'".into()))?;
                    let i = self.loja.instalar(nome).await.map_err(ToolError::Failed)?;
                    Ok(ToolOutput::text(format!(
                        "instalado {} {} em {} (entra no proximo arranque do agente)",
                        i.nome,
                        i.versao,
                        i.caminho.display()
                    )))
                }
                outro => Err(ToolError::InvalidArguments(format!(
                    "action {outro:?}: list, search ou install"
                ))),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tar_vai_e_volta_e_recusa_caminho_fora() {
        let d = std::env::temp_dir().join(format!("phx-tar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("o/sub/x")).unwrap();
        std::fs::write(d.join("o/a.txt"), b"aaa").unwrap();
        std::fs::write(d.join("o/sub/x/b.bin"), vec![7u8; 1000]).unwrap();
        let longo = format!("o/{}/c.txt", "p".repeat(120));
        std::fs::create_dir_all(d.join(&longo).parent().unwrap()).unwrap();
        std::fs::write(d.join(&longo), b"c").unwrap();
        let t = empacotar(&d.join("o")).unwrap();
        assert_eq!(t.len() % BLOCO, 0);
        let n = desempacotar(&t, &d.join("v")).unwrap();
        assert_eq!(n, 3);
        assert_eq!(std::fs::read(d.join("v/a.txt")).unwrap(), b"aaa");
        assert_eq!(std::fs::read(d.join("v/sub/x/b.bin")).unwrap().len(), 1000);
        assert_eq!(
            std::fs::read(d.join("v").join(longo.trim_start_matches("o/"))).unwrap(),
            b"c"
        );
        // cabecalho com `..` e recusado
        let mut mau = cabecalho("../fora.txt", 1, b'0').unwrap().to_vec();
        mau.extend_from_slice(b"x");
        mau.extend(std::iter::repeat_n(0u8, 3 * BLOCO - 1));
        assert!(
            desempacotar(&mau, &d.join("w"))
                .unwrap_err()
                .contains("recusado")
        );
        assert!(!d.join("fora.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
