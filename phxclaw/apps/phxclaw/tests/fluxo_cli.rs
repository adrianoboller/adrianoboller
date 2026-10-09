//! `phxclaw fluxo pinar|exportar|importar` pelo processo de verdade: o teste
//! `cli_importar_exportar` do agente prova as funcoes do motor; este prova que a CLI e porta
//! fina para elas -- os argumentos chegam, a saida diz o que fez, e a recusa sai com codigo
//! diferente de zero e o motivo.

use std::path::{Path, PathBuf};
use std::process::Command;

fn pasta(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-cli-fluxo-{nome}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn phxclaw(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args(args)
        .args(["--pasta", dir.join("agente").to_str().unwrap()])
        .env_remove("PHXCLAW_HOME")
        .current_dir(dir)
        .output()
        .unwrap()
}

fn texto(o: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn pinar_exportar_importar_pela_cli() {
    let d = pasta("pacote");
    let arq = d.join("origem.json");
    std::fs::write(
        &arq,
        r#"{"nome":"origem","etiquetas":["vendas"],"passos":[
            {"id":"a","ferramenta":"eco","args":{"texto":"x"}},
            {"id":"b","depende":["a"],"ferramenta":"eco","args":{"texto":"{{a}}!"}}]}"#,
    )
    .unwrap();
    let a = arq.to_str().unwrap();
    let o = phxclaw(&d, &["fluxo", "pinar", a, "a", "--json", r#"["PIN"]"#]);
    assert!(o.status.success(), "{}", texto(&o));
    assert!(texto(&o).contains("pinado a"), "{}", texto(&o));
    let pacote = d.join("pacote.json");
    let o = phxclaw(
        &d,
        &["fluxo", "exportar", a, "--saida", pacote.to_str().unwrap()],
    );
    assert!(o.status.success(), "{}", texto(&o));
    let destino = d.join("copia").join("importado.json");
    let imp = |p: &Path| {
        phxclaw(
            &d,
            &[
                "fluxo",
                "importar",
                p.to_str().unwrap(),
                destino.to_str().unwrap(),
            ],
        )
    };
    let o = imp(&pacote);
    assert!(o.status.success(), "{}", texto(&o));
    assert!(destino.is_file());
    assert!(d.join("copia").join("importado.pins.json").is_file());
    // de novo: nao sobrescreve
    let o = imp(&pacote);
    assert!(!o.status.success());
    assert!(texto(&o).contains("ja existe"), "{}", texto(&o));
    // pacote editado no caminho: a conferencia (sha256) recusa
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pacote).unwrap()).unwrap();
    v["fluxo"]["passos"][0]["pin"] = serde_json::json!(["OUTRO"]);
    let torto = d.join("torto.json");
    std::fs::write(&torto, v.to_string()).unwrap();
    std::fs::remove_file(&destino).unwrap();
    std::fs::remove_file(d.join("copia").join("importado.pins.json")).unwrap();
    let o = imp(&torto);
    assert!(!o.status.success());
    assert!(texto(&o).contains("conferencia"), "{}", texto(&o));
    assert!(!destino.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// A galeria, o rascunho x publicada e o git dos fluxos pela CLI de verdade: os argumentos
/// chegam ao motor, a saida diz o que fez e a recusa sai com codigo diferente de zero.
#[test]
fn modelos_versoes_e_git_pela_cli() {
    let d = pasta("f3");
    let o = phxclaw(&d, &["fluxo", "modelos"]);
    assert!(o.status.success(), "{}", texto(&o));
    let t = texto(&o);
    assert!(
        t.contains("ponte-n8n") && t.contains("credenciais: n8n"),
        "{t}"
    );
    assert!(t.contains("12 modelo(s)"), "{t}");

    let fluxos = d.join("fluxos");
    std::fs::create_dir_all(&fluxos).unwrap();
    let meu = fluxos.join("meu.json");
    let m = meu.to_str().unwrap();
    let o = phxclaw(&d, &["fluxo", "usar", "triagem-por-prioridade", m]);
    assert!(o.status.success(), "{}", texto(&o));
    let o = phxclaw(&d, &["fluxo", "usar", "triagem-por-prioridade", m]);
    assert!(
        !o.status.success() && texto(&o).contains("ja existe"),
        "{}",
        texto(&o)
    );
    let o = phxclaw(
        &d,
        &[
            "fluxo",
            "usar",
            "ponte-n8n",
            fluxos.join("p.json").to_str().unwrap(),
        ],
    );
    assert!(
        texto(&o).contains("guarde antes de rodar") && texto(&o).contains("n8n"),
        "{}",
        texto(&o)
    );

    // publicar, recusar o igual, editar, ver a diferenca, publicar de novo e voltar
    let o = phxclaw(&d, &["fluxo", "publicar", m, "--nota", "primeira"]);
    assert!(texto(&o).contains("publicado: v1"), "{}", texto(&o));
    let o = phxclaw(&d, &["fluxo", "publicar", m]);
    assert!(
        !o.status.success() && texto(&o).contains("nada a publicar"),
        "{}",
        texto(&o)
    );
    let editado = std::fs::read_to_string(&meu)
        .unwrap()
        .replace("\"max_paralelo\": 2", "\"max_paralelo\": 3");
    std::fs::write(&meu, editado).unwrap();
    let o = phxclaw(&d, &["fluxo", "versoes", m]);
    assert!(
        texto(&o).contains("<- publicada") && texto(&o).contains("difere da publicada"),
        "{}",
        texto(&o)
    );
    let o = phxclaw(&d, &["fluxo", "publicar", m]);
    assert!(texto(&o).contains("publicado: v2"), "{}", texto(&o));
    let o = phxclaw(&d, &["fluxo", "voltar", m, "2"]);
    assert!(
        !o.status.success() && texto(&o).contains("ja e a publicada"),
        "{}",
        texto(&o)
    );
    let o = phxclaw(&d, &["fluxo", "voltar", m, "1"]);
    assert!(texto(&o).contains("publicada: v3"), "{}", texto(&o));

    // `rodar` le o RASCUNHO; `rodar --publicada` le a versao publicada
    let r = fluxos.join("r.json");
    let escreve = |t: &str| {
        std::fs::write(&r, format!(r#"{{"nome":"r","passos":[{{"id":"a","ferramenta":"write_file","args":{{"path":"saida.txt","content":"{t}"}}}}]}}"#)).unwrap();
    };
    escreve("versao-publicada");
    let o = phxclaw(&d, &["fluxo", "publicar", r.to_str().unwrap()]);
    assert!(o.status.success(), "{}", texto(&o));
    escreve("rascunho-novo");
    let gravado = |o: &std::process::Output| {
        let id = texto(o)
            .split("tarefa do fluxo: ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .to_string();
        std::fs::read_to_string(d.join("agente/tasks").join(id).join("work/saida.txt")).unwrap()
    };
    let o = phxclaw(&d, &["fluxo", "rodar", r.to_str().unwrap(), "--publicada"]);
    assert_eq!(gravado(&o), "versao-publicada", "{}", texto(&o));
    let o = phxclaw(&d, &["fluxo", "rodar", r.to_str().unwrap()]);
    assert_eq!(gravado(&o), "rascunho-novo", "{}", texto(&o));

    // git: o prod exporta a publicada (v3), o ambiente invalido e recusado, e o outro projeto importa e publica
    let repo = d.join("repo");
    let (f, r) = (fluxos.to_str().unwrap(), repo.to_str().unwrap());
    let o = phxclaw(
        &d,
        &[
            "fluxo",
            "exportar",
            "--ambiente",
            "prod",
            "--fluxos",
            f,
            "--repo",
            r,
        ],
    );
    assert!(o.status.success(), "{}", texto(&o));
    assert!(
        std::fs::read_to_string(repo.join("prod/meu.json"))
            .unwrap()
            .contains("\"versao\": 3")
    );
    let o = phxclaw(
        &d,
        &[
            "fluxo",
            "exportar",
            "--ambiente",
            "homologacao",
            "--fluxos",
            f,
            "--repo",
            r,
        ],
    );
    assert!(
        !o.status.success() && texto(&o).contains("dev ou prod"),
        "{}",
        texto(&o)
    );
    let outro = d.join("outro");
    let o = phxclaw(
        &d,
        &[
            "fluxo",
            "importar",
            "--ambiente",
            "prod",
            "--fluxos",
            outro.to_str().unwrap(),
            "--repo",
            r,
        ],
    );
    assert!(
        o.status.success() && texto(&o).contains("publicada v1"),
        "{}",
        texto(&o)
    );
    let o = phxclaw(
        &d,
        &["fluxo", "versoes", outro.join("meu.json").to_str().unwrap()],
    );
    assert!(texto(&o).contains("<- publicada"), "{}", texto(&o));
    let _ = std::fs::remove_dir_all(&d);
}
