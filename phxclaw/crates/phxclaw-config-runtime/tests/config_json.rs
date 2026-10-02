//! Os JSON de `config/` (SP000020): dado, nao configuracao de usuario. O inventario de quem
//! le cada um sai de `tools/config_inventario.py`; aqui fica o que a suite PROVA sobre
//! todos eles, sem lista digitada:
//!
//! 1. nenhum carrega segredo -- chave com nome de credencial (`*_key`, `*_token`,
//!    `*_secret`, `password`, `senha`) com valor preenchido, ou valor com cara de
//!    credencial (prefixo conhecido, URL com senha, forma de chave), pelos MESMOS crivos
//!    que recusam segredo no `config.json` (`carga::nome_de_segredo`,
//!    `carga::credencial_aparente`);
//! 2. nenhum nomeia uma variavel `PHXCLAW_*` fora do catalogo -- configuracao que a tela
//!    nao mostra e o `config.json` nao alcanca;
//! 3. todos sao JSON valido, e a varredura alcanca as subpastas (agents, skills, trust...).

use phxclaw_config_runtime::agente::carga::{
    credencial_aparente, nome_de_segredo, prefixo_de_credencial,
};
use phxclaw_config_runtime::agente::por_variavel;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn pasta_config() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config")
}

fn jsons(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            jsons(&p, saida);
        } else if p.extension().is_some_and(|x| x == "json") {
            saida.push(p);
        }
    }
}

/// Valor de chave com nome de segredo que e placeholder, nao credencial: vazio, referencia
/// (`env:`, `${...}`, `<...>`, `secret://`), o nome de uma variavel, ou uma palavra de
/// politica em kebab (`os-keyring-only`, `one-time-sha256-at-rest`) -- credencial nao se
/// escreve em minusculas com hifens.
fn placeholder(v: &str) -> bool {
    let t = v.trim();
    let kebab = t.contains('-')
        && t.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    t.is_empty()
        || t.starts_with("env:")
        || t.starts_with("${")
        || t.starts_with('<')
        || t.starts_with("secret://")
        || t.starts_with("broker:")
        || t.chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        || kebab
}

/// Credencial de verdade num texto qualquer: prefixo conhecido COM material depois dele
/// (o marcador `-----BEGIN PRIVATE KEY-----` sozinho, sem corpo, e um detector, nao uma
/// chave) ou URL com senha. A «forma de chave» (hexa de 64, base64 de 44) so conta sob
/// chave com nome de segredo: `sha256` e `public_key_base64` tem essa forma de proposito.
fn credencial_em_texto(s: &str) -> Option<String> {
    let t = s.trim();
    if let Some(p) = prefixo_de_credencial(t) {
        let resto = &t[p.len()..];
        let material = if p == "-----BEGIN" {
            resto.contains('\n')
        } else {
            resto.chars().count() >= 8
        };
        return material.then(|| {
            format!(
                "prefixo de credencial conhecido ({} caracteres)",
                t.chars().count()
            )
        });
    }
    credencial_aparente(t).filter(|m| m.starts_with("URL com senha"))
}

fn varrer(caminho: &str, v: &Value, erros: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                let c = format!("{caminho}.{k}");
                if let Value::String(s) = x {
                    if nome_de_segredo(k) && (credencial_aparente(s).is_some() || !placeholder(s)) {
                        erros.push(format!(
                            "{c}: chave com nome de segredo preenchida ({} caracteres)",
                            s.chars().count()
                        ));
                    }
                }
                nomes_phxclaw(&c, k, erros);
                varrer(&c, x, erros);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                varrer(&format!("{caminho}[{i}]"), x, erros);
            }
        }
        Value::String(s) => {
            if let Some(m) = credencial_em_texto(s) {
                erros.push(format!("{caminho}: {m}"));
            }
            nomes_phxclaw(caminho, s, erros);
        }
        _ => {}
    }
}

/// Todo `PHXCLAW_<NOME>` dentro do texto tem de estar no catalogo -- ou ter leitor nas
/// ferramentas (tools/, scripts/, ci/, .github/, installer/): as variaveis de portao de
/// release (`..._RELEASE_SIGN_CMD`, `..._TEST_DATABASE_URL`) sao lidas pelo CI e pelos scripts, nao pelo
/// agente, e nao cabem no config.json. O que ninguem le em lugar nenhum e erro.
fn nomes_phxclaw(caminho: &str, texto: &str, erros: &mut Vec<String>) {
    let mut resto = texto;
    while let Some(i) = resto.find("PHXCLAW_") {
        let fim = resto[i..]
            .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
            .map(|j| i + j)
            .unwrap_or(resto.len());
        let nome = resto[i..fim].trim_end_matches('_');
        if nome.len() > "PHXCLAW_".len()
            && por_variavel(nome).is_none()
            && !lida_pelas_ferramentas(nome)
        {
            erros.push(format!(
                "{caminho}: {nome} fora do catalogo e sem leitor nas ferramentas"
            ));
        }
        resto = &resto[fim..];
    }
}

