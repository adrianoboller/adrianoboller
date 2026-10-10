//! Prova do monitor de hardware contra um `/proc`+`/sys` de MENTIRA, montado em tempdir com
//! valores conhecidos. Os numeros de saida sao deterministas, entao o teste os afirma de cheio:
//! CPU% do delta de dois `/proc/stat`, memoria em bytes de `MemAvailable`, temperatura em C (os
//! arquivos sao miligraus), RPM cru e o veredito.
//!
//! RED nos dois sentidos, cada um num teste que CAI com o defeito reposto em `hardware.rs`:
//! - `memoria_usa_memavailable_nao_memfree`: trocar `MemAvailable` por `MemFree` no
//!   `montar_memoria` muda o disponivel de 409.600 para 102.400 e o teste cai;
//! - `temperatura_em_graus_divide_por_mil`: trocar `mc / 1000.0` por `mc / 100.0` no `ler_hwmon`
//!   vira 45 C em 450 C e o teste cai.

use phxclaw_agent::hardware::{LeitorHardware, Leitura, Saude, calcular_disco};
use std::path::PathBuf;

/// Monta a arvore falsa. `stat` recebe o conteudo do `/proc/stat` para esta amostra -- o teste
/// reescreve o arquivo entre as duas amostras para medir o delta de CPU.
struct Arvore {
    raiz: PathBuf,
}

impl Arvore {
    fn nova(nome: &str) -> Arvore {
        let raiz =
            std::env::temp_dir().join(format!("phx-hw-{nome}-{}", phxclaw_types::new_uuid_v7()));
        let a = Arvore { raiz };
        a.escrever("proc/meminfo", MEMINFO);
        a.escrever("proc/loadavg", "0.50 1.00 2.00 1/123 4567\n");
        a.escrever("proc/uptime", "3600.50 9000.00\n");
        // Um hwmon com uma temperatura (45 C, crit 100 C) e uma ventoinha (1200 RPM).
        a.escrever("sys/class/hwmon/hwmon0/name", "coretemp\n");
        a.escrever("sys/class/hwmon/hwmon0/temp1_input", "45000\n");
        a.escrever("sys/class/hwmon/hwmon0/temp1_label", "Core 0\n");
        a.escrever("sys/class/hwmon/hwmon0/temp1_crit", "100000\n");
        a.escrever("sys/class/hwmon/hwmon0/fan1_input", "1200\n");
        a.escrever("sys/class/hwmon/hwmon0/fan1_label", "cpu_fan\n");
        a
    }

    fn escrever(&self, rel: &str, conteudo: &str) {
        let p = self.raiz.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, conteudo).unwrap();
    }

    fn proc(&self) -> PathBuf {
        self.raiz.join("proc")
    }

    fn sys(&self) -> PathBuf {
        self.raiz.join("sys")
    }
}

impl Drop for Arvore {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.raiz);
    }
}

// MemTotal 1000 kB, MemAvailable 400 kB -> disponivel 409.600 bytes; usado 60%.
// MemFree 100 kB existe de proposito: a troca errada o pegaria (disponivel 102.400 -> 90%).
// SwapTotal 500 kB, SwapFree 300 kB -> swap usado 40%.
const MEMINFO: &str = "\
MemTotal:           1000 kB
MemFree:             100 kB
MemAvailable:        400 kB
Cached:              200 kB
SReclaimable:         50 kB
SwapTotal:           500 kB
SwapFree:            300 kB
";

// cpu: user nice system idle iowait irq softirq steal guest guest_nice
const STAT_ANTES: &str = "\
cpu  100 0 100 700 0 0 0 0 0 0
cpu0 50 0 50 400 0 0 0 0 0 0
cpu1 50 0 50 300 0 0 0 0 0 0
intr 0
";

const STAT_DEPOIS: &str = "\
cpu  200 0 200 1300 0 0 0 0 0 0
cpu0 150 0 150 700 0 0 0 0 0 0
cpu1 50 0 50 900 0 0 0 0 0 0
intr 0
";

fn amostrar_delta(arv: &Arvore, antes: &str, depois: &str) -> phxclaw_agent::hardware::Hardware {
    let leitor = LeitorHardware::de_raiz(&arv.proc(), &arv.sys());
    arv.escrever("proc/stat", antes);
    let a = leitor.amostra();
    arv.escrever("proc/stat", depois);
    let b = leitor.amostra();
    leitor.entre(&a, &b)
}

