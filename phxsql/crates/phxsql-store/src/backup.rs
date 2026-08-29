//! Copia de seguranca dos dados, com manifesto conferivel.
//!
//! # O que faz uma copia ser confiavel
//!
//! Copiar arquivo e facil. O dificil e saber, seis meses depois, que a copia
//! presta. Por isso o backup daqui nao e so uma copia: e uma copia mais um
//! `backup.json` com o SHA-256 de cada arquivo, e um comando que le tudo de
//! volta e confere. Backup que ninguem consegue conferir e esperanca, nao
//! copia de seguranca.
//!
//! # Consistencia
//!
//! O motor ainda nao tem transacoes, entao "consistente" aqui quer dizer uma
//! coisa precisa: **nenhuma escrita acontece durante a copia**. Quem chama
//! segura a trava unica de dados do inicio ao fim, e como toda escrita passa
//! por essa mesma trava, nao ha registro pela metade no meio do caminho.
//!
//! E menos do que um snapshot de verdade -- uma escrita longa faz o backup
//! esperar, e o backup faz a escrita esperar. E o que da para prometer sem
//! mentir enquanto nao houver `commit`.
//!
//! # O que a copia leva
//!
//! Tudo que esta debaixo da raiz de dados: os cinco arquivos de cada tabela,
//! os volumes numerados e os diretorios de database e schema. O `config.json`
//! NAO vai junto -- ele tem o token e os hashes de senha, e backup de dado
//! costuma ir para lugar diferente de backup de segredo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::hash::{para_hex, sha256};
use phxsql_core::json::Json;

/// Nome do manifesto dentro do destino.
pub const MANIFESTO: &str = "backup.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arquivo {
    /// Caminho relativo a raiz dos dados, sempre com barra normal.
    pub caminho: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Default)]
pub struct Relatorio {
    pub arquivos: Vec<Arquivo>,
    pub bytes: u64,
    /// Tamanho do ZIP, quando a copia foi para arquivo unico.
    pub comprimido: u64,
    /// Preenchido so na conferencia: o que nao bate.
    pub divergencias: Vec<String>,
    /// O database UNICO desta copia; `None` quando ela e da raiz inteira.
    ///
    /// # Por que o manifesto precisa dizer isto
    ///
    /// Os caminhos de dentro nao contam: uma copia da raiz lista
    /// `Z/clientes.reg` e uma copia do banco Z lista `clientes.reg`, mas um
    /// banco que so tenha schemas lista `matriz/clientes.reg` -- que se
    /// escreve igualzinho a uma raiz com o database `matriz`. Adivinhar pelo
    /// formato do caminho acerta quase sempre, e "quase sempre" numa
    /// restauracao quer dizer restaurar um schema como se fosse um banco.
    pub database: Option<String>,
}

impl Relatorio {
    pub fn ok(&self) -> bool {
        self.divergencias.is_empty()
    }

