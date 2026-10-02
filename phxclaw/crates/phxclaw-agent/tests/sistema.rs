//! As quatro ferramentas da maquina Linux contra o sistema de verdade: processo morto
//! conferido pelo `wait` do pai, porta aberta por um `TcpListener` daqui, o PostgreSQL
//! desta maquina recusando a escrita pela transacao READ ONLY, e o cargo dentro do bwrap
//! devolvendo o erro de tipo na linha certa.

use phxclaw_agent::sistema::*;
use phxclaw_agent::*;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use phxclaw_test_support::pulado;
use serde_json::{Value, json};
use std::os::unix::process::ExitStatusExt;
use std::sync::Arc;
use std::time::Duration;

fn ctx() -> ToolContext {
    let d = std::env::temp_dir().join(format!("phx-sis-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    ToolContext {
        task_id: "t".into(),
        workdir: d,
        timeout: Duration::from_secs(60),
    }
}

async fn roda(t: &dyn Tool, args: Value) -> Result<String, ToolError> {
    t.run(args, &ctx()).await.map(|o| o.content)
}

// ---------------------------------------------------------------- linux_system

#[tokio::test]
async fn sistema_le_painel_processos_e_unidade_validada() {
    let t = LinuxSystemTool::leitura();
    assert_eq!(t.capability(), "system.read");
    let k = roda(&t, json!({"action":"panel","item":"kernel"}))
        .await
        .unwrap();
    assert!(k.contains("Linux"), "{k}");
    let tudo = roda(&t, json!({"action":"panel"})).await.unwrap();
    assert!(
        tudo.contains("== memory ==") && tudo.contains("Mem"),
        "{tudo}"
    );

    let mut filho = std::process::Command::new("sleep")
        .arg("37.5")
        .spawn()
        .unwrap();
    let p = roda(&t, json!({"action":"processes","filter":"sleep 37.5"}))
        .await
        .unwrap();
    let _ = filho.kill();
    let _ = filho.wait();
    assert!(p.contains(&filho.id().to_string()), "{p}");

    // Texto de shell no nome da unidade nao chega ao systemctl.
    let r = roda(&t, json!({"action":"service_status","unit":"x; reboot"})).await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
    // A ferramenta de leitura nao tem acao de mudanca.
    let r = roda(&t, json!({"action":"kill","pid":12345})).await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
}

#[tokio::test]
async fn sem_systemd_a_acao_diz_isso_em_vez_de_erro_cru() {
    if systemd_rodando() {
        pulado::pular(
            "sem-systemd",
            "systemd rodando aqui: o caso do conteiner nao se prova",
        );
        return;
    }
    let s = roda(&LinuxSystemTool::leitura(), json!({"action":"services"}))
        .await
        .unwrap();
    assert!(s.contains("systemd nao esta rodando aqui"), "{s}");
    let s = roda(
        &LinuxSystemTool::admin(),
        json!({"action":"service","operation":"restart","unit":"ssh.service"}),
    )
    .await
    .unwrap();
    assert!(s.contains("systemd nao esta rodando aqui"), "{s}");
}

#[tokio::test]
async fn admin_mata_o_pid_e_recusa_o_init() {
    let t = LinuxSystemTool::admin();
    assert_eq!(t.capability(), "system.admin");
    let mut filho = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let r = roda(&t, json!({"action":"kill","pid":filho.id()}))
        .await
        .unwrap();
    assert!(r.contains("TERM"), "{r}");
    // Conferido pelo pai, nao pela palavra da ferramenta.
    assert_eq!(filho.wait().unwrap().signal(), Some(15));

    let r = roda(&t, json!({"action":"kill","pid":1})).await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");
    let r = roda(&t, json!({"action":"kill","pid":"1 2"})).await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
}

/// O principal: sem `system.admin`, o motor nem deixa a ferramenta de mudanca rodar,
/// mesmo o modelo pedindo pelo nome. O processo sobrevive.
#[tokio::test]
async fn sem_system_admin_o_motor_nega_o_kill() {
    let mut filho = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c1",
            "linux_system_admin",
            json!({"action":"kill","pid":filho.id()}),
        ),
        ScriptedLlm::text("fim"),
    ]));
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(LinuxSystemTool::leitura()),
        Arc::new(LinuxSystemTool::admin()),
    ];
    let store = TaskStore::new(
        std::env::temp_dir().join(format!("phx-sis-m-{}", phxclaw_types::new_uuid_v7())),
    )
    .unwrap();
    let a = Agent::new(
        llm.clone(),
        tools,
        AgentConfig::default().grant(&["system.read"]),
        store,
    );
    a.run(
        Task::new("mate", "roteiro"),
        &CancelFlag::default(),
        &NoObserver,
    )
    .await;
    let vistos = llm.seen.lock().unwrap();
    // A ferramenta negada nem aparece para o modelo...
    assert!(!vistos[0].1.contains(&"linux_system_admin".to_string()));
    // ...e pedida pelo nome, volta negada.
    let r = &vistos[1].0.last().unwrap().content;
    assert!(r.contains("NEGADO") && r.contains("system.admin"), "{r}");
    assert!(
        filho.try_wait().unwrap().is_none(),
        "o processo morreu sem system.admin"
    );
    let _ = filho.kill();
    let _ = filho.wait();
}

