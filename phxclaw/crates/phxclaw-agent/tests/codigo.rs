//! A frente de codigo contra o que ela usa de verdade: o git e o bwrap da maquina num
//! repositorio temporario, o portao do motor tirando ponto antes da escrita, o `.gitignore`
//! na busca, o caderno regravado que o JSON ainda le, e a revisao conferindo o que o
//! modelo (roteirizado) devolve. GitHub e GitLab: contra um servidor FALSO local -- nao ha
//! credencial real aqui.

use phxclaw_agent::git::{GitTool, WorktreeTool};
use phxclaw_agent::montagem::{CAPACIDADES_PADRAO, Montagem};
use phxclaw_agent::*;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-cod-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ctx_em(d: &Path) -> ToolContext {
    ToolContext {
        task_id: "t".into(),
        workdir: d.to_path_buf(),
        timeout: Duration::from_secs(60),
    }
}

async fn rodar(t: &dyn Tool, c: &ToolContext, args: Value) -> Value {
    let r = t.run(args.clone(), c).await;
    let r = r.unwrap_or_else(|e| panic!("{} {args}: {e}", t.spec().name));
    serde_json::from_str(&r.content).unwrap_or_else(|_| panic!("nao e JSON: {}", r.content))
}

fn bwrap() -> Option<PathBuf> {
    let b = phxclaw_agent::arquivos::achar_bwrap();
    if b.is_none() {
        eprintln!("sem bwrap: prova do git pulada");
    }
    b
}

// ------------------------------------------------------------------ git e worktree

/// O principal do git: o binario de verdade, no sandbox, devolvendo estrutura -- status com
/// o arquivo novo e o mudado, diff com o hunk na linha certa, commit com hash, log, show,
/// blame com o autor do agente, ramo novo, stash que guarda e devolve.
#[tokio::test]
async fn git_de_verdade_no_sandbox_devolve_estrutura() {
    let Some(b) = bwrap() else { return };
    let d = tmp("git");
    let c = ctx_em(&d);
    let (ler, esc) = (GitTool::leitura(b.clone()), GitTool::escrita(b.clone()));
    assert_eq!(
        (ler.capability(), esc.capability()),
        ("git.read", "git.write")
    );
    rodar(
        &esc,
        &c,
        json!({"action":"init","path":"repo","branch":"main"}),
    )
    .await;
    std::fs::write(d.join("repo/a.txt"), "um\ndois\ntres\n").unwrap();
    rodar(
        &esc,
        &c,
        json!({"action":"add","path":"repo","paths":["."]}),
    )
    .await;
    let v = rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"repo","message":"primeiro"}),
    )
    .await;
    let hash = v["commit"]["commit"].as_str().unwrap().to_string();
    assert_eq!(hash.len(), 40, "{v}");
    assert_eq!(v["commit"]["autor"], "PhxClaw agente");

    std::fs::write(d.join("repo/a.txt"), "um\nDOIS\ntres\n").unwrap();
    std::fs::write(d.join("repo/novo.txt"), "x\n").unwrap();
    let st = rodar(&ler, &c, json!({"action":"status","path":"repo"})).await;
    assert_eq!(st["ramo"], "main", "{st}");
    let arqs = st["arquivos"].as_array().unwrap();
    let est = |n: &str| {
        arqs.iter()
            .find(|a| a["caminho"] == n)
            .map(|a| a["estado"].clone())
    };
    assert_eq!(est("a.txt"), Some(json!("modificado")), "{st}");
    assert_eq!(est("novo.txt"), Some(json!("nao_rastreado")), "{st}");

    let df = rodar(&ler, &c, json!({"action":"diff","path":"repo"})).await;
    let f = &df["arquivos"][0];
    assert_eq!(f["caminho"], "a.txt", "{df}");
    assert_eq!(
        (f["adicoes"].as_u64(), f["remocoes"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(f["hunks"][0]["novo_inicio"], 1);
    assert!(
        f["hunks"][0]["linhas"]
            .as_array()
            .unwrap()
            .contains(&json!("+DOIS"))
    );

    rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"repo","message":"segundo","all":true}),
    )
    .await;
    let lg = rodar(&ler, &c, json!({"action":"log","path":"repo","limit":5})).await;
    let assuntos: Vec<&str> = lg["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["assunto"].as_str().unwrap())
        .collect();
    assert_eq!(assuntos, ["segundo", "primeiro"]);
    let sh = rodar(
        &ler,
        &c,
        json!({"action":"show","path":"repo","rev":"HEAD"}),
    )
    .await;
    assert_eq!(sh["commit"]["assunto"], "segundo");
    assert_eq!(sh["arquivos"][0]["caminho"], "a.txt", "{sh}");
    let bl = rodar(
        &ler,
        &c,
        json!({"action":"blame","path":"repo","file":"a.txt"}),
    )
    .await;
    let l = bl["linhas"].as_array().unwrap();
    assert_eq!(l.len(), 3, "{bl}");
    assert_eq!(
        (l[1]["texto"].as_str(), l[1]["resumo"].as_str()),
        (Some("DOIS"), Some("segundo"))
    );
    assert_eq!(l[0]["commit"].as_str().unwrap(), &hash[..12]);

    rodar(
        &esc,
        &c,
        json!({"action":"checkout","path":"repo","branch":"topico","create":true}),
    )
    .await;
    let br = rodar(&ler, &c, json!({"action":"branches","path":"repo"})).await;
    let atual: Vec<&str> = br["ramos"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["atual"] == true)
        .map(|r| r["ramo"].as_str().unwrap())
        .collect();
    assert_eq!(atual, ["topico"], "{br}");

    std::fs::write(d.join("repo/a.txt"), "guardado\n").unwrap();
    let s = rodar(
        &esc,
        &c,
        json!({"action":"stash","path":"repo","op":"push","message":"wip"}),
    )
    .await;
    assert_eq!(
        s["status"]["arquivos"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["caminho"] == "a.txt")
            .count(),
        0,
        "{s}"
    );
    let s = rodar(
        &esc,
        &c,
        json!({"action":"stash","path":"repo","op":"list"}),
    )
    .await;
    assert!(
        s["pilha"][0]["mensagem"].as_str().unwrap().contains("wip"),
        "{s}"
    );
    rodar(&esc, &c, json!({"action":"stash","path":"repo","op":"pop"})).await;
    assert_eq!(
        std::fs::read_to_string(d.join("repo/a.txt")).unwrap(),
        "guardado\n"
    );

    // Leitura nao muda nada, e opcao disfarcada de referencia e recusada antes do git.
    let e = ler
        .run(
            json!({"action":"diff","path":"repo","rev":"--output=/work/vazou"}),
            &c,
        )
        .await;
    assert!(matches!(e, Err(ToolError::InvalidArguments(_))), "{e:?}");
    assert!(!d.join("vazou").exists());
    let e = ler
        .run(json!({"action":"commit","path":"repo","message":"x"}), &c)
        .await;
    assert!(
        matches!(e, Err(ToolError::InvalidArguments(_))),
        "git.read nao comita: {e:?}"
    );
}

