//! A guarda contra pulo calado, provada nos dois sentidos: o repositorio inteiro passa em 0,
//! e um pulo plantado numa COPIA reprova (RED) -- o mesmo pulo pelo `pulado::pular` passa.

use phxclaw_test_support::guarda::{arquivos_de_teste, pulos_calados, pulos_calados_no_texto};
use std::path::{Path, PathBuf};

fn raiz() -> PathBuf {
    // Canonica: `crates/phxclaw-test-support/../..` carrega o nome do proprio crate no
    // caminho, e a exclusao dele e por nome.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn copia(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phxclaw-guarda-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn planta(raiz: &Path, rel: &str, corpo: &str) {
    let p = raiz.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, corpo).unwrap();
}

#[test]
fn o_repositorio_nao_tem_pulo_calado() {
    let raiz = raiz();
    let arquivos = arquivos_de_teste(&raiz);
    // A varredura tem de alcancar os tres lugares onde o pulo vivia: integracao de crate,
    // integracao de app e unitario em src/. Lista vazia seria «0 calados» por nao olhar.
    let tem = |pedaco: &str| {
        arquivos
            .iter()
            .any(|a| a.to_string_lossy().contains(pedaco))
    };
    assert!(
        tem("/crates/phxclaw-agent/tests/"),
        "nao varreu crates/*/tests"
    );
    assert!(tem("/apps/phxclaw/tests/"), "nao varreu apps/*/tests");
    assert!(
        tem("/crates/phxclaw-agent/src/arquivos/tests.rs"),
        "nao varreu src/**/tests.rs"
    );
    let achados = pulos_calados(&raiz);
    let lista: Vec<String> = achados.iter().map(|a| a.to_string()).collect();
    // Catraca: SO DESCE, e nasceu em 0 em 01/10/2026, depois de migrar os 46 lugares que so
    // imprimiam «PULADO» (SP000013). Zero e o chao: nao ha teto a baixar, so a lista a manter
    // vazia. Quem pula por recurso da maquina registra pelo `pulado::pular`, e o portao
    // (`tools/suite.sh`) decide.
    assert!(
        achados.is_empty(),
        "{} pulo(s) calado(s) (catraca em 0); registre pelo \
         phxclaw_test_support::pulado::pular:\n{}",
        achados.len(),
        lista.join("\n")
    );
}

/// RED: o pulo calado plantado numa copia reprova, nas duas formas e nos tres lugares.
#[test]
fn pulo_calado_plantado_reprova() {
    let r = copia("red");
    planta(
        &r,
        "crates/x/tests/a.rs",
        "#[test]\nfn t() {\n    if falta() {\n        eprintln!(\"sem bwrap: pulado\");\n        return;\n    }\n}\n",
    );
    planta(
        &r,
        "apps/y/tests/b.rs",
        "#[test]\nfn t() {\n    let Ok(d) = std::env::var(\"PHX_X\") else {\n        return;\n    };\n    let _ = d;\n}\n",
    );
    planta(
        &r,
        "crates/z/src/m/tests.rs",
        "fn t() {\n    if std::env::var(\"PHX_Y\").is_err() {\n        return;\n    }\n    eprintln!(\n        \"PULADO: Chromium nao encontrado\"\n    );\n}\n",
    );
    // Comentario com a palavra nao conta; string de outro assunto tambem nao.
    planta(
        &r,
        "crates/z/tests/limpo.rs",
        "// o teste pulado e o problema\nfn t() { let _ = \"repulsa\"; }\n",
    );
    let achados = pulos_calados(&r);
    let chaves: Vec<(String, usize, &str)> = achados
        .iter()
        .map(|a| (a.arquivo.clone(), a.linha, a.regra))
        .collect();
    assert_eq!(
        chaves,
        vec![
            (
                "apps/y/tests/b.rs".to_string(),
                3,
                "recurso-ausente-sem-registro"
            ),
            (
                "crates/x/tests/a.rs".to_string(),
                4,
                "texto-de-pulo-sem-registro"
            ),
            (
                "crates/z/src/m/tests.rs".to_string(),
                2,
                "recurso-ausente-sem-registro"
            ),
            (
                "crates/z/src/m/tests.rs".to_string(),
                6,
                "texto-de-pulo-sem-registro"
            ),
        ],
        "{achados:#?}"
    );
    let _ = std::fs::remove_dir_all(&r);
}