fn lida_pelas_ferramentas(nome: &str) -> bool {
    let raiz = pasta_config().join("..");
    let mut arquivos = vec![];
    for d in ["tools", "scripts", "ci", ".github", "installer"] {
        textos(&raiz.join(d), &mut arquivos);
    }
    arquivos.iter().any(|t| t.contains(nome))
}

fn textos(dir: &Path, saida: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            textos(&p, saida);
        } else if p
            .extension()
            .is_some_and(|x| ["py", "sh", "yml", "yaml", "rs"].contains(&x.to_str().unwrap_or("")))
        {
            if let Ok(t) = std::fs::read_to_string(&p) {
                saida.push(t);
            }
        }
    }
}

#[test]
fn os_json_de_config_nao_carregam_segredo_nem_variavel_fora_do_catalogo() {
    let mut arquivos = vec![];
    jsons(&pasta_config(), &mut arquivos);
    arquivos.sort();
    assert!(
        arquivos.len() >= 100,
        "varreu so {} arquivos",
        arquivos.len()
    );
    let tem = |p: &str| arquivos.iter().any(|a| a.to_string_lossy().contains(p));
    assert!(tem("/config/agents/"), "nao alcancou config/agents");
    assert!(tem("/config/trust/"), "nao alcancou config/trust");
    let mut erros = vec![];
    for a in &arquivos {
        let rel = a
            .strip_prefix(pasta_config().join(".."))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| a.display().to_string())
            .replace("../../", "");
        let texto = std::fs::read_to_string(a).unwrap();
        match serde_json::from_str::<Value>(&texto) {
            Ok(v) => varrer(&rel, &v, &mut erros),
            Err(e) => erros.push(format!("{rel}: JSON invalido: {e}")),
        }
    }
    assert!(
        erros.is_empty(),
        "{} problema(s) em {} arquivos de config/:\n{}",
        erros.len(),
        arquivos.len(),
        erros.join("\n")
    );
}

/// RED: um JSON plantado com credencial e com variavel fora do catalogo reprova pelos
/// mesmos crivos; o placeholder (`env:`, nome de variavel) passa.
#[test]
fn segredo_plantado_num_json_reprova() {
    let mut erros = vec![];
    // O nome inexistente e montado em duas partes: a catraca de `tools/config_catalogo.py`
    // varre todo .rs atras de `PHXCLAW_*` e acusaria o proprio teste.
    let inexistente = concat!("PHXCLAW_", "NAO_EXISTE_X");
    let v: Value = serde_json::from_str(&format!(
        r#"{{"provider": {{"api_key": "sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH",
            "token": "env:PHXCLAW_API_TOKEN", "senha": "", "url": "https://u:p@h/x",
            "nota": "defina {inexistente}"}}}}"#
    ))
    .unwrap();
    varrer("t.json", &v, &mut erros);
    let texto = erros.join("\n");
    assert!(
        texto.contains("provider.api_key: chave com nome de segredo"),
        "{texto}"
    );
    assert!(texto.contains("provider.url: URL com senha"), "{texto}");
    assert!(
        texto.contains(&format!("{inexistente} fora do catalogo")),
        "{texto}"
    );
    // Marcador de detector sem corpo nao e chave; hexa de 64 sob `sha256` tambem nao.
    let v: Value = serde_json::from_str(
        r#"{"secret_markers": ["-----BEGIN PRIVATE KEY-----"],
            "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "pairing": {"private_key": "os-keyring-only", "token": "one-time-sha256-at-rest"}}"#,
    )
    .unwrap();
    let mut limpo = vec![];
    varrer("l.json", &v, &mut limpo);
    assert!(limpo.is_empty(), "{limpo:#?}");
    assert!(
        !texto.contains("provider.token"),
        "placeholder env: nao e segredo: {texto}"
    );
    assert!(
        !texto.contains("provider.senha"),
        "vazio nao e segredo: {texto}"
    );
    assert!(!texto.contains("PHXCLAW_API_TOKEN"), "{texto}");
}