/// O gancho do repositorio NAO roda: `pre-commit` que grava um arquivo nao grava, porque o
/// `core.hooksPath` vai para /dev/null e o commit e `--no-verify`.
#[tokio::test]
async fn gancho_do_repositorio_nao_executa() {
    let Some(b) = bwrap() else { return };
    let d = tmp("gancho");
    let c = ctx_em(&d);
    let esc = GitTool::escrita(b.clone());
    rodar(&esc, &c, json!({"action":"init"})).await;
    let g = d.join(".git/hooks/post-commit");
    std::fs::write(&g, "#!/bin/sh\necho executou > /work/gancho.txt\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(d.join("a"), "1").unwrap();
    rodar(&esc, &c, json!({"action":"add","paths":["a"]})).await;
    rodar(&esc, &c, json!({"action":"commit","message":"m"})).await;
    assert!(
        !d.join("gancho.txt").exists(),
        "o gancho do repositorio rodou"
    );
}

/// Worktree por tarefa: duas arvores em ramos proprios, commit numa nao aparece na outra,
/// o repositorio principal nao as lista como arquivo novo, e remover recusa arvore suja.
#[tokio::test]
async fn worktrees_isolam_tarefas_paralelas() {
    let Some(b) = bwrap() else { return };
    let d = tmp("wt");
    let c = ctx_em(&d);
    let (ler, esc) = (GitTool::leitura(b.clone()), GitTool::escrita(b.clone()));
    let wt = WorktreeTool {
        bwrap: b.clone(),
        timeout: Duration::from_secs(60),
    };
    rodar(
        &esc,
        &c,
        json!({"action":"init","path":"r","branch":"main"}),
    )
    .await;
    std::fs::write(d.join("r/base.txt"), "base\n").unwrap();
    rodar(&esc, &c, json!({"action":"add","path":"r","paths":["."]})).await;
    rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"r","message":"base"}),
    )
    .await;

    let a = rodar(
        &wt,
        &c,
        json!({"action":"add","path":"r","name":"tarefa-a"}),
    )
    .await;
    let bb = rodar(
        &wt,
        &c,
        json!({"action":"add","path":"r","name":"tarefa-b"}),
    )
    .await;
    assert_eq!(a["path"], "r/.worktrees/tarefa-a", "{a}");
    assert_eq!(bb["ramo"], "phxclaw/tarefa-b");
    std::fs::write(d.join("r/.worktrees/tarefa-a/so-a.txt"), "a\n").unwrap();
    rodar(
        &esc,
        &c,
        json!({"action":"add","path":"r/.worktrees/tarefa-a","paths":["."]}),
    )
    .await;
    rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"r/.worktrees/tarefa-a","message":"trabalho a"}),
    )
    .await;
    let la = rodar(
        &ler,
        &c,
        json!({"action":"log","path":"r/.worktrees/tarefa-a"}),
    )
    .await;
    let lb = rodar(
        &ler,
        &c,
        json!({"action":"log","path":"r/.worktrees/tarefa-b"}),
    )
    .await;
    assert_eq!(la["commits"][0]["assunto"], "trabalho a");
    assert_eq!(
        lb["commits"][0]["assunto"], "base",
        "a arvore b viu o commit da a"
    );
    assert!(!d.join("r/.worktrees/tarefa-b/so-a.txt").exists());

    let st = rodar(&ler, &c, json!({"action":"status","path":"r"})).await;
    assert_eq!(
        st["limpo"], true,
        "o principal lista as arvores como arquivo novo: {st}"
    );
    let l = rodar(&wt, &c, json!({"action":"list","path":"r"})).await;
    let caminhos: Vec<&str> = l["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        caminhos,
        ["r", "r/.worktrees/tarefa-a", "r/.worktrees/tarefa-b"],
        "{l}"
    );

    std::fs::write(d.join("r/.worktrees/tarefa-b/sujo.txt"), "nao gravado\n").unwrap();
    let e = wt
        .run(json!({"action":"remove","path":"r","name":"tarefa-b"}), &c)
        .await;
    assert!(e.is_err(), "removeu arvore com trabalho nao gravado");
    assert!(d.join("r/.worktrees/tarefa-b/sujo.txt").exists());
    rodar(
        &wt,
        &c,
        json!({"action":"remove","path":"r","name":"tarefa-a"}),
    )
    .await;
    assert!(!d.join("r/.worktrees/tarefa-a").exists());
    let br = rodar(&ler, &c, json!({"action":"branches","path":"r"})).await;
    assert!(
        br.to_string().contains("phxclaw/tarefa-a"),
        "o ramo do trabalho tem de ficar: {br}"
    );
}

