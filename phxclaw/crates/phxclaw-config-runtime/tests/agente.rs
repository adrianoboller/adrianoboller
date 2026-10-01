//! O `config.json` do agente: catalogo, precedencia, recusas, gravacao com revisao, os
//! artefatos gerados e o levantamento do fonte (catraca das leituras soltas).

use phxclaw_config_runtime::agente::carga::{
    self, credencial_aparente, validar_documento, Alvo, Origem, Recusa,
};
use phxclaw_config_runtime::agente::{catalogo, gerar, por_chave, por_variavel, Natureza};
use phxclaw_config_runtime::ConfigStore;
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-cfg-{nome}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn grava(p: &Path, v: Value) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

fn amb(pares: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let m: HashMap<String, String> = pares
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |k: &str| m.get(k).cloned()
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn catalogo_sem_repeticao_e_com_padroes_do_proprio_tipo() {
    let c = catalogo();
    assert!(c.len() > 200, "catalogo encolheu: {}", c.len());
    let chaves: BTreeSet<_> = c.iter().map(|x| &x.chave).collect();
    let vars: BTreeSet<_> = c.iter().map(|x| &x.variavel).collect();
    assert_eq!(chaves.len(), c.len(), "chave repetida");
    assert_eq!(vars.len(), c.len(), "variavel repetida");
    for x in c {
        assert!(x.variavel.starts_with("PHXCLAW_"), "{}", x.variavel);
        assert!(x.chave.contains('.'), "{} sem secao", x.chave);
        assert!(
            x.chave
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || "._".contains(ch)),
            "{}",
            x.chave
        );
        // Chave folha nao pode ser tambem secao de outra.
        let pref = format!("{}.", x.chave);
        assert!(
            !c.iter().any(|y| y.chave.starts_with(&pref)),
            "{} e folha e secao",
            x.chave
        );
        if let Some(p) = x.padrao {
            carga::do_texto(x, p).unwrap_or_else(|e| panic!("padrao de {}: {e}", x.chave));
        }
        if x.segredo() {
            assert!(x.padrao.is_none(), "segredo com padrao: {}", x.chave);
        }
    }
    assert_eq!(
        por_variavel("PHXCLAW_WHISPER_BIN").unwrap().chave,
        "voz.whisper.bin"
    );
    assert_eq!(
        por_chave("canais.telegram.chats").unwrap().variavel,
        "PHXCLAW_TELEGRAM_CHATS"
    );
    assert_eq!(
        por_chave("canais.email.tls").unwrap().variavel,
        "PHXCLAW_EMAIL_CANAL_TLS"
    );
}

