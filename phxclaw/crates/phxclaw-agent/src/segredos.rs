//! Varredura de segredos antes de gravar no git: o hook nativo de `PreToolUse` do
//! `git_write` nas acoes `add` e `commit`, com o binario oficial do gitleaks (MIT, versao
//! travada pelo SHA-256 que o operador declara).
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Mora na ferramenta, nao no `hooks.json`.** O `parallel_tasks` (nuvem.rs) faz `add` e
//!   `commit` chamando o `GitTool` direto, sem passar pelo portao do motor; uma guarda no
//!   portao deixaria esse caminho irmao gravar segredo calado. Dentro de `escrever`, todo
//!   `add`/`commit` passa por ela -- o do modelo, o do `mcp-serve` e o da nuvem.
//! - **Varre o que SERIA gravado, sem tocar no indice de verdade.** O indice e copiado para
//!   um temporario do sandbox, o `add` (ou o `add -u` do `commit -a`) roda nele, e o diff
//!   `--cached` dele e o que o commit levaria. Recusar depois de `git add` exigiria desfazer
//!   o indice do operador, que pode ter coisa preparada antes.
//! - **O gitleaks nao roda git nem le regra da arvore.** O diff sai do motor do git desta
//!   casa (`rodar_roteiro_git`, com filtros e textconv do repositorio neutralizados); o
//!   gitleaks recebe pelo stdin so as linhas ADICIONADAS e roda numa pasta vazia. Rodando
//!   `gitleaks git` no repositorio, ele chamaria `git diff` sem a neutralizacao e leria o
//!   `.gitleaks.toml` e o `.gitleaksignore` da arvore -- que o modelo escreve. Pelo mesmo
//!   motivo vai `--ignore-gitleaks-allow`: o comentario `gitleaks:allow` e do autor do
//!   codigo, e aqui o autor e o modelo (medido: a mesma linha com o comentario passa calada
//!   sem a opcao e e achada com ela).
//! - **O achado sai redigido na origem.** `--redact=100` faz o gitleaks nao escrever o
//!   segredo nem no relatorio; daqui so sai arquivo, linha e regra.
//! - **Sem o binario configurado, nem bloqueia calado nem libera calado.** O resultado do
//!   `add`/`commit` diz que a varredura NAO foi feita e por que; quem quer que seja
//!   obrigatoria liga `git.segredos.exigir` e a gravacao e recusada. Configurado e quebrado
//!   (hash que nao confere, binario ausente, sandbox que falha) sempre fecha: guarda que o
//!   operador ligou e nao roda nao pode virar guarda desligada.

use crate::git::{ArquivoDiff, analisar_diff, rodar_roteiro_git};
use crate::python::aspas;
use phxclaw_agent_core::ToolError;
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, run_in_workdir_com};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Teto do diff varrido. Acima dele a guarda fecha: varrer so o comeco e dizer «limpo»
/// seria a pior resposta possivel.
pub const DIFF_MAX_BYTES: usize = 32 * 1024 * 1024;
/// Codigo de saida do gitleaks quando acha algo; o 1 fica para erro dele.
const SAIDA_ACHOU: i32 = 3;
/// Onde o binario e o fluxo aparecem dentro do sandbox.
const BIN_NO_SANDBOX: &str = "/tmp/phxclaw-gitleaks/gitleaks";
const FLUXO_NO_SANDBOX: &str = "/tmp/phxclaw-varredura";

/// A configuracao da varredura: de onde vem o binario e se ela e obrigatoria.
#[derive(Debug, Clone, Default)]
pub struct Varredura {
    pub bin: Option<PathBuf>,
    pub sha256: Option<String>,
    pub exigir: bool,
    /// A configuracao nao se leu: a guarda fecha dizendo isto.
    pub erro: Option<String>,
}

/// O que a gravacao vai levar ao repositorio.
#[derive(Debug, Clone, PartialEq)]
pub enum Gravacao {
    Add(Vec<String>),
    Commit { todos: bool },
}

/// Um achado, ja sem o segredo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Achado {
    pub arquivo: String,
    pub linha: u64,
    pub regra: String,
}

