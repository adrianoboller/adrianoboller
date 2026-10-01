//! `python_project` contra o bwrap, o uv e o interpretador de verdade: o erro de tipo do
//! mypy na linha certa, o teste que falha dito pelo nome, a dependencia que esta no cache
//! instalada sem rede e a que nao esta recusada dizendo isso.

use phxclaw_agent::python::PythonProjectTool;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

fn ctx() -> ToolContext {
    let d = std::env::temp_dir().join(format!("phx-py-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    ToolContext {
        task_id: "t".into(),
        workdir: d,
        timeout: Duration::from_secs(300),
    }
}

fn ferramenta() -> Option<PythonProjectTool> {
    let bwrap = phxclaw_agent::arquivos::achar_bwrap()?;
    match PythonProjectTool::detectar(bwrap) {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("python ausente: {e}");
            None
        }
    }
}

fn escrever(raiz: &Path, arquivos: &[(&str, &str)]) {
    for (nome, texto) in arquivos {
        let p = raiz.join(nome);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, texto).unwrap();
    }
}

async fn rodar(t: &PythonProjectTool, c: &ToolContext, args: Value) -> Value {
    let r = t.run(args, c).await.unwrap().content;
    serde_json::from_str(&r).unwrap()
}

const CALC: &str = "import os\n\n\ndef soma(a: int, b: int) -> int:\n    return a + b\n\n\ndef errado() -> int:\n    return \"texto\"\n";
const TESTE: &str = "from calc import soma\n\n\ndef test_ok():\n    assert soma(1, 2) == 3\n\n\ndef test_falha():\n    assert soma(2, 2) == 5\n";