/// RED da regra 3 (SP000020): o `return;` dentro de um bloco condicionado a recurso --
/// `is_none()` de uma sonda, `which`, `Path::exists` -- sem `pular(` reprova; com o
/// `pular(` no bloco passa; o `is_err()` que nao e recurso passa SO declarado com o motivo.
#[test]
fn retorno_condicionado_sem_registro_reprova() {
    let r = copia("red3");
    planta(
        &r,
        "crates/x/tests/a.rs",
        "#[test]\nfn t() {\n    if phxclaw_browser::find_chromium().is_none() {\n        return;\n    }\n    if !std::path::Path::new(\"/x\").exists() {\n        eprintln!(\"sem\");\n        return Ok(());\n    }\n    if which(\"hx\").is_err() {\n        return;\n    }\n}\n",
    );
    let achados = pulos_calados(&r);
    let linhas: Vec<(usize, &str)> = achados.iter().map(|a| (a.linha, a.regra)).collect();
    assert_eq!(
        linhas,
        vec![
            (4, "retorno-condicionado-sem-registro"),
            (8, "retorno-condicionado-sem-registro"),
            (11, "retorno-condicionado-sem-registro"),
        ],
        "{achados:#?}"
    );
    let _ = std::fs::remove_dir_all(&r);
    // GREEN: registrado (mesmo com o `pular(` a quatro linhas, quebrado pelo rustfmt), ou
    // declarado como nao-pulo.
    let g = "fn t() {\n    if !p.is_file() {\n        pulado::pular(\n            \"print\",\n            \"rode antes o navegador\",\n        );\n        return;\n    }\n    if s.send(x).await.is_err() {\n        // nao e pulo: o cliente fechou o soquete do servidor falso.\n        return;\n    }\n    if r.is_err() {\n        return Err(e);\n    }\n}\n";
    assert!(
        pulos_calados_no_texto("g.rs", g).is_empty(),
        "{:#?}",
        pulos_calados_no_texto("g.rs", g)
    );
}

/// GREEN: o mesmo pulo pelo `pulado::pular` passa -- inclusive com o `pular(` quebrado em
/// linhas pelo rustfmt, e com a string «pulado» a ate tres linhas dele.
#[test]
fn pulo_registrado_passa() {
    let a = "#[test]\nfn t() {\n    if falta() {\n        pulado::pular(\"bwrap\", \"bwrap ausente\");\n        return;\n    }\n}\n";
    assert!(pulos_calados_no_texto("a.rs", a).is_empty());
    let b = "fn t() {\n    let Ok(d) = std::env::var(\"PHX_X\") else {\n        pulado::pular(\n            \"PHX_X\",\n            \"nao apontado\",\n        );\n        return;\n    };\n}\n";
    assert!(pulos_calados_no_texto("b.rs", b).is_empty());
    let c = "fn t() {\n    pulado::pular(\n        \"x\",\n        \"a prova real nao rodou (pulada)\",\n    );\n}\n";
    assert!(pulos_calados_no_texto("c.rs", c).is_empty());
    // A quatro linhas de distancia ja nao e «ao lado».
    let d = "fn t() {\n    pulado::pular(\"x\", \"y\");\n    let _ = 1;\n    let _ = 2;\n    let _ = 3;\n    eprintln!(\"pulado\");\n}\n";
    assert_eq!(pulos_calados_no_texto("d.rs", d).len(), 1);
}

/// O proprio crate fica fora da varredura: os testes dele plantam o texto proibido.
#[test]
fn o_proprio_crate_fica_fora() {
    let r = raiz();
    assert!(
        arquivos_de_teste(&r)
            .iter()
            .all(|a| !a.to_string_lossy().contains("phxclaw-test-support"))
    );
}
