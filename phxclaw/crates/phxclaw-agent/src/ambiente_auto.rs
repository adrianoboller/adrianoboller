//! Auto-instalacao do ambiente sob permissao (parte C do v070-nativo).
//!
//! "Guarda nova entra pedida, nao imposta": a chave `ambiente.auto_instalar` nasce `false`,
//! entao quem ja usa o agente nao ganha instalacao nenhuma de um dia para o outro -- o
//! comportamento velho fica. So liga quem pede, e so dispara sozinho quando TAMBEM tem a
//! permissao concedida; nos demais casos PROPOE, nunca instala.
//!
//! A decisao mora numa funcao pura (`decidir`) para a regra ser provada nos quatro cantos
//! sem tocar disco nem processo; o arranque (`verificar_no_arranque`) e so a casca que le a
//! configuracao, roda o `--health` do instalador assinado e aplica a decisao.

use phxclaw_plugin_registry::{PluginRegistry, TrustStore};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Capacidade que autoriza DISPARAR o instalador. E a capacidade primaria do plugin
/// `com.phxclaw.tool.ambiente` (a que o portao do motor confere a cada chamada), e por ela
/// o par `system.admin` + `net.download` do manifesto chega ao agente: conceder
/// `environment.install` ao agente e dizer "pode rodar o instalador assinado". Sem ela, o
/// arranque so PROPOE.
pub const CAP_INSTALAR: &str = "environment.install";

/// O nome do plugin do instalador no registro.
pub const NOME_PLUGIN: &str = "com.phxclaw.tool.ambiente";

/// O que o arranque decide fazer.
#[derive(Debug, PartialEq, Eq)]
pub enum Decisao {
    /// Ambiente incompleto, auto ligado e permissao concedida: dispara o instalador.
    Instalar,
    /// Ambiente incompleto, mas sem auto ou sem permissao: so propoe (texto ao operador).
    Propor(String),
    /// Nada a fazer (ambiente completo).
    Nada,
}

/// O retrato do `--health` do instalador.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Saude {
    /// O `--health` saiu 0 (tudo presente)?
    pub ok: bool,
    /// Os componentes que faltam ou divergem, para o texto da proposta.
    pub faltam: Vec<String>,
}

/// O operador concedeu ao agente a capacidade de instalar?
pub fn permissao_concedida(caps: &[String]) -> bool {
    caps.iter().any(|c| c == CAP_INSTALAR)
}

/// A regra, pura: ambiente completo nunca age; incompleto so instala com auto E permissao,
/// senao propoe dizendo por que nao instalou. Com `auto_instalar=false` NUNCA devolve
/// `Instalar` -- e o comportamento velho, provado no teste.
pub fn decidir(auto_instalar: bool, permissao: bool, saude: &Saude) -> Decisao {
    if saude.ok {
        return Decisao::Nada;
    }
    if auto_instalar && permissao {
        return Decisao::Instalar;
    }
    let falta = if saude.faltam.is_empty() {
        "componentes do ambiente".to_string()
    } else {
        saude.faltam.join(", ")
    };
    let porque = if !auto_instalar {
        "ambiente.auto_instalar=false (ligue para instalar sozinho)"
    } else {
        "a permissao environment.install nao foi concedida ao agente"
    };
    Decisao::Propor(format!(
        "ambiente de desenvolvimento incompleto (falta: {falta}); {porque}. \
         Rode o instalador assinado: ambiente-instalador tudo"
    ))
}

/// Le o `--health` do instalador: exit 0 => ok; as linhas com AUSENTE ou "falta" viram a
/// lista do que falta. Fora do caminho feliz (instalador ausente, nao executavel), devolve
/// `None` -- o arranque trata como "nao da para medir", e nao propoe nada no escuro.
pub fn saude_do_instalador(instalador: &Path) -> Option<Saude> {
    let saida = Command::new(instalador).arg("--health").output().ok()?;
    let texto = String::from_utf8_lossy(&saida.stdout);
    let faltam: Vec<String> = texto
        .lines()
        .filter(|l| l.contains("AUSENTE") || l.contains("falta"))
        .map(|l| l.trim().to_string())
        .collect();
    Some(Saude {
        ok: saida.status.success(),
        faltam,
    })
}

