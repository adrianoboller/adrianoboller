//! Monitor de hardware do hospedeiro, lido do SO em Rust puro -- sem crate de terceiro para
//! o sensor, como o SHA-256 da casa e escrito e nao importado. No Linux le `/proc` e `/sys`;
//! a raiz e parametro (`de_raiz`) para a prova rodar contra uma arvore de mentira em tempdir,
//! o mesmo molde do `LeitorEnergia` (`avaliacao.rs`).
//!
//! CPU% e taxa sao DELTA entre duas leituras -- uma leitura so nao tem com o que comparar --,
//! entao a API e `amostra()` + `entre(antes, depois)`, nunca medir CPU% de uma amostra. Cada
//! campo e `Leitura::Medida(valor)` ou `Leitura::NaoMedida(motivo)`: nunca um zero mudo, nunca
//! uma estimativa.
//!
//! **Espaco em disco** vem de `statvfs` (Linux) e `GetDiskFreeSpaceExW` (Windows): sao FFI, e
//! FFI e `unsafe`. O unsafe fica CONFINADO em `statvfs_em` e no `mod win`, cada um com o seu
//! `#[allow(unsafe_code)]` estreito -- o crate e `#![deny(unsafe_code)]` (decisao do dono,
//! 2026-10-10). A aritmetica do disco e PURA (`calcular_disco`), fora do unsafe, para ser
//! testada com numeros conhecidos; o unsafe so enche a struct da syscall.
//!
//! O que NAO entra neste corte, e o porque de cada um:
//! - **Temperatura e ventoinha no Windows** pedem driver/LHM -- `NaoMedida`, camada 2.
//! - **Carga no Windows** e `NaoMedida` porque o Windows nao expoe load average, e emular
//!   seria inventar numero.
//!
//! A leitura nativa do Windows (`win`) usa FFI direto a Win32 por `extern "system"`, sem crate
//! de terceiro. Ela e `#[cfg(windows)]` e nao compila neste alvo Linux.

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ------------------------------------------------------------------ limiares da saude
//
// Todos sao CONSTANTES nomeadas e sao PALPITE A CONFIRMAR NA BANCADA: a origem esta no
// comentario de cada um; nenhum saiu de medicao nossa ainda.

/// Temperatura sem `temp*_crit` do chip: ~85 C de atencao, ~95 C de critico. Palpite a
/// confirmar na bancada (faixa tipica de Tjunction de CPU de mesa).
pub const TEMP_ATENCAO_C: f64 = 85.0;
pub const TEMP_CRITICO_C: f64 = 95.0;
/// Com `temp*_crit` do chip, a atencao comeca 15 C abaixo do critico e o critico 5 C abaixo:
/// o fabricante sabe o teto melhor que um numero fixo. Palpite a confirmar na bancada.
pub const TEMP_MARGEM_ATENCAO_C: f64 = 15.0;
pub const TEMP_MARGEM_CRITICO_C: f64 = 5.0;

/// Disco: atencao com menos de 10% OU menos de 10 GiB livres; critico com menos de 5% OU
/// menos de 2 GiB. Palpite a confirmar na bancada.
pub const DISCO_ATENCAO_LIVRE_PCT: f64 = 10.0;
pub const DISCO_CRITICO_LIVRE_PCT: f64 = 5.0;
pub const DISCO_ATENCAO_LIVRE_BYTES: u64 = 10 * 1024 * 1024 * 1024;
pub const DISCO_CRITICO_LIVRE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Memoria: atencao com mais de 85% usado, critico com mais de 95%. Palpite a confirmar na
/// bancada.
pub const MEM_ATENCAO_USADO_PCT: f64 = 85.0;
pub const MEM_CRITICO_USADO_PCT: f64 = 95.0;

/// Carga normalizada pelo numero de CPUs: atencao acima de 0,70xN, critico acima de 1,0xN (a
/// fila ja passou do que os nucleos atendem). Palpite a confirmar na bancada.
pub const CARGA_ATENCAO_POR_CPU: f64 = 0.70;
pub const CARGA_CRITICO_POR_CPU: f64 = 1.0;

/// Intervalo entre as duas amostras do CPU% na CLI e na rota. Curto porque e a diferenca de
/// contadores monotonos, nao uma media de janela longa.
pub const INTERVALO_MS: u64 = 300;

// ------------------------------------------------------------------ uma leitura

/// Um valor medido, ou o motivo de nao ter sido medido -- nunca um zero mudo nem uma
/// estimativa. O mesmo molde do `avaliacao::Energia`/`Medida`.
#[derive(Debug, Clone, PartialEq)]
pub enum Leitura<T> {
    Medida(T),
    NaoMedida(String),
}

impl<T> Leitura<T> {
    /// O valor, quando medido.
    pub fn medida(&self) -> Option<&T> {
        match self {
            Leitura::Medida(v) => Some(v),
            Leitura::NaoMedida(_) => None,
        }
    }
}

// ------------------------------------------------------------------ estruturas do painel

