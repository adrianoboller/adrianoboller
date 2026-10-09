//! SP000031, frente V1 (git e projeto do VS Code): stage por trecho, merge com conflito
//! resolvido pelo agente, substituicao no projeto inteiro com previa e ponto de
//! restauracao, tarefas declaradas pelo projeto sob as regras de comando, e o historico
//! local das gravacoes. O git e o bwrap sao os da maquina, num repositorio temporario.

use phxclaw_agent::busca::ReplaceInProjectTool;
use phxclaw_agent::git::{GitTool, Trecho, blocos_de_conflito, patch_do_trecho};
use phxclaw_agent::historico::{FileHistoryTool, Historico};
use phxclaw_agent::motor::linha_para_as_regras;
use phxclaw_agent::projeto_tarefas::{ProjectTaskTool, carregar};
use phxclaw_agent::regras::{Decisao, RegrasDeComando};
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use phxclaw_test_support::pulado;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-v1-{nome}-{}", phxclaw_types::new_uuid_v7()));
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

async fn erro(t: &dyn Tool, c: &ToolContext, args: Value) -> ToolError {
    match t.run(args.clone(), c).await {
        Ok(r) => panic!("{} {args} devia recusar: {}", t.spec().name, r.content),
        Err(e) => e,
    }
}

fn bwrap() -> Option<PathBuf> {
    let b = phxclaw_agent::arquivos::achar_bwrap();
    if b.is_none() {
        pulado::pular("bwrap", "sem bwrap a prova do git nao roda");
    }
    b
}

/// Repositorio com `a.txt` de `n` linhas ja comitado em `main`.
async fn repo_com_arquivo(esc: &GitTool, c: &ToolContext, n: usize) -> String {
    rodar(
        esc,
        c,
        json!({"action":"init","path":"repo","branch":"main"}),
    )
    .await;
    let texto: String = (1..=n).map(|i| format!("linha {i}\n")).collect();
    std::fs::write(c.workdir.join("repo/a.txt"), &texto).unwrap();
    rodar(esc, c, json!({"action":"add","path":"repo","paths":["."]})).await;
    rodar(
        esc,
        c,
        json!({"action":"commit","path":"repo","message":"base"}),
    )
    .await;
    texto
}

fn linhas_mais(diff: &Value) -> Vec<String> {
    diff["arquivos"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|a| a["hunks"].as_array().cloned().unwrap_or_default())
        .flat_map(|h| h["linhas"].as_array().cloned().unwrap_or_default())
        .filter_map(|l| l.as_str().map(str::to_string))
        .filter(|l| l.starts_with('+'))
        .collect()
}

// ------------------------------------------------------------------ 1. stage por trecho