    pub fn para_json(&self, quando: &str) -> Json {
        Json::objeto(vec![
            ("phxsql", Json::texto_de(env!("CARGO_PKG_VERSION"))),
            ("quando", Json::texto_de(quando)),
            ("arquivos", Json::de_u64(self.arquivos.len() as u64)),
            ("bytes", Json::de_u64(self.bytes)),
            // Os dois campos entraram JUNTO com a restauracao. Manifesto
            // gravado antes nao os tem e continua valendo: quem le cai na
            // deducao pelo formato dos caminhos. Backup ja gravado nao se
            // reescreve, e leitor antigo ignora campo que nao conhece.
            (
                "escopo",
                Json::texto_de(match &self.database {
                    Some(_) => "database",
                    None => "raiz",
                }),
            ),
            (
                "database",
                match &self.database {
                    Some(n) => Json::texto_de(n),
                    None => Json::Nulo,
                },
            ),
            (
                "conteudo",
                Json::Lista(
                    self.arquivos
                        .iter()
                        .map(|a| {
                            Json::objeto(vec![
                                ("caminho", Json::texto_de(&a.caminho)),
                                ("bytes", Json::de_u64(a.bytes)),
                                ("sha256", Json::texto_de(&a.sha256)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

/// Lista os arquivos debaixo de `raiz`, em ordem, com caminho relativo.
///
/// Ordenado de proposito: dois backups da mesma coisa tem de dar manifestos
/// comparaveis, e a ordem que o sistema de arquivos devolve nao e estavel.
pub(crate) fn listar(raiz: &Path) -> Result<Vec<PathBuf>> {
    let mut achados = Vec::new();
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(dir) = pilha.pop() {
        let leitura = std::fs::read_dir(&dir).map_err(|e| {
            PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}", dir.display()))
        })?;
        for entrada in leitura {
            let entrada = entrada?;
            let caminho = entrada.path();
            if caminho.is_dir() {
                pilha.push(caminho);
            } else {
                achados.push(caminho);
            }
        }
    }
    achados.sort();
    Ok(achados)
}

pub(crate) fn relativo(raiz: &Path, arquivo: &Path) -> String {
    arquivo
        .strip_prefix(raiz)
        .unwrap_or(arquivo)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Nome do arquivo: `BancoNome_Admin_Data_HoraMin.zip`.
///
/// Traz quem fez e quando no proprio nome porque e assim que se acha o
/// arquivo certo numa pasta com trezentos backups -- sem abrir nenhum.
pub fn nome_do_zip(banco: &str, admin: &str, quando_ms: i64) -> String {
    let dias = (quando_ms.div_euclid(86_400_000)) as i32;
    let (ano, mes, dia) = phxsql_core::datahora::civil_de_dias(dias);
    let minutos = quando_ms.rem_euclid(86_400_000) / 60_000;
    let limpo = |s: &str, padrao: &str| -> String {
        let t: String = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        if t.is_empty() {
            padrao.to_string()
        } else {
            t
        }
    };
    format!(
        "{}_{}_{ano:04}-{mes:02}-{dia:02}_{:02}{:02}.zip",
        limpo(banco, "dados"),
        limpo(admin, "sistema"),
        minutos / 60,
        minutos % 60
    )
}

/// Copia para um unico arquivo ZIP, com o manifesto dentro.
///
/// Um arquivo so viaja melhor do que uma arvore de diretorios: cabe em anexo,
/// sobe para nuvem inteiro, e o Windows(R), o Linux e o celular abrem sem
/// instalar nada. O manifesto vai dentro, entao a copia carrega a propria
/// conferencia.
///
/// `banco` vazio copia a raiz inteira. Quem chama segura a trava de dados.
pub fn executar_zip(
    raiz: &Path,
    pasta: &Path,
    banco: &str,
    admin: &str,
    quando_ms: i64,
) -> Result<(PathBuf, Relatorio)> {
    let origem = if banco.is_empty() {
        raiz.to_path_buf()
    } else {
        crate::catalogo::validar_nome("database", banco)?;
        raiz.join(banco)
    };
    if !origem.is_dir() {
        return Err(PhxError::NaoEncontrado(format!(
            "{} nao existe",
            origem.display()
        )));
    }
    std::fs::create_dir_all(pasta)?;
    let alvo = pasta.join(nome_do_zip(
        if banco.is_empty() { "dados" } else { banco },
        admin,
        quando_ms,
    ));

    let quando = phxsql_core::datahora::instante_iso(quando_ms);
    let mut zip = phxsql_core::zip::Zip::novo(quando_ms);
    let mut r = Relatorio {
        database: (!banco.is_empty()).then(|| banco.to_string()),
        ..Relatorio::default()
    };
    for arquivo in listar(&origem)? {
        let rel = relativo(&origem, &arquivo);
        let dados = std::fs::read(&arquivo)?;
        zip.acrescentar(&rel, &dados);
        r.bytes += dados.len() as u64;
        r.arquivos.push(Arquivo {
            caminho: rel,
            bytes: dados.len() as u64,
            sha256: para_hex(&sha256(&dados)),
        });
    }
    // O manifesto entra por ultimo, ja sabendo de todos os outros.
    zip.acrescentar(MANIFESTO, r.para_json(&quando).escrever().as_bytes());

    let bytes = zip.terminar();
    r.comprimido = bytes.len() as u64;
    std::fs::write(&alvo, &bytes)?;
    Ok((alvo, r))
}

/// Dos arquivos da pasta, quais apagar para sobrarem `manter`.
///
/// Separada da faxina de verdade para poder ser testada sem mexer em disco --
/// e porque a regra que importa aqui e "o que NAO apagar".
///
/// So entram arquivos com a cara dos nossos: `.zip` com pelo menos tres
/// sublinhados, que e o formato `Banco_Admin_Data_HoraMin.zip`. Backup nao
/// apaga arquivo que nao criou; alguem pode ter guardado outra coisa na pasta.
pub fn escolher_para_apagar(nomes: &[String], manter: usize) -> Vec<String> {
    if manter == 0 {
        return Vec::new();
    }
    let mut nossos: Vec<&String> = nomes
        .iter()
        .filter(|n| n.ends_with(".zip") && n.matches('_').count() >= 3)
        .collect();
    if nossos.len() <= manter {
        return Vec::new();
    }
    // O nome ja ordena por data: Banco_Admin_AAAA-MM-DD_HHMM.zip.
    nossos.sort();
    let sobra = nossos.len() - manter;
    nossos.into_iter().take(sobra).cloned().collect()
}

/// Copia a raiz de dados para o destino e escreve o manifesto.
///
/// Quem chama e responsavel por segurar a trava de dados. Ver a nota de
/// consistencia no topo do modulo.
pub fn executar(raiz: &Path, destino: &Path, quando: &str) -> Result<Relatorio> {
    if !raiz.is_dir() {
        return Err(PhxError::NaoEncontrado(format!(
            "a raiz de dados {} nao existe",
            raiz.display()
        )));
    }
    // Copiar para dentro da propria raiz copiaria a copia, sem parar.
    if destino.starts_with(raiz) {
        return Err(PhxError::Esquema(
            "o destino do backup nao pode ficar dentro da raiz de dados".into(),
        ));
    }
    std::fs::create_dir_all(destino)?;

    let mut r = Relatorio::default();
    for arquivo in listar(raiz)? {
        let rel = relativo(raiz, &arquivo);
        let dados = std::fs::read(&arquivo)?;
        let alvo = destino.join(&rel);
        if let Some(pai) = alvo.parent() {
            std::fs::create_dir_all(pai)?;
        }
        std::fs::write(&alvo, &dados)?;
        r.bytes += dados.len() as u64;
        r.arquivos.push(Arquivo {
            caminho: rel,
            bytes: dados.len() as u64,
            sha256: para_hex(&sha256(&dados)),
        });
    }

    std::fs::write(destino.join(MANIFESTO), r.para_json(quando).escrever())?;
    Ok(r)
}

/// Le o manifesto e confere cada arquivo do destino, byte a byte.
///
/// Acha as tres coisas que estragam um backup: arquivo que sumiu, arquivo que
/// mudou e arquivo que apareceu sem estar no manifesto.
pub fn conferir(destino: &Path) -> Result<Relatorio> {
    let texto = std::fs::read_to_string(destino.join(MANIFESTO)).map_err(|e| {
        PhxError::NaoEncontrado(format!("{} nao tem {MANIFESTO}: {e}", destino.display()))
    })?;
    let manifesto = Json::analisar(&texto)?;

    let mut esperados: BTreeMap<String, (u64, String)> = BTreeMap::new();
    if let Some(lista) = manifesto.campo("conteudo").and_then(Json::lista) {
        for a in lista {
            esperados.insert(
                a.texto_ou("caminho", "").to_string(),
                (
                    a.campo("bytes").and_then(Json::inteiro).unwrap_or(0) as u64,
                    a.texto_ou("sha256", "").to_string(),
                ),
            );
        }
    }

    let mut r = Relatorio::default();
    let mut vistos = BTreeMap::new();
    for arquivo in listar(destino)? {
        let rel = relativo(destino, &arquivo);
        if rel == MANIFESTO {
            continue;
        }
        let dados = std::fs::read(&arquivo)?;
        let sha = para_hex(&sha256(&dados));
        vistos.insert(rel.clone(), ());
        match esperados.get(&rel) {
            None => r
                .divergencias
                .push(format!("{rel}: existe no destino e nao esta no manifesto")),
            Some((bytes, esperado)) => {
                if *bytes != dados.len() as u64 {
                    r.divergencias.push(format!(
                        "{rel}: o manifesto diz {bytes} bytes, o arquivo tem {}",
                        dados.len()
                    ));
                } else if *esperado != sha {
                    r.divergencias
                        .push(format!("{rel}: o conteudo mudou (SHA-256 diferente)"));
                }
            }
        }
        r.bytes += dados.len() as u64;
        r.arquivos.push(Arquivo {
            caminho: rel,
            bytes: dados.len() as u64,
            sha256: sha,
        });
    }

    for caminho in esperados.keys() {
        if !vistos.contains_key(caminho) {
            r.divergencias
                .push(format!("{caminho}: esta no manifesto e sumiu do destino"));
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phxbkp-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn dados_de_exemplo(raiz: &Path) {
        std::fs::create_dir_all(raiz.join("Z/schemaX")).unwrap();
        std::fs::write(raiz.join("Z/cadastroClientes.reg"), b"registros aqui").unwrap();
        std::fs::write(raiz.join("Z/cadastroClientes.ndx"), b"indices aqui").unwrap();
        std::fs::write(raiz.join("Z/cadastroClientes_002.reg"), b"volume dois").unwrap();
        std::fs::write(raiz.join("Z/schemaX/pedidos.reg"), b"pedidos do schema").unwrap();
    }

    #[test]
    fn copia_tudo_e_confere() {
        let base = temp("copia");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let r = executar(&raiz, &destino, "2026-08-27 20:00:00").unwrap();
        assert_eq!(r.arquivos.len(), 4);
        assert!(r.bytes > 0);
        assert!(destino.join(MANIFESTO).is_file());
        // A hierarquia veio junto.
        assert!(destino.join("Z/schemaX/pedidos.reg").is_file());
        assert!(destino.join("Z/cadastroClientes_002.reg").is_file());

        let c = conferir(&destino).unwrap();
        assert!(
            c.ok(),
            "backup recem-feito tem de conferir: {:?}",
            c.divergencias
        );
        assert_eq!(c.arquivos.len(), 4);
    }

    #[test]
    fn acha_arquivo_alterado() {
        let base = temp("alterado");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        executar(&raiz, &destino, "agora").unwrap();

        // Mesmo tamanho, conteudo diferente: so o SHA pega.
        std::fs::write(destino.join("Z/cadastroClientes.reg"), b"registros AQUI").unwrap();
        let c = conferir(&destino).unwrap();
        assert!(!c.ok());
        assert!(
            c.divergencias[0].contains("SHA-256"),
            "{:?}",
            c.divergencias
        );
    }

    #[test]
    fn acha_arquivo_sumido_e_arquivo_a_mais() {
        let base = temp("sumido");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        executar(&raiz, &destino, "agora").unwrap();

        std::fs::remove_file(destino.join("Z/cadastroClientes.ndx")).unwrap();
        std::fs::write(destino.join("Z/intruso.reg"), b"nao estava no manifesto").unwrap();

        let c = conferir(&destino).unwrap();
        assert_eq!(c.divergencias.len(), 2, "{:?}", c.divergencias);
        assert!(c.divergencias.iter().any(|d| d.contains("sumiu")));
        assert!(c
            .divergencias
            .iter()
            .any(|d| d.contains("nao esta no manifesto")));
    }

    #[test]
    fn acha_tamanho_diferente() {
        let base = temp("tamanho");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        executar(&raiz, &destino, "agora").unwrap();
        std::fs::write(destino.join("Z/cadastroClientes.reg"), b"curto").unwrap();
        let c = conferir(&destino).unwrap();
        assert!(c.divergencias[0].contains("bytes"), "{:?}", c.divergencias);
    }

    #[test]
    fn nao_copia_para_dentro_de_si_mesmo() {
        let base = temp("dentro");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        // Copiar a raiz para dentro dela copiaria a copia, sem parar.
        assert!(executar(&raiz, &raiz.join("copia"), "agora").is_err());
        assert!(executar(&raiz, &raiz, "agora").is_err());
    }

    #[test]
    fn destino_sem_manifesto_nao_e_backup() {
        let base = temp("semmanifesto");
        std::fs::create_dir_all(base.join("qualquer")).unwrap();
        assert!(conferir(&base.join("qualquer")).is_err());
    }

    #[test]
    fn o_manifesto_e_estavel() {
        // Dois backups dos mesmos dados listam na mesma ordem. Sem isso,
        // comparar dois manifestos seria comparar ruido.
        let base = temp("estavel");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let a = executar(&raiz, &base.join("c1"), "agora").unwrap();
        let b = executar(&raiz, &base.join("c2"), "depois").unwrap();
        assert_eq!(a.arquivos, b.arquivos);
    }
    #[test]
    fn o_nome_do_zip_traz_banco_admin_data_e_hora() {
        // 2026-08-27 20:43 UTC
        let ms = (phxsql_core::datahora::dias_de_civil(2026, 8, 27) as i64) * 86_400_000
            + 20 * 3_600_000
            + 43 * 60_000;
        assert_eq!(
            nome_do_zip("Comercial", "adriano", ms),
            "Comercial_adriano_2026-08-27_2043.zip"
        );
        // Nome com o que nao cabe em arquivo sai limpo, nao quebrado.
        assert_eq!(
            nome_do_zip("Com/ercial", "ana maria", ms),
            "Comercial_anamaria_2026-08-27_2043.zip"
        );
        // Vazio vira o padrao, nunca um nome comecando com sublinhado.
        assert_eq!(nome_do_zip("", "", ms), "dados_sistema_2026-08-27_2043.zip");
    }

    #[test]
    fn o_zip_leva_tudo_e_o_manifesto_dentro() {
        let base = temp("zip");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let (alvo, r) = executar_zip(
            &raiz,
            &base.join("copias"),
            "",
            "adriano",
            1_787_000_000_000,
        )
        .unwrap();
        assert!(alvo.is_file());
        assert!(alvo.to_string_lossy().ends_with(".zip"));
        assert_eq!(r.arquivos.len(), 4);
        assert!(r.comprimido > 0);

        let bytes = std::fs::read(&alvo).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        let texto = String::from_utf8_lossy(&bytes);
        assert!(texto.contains("backup.json"), "o manifesto vai dentro");
        assert!(
            texto.contains("Z/schemaX/pedidos.reg"),
            "a hierarquia vai junto"
        );
    }

    #[test]
    fn da_para_copiar_um_banco_so() {
        let base = temp("zipbanco");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        std::fs::create_dir_all(raiz.join("Financeiro")).unwrap();
        std::fs::write(raiz.join("Financeiro/contas.reg"), b"nao deve entrar").unwrap();

        let (alvo, r) =
            executar_zip(&raiz, &base.join("c"), "Z", "ana", 1_787_000_000_000).unwrap();
        assert!(alvo
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("Z_ana_"));
        assert_eq!(r.arquivos.len(), 4, "so os quatro do Z");
        let bytes = std::fs::read(&alvo).unwrap();
        let texto = String::from_utf8_lossy(&bytes);
        assert!(!texto.contains("contas.reg"), "o outro banco ficou de fora");
    }

    #[test]
    fn nome_de_banco_hostil_nao_vira_caminho() {
        let base = temp("ziphostil");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        for mau in ["../..", "/etc", "a/b"] {
            assert!(
                executar_zip(&raiz, &base.join("c"), mau, "x", 0).is_err(),
                "{mau:?} passou"
            );
        }
    }

    #[test]
    fn a_retencao_guarda_os_mais_novos_e_nao_toca_no_alheio() {
        let nomes: Vec<String> = [
            "dados_noturno_2026-08-20_0300.zip",
            "dados_noturno_2026-08-24_0300.zip",
            "dados_noturno_2026-08-21_0300.zip",
            "dados_noturno_2026-08-27_2116.zip",
            "dados_noturno_2026-08-22_0300.zip",
            // Nao sao nossos: ficam, aconteca o que acontecer.
            "relatorio-do-contador.zip",
            "backup.json",
            "notas_fiscais.zip",
            "dados_noturno.zip",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let apagar = escolher_para_apagar(&nomes, 3);
        assert_eq!(
            apagar,
            vec![
                "dados_noturno_2026-08-20_0300.zip".to_string(),
                "dados_noturno_2026-08-21_0300.zip".to_string(),
            ],
            "apaga os dois mais velhos e so eles"
        );
        for alheio in [
            "relatorio-do-contador.zip",
            "notas_fiscais.zip",
            "backup.json",
        ] {
            assert!(!apagar.iter().any(|a| a == alheio), "{alheio} nao e nosso");
        }
    }

    #[test]
    fn manter_zero_nao_apaga_nada_e_poucos_tambem_nao() {
        let nomes: Vec<String> = (20..25)
            .map(|d| format!("dados_x_2026-08-{d}_0300.zip"))
            .collect();
        assert!(
            escolher_para_apagar(&nomes, 0).is_empty(),
            "zero = guarda tudo"
        );
        assert!(
            escolher_para_apagar(&nomes, 5).is_empty(),
            "cabe todo mundo"
        );
        assert!(escolher_para_apagar(&nomes, 99).is_empty());
        assert_eq!(escolher_para_apagar(&nomes, 1).len(), 4);
    }
}