// ---------------------------------------------------------------- network

#[tokio::test]
async fn sonda_lan_ve_porta_aberta_e_fechada() {
    let lan = NetworkTool::new(true, &[]);
    assert_eq!(lan.capability(), "net.lan");
    let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let aberta = ouvinte.local_addr().unwrap().port();
    let fechada = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let r = roda(
        &lan,
        json!({"action":"tcp_probe","host":"127.0.0.1","port":aberta}),
    )
    .await
    .unwrap();
    assert!(r.contains("aberta") && r.contains(" ms"), "{r}");
    let r = roda(
        &lan,
        json!({"action":"tcp_probe","host":"127.0.0.1","port":fechada}),
    )
    .await
    .unwrap();
    assert!(r.contains("recusada"), "{r}");
    // A LAN nao alcanca destino publico.
    let r = roda(
        &lan,
        json!({"action":"tcp_probe","host":"1.1.1.1","port":443}),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Denied(_))), "{r:?}");

    // A porta aparece em escuta, com o PID deste processo.
    let net = NetworkTool::new(false, &[]);
    let l = roda(&net, json!({"action":"listening"})).await.unwrap();
    let linha = l
        .lines()
        .find(|x| x.starts_with("tcp\t127.0.0.1\t") && x.contains(&format!("\t{aberta}\t")))
        .unwrap_or_else(|| panic!("porta {aberta} fora da lista:\n{l}"));
    assert!(linha.contains(&std::process::id().to_string()), "{linha}");
    drop(ouvinte);
}

/// O principal: a `network` (net.diagnose) nao toca loopback nem rede privada, e
/// destino publico so com a politica de egresso liberando.
#[tokio::test]
async fn sonda_publica_recusa_interno_e_passa_pelo_broker() {
    let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let net = NetworkTool::new(false, &[]);
    assert_eq!(net.capability(), "net.diagnose");
    for host in ["127.0.0.1", "localhost", "10.0.0.1", "192.168.0.1", "[::1]"] {
        let r = roda(
            &net,
            json!({"action":"tcp_probe","host":host,"port":porta,"timeout_ms":200}),
        )
        .await;
        match r {
            Err(ToolError::Denied(m)) => assert!(m.contains("net.lan"), "{m}"),
            outro => panic!("{host}: {outro:?}"),
        }
    }
    // Publico fora da lista do broker: negado, dizendo onde liberar.
    let r = roda(
        &net,
        json!({"action":"tcp_probe","host":"1.1.1.1","port":9,"timeout_ms":200}),
    )
    .await;
    match r {
        Err(ToolError::Denied(m)) => assert!(m.contains(NET_DESTINOS_ENV), "{m}"),
        outro => panic!("{outro:?}"),
    }
    // Na lista: a sonda roda (o resultado depende da rede desta maquina, a decisao nao).
    let liberada = NetworkTool::new(false, &["1.1.1.1:9".into()]);
    let r = roda(
        &liberada,
        json!({"action":"tcp_probe","host":"1.1.1.1","port":9,"timeout_ms":200}),
    )
    .await
    .unwrap();
    assert!(r.starts_with("1.1.1.1:9\t"), "{r}");
    // Texto de shell no host nao vira argumento de nada.
    let r = roda(&net, json!({"action":"ping","host":"x; reboot"})).await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
}

#[tokio::test]
async fn rede_lista_interfaces_rotas_e_resolve() {
    let net = NetworkTool::new(false, &[]);
    let i = roda(&net, json!({"action":"interfaces"})).await.unwrap();
    assert!(i.contains("lo\t") && i.contains("127.0.0.1"), "{i}");
    let r = roda(&net, json!({"action":"routes"})).await.unwrap();
    assert!(!r.is_empty());
    let d = roda(&net, json!({"action":"resolve","host":"localhost"}))
        .await
        .unwrap();
    assert!(d.contains("interno") && d.contains(" ms"), "{d}");
}

// ---------------------------------------------------------------- postgres

const SENHA: &str = "senha-que-nunca-sai-7731";