/// Verifica o instalador pelo MESMO motor da via do modelo -- o `PluginRegistry`: assinatura
/// ed25519 contra o trust store e `sha256` do artefato. Devolve o caminho do artefato so se o
/// plugin esta VALIDADO (fora da quarentena). Nao e um segundo verificador: e o registro que
/// os plugins do modelo ja usam (`plugins.rs`). Consequencia desejada: enquanto o manifesto
/// nao for re-assinado pelo dono, o auto-instalar NAO roda como root -- a quarentena manda.
pub fn verificar_instalador(
    raiz: &Path,
    manifestos: &Path,
    signers: &Path,
    nome: &str,
) -> Result<PathBuf, String> {
    let trust = TrustStore::from_json(
        &std::fs::read_to_string(signers).map_err(|e| format!("{}: {e}", signers.display()))?,
    )
    .map_err(|e| e.to_string())?;
    let c: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(raiz.join("config/constitution.json"))
            .map_err(|e| format!("constituicao: {e}"))?,
    )
    .map_err(|e| e.to_string())?;
    let api = c["plugin_api_version"]
        .as_str()
        .ok_or("constituicao sem plugin_api_version")?;
    let raiz_c = std::fs::canonicalize(raiz).map_err(|e| format!("raiz dos plugins: {e}"))?;
    let mut reg = PluginRegistry::new(api, &raiz_c, trust);
    reg.discover_tree(manifestos).map_err(|e| e.to_string())?;
    // So entra aqui o manifesto VALIDADO; o que falhou assinatura ou digest esta na
    // quarentena, com o motivo, e nao aparece em `manifests()`.
    let Some(m) = reg.manifests().find(|m| m.name == nome).cloned() else {
        let motivo = reg
            .quarantine()
            .iter()
            .find(|q| q.plugin_name.as_deref() == Some(nome))
            .map(|q| q.reason.clone())
            .unwrap_or_else(|| "nao encontrado na arvore de plugins".into());
        return Err(format!("instalador {nome} nao validado: {motivo}"));
    };
    // TOCTOU: re-le e re-confere o digest antes de entregar o caminho -- a mesma protecao que
    // a via do modelo faz imediatamente antes de executar (plugins.rs).
    let artefato = raiz_c.join(&m.integrity.artifact);
    let bytes = std::fs::read(&artefato).map_err(|e| format!("artefato: {e}"))?;
    let visto: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if visto != m.integrity.digest.to_ascii_lowercase() {
        return Err(format!("digest do artefato diverge do assinado ({visto})"));
    }
    Ok(artefato)
}

/// A raiz, os manifestos e o trust store dos plugins (mesma conta de `plugins::do_ambiente`).
fn caminhos_dos_plugins() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let raiz = crate::config::caminho_de("plugins.raiz")?;
    let manifestos =
        crate::config::caminho_de("plugins.dir").unwrap_or_else(|| raiz.join("plugins"));
    let signers = crate::config::caminho_de("plugins.assinantes")
        .unwrap_or_else(|| raiz.join("config/trust/plugin-signers.json"));
    Some((raiz, manifestos, signers))
}