/// O que a varredura diz para o resultado da ferramenta.
#[derive(Debug, Clone, PartialEq)]
pub enum Veredito {
    Limpo { linhas: usize },
    NaoFeita(String),
}

impl Veredito {
    pub fn json(&self) -> Value {
        match self {
            Veredito::Limpo { linhas } => {
                json!({"feita": true, "achados": 0, "linhas_varridas": linhas})
            }
            Veredito::NaoFeita(m) => json!({"feita": false, "motivo": m}),
        }
    }
}

impl Varredura {
    /// `git.segredos.*` pelo ponto unico da configuracao.
    pub fn da_configuracao() -> Self {
        let lida = (|| -> Result<Self, String> {
            let c = crate::config::configuracao()?;
            Ok(Self {
                bin: c.texto("git.segredos.gitleaks_bin").map(PathBuf::from),
                sha256: c.texto("git.segredos.gitleaks_sha256"),
                exigir: c
                    .valor("git.segredos.exigir")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                erro: None,
            })
        })();
        lida.unwrap_or_else(|e| Self {
            erro: Some(e),
            ..Self::default()
        })
    }

    /// Roda antes de `git add`/`git commit`. `Ok` com o veredito para o resultado;
    /// `Err(Denied)` quando a gravacao nao pode acontecer.
    pub async fn antes_de_gravar(
        &self,
        bwrap: &Path,
        workdir: &Path,
        repo: &str,
        g: &Gravacao,
        timeout: Duration,
    ) -> Result<Veredito, ToolError> {
        if let Some(e) = &self.erro {
            return Err(ToolError::Denied(format!(
                "varredura de segredos: configuracao ilegivel, gravacao recusada: {e}"
            )));
        }
        let Some(bin) = &self.bin else {
            let m = "varredura de segredos NAO feita: git.segredos.gitleaks_bin \
                     (PHXCLAW_GITLEAKS_BIN) nao configurado"
                .to_string();
            if self.exigir {
                return Err(ToolError::Denied(format!(
                    "{m}, e git.segredos.exigir esta ligado"
                )));
            }
            return Ok(Veredito::NaoFeita(m));
        };
        let sha = self.sha256.as_deref().ok_or_else(|| {
            ToolError::Denied(
                "varredura de segredos: binario configurado sem git.segredos.gitleaks_sha256 \
                 (PHXCLAW_GITLEAKS_SHA256); sem o hash conferido ele nao roda"
                    .into(),
            )
        })?;
        phxclaw_media_intelligence::verify_sha256(bin, sha).map_err(|e| {
            ToolError::Denied(format!(
                "varredura de segredos: binario {} recusado: {e}",
                bin.display()
            ))
        })?;
        let diff = diff_do_que_sera_gravado(bwrap, workdir, repo, g, timeout).await?;
        let fluxo = Fluxo::do_diff(&analisar_diff(&diff));
        if fluxo.origem.iter().all(Option::is_none) {
            return Ok(Veredito::Limpo { linhas: 0 });
        }
        let achados = rodar_gitleaks(bwrap, bin, &fluxo, timeout).await?;
        if achados.is_empty() {
            return Ok(Veredito::Limpo {
                linhas: fluxo.origem.iter().flatten().count(),
            });
        }
        Err(ToolError::Denied(texto_do_bloqueio(&achados)))
    }
}

pub fn texto_do_bloqueio(achados: &[Achado]) -> String {
    let lista: Vec<String> = achados
        .iter()
        .take(20)
        .map(|a| format!("{}:{} ({})", a.arquivo, a.linha, a.regra))
        .collect();
    let mais = achados.len().saturating_sub(lista.len());
    format!(
        "varredura de segredos (gitleaks) bloqueou a gravacao: {} achado(s): {}{}. Tire a \
         credencial do arquivo (variavel de ambiente ou cofre) antes de gravar.",
        achados.len(),
        lista.join(", "),
        if mais > 0 {
            format!(" e mais {mais}")
        } else {
            String::new()
        }
    )
}

