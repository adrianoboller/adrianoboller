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

// ---------------------------------------------------------------- onda dos 100% (09/10/2026)

fn git_fora(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

/// `exportar --commit --push` e `importar --pull` pela CLI, contra um remoto bare LOCAL: a CLI
/// e porta fina para o `fluxo_git::empurrar`/`puxar`, e a recusa sai com o motivo.
#[test]
fn push_e_pull_pela_cli() {
    if !Path::new("/usr/bin/bwrap").exists() {
        phxclaw_test_support::pulado::pular("bwrap", "sem bwrap o git do agente nao roda");
        return;
    }
    let d = pasta("remoto");
    let bare = d.join("remoto.git");
    std::fs::create_dir_all(&bare).unwrap();
    git_fora(&bare, &["init", "-q", "--bare", "-b", "main"]);
    let url = format!("file://{}", bare.display());
    let fluxos = d.join("fluxos");
    std::fs::create_dir_all(&fluxos).unwrap();
    std::fs::write(
        fluxos.join("x.json"),
        r#"{"nome":"x","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"v1"}}]}"#,
    )
    .unwrap();
    let repo = d.join("repo");
    let (f, r) = (fluxos.to_str().unwrap(), repo.to_str().unwrap());
    let exportar = |extra: &[&str]| {
        let mut a = vec![
            "fluxo",
            "exportar",
            "--ambiente",
            "prod",
            "--fluxos",
            f,
            "--repo",
            r,
        ];
        a.extend_from_slice(extra);
        phxclaw(&d, &a)
    };
    let o = exportar(&["--commit", "fluxos v1", "--push", "--remoto", &url]);
    assert!(o.status.success(), "{}", texto(&o));
    assert!(texto(&o).contains("\"enviado\":true"), "{}", texto(&o));

    // a outra instancia puxa e importa (no prod, publica)
    let outros = d.join("outros");
    std::fs::create_dir_all(&outros).unwrap();
    let repo2 = d.join("repo2");
    let (f2, r2) = (outros.to_str().unwrap(), repo2.to_str().unwrap());
    let importar = || {
        phxclaw(
            &d,
            &[
                "fluxo",
                "importar",
                "--ambiente",
                "prod",
                "--fluxos",
                f2,
                "--repo",
                r2,
                "--pull",
                "--remoto",
                &url,
            ],
        )
    };
    let o = importar();
    assert!(o.status.success(), "{}", texto(&o));
    assert!(
        texto(&o).contains("\"trazido\":true") && texto(&o).contains("publicada v1"),
        "{}",
        texto(&o)
    );
    // a outra instancia edita o rascunho dela
    let local = r#"{"nome":"x","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"local"}}]}"#;
    std::fs::write(outros.join("x.json"), local).unwrap();

    // a primeira edita e manda empurrar sem registrar: recusa dizendo; registrando, vai
    std::fs::write(
        fluxos.join("x.json"),
        r#"{"nome":"x","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"v2"}}]}"#,
    )
    .unwrap();
    let o = exportar(&["--push", "--remoto", &url]);
    assert!(
        !o.status.success() && texto(&o).contains("nao registrada"),
        "{}",
        texto(&o)
    );
    let o = exportar(&["--commit", "fluxos v2", "--push", "--remoto", &url]);
    assert!(o.status.success(), "{}", texto(&o));

    // o pull traz a v2, e o importar nao troca o rascunho alheio sem --sobrescrever
    let o = importar();
    assert!(
        !o.status.success() && texto(&o).contains("seriam perdidos"),
        "{}",
        texto(&o)
    );
    assert_eq!(
        std::fs::read_to_string(outros.join("x.json")).unwrap(),
        local
    );
}

/// O segredo da prova do remoto de rede e o base64 do Basic dele (`fluxos-bot:<segredo>`),
/// fixo para a prova nao precisar de crate de base64.
const SEGREDO_GIT: &str = "phxtok_cli_W3nQ8rT1yU6iO9pA2sD5fG7h";
const BASICO_GIT: &str = "Zmx1eG9zLWJvdDpwaHh0b2tfY2xpX1czblE4clQxeVU2aU85cEEyc0Q1Zkc3aA==";

/// `exportar --push` e `importar --pull` SEM `--remoto`, com o remoto de REDE da configuracao
/// do operador (`fluxos.git.remoto` no `config.json` da pasta do agente) e a credencial por
/// nome (`fluxos.git.credencial_nome`, guardada por `phxclaw credencial guardar`), contra o
/// servidor git HTTP falso em loopback: a CLI liga a chave ao `empurrar_pela_rede`/
/// `puxar_pela_rede`, a credencial chega ao servidor e nao aparece na saida nem no argv. E a
/// instancia ligada a um RAMO (`fluxos.git.ramo`): o repositorio novo nasce nele, o push vai a
/// ele, e o repositorio em outro ramo e recusado sem `checkout` calado.
///
/// RED medido em 09/10/2026, cada um com `// REPOSTO` e restaurado por escrita:
/// - `let rede = None` no `main::fluxo_git` (a chave ignorada): o push caiu no `origin` do
///   `.git/config`, que nao existe, e a CLI recusou;
/// - `ligar_ramo` sem chamada: o push foi ao `main`, e o `rev-parse producao` do remoto caiu.
#[test]
fn push_e_pull_pelo_remoto_de_rede_do_operador() {
    use phxclaw_test_support::git_http::Servidor;
    if !Path::new("/usr/bin/bwrap").exists() {
        phxclaw_test_support::pulado::pular("bwrap", "sem bwrap o git do agente nao roda");
        return;
    }
    let d = pasta("rede");
    let servidos = d.join("servidos");
    std::fs::create_dir_all(&servidos).unwrap();
    git_fora(
        &d,
        &["init", "-q", "--bare", "-b", "main", "servidos/fluxos.git"],
    );
    let srv = Servidor::subir(
        &servidos,
        Some(format!("Basic {BASICO_GIT}")),
        vec![SEGREDO_GIT.into(), BASICO_GIT.into()],
    );
    let agente = d.join("agente");
    std::fs::create_dir_all(&agente).unwrap();
    std::fs::write(
        agente.join("http.json"),
        serde_json::json!({"credenciais": {"git-fluxos": {
            "tipo": "basico", "usuario": "fluxos-bot", "origens": [srv.base]}}})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        agente.join("config.json"),
        serde_json::json!({"fluxos": {"git": {
            "remoto": format!("{}/fluxos.git", srv.base), "credencial_nome": "git-fluxos",
            "ramo": "producao"}}})
        .to_string(),
    )
    .unwrap();
    // O segredo pela entrada padrao, como o operador faz.
    let mut c = Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args(["credencial", "guardar", "git-fluxos", "--pasta"])
        .arg(&agente)
        .env_remove("PHXCLAW_HOME")
        .current_dir(&d)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        writeln!(c.stdin.take().unwrap(), "{SEGREDO_GIT}").unwrap();
    }
    let o = c.wait_with_output().unwrap();
    assert!(o.status.success(), "{}", texto(&o));

    let fluxos = d.join("fluxos");
    std::fs::create_dir_all(&fluxos).unwrap();
    std::fs::write(
        fluxos.join("x.json"),
        r#"{"nome":"x","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"v1"}}]}"#,
    )
    .unwrap();
    let repo = d.join("repo");
    let o = phxclaw(
        &d,
        &[
            "fluxo",
            "exportar",
            "--ambiente",
            "prod",
            "--fluxos",
            fluxos.to_str().unwrap(),
            "--repo",
            repo.to_str().unwrap(),
            "--commit",
            "fluxos v1",
            "--push",
        ],
    );
    assert!(o.status.success(), "{}", texto(&o));
    assert!(texto(&o).contains("\"enviado\":true"), "{}", texto(&o));

    let outros = d.join("outros");
    std::fs::create_dir_all(&outros).unwrap();
    let o = phxclaw(
        &d,
        &[
            "fluxo",
            "importar",
            "--ambiente",
            "prod",
            "--fluxos",
            outros.to_str().unwrap(),
            "--repo",
            d.join("repo2").to_str().unwrap(),
            "--pull",
        ],
    );
    assert!(o.status.success(), "{}", texto(&o));
    assert!(
        texto(&o).contains("\"trazido\":true") && texto(&o).contains("publicada v1"),
        "{}",
        texto(&o)
    );
    // O ramo da instancia: o remoto recebeu `producao`, e o repositorio novo nasceu nele.
    git_fora(
        &servidos.join("fluxos.git"),
        &["rev-parse", "--verify", "producao"],
    );
    let o = Command::new("git")
        .args([
            "-C",
            d.join("repo2").to_str().unwrap(),
            "symbolic-ref",
            "--short",
            "HEAD",
        ])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "producao");
    // Repositorio em OUTRO ramo: recusa dizendo, e o ramo dele fica onde estava.
    let repo3 = d.join("repo3");
    std::fs::create_dir_all(&repo3).unwrap();
    git_fora(&repo3, &["init", "-q", "-b", "main"]);
    let o = phxclaw(
        &d,
        &[
            "fluxo",
            "importar",
            "--ambiente",
            "prod",
            "--fluxos",
            outros.to_str().unwrap(),
            "--repo",
            repo3.to_str().unwrap(),
            "--pull",
        ],
    );
    assert!(
        !o.status.success() && texto(&o).contains("ligada ao ramo producao"),
        "{}",
        texto(&o)
    );
    let visto = srv.visto();
    assert!(
        visto.pedidos.iter().any(|p| p.contains("git-receive-pack")),
        "{:?}",
        visto.pedidos
    );
    assert!(
        visto
            .autorizacoes
            .iter()
            .all(|a| *a == format!("Basic {BASICO_GIT}")),
        "{:?}",
        visto.pedidos
    );
    assert_eq!(visto.argv_com_segredo, 0);
    let _ = std::fs::remove_dir_all(&d);
}