/// Dois trechos no mesmo arquivo: so o pedido vai ao indice, por indice do hunk e por
/// faixa de linhas dentro de UM hunk (o que o `git add -p` com `e` faz a mao).
#[tokio::test]
async fn add_trecho_leva_so_o_trecho_pedido_ao_indice() {
    let Some(b) = bwrap() else { return };
    let d = tmp("trecho");
    let c = ctx_em(&d);
    let (ler, esc) = (GitTool::leitura(b.clone()), GitTool::escrita(b));
    let base = repo_com_arquivo(&esc, &c, 20).await;

    // Linhas 2 e 18 mudadas: dois hunks separados no -U3.
    let mudado = base
        .replace("linha 2\n", "LINHA DOIS\n")
        .replace("linha 18\n", "LINHA DEZOITO\n");
    std::fs::write(d.join("repo/a.txt"), &mudado).unwrap();
    let df = rodar(&ler, &c, json!({"action":"diff","path":"repo"})).await;
    assert_eq!(
        df["arquivos"][0]["hunks"].as_array().unwrap().len(),
        2,
        "{df}"
    );

    let r = rodar(
        &esc,
        &c,
        json!({"action":"add_trecho","path":"repo","file":"a.txt","hunk":1}),
    )
    .await;
    assert_eq!(r["arquivo"], "a.txt", "{r}");
    let no_indice = rodar(
        &ler,
        &c,
        json!({"action":"diff","path":"repo","cached":true}),
    )
    .await;
    assert_eq!(
        linhas_mais(&no_indice),
        vec!["+LINHA DEZOITO"],
        "{no_indice}"
    );
    let fora = rodar(&ler, &c, json!({"action":"diff","path":"repo"})).await;
    assert_eq!(linhas_mais(&fora), vec!["+LINHA DOIS"], "{fora}");

    // Faixa de linhas dentro de um hunk so: linhas 2 e 4 mudam juntas; so a 4 vai.
    rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"repo","message":"dezoito"}),
    )
    .await;
    let mudado = mudado.replace("linha 4\n", "LINHA QUATRO\n");
    std::fs::write(d.join("repo/a.txt"), &mudado).unwrap();
    let df = rodar(&ler, &c, json!({"action":"diff","path":"repo"})).await;
    assert_eq!(
        df["arquivos"][0]["hunks"].as_array().unwrap().len(),
        1,
        "{df}"
    );
    rodar(
        &esc,
        &c,
        json!({"action":"add_trecho","path":"repo","file":"a.txt","start_line":4,"end_line":4}),
    )
    .await;
    let no_indice = rodar(
        &ler,
        &c,
        json!({"action":"diff","path":"repo","cached":true}),
    )
    .await;
    assert_eq!(
        linhas_mais(&no_indice),
        vec!["+LINHA QUATRO"],
        "{no_indice}"
    );
    let fora = rodar(&ler, &c, json!({"action":"diff","path":"repo"})).await;
    assert_eq!(linhas_mais(&fora), vec!["+LINHA DOIS"], "{fora}");
    // O que foi ao indice comita sozinho e a arvore continua com a outra mudanca.
    rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"repo","message":"quatro"}),
    )
    .await;
    let st = rodar(&ler, &c, json!({"action":"status","path":"repo"})).await;
    assert_eq!(st["arquivos"][0]["estado"], "modificado", "{st}");

    // Trecho que nao existe e arquivo sem diff recusam dizendo o que ha.
    let e = erro(
        &esc,
        &c,
        json!({"action":"add_trecho","path":"repo","file":"a.txt","hunk":7}),
    )
    .await;
    assert!(e.to_string().contains("trecho 7 nao existe"), "{e}");
    std::fs::write(d.join("repo/novo.txt"), "x\n").unwrap();
    let e = erro(
        &esc,
        &c,
        json!({"action":"add_trecho","path":"repo","file":"novo.txt","hunk":0}),
    )
    .await;
    assert!(e.to_string().contains("sem diff contra o indice"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// O recorte por faixa, sem git: `+` fora da faixa cai, `-` fora vira contexto, e o
/// inicio do lado novo do segundo hunk e recalculado quando o primeiro e pulado.
#[test]
fn patch_do_trecho_recorta_por_faixa_e_recalcula_o_inicio() {
    let diff = "diff --git a/f b/f\n--- a/f\n+++ b/f\n\
@@ -1,3 +1,4 @@\n a\n+b\n-c\n+C\n d\n\
@@ -10,2 +11,2 @@\n x\n-y\n+Y\n";
    let arq = &phxclaw_agent::git::analisar_diff(diff)[0];
    let so_c = patch_do_trecho(arq, &Trecho::Linhas(3, 3)).unwrap();
    assert!(so_c.contains("@@ -1,3 +1,3 @@\n a\n-c\n+C\n d\n"), "{so_c}");
    assert!(!so_c.contains("+b"), "{so_c}");
    assert!(!so_c.contains("-y"), "{so_c}");
    let so_segundo = patch_do_trecho(arq, &Trecho::Hunks(vec![1])).unwrap();
    assert!(
        so_segundo.contains("@@ -10,2 +10,2 @@\n x\n-y\n+Y\n"),
        "{so_segundo}"
    );
    let e = patch_do_trecho(arq, &Trecho::Linhas(1, 1)).unwrap_err();
    assert!(e.to_string().contains("nenhuma mudanca"), "{e}");
}

// ------------------------------------------------------------------ 2. merge e conflitos

/// Merge com conflito: a resposta diz que nao concluiu e qual arquivo; `conflitos` traz o
/// bloco com as duas versoes; `resolver` por texto grava e leva ao indice; `commit` fecha
/// o merge com dois pais. E `abort` desfaz um segundo conflito.
#[tokio::test]
async fn merge_com_conflito_lista_resolve_por_texto_e_comita() {
    let Some(b) = bwrap() else { return };
    let d = tmp("merge");
    let c = ctx_em(&d);
    let (ler, esc) = (GitTool::leitura(b.clone()), GitTool::escrita(b));
    let base = repo_com_arquivo(&esc, &c, 3).await;
    rodar(
        &esc,
        &c,
        json!({"action":"branch_create","path":"repo","branch":"topico"}),
    )
    .await;
    std::fs::write(d.join("repo/a.txt"), base.replace("linha 2", "DOIS-main")).unwrap();
    rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"repo","message":"main","all":true}),
    )
    .await;
    rodar(
        &esc,
        &c,
        json!({"action":"checkout","path":"repo","branch":"topico"}),
    )
    .await;
    std::fs::write(d.join("repo/a.txt"), base.replace("linha 2", "DOIS-topico")).unwrap();
    rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"repo","message":"topico","all":true}),
    )
    .await;
    rodar(
        &esc,
        &c,
        json!({"action":"checkout","path":"repo","branch":"main"}),
    )
    .await;

    let m = rodar(
        &esc,
        &c,
        json!({"action":"merge","path":"repo","branch":"topico"}),
    )
    .await;
    assert_eq!(m["concluido"], false, "{m}");
    assert_eq!(m["conflitos"], json!(["a.txt"]), "{m}");

    let cf = rodar(&esc, &c, json!({"action":"conflitos","path":"repo"})).await;
    let bloco = &cf["arquivos"][0]["blocos"][0];
    assert_eq!(cf["arquivos"][0]["arquivo"], "a.txt", "{cf}");
    assert_eq!(bloco["nosso"], json!(["DOIS-main"]), "{cf}");
    assert_eq!(bloco["deles"], json!(["DOIS-topico"]), "{cf}");
    assert_eq!(bloco["rotulo_nosso"], "HEAD", "{cf}");

    let r = rodar(
        &esc,
        &c,
        json!({"action":"resolver","path":"repo","file":"a.txt","block":0,"choice":"texto","text":"DOIS-ambos"}),
    )
    .await;
    assert_eq!(r["blocos_restantes"], 0, "{r}");
    assert_eq!(r["no_indice"], true, "{r}");
    assert_eq!(
        std::fs::read_to_string(d.join("repo/a.txt")).unwrap(),
        "linha 1\nDOIS-ambos\nlinha 3\n"
    );
    let cm = rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"repo","message":"merge topico"}),
    )
    .await;
    assert_eq!(
        cm["commit"]["pais"].as_array().map(Vec::len),
        Some(2),
        "{cm}"
    );
    let st = rodar(&ler, &c, json!({"action":"status","path":"repo"})).await;
    assert!(st["arquivos"].as_array().unwrap().is_empty(), "{st}");

    // Segundo conflito, resolvido por `deles`, e depois `abort` em um terceiro.
    rodar(
        &esc,
        &c,
        json!({"action":"checkout","path":"repo","branch":"outro","create":true,"base":"topico"}),
    )
    .await;
    std::fs::write(d.join("repo/a.txt"), base.replace("linha 2", "DOIS-outro")).unwrap();
    rodar(
        &esc,
        &c,
        json!({"action":"commit","path":"repo","message":"outro","all":true}),
    )
    .await;
    rodar(
        &esc,
        &c,
        json!({"action":"checkout","path":"repo","branch":"main"}),
    )
    .await;
    let m = rodar(
        &esc,
        &c,
        json!({"action":"merge","path":"repo","branch":"outro"}),
    )
    .await;
    assert_eq!(m["concluido"], false, "{m}");
    let r = rodar(
        &esc,
        &c,
        json!({"action":"resolver","path":"repo","file":"a.txt","block":0,"choice":"deles"}),
    )
    .await;
    assert_eq!(r["no_indice"], true, "{r}");
    assert!(
        std::fs::read_to_string(d.join("repo/a.txt"))
            .unwrap()
            .contains("DOIS-outro")
    );
    let ab = rodar(&esc, &c, json!({"action":"abort","path":"repo"})).await;
    assert!(ab["arquivos"].as_array().unwrap().is_empty(), "{ab}");
    assert!(
        std::fs::read_to_string(d.join("repo/a.txt"))
            .unwrap()
            .contains("DOIS-ambos")
    );
    // Rebase e force nao existem no git_write: ramo alheio nunca se reescreve.
    let e = erro(
        &esc,
        &c,
        json!({"action":"rebase","path":"repo","branch":"outro"}),
    )
    .await;
    assert!(e.to_string().contains("action desconhecida"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn blocos_de_conflito_leem_diff3_e_ignoram_marcador_solto() {
    let t = "a\n<<<<<<< HEAD\nx\n||||||| base\no\n=======\ny\n>>>>>>> topico\nb\n=======\nc\n";
    let v = blocos_de_conflito(t);
    assert_eq!(v.len(), 1, "{v:?}");
    assert_eq!((v[0].de, v[0].ate), (2, 8));
    assert_eq!(v[0].base.as_deref(), Some(&["o".to_string()][..]));
    let (novo, resto) = phxclaw_agent::git::resolver_bloco(t, 0, &["z".into()]).unwrap();
    assert_eq!(novo, "a\nz\nb\n=======\nc\n");
    assert_eq!(resto, 0);
}

// ------------------------------------------------------------------ 3. substituir no projeto

/// A previa conta e nao grava; `confirm` grava N arquivos pelo mesmo caminho do
/// `edit_file`; e o ponto tirado antes (o que o portao faz para toda `fs.write`) devolve
/// o conteudo.
#[tokio::test]
async fn replace_in_project_previa_nao_altera_confirmar_altera_e_o_ponto_restaura() {
    let d = tmp("replace");
    let c = ctx_em(&d);
    let t = ReplaceInProjectTool;
    assert!(phxclaw_agent::checkpoint::escreve(t.capability()));
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::create_dir_all(d.join("target")).unwrap();
    std::fs::write(d.join(".gitignore"), "target/\n").unwrap();
    std::fs::write(d.join("src/a.rs"), "fn velho() { velho(); }\n").unwrap();
    std::fs::write(d.join("src/b.rs"), "// velho\n").unwrap();
    std::fs::write(d.join("c.md"), "velho $x\n").unwrap();
    std::fs::write(d.join("target/x.rs"), "velho\n").unwrap();

    let p = rodar(&t, &c, json!({"pattern":"velho","replacement":"novo"})).await;
    assert_eq!(p["previa"], true, "{p}");
    assert_eq!(p["total"], 4, "{p}");
    assert_eq!(p["arquivos"].as_array().unwrap().len(), 3, "{p}");
    assert_eq!(
        std::fs::read_to_string(d.join("src/a.rs")).unwrap(),
        "fn velho() { velho(); }\n"
    );

    let pontos = tmp("pontos");
    let id =
        phxclaw_agent::checkpoint::antes_de_escrever(&pontos, &d, "antes de replace_in_project")
            .unwrap()
            .expect("primeiro ponto");
    let g = rodar(
        &t,
        &c,
        json!({"pattern":"velho","replacement":"novo","glob":"*.rs","confirm":true}),
    )
    .await;
    assert_eq!(g["gravados"], 2, "{g}");
    assert_eq!(
        std::fs::read_to_string(d.join("src/a.rs")).unwrap(),
        "fn novo() { novo(); }\n"
    );
    assert_eq!(
        std::fs::read_to_string(d.join("src/b.rs")).unwrap(),
        "// novo\n"
    );
    assert_eq!(
        std::fs::read_to_string(d.join("c.md")).unwrap(),
        "velho $x\n"
    );
    assert_eq!(
        std::fs::read_to_string(d.join("target/x.rs")).unwrap(),
        "velho\n"
    );

    // Literal: `$` na troca e texto; regex: grupo.
    rodar(
        &t,
        &c,
        json!({"pattern":"$x","replacement":"$y","confirm":true}),
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(d.join("c.md")).unwrap(),
        "velho $y\n"
    );
    rodar(&t, &c, json!({"pattern":"(ve)(lho)","replacement":"$2$1","regex":true,"glob":"*.md","confirm":true})).await;
    assert_eq!(
        std::fs::read_to_string(d.join("c.md")).unwrap(),
        "lhove $y\n"
    );

    let m = phxclaw_checkpoint::CheckpointManager::new(&pontos);
    let ponto = m.load(id.parse().unwrap()).unwrap();
    let r = m.restore(&ponto, false).unwrap();
    assert!(r.restored.len() >= 2, "{r:?}");
    assert_eq!(
        std::fs::read_to_string(d.join("src/a.rs")).unwrap(),
        "fn velho() { velho(); }\n"
    );
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&pontos);
}

// ------------------------------------------------------------------ 4. tarefas do projeto

/// Tarefa declarada roda no sandbox e devolve codigo e saida; arquivo invalido e erro
/// com o motivo; e a linha de verdade da tarefa e o que as regras de comando conferem,
/// pelo mesmo `linha_para_as_regras` do portao do motor.
#[tokio::test]
async fn project_task_roda_a_tarefa_definida_e_a_regra_nega_o_comando() {
    let Some(b) = bwrap() else { return };
    let d = tmp("tarefas");
    let c = ctx_em(&d);
    std::fs::create_dir_all(d.join(".phxclaw")).unwrap();
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(
        d.join(".phxclaw/tarefas.json"),
        r#"[{"nome":"ola","comando":"echo","args":["ola mundo"],"grupo":"run"},
            {"nome":"falha","comando":"sh","args":["-c","pwd; exit 3"],"cwd":"sub","grupo":"test"}]"#,
    )
    .unwrap();
    let t = ProjectTaskTool {
        bwrap: b.clone(),
        timeout: Duration::from_secs(30),
        tarefas: carregar(&d),
    };
    let l = rodar(&t, &c, json!({"action":"list"})).await;
    assert_eq!(l["tarefas"].as_array().unwrap().len(), 2, "{l}");
    assert!(l["padrao"].is_null(), "{l}");

    let r = rodar(&t, &c, json!({"action":"run","name":"ola"})).await;
    assert_eq!(r["exit_code"], 0, "{r}");
    assert_eq!(r["stdout"], "ola mundo\n", "{r}");
    assert_eq!(r["comando"], "'echo' 'ola mundo'", "{r}");
    let r = rodar(&t, &c, json!({"action":"run","name":"falha"})).await;
    assert_eq!(r["exit_code"], 3, "{r}");
    assert_eq!(r["stdout"], "/work/sub\n", "{r}");
    let e = erro(&t, &c, json!({"action":"run","name":"nada"})).await;
    assert!(
        e.to_string().contains("nao esta em .phxclaw/tarefas.json"),
        "{e}"
    );

    // A regra ve `echo`, nao `project_task`: negar `echo` nega a tarefa.
    let regras =
        RegrasDeComando::de_json(r#"{"regras":[{"padrao":"echo *","decisao":"negar"}]}"#).unwrap();
    let linha = linha_para_as_regras(&t, &json!({"action":"run","name":"ola"})).unwrap();
    assert_eq!(linha, "'echo' 'ola mundo'");
    assert_eq!(regras.avaliar(&linha).decisao, Decisao::Negar);
    assert_eq!(
        regras
            .avaliar(&linha_para_as_regras(&t, &json!({"action":"run","name":"falha"})).unwrap())
            .decisao,
        Decisao::Permitir
    );
    // `list` nao roda comando: cai no `<nome> <action>` que o portao usa para toda
    // ferramenta que executa, como o `shell_bg status`.
    assert_eq!(
        linha_para_as_regras(&t, &json!({"action":"list"})).as_deref(),
        Some("project_task 'list'")
    );

    // Sem arquivo: lista vazia e o padrao nomeado. Arquivo invalido: erro com o motivo.
    let vazio = tmp("sem-tarefas");
    assert_eq!(carregar(&vazio).unwrap(), vec![]);
    let sem = ProjectTaskTool {
        bwrap: b,
        timeout: Duration::from_secs(30),
        tarefas: carregar(&vazio),
    };
    let l = rodar(&sem, &ctx_em(&vazio), json!({"action":"list"})).await;
    assert_eq!(
        l["padrao"],
        json!(["rust_project", "python_project"]),
        "{l}"
    );
    std::fs::write(
        d.join(".phxclaw/tarefas.json"),
        r#"[{"nome":"x","comando":"ls","grupo":"deploy"}]"#,
    )
    .unwrap();
    let e = carregar(&d).unwrap_err();
    assert!(e.contains("grupo \"deploy\""), "{e}");
    // A ferramenta resolveu a lista UMA vez: o arquivo trocado depois nao muda a linha que
    // as regras veem nem a que roda (as duas saem da mesma lista).
    let linha = linha_para_as_regras(&t, &json!({"action":"run","name":"ola"})).unwrap();
    assert_eq!(linha, "'echo' 'ola mundo'");
    let r = rodar(&t, &c, json!({"action":"run","name":"ola"})).await;
    assert_eq!(r["stdout"], "ola mundo\n", "{r}");
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&vazio);
}