// ------------------------------------------------------------------ checkpoint pelo motor

fn store() -> TaskStore {
    TaskStore::new(tmp("store")).unwrap()
}

fn fim(id: &str) -> phxclaw_agent_core::LlmReply {
    ScriptedLlm::call(id, "final_answer", json!({"answer": "feito"}))
}

/// O ponto nasce no PORTAO, antes da escrita: o agente grava, edita, e restaura o ponto de
/// antes da edicao -- o arquivo volta, e a propria restauracao deixou um ponto.
#[tokio::test]
async fn portao_tira_ponto_antes_de_cada_escrita_e_restaura() {
    for c in ["git.read", "git.write", "code.review"] {
        assert!(CAPACIDADES_PADRAO.contains(&c), "{c} fora do padrao");
    }
    let s = store();
    let m = Montagem::new(s.clone());
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path":"a.txt","content":"versao 1"}),
        ),
        ScriptedLlm::call(
            "c2",
            "edit_file",
            json!({"path":"a.txt","old_text":"versao 1","new_text":"versao 2"}),
        ),
        ScriptedLlm::call("c3", "write_file", json!({"path":"b.txt","content":"novo"})),
        ScriptedLlm::call("c4", "checkpoint_list", json!({})),
        fim("c5"),
    ]));
    let t = m
        .agent_with(llm.clone())
        .run(
            Task::new("escreva", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let pontos = phxclaw_checkpoint::CheckpointManager::new(
        phxclaw_agent::checkpoint::pasta_de_pontos(&s, &t.id),
    )
    .list()
    .unwrap();
    // Antes do write (pasta vazia), antes do edit (a=v1), antes do 2o write (a=v2).
    assert_eq!(pontos.len(), 3, "{pontos:#?}");
    assert_eq!(pontos[1].label.as_deref(), Some("antes de edit_file"));
    assert_eq!(pontos[0].files.len(), 0, "o primeiro ponto e a pasta vazia");
    let lista = &t.steps[3].summary;
    assert!(
        lista.contains("antes de write_file"),
        "checkpoint_list: {lista}"
    );

    // Restaurar o ponto de antes do edit, por uma tarefa nova sobre a mesma pasta e pelo
    // portao: e a ferramenta de verdade, com o id que o list devolveu.
    let id = pontos[1].uuid.to_string();
    let w = s.workdir(&t.id);
    let r = phxclaw_agent::checkpoint::CheckpointTool {
        store: s.clone(),
        restaurar: true,
    };
    let c = ToolContext {
        task_id: t.id.clone(),
        workdir: w.clone(),
        timeout: Duration::from_secs(30),
    };
    let v = rodar(&r, &c, json!({"id": id, "remove_new": true})).await;
    assert_eq!(
        std::fs::read_to_string(w.join("a.txt")).unwrap(),
        "versao 1",
        "{v}"
    );
    assert!(!w.join("b.txt").exists(), "{v}");
    assert_eq!(v["removidos"], json!(["b.txt"]));
    // Ponto de outra tarefa nao restaura nesta pasta.
    let outra = ToolContext {
        task_id: "outra".into(),
        workdir: w.clone(),
        timeout: Duration::from_secs(30),
    };
    assert!(r.run(json!({"id": id}), &outra).await.is_err());
}

/// Ferramenta que so le nao tira ponto: o portao decide pela capacidade.
#[tokio::test]
async fn leitura_nao_tira_ponto() {
    let s = store();
    let m = Montagem::new(s.clone());
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call("c1", "list_files", json!({})),
        ScriptedLlm::call("c2", "glob", json!({"pattern":"*"})),
        fim("c3"),
    ]));
    let t = m
        .agent_with(llm)
        .run(
            Task::new("leia", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    assert!(!phxclaw_agent::checkpoint::pasta_de_pontos(&s, &t.id).exists());
}

// ------------------------------------------------------------------ busca

#[tokio::test]
async fn glob_e_grep_respeitam_o_gitignore() {
    let d = tmp("busca");
    let w = |p: &str, t: &str| {
        let f = d.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, t).unwrap();
    };
    w(".gitignore", "target/\n*.log\n");
    w("src/main.rs", "fn main() {\n    let alvo = 1;\n}\n");
    w("src/lib/util.rs", "pub fn alvo() {}\n");
    w("target/debug/gerado.rs", "fn alvo() {}\n");
    w("x.log", "alvo\n");
    w("dados.bin", "alvo\0binario");
    w("sub/.gitignore", "local.rs\n");
    w("sub/local.rs", "alvo\n");
    let c = ctx_em(&d);
    let g = busca::GlobTool;
    let v = rodar(&g, &c, json!({"pattern":"**/*.rs"})).await;
    let mut a: Vec<String> = serde_json::from_value(v["arquivos"].clone()).unwrap();
    a.sort();
    assert_eq!(a, ["src/lib/util.rs", "src/main.rs"], "{v}");
    let v = rodar(&g, &c, json!({"pattern":"*.rs","path":"src/lib"})).await;
    assert_eq!(v["arquivos"], json!(["src/lib/util.rs"]));
    let v = rodar(&g, &c, json!({"pattern":"**/*.rs","include_ignored":true})).await;
    assert_eq!(v["total"], 4, "{v}");

    let gr = busca::GrepTool;
    let v = rodar(&gr, &c, json!({"pattern":"\\balvo\\b"})).await;
    let achados = v["achados"].as_array().unwrap();
    let onde: Vec<(String, u64)> = achados
        .iter()
        .map(|a| {
            (
                a["arquivo"].as_str().unwrap().to_string(),
                a["linha"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        onde,
        [
            ("src/lib/util.rs".to_string(), 1),
            ("src/main.rs".into(), 2)
        ],
        "{v}"
    );
    let v = rodar(
        &gr,
        &c,
        json!({"pattern":"ALVO","case_insensitive":true,"output_mode":"count","glob":"main.rs"}),
    )
    .await;
    assert_eq!(
        v["contagem"],
        json!([{"arquivo":"src/main.rs","ocorrencias":1}])
    );
    let v = rodar(
        &gr,
        &c,
        json!({"pattern":"alvo","context":1,"glob":"main.rs"}),
    )
    .await;
    assert_eq!(v["achados"][0]["antes"], json!(["fn main() {"]));
    assert!(gr.run(json!({"pattern":"("}), &c).await.is_err());
    assert!(
        gr.run(json!({"pattern":"x","path":"../fora"}), &c)
            .await
            .is_err()
    );
}

// ------------------------------------------------------------------ notebook

const CADERNO: &str = r##"{
 "cells": [
  {"cell_type": "markdown", "id": "m1", "metadata": {}, "source": ["# Titulo\n", "texto"]},
  {"cell_type": "code", "id": "c1", "metadata": {}, "execution_count": 3,
   "source": ["print(1)\n", "x = 2"],
   "outputs": [{"output_type": "stream", "name": "stdout", "text": ["1\n"]},
               {"output_type": "display_data", "data": {"image/png": "iVBORw0K", "text/plain": ["<Figure>"]}, "metadata": {}}]}
 ],
 "metadata": {"kernelspec": {"language": "python", "name": "python3", "display_name": "Python 3"}},
 "nbformat": 4, "nbformat_minor": 5
}"##;

#[tokio::test]
async fn caderno_lido_e_editado_por_celula_continua_caderno() {
    let d = tmp("nb");
    std::fs::write(d.join("n.ipynb"), CADERNO).unwrap();
    let c = ctx_em(&d);
    let (ler, ed) = (notebook::NotebookReadTool, notebook::NotebookEditTool);
    let v = rodar(&ler, &c, json!({"path":"n.ipynb"})).await;
    assert_eq!(v["linguagem"], "python");
    assert_eq!(v["celulas"][1]["fonte"], "print(1)\nx = 2");
    assert_eq!(v["celulas"][1]["saidas"][0]["texto"], "1\n");
    assert_eq!(
        v["celulas"][1]["saidas"][1]["outros"][0]["mime"],
        "image/png"
    );

    rodar(
        &ed,
        &c,
        json!({"path":"n.ipynb","action":"replace","cell_id":"c1","source":"print(2)\ny = 3\n"}),
    )
    .await;
    rodar(&ed, &c, json!({"path":"n.ipynb","action":"insert","index":0,"cell_type":"markdown","source":"intro"})).await;
    rodar(
        &ed,
        &c,
        json!({"path":"n.ipynb","action":"delete","cell_id":"m1"}),
    )
    .await;
    let bruto = std::fs::read_to_string(d.join("n.ipynb")).unwrap();
    let nb: Value = serde_json::from_str(&bruto).unwrap();
    let cells = nb["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2, "{bruto}");
    assert_eq!(cells[0]["source"], json!(["intro"]));
    assert_eq!(
        cells[0]["id"].as_str().map(str::len),
        Some(8),
        "4.5 exige id"
    );
    assert!(
        cells[0].get("outputs").is_none(),
        "markdown com outputs o nbformat recusa"
    );
    assert_eq!(cells[1]["source"], json!(["print(2)\n", "y = 3\n"]));
    assert_eq!(
        cells[1]["outputs"],
        json!([]),
        "saida velha de codigo mudado ficou"
    );
    assert_eq!(cells[1]["execution_count"], Value::Null);
    assert!(
        bruto.starts_with("{\n \"cells\""),
        "indentacao do nbformat: {bruto:.40}"
    );
    assert!(
        ed.run(json!({"path":"n.ipynb","action":"delete","index":9}), &c)
            .await
            .is_err()
    );
}

// ------------------------------------------------------------------ revisao

const DIFF_REV: &str = "diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -10,3 +10,4 @@ fn f()
 let a = 1;
-let b = a / 2;
+let b = a / 0;
+let c = b;
 fim();
";

#[tokio::test]
async fn revisao_confere_arquivo_linha_e_severidade_do_modelo() {
    let resposta = json!({"resumo":"divisao por zero","achados":[
        {"arquivo":"src/a.rs","linha":11,"severidade":"high","achado":"divisao por zero","sugestao":"use a/2"},
        {"arquivo":"src/a.rs","linha":99,"severidade":"alta","achado":"linha inventada"},
        {"arquivo":"src/b.rs","linha":11,"severidade":"alta","achado":"arquivo inventado"},
        {"arquivo":"src/a.rs","linha":12,"severidade":"gravissima","achado":"escala inventada"},
        {"arquivo":"b/src/a.rs","linha":12,"severidade":"info","achado":"c nao usado"}
    ]});
    // Primeira resposta em prosa: a ferramenta pede de novo uma vez.
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::text("Acho que tem um problema."),
        ScriptedLlm::text(&format!("```json\n{resposta}\n```")),
    ]));
    let t = revisao::CodeReviewTool {
        llm: llm.clone(),
        bwrap: None,
    };
    assert_eq!(t.capability(), "code.review");
    let v = rodar(&t, &ctx_em(&tmp("rev")), json!({"diff": DIFF_REV})).await;
    let a = v["achados"].as_array().unwrap();
    assert_eq!(a.len(), 2, "{v}");
    assert_eq!(
        (a[0]["linha"].as_u64(), a[0]["severidade"].as_str()),
        (Some(11), Some("alta"))
    );
    assert_eq!(a[1]["arquivo"], "src/a.rs");
    let motivos: Vec<&str> = v["descartados"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["motivo"].as_str().unwrap())
        .collect();
    assert_eq!(
        motivos,
        [
            "linha fora do diff",
            "arquivo fora do diff",
            "severidade fora da escala"
        ]
    );
    // O modelo viu o diff numerado pela linha nova.
    let visto = llm.seen.lock().unwrap()[0].0[1].content.clone();
    assert!(visto.contains("    11 +let b = a / 0;"), "{visto}");
    assert_eq!(llm.seen.lock().unwrap().len(), 2);

    // Duas respostas sem JSON: erro, nunca revisao vazia.
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::text("nada"),
        ScriptedLlm::text("ainda nada"),
    ]));
    let t = revisao::CodeReviewTool { llm, bwrap: None };
    assert!(
        t.run(json!({"diff": DIFF_REV}), &ctx_em(&tmp("rev")))
            .await
            .is_err()
    );
}

