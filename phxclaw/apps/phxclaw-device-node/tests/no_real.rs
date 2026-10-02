//! O binario real do no contra o servidor real e a ferramenta do agente, na mesma maquina:
//! TLS com CA propria, chave do no em arquivo 0600, pareamento por token. O que se prova
//! e a regra das TRES listas do `node_invoke`: so passa a capacidade declarada pelo no,
//! aprovada pelo operador no pareamento E concedida ao agente -- faltando qualquer uma, o
//! comando e negado antes de sair pelo fio.

use phxclaw_agent::dispositivos::DeviceTool;
use phxclaw_agent_core::{ToolContext, ToolError};
use phxclaw_device_transport::servidor::{RegistroMemoria, ServidorDispositivos, tls_de_pem};
use phxclaw_test_support::pulado;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const TOKEN: &str = "token-do-no-real-com-mais-de-24-caracteres";

fn certificado(d: &std::path::Path) -> Option<(Vec<u8>, Vec<u8>)> {
    let sh = |args: &[&str]| {
        Command::new("openssl")
            .args(args)
            .current_dir(d)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    std::fs::write(
        d.join("ext.cnf"),
        "basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost\nextendedKeyUsage=serverAuth\n",
    )
    .ok()?;
    let ok = sh(&[
        "req",
        "-x509",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:P-256",
        "-nodes",
        "-days",
        "1",
        "-subj",
        "/CN=CA no real",
        "-keyout",
        "ca.key",
        "-out",
        "ca.pem",
    ]) && sh(&[
        "req",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:P-256",
        "-nodes",
        "-subj",
        "/CN=localhost",
        "-keyout",
        "srv.key",
        "-out",
        "srv.csr",
    ]) && sh(&[
        "x509",
        "-req",
        "-in",
        "srv.csr",
        "-CA",
        "ca.pem",
        "-CAkey",
        "ca.key",
        "-CAcreateserial",
        "-days",
        "1",
        "-extfile",
        "ext.cnf",
        "-out",
        "srv.pem",
    ]);
    ok.then(|| {
        let l = |n: &str| std::fs::read(d.join(n)).unwrap();
        (l("srv.pem"), l("srv.key"))
    })
}

#[tokio::test]
async fn node_invoke_so_passa_pelas_tres_listas_contra_o_no_real() {
    let d = std::env::temp_dir().join(format!("phx-no-real-{}", Uuid::now_v7()));
    std::fs::create_dir_all(d.join("chaves")).unwrap();
    let Some((cert, chave)) = certificado(&d) else {
        pulado::pular("openssl", "sem openssl nao ha certificado para o no");
        return;
    };
    let tenant = Uuid::now_v7();
    let reg = RegistroMemoria::default();
    // aprovado no pareamento: info (vai passar), files.read (o agente nao tem) e
    // camera (o no nao declara). screen.capture o no declara e o agente tem, mas o
    // operador nao aprovou.
    reg.emitir_token_aprovando(
        tenant,
        TOKEN,
        &[
            "device.system.info",
            "device.files.read",
            "device.camera.shoot",
        ],
    );
    let srv = Arc::new(ServidorDispositivos::novo(Arc::new(Mutex::new(reg))).unwrap());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = l.local_addr().unwrap().port();
    tokio::spawn(srv.clone().servir(l, tls_de_pem(&cert, &chave).unwrap()));

    let node = Uuid::now_v7();
    let mut filho = tokio::process::Command::new(env!("CARGO_BIN_EXE_phxclaw-device-node"))
        .env_clear()
        .env(
            "PHXCLAW_DEVICE_WSS_URL",
            format!("wss://localhost:{porta}/"),
        )
        .env("PHXCLAW_TENANT_UUID", tenant.to_string())
        .env("PHXCLAW_NODE_UUID", node.to_string())
        .env("PHXCLAW_ENROLLMENT_TOKEN", TOKEN)
        .env("PHXCLAW_DEVICE_CA_PEM", d.join("ca.pem"))
        .env(
            "PHXCLAW_DEVICE_KEYSTORE",
            format!("arquivo:{}", d.join("chaves").display()),
        )
        .env("HOSTNAME", "no-real-do-teste")
        .kill_on_drop(true)
        .spawn()
        .unwrap();

    let mut pronto = false;
    for _ in 0..250 {
        if srv
            .nos()
            .iter()
            .any(|n| n.conectado && format!("{:?}", n.no.state) == "Active")
        {
            pronto = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(pronto, "o no real nao pareou: {:?}", filho.try_wait());

    let politica: BTreeSet<String> = [
        "device.read",
        "device.command",
        "device.system.info",
        "device.screen.capture",
        "device.camera.shoot",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let [listar, invocar] = DeviceTool::par(srv.clone(), &politica);
    let ctx = ToolContext {
        task_id: "t".into(),
        workdir: d.clone(),
        timeout: Duration::from_secs(30),
    };
    let v: Value =
        serde_json::from_str(&listar.run(json!({}), &ctx).await.unwrap().content).unwrap();
    assert_eq!(v["nos"][0]["node"], json!(node));
    assert_eq!(v["nos"][0]["invocaveis"], json!(["device.system.info"]));

    let ok = invocar
        .run(
            json!({"node": node.to_string(), "capability": "device.system.info"}),
            &ctx,
        )
        .await
        .unwrap();
    let v: Value = serde_json::from_str(&ok.content).unwrap();
    assert_eq!(v["saida"]["hostname"], "no-real-do-teste", "{v}");
    assert_eq!(v["saida"]["os"], std::env::consts::OS);

    for (cap, falta) in [
        ("device.screen.capture", "pareamento"),
        ("device.files.read", "concedida ao agente"),
        ("device.camera.shoot", "not declared"),
    ] {
        let e = invocar
            .run(json!({"node": node.to_string(), "capability": cap}), &ctx)
            .await
            .unwrap_err();
        assert!(matches!(e, ToolError::Denied(_)), "{cap}: {e}");
        assert!(e.to_string().contains(falta), "{cap}: {e}");
    }
    // o no continua vivo e atende depois das recusas (nada negado chegou a ele)
    assert!(
        invocar
            .run(
                json!({"node": node.to_string(), "capability": "device.system.info"}),
                &ctx,
            )
            .await
            .is_ok()
    );
    let _ = filho.kill().await;
    let _ = std::fs::remove_dir_all(&d);
}
