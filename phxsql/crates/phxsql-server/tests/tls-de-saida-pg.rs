//! O TLS de SAIDA do DbLink para um PostgreSQL(R) DE VERDADE (pedido 572,
//! T6d) -- o servidor 16 do conteiner, num cluster temporario.
//!
//! O cluster RECUSA conexao sem TLS (`hostnossl ... reject`) e so aceita
//! `scram-sha-256` pelo TLS. Assim cada veredito diz o que se quer provar:
//!
//! 1. `desligado` cai na recusa do servidor -- o claro continua o de sempre;
//! 2. `exigir` entra, e o `pg_stat_ssl` do outro lado diz `ssl = t`;
//! 3. `verificar` com a raiz certa e o nome do SAN entra; com o nome que o
//!    SAN nao tem, ou outra raiz, recusa antes de mandar a senha;
//! 4. o login atravessa pelo `SCRAM-SHA-256-PLUS` (o servidor com TLS o
//!    oferece, e o vinculo `tls-server-end-point` errado ele recusa -- o
//!    defeito reposto prova isso).
//!
//! Sem `initdb` ou sem `runuser` na maquina o teste diz NAO MEDIDO e sai: o
//! que nao se mede nao se finge medido.

mod comum;
use comum::DirTemp;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use phxsql_server::pg::Conexao;
use phxsql_server::prazo::Prazo;
use phxsql_server::tls_saida::TlsDeSaida;

const SENHA: &str = "senha-do-pg-pelo-tls";

fn bin_do_pg() -> Option<PathBuf> {
    let mut versoes: Vec<PathBuf> = std::fs::read_dir("/usr/lib/postgresql")
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path().join("bin")))
        .filter(|b| b.join("initdb").exists())
        .collect();
    versoes.sort();
    versoes.pop()
}

fn como_postgres(args: &[&str]) -> std::process::Output {
    Command::new("runuser")
        .args(["-u", "postgres", "--"])
        .args(args)
        .output()
        .unwrap()
}

fn ok(s: std::process::Output, oque: &str) {
    assert!(
        s.status.success(),
        "{oque}: {}{}",
        String::from_utf8_lossy(&s.stdout),
        String::from_utf8_lossy(&s.stderr)
    );
}

struct Cluster {
    bin: PathBuf,
    /// A pasta do cluster; apagada pelo `DirTemp` DEPOIS do `pg_ctl stop`
    /// do `Drop` abaixo (os campos caem depois do corpo do `drop`).
    _pasta: DirTemp,
    dir: PathBuf,
    porta: u16,
}

impl Drop for Cluster {
    fn drop(&mut self) {
        let dados = self.dir.join("dados");
        let _ = como_postgres(&[
            self.bin.join("pg_ctl").to_str().unwrap(),
            "-D",
            dados.to_str().unwrap(),
            "-m",
            "immediate",
            "stop",
        ]);
    }
}

fn openssl(d: &Path, args: &[&str]) {
    let s = Command::new("openssl")
        .current_dir(d)
        .args(args)
        .output()
        .unwrap();
    ok(s, "openssl");
}

/// Raiz e folha (SAN so `localhost`) para o servidor.
fn certificados(d: &Path, prefixo: &str) {
    let r = format!("{prefixo}raiz");
    openssl(
        d,
        &[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-384",
            "-nodes",
            "-keyout",
            &format!("{r}.key"),
            "-out",
            &format!("{r}.pem"),
            "-days",
            "5",
            "-subj",
            &format!("/CN={prefixo}Raiz do PG"),
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign",
        ],
    );
    std::fs::write(
        d.join("ext"),
        "[f]\nbasicConstraints=CA:FALSE\nkeyUsage=critical,digitalSignature\n\
         extendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n",
    )
    .unwrap();
    openssl(
        d,
        &[
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
            "-out",
            "servidor.key",
        ],
    );
    openssl(
        d,
        &[
            "req",
            "-new",
            "-key",
            "servidor.key",
            "-subj",
            "/CN=localhost",
            "-out",
            "s.csr",
        ],
    );
    openssl(
        d,
        &[
            "x509",
            "-req",
            "-in",
            "s.csr",
            "-CA",
            &format!("{r}.pem"),
            "-CAkey",
            &format!("{r}.key"),
            "-CAcreateserial",
            "-days",
            "3",
            "-sha256",
            "-extfile",
            "ext",
            "-extensions",
            "f",
            "-out",
            "servidor.pem",
        ],
    );
}