/// Ollama falso com ROTEIRO: cada `POST /api/chat` leva a proxima resposta (a ultima se
/// repete); os corpos ficam para o teste ler.
fn ollama_roteiro(
    respostas: Vec<String>,
) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let vistos: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    let v2 = vistos.clone();
    std::thread::spawn(move || {
        let mut fila = respostas.into_iter();
        let mut atual = String::new();
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut tamanho = 0usize;
            let mut primeira = String::new();
            loop {
                let mut linha = String::new();
                if r.read_line(&mut linha).unwrap_or(0) == 0 || linha == "\r\n" {
                    break;
                }
                if primeira.is_empty() {
                    primeira = linha.clone();
                }
                if let Some(n) = linha.to_lowercase().strip_prefix("content-length:") {
                    tamanho = n.trim().parse().unwrap_or(0);
                }
            }
            let mut corpo = vec![0; tamanho];
            let _ = r.read_exact(&mut corpo);
            if primeira.contains("/api/chat") {
                if let Some(p) = fila.next() {
                    atual = p;
                }
                v2.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&corpo).into());
            }
            let resp = serde_json::json!({
                "model": "falso",
                "message": {"role": "assistant", "content": atual},
                "prompt_eval_count": 10, "eval_count": 5, "done": true
            })
            .to_string();
            let mut s = s;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{resp}",
                resp.len()
            );
        }
    });
    (base, vistos)
}