/// A revisao do repositorio da pasta passa pelo MESMO motor de diff do `git diff`.
#[tokio::test]
async fn revisao_do_repo_le_o_diff_pelo_git() {
    let Some(b) = bwrap() else { return };
    let d = tmp("revgit");
    let c = ctx_em(&d);
    let esc = GitTool::escrita(b.clone());
    rodar(&esc, &c, json!({"action":"init"})).await;
    std::fs::write(d.join("m.py"), "a = 1\n").unwrap();
    rodar(&esc, &c, json!({"action":"add","paths":["."]})).await;
    rodar(&esc, &c, json!({"action":"commit","message":"m"})).await;
    std::fs::write(d.join("m.py"), "a = 1\nsenha = 'x'\n").unwrap();
    let llm = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text(
        r#"{"resumo":"segredo","achados":[{"arquivo":"m.py","linha":2,"severidade":"critica","achado":"senha no codigo"}]}"#,
    )]));
    let t = revisao::CodeReviewTool {
        llm,
        bwrap: Some(b),
    };
    let v = rodar(&t, &c, json!({})).await;
    assert_eq!(v["achados"][0]["linha"], 2, "{v}");
    assert_eq!(v["arquivos"], 1);
}

// ------------------------------------------------------------------ forjas (servidor FALSO)

