//! shell_bg contra o sistema operacional de verdade: o processo roda no bwrap, a saida
//! chega ao arquivo, o stop mata de fato (conferido pelo /proc, nao pela palavra da
//! ferramenta), e os dois tetos seguram.

use phxclaw_agent::*;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::json;
use std::time::Duration;

fn bwrap() -> Option<std::path::PathBuf> {
    ["/usr/bin/bwrap", "/bin/bwrap"]
        .iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists())
}

fn ctx() -> ToolContext {
    let d = std::env::temp_dir().join(format!("phx-bg-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    ToolContext {
        task_id: "t".into(),
        workdir: d,
        timeout: Duration::from_secs(5),
    }
}

/// Processos cuja linha de comando contem a marca (o sleep dentro do sandbox).
fn vivos_com(marca: &str) -> usize {
    std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|e| std::fs::read(e.path().join("cmdline")).ok())
        .filter(|c| String::from_utf8_lossy(c).contains(marca))
        .count()
}

#[tokio::test]
async fn start_status_stop_mata_de_verdade() {
    let Some(b) = bwrap() else {
        eprintln!("sem bwrap: pulado");
        return;
    };
    let t = BackgroundShellTool::new(b, false);
    let c = ctx();
    let marca = format!("{}", 3000 + std::process::id() % 1000);
    let r = t
        .run(
            json!({"action":"start","command":format!("echo comecou; sleep {marca}.5")}),
            &c,
        )
        .await
        .unwrap();
    assert!(r.content.contains("id 1"), "{}", r.content);
    tokio::time::sleep(Duration::from_millis(400)).await;
    let s = t.run(json!({"action":"status","id":1}), &c).await.unwrap();
    assert!(
        s.content.contains("rodando") && s.content.contains("comecou"),
        "{}",
        s.content
    );
    assert!(
        vivos_com(&format!("{marca}.5")) >= 1,
        "o sleep devia estar vivo"
    );
    let s = t.run(json!({"action":"stop","id":1}), &c).await.unwrap();
    assert!(s.content.contains("terminou"), "{}", s.content);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        vivos_com(&format!("{marca}.5")),
        0,
        "stop deixou o processo vivo"
    );
}

#[tokio::test]
async fn teto_de_processos_e_vida_maxima() {
    let Some(b) = bwrap() else {
        return;
    };
    let mut t = BackgroundShellTool::new(b, false);
    t.max_por_tarefa = 2;
    t.vida_max = Duration::from_millis(700);
    let c = ctx();
    for _ in 0..2 {
        t.run(json!({"action":"start","command":"sleep 30"}), &c)
            .await
            .unwrap();
    }
    let e = t
        .run(json!({"action":"start","command":"sleep 30"}), &c)
        .await
        .unwrap_err();
    assert!(matches!(e, ToolError::Denied(_)), "{e}");
    // o vigia mata os dois sem ninguem perguntar
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let l = t.run(json!({"action":"list"}), &c).await.unwrap();
    assert!(!l.content.contains("rodando"), "{}", l.content);
    // e o teto libera
    t.run(json!({"action":"start","command":"true"}), &c)
        .await
        .unwrap();
}

#[tokio::test]
async fn saida_fica_no_arquivo_e_codigo_de_saida_aparece() {
    let Some(b) = bwrap() else {
        return;
    };
    let t = BackgroundShellTool::new(b, false);
    let c = ctx();
    t.run(
        json!({"action":"start","command":"echo erro >&2; echo ok; exit 3"}),
        &c,
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let s = t.run(json!({"action":"status","id":1}), &c).await.unwrap();
    assert!(s.content.contains("terminou com 3"), "{}", s.content);
    let log = std::fs::read_to_string(c.workdir.join(".bg/1.log")).unwrap();
    assert!(log.contains("ok") && log.contains("erro"), "{log}");
}