fn pg_url() -> Option<String> {
    let u = format!("host=/tmp port=55432 user=postgres dbname=postgres password={SENHA}");
    let ok = std::process::Command::new("psql")
        .args([
            "-h", "/tmp", "-p", "55432", "-U", "postgres", "-Atc", "select 1",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then_some(u)
}

/// O principal: o INSERT pela ferramenta de leitura e recusado pelo PROPRIO banco
/// (transacao READ ONLY, SQLSTATE 25006), e nada entra na tabela.
#[tokio::test]
async fn postgres_leitura_recusa_escrita_pelo_banco() {
    let Some(url) = pg_url() else {
        // O recurso e o SERVIDOR, nao o `psql`: a maquina com o binario e sem o servidor nao
        // TEM o que o teste precisa (o portao conferia o binario e dava NoGo errado).
        pulado::pular("postgres:/tmp:55432", "PostgreSQL em /tmp:55432 parado");
        return;
    };
    let ler = PostgresTool::new(false, url.clone());
    let gravar = PostgresTool::new(true, url);
    assert_eq!(
        (ler.capability(), gravar.capability()),
        ("db.read", "db.write")
    );
    let esq = format!("phx_t_{}", std::process::id());
    let mut saidas = Vec::new();
    saidas.push(
        roda(
            &gravar,
            json!({"action":"execute","sql":format!(
                "CREATE SCHEMA {esq} CREATE TABLE conta (id int PRIMARY KEY, valor numeric(10,2) NOT NULL, criada timestamptz DEFAULT now())"
            )}),
        )
        .await
        .unwrap(),
    );
    let tabela = format!("{esq}.conta");

    // Escrita pela leitura: o banco recusa.
    let r = roda(
        &ler,
        json!({"action":"query","sql":format!("INSERT INTO {tabela} (id, valor) VALUES (1, 9.99)")}),
    )
    .await;
    match &r {
        Err(ToolError::Failed(m)) => {
            assert!(m.contains("25006") && m.contains("read-only"), "{m}")
        }
        outro => panic!("INSERT pela leitura nao foi recusado: {outro:?}"),
    }
    // Duas instrucoes numa chamada nao escapam da transacao.
    let r = roda(
        &ler,
        json!({"action":"query","sql":format!("SELECT 1; COMMIT; INSERT INTO {tabela} (id, valor) VALUES (2, 1)")}),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Failed(_))), "{r:?}");
    let n = roda(
        &ler,
        json!({"action":"query","sql":format!("SELECT count(*) AS n FROM {tabela}")}),
    )
    .await
    .unwrap();
    assert!(n.starts_with("n\n0\n"), "entrou linha pela leitura: {n}");

    // Escrita pela de escrita: entra, e a leitura ve.
    saidas.push(
        roda(
            &gravar,
            json!({"action":"execute","sql":format!("INSERT INTO {tabela} (id, valor) VALUES (1, 9.99), (2, -0.5), (3, 1000)")}),
        )
        .await
        .unwrap(),
    );
    assert!(saidas[1].contains("3 linhas afetadas"), "{}", saidas[1]);
    let q = roda(
        &ler,
        json!({"action":"query","sql":format!("SELECT sum(valor) AS s, count(*) AS n, bool_and(valor > -1) AS b FROM {tabela}"),"max_rows":5}),
    )
    .await
    .unwrap();
    assert!(q.contains("1009.49\t3\ttrue"), "{q}");
    let teto = roda(
        &ler,
        json!({"action":"query","sql":format!("SELECT id FROM {tabela} ORDER BY id"),"max_rows":2}),
    )
    .await
    .unwrap();
    assert!(teto.contains("teto 2 atingido"), "{teto}");
    saidas.push(teto);

    let s = roda(&ler, json!({"action":"schemas"})).await.unwrap();
    assert!(s.contains(&esq), "{s}");
    let t = roda(&ler, json!({"action":"tables","schema":esq}))
        .await
        .unwrap();
    assert!(t.contains("conta\ttabela"), "{t}");
    let d = roda(
        &ler,
        json!({"action":"describe","schema":esq,"table":"conta"}),
    )
    .await
    .unwrap();
    assert!(
        d.contains("valor\tnumeric(10,2)\tfalse") && d.contains("PRIMARY KEY (id)"),
        "{d}"
    );
    let e = roda(
        &ler,
        json!({"action":"explain","sql":format!("SELECT * FROM {tabela} WHERE id = 1")}),
    )
    .await
    .unwrap();
    assert!(e.contains("QUERY PLAN"), "{e}");
    // ANALYZE escondido no texto nao passa pela leitura.
    let r = roda(
        &ler,
        json!({"action":"explain","sql":format!("ANALYZE SELECT * FROM {tabela}")}),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Failed(_))), "{r:?}");
    let ea = roda(
        &gravar,
        json!({"action":"explain_analyze","sql":format!("DELETE FROM {tabela}")}),
    )
    .await
    .unwrap();
    assert!(
        ea.contains("actual time") && ea.contains("ROLLBACK"),
        "{ea}"
    );
    let n = roda(
        &ler,
        json!({"action":"query","sql":format!("SELECT count(*) AS n FROM {tabela}")}),
    )
    .await
    .unwrap();
    assert!(n.starts_with("n\n3\n"), "o EXPLAIN ANALYZE apagou: {n}");
    saidas.extend([s, t, d, e, ea]);

    let _ = roda(
        &gravar,
        json!({"action":"execute","sql":format!("DROP SCHEMA {esq} CASCADE")}),
    )
    .await;
    for s in &saidas {
        assert!(!s.contains(SENHA), "a senha vazou: {s}");
    }
}