/// O diff `--cached` de um indice temporario com a gravacao aplicada.
async fn diff_do_que_sera_gravado(
    bwrap: &Path,
    workdir: &Path,
    repo: &str,
    g: &Gravacao,
    timeout: Duration,
) -> Result<String, ToolError> {
    let preparar = match g {
        Gravacao::Add(caminhos) => {
            let c: String = caminhos.iter().map(|p| format!(" {}", aspas(p))).collect();
            format!("add --{c}")
        }
        Gravacao::Commit { todos: true } => "add -u".into(),
        Gravacao::Commit { todos: false } => String::new(),
    };
    let s = rodar_roteiro_git(
        bwrap,
        workdir,
        repo,
        |git| {
            // Indice ausente (repositorio sem nada preparado) e indice vazio: o arquivo
            // temporario tem de NAO existir, um arquivo de zero bytes o git recusa.
            let mut r = format!(
                "orig=$({git} rev-parse --path-format=absolute --git-path index) || exit 96\n\
                 idx=$(mktemp -u /tmp/phxclaw-indice.XXXXXX)\n\
                 if [ -f \"$orig\" ]; then cp \"$orig\" \"$idx\" || exit 96; fi\n\
                 export GIT_INDEX_FILE=\"$idx\"\n"
            );
            if !preparar.is_empty() {
                r.push_str(&format!("{git} {preparar} || exit $?\n"));
            }
            r.push_str(&format!(
                "exec {git} diff --cached --no-color --no-ext-diff --no-textconv -U0"
            ));
            r
        },
        timeout,
        DIFF_MAX_BYTES,
    )
    .await?;
    if s.exit_code != Some(0) {
        return Err(ToolError::Failed(format!(
            "varredura de segredos: o git nao montou o que seria gravado (saida {:?}): {}",
            s.exit_code,
            s.stderr.trim()
        )));
    }
    if s.truncated {
        return Err(ToolError::Denied(format!(
            "varredura de segredos: o que seria gravado passa de {} MiB; varrer so o comeco \
             nao prova nada, grave em partes menores",
            DIFF_MAX_BYTES / (1024 * 1024)
        )));
    }
    Ok(s.stdout)
}

/// As linhas adicionadas, uma por linha do fluxo, e de onde cada uma veio. Arquivos e
/// hunks se separam por uma linha vazia sem origem: uma chave privada que cruzasse dois
/// arquivos colados seria achado inventado.
#[derive(Debug, Default, PartialEq)]
pub struct Fluxo {
    pub texto: String,
    pub origem: Vec<Option<(String, u64)>>,
}

impl Fluxo {
    pub fn do_diff(arquivos: &[ArquivoDiff]) -> Self {
        let mut f = Fluxo::default();
        for a in arquivos.iter().filter(|a| !a.binario) {
            for h in &a.hunks {
                let mut n = h.novo_inicio;
                let mut abriu = false;
                for l in &h.linhas {
                    match l.chars().next() {
                        Some('+') => {
                            if !abriu {
                                f.separar();
                                abriu = true;
                            }
                            f.texto.push_str(&l[1..]);
                            f.texto.push('\n');
                            f.origem.push(Some((a.caminho.clone(), n)));
                            n += 1;
                        }
                        Some('-') | Some('\\') => {}
                        _ => n += 1,
                    }
                }
            }
        }
        f
    }

    fn separar(&mut self) {
        if !self.origem.is_empty() {
            self.texto.push('\n');
            self.origem.push(None);
        }
    }

    /// Linha do fluxo (1 em diante, como o gitleaks conta) para arquivo e linha.
    pub fn onde(&self, linha: u64) -> Option<&(String, u64)> {
        self.origem
            .get(usize::try_from(linha).ok()?.checked_sub(1)?)?
            .as_ref()
    }
}