type Pedidos = Arc<Mutex<Vec<(String, String, String, String)>>>;

/// Servidor HTTP falso: grava (metodo, caminho+query, cabecalho de token, corpo) e responde
/// o minimo que GitHub e GitLab responderiam.
async fn servidor_falso() -> (String, Pedidos) {
    use axum::body::Bytes;
    use axum::http::{HeaderMap, Method, Uri};
    let pedidos: Pedidos = Arc::default();
    let p2 = pedidos.clone();
    let app = axum::Router::new().fallback(move |m: Method, u: Uri, h: HeaderMap, corpo: Bytes| {
        let p = p2.clone();
        async move {
            let tok = h
                .get("authorization")
                .or_else(|| h.get("private-token"))
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let caminho = u.path_and_query().map(|x| x.to_string()).unwrap_or_default();
            p.lock().unwrap().push((m.to_string(), caminho.clone(), tok.clone(), String::from_utf8_lossy(&corpo).into()));
            let path = u.path();
            let ok = axum::http::StatusCode::OK;
            let j = |v: Value| (ok, [("content-type", "application/json")], v.to_string());
            match (m.as_str(), path) {
                ("GET", "/repos/dono/proj/issues") => j(json!([
                    {"number":1,"title":"bug","state":"open","user":{"login":"ana"},"html_url":"u1","body":"corpo","labels":[{"name":"bug"}],"comments":0},
                    {"number":2,"title":"pr disfarcado","state":"open","user":{"login":"bia"},"pull_request":{}}
                ])),
                ("GET", "/repos/dono/proj/pulls/7") if h.get("accept").and_then(|v| v.to_str().ok()) == Some("application/vnd.github.diff") => {
                    (ok, [("content-type", "text/plain")], DIFF_REV.to_string())
                }
                ("GET", "/repos/dono/proj/pulls/7") => j(json!({"number":7,"title":"pr","state":"open","user":{"login":"ana"},"head":{"ref":"topico"},"base":{"ref":"main"},"draft":false,"merged":false})),
                ("GET", "/repos/dono/proj/issues/7/comments") => j(json!([{"user":{"login":"rev"},"body":"ok","created_at":"2026-10-01"}])),
                ("POST", "/repos/dono/proj/issues/7/comments") => j(json!({"id":99,"html_url":"c99"})),
                ("POST", "/repos/dono/proj/pulls") => j(json!({"number":8,"title":"novo","state":"open","head":{"ref":"topico"},"base":{"ref":"main"}})),
                ("GET", "/repos/dono/sumiu/issues") => (
                    axum::http::StatusCode::NOT_FOUND,
                    [("content-type", "application/json")],
                    json!({"message":"Not Found"}).to_string(),
                ),
                ("GET", "/projects/grupo%2Fproj/merge_requests") => j(json!([{"iid":3,"title":"mr","state":"opened","author":{"username":"caio"},"source_branch":"f","target_branch":"main","web_url":"w"}])),
                ("GET", "/projects/grupo%2Fproj/merge_requests/3/diffs") => j(json!([
                    {"old_path":"src/a.rs","new_path":"src/a.rs","a_mode":"100644","b_mode":"100644","new_file":false,"renamed_file":false,"deleted_file":false,
                     "diff":"@@ -10,3 +10,4 @@ fn f()\n let a = 1;\n-let b = a / 2;\n+let b = a / 0;\n+let c = b;\n fim();\n"},
                    {"old_path":"n.txt","new_path":"n.txt","a_mode":"0","b_mode":"100644","new_file":true,"renamed_file":false,"deleted_file":false,"diff":"@@ -0,0 +1 @@\n+oi\n"}
                ])),
                ("POST", "/projects/grupo%2Fproj/merge_requests/3/notes") => j(json!({"id":5})),
                // Servidor hostil: ecoa o cabecalho do token no erro e no sucesso.
                ("GET", "/repos/dono/eco/issues") | ("GET", "/projects/grupo%2Feco/issues") => (
                    axum::http::StatusCode::UNAUTHORIZED,
                    [("content-type", "application/json")],
                    json!({"message": format!("credencial recusada: {tok}")}).to_string(),
                ),
                ("GET", "/repos/dono/eco/pulls") => j(json!([{"number":1,"title": tok}])),
                _ => j(json!({"message":"rota falsa sem resposta"})),
            }
        }
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, pedidos)
}

#[tokio::test]
async fn github_e_gitlab_contra_servidor_falso_com_token_so_pelo_broker() {
    use phxclaw_agent::forja::{Forja, ForjaCliente, ForjaTool, guardar_token};
    use phxclaw_secret_broker::SecretValue;
    let (base, pedidos) = servidor_falso().await;
    let raiz = tmp("forja");
    let broker =
        phxclaw_agent::canais::broker_em(&phxclaw_agent::forja::pasta_da_forja(&raiz)).unwrap();
    let id_gh = guardar_token(
        &broker,
        Forja::Github,
        SecretValue::new("ghp_SEGREDO_GH".into()),
    )
    .unwrap();
    let id_gl = guardar_token(
        &broker,
        Forja::Gitlab,
        SecretValue::new("glpat-SEGREDO-GL".into()),
    )
    .unwrap();
    // Mesmo token de novo: mesmo envelope.
    assert_eq!(
        guardar_token(
            &broker,
            Forja::Github,
            SecretValue::new("ghp_SEGREDO_GH".into())
        )
        .unwrap(),
        id_gh
    );
    // O que o disco guarda nao tem o token em claro.
    for e in walk(&raiz) {
        let b = std::fs::read(&e).unwrap();
        assert!(
            !String::from_utf8_lossy(&b).contains("SEGREDO"),
            "token em claro em {}",
            e.display()
        );
    }

    let gh = Arc::new(ForjaCliente::novo(
        Forja::Github,
        base.clone(),
        broker.clone(),
        id_gh,
    ));
    let gl = Arc::new(ForjaCliente::novo(
        Forja::Gitlab,
        base.clone(),
        broker.clone(),
        id_gl,
    ));
    let c = ctx_em(&tmp("forja-w"));
    let ghr = ForjaTool {
        cliente: gh.clone(),
        escrita: false,
    };
    let ghw = ForjaTool {
        cliente: gh.clone(),
        escrita: true,
    };
    let glr = ForjaTool {
        cliente: gl.clone(),
        escrita: false,
    };
    let glw = ForjaTool {
        cliente: gl.clone(),
        escrita: true,
    };
    assert_eq!(
        [
            ghr.capability(),
            ghw.capability(),
            glr.capability(),
            glw.capability()
        ],
        ["github.read", "github.write", "gitlab.read", "gitlab.write"]
    );

    let v = rodar(&ghr, &c, json!({"action":"list_issues","repo":"dono/proj"})).await;
    assert_eq!(
        v["issues"].as_array().unwrap().len(),
        1,
        "PR na lista de issues: {v}"
    );
    assert_eq!(v["issues"][0]["autor"], "ana");
    let v = rodar(
        &ghr,
        &c,
        json!({"action":"get_pr","repo":"dono/proj","number":7}),
    )
    .await;
    assert_eq!(
        (v["de"].as_str(), v["para"].as_str()),
        (Some("topico"), Some("main"))
    );
    assert_eq!(v["lista_de_comentarios"][0]["autor"], "rev");
    let v = rodar(
        &ghr,
        &c,
        json!({"action":"pr_diff","repo":"dono/proj","number":7}),
    )
    .await;
    assert_eq!(v["arquivos"][0]["caminho"], "src/a.rs", "{v}");
    rodar(
        &ghw,
        &c,
        json!({"action":"comment","repo":"dono/proj","number":7,"body":"revisado"}),
    )
    .await;
    let v = rodar(&ghw, &c, json!({"action":"create_pr","repo":"dono/proj","title":"novo","head":"topico","base":"main"})).await;
    assert_eq!(v["numero"], 8);
    let e = ghr
        .run(json!({"action":"list_issues","repo":"dono/sumiu"}), &c)
        .await;
    let e = format!("{e:?}");
    assert!(
        e.contains("404") && e.contains("Not Found") && !e.contains("SEGREDO"),
        "{e}"
    );
    assert!(
        ghr.run(json!({"action":"list_issues","repo":"../x/y"}), &c)
            .await
            .is_err()
    );
    // Servidor que ecoa o token: nem o erro nem o sucesso o devolvem ao modelo.
    for (t, args) in [
        (&ghr, json!({"action":"list_issues","repo":"dono/eco"})),
        (&ghr, json!({"action":"list_prs","repo":"dono/eco"})),
        (&glr, json!({"action":"list_issues","repo":"grupo/eco"})),
    ] {
        let e = format!("{:?}", t.run(args.clone(), &c).await);
        assert!(e.starts_with("Err("), "{args}: {e}");
        assert!(!e.contains("SEGREDO"), "{args} vazou o token: {e}");
    }

    let v = rodar(&glr, &c, json!({"action":"list_prs","repo":"grupo/proj"})).await;
    assert_eq!(v["prs"][0]["numero"], 3);
    let v = rodar(
        &glr,
        &c,
        json!({"action":"pr_diff","repo":"grupo/proj","number":3}),
    )
    .await;
    let arqs = v["arquivos"].as_array().unwrap();
    assert_eq!(arqs.len(), 2, "{v}");
    assert_eq!(arqs[1]["estado"], "adicionado");
    rodar(
        &glw,
        &c,
        json!({"action":"comment","repo":"grupo/proj","number":3,"on":"pr","body":"ok"}),
    )
    .await;

    let p = pedidos.lock().unwrap().clone();
    let gh_p: Vec<_> = p.iter().filter(|x| x.1.starts_with("/repos/")).collect();
    let gl_p: Vec<_> = p.iter().filter(|x| x.1.starts_with("/projects/")).collect();
    assert!(
        gh_p.iter().all(|x| x.2 == "Bearer ghp_SEGREDO_GH"),
        "{gh_p:?}"
    );
    assert!(gl_p.iter().all(|x| x.2 == "glpat-SEGREDO-GL"), "{gl_p:?}");
    assert!(p.iter().any(|x| x.0 == "POST"
        && x.1 == "/repos/dono/proj/issues/7/comments"
        && x.3.contains("revisado")));
    assert!(
        p.iter()
            .any(|x| x.0 == "POST" && x.1 == "/projects/grupo%2Fproj/merge_requests/3/notes")
    );

    // A montagem acha as forjas pela pasta: com token, quatro ferramentas; sem, nenhuma.
    assert_eq!(phxclaw_agent::forja::ferramentas_da_pasta(&raiz).len(), 4);
    assert!(phxclaw_agent::forja::ferramentas_da_pasta(&tmp("sem-forja")).is_empty());
}

fn walk(d: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(walk(&p));
        } else {
            v.push(p);
        }
    }
    v
}