/// O principal: mypy aponta arquivo, linha e codigo do erro de tipo; pytest diz QUAL teste
/// falhou e em que linha; ruff aponta o import sem uso. Nada disso e texto de terminal.
#[tokio::test]
async fn mypy_pytest_e_ruff_devolvem_diagnostico_estruturado() {
    let Some(t) = ferramenta() else { return };
    assert_eq!(t.capability(), "shell.exec");
    let c = ctx();
    let p = c.workdir.join("proj");
    escrever(&p, &[("calc.py", CALC), ("tests/test_calc.py", TESTE)]);

    let v = rodar(&t, &c, json!({"action":"typecheck","path":"proj"})).await;
    assert_eq!(v["sucesso"], false, "{v}");
    assert_eq!(v["venv"]["criado"], true, "{v}");
    let d = v["diagnosticos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["codigo"] == "return-value")
        .unwrap_or_else(|| panic!("sem return-value: {v}"));
    assert_eq!(d["arquivo"], "calc.py", "{v}");
    assert_eq!(d["linha"], 9, "{v}");
    assert_eq!(d["nivel"], "error", "{v}");

    let v = rodar(&t, &c, json!({"action":"test","path":"proj"})).await;
    assert_eq!(v["venv"]["criado"], false, "o venv se reaproveita: {v}");
    assert_eq!(v["testes"]["aprovados"], 1, "{v}");
    assert_eq!(v["testes"]["falhas"], 1, "{v}");
    let d = &v["diagnosticos"][0];
    assert_eq!(d["codigo"], "tests/test_calc.py::test_falha", "{v}");
    assert_eq!(d["arquivo"], "tests/test_calc.py", "{v}");
    assert_eq!(d["linha"], 9, "{v}");
    assert_eq!(d["nivel"], "failure", "{v}");
    assert!(d["mensagem"].as_str().unwrap().contains("4 == 5"), "{v}");

    let v = rodar(&t, &c, json!({"action":"lint","path":"proj"})).await;
    let d = &v["diagnosticos"][0];
    assert_eq!(
        (&d["codigo"], &d["arquivo"], &d["linha"]),
        (&json!("F401"), &json!("calc.py"), &json!(1)),
        "{v}"
    );

    // Consertado, os tres passam limpos -- e o teste so daquele no tambem.
    escrever(
        &p,
        &[
            (
                "calc.py",
                "def soma(a: int, b: int) -> int:\n    return a + b\n",
            ),
            (
                "tests/test_calc.py",
                "from calc import soma\n\n\ndef test_ok() -> None:\n    assert soma(1, 2) == 3\n",
            ),
        ],
    );
    for acao in ["typecheck", "test", "lint"] {
        let v = rodar(&t, &c, json!({"action":acao,"path":"/work/proj"})).await;
        assert_eq!(v["sucesso"], true, "{acao}: {v}");
        assert_eq!(v["erros"], 0, "{acao}: {v}");
    }
    let v = rodar(
        &t,
        &c,
        json!({"action":"test","path":"proj","target":"tests/test_calc.py::test_ok"}),
    )
    .await;
    assert_eq!(v["testes"]["aprovados"], 1, "{v}");
}

/// Dependencia que esta no cache do uv entra sem rede; a que nao esta e recusada dizendo
/// que esta fora do cache e qual e -- e o venv nao finge que ficou pronto.
#[tokio::test]
async fn dependencia_do_cache_instala_e_a_de_fora_recusa_dizendo() {
    let Some(t) = ferramenta() else { return };
    if t.uv.is_none() || t.cache_uv.is_none() {
        eprintln!("uv ou cache ausente: pulado");
        return;
    }
    let c = ctx();
    let p = c.workdir.join("dep");
    escrever(
        &p,
        &[
            ("requirements.txt", "requests\n"),
            (
                "main.py",
                "import sys\nimport requests\nprint('versao', requests.__version__, sys.argv[1:])\n",
            ),
        ],
    );
    let v = rodar(&t, &c, json!({"action":"setup","path":"dep"})).await;
    assert_eq!(v["sucesso"], true, "{v}");
    assert_eq!(v["venv"]["dependencias"], "instaladas", "{v}");
    // O argumento hostil chega como texto ao script, sem shell no meio.
    let v = rodar(
        &t,
        &c,
        json!({"action":"run","path":"dep","script":"main.py","args":["a b","$(id)","x'; touch /work/pwn; '"]}),
    )
    .await;
    assert_eq!(v["sucesso"], true, "{v}");
    assert_eq!(v["venv"]["dependencias"], "em dia", "{v}");
    let saida = v["saida_cauda"].as_str().unwrap();
    assert!(saida.contains("versao"), "{v}");
    assert!(saida.contains("'$(id)'"), "{v}");
    assert!(!c.workdir.join("pwn").exists());
    // O venv tem a dependencia DENTRO dele, nao emprestada do interpretador.
    assert!(
        std::fs::read_dir(p.join(".venv/lib"))
            .unwrap()
            .flatten()
            .any(|py| py.path().join("site-packages/requests").is_dir())
    );

    escrever(
        &p,
        &[("requirements.txt", "requests\nnaoexiste-phxclaw-xyz\n")],
    );
    let v = rodar(&t, &c, json!({"action":"test","path":"dep"})).await;
    assert_eq!(v["sucesso"], false, "{v}");
    let e = v["erro"].as_str().unwrap();
    assert!(e.contains("fora do cache local"), "{e}");
    assert!(e.contains("naoexiste-phxclaw-xyz"), "{e}");
    assert!(e.contains("nao tem rede"), "{e}");
}

/// Script que quebra devolve o traceback como diagnostico: arquivo, linha e excecao.
#[tokio::test]
async fn run_aponta_a_linha_da_excecao() {
    let Some(t) = ferramenta() else { return };
    let c = ctx();
    escrever(
        &c.workdir,
        &[
            ("lib.py", "def f():\n    return 1 / 0\n"),
            ("main.py", "from lib import f\n\nf()\n"),
        ],
    );
    let v = rodar(&t, &c, json!({"action":"run","script":"main.py"})).await;
    assert_eq!(v["sucesso"], false, "{v}");
    let d = &v["diagnosticos"][0];
    assert_eq!(
        (&d["arquivo"], &d["linha"]),
        (&json!("lib.py"), &json!(2)),
        "{v}"
    );
    assert_eq!(d["codigo"], "ZeroDivisionError", "{v}");
}

#[tokio::test]
async fn recusa_caminho_fora_shell_e_acao_desconhecida() {
    let Some(t) = ferramenta() else { return };
    let c = ctx();
    escrever(&c.workdir, &[("ok.py", "print(1)\n")]);
    for (args, esperado) in [
        (json!({"action":"test","path":"../fora"}), "fora"),
        (json!({"action":"test","path":"p'; reboot; '"}), "invalido"),
        (json!({"action":"test","path":"nada"}), "inexistente"),
        (json!({"action":"test","target":"ok.py::t;id"}), "nodeid"),
        (json!({"action":"test","target":"../x"}), "fora"),
        (json!({"action":"run"}), "script"),
        (json!({"action":"run","script":"../x.py"}), "fora"),
        (json!({"action":"run","script":"/etc/passwd"}), ".py"),
        (
            json!({"action":"run","script":"ok.py","args":"rm -rf /"}),
            "lista",
        ),
        (json!({"action":"publish"}), "desconhecida"),
    ] {
        match t.run(args.clone(), &c).await {
            Err(ToolError::Denied(m)) | Err(ToolError::InvalidArguments(m)) => {
                assert!(m.contains(esperado), "{args}: {m}")
            }
            outro => panic!("{args}: {outro:?}"),
        }
    }
    // Recusa vem antes de processo: nenhum venv nasceu.
    assert!(!c.workdir.join(".venv").exists());
}