#[derive(Debug, Clone, PartialEq)]
pub struct Memoria {
    pub total: Leitura<u64>,
    pub disponivel: Leitura<u64>,
    pub usado: Leitura<u64>,
    pub percent: Leitura<f64>,
    /// Qual campo deu o disponivel: `MemAvailable` (o certo) ou o fallback de kernel antigo.
    pub fonte_disponivel: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Swap {
    pub total: Leitura<u64>,
    pub livre: Leitura<u64>,
    pub usado: Leitura<u64>,
    pub percent: Leitura<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disco {
    pub montagem: String,
    pub total: u64,
    /// Livre para o usuario (`f_bavail`), nao o livre total que inclui o reservado do root.
    pub livre: u64,
    pub usado: u64,
    pub percent: f64,
}

/// A aritmetica do disco, separada da syscall para ser PURA e testavel (o unsafe de `statvfs`
/// so enche a struct; a conta fica aqui, fora dele).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscoCalc {
    pub total: u64,
    pub livre: u64,
    pub usado: u64,
    pub percent: f64,
}

/// `total = f_blocks*f_frsize`; `livre` e o do usuario (`f_bavail*f_frsize`); `usado` desconta
/// o livre TOTAL (`f_bfree`, que inclui o reservado do root) -- senao o reservado apareceria
/// como usado. `f_frsize` (fragmento) e a unidade dos contadores, nao `f_bsize` (bloco de I/O).
pub fn calcular_disco(f_blocks: u64, f_bfree: u64, f_bavail: u64, f_frsize: u64) -> DiscoCalc {
    let total = f_blocks.saturating_mul(f_frsize);
    let livre = f_bavail.saturating_mul(f_frsize);
    let usado = total.saturating_sub(f_bfree.saturating_mul(f_frsize));
    let percent = if usado + livre == 0 {
        0.0
    } else {
        usado as f64 / (usado + livre) as f64 * 100.0
    };
    DiscoCalc {
        total,
        livre,
        usado,
        percent,
    }
}

/// Espaco em disco de uma montagem pelo caminho. No Linux chama `statvfs`; fora do Linux fica
/// `NaoMedida` (a struct do `statvfs` varia de ABI, e este corte so provou a do Linux).
#[cfg(target_os = "linux")]
pub fn disco_em(caminho: &Path) -> Leitura<Disco> {
    match statvfs_em(caminho) {
        Some((blocks, bfree, bavail, frsize)) if blocks > 0 && frsize > 0 => {
            let c = calcular_disco(blocks, bfree, bavail, frsize);
            Leitura::Medida(Disco {
                montagem: caminho.to_string_lossy().into_owned(),
                total: c.total,
                livre: c.livre,
                usado: c.usado,
                percent: c.percent,
            })
        }
        _ => Leitura::NaoMedida(format!("statvfs falhou em {}", caminho.display())),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn disco_em(_caminho: &Path) -> Leitura<Disco> {
    Leitura::NaoMedida("statvfs nativo so no Linux neste corte".into())
}

/// A UNICA syscall unsafe do caminho Linux, confinada e auditada: `statvfs` enche a struct e a
/// gente copia os quatro numeros para a conta pura. O `#[allow(unsafe_code)]` cobre so esta
/// funcao -- o crate e `#![deny(unsafe_code)]`.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
// `u64::from` em c_ulong e inofensivo em 64 bits (c_ulong == u64) mas deixa a conta correta se
// um dia compilar em 32 bits, onde c_ulong e u32 -- por isso o `useless_conversion` e tolerado.
#[allow(clippy::useless_conversion)]
fn statvfs_em(caminho: &Path) -> Option<(u64, u64, u64, u64)> {
    use std::ffi::{CString, c_char, c_int, c_ulong};
    use std::os::unix::ffi::OsStrExt;

    // Layout do `struct statvfs` da glibc em 64 bits. So os quatro campos marcados sao lidos;
    // os demais existem para o OFFSET bater com o que o kernel preenche.
    #[repr(C)]
    #[allow(dead_code)]
    struct Statvfs {
        f_bsize: c_ulong,
        f_frsize: c_ulong,
        f_blocks: c_ulong,
        f_bfree: c_ulong,
        f_bavail: c_ulong,
        f_files: c_ulong,
        f_ffree: c_ulong,
        f_favail: c_ulong,
        f_fsid: c_ulong,
        f_flag: c_ulong,
        f_namemax: c_ulong,
        f_spare: [c_int; 6],
    }

    unsafe extern "C" {
        fn statvfs(caminho: *const c_char, buf: *mut Statvfs) -> c_int;
    }

    let c = CString::new(caminho.as_os_str().as_bytes()).ok()?;
    let mut buf = std::mem::MaybeUninit::<Statvfs>::zeroed();
    // SAFETY: `c` vive ate o fim da chamada e e NUL-terminada; `buf` aponta para memoria valida
    // e alinhada desta pilha; `statvfs` so le a string ate o NUL e so escreve em `buf`, sem
    // reter ponteiro. O rc e conferido antes de ler a struct.
    let rc = unsafe { statvfs(c.as_ptr(), buf.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    // SAFETY: rc == 0, entao o kernel inicializou a struct inteira.
    let s = unsafe { buf.assume_init() };
    Some((
        u64::from(s.f_blocks),
        u64::from(s.f_bfree),
        u64::from(s.f_bavail),
        u64::from(s.f_frsize),
    ))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperatura {
    /// O `name` do chip (`coretemp`, `k10temp`, `acpitz`...).
    pub chip: String,
    pub rotulo: Option<String>,
    pub celsius: f64,
    /// O critico do proprio chip (`temp*_crit`), quando ele o informa.
    pub critico: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ventoinha {
    pub rotulo: Option<String>,
    /// RPM cru, sem divisor -- o `fan*_input` ja e em rotacoes por minuto.
    pub rpm: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Saude {
    Ok,
    Atencao(Vec<String>),
    Critico(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct Hardware {
    pub cpu_total_pct: Leitura<f64>,
    pub cpu_por_nucleo: Vec<Leitura<f64>>,
    pub ncpu: Leitura<usize>,
    pub carga: Leitura<[f64; 3]>,
    pub memoria: Memoria,
    pub swap: Swap,
    pub disco: Leitura<Disco>,
    pub uptime: Leitura<f64>,
    pub temperaturas: Vec<Temperatura>,
    pub ventoinhas: Vec<Ventoinha>,
    pub frequencias: Vec<Leitura<f64>>,
    pub saude: Saude,
}

// ------------------------------------------------------------------ CPU: jiffies e delta

/// Os campos de uma linha `cpu`/`cpuN` do `/proc/stat`, em jiffies (USER_HZ). `user` e `nice`
/// ja incluem `guest`/`guest_nice` -- o kernel os conta duas vezes de proposito, e e por isso
/// que o total os subtrai.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Jiffies {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    guest: u64,
    guest_nice: u64,
}

impl Jiffies {
    /// Os quatro primeiros campos (user, nice, system, idle) sao obrigatorios; os demais sao
    /// opcionais porque kernels antigos trazem menos colunas.
    fn parse(campos: &[&str]) -> Option<Jiffies> {
        let n = |i: usize| campos.get(i).and_then(|c| c.parse::<u64>().ok());
        Some(Jiffies {
            user: n(0)?,
            nice: n(1)?,
            system: n(2)?,
            idle: n(3)?,
            iowait: n(4).unwrap_or(0),
            irq: n(5).unwrap_or(0),
            softirq: n(6).unwrap_or(0),
            steal: n(7).unwrap_or(0),
            guest: n(8).unwrap_or(0),
            guest_nice: n(9).unwrap_or(0),
        })
    }
}

/// CPU% entre duas leituras, pela conta do psutil/kernel:
/// `total = soma_todos - guest - guest_nice` (guest ja conta em user/nice; subtrair evita
/// contar duas vezes), `busy = total - idle - iowait`, `cpu% = busy_delta/total_delta*100`.
/// Cada delta de campo e clampado a >=0 (o contador recua em hotplug); total zero da 0,0.
fn pct_cpu(a: &Jiffies, b: &Jiffies) -> f64 {
    let d = |x: u64, y: u64| x.saturating_sub(y);
    let user = d(b.user, a.user);
    let nice = d(b.nice, a.nice);
    let system = d(b.system, a.system);
    let idle = d(b.idle, a.idle);
    let iowait = d(b.iowait, a.iowait);
    let irq = d(b.irq, a.irq);
    let softirq = d(b.softirq, a.softirq);
    let steal = d(b.steal, a.steal);
    // guest/guest_nice ficam de fora do total porque ja estao dentro de user/nice.
    let total = user + nice + system + idle + iowait + irq + softirq + steal;
    let busy = user + nice + system + irq + softirq + steal;
    if total == 0 {
        0.0
    } else {
        busy as f64 / total as f64 * 100.0
    }
}

/// A linha `cpu` agregada e as linhas `cpuN` por nucleo, na ordem do arquivo.
fn ler_stat(proc: &Path) -> (Option<Jiffies>, Vec<Jiffies>) {
    let t = std::fs::read_to_string(proc.join("stat")).unwrap_or_default();
    let mut total = None;
    let mut nucleos = Vec::new();
    for linha in t.lines() {
        let mut it = linha.split_whitespace();
        let Some(primeiro) = it.next() else {
            continue;
        };
        let resto: Vec<&str> = it.collect();
        if primeiro == "cpu" {
            total = Jiffies::parse(&resto);
        } else if let Some(idx) = primeiro.strip_prefix("cpu")
            && !idx.is_empty()
            && idx.chars().all(|c| c.is_ascii_digit())
            && let Some(j) = Jiffies::parse(&resto)
        {
            nucleos.push(j);
        }
    }
    (total, nucleos)
}

// ------------------------------------------------------------------ meminfo

/// `/proc/meminfo` com os valores ja em bytes (os campos vem em kB; `x1024`).
fn ler_meminfo(proc: &Path) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    let Ok(t) = std::fs::read_to_string(proc.join("meminfo")) else {
        return m;
    };
    for l in t.lines() {
        let Some((chave, resto)) = l.split_once(':') else {
            continue;
        };
        let resto = resto.trim();
        let Some(num) = resto
            .split_whitespace()
            .next()
            .and_then(|n| n.parse::<u64>().ok())
        else {
            continue;
        };
        // Os campos que o painel usa vem todos em kB; os poucos sem unidade que sobram nao sao
        // lidos, entao multiplicar sempre e seguro para o que importa.
        let bytes = if resto.ends_with("kB") {
            num.saturating_mul(1024)
        } else {
            num
        };
        m.insert(chave.trim().to_string(), bytes);
    }
    m
}

fn montar_memoria(m: &BTreeMap<String, u64>) -> Memoria {
    let total = m.get("MemTotal").copied();
    // MemAvailable e o numero certo; MemFree superestima a pressao porque ignora o cache
    // recuperavel. Sem ele (kernel < 3.14), o fallback aproxima e o motivo diz qual veio.
    let (disp, fonte) = match m.get("MemAvailable").copied() {
        Some(a) => (Some(a), "MemAvailable".to_string()),
        None => match (m.get("MemFree"), m.get("Cached"), m.get("SReclaimable")) {
            (Some(f), Some(c), Some(s)) => (
                Some(f + c + s),
                "MemFree+Cached+SReclaimable (sem MemAvailable; kernel < 3.14)".to_string(),
            ),
            _ => (None, "sem MemAvailable e sem fallback".to_string()),
        },
    };
    let usado = total.zip(disp).map(|(t, d)| t.saturating_sub(d));
    let percent = total
        .zip(usado)
        .and_then(|(t, u)| (t > 0).then(|| u as f64 / t as f64 * 100.0));
    Memoria {
        total: ou_sem(total, "sem MemTotal em /proc/meminfo"),
        disponivel: ou_sem(disp, &fonte),
        usado: ou_sem(usado, "sem MemTotal/MemAvailable"),
        percent: ou_sem(percent, "sem MemTotal/MemAvailable"),
        fonte_disponivel: fonte,
    }
}

fn montar_swap(m: &BTreeMap<String, u64>) -> Swap {
    let total = m.get("SwapTotal").copied();
    let livre = m.get("SwapFree").copied();
    let usado = total.zip(livre).map(|(t, l)| t.saturating_sub(l));
    // Sem swap (SwapTotal 0) o percent e 0 de verdade, nao "nao medido".
    let percent = total.zip(usado).map(|(t, u)| {
        if t == 0 {
            0.0
        } else {
            u as f64 / t as f64 * 100.0
        }
    });
    Swap {
        total: ou_sem(total, "sem SwapTotal em /proc/meminfo"),
        livre: ou_sem(livre, "sem SwapFree em /proc/meminfo"),
        usado: ou_sem(usado, "sem SwapTotal/SwapFree"),
        percent: ou_sem(percent, "sem SwapTotal/SwapFree"),
    }
}

fn ou_sem<T>(v: Option<T>, motivo: &str) -> Leitura<T> {
    match v {
        Some(v) => Leitura::Medida(v),
        None => Leitura::NaoMedida(motivo.to_string()),
    }
}

// ------------------------------------------------------------------ loadavg, uptime

fn ler_loadavg(proc: &Path) -> Leitura<[f64; 3]> {
    let Ok(t) = std::fs::read_to_string(proc.join("loadavg")) else {
        return Leitura::NaoMedida("sem /proc/loadavg".into());
    };
    let c: Vec<f64> = t
        .split_whitespace()
        .take(3)
        .filter_map(|x| x.parse().ok())
        .collect();
    if c.len() == 3 {
        Leitura::Medida([c[0], c[1], c[2]])
    } else {
        Leitura::NaoMedida("/proc/loadavg sem os tres campos".into())
    }
}

fn ler_uptime(proc: &Path) -> Leitura<f64> {
    match std::fs::read_to_string(proc.join("uptime"))
        .ok()
        .and_then(|t| t.split_whitespace().next()?.parse::<f64>().ok())
    {
        Some(s) => Leitura::Medida(s),
        None => Leitura::NaoMedida("sem /proc/uptime".into()),
    }
}

// ------------------------------------------------------------------ hwmon: temperatura/ventoinha

/// O miolo de um arquivo de sensor: `temp<N>_input` -> `Some("N")`.
fn indice(nome: &str, pre: &str, suf: &str) -> Option<String> {
    let meio = nome.strip_prefix(pre)?.strip_suffix(suf)?;
    (!meio.is_empty() && meio.chars().all(|c| c.is_ascii_digit())).then(|| meio.to_string())
}

fn ler_f64(p: &Path) -> Option<f64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

fn ler_texto(p: &Path) -> Option<String> {
    std::fs::read_to_string(p)
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Nomes de arquivo de um diretorio, ordenados -- a ordem do `read_dir` nao e estavel.
fn arquivos_ordenados(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// Temperaturas e ventoinhas de `/sys/class/hwmon/hwmon*`. Os `temp*_input`/`fan*_input` sao
/// MILIGRAUS e RPM: a temperatura divide por 1000, a ventoinha e crua.
fn ler_hwmon(sys: &Path) -> (Vec<Temperatura>, Vec<Ventoinha>) {
    let mut temps = Vec::new();
    let mut fans = Vec::new();
    let base = sys.join("class/hwmon");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for dir in &dirs {
        let chip = ler_texto(&dir.join("name")).unwrap_or_default();
        for nome in arquivos_ordenados(dir) {
            if let Some(n) = indice(&nome, "temp", "_input")
                && let Some(mc) = ler_f64(&dir.join(&nome))
            {
                temps.push(Temperatura {
                    chip: chip.clone(),
                    rotulo: ler_texto(&dir.join(format!("temp{n}_label"))),
                    celsius: mc / 1000.0,
                    critico: ler_f64(&dir.join(format!("temp{n}_crit"))).map(|c| c / 1000.0),
                });
            } else if let Some(n) = indice(&nome, "fan", "_input")
                && let Some(rpm) = ler_f64(&dir.join(&nome))
            {
                fans.push(Ventoinha {
                    rotulo: ler_texto(&dir.join(format!("fan{n}_label"))),
                    rpm,
                });
            }
        }
    }
    // Fallback: sem hwmon, as zonas termicas da ACPI (tambem miligraus).
    if temps.is_empty() {
        let base = sys.join("class/thermal");
        let mut zonas: Vec<PathBuf> = std::fs::read_dir(&base)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("thermal_zone"))
            })
            .collect();
        zonas.sort();
        for z in zonas {
            if let Some(mc) = ler_f64(&z.join("temp")) {
                temps.push(Temperatura {
                    chip: ler_texto(&z.join("type")).unwrap_or_default(),
                    rotulo: None,
                    celsius: mc / 1000.0,
                    critico: None,
                });
            }
        }
    }
    (temps, fans)
}

/// Frequencia corrente de cada politica de cpufreq, em MHz (o arquivo e kHz).
fn ler_frequencias(sys: &Path) -> Vec<Leitura<f64>> {
    let base = sys.join("devices/system/cpu/cpufreq");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("policy"))
        })
        .collect();
    dirs.sort();
    dirs.iter()
        .map(|d| match ler_f64(&d.join("scaling_cur_freq")) {
            Some(khz) => Leitura::Medida(khz / 1000.0),
            None => Leitura::NaoMedida("sem scaling_cur_freq".into()),
        })
        .collect()
}

// ------------------------------------------------------------------ amostra e montagem

/// Uma leitura instantanea dos arquivos. Guarda os jiffies do CPU (para o delta) e o resto do
/// painel; o `entre` usa o CPU das duas amostras e o resto da amostra de DEPOIS.
#[derive(Debug, Clone)]
struct SnapArquivos {
    cpu_total: Option<Jiffies>,
    cpu_nucleos: Vec<Jiffies>,
    meminfo: BTreeMap<String, u64>,
    carga: Leitura<[f64; 3]>,
    uptime: Leitura<f64>,
    temperaturas: Vec<Temperatura>,
    ventoinhas: Vec<Ventoinha>,
    frequencias: Vec<Leitura<f64>>,
}

impl SnapArquivos {
    fn ler(proc: &Path, sys: &Path) -> SnapArquivos {
        let (cpu_total, cpu_nucleos) = ler_stat(proc);
        let (temperaturas, ventoinhas) = ler_hwmon(sys);
        SnapArquivos {
            cpu_total,
            cpu_nucleos,
            meminfo: ler_meminfo(proc),
            carga: ler_loadavg(proc),
            uptime: ler_uptime(proc),
            temperaturas,
            ventoinhas,
            frequencias: ler_frequencias(sys),
        }
    }
}

/// O instante guardado por `amostra`. O conteudo depende da fonte (arquivos ou Win32), mas o
/// `Hardware` que sai de `entre` e o mesmo para as duas.
#[derive(Debug, Clone)]
enum Instante {
    Arquivos(SnapArquivos),
    #[cfg(windows)]
    Windows(win::Snap),
}

#[derive(Debug, Clone)]
pub struct AmostraHardware(Instante);

#[derive(Debug, Clone)]
enum Fonte {
    Arquivos {
        proc: PathBuf,
        sys: PathBuf,
        /// A montagem a medir com `statvfs`. `None` na porta da prova (`de_raiz`): o disco e
        /// o real da maquina, nao cabe numa arvore de mentira, entao ali ele fica `NaoMedida`.
        disco: Option<PathBuf>,
    },
    #[cfg(windows)]
    Windows,
}

pub struct LeitorHardware {
    fonte: Fonte,
}

impl LeitorHardware {
    /// O leitor do sistema: no Linux, `/proc` e `/sys` (e o disco do diretorio corrente); no
    /// Windows, a FFI Win32.
    pub fn do_sistema() -> LeitorHardware {
        #[cfg(windows)]
        {
            LeitorHardware {
                fonte: Fonte::Windows,
            }
        }
        #[cfg(not(windows))]
        {
            let disco = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
            LeitorHardware {
                fonte: Fonte::Arquivos {
                    proc: PathBuf::from("/proc"),
                    sys: PathBuf::from("/sys"),
                    disco: Some(disco),
                },
            }
        }
    }

    /// Le de uma raiz dada -- a porta da prova, que monta um `/proc`+`/sys` de mentira. O disco
    /// NAO se finge (e o real da maquina): fica `NaoMedida` por aqui.
    pub fn de_raiz(proc: &Path, sys: &Path) -> LeitorHardware {
        LeitorHardware {
            fonte: Fonte::Arquivos {
                proc: proc.to_path_buf(),
                sys: sys.to_path_buf(),
                disco: None,
            },
        }
    }

    pub fn amostra(&self) -> AmostraHardware {
        match &self.fonte {
            Fonte::Arquivos { proc, sys, .. } => {
                AmostraHardware(Instante::Arquivos(SnapArquivos::ler(proc, sys)))
            }
            #[cfg(windows)]
            Fonte::Windows => AmostraHardware(Instante::Windows(win::amostrar())),
        }
    }

    /// O painel entre duas amostras: CPU% do delta das duas, o resto do estado de DEPOIS.
    pub fn entre(&self, antes: &AmostraHardware, depois: &AmostraHardware) -> Hardware {
        match (&antes.0, &depois.0) {
            (Instante::Arquivos(a), Instante::Arquivos(b)) => {
                let disco = match &self.fonte {
                    Fonte::Arquivos { disco: Some(p), .. } => disco_em(p),
                    _ => Leitura::NaoMedida(
                        "a porta de prova (de_raiz) nao mede o disco real".into(),
                    ),
                };
                montar_arquivos(a, b, disco)
            }
            #[cfg(windows)]
            (Instante::Windows(a), Instante::Windows(b)) => win::montar(a, b),
            #[cfg(windows)]
            _ => Hardware::vazio("amostras de fontes diferentes"),
        }
    }
}

/// Monta o painel a partir de duas leituras de arquivos e do disco ja lido pelo chamador.
fn montar_arquivos(a: &SnapArquivos, b: &SnapArquivos, disco: Leitura<Disco>) -> Hardware {
    let cpu_total_pct = match (a.cpu_total, b.cpu_total) {
        (Some(a), Some(b)) => Leitura::Medida(pct_cpu(&a, &b)),
        _ => Leitura::NaoMedida("sem a linha cpu em /proc/stat".into()),
    };
    // Por nucleo pelo indice; nucleo que apareceu/sumiu entre as amostras (hotplug) nao tem par.
    let n = a.cpu_nucleos.len().min(b.cpu_nucleos.len());
    let mut cpu_por_nucleo: Vec<Leitura<f64>> = (0..n)
        .map(|i| Leitura::Medida(pct_cpu(&a.cpu_nucleos[i], &b.cpu_nucleos[i])))
        .collect();
    for _ in n..b.cpu_nucleos.len() {
        cpu_por_nucleo.push(Leitura::NaoMedida(
            "nucleo sem par entre as amostras".into(),
        ));
    }
    let ncpu = if b.cpu_nucleos.is_empty() {
        Leitura::NaoMedida("sem linhas cpuN em /proc/stat".into())
    } else {
        Leitura::Medida(b.cpu_nucleos.len())
    };
    let memoria = montar_memoria(&b.meminfo);
    let swap = montar_swap(&b.meminfo);
    let mut h = Hardware {
        cpu_total_pct,
        cpu_por_nucleo,
        ncpu,
        carga: b.carga.clone(),
        memoria,
        swap,
        disco,
        uptime: b.uptime.clone(),
        temperaturas: b.temperaturas.clone(),
        ventoinhas: b.ventoinhas.clone(),
        frequencias: b.frequencias.clone(),
        saude: Saude::Ok,
    };
    h.saude = saude(&h);
    h
}

impl Hardware {
    /// Painel todo nao medido, para o caso que nao se deve alcancar (amostras de fontes
    /// diferentes). Mantem o `entre` total em vez de entrar em panico.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn vazio(motivo: &str) -> Hardware {
        let m = motivo.to_string();
        Hardware {
            cpu_total_pct: Leitura::NaoMedida(m.clone()),
            cpu_por_nucleo: vec![],
            ncpu: Leitura::NaoMedida(m.clone()),
            carga: Leitura::NaoMedida(m.clone()),
            memoria: Memoria {
                total: Leitura::NaoMedida(m.clone()),
                disponivel: Leitura::NaoMedida(m.clone()),
                usado: Leitura::NaoMedida(m.clone()),
                percent: Leitura::NaoMedida(m.clone()),
                fonte_disponivel: m.clone(),
            },
            swap: Swap {
                total: Leitura::NaoMedida(m.clone()),
                livre: Leitura::NaoMedida(m.clone()),
                usado: Leitura::NaoMedida(m.clone()),
                percent: Leitura::NaoMedida(m.clone()),
            },
            disco: Leitura::NaoMedida(m.clone()),
            uptime: Leitura::NaoMedida(m),
            temperaturas: vec![],
            ventoinhas: vec![],
            frequencias: vec![],
            saude: Saude::Ok,
        }
    }
}

// ------------------------------------------------------------------ veredito de saude

/// O veredito, cada motivo nomeando o numero que disparou. Metrica nao medida nao julga (nem
/// Ok nem alarme): nao da para dizer saude do que nao se leu.
pub fn saude(h: &Hardware) -> Saude {
    let mut atencao: Vec<String> = Vec::new();
    let mut critico: Vec<String> = Vec::new();

    for t in &h.temperaturas {
        let (lim_at, lim_cr) = match t.critico {
            Some(c) => (c - TEMP_MARGEM_ATENCAO_C, c - TEMP_MARGEM_CRITICO_C),
            None => (TEMP_ATENCAO_C, TEMP_CRITICO_C),
        };
        let rot = t.rotulo.clone().unwrap_or_else(|| t.chip.clone());
        if t.celsius >= lim_cr {
            critico.push(format!(
                "temperatura {rot}: {:.1} C >= {lim_cr:.1} C",
                t.celsius
            ));
        } else if t.celsius >= lim_at {
            atencao.push(format!(
                "temperatura {rot}: {:.1} C >= {lim_at:.1} C",
                t.celsius
            ));
        }
    }

    if let Leitura::Medida(d) = &h.disco {
        let livre_pct = 100.0 - d.percent;
        if livre_pct < DISCO_CRITICO_LIVRE_PCT || d.livre < DISCO_CRITICO_LIVRE_BYTES {
            critico.push(format!(
                "disco {}: {livre_pct:.1}% / {} livres",
                d.montagem,
                humano(d.livre)
            ));
        } else if livre_pct < DISCO_ATENCAO_LIVRE_PCT || d.livre < DISCO_ATENCAO_LIVRE_BYTES {
            atencao.push(format!(
                "disco {}: {livre_pct:.1}% / {} livres",
                d.montagem,
                humano(d.livre)
            ));
        }
    }

    if let Leitura::Medida(p) = &h.memoria.percent {
        if *p > MEM_CRITICO_USADO_PCT {
            critico.push(format!(
                "memoria: {p:.1}% usada > {MEM_CRITICO_USADO_PCT:.0}%"
            ));
        } else if *p > MEM_ATENCAO_USADO_PCT {
            atencao.push(format!(
                "memoria: {p:.1}% usada > {MEM_ATENCAO_USADO_PCT:.0}%"
            ));
        }
    }

    if let (Leitura::Medida(c), Leitura::Medida(n)) = (&h.carga, &h.ncpu)
        && *n > 0
    {
        let um = c[0];
        let nf = *n as f64;
        if um > CARGA_CRITICO_POR_CPU * nf {
            critico.push(format!(
                "carga 1min {um:.2} > {:.2} ({n} CPU x {CARGA_CRITICO_POR_CPU:.2})",
                CARGA_CRITICO_POR_CPU * nf
            ));
        } else if um > CARGA_ATENCAO_POR_CPU * nf {
            atencao.push(format!(
                "carga 1min {um:.2} > {:.2} ({n} CPU x {CARGA_ATENCAO_POR_CPU:.2})",
                CARGA_ATENCAO_POR_CPU * nf
            ));
        }
    }

    if !critico.is_empty() {
        // Os de atencao viajam junto, para o painel nao esconder o que ainda nao e critico.
        critico.extend(atencao);
        Saude::Critico(critico)
    } else if !atencao.is_empty() {
        Saude::Atencao(atencao)
    } else {
        Saude::Ok
    }
}

fn humano(bytes: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", U[i])
}

// ------------------------------------------------------------------ saida: JSON e texto

fn jl_f64(l: &Leitura<f64>) -> Value {
    match l {
        Leitura::Medida(v) => json!(v),
        Leitura::NaoMedida(m) => json!({ "nao_medida": m }),
    }
}

fn jl_u64(l: &Leitura<u64>) -> Value {
    match l {
        Leitura::Medida(v) => json!(v),
        Leitura::NaoMedida(m) => json!({ "nao_medida": m }),
    }
}

/// O painel em JSON, do jeito que a rota `GET /v1/hardware` devolve.
pub fn para_json(h: &Hardware) -> Value {
    let carga = match &h.carga {
        Leitura::Medida(c) => json!([c[0], c[1], c[2]]),
        Leitura::NaoMedida(m) => json!({ "nao_medida": m }),
    };
    let ncpu = match &h.ncpu {
        Leitura::Medida(n) => json!(n),
        Leitura::NaoMedida(m) => json!({ "nao_medida": m }),
    };
    let disco = match &h.disco {
        Leitura::Medida(d) => json!({
            "montagem": d.montagem,
            "total": d.total,
            "livre": d.livre,
            "usado": d.usado,
            "percent": d.percent,
        }),
        Leitura::NaoMedida(m) => json!({ "nao_medida": m }),
    };
    let temperaturas: Vec<Value> = h
        .temperaturas
        .iter()
        .map(|t| {
            json!({
                "chip": t.chip,
                "rotulo": t.rotulo,
                "celsius": t.celsius,
                "critico": t.critico,
            })
        })
        .collect();
    let ventoinhas: Vec<Value> = h
        .ventoinhas
        .iter()
        .map(|v| json!({ "rotulo": v.rotulo, "rpm": v.rpm }))
        .collect();
    let frequencias: Vec<Value> = h.frequencias.iter().map(jl_f64).collect();
    let saude = match &h.saude {
        Saude::Ok => json!({ "estado": "ok", "motivos": [] }),
        Saude::Atencao(m) => json!({ "estado": "atencao", "motivos": m }),
        Saude::Critico(m) => json!({ "estado": "critico", "motivos": m }),
    };
    json!({
        "cpu": {
            "total": jl_f64(&h.cpu_total_pct),
            "por_nucleo": h.cpu_por_nucleo.iter().map(jl_f64).collect::<Vec<_>>(),
            "ncpu": ncpu,
        },
        "carga": carga,
        "memoria": {
            "total": jl_u64(&h.memoria.total),
            "disponivel": jl_u64(&h.memoria.disponivel),
            "usado": jl_u64(&h.memoria.usado),
            "percent": jl_f64(&h.memoria.percent),
            "fonte_disponivel": h.memoria.fonte_disponivel,
        },
        "swap": {
            "total": jl_u64(&h.swap.total),
            "livre": jl_u64(&h.swap.livre),
            "usado": jl_u64(&h.swap.usado),
            "percent": jl_f64(&h.swap.percent),
        },
        "disco": disco,
        "uptime": jl_f64(&h.uptime),
        "temperaturas": temperaturas,
        "ventoinhas": ventoinhas,
        "frequencias": frequencias,
        "saude": saude,
    })
}

/// O painel em texto, do jeito que a CLI `phxclaw hardware` imprime.
pub fn texto(h: &Hardware) -> String {
    let pct = |l: &Leitura<f64>| match l {
        Leitura::Medida(v) => format!("{v:.1}%"),
        Leitura::NaoMedida(m) => format!("nao medida ({m})"),
    };
    let mut s = String::from("monitor de hardware\n");
    s.push_str(&format!("  CPU          : {}\n", pct(&h.cpu_total_pct)));
    let nucleos: Vec<String> = h
        .cpu_por_nucleo
        .iter()
        .enumerate()
        .map(|(i, l)| format!("cpu{i} {}", pct(l)))
        .collect();
    if !nucleos.is_empty() {
        s.push_str(&format!("  por nucleo   : {}\n", nucleos.join("  ")));
    }
    match &h.carga {
        Leitura::Medida(c) => {
            s.push_str(&format!(
                "  carga        : {:.2} {:.2} {:.2}",
                c[0], c[1], c[2]
            ));
            if let Leitura::Medida(n) = &h.ncpu {
                s.push_str(&format!(" ({n} CPU)"));
            }
            s.push('\n');
        }
        Leitura::NaoMedida(m) => s.push_str(&format!("  carga        : nao medida ({m})\n")),
    }
    s.push_str(&format!(
        "  memoria      : {} / {} ({})\n",
        match &h.memoria.usado {
            Leitura::Medida(v) => humano(*v),
            Leitura::NaoMedida(m) => format!("nao medida ({m})"),
        },
        match &h.memoria.total {
            Leitura::Medida(v) => humano(*v),
            Leitura::NaoMedida(_) => "?".into(),
        },
        pct(&h.memoria.percent),
    ));
    s.push_str(&format!(
        "  swap         : {} / {} ({})\n",
        match &h.swap.usado {
            Leitura::Medida(v) => humano(*v),
            Leitura::NaoMedida(m) => format!("nao medida ({m})"),
        },
        match &h.swap.total {
            Leitura::Medida(v) => humano(*v),
            Leitura::NaoMedida(_) => "?".into(),
        },
        pct(&h.swap.percent),
    ));
    match &h.disco {
        Leitura::Medida(d) => s.push_str(&format!(
            "  disco {}  : {} / {} ({:.1}%)\n",
            d.montagem,
            humano(d.usado),
            humano(d.total),
            d.percent
        )),
        Leitura::NaoMedida(m) => s.push_str(&format!("  disco        : nao medido ({m})\n")),
    }
    match &h.uptime {
        Leitura::Medida(u) => s.push_str(&format!("  uptime       : {}\n", duracao(*u))),
        Leitura::NaoMedida(m) => s.push_str(&format!("  uptime       : nao medido ({m})\n")),
    }
    for t in &h.temperaturas {
        let rot = t.rotulo.clone().unwrap_or_else(|| t.chip.clone());
        let crit = t
            .critico
            .map(|c| format!(" (crit {c:.0} C)"))
            .unwrap_or_default();
        s.push_str(&format!("  temp {rot}: {:.1} C{crit}\n", t.celsius));
    }
    for v in &h.ventoinhas {
        let rot = v.rotulo.clone().unwrap_or_else(|| "fan".into());
        s.push_str(&format!("  fan {rot}: {:.0} RPM\n", v.rpm));
    }
    for (i, f) in h.frequencias.iter().enumerate() {
        if let Leitura::Medida(mhz) = f {
            s.push_str(&format!("  freq policy{i}: {mhz:.0} MHz\n"));
        }
    }
    let (estado, motivos) = match &h.saude {
        Saude::Ok => ("OK", &Vec::new()),
        Saude::Atencao(m) => ("ATENCAO", m),
        Saude::Critico(m) => ("CRITICO", m),
    };
    s.push_str(&format!("  saude        : {estado}\n"));
    for m in motivos {
        s.push_str(&format!("                 - {m}\n"));
    }
    s
}

fn duracao(seg: f64) -> String {
    let s = seg as u64;
    let (d, h, m) = (s / 86_400, (s % 86_400) / 3_600, (s % 3_600) / 60);
    format!("{d}d {h}h {m}m")
}

// ------------------------------------------------------------------ rota HTTP

use crate::api::{ApiState, auth};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};

/// A rota do monitor. Leitura da instancia (o hospedeiro inteiro, nao um projeto), do leitor
/// para cima -- a linha na `rbac::MATRIZ` e em `rotas::ROTAS`.
pub const ROTA: &str = "/v1/hardware";

pub fn rotas() -> Router<ApiState> {
    Router::new().route(ROTA, get(hardware_http))
}

async fn hardware_http(
    State(s): State<ApiState>,
    h: HeaderMap,
) -> Result<Response, (StatusCode, Json<Value>)> {
    auth(&s, &h)?;
    // As duas amostras com o sleep entre elas sao I/O bloqueante: fora do executor async.
    let hw = tokio::task::spawn_blocking(|| {
        let leitor = LeitorHardware::do_sistema();
        let a = leitor.amostra();
        std::thread::sleep(std::time::Duration::from_millis(INTERVALO_MS));
        let b = leitor.amostra();
        leitor.entre(&a, &b)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;
    Ok(Json(para_json(&hw)).into_response())
}

// ------------------------------------------------------------------ Windows (FFI Win32)

/// Leitura nativa do Windows por FFI direto a Win32 (`extern "system"`), sem crate de
/// terceiro -- o mesmo padrao do SHA-256 da casa. NAO compila neste alvo Linux (fica fora pelo
/// `cfg`). O `#[allow(unsafe_code)]` confina a FFI a este modulo; o crate e
/// `#![deny(unsafe_code)]`.
#[cfg(windows)]
#[allow(unsafe_code)]
mod win {
    use super::{Disco, Hardware, Leitura, Memoria, Saude, Swap, humano, saude};

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Filetime {
        baixo: u32,
        alto: u32,
    }

    impl Filetime {
        fn cem_ns(self) -> u64 {
            (self.alto as u64) << 32 | self.baixo as u64
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MemoryStatusEx {
        tamanho: u32,
        carga_memoria: u32,
        total_fisica: u64,
        disponivel_fisica: u64,
        total_pagefile: u64,
        disponivel_pagefile: u64,
        total_virtual: u64,
        disponivel_virtual: u64,
        estendida_virtual: u64,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemTimes(
            ocioso: *mut Filetime,
            kernel: *mut Filetime,
            usuario: *mut Filetime,
        ) -> i32;
        fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32;
        fn GetDiskFreeSpaceExW(
            dir: *const u16,
            disponivel_chamador: *mut u64,
            total: *mut u64,
            total_livre: *mut u64,
        ) -> i32;
    }

    /// O instante do Windows: os tempos do sistema (para o CPU%) e a memoria. Disco e lido em
    /// `montar` (instantaneo, nao precisa de delta).
    #[derive(Debug, Clone)]
    pub struct Snap {
        ocioso: u64,
        kernel: u64,
        usuario: u64,
        mem: Option<(u64, u64, u64, u64)>,
    }

    pub fn amostrar() -> Snap {
        let mut ocioso = Filetime::default();
        let mut kernel = Filetime::default();
        let mut usuario = Filetime::default();
        let ok = unsafe { GetSystemTimes(&mut ocioso, &mut kernel, &mut usuario) } != 0;
        let mut m = MemoryStatusEx {
            tamanho: std::mem::size_of::<MemoryStatusEx>() as u32,
            carga_memoria: 0,
            total_fisica: 0,
            disponivel_fisica: 0,
            total_pagefile: 0,
            disponivel_pagefile: 0,
            total_virtual: 0,
            disponivel_virtual: 0,
            estendida_virtual: 0,
        };
        let mem = (unsafe { GlobalMemoryStatusEx(&mut m) } != 0).then_some((
            m.total_fisica,
            m.disponivel_fisica,
            m.total_pagefile,
            m.disponivel_pagefile,
        ));
        if ok {
            Snap {
                ocioso: ocioso.cem_ns(),
                kernel: kernel.cem_ns(),
                usuario: usuario.cem_ns(),
                mem,
            }
        } else {
            Snap {
                ocioso: 0,
                kernel: 0,
                usuario: 0,
                mem,
            }
        }
    }

    fn disco_do_cwd() -> Leitura<Disco> {
        let Ok(cwd) = std::env::current_dir() else {
            return Leitura::NaoMedida("sem diretorio corrente".into());
        };
        // Caminho em UTF-16 terminado em NUL, como a API W espera.
        let mut largo: Vec<u16> = cwd.to_string_lossy().encode_utf16().collect();
        largo.push(0);
        let (mut disp, mut total, mut livre) = (0u64, 0u64, 0u64);
        let ok =
            unsafe { GetDiskFreeSpaceExW(largo.as_ptr(), &mut disp, &mut total, &mut livre) } != 0;
        if !ok || total == 0 {
            return Leitura::NaoMedida("GetDiskFreeSpaceExW falhou".into());
        }
        let usado = total.saturating_sub(livre);
        Leitura::Medida(Disco {
            montagem: cwd.to_string_lossy().into_owned(),
            total,
            livre: disp,
            usado,
            percent: usado as f64 / total as f64 * 100.0,
        })
    }

    pub fn montar(a: &Snap, b: &Snap) -> Hardware {
        // GetSystemTimes: kernel inclui o ocioso. busy = (kernel+usuario) - ocioso.
        let d = |x: u64, y: u64| x.saturating_sub(y);
        let total = d(b.kernel, a.kernel) + d(b.usuario, a.usuario);
        let ocioso = d(b.ocioso, a.ocioso);
        let busy = total.saturating_sub(ocioso);
        let cpu = if total == 0 {
            0.0
        } else {
            busy as f64 / total as f64 * 100.0
        };
        let memoria = match b.mem {
            Some((tot, disp, _, _)) => {
                let usado = tot.saturating_sub(disp);
                Memoria {
                    total: Leitura::Medida(tot),
                    disponivel: Leitura::Medida(disp),
                    usado: Leitura::Medida(usado),
                    percent: Leitura::Medida(if tot == 0 {
                        0.0
                    } else {
                        usado as f64 / tot as f64 * 100.0
                    }),
                    fonte_disponivel: "ullAvailPhys (GlobalMemoryStatusEx)".into(),
                }
            }
            None => Memoria {
                total: Leitura::NaoMedida("GlobalMemoryStatusEx falhou".into()),
                disponivel: Leitura::NaoMedida("GlobalMemoryStatusEx falhou".into()),
                usado: Leitura::NaoMedida("GlobalMemoryStatusEx falhou".into()),
                percent: Leitura::NaoMedida("GlobalMemoryStatusEx falhou".into()),
                fonte_disponivel: "GlobalMemoryStatusEx falhou".into(),
            },
        };
        let swap = match b.mem {
            Some((_, _, tot, disp)) => {
                let usado = tot.saturating_sub(disp);
                Swap {
                    total: Leitura::Medida(tot),
                    livre: Leitura::Medida(disp),
                    usado: Leitura::Medida(usado),
                    percent: Leitura::Medida(if tot == 0 {
                        0.0
                    } else {
                        usado as f64 / tot as f64 * 100.0
                    }),
                }
            }
            None => Swap {
                total: Leitura::NaoMedida("GlobalMemoryStatusEx falhou".into()),
                livre: Leitura::NaoMedida("GlobalMemoryStatusEx falhou".into()),
                usado: Leitura::NaoMedida("GlobalMemoryStatusEx falhou".into()),
                percent: Leitura::NaoMedida("GlobalMemoryStatusEx falhou".into()),
            },
        };
        let _ = humano;
        let mut h = Hardware {
            cpu_total_pct: Leitura::Medida(cpu),
            cpu_por_nucleo: vec![],
            ncpu: Leitura::NaoMedida("por nucleo pede NtQuerySystemInformation -- camada 2".into()),
            // O Windows nao expoe load average; emular seria inventar numero.
            carga: Leitura::NaoMedida("Windows nao expoe load average".into()),
            memoria,
            swap,
            disco: disco_do_cwd(),
            uptime: Leitura::NaoMedida("uptime pede GetTickCount64 -- camada 2".into()),
            // Sensor profundo no Windows precisa de driver/LHM -- camada 2.
            temperaturas: vec![],
            ventoinhas: vec![],
            frequencias: vec![],
            saude: Saude::Ok,
        };
        h.saude = saude(&h);
        h
    }
}