fn f64_de(l: &Leitura<f64>) -> f64 {
    *l.medida().expect("medida")
}

#[test]
fn cpu_percent_sai_do_delta_das_duas_amostras() {
    let arv = Arvore::nova("cpu");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    // Agregado: du=100 ds=100 di=600 -> total=800 busy=200 -> 25%.
    assert_eq!(f64_de(&hw.cpu_total_pct), 25.0);
    // Dois nucleos. cpu0: du=100 ds=100 di=300 -> total=500 busy=200 -> 40%.
    assert_eq!(hw.cpu_por_nucleo.len(), 2);
    assert_eq!(f64_de(&hw.cpu_por_nucleo[0]), 40.0);
    // cpu1: du=0 ds=0 di=600 -> total=600 busy=0 -> 0%.
    assert_eq!(f64_de(&hw.cpu_por_nucleo[1]), 0.0);
    assert_eq!(hw.ncpu.medida().copied(), Some(2));
}

#[test]
fn memoria_usa_memavailable_nao_memfree() {
    let arv = Arvore::nova("mem");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    // MemTotal 1000 kB -> 1.024.000 bytes.
    assert_eq!(hw.memoria.total.medida().copied(), Some(1_024_000));
    // MemAvailable 400 kB -> 409.600. A troca por MemFree daria 102.400: o RED cai aqui.
    assert_eq!(hw.memoria.disponivel.medida().copied(), Some(409_600));
    assert_eq!(hw.memoria.usado.medida().copied(), Some(614_400));
    assert_eq!(f64_de(&hw.memoria.percent), 60.0);
    assert_eq!(hw.memoria.fonte_disponivel, "MemAvailable");
}

#[test]
fn swap_sai_de_swaptotal_e_swapfree() {
    let arv = Arvore::nova("swap");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    assert_eq!(hw.swap.total.medida().copied(), Some(512_000));
    assert_eq!(hw.swap.livre.medida().copied(), Some(307_200));
    assert_eq!(hw.swap.usado.medida().copied(), Some(204_800));
    assert_eq!(f64_de(&hw.swap.percent), 40.0);
}

#[test]
fn temperatura_em_graus_divide_por_mil() {
    let arv = Arvore::nova("temp");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    assert_eq!(hw.temperaturas.len(), 1);
    let t = &hw.temperaturas[0];
    // 45000 miligraus -> 45,0 C. A troca por /100 daria 450,0: o RED cai aqui.
    assert_eq!(t.celsius, 45.0);
    assert_eq!(t.critico, Some(100.0));
    assert_eq!(t.chip, "coretemp");
    assert_eq!(t.rotulo.as_deref(), Some("Core 0"));
}

#[test]
fn ventoinha_em_rpm_cru() {
    let arv = Arvore::nova("fan");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    assert_eq!(hw.ventoinhas.len(), 1);
    assert_eq!(hw.ventoinhas[0].rpm, 1200.0);
    assert_eq!(hw.ventoinhas[0].rotulo.as_deref(), Some("cpu_fan"));
}

#[test]
fn carga_e_uptime_saem_dos_proprios_arquivos() {
    let arv = Arvore::nova("carga");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    assert_eq!(hw.carga.medida().copied(), Some([0.50, 1.00, 2.00]));
    assert_eq!(hw.uptime.medida().copied(), Some(3600.50));
}

#[test]
fn disco_pela_porta_de_prova_nao_e_medido() {
    // A arvore falsa nao finge disco (ele e o real da maquina), entao de_raiz o deixa NaoMedida.
    let arv = Arvore::nova("disco");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    assert!(matches!(hw.disco, Leitura::NaoMedida(_)));
}

