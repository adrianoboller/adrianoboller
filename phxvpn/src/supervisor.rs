//! Sobe e vigia um processo `openvpn` por rede.
//!
//! Opcional (`--openvpn`): sem ele o painel so escreve os arquivos e quem
//! administra sobe o OpenVPN como quiser (systemd, servico do Windows). Com
//! ele, rede criada ja entra no ar. Entrar e sair de rede NAO reinicia nada:
//! o OpenVPN rele o `ccd/` a cada conexao.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

pub struct Supervisor {
    binario: PathBuf,
    filhos: Mutex<HashMap<PathBuf, Child>>,
}

impl Supervisor {
    /// Acha o `openvpn` no PATH. Nao achar e erro dito, nao silencio: o
    /// administrador pediu `--openvpn` esperando a VPN no ar.
    pub fn novo() -> Result<Supervisor, String> {
        let binario = achar_no_path("openvpn").ok_or(
            "--openvpn pedido, mas o binario openvpn nao esta no PATH (instale o OpenVPN 2.6+)",
        )?;
        Ok(Supervisor {
            binario,
            filhos: Mutex::new(HashMap::new()),
        })
    }

    pub fn garantir(&self, nome: &str, dir: &Path) -> Result<(), String> {
        let mut filhos = self.filhos.lock().expect("filhos");
        if let Some(f) = filhos.get_mut(dir) {
            if matches!(f.try_wait(), Ok(None)) {
                return Ok(());
            }
            eprintln!("phxvpn: o OpenVPN da rede «{nome}» parou; subindo de novo");
        }
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("openvpn.log"))
            .map_err(|e| format!("abrir log: {e}"))?;
        let filho = Command::new(&self.binario)
            .arg("--config")
            .arg(dir.join("servidor.conf"))
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            .spawn()
            .map_err(|e| format!("subir openvpn da rede «{nome}»: {e}"))?;
        eprintln!(
            "phxvpn: OpenVPN da rede «{nome}» no ar (pid {})",
            filho.id()
        );
        filhos.insert(dir.to_path_buf(), filho);
        Ok(())
    }
}

impl Supervisor {
    /// Para e sobe de novo: a configuracao mudou (ligar/desligar o
    /// autenticador da rede), e o OpenVPN so le o `servidor.conf` ao subir.
    pub fn reiniciar(&self, nome: &str, dir: &Path) -> Result<(), String> {
        if let Some(mut f) = self.filhos.lock().expect("filhos").remove(dir) {
            let _ = f.kill();
            let _ = f.wait();
        }
        self.garantir(nome, dir)
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Ok(mut f) = self.filhos.lock() {
            for (_, filho) in f.iter_mut() {
                let _ = filho.kill();
            }
        }
    }
}

pub fn achar_no_path(nome: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| {
        [nome.to_string(), format!("{nome}.exe")]
            .iter()
            .map(|n| d.join(n))
            .find(|c| c.is_file())
    })
}