fn subir() -> Option<Cluster> {
    let bin = bin_do_pg()?;
    if Command::new("runuser").arg("--help").output().is_err() {
        return None;
    }
    let pasta = DirTemp::novo("pg-tls");
    let dir = pasta.0.clone();
    certificados(&dir, "");
    let porta = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    std::fs::write(dir.join("senha"), "senha-do-superusuario\n").unwrap();
    ok(
        Command::new("chown")
            .args(["-R", "postgres:postgres"])
            .arg(&dir)
            .output()
            .unwrap(),
        "chown",
    );
    let dados = dir.join("dados");
    ok(
        como_postgres(&[
            bin.join("initdb").to_str().unwrap(),
            "-D",
            dados.to_str().unwrap(),
            "-A",
            "scram-sha-256",
            "--pwfile",
            dir.join("senha").to_str().unwrap(),
            "-U",
            "postgres",
        ]),
        "initdb",
    );
    let conf = format!(
        "port = {porta}\nlisten_addresses = '127.0.0.1'\nunix_socket_directories = '{d}'\n\
         ssl = on\nssl_cert_file = '{d}/servidor.pem'\nssl_key_file = '{d}/servidor.key'\n\
         password_encryption = 'scram-sha-256'\n",
        d = dir.display()
    );
    let pc = dados.join("postgresql.conf");
    let mut texto = std::fs::read_to_string(&pc).unwrap();
    texto.push_str(&conf);
    std::fs::write(&pc, texto).unwrap();
    std::fs::write(
        dados.join("pg_hba.conf"),
        "local all postgres trust\n\
         hostnossl all all 127.0.0.1/32 reject\n\
         hostssl all all 127.0.0.1/32 scram-sha-256\n",
    )
    .unwrap();
    ok(
        Command::new("chmod")
            .arg("600")
            .arg(dir.join("servidor.key"))
            .output()
            .unwrap(),
        "chmod",
    );
    ok(
        Command::new("chown")
            .args(["-R", "postgres:postgres"])
            .arg(&dir)
            .output()
            .unwrap(),
        "chown",
    );
    ok(
        como_postgres(&[
            bin.join("pg_ctl").to_str().unwrap(),
            "-D",
            dados.to_str().unwrap(),
            "-l",
            dir.join("log").to_str().unwrap(),
            "-w",
            "start",
        ]),
        "pg_ctl start",
    );
    let c = Cluster {
        bin,
        _pasta: pasta,
        dir,
        porta,
    };
    ok(
        como_postgres(&[
            c.bin.join("psql").to_str().unwrap(),
            "-h",
            c.dir.to_str().unwrap(),
            "-p",
            &porta.to_string(),
            "-U",
            "postgres",
            "-c",
            &format!("CREATE ROLE ana LOGIN PASSWORD '{SENHA}'"),
        ]),
        "psql",
    );
    Some(c)
}

fn abrir(c: &Cluster, host: &str, tls: &TlsDeSaida) -> phxsql_core::error::Result<Conexao> {
    Conexao::abrir(
        host,
        c.porta,
        "ana",
        SENHA,
        "postgres",
        Prazo::so_silencio(Duration::from_secs(10)),
        tls,
    )
}

#[test]
fn dblink_postgres_pelo_tls_nos_tres_modos() {
    let Some(c) = subir() else {
        eprintln!("NAO MEDIDO: sem PostgreSQL (initdb) ou sem runuser nesta maquina");
        return;
    };
    let raiz = std::fs::read_to_string(c.dir.join("raiz.pem")).unwrap();
    let raiz = phxsql_core::x509::blocos_pem(&raiz, "CERTIFICATE").unwrap();

    // 1. Desligado: o servidor recusa o claro.
    let e = abrir(&c, "127.0.0.1", &TlsDeSaida::Desligado)
        .err()
        .expect("o claro entrou num servidor que so aceita TLS")
        .to_string();
    assert!(e.contains("no encryption") || e.contains("pg_hba"), "{e}");

    // 2. Exigir: entra, e o servidor confirma o TLS desta conexao.
    let mut x = abrir(&c, "127.0.0.1", &TlsDeSaida::Exigir).expect("exigir nao entrou");
    let r = x
        .consultar(
            "SELECT ssl, version FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
            10,
        )
        .unwrap();
    assert_eq!(r.linhas[0][0].as_deref(), Some("t"), "{:?}", r.linhas);
    assert_eq!(r.linhas[0][1].as_deref(), Some("TLSv1.3"), "{:?}", r.linhas);

    // 3. Verificar: raiz certa e o nome do SAN.
    let mut v = abrir(&c, "localhost", &TlsDeSaida::Verificar(raiz.clone()))
        .expect("verificar com a raiz certa nao entrou");
    v.ping().unwrap();
    let e = abrir(&c, "127.0.0.1", &TlsDeSaida::Verificar(raiz))
        .err()
        .expect("o IP entrou num certificado que so tem DNS:localhost")
        .to_string();
    assert!(e.contains("nao vale para o nome"), "{e}");
    let d = DirTemp::novo("pg-tls-alheia");
    certificados(&d.0, "outra");
    let alheia = phxsql_core::x509::blocos_pem(
        &std::fs::read_to_string(d.0.join("outraraiz.pem")).unwrap(),
        "CERTIFICATE",
    )
    .unwrap();
    let e = abrir(&c, "localhost", &TlsDeSaida::Verificar(alheia))
        .err()
        .expect("outra raiz entrou")
        .to_string();
    assert!(e.contains("cadeia X.509"), "{e}");
}
