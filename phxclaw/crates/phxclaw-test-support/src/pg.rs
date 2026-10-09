//! Um PostgreSQL de verdade, efemero, para o teste que precisa de banco: `initdb` numa
//! pasta temporaria, porta livre, so TCP em 127.0.0.1, senha SCRAM, e `pg_ctl stop` + a
//! pasta apagada no `Drop`.
//!
//! Por que sobe o proprio banco em vez de ler uma URL do ambiente: um teste que apaga e
//! recria tabela nao pode apontar para o banco de alguem (a URL do operador mora no
//! catalogo do config.json, e o teste nao tem como saber se e de producao). E por que mora
//! aqui: o agente e a CLI provam a fila contra o MESMO banco efemero, e duas copias do
//! arranque seriam a mesma decisao escrita duas vezes.
//!
//! Sem os binarios, `subir` devolve o motivo e quem chama registra o pulo pelo
//! [`crate::pulado`] -- nunca um verde calado. Como root (o contêiner das provas), o servidor
//! roda como o usuario `postgres` pelo `runuser`: o PostgreSQL recusa rodar como root.
//!
//! A pasta e curta e fica no `temp_dir()`: o caminho do soquete Unix tem teto de 107 bytes,
//! e por isso o soquete Unix fica desligado (`unix_socket_directories=''`).

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PgEfemero {
    pub dir: PathBuf,
    pub porta: u16,
    pub senha: String,
    bin: PathBuf,
    como_postgres: bool,
}

/// A pasta dos binarios: `pg_config --bindir`, ou a versao mais alta em
/// `/usr/lib/postgresql/*/bin` (o pacote do Debian nao poe o `initdb` no PATH).
pub fn bindir() -> Option<PathBuf> {
    if let Ok(o) = Command::new("pg_config").arg("--bindir").output()
        && o.status.success()
    {
        let p = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
        if p.join("initdb").is_file() {
            return Some(p);
        }
    }
    let mut versoes: Vec<(u32, PathBuf)> = std::fs::read_dir("/usr/lib/postgresql")
        .ok()?
        .flatten()
        .filter_map(|e| {
            let v = e.file_name().to_string_lossy().parse::<u32>().ok()?;
            let b = e.path().join("bin");
            b.join("initdb").is_file().then_some((v, b))
        })
        .collect();
    versoes.sort();
    versoes.pop().map(|(_, b)| b)
}

fn root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}

impl PgEfemero {
    /// Sobe o banco. `Err` traz o motivo (sem binario, sem `runuser`, `initdb` que falhou).
    pub fn subir() -> Result<Self, String> {
        let bin = bindir().ok_or("sem initdb (pg_config --bindir ou /usr/lib/postgresql/*/bin)")?;
        let como_postgres = root();
        let unico = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("phx-pg-{}-{unico}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        // Senha so desta corrida; nunca vai para log nem para a linha de comando.
        let senha = format!("prova{unico:x}");
        let pw = dir.join("pw");
        std::fs::write(&pw, &senha).map_err(|e| e.to_string())?;
        if como_postgres {
            let ok = Command::new("chown")
                .args(["-R", "postgres:postgres"])
                .arg(&dir)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !ok {
                let _ = std::fs::remove_dir_all(&dir);
                return Err("rodando como root e sem usuario postgres para o servidor".into());
            }
        }
        let porta = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .map_err(|e| e.to_string())?;
        let pg = Self {
            dir,
            porta,
            senha,
            bin,
            como_postgres,
        };
        let dados = pg.dir.join("dados");
        pg.rodar(
            "initdb",
            &[
                "-D",
                &dados.to_string_lossy(),
                "-U",
                "postgres",
                "--auth=scram-sha-256",
                &format!("--pwfile={}", pw.display()),
                "-E",
                "UTF8",
                "--no-sync",
            ],
        )?;
        let _ = std::fs::remove_file(&pw);
        pg.rodar(
            "pg_ctl",
            &[
                "-D",
                &dados.to_string_lossy(),
                "-o",
                &format!(
                    "-p {porta} -c unix_socket_directories='' -c listen_addresses=127.0.0.1 -c fsync=off"
                ),
                "-l",
                &pg.dir.join("log").to_string_lossy(),
                "-w",
                "start",
            ],
        )?;
        Ok(pg)
    }

    fn rodar(&self, prog: &str, args: &[&str]) -> Result<(), String> {
        let exe = self.bin.join(prog);
        let mut c = if self.como_postgres {
            let mut c = Command::new("runuser");
            c.args(["-u", "postgres", "--"]).arg(&exe);
            c
        } else {
            Command::new(&exe)
        };
        // O cwd do filho e a pasta do banco: o `runuser` herdaria um cwd que o usuario
        // postgres nao alcanca, e o initdb reclama.
        let o = c
            .args(args)
            .current_dir(&self.dir)
            .output()
            .map_err(|e| format!("{prog}: {e}"))?;
        if o.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{prog} falhou: {}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ))
        }
    }

    /// URL com a senha (para o cliente do teste).
    pub fn url(&self) -> String {
        format!(
            "postgresql://postgres:{}@127.0.0.1:{}/postgres",
            self.senha, self.porta
        )
    }

    /// URL SEM a senha (a forma que o operador poe no config.json, com a senha no broker).
    pub fn url_sem_senha(&self) -> String {
        format!("postgresql://postgres@127.0.0.1:{}/postgres", self.porta)
    }

    /// Uma consulta pelo `psql` (`-At`: sem cabecalho, colunas separadas por `|`).
    pub fn sql(&self, consulta: &str) -> Result<String, String> {
        let o = Command::new(self.bin.join("psql"))
            .args([
                "-X",
                "-At",
                "-v",
                "ON_ERROR_STOP=1",
                "-h",
                "127.0.0.1",
                "-U",
                "postgres",
            ])
            .args([
                "-p",
                &self.porta.to_string(),
                "-d",
                "postgres",
                "-c",
                consulta,
            ])
            .env("PGPASSWORD", &self.senha)
            .output()
            .map_err(|e| format!("psql: {e}"))?;
        if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else {
            Err(format!("psql: {}", String::from_utf8_lossy(&o.stderr)))
        }
    }

    /// Roda um arquivo SQL pelo `psql`, parando no primeiro erro.
    pub fn aplicar(&self, arquivo: &Path) -> Result<(), String> {
        let o = Command::new(self.bin.join("psql"))
            .args([
                "-X",
                "-q",
                "-v",
                "ON_ERROR_STOP=1",
                "-h",
                "127.0.0.1",
                "-U",
                "postgres",
            ])
            .args(["-p", &self.porta.to_string(), "-d", "postgres", "-f"])
            .arg(arquivo)
            .env("PGPASSWORD", &self.senha)
            .output()
            .map_err(|e| format!("psql: {e}"))?;
        if o.status.success() {
            Ok(())
        } else {
            Err(format!(
                "psql {}: {}",
                arquivo.display(),
                String::from_utf8_lossy(&o.stderr)
            ))
        }
    }
}

impl Drop for PgEfemero {
    fn drop(&mut self) {
        let dados = self.dir.join("dados");
        let _ = self.rodar(
            "pg_ctl",
            &[
                "-D",
                &dados.to_string_lossy(),
                "-m",
                "immediate",
                "-w",
                "stop",
            ],
        );
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