/// O arranque (`phxclaw servir`): verifica o instalador pelo registro (assinatura + digest)
/// ANTES de qualquer execucao, mede o `--health` e aplica a decisao. Sem a raiz dos plugins,
/// ou com o instalador em quarentena, nao executa NADA -- nem o `--health`, que tambem roda o
/// artefato. Instalador nao confiavel que o `servir` roda como root era o furo.
pub fn verificar_no_arranque(caps: &[String]) {
    let auto = crate::config::booleano_de("ambiente.auto_instalar").unwrap_or(false);
    let Some((raiz, manifestos, signers)) = caminhos_dos_plugins() else {
        return;
    };
    let instalador = match verificar_instalador(&raiz, &manifestos, &signers, NOME_PLUGIN) {
        Ok(p) => p,
        Err(e) => {
            // Nao confiavel: nao se executa como root nem para medir. So avisa quem pediu.
            if auto {
                eprintln!(
                    "aviso: ambiente: auto_instalar ligado, mas o instalador nao passou na \
                     verificacao do registro ({e}); nao vou executa-lo"
                );
            }
            return;
        }
    };
    let Some(saude) = saude_do_instalador(&instalador) else {
        eprintln!("aviso: ambiente: --health do instalador nao pode ser medido");
        return;
    };
    match decidir(auto, permissao_concedida(caps), &saude) {
        Decisao::Nada => {}
        Decisao::Propor(msg) => eprintln!("aviso: {msg}"),
        Decisao::Instalar => {
            eprintln!("ambiente: auto_instalar ligado e permissao concedida; instalando...");
            match Command::new(&instalador).arg("tudo").status() {
                Ok(s) if s.success() => eprintln!("ambiente: instalacao concluida"),
                Ok(s) => eprintln!("aviso: ambiente: instalador saiu com {s}"),
                Err(e) => eprintln!("aviso: ambiente: instalador nao rodou: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn incompleto() -> Saude {
        Saude {
            ok: false,
            faltam: vec!["node     AUSENTE".into(), "ollama   AUSENTE".into()],
        }
    }

    #[test]
    fn auto_desligado_nunca_instala_sozinho() {
        // Comportamento VELHO: guarda nova entra pedida. Mesmo com permissao e ambiente
        // incompleto, auto=false so propoe. RED medido: trocar o guard por `if permissao`
        // (ignorando `auto_instalar`) faz este assert falhar -- Instalar onde o velho exige
        // Propor.
        let d = decidir(false, true, &incompleto());
        assert!(matches!(d, Decisao::Propor(_)), "{d:?}");
    }

    #[test]
    fn auto_ligado_com_permissao_e_falta_instala() {
        assert_eq!(decidir(true, true, &incompleto()), Decisao::Instalar);
    }

    #[test]
    fn auto_ligado_sem_permissao_so_propoe() {
        // RED medido: remover `&& permissao` da condicao de Instalar faz este assert falhar
        // -- instalaria sem a permissao concedida.
        let d = decidir(true, false, &incompleto());
        match d {
            Decisao::Propor(m) => assert!(m.contains("environment.install"), "{m}"),
            outra => panic!("{outra:?}"),
        }
    }

    #[test]
    fn ambiente_completo_nao_faz_nada() {
        let s = Saude {
            ok: true,
            faltam: vec![],
        };
        assert_eq!(decidir(true, true, &s), Decisao::Nada);
        assert_eq!(decidir(false, false, &s), Decisao::Nada);
    }

    #[test]
    fn permissao_le_a_capacidade_certa() {
        assert!(permissao_concedida(&["environment.install".into()]));
        assert!(!permissao_concedida(&[
            "shell.exec".into(),
            "fs.write".into()
        ]));
    }

    #[test]
    fn a_proposta_nomeia_o_que_falta() {
        let d = decidir(false, false, &incompleto());
        match d {
            Decisao::Propor(m) => {
                assert!(m.contains("node"), "{m}");
                assert!(m.contains("ollama"), "{m}");
            }
            outra => panic!("{outra:?}"),
        }
    }

    /// Monta uma arvore de plugins assinada por uma chave de teste e devolve (raiz,
    /// manifestos, signers, manifesto_path, json_assinado).
    fn arvore_assinada() -> (PathBuf, PathBuf, PathBuf, PathBuf, String) {
        use base64::{Engine, engine::general_purpose::STANDARD as B64};
        use ed25519_dalek::SigningKey;
        use phxclaw_plugin_registry::assinatura::reassinar;

        let raiz =
            std::env::temp_dir().join(format!("phxclaw-arranque-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(raiz.join("manifestos")).unwrap();
        std::fs::create_dir_all(raiz.join("config/trust")).unwrap();
        std::fs::create_dir_all(raiz.join("plugins/builtin/artifacts")).unwrap();
        let artefato_rel = "plugins/builtin/artifacts/ambiente-instalador";
        std::fs::write(raiz.join(artefato_rel), b"#!/bin/sh\nexit 0\n").unwrap();

        let chave = SigningKey::from_bytes(&[7u8; 32]);
        let publica = B64.encode(chave.verifying_key().as_bytes());
        let signer_id = "fixture-arranque";
        let trust = serde_json::json!({
            "version": "1.0.0",
            "signers": [{
                "id": signer_id,
                "algorithm": "ed25519",
                "public_key_base64": publica,
                "status": "active",
                "allowed_name_prefixes": ["com.phxclaw."],
                "signature_format": 2
            }]
        });
        let signers = raiz.join("config/trust/plugin-signers.json");
        std::fs::write(&signers, serde_json::to_string(&trust).unwrap()).unwrap();
        std::fs::write(
            raiz.join("config/constitution.json"),
            br#"{"plugin_api_version":"0.5.0"}"#,
        )
        .unwrap();

        // Molde: o proprio manifesto do instalador (todos os campos validos); troca-se o
        // signatario e o artefato, e re-assina-se com a chave de teste.
        let mut mv: serde_json::Value = serde_json::from_str(include_str!(
            "../../../plugins/builtin/manifests/ambiente.tool.plugin.json"
        ))
        .unwrap();
        mv["integrity"]["signer"] = serde_json::json!(signer_id);
        mv["integrity"]["artifact"] = serde_json::json!(artefato_rel);
        let assinado = reassinar(&mv.to_string(), &raiz, &chave, 2).unwrap();
        let manifesto = raiz.join("manifestos/ambiente.tool.plugin.json");
        std::fs::write(&manifesto, &assinado).unwrap();
        let manifestos = raiz.join("manifestos");
        (raiz, manifestos, signers, manifesto, assinado)
    }

    #[test]
    fn arranque_valida_instalador_assinado() {
        let (raiz, manifestos, signers, _m, _j) = arvore_assinada();
        let r = verificar_instalador(&raiz, &manifestos, &signers, NOME_PLUGIN);
        assert!(r.is_ok(), "instalador assinado deveria validar: {r:?}");
        std::fs::remove_dir_all(&raiz).ok();
    }

    #[test]
    fn arranque_nao_roda_plugin_em_quarentena() {
        let (raiz, manifestos, signers, manifesto, assinado) = arvore_assinada();
        // Assinatura adulterada, artefato intacto: o registro quarentena o plugin.
        // RED medido: a versao antiga (`instalador_configurado`, so `p.exists()`) devolvia o
        // caminho aqui e o `servir` rodaria o artefato como root ignorando a quarentena.
        let mut qv: serde_json::Value = serde_json::from_str(&assinado).unwrap();
        let sig = qv["integrity"]["signature"].as_str().unwrap();
        // Vira o primeiro caractere (A<->B): quebra a assinatura sem mudar o tamanho.
        let trocada: String = sig
            .chars()
            .enumerate()
            .map(|(i, c)| match (i, c) {
                (0, 'A') => 'B',
                (0, _) => 'A',
                _ => c,
            })
            .collect();
        qv["integrity"]["signature"] = serde_json::json!(trocada);
        std::fs::write(&manifesto, qv.to_string()).unwrap();

        let r = verificar_instalador(&raiz, &manifestos, &signers, NOME_PLUGIN);
        assert!(
            r.is_err(),
            "assinatura adulterada tinha de cair na quarentena, nao {r:?}"
        );
        std::fs::remove_dir_all(&raiz).ok();
    }
}