/// RED do disco (papel F): `statvfs` e syscall e nao da arvore falsa, entao a prova e sobre a
/// FUNCAO PURA `calcular_disco`, com numeros de struct CONHECIDOS. Dois defeitos repostos a
/// derrubam, cada um num `assert`:
/// - (a) `usado` com `f_bavail` no lugar de `f_bfree`: usado 1.638.400 -> 2.048.000;
/// - (b) esquecer o `x f_frsize`: total 4.096.000 -> 1.000.
#[test]
fn calcular_disco_a_aritmetica_pura_do_statvfs() {
    // f_blocks=1000, f_bfree=600, f_bavail=500, f_frsize=4096.
    let d = calcular_disco(1000, 600, 500, 4096);
    assert_eq!(d.total, 4_096_000); // 1000 x 4096; (b) quebra aqui
    assert_eq!(d.livre, 2_048_000); // f_bavail x frsize
    assert_eq!(d.usado, 1_638_400); // total - f_bfree x frsize; (a) quebra aqui
    // usado/(usado+livre) = 1.638.400 / 3.686.400 = 4/9 = 44,444...%
    assert!(
        (d.percent - 400.0 / 9.0).abs() < 1e-9,
        "percent: {}",
        d.percent
    );
    // Disco cheio (nada disponivel) e 100% usado, nunca divisao por zero.
    assert_eq!(calcular_disco(1000, 0, 0, 4096).percent, 100.0);
    // Tudo zero: 0%, sem panico.
    assert_eq!(calcular_disco(0, 0, 0, 0).percent, 0.0);
}

/// Fumaca contra o `statvfs` REAL (so Linux): numero varia por maquina, entao a prova e de
/// forma, nao de valor -- total e livre positivos e percent dentro de [0, 100].
#[cfg(target_os = "linux")]
#[test]
fn disco_em_um_caminho_real_mede() {
    use phxclaw_agent::hardware::disco_em;
    let dir = std::env::temp_dir();
    match disco_em(&dir) {
        Leitura::Medida(d) => {
            assert!(d.total > 0, "total: {}", d.total);
            assert!(d.livre > 0, "livre: {}", d.livre);
            assert!((0.0..=100.0).contains(&d.percent), "percent: {}", d.percent);
            assert!(d.usado <= d.total, "usado {} > total {}", d.usado, d.total);
        }
        Leitura::NaoMedida(m) => panic!("statvfs real devia medir o tempdir: {m}"),
    }
}

/// O caminho real (`do_sistema`) le o disco de verdade e o entrega Medido no Linux: prova que a
/// syscall esta ligada ao painel, nao so a funcao pura.
#[cfg(target_os = "linux")]
#[test]
fn disco_do_sistema_e_medido_no_linux() {
    let leitor = LeitorHardware::do_sistema();
    let a = leitor.amostra();
    let b = leitor.amostra();
    let hw = leitor.entre(&a, &b);
    assert!(
        matches!(hw.disco, Leitura::Medida(_)),
        "disco: {:?}",
        hw.disco
    );
}

#[test]
fn veredito_ok_quando_tudo_esta_abaixo_dos_limiares() {
    let arv = Arvore::nova("ok");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    // 45 C (crit 100 -> atencao >= 85), memoria 60%, carga 0,5 em 2 CPUs: tudo folgado.
    assert_eq!(hw.saude, Saude::Ok);
}

#[test]
fn veredito_critico_usa_o_crit_do_chip() {
    let arv = Arvore::nova("critico");
    // 98 C com crit 100 C -> limiar critico em 95 C (crit-5): dispara critico.
    arv.escrever("sys/class/hwmon/hwmon0/temp1_input", "98000\n");
    let hw = amostrar_delta(&arv, STAT_ANTES, STAT_DEPOIS);
    match &hw.saude {
        Saude::Critico(motivos) => {
            assert!(
                motivos.iter().any(|m| m.contains("98")),
                "o motivo nomeia o numero: {motivos:?}"
            );
        }
        outro => panic!("esperava critico, veio {outro:?}"),
    }
}

#[test]
fn arvore_vazia_nao_entra_em_panico_e_marca_nao_medida() {
    let raiz = std::env::temp_dir().join(format!("phx-hw-vazia-{}", phxclaw_types::new_uuid_v7()));
    let leitor = LeitorHardware::de_raiz(&raiz.join("proc"), &raiz.join("sys"));
    let a = leitor.amostra();
    let b = leitor.amostra();
    let hw = leitor.entre(&a, &b);
    assert!(matches!(hw.cpu_total_pct, Leitura::NaoMedida(_)));
    assert!(matches!(hw.memoria.total, Leitura::NaoMedida(_)));
    assert!(hw.temperaturas.is_empty());
    // Sem numero que dispare alarme e sem numero que prove saude: Ok vazio.
    assert_eq!(hw.saude, Saude::Ok);
    let _ = std::fs::remove_dir_all(&raiz);
}
