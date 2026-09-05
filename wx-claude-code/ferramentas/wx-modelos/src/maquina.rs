//! O que esta maquina tem, MEDIDO -- nunca chutado.
//!
//! Cada campo que nao se consegue ler vira None e a tela mostra INDISPONIVEL.
//! A tentacao aqui e grande: "todo mac tem 16 GB", "deve ter 8 nucleos". Um
//! numero errado sobre a maquina do cliente decide o modelo errado, e o
//! sintoma aparece meia hora depois, como lentidao inexplicada.

use std::fs;
use std::process::Command;

#[derive(Debug, Default, Clone)]
pub struct Maquina {
    pub so: String,
    pub arquitetura: String,
    pub processador: Option<String>,
    pub nucleos: Option<u32>,
    pub memoria_bytes: Option<u64>,
    pub memoria_livre_bytes: Option<u64>,
    pub acelerador: Option<String>,
    pub memoria_unificada: bool,
}

fn comando(programa: &str, args: &[&str]) -> Option<String> {
    let saida = Command::new(programa).args(args).output().ok()?;
    if !saida.status.success() {
        return None;
    }
    let texto = String::from_utf8_lossy(&saida.stdout).trim().to_string();
    if texto.is_empty() {
        None
    } else {
        Some(texto)
    }
}

fn campo_proc(arquivo: &str, chave: &str) -> Option<String> {
    let texto = fs::read_to_string(arquivo).ok()?;
    for linha in texto.lines() {
        if let Some((k, v)) = linha.split_once(':') {
            if k.trim() == chave {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

fn meminfo_kb(chave: &str) -> Option<u64> {
    let texto = fs::read_to_string("/proc/meminfo").ok()?;
    for linha in texto.lines() {
        if let Some(resto) = linha.strip_prefix(chave) {
            let numero: String = resto.chars().filter(|c| c.is_ascii_digit()).collect();
            return numero.parse::<u64>().ok().map(|kb| kb * 1024);
        }
    }
    None
}

impl Maquina {
    pub fn medir() -> Self {
        let mut m = Maquina {
            so: std::env::consts::OS.to_string(),
            arquitetura: std::env::consts::ARCH.to_string(),
            ..Default::default()
        };
        match std::env::consts::OS {
            "macos" => m.medir_macos(),
            "linux" => m.medir_linux(),
            _ => {}
        }
        m
    }

    fn medir_macos(&mut self) {
        self.processador = comando("sysctl", &["-n", "machdep.cpu.brand_string"]);
        self.nucleos = comando("sysctl", &["-n", "hw.ncpu"]).and_then(|s| s.parse().ok());
        self.memoria_bytes = comando("sysctl", &["-n", "hw.memsize"]).and_then(|s| s.parse().ok());
        // Apple Silicon: memoria unificada, e o acelerador e o Metal.
        let apple = self.arquitetura == "aarch64";
        self.memoria_unificada = apple;
        if apple {
            self.acelerador = Some("Metal".to_string());
        }
        // Livre: paginas livres x tamanho da pagina, os dois medidos.
        if let (Some(vm), Some(pagina)) = (
            comando("vm_stat", &[]),
            comando("sysctl", &["-n", "hw.pagesize"]).and_then(|s| s.parse::<u64>().ok()),
        ) {
            for linha in vm.lines() {
                if linha.starts_with("Pages free:") {
                    let n: String = linha.chars().filter(|c| c.is_ascii_digit()).collect();
                    if let Ok(paginas) = n.parse::<u64>() {
                        self.memoria_livre_bytes = Some(paginas * pagina);
                    }
                }
            }
        }
    }

    fn medir_linux(&mut self) {
        self.processador = campo_proc("/proc/cpuinfo", "model name")
            .or_else(|| campo_proc("/proc/cpuinfo", "Model"));
        self.nucleos = fs::read_to_string("/proc/cpuinfo")
            .ok()
            .map(|t| t.lines().filter(|l| l.starts_with("processor")).count() as u32)
            .filter(|n| *n > 0);
        self.memoria_bytes = meminfo_kb("MemTotal:");
        self.memoria_livre_bytes = meminfo_kb("MemAvailable:");
        // Placa de video: so afirma o que o sistema respondeu.
        if fs::metadata("/proc/driver/nvidia/version").is_ok() {
            self.acelerador = comando("nvidia-smi", &["--query-gpu=name", "--format=csv,noheader"])
                .map(|s| s.lines().next().unwrap_or("NVIDIA").to_string())
                .or(Some("NVIDIA".to_string()));
        } else if fs::metadata("/dev/kfd").is_ok() {
            self.acelerador = Some("ROCm".to_string());
        }
    }

    /// Quanto da memoria da para entregar a um modelo sem estrangular o resto.
    /// Regra explicita, para nao virar magica: 70% do que esta LIVRE agora, ou
    /// 60% do total quando o livre nao se mede.
    pub fn orcamento_de_memoria(&self) -> Option<u64> {
        if let Some(livre) = self.memoria_livre_bytes {
            return Some(livre * 7 / 10);
        }
        self.memoria_bytes.map(|total| total * 6 / 10)
    }
}

pub fn gb(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}