/// Le o relatorio JSON do gitleaks e o traduz para arquivo e linha; nenhum campo com o
/// segredo (`Secret`, `Match`, `Line`) e lido.
pub fn achados_do_relatorio(relatorio: &str, fluxo: &Fluxo) -> Result<Vec<Achado>, String> {
    let v: Value = serde_json::from_str(relatorio.trim())
        .map_err(|e| format!("relatorio do gitleaks ilegivel: {e}"))?;
    let lista = v
        .as_array()
        .ok_or("relatorio do gitleaks nao e uma lista")?;
    let mut achados: Vec<Achado> = lista
        .iter()
        .map(|f| {
            let linha = f["StartLine"].as_u64().unwrap_or(0);
            let regra = f["RuleID"].as_str().unwrap_or("?").to_string();
            match fluxo.onde(linha) {
                Some((arquivo, n)) => Achado {
                    arquivo: arquivo.clone(),
                    linha: *n,
                    regra,
                },
                None => Achado {
                    arquivo: "?".into(),
                    linha,
                    regra,
                },
            }
        })
        .collect();
    achados.sort_by(|a, b| (&a.arquivo, a.linha, &a.regra).cmp(&(&b.arquivo, b.linha, &b.regra)));
    achados.dedup();
    Ok(achados)
}

/// Pasta temporaria do hospedeiro com o fluxo, montada so leitura no sandbox e apagada ao
/// sair (inclusive no erro).
struct PastaTemporaria(PathBuf);