#[tokio::test]
async fn postgres_erro_de_conexao_nao_mostra_a_senha() {
    let t = PostgresTool::new(
        false,
        format!("postgresql://postgres:{SENHA}@127.0.0.1:1/postgres"),
    );
    let r = roda(&t, json!({"action":"schemas"})).await;
    match r {
        Err(ToolError::Failed(m)) => {
            assert!(m.contains("conexao") && !m.contains(SENHA), "{m}")
        }
        outro => panic!("{outro:?}"),
    }
    let t = PostgresTool::new(false, format!("postgresql://x:{SENHA}@[nao fecha/db"));
    match roda(&t, json!({"action":"schemas"})).await {
        Err(ToolError::Failed(m)) => assert!(!m.contains(SENHA), "{m}"),
        outro => panic!("{outro:?}"),
    }
}

// ---------------------------------------------------------------- rust_project

fn rust_tool() -> Option<RustProjectTool> {
    let bwrap = ["/usr/bin/bwrap", "/bin/bwrap"]
        .iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists())?;
    RustProjectTool::detectar(bwrap)
}

/// O principal: um erro de tipo volta como diagnostico com arquivo, linha, nivel e
/// codigo -- nao como texto de terminal.
#[tokio::test]
async fn cargo_check_devolve_o_erro_de_tipo_na_linha_certa() {
    let Some(t) = rust_tool() else {
        pulado::pular("cargo", "bwrap ou toolchain ausente");
        return;
    };
    assert_eq!(t.capability(), "shell.exec");
    let c = ctx();
    let p = c.workdir.join("proj");
    std::fs::create_dir_all(p.join("src")).unwrap();
    std::fs::write(
        p.join("Cargo.toml"),
        "[package]\nname = \"proj\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        p.join("src/main.rs"),
        "fn main() {\n    let total = 0;\n    let x: i32 = \"texto\";\n    println!(\"{x}{total}\");\n}\n",
    )
    .unwrap();
    let r = t
        .run(json!({"action":"check","path":"proj"}), &c)
        .await
        .unwrap()
        .content;
    let v: Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v["sucesso"], false, "{r}");
    let d = v["diagnosticos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["nivel"] == "error" && d["codigo"] == "E0308")
        .unwrap_or_else(|| panic!("sem E0308: {r}"));
    assert_eq!(d["arquivo"], "src/main.rs", "{r}");
    assert_eq!(d["linha"], 3, "{r}");
    assert_eq!(d["coluna"], 18, "{r}");

    // Consertado, o mesmo check passa limpo.
    std::fs::write(
        p.join("src/main.rs"),
        "fn main() {\n    let x: i32 = 7;\n    println!(\"{x}\");\n}\n",
    )
    .unwrap();
    let r = t
        .run(json!({"action":"check","path":"/work/proj"}), &c)
        .await
        .unwrap()
        .content;
    let v: Value = serde_json::from_str(&r).unwrap();
    assert_eq!(v["sucesso"], true, "{r}");
    assert_eq!(v["erros"], 0, "{r}");
}

#[tokio::test]
async fn cargo_recusa_caminho_fora_e_com_shell() {
    let Some(t) = rust_tool() else {
        pulado::pular("cargo", "bwrap ou toolchain ausente");
        return;
    };
    for (path, esperado) in [
        ("../fora", "fora"),
        ("proj'; reboot; '", "invalido"),
        ("nada", "Cargo.toml"),
    ] {
        let r = t.run(json!({"action":"check","path":path}), &ctx()).await;
        match r {
            Err(ToolError::Denied(m)) | Err(ToolError::InvalidArguments(m)) => {
                assert!(m.contains(esperado), "{path}: {m}")
            }
            outro => panic!("{path}: {outro:?}"),
        }
    }
    let c = ctx();
    std::fs::write(c.workdir.join("Cargo.toml"), "").unwrap();
    let r = t.run(json!({"action":"publish"}), &c).await;
    assert!(matches!(r, Err(ToolError::InvalidArguments(_))), "{r:?}");
}