/// Sentinela: o repositorio declara script em assinatura (`gpg.program` + commit
/// assinado), `textconv`, filtro `clean`/`smudge` e driver de merge; nenhuma acao do git
/// do agente pode roda-lo. Reproduz o achado da revisao de seguranca de 01/10/2026.
#[tokio::test]
async fn git_nao_executa_programa_declarado_pelo_repositorio() {
    let Some(b) = bwrap() else { return };
    let d = tmp("sentinela");
    let c = ctx_em(&d);
    let (ler, esc) = (GitTool::leitura(b.clone()), GitTool::escrita(b.clone()));
    rodar(&esc, &c, json!({"action":"init","branch":"main"})).await;
    let x = d.join("x.sh");
    std::fs::write(&x, "#!/bin/sh\necho \"$0 $*\" >> /work/SENTINELA\ncat\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&x, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(d.join(".gitattributes"), "a.txt filter=f diff=d merge=m\n").unwrap();
    std::fs::write(d.join("a.txt"), "um\n").unwrap();
    rodar(&esc, &c, json!({"action":"add","paths":["."]})).await;
    rodar(&esc, &c, json!({"action":"commit","message":"base"})).await;
    // Commit com cabecalho gpgsig, montado pelo git do hospedeiro: e o que faz o
    // `log.showSignature` chamar o `gpg.program`.
    let git = |args: &[&str], entrada: Option<&str>| {
        use std::io::Write;
        let mut p = std::process::Command::new("git")
            .args(args)
            .current_dir(&d)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(e) = entrada {
            p.stdin.take().unwrap().write_all(e.as_bytes()).unwrap();
        }
        let o = p.wait_with_output().unwrap();
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    };
    let arvore = git(&["rev-parse", "HEAD^{tree}"], None);
    let pai = git(&["rev-parse", "HEAD"], None);
    let corpo = format!(
        "tree {arvore}\nparent {pai}\nauthor t <t@t> 1 +0000\ncommitter t <t@t> 1 +0000\n\
gpgsig -----BEGIN PGP SIGNATURE-----\n \n abc\n -----END PGP SIGNATURE-----\n\nassinado\n"
    );
    let assinado = git(
        &["hash-object", "-t", "commit", "-w", "--stdin"],
        Some(&corpo),
    );
    git(&["update-ref", "HEAD", &assinado], None);
    for (k, v) in [
        ("log.showSignature", "true"),
        ("gpg.program", "/work/x.sh"),
        ("filter.f.clean", "/work/x.sh"),
        ("filter.f.smudge", "/work/x.sh"),
        ("diff.d.textconv", "/work/x.sh"),
        ("merge.m.driver", "/work/x.sh %O %A %B"),
    ] {
        git(&["config", k, v], None);
    }
    std::fs::write(d.join("a.txt"), "dois\n").unwrap();
    let sentinela = d.join("SENTINELA");
    for (t, args) in [
        (&ler, json!({"action":"log"})),
        (&ler, json!({"action":"show"})),
        (&ler, json!({"action":"blame","file":"a.txt"})),
        (&ler, json!({"action":"status"})),
        (&ler, json!({"action":"diff"})),
        (&esc, json!({"action":"stash","op":"push"})),
        (&esc, json!({"action":"stash","op":"pop"})),
    ] {
        rodar(t, &c, args.clone()).await;
        assert!(
            !sentinela.exists(),
            "{args} executou o programa do repositorio: {}",
            std::fs::read_to_string(&sentinela).unwrap_or_default()
        );
    }
}

// ------------------------------------------------------------------ prova real (papel F)

/// O guarda «o ponto e de outra pasta» do `checkpoint_restore`. O teste do portao acima
/// conferia isso com `task_id: "outra"` -- e ali o ponto nem CARREGA (a pasta de pontos da
/// tarefa "outra" nao existe), entao ele passava com o guarda apagado. Aqui o ponto carrega
/// (mesma tarefa) e so a pasta difere: sem o guarda, o restore gravaria na pasta que o
/// MANIFESTO diz, nao na da chamada.
#[tokio::test]
async fn restore_de_ponto_de_outra_pasta_e_negado_pelo_guarda() {
    let s = store();
    let w = s.workdir("t1");
    std::fs::create_dir_all(&w).unwrap();
    std::fs::write(w.join("a.txt"), "do ponto").unwrap();
    let pontos = phxclaw_agent::checkpoint::pasta_de_pontos(&s, "t1");
    let id = phxclaw_agent::checkpoint::antes_de_escrever(&pontos, &w, "prova")
        .unwrap()
        .unwrap();
    std::fs::write(w.join("a.txt"), "depois").unwrap();
    let r = phxclaw_agent::checkpoint::CheckpointTool {
        store: s.clone(),
        restaurar: true,
    };
    let outra_pasta = ToolContext {
        task_id: "t1".into(),
        workdir: tmp("outra-pasta"),
        timeout: Duration::from_secs(30),
    };
    let e = r.run(json!({"id": id}), &outra_pasta).await;
    assert_eq!(
        std::fs::read_to_string(w.join("a.txt")).unwrap(),
        "depois",
        "o restore alcancou a pasta do manifesto: {e:?}"
    );
    assert!(matches!(e, Err(ToolError::Denied(_))), "{e:?}");
}

/// Forja que redireciona: o token nao pode ir para o servidor do `Location`, e o 3xx tem de
/// ser ERRO. O GitHub responde 301 para repositorio renomeado; aceito como sucesso, o
/// `pr_diff` devolvia diff VAZIO e o `phxclaw revisar --pr ... --falhar-em alta` saia 0 --
/// portao de CI verde sem revisar nada.
#[tokio::test]
async fn forja_que_redireciona_nao_leva_o_token_e_nao_vira_diff_vazio() {
    use axum::http::{StatusCode, Uri, header};
    use phxclaw_agent::forja::{Forja, ForjaCliente, ForjaTool, guardar_token};
    use phxclaw_secret_broker::SecretValue;
    // B: o destino do redirecionamento. Conta tudo o que chega.
    let (base_b, pedidos_b) = servidor_falso().await;
    // A: a forja configurada, que manda tudo para B.
    let b2 = base_b.clone();
    let app = axum::Router::new().fallback(move |u: Uri| {
        let destino = format!(
            "{b2}{}",
            u.path_and_query()
                .map(|x| x.to_string())
                .unwrap_or_default()
        );
        async move {
            (
                StatusCode::MOVED_PERMANENTLY,
                [(header::LOCATION, destino)],
                "",
            )
        }
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_a = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });

    let raiz = tmp("forja-redir");
    let broker =
        phxclaw_agent::canais::broker_em(&phxclaw_agent::forja::pasta_da_forja(&raiz)).unwrap();
    let id_gh = guardar_token(
        &broker,
        Forja::Github,
        SecretValue::new("ghp_REDIR_GH".into()),
    )
    .unwrap();
    let id_gl = guardar_token(
        &broker,
        Forja::Gitlab,
        SecretValue::new("glpat-REDIR-GL".into()),
    )
    .unwrap();
    let c = ctx_em(&tmp("forja-redir-w"));
    let gh = ForjaTool {
        cliente: Arc::new(ForjaCliente::novo(
            Forja::Github,
            base_a.clone(),
            broker.clone(),
            id_gh,
        )),
        escrita: false,
    };
    let gl = ForjaTool {
        cliente: Arc::new(ForjaCliente::novo(
            Forja::Gitlab,
            base_a.clone(),
            broker.clone(),
            id_gl,
        )),
        escrita: false,
    };
    // Roda as tres ANTES de conferir: o dano (token no host B) se mede primeiro, o veredito
    // depois -- conferir so o `is_err` esconderia o vazamento atras do primeiro assert.
    let diff = gh
        .run(
            json!({"action":"pr_diff","repo":"dono/proj","number":7}),
            &c,
        )
        .await
        .map(|o| o.content);
    let issues = gh
        .run(json!({"action":"list_issues","repo":"dono/proj"}), &c)
        .await
        .map(|o| o.content);
    let mrs = gl
        .run(json!({"action":"list_prs","repo":"grupo/proj"}), &c)
        .await
        .map(|o| o.content);
    let p = pedidos_b.lock().unwrap().clone();
    let vazou: Vec<_> = p.iter().filter(|x| x.2.contains("REDIR")).collect();
    assert!(
        vazou.is_empty(),
        "token foi para o host do redirecionamento: {vazou:?}"
    );
    assert!(p.is_empty(), "o redirecionamento foi seguido: {p:?}");
    assert!(diff.is_err(), "301 virou diff: {diff:?}");
    assert!(issues.is_err(), "{issues:?}");
    assert!(mrs.is_err(), "{mrs:?}");
}