// ------------------------------------------------------------------ 5. historico de gravacoes

/// Tres gravacoes vistas pelo poller sao tres versoes; o restore devolve a segunda e
/// guarda a atual antes; o teto poda a mais antiga; e a ferramenta le o mesmo historico.
#[tokio::test]
async fn historico_guarda_tres_versoes_e_restaura_a_segunda() {
    let d = tmp("historico");
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(d.join("src/a.txt"), "zero\n").unwrap();
    let mut h = Historico::novo(d.clone(), 50);
    // A primeira varredura so lembra: o que ja estava la nao e gravacao do usuario.
    assert!(h.varrer().is_empty());
    // Tamanhos diferentes de proposito: mtime e tamanho sao o criterio da mudanca, e o
    // relogio do kernel pode empatar duas gravacoes no mesmo tique.
    for v in ["um\n", "dois\n", "tres\n"] {
        std::fs::write(d.join("src/a.txt"), v).unwrap();
        assert_eq!(h.varrer(), vec!["src/a.txt".to_string()]);
    }
    let versoes = h.versoes("src/a.txt").unwrap();
    assert_eq!(versoes.len(), 3, "{versoes:?}");
    assert!(versoes.windows(2).all(|w| w[0].rowstamp < w[1].rowstamp));
    assert_eq!(h.ler("src/a.txt", versoes[1].rowstamp).unwrap(), b"dois\n");

    let guardada = h.restaurar("src/a.txt", versoes[1].rowstamp).unwrap();
    assert_eq!(
        std::fs::read_to_string(d.join("src/a.txt")).unwrap(),
        "dois\n"
    );
    assert!(guardada.is_some());
    assert_eq!(h.versoes("src/a.txt").unwrap().len(), 4);
    // O restore nao e gravacao do usuario: a varredura seguinte nao guarda de novo.
    assert!(h.varrer().is_empty());
    // Fora de .phxclaw e sem `..`.
    assert!(h.versoes("../x").is_err());
    assert!(h.versoes(".phxclaw/historico/x").is_err());

    // Teto de 2: a terceira gravacao poda a primeira.
    let mut curto = Historico::novo(d.clone(), 2);
    curto.varrer();
    for v in ["a\n", "bb\n", "ccc\n"] {
        std::fs::write(d.join("src/b.txt"), v).unwrap();
        curto.varrer();
    }
    let vb = curto.versoes("src/b.txt").unwrap();
    assert_eq!(vb.len(), 2, "{vb:?}");
    assert_eq!(curto.ler("src/b.txt", vb[0].rowstamp).unwrap(), b"bb\n");

    // A ferramenta: list, show e restore sobre a mesma pasta.
    let c = ctx_em(&d);
    let (ler, esc) = (
        FileHistoryTool { escrita: false },
        FileHistoryTool { escrita: true },
    );
    assert_eq!(
        (ler.capability(), esc.capability()),
        ("fs.read", "fs.write")
    );
    let l = rodar(&ler, &c, json!({"action":"list","path":"src/a.txt"})).await;
    assert_eq!(l["versoes"].as_array().unwrap().len(), 4, "{l}");
    let primeira = l["versoes"][0]["rowstamp"].as_u64().unwrap();
    let s = rodar(
        &ler,
        &c,
        json!({"action":"show","path":"src/a.txt","version":primeira}),
    )
    .await;
    assert_eq!(s["texto"], "um\n", "{s}");
    let r = rodar(&esc, &c, json!({"path":"src/a.txt","version":primeira})).await;
    assert_eq!(r["restaurada"], primeira, "{r}");
    assert_eq!(
        std::fs::read_to_string(d.join("src/a.txt")).unwrap(),
        "um\n"
    );
    let e = erro(&esc, &c, json!({"path":"src/a.txt","version":1})).await;
    assert!(e.to_string().contains("nao ha versao 1"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}