/// `fluxo criar --descricao` pela CLI, com o modelo de verdade do agente falando com um
/// Ollama falso: a 1a resposta o motor recusa, a 2a vira rascunho na pasta de fluxos do
/// agente, nada publicado; sem --descricao, recusa antes de montar o modelo.
#[test]
fn criar_pela_cli() {
    let d = pasta("criar");
    let o = phxclaw(&d, &["fluxo", "criar"]);
    assert!(
        !o.status.success() && texto(&o).contains("falta --descricao"),
        "{}",
        texto(&o)
    );
    let (base, vistos) = ollama_roteiro(vec![
        r#"{"nome":"resumo","passos":[{"id":"a","tarefa":"resuma {{coleta}}"}]}"#.into(),
        r#"{"nome":"resumo","passos":[{"id":"coleta","ferramenta":"read_file","args":{"path":"n.md"}},{"id":"a","depende":["coleta"],"tarefa":"resuma {{coleta}}"}]}"#.into(),
    ]);
    let o = Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args([
            "fluxo",
            "criar",
            "--descricao",
            "resumir as notas",
            "--modelo",
            "ollama:falso",
            "--pasta",
            d.join("agente").to_str().unwrap(),
        ])
        .env_remove("PHXCLAW_HOME")
        .env("OLLAMA_HOST", &base)
        .current_dir(&d)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", texto(&o));
    assert!(
        texto(&o).contains("rascunho resumo (2 passos)")
            && texto(&o).contains("2 tentativa(s)")
            && texto(&o).contains("nada foi publicado"),
        "{}",
        texto(&o)
    );
    let arq = d.join("agente/fluxos/resumo.json");
    assert!(arq.exists());
    assert!(!d.join("agente/fluxos/.resumo.versoes").exists());
    let vistos = vistos.lock().unwrap();
    assert_eq!(vistos.len(), 2);
    assert!(vistos[1].contains("sem declarar"), "{}", vistos[1]);
}