impl Drop for PastaTemporaria {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn rodar_gitleaks(
    bwrap: &Path,
    bin: &Path,
    fluxo: &Fluxo,
    timeout: Duration,
) -> Result<Vec<Achado>, ToolError> {
    let falha = |e: std::io::Error| ToolError::Failed(format!("varredura de segredos: {e}"));
    let pasta = PastaTemporaria(std::env::temp_dir().join(format!(
        "phxclaw-varredura-{}",
        phxclaw_types::new_uuid_v7()
    )));
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&pasta.0)
            .map_err(falha)?;
    }
    std::fs::write(pasta.0.join("fluxo"), &fluxo.texto).map_err(falha)?;
    // Pasta vazia como cwd: o gitleaks procura `.gitleaks.toml` e `.gitleaksignore` em
    // `.`, e la nao ha nada -- valem as regras embutidas da versao travada.
    let script = format!(
        "mkdir -p /tmp/phxclaw-vazia && cd /tmp/phxclaw-vazia && exec {BIN_NO_SANDBOX} stdin \
         --no-banner --no-color --log-level error --redact=100 --ignore-gitleaks-allow \
         --report-format json --report-path - --exit-code {SAIDA_ACHOU} \
         < {FLUXO_NO_SANDBOX}/fluxo"
    );
    let cmd = WorkdirCommand {
        // O /work desta chamada e a propria pasta temporaria: o gitleaks nao precisa ver o
        // repositorio, so o fluxo.
        workdir: pasta.0.clone(),
        script,
        timeout,
        network: false,
        max_output_bytes: 8 * 1024 * 1024,
    };
    let extras = SandboxExtras {
        ro_binds: vec![
            (bin.to_path_buf(), BIN_NO_SANDBOX.into()),
            (pasta.0.clone(), FLUXO_NO_SANDBOX.into()),
        ],
        env: vec![],
    };
    let b = bwrap.to_path_buf();
    let s = tokio::task::spawn_blocking(move || run_in_workdir_com(&b, &cmd, &extras))
        .await
        .map_err(|e| ToolError::Failed(e.to_string()))?
        .map_err(|e| ToolError::Denied(format!("varredura de segredos: sandbox: {e}")))?;
    match s.exit_code {
        Some(0) => Ok(vec![]),
        Some(SAIDA_ACHOU) if !s.truncated => {
            let a = achados_do_relatorio(&s.stdout, fluxo)
                .map_err(|e| ToolError::Denied(format!("varredura de segredos: {e}")))?;
            if a.is_empty() {
                // Saiu dizendo que achou e o relatorio veio vazio: nao se sabe o que achou,
                // e na duvida a gravacao nao acontece.
                return Err(ToolError::Denied(
                    "varredura de segredos: o gitleaks acusou achado sem relatorio".into(),
                ));
            }
            Ok(a)
        }
        outro => Err(ToolError::Denied(format!(
            "varredura de segredos: o gitleaks nao terminou (saida {outro:?}{}): {}; a \
             gravacao fica recusada ate ele rodar",
            if s.truncated {
                ", relatorio cortado"
            } else {
                ""
            },
            s.stderr.trim().chars().take(400).collect::<String>()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/a.txt b/a.txt
new file mode 100644
--- /dev/null
+++ b/a.txt
@@ -0,0 +1,2 @@
+primeira
+segunda
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -3 +3 @@ fn x()
-velha
+nova
@@ -10,0 +11,2 @@
+++dentro
+fim
";

    #[test]
    fn fluxo_leva_so_linhas_novas_e_sabe_de_onde_cada_uma_veio() {
        let f = Fluxo::do_diff(&analisar_diff(DIFF));
        assert_eq!(
            f.texto, "primeira\nsegunda\n\nnova\n\n++dentro\nfim\n",
            "a linha removida nao entra; separador entre hunks"
        );
        assert_eq!(f.onde(1), Some(&("a.txt".into(), 1)));
        assert_eq!(f.onde(2), Some(&("a.txt".into(), 2)));
        assert_eq!(f.onde(3), None, "separador");
        assert_eq!(f.onde(4), Some(&("b.rs".into(), 3)));
        assert_eq!(f.onde(6), Some(&("b.rs".into(), 11)));
        assert_eq!(f.onde(7), Some(&("b.rs".into(), 12)));
        assert_eq!(f.onde(0), None);
    }

    #[test]
    fn relatorio_vira_arquivo_linha_e_regra_sem_ler_o_segredo() {
        let f = Fluxo::do_diff(&analisar_diff(DIFF));
        let r = r#"[{"RuleID":"aws-access-token","StartLine":4,"Secret":"AKIAXXXX","Match":"AKIAXXXX","Line":"k=AKIAXXXX"},
                    {"RuleID":"aws-access-token","StartLine":4,"Secret":"AKIAXXXX"}]"#;
        let a = achados_do_relatorio(r, &f).unwrap();
        assert_eq!(
            a,
            vec![Achado {
                arquivo: "b.rs".into(),
                linha: 3,
                regra: "aws-access-token".into()
            }]
        );
        let t = texto_do_bloqueio(&a);
        assert!(
            t.contains("b.rs:3 (aws-access-token)") && !t.contains("AKIA"),
            "{t}"
        );
        assert!(achados_do_relatorio("nao e json", &f).is_err());
    }

    #[tokio::test]
    async fn sem_binario_avisa_e_com_exigir_recusa() {
        let d = std::env::temp_dir();
        let g = Gravacao::Commit { todos: false };
        let p = Duration::from_secs(5);
        let v = Varredura::default()
            .antes_de_gravar(Path::new("/nao/ha"), &d, "", &g, p)
            .await
            .unwrap();
        assert!(matches!(&v, Veredito::NaoFeita(m) if m.contains("NAO feita")));
        assert_eq!(v.json()["feita"], false);
        let exigida = Varredura {
            exigir: true,
            ..Varredura::default()
        };
        assert!(matches!(
            exigida
                .antes_de_gravar(Path::new("/nao/ha"), &d, "", &g, p)
                .await,
            Err(ToolError::Denied(_))
        ));
        // Configurado sem hash, ou com hash errado: fecha, mesmo sem exigir.
        let sem_hash = Varredura {
            bin: Some("/bin/true".into()),
            ..Varredura::default()
        };
        assert!(matches!(
            sem_hash
                .antes_de_gravar(Path::new("/nao/ha"), &d, "", &g, p)
                .await,
            Err(ToolError::Denied(_))
        ));
        let hash_errado = Varredura {
            bin: Some("/bin/true".into()),
            sha256: Some("0".repeat(64)),
            ..Varredura::default()
        };
        assert!(matches!(
            hash_errado
                .antes_de_gravar(Path::new("/nao/ha"), &d, "", &g, p)
                .await,
            Err(ToolError::Denied(m)) if m.contains("recusado")
        ));
    }
}