#[test]
fn precedencia_ambiente_projeto_pasta_padrao_e_cada_valor_sabe_a_origem() {
    let d = tmp("prec");
    let pasta = d.join("agente/config.json");
    let projeto = d.join("proj/.phxclaw/config.json");
    grava(
        &pasta,
        json!({"revisao": 4, "modelo": {"padrao": "da-pasta", "visao": "visao-da-pasta"},
               "agente": {"estilo": "estilo-da-pasta"}, "api": {"tarefas_por_minuto": 7}}),
    );
    grava(
        &projeto,
        json!({"revisao": 2, "modelo": {"padrao": "do-projeto", "visao": "visao-do-projeto"}}),
    );
    let a = amb(&[
        ("PHXCLAW_MODELO", "do-ambiente"),
        ("PHXCLAW_XAI_API_KEY", "x"),
    ]);
    let c = carga::carregar(&a, &pasta, Some(&projeto)).unwrap();
    assert_eq!(c.texto("modelo.padrao").unwrap(), "do-ambiente");
    assert_eq!(c.origem("modelo.padrao"), Origem::Ambiente);
    assert_eq!(c.texto("modelo.visao").unwrap(), "visao-do-projeto");
    assert_eq!(c.origem("modelo.visao"), Origem::Projeto);
    assert_eq!(c.texto("agente.estilo").unwrap(), "estilo-da-pasta");
    assert_eq!(c.origem("agente.estilo"), Origem::Pasta);
    assert_eq!(c.inteiro("api.tarefas_por_minuto"), Some(7));
    assert_eq!(c.texto("api.host").unwrap(), "127.0.0.1");
    assert_eq!(c.origem("api.host"), Origem::Padrao);
    assert_eq!(c.valor("voz.whisper.bin"), None);
    assert_eq!(c.origem("voz.whisper.bin"), Origem::Ausente);
    // Segredo: so a presenca, nunca o valor.
    assert_eq!(c.origem("xai.chave"), Origem::Ambiente);
    assert_eq!(c.valor("xai.chave"), None);
    assert_eq!(c.revisao(), 6);
    // Sem projeto (nao confiado), o projeto nao conta.
    let c = carga::carregar(&a, &pasta, None).unwrap();
    assert_eq!(c.texto("modelo.visao").unwrap(), "visao-da-pasta");
    assert_eq!(c.origem("modelo.visao"), Origem::Pasta);
    // O ambiente e tipado tambem: lista separada, booleano por extenso.
    let a = amb(&[
        ("PHXCLAW_TTS_DIRS", "/a: /b"),
        ("PHXCLAW_LSP", "0"),
        ("PHXCLAW_EMAIL_CANAL_TLS", "nao"),
    ]);
    let c = carga::carregar(&a, &pasta, None).unwrap();
    assert_eq!(c.lista("voz.tts.pastas").unwrap(), vec!["/a", "/b"]);
    assert_eq!(c.booleano("agente.lsp"), Some(false));
    assert_eq!(c.booleano("canais.email.tls"), Some(false));
    // Valor de ambiente com tipo errado e erro, com a variavel.
    let a = amb(&[("PHXCLAW_HEARTBEAT_MIN", "meia hora")]);
    let e = carga::carregar(&a, &pasta, None).unwrap_err();
    assert!(
        e[0].chave == "agente.heartbeat_min" && e[0].motivo.contains("PHXCLAW_HEARTBEAT_MIN"),
        "{e:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn segredo_nunca_entra_no_arquivo_e_a_recusa_diz_o_comando() {
    let e = validar_documento(&json!({"forja": {"github": {"token": "qualquer"}}})).unwrap_err();
    assert_eq!(e[0].chave, "forja.github.token");
    assert!(e[0].motivo.contains("phxclaw forja token github"), "{e:?}");
    // Nem como null: o arquivo nao ensina a por segredo nele.
    let e = validar_documento(&json!({"canais": {"discord": {"token": null}}})).unwrap_err();
    assert_eq!(e[0].chave, "canais.discord.token");
    assert!(e[0].motivo.contains("phxclaw canal discord"), "{e:?}");
    // Valor com cara de credencial numa chave que nao e segredo.
    for (chave, v) in [
        ("modelo.padrao", json!("ghp_abcdefghijklmnop")),
        ("agente.estilo", json!("glpat-xyz")),
        ("api.webhook_origens", json!(["https://ok", "xoxb-123"])),
        ("busca.searxng_url", json!("AIzaSyQualquer")),
        ("postgres.url", json!("postgresql://eu:senha@db/x")),
    ] {
        let mut doc = Map::new();
        let (s, f) = chave.split_once('.').unwrap();
        doc.insert(s.into(), json!({ f: v }));
        let e = validar_documento(&Value::Object(doc)).unwrap_err();
        assert_eq!(e[0].chave, chave, "{e:?}");
        assert!(
            e[0].motivo.contains("credencial") || e[0].motivo.contains("senha"),
            "{e:?}"
        );
    }
    assert!(credencial_aparente("postgresql://eu@db/x").is_none());
    assert!(credencial_aparente("https://api.x.ai/v1").is_none());
    // Chave so de ambiente tambem nao mora no arquivo.
    let e = validar_documento(&json!({"agente": {"pasta": "/x"}})).unwrap_err();
    assert!(e[0].motivo.contains("PHXCLAW_HOME"), "{e:?}");
}

#[test]
fn chave_desconhecida_e_erro_com_sugestao_e_todos_os_erros_vem_juntos() {
    let e = validar_documento(&json!({
        "voz": {"whisper": {"binn": "/x"}},
        "modleo": {"padrao": "x"},
        "nada": 1,
        "modelo": {"padrao": 3}
    }))
    .unwrap_err();
    let por: HashMap<_, _> = e
        .iter()
        .map(|x| (x.chave.as_str(), x.motivo.as_str()))
        .collect();
    assert!(
        por["voz.whisper.binn"].contains("`voz.whisper.bin`"),
        "{e:?}"
    );
    assert!(por["modleo"].contains("desconhecida"), "{e:?}");
    assert!(por["nada"].contains("desconhecida"), "{e:?}");
    assert!(por["modelo.padrao"].contains("esperado texto"), "{e:?}");
    assert_eq!(e.len(), 4, "{e:?}");
}

#[test]
fn tipo_errado_diz_a_chave_e_o_tipo_esperado() {
    for (doc, chave, esperado) in [
        (
            json!({"api": {"tarefas_por_minuto": "dez"}}),
            "api.tarefas_por_minuto",
            "inteiro",
        ),
        (json!({"agente": {"lsp": "sim"}}), "agente.lsp", "booleano"),
        (
            json!({"voz": {"tts": {"pastas": "/a:/b"}}}),
            "voz.tts.pastas",
            "lista",
        ),
        (
            json!({"voz": {"tts": {"provedor": "piper"}}}),
            "voz.tts.provedor",
            "comando|elevenlabs",
        ),
        (
            json!({"elevenlabs": {"estabilidade": "alta"}}),
            "elevenlabs.estabilidade",
            "real",
        ),
        (
            json!({"modelo": {"padrao": {"x": 1}}}),
            "modelo.padrao",
            "texto",
        ),
    ] {
        let e = validar_documento(&doc).unwrap_err();
        assert_eq!(e[0].chave, chave, "{e:?}");
        assert!(e[0].motivo.contains(esperado), "{chave}: {e:?}");
    }
    let ok = validar_documento(&json!({
        "revisao": 3, "$schema": "x", "//": "comentario",
        "voz": {"//": ["a"], "tts": {"pastas": ["/a"], "provedor": "elevenlabs"}},
        "elevenlabs": {"estabilidade": 0.5}, "api": {"tarefas_por_minuto": 5}
    }))
    .unwrap();
    assert_eq!(ok.revisao, 3);
    assert_eq!(ok.valores.len(), 4);
}

#[test]
fn definir_grava_com_revisao_historico_conflito_e_recusa_do_ambiente() {
    let d = tmp("def");
    let arq = d.join("config.json");
    let nada = amb(&[]);
    let c = carga::carregar(&nada, &arq, None).unwrap();
    let mut m = Map::new();
    m.insert("modelo.padrao".into(), json!("um"));
    m.insert("canais.discord.permitidos".into(), json!(["a"]));
    let n = carga::definir(
        Alvo {
            arquivo: &arq,
            atual: &c,
            esperada: Some(0),
        },
        &m,
    )
    .unwrap();
    assert_eq!(n, 1);
    let doc: Value = serde_json::from_slice(&std::fs::read(&arq).unwrap()).unwrap();
    assert_eq!(doc["revisao"], 1);
    assert_eq!(doc["modelo"]["padrao"], "um");
    assert_eq!(doc["canais"]["discord"]["permitidos"], json!(["a"]));
    assert!(!d.join("config.json.tmp").exists());
    // Revisao velha: conflito com a atual, e nada muda.
    let c = carga::carregar(&nada, &arq, None).unwrap();
    let mut m2 = Map::new();
    m2.insert("modelo.padrao".into(), json!("dois"));
    match carga::definir(
        Alvo {
            arquivo: &arq,
            atual: &c,
            esperada: Some(0),
        },
        &m2,
    ) {
        Err(Recusa::Conflito { atual: 1 }) => {}
        r => panic!("{r:?}"),
    }
    // null remove; a anterior vai para o historico.
    let mut m3 = Map::new();
    m3.insert("canais.discord.permitidos".into(), Value::Null);
    m3.insert("modelo.padrao".into(), json!("dois"));
    assert_eq!(
        carga::definir(
            Alvo {
                arquivo: &arq,
                atual: &c,
                esperada: Some(1)
            },
            &m3
        )
        .unwrap(),
        2
    );
    let doc: Value = serde_json::from_slice(&std::fs::read(&arq).unwrap()).unwrap();
    assert_eq!(doc["modelo"]["padrao"], "dois");
    assert!(doc.get("canais").is_none(), "{doc}");
    let hist: Vec<_> = std::fs::read_dir(carga::historico_de(&arq))
        .unwrap()
        .collect();
    assert_eq!(hist.len(), 1);
    // Chave que vem do ambiente, segredo, desconhecida e tipo errado: 422, cada uma.
    let a = amb(&[("PHXCLAW_MODELO", "amb")]);
    let c = carga::carregar(&a, &arq, None).unwrap();
    let mut m4 = Map::new();
    m4.insert("modelo.padrao".into(), json!("tres"));
    m4.insert("xai.chave".into(), json!("xai-123"));
    m4.insert("modelo.padra".into(), json!("x"));
    m4.insert("api.tarefas_por_minuto".into(), json!("dez"));
    match carga::definir(
        Alvo {
            arquivo: &arq,
            atual: &c,
            esperada: None,
        },
        &m4,
    ) {
        Err(Recusa::Invalida(e)) => {
            let por: HashMap<_, _> = e
                .iter()
                .map(|x| (x.chave.as_str(), x.motivo.as_str()))
                .collect();
            assert!(por["modelo.padrao"].contains("vem do ambiente"), "{e:?}");
            assert!(por["xai.chave"].contains("phxclaw xai chave"), "{e:?}");
            assert!(por["modelo.padra"].contains("`modelo.padrao`"), "{e:?}");
            assert!(por["api.tarefas_por_minuto"].contains("inteiro"), "{e:?}");
        }
        r => panic!("{r:?}"),
    }
    let doc: Value = serde_json::from_slice(&std::fs::read(&arq).unwrap()).unwrap();
    assert_eq!(doc["revisao"], 2, "recusa nao grava");
    let _ = std::fs::remove_dir_all(&d);
}

/// A gravacao foi tirada do `ConfigStore` para uma funcao comum: o caminho antigo tem de
/// continuar igual (conflito, revisao carimbada, historico).
#[test]
fn o_config_store_antigo_grava_pelo_mesmo_caminho_e_igual() {
    let d = tmp("store");
    let p = d.join("config.json");
    let doc = json!({
        "schema_version": "0.40.0",
        "config_uuid": "01890a5d-ac96-774b-bcce-b302099a8057",
        "revision": 1,
        "security": {"deny_by_default": true, "secrets_plaintext_forbidden": true}
    });
    std::fs::write(&p, serde_json::to_vec(&doc).unwrap()).unwrap();
    let s = ConfigStore::new(&p);
    assert!(matches!(
        s.save(doc.clone(), 7),
        Err(phxclaw_config_runtime::ConfigError::RevisionConflict {
            expected: 7,
            actual: 1
        })
    ));
    let snap = s.save(doc, 1).unwrap();
    assert_eq!(snap.revision, 2);
    assert_eq!(std::fs::read_dir(d.join("history")).unwrap().count(), 1);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn o_exemplo_gerado_se_valida_e_o_schema_nao_tem_segredo() {
    let a = validar_documento(&gerar::exemplo()).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(a.valores.len() > 30, "{}", a.valores.len());
    let s = serde_json::to_string(&gerar::esquema()).unwrap();
    for c in catalogo() {
        let folha = c.chave.rsplit('.').next().unwrap();
        if matches!(c.natureza, Natureza::Segredo { .. }) {
            assert!(
                !s.contains(&format!("(ambiente: {})", c.variavel)),
                "{} no schema",
                c.chave
            );
        } else if c.no_arquivo() {
            assert!(
                s.contains(&format!("\"{folha}\"")),
                "{} fora do schema",
                c.chave
            );
        }
    }
}

#[test]
fn artefatos_gerados_estao_em_dia() {
    for (rel, esperado) in gerar::artefatos() {
        let atual = std::fs::read_to_string(raiz().join(rel)).unwrap_or_default();
        assert!(
            atual == esperado,
            "{rel} velho em relacao ao catalogo: rode `cargo run -p phxclaw-config-runtime \
             --example gerar_config`"
        );
    }
}

/// O levantamento do fonte (`tools/config_catalogo.py`): variavel no fonte fora do
/// catalogo, catalogo sem leitor, e a catraca das leituras soltas.
#[test]
fn levantamento_do_fonte_e_catraca_das_leituras_soltas() {
    let s = std::process::Command::new("python3")
        .arg(raiz().join("tools/config_catalogo.py"))
        .output()
        .expect("python3 para o levantamento");
    assert!(
        s.status.success(),
        "{}{}",
        String::from_utf8_lossy(&s.stdout),
        String::from_utf8_lossy(&s.stderr)
    );
}

/// Palavras-guia: a forma sem acento que, no texto em portugues, so pode aparecer com
/// acento. Lista curta de proposito -- e a amostra das que ja sairam erradas na tela
/// (qualificacao da UI de 01/10/2026, M14: «Binario», «nao configuracao»).
const SEM_ACENTO: &[&str] = &[
    "configuracao",
    "padrao",
    "diretorio",
    "endereco",
    "nao",
    "so",
    "servico",
    "usuario",
    "binario",
    "revisao",
    "saida",
    "codigo",
    "memoria",
    "executavel",
    "numero",
    "publica",
    "publicas",
    "obrigatoria",
    "obrigatorio",
    "visao",
    "seguranca",
    "relatorio",
    "duracao",
    "medicao",
    "ativacao",
    "pagina",
    "espaco",
    "verificacao",
    "raizes",
    "propria",
    "criacao",
    "decisao",
    "transcricao",
    "reordenacao",
    "missao",
    "papeis",
    "visivel",
    "destinatarios",
    "audio",
];

/// Palavras que so existem em portugues: texto «em ingles» com uma delas e copia nao
/// traduzida.
const SO_PORTUGUES: &[&str] = &[
    "do", "da", "dos", "das", "que", "para", "nao", "pasta", "chave", "senha", "modelo",
];

/// As palavras de um texto, sem as que sao codigo (caminho, constante, `chave=valor`,
/// `host:porta`): `CAPACIDADES_PADRAO` e `_memoria` sao identificadores, e identificador
/// nao leva acento.
fn palavras(t: &str) -> impl Iterator<Item = String> + '_ {
    t.split_whitespace()
        .map(|w| w.trim_matches(|c: char| "()[],;:.«»\"'".contains(c)))
        .filter(|w| !w.is_empty() && !w.contains(|c: char| "_/.=<>:@".contains(c)))
        .map(str::to_lowercase)
}

/// A descricao e a do motivo sao ROTULO: portugues com acento e ingles em toda chave.
/// Reprova chave sem ingles, ingles que e o portugues copiado, e portugues sem acento
/// onde a palavra-guia exige.
#[test]
fn descricao_tem_ingles_e_portugues_com_acento() {
    let mut erros = vec![];
    for c in catalogo() {
        let mut pt = vec![c.descricao.as_str()];
        let mut en = vec![c.descricao_en.as_str()];
        if let Natureza::Ambiente { motivo } = &c.natureza {
            pt.push(motivo);
            match c.motivo_en {
                Some(m) => en.push(m),
                None => erros.push(format!("{}: motivo sem ingles", c.chave)),
            }
        }
        for t in &pt {
            for p in palavras(t) {
                if SEM_ACENTO.contains(&p.as_str()) {
                    erros.push(format!("{}: «{p}» sem acento em «{t}»", c.chave));
                }
            }
        }
        for t in &en {
            if t.trim().is_empty() {
                erros.push(format!("{}: sem ingles", c.chave));
            } else if !t.is_ascii() {
                erros.push(format!("{}: ingles com acento «{t}»", c.chave));
            }
            for p in palavras(t) {
                if SO_PORTUGUES.contains(&p.as_str()) {
                    erros.push(format!("{}: ingles com «{p}» em «{t}»", c.chave));
                }
            }
        }
    }
    assert!(
        erros.is_empty(),
        "{} defeitos:\n{}",
        erros.len(),
        erros.join("\n")
    );
}

/// O catalogo da tela e a API levam os dois idiomas (a tela escolhe pela fabrica).
#[test]
fn a_entrada_do_catalogo_leva_os_dois_idiomas() {
    let c = por_chave("acao.bin").unwrap();
    let e = gerar::entrada(c);
    assert_eq!(
        e["descricao"],
        json!("Binário do phxclaw exportado entre passos")
    );
    assert_eq!(
        e["descricao_en"],
        json!("phxclaw binary exported between steps")
    );
    assert!(e["motivo_so_ambiente_en"]
        .as_str()
        .is_some_and(|m| m.contains("GitHub Action")));
    let d = gerar::entrada(por_chave("canais.discord.token").unwrap());
    assert_eq!(d["descricao_en"], json!("Bot token (discord)"));
}
