//! Chave fora do swap: `mlockall` no painel, no no P2P e no repasse, e a
//! diretiva `mlock` no OpenVPN do modo servidor.
//!
//! # O que protege
//!
//! A chave privada do no, as chaves de sessao, a chave do cofre destrancado
//! e o `tls-crypt` de cada rede vivem na memoria do processo. Pagina que vai
//! para o swap fica no disco depois que o processo morre -- e quem leva o
//! disco leva a chave. `mlockall` tira o processo inteiro do swap.
//!
//! # So quando da para travar sem quebrar
//!
//! Com `MCL_FUTURE` e um `RLIMIT_MEMLOCK` finito (8 MiB no systemd novo, 64
//! KiB no velho), a alocacao que passar do limite FALHA -- `malloc` sem
//! memoria, pilha de thread que nao nasce. Um processo que aborta na
//! conexao seguinte e pior que a pagina no swap. Entao [`decidir`]: trava
//! quando o limite nao alcanca (`CAP_IPC_LOCK`, ou limite infinito), sobe o
//! limite macio quando o duro e infinito, e fora disso NAO trava e avisa. O
//! servico instalado leva `LimitMEMLOCK=infinity` (`servico.rs`), que e o
//! caminho sem privilegio a mais.
//!
//! # `MCL_ONFAULT`: trava o que foi tocado, nao o que foi reservado
//!
//! Medido em netns (`provas/operacao/mlock.sh`): sem ele, cada pilha de
//! thread (2 MiB reservados) entra inteira na memoria travada no nascimento
//! -- o painel sob conexoes paralelas trava centenas de MiB que nunca usa.
//! Com ele, a pagina trava quando e tocada, e chave e sempre pagina tocada
//! (esta em uso). Linux 4.4+; kernel sem ele recusa (`EINVAL`) e o codigo
//! tenta de novo sem, dizendo no log.
//!
//! # Windows
//!
//! Nao ha `mlockall`. `VirtualLock` trava REGIOES, e as chaves daqui vivem em
//! `Vec` do heap que mudam de lugar ao crescer -- travar uma regiao seria
//! travar o endereco errado depois do primeiro `push`. Fica registrado como
//! limite (PHXVPN.md, «mlock»), sem meia protecao que parece inteira.

/// O que fazer com o limite que o processo tem.
#[derive(Debug, PartialEq, Eq)]
pub enum Decisao {
    /// O limite nao alcanca: trava.
    Travar,
    /// Macio finito, duro infinito: sobe o macio e trava.
    SubirETravar,
    /// Limite finito que nao sobe: travar faria a proxima alocacao falhar.
    NaoTravar,
}

/// `None` = infinito (`RLIM_INFINITY`).
pub fn decidir(macio: Option<u64>, duro: Option<u64>, cap_ipc_lock: bool) -> Decisao {
    if cap_ipc_lock || macio.is_none() {
        Decisao::Travar
    } else if duro.is_none() {
        Decisao::SubirETravar
    } else {
        Decisao::NaoTravar
    }
}

/// O OpenVPN com `mlock` sobe o limite para 100 MiB se estiver abaixo, e
/// sai com erro FATAL se o `setrlimit` falhar (platform.c:344-372 da
/// 2.6.19). Subir o macio ate o duro nao pede privilegio; subir o duro pede
/// `CAP_SYS_RESOURCE`. A diretiva so vai quando o `openvpn` filho herda um
/// limite com que ele sobe -- senao a rede nao subiria.
pub fn openvpn_aguenta_mlock(
    macio: Option<u64>,
    duro: Option<u64>,
    cap_sys_resource: bool,
) -> bool {
    const MINIMO: u64 = 100 * 1024 * 1024;
    let basta = |l: Option<u64>| l.map_or(true, |v| v >= MINIMO);
    basta(macio) || basta(duro) || cap_sys_resource
}

/// Como terminou a tentativa, para o log e para a prova.
#[derive(Debug)]
pub enum Resultado {
    Travado { kib: u64, na_falta: bool },
    NaoTravado(String),
}

/// Trava a memoria do processo `quem` (painel, p2p, repasse) e diz no log o
/// que fez. Nunca falha o arranque: sem travar, o processo segue e avisa.
pub fn travar(quem: &str) -> Resultado {
    let r = travar_sem_log();
    match &r {
        Resultado::Travado { kib, na_falta } => eprintln!(
            "phxvpn: {quem}: memoria travada fora do swap (mlockall{}; VmLck {kib} KiB)",
            if *na_falta { ", MCL_ONFAULT" } else { "" }
        ),
        Resultado::NaoTravado(m) => {
            eprintln!("phxvpn: AVISO {quem}: chaves podem ir ao swap -- {m}")
        }
    }
    r
}

/// As linhas do `servidor.conf` para o OpenVPN travar a propria memoria.
/// Fora do Linux, nenhuma: o `openvpn` do Windows nao tem `mlockall`.
pub fn conf_openvpn() -> String {
    #[cfg(target_os = "linux")]
    {
        let (macio, duro) = so::limite();
        if openvpn_aguenta_mlock(macio, duro, so::tem_cap(so::CAP_SYS_RESOURCE)) {
            return "# chaves do openvpn fora do swap (phxvpn memoria.rs)\nmlock\n".into();
        }
    }
    String::new()
}

/// `VmLck` do proprio processo, em KiB (0 fora do Linux).
pub fn vmlck_kib() -> u64 {
    campo_status(
        &std::fs::read_to_string("/proc/self/status").unwrap_or_default(),
        "VmLck:",
    )
}

/// Um campo `Nome:   123 kB` do `/proc/<pid>/status`.
pub fn campo_status(status: &str, nome: &str) -> u64 {
    status
        .lines()
        .find_map(|l| l.strip_prefix(nome))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn travar_sem_log() -> Resultado {
    let (macio, duro) = so::limite();
    match decidir(macio, duro, so::tem_cap(so::CAP_IPC_LOCK)) {
        Decisao::NaoTravar => {
            return Resultado::NaoTravado(format!(
                "RLIMIT_MEMLOCK de {} KiB sem CAP_IPC_LOCK; travar faria a proxima alocacao \
                 falhar. De LimitMEMLOCK=infinity ao servico (o `phxvpn servico instalar` ja da)",
                macio.unwrap_or(0) / 1024
            ))
        }
        Decisao::SubirETravar => {
            if let Err(m) = so::subir_limite() {
                return Resultado::NaoTravado(m);
            }
        }
        Decisao::Travar => {}
    }
    match so::mlockall(true) {
        Ok(()) => Resultado::Travado {
            kib: vmlck_kib(),
            na_falta: true,
        },
        // Kernel anterior ao 4.4 nao conhece MCL_ONFAULT.
        Err(22) => match so::mlockall(false) {
            Ok(()) => Resultado::Travado {
                kib: vmlck_kib(),
                na_falta: false,
            },
            Err(e) => Resultado::NaoTravado(format!("mlockall: erro {e}")),
        },
        Err(e) => Resultado::NaoTravado(format!("mlockall: erro {e}")),
    }
}

#[cfg(not(target_os = "linux"))]
fn travar_sem_log() -> Resultado {
    Resultado::NaoTravado(
        "este sistema nao tem mlockall (Windows: VirtualLock so trava regioes; ver PHXVPN.md, «mlock»)"
            .into(),
    )
}

#[cfg(target_os = "linux")]
mod so {
    pub const CAP_IPC_LOCK: u32 = 14;
    pub const CAP_SYS_RESOURCE: u32 = 24;
    const RLIMIT_MEMLOCK: i32 = 8;
    const RLIM_INFINITY: u64 = u64::MAX;
    // (MCL_CURRENT, MCL_FUTURE, MCL_ONFAULT): os do asm-generic/mman.h; o
    // powerpc e o sparc tem os proprios (arch/*/include/uapi/asm/mman.h).
    #[cfg(not(any(
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "sparc64"
    )))]
    const MCL: (i32, i32, i32) = (1, 2, 4);
    #[cfg(any(
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "sparc64"
    ))]
    const MCL: (i32, i32, i32) = (0x2000, 0x4000, 0x8000);

    #[repr(C)]
    struct Rlimit {
        cur: u64,
        max: u64,
    }
    extern "C" {
        fn getrlimit(recurso: i32, r: *mut Rlimit) -> i32;
        fn setrlimit(recurso: i32, r: *const Rlimit) -> i32;
        #[link_name = "mlockall"]
        fn mlockall_c(flags: i32) -> i32;
    }

    fn opcao(v: u64) -> Option<u64> {
        (v != RLIM_INFINITY).then_some(v)
    }

    /// (macio, duro) do `RLIMIT_MEMLOCK`, `None` = infinito.
    pub fn limite() -> (Option<u64>, Option<u64>) {
        let mut r = Rlimit { cur: 0, max: 0 };
        // SAFETY: `r` vive ate o fim da chamada e tem o layout de `struct rlimit`.
        if unsafe { getrlimit(RLIMIT_MEMLOCK, &mut r) } != 0 {
            return (Some(0), Some(0));
        }
        (opcao(r.cur), opcao(r.max))
    }

    pub fn subir_limite() -> Result<(), String> {
        let r = Rlimit {
            cur: RLIM_INFINITY,
            max: RLIM_INFINITY,
        };
        // SAFETY: `r` vive ate o fim da chamada.
        if unsafe { setrlimit(RLIMIT_MEMLOCK, &r) } == 0 {
            Ok(())
        } else {
            Err(format!(
                "setrlimit(RLIMIT_MEMLOCK): {}",
                std::io::Error::last_os_error()
            ))
        }
    }

    /// `Err(errno)`.
    pub fn mlockall(na_falta: bool) -> Result<(), i32> {
        let (atual, futuro, na_falta_bit) = MCL;
        let flags = atual | futuro | if na_falta { na_falta_bit } else { 0 };
        // SAFETY: sem ponteiros; o kernel so muda o estado de travamento.
        if unsafe { mlockall_c(flags) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0))
        }
    }

    /// A capacidade `n` esta no conjunto EFETIVO (`CapEff` do status)?
    pub fn tem_cap(n: u32) -> bool {
        tem_cap_em(
            &std::fs::read_to_string("/proc/self/status").unwrap_or_default(),
            n,
        )
    }

    pub fn tem_cap_em(status: &str, n: u32) -> bool {
        status
            .lines()
            .find_map(|l| l.strip_prefix("CapEff:"))
            .and_then(|v| u64::from_str_radix(v.trim(), 16).ok())
            .is_some_and(|m| m & (1 << n) != 0)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const KIB: u64 = 1024;

    /// RED: travar com limite finito (a guarda tirada) faz a alocacao
    /// seguinte falhar depois que o processo passa do limite.
    #[test]
    fn com_limite_finito_nao_trava() {
        assert_eq!(
            decidir(Some(8192 * KIB), Some(8192 * KIB), false),
            Decisao::NaoTravar
        );
        assert_eq!(
            decidir(Some(64 * KIB), Some(64 * KIB), false),
            Decisao::NaoTravar
        );
        assert_eq!(
            decidir(Some(8192 * KIB), None, false),
            Decisao::SubirETravar
        );
        assert_eq!(decidir(None, None, false), Decisao::Travar);
        assert_eq!(
            decidir(Some(64 * KIB), Some(64 * KIB), true),
            Decisao::Travar
        );
    }

    /// O `mlock` do OpenVPN so vai quando ele consegue subir o limite para
    /// 100 MiB; senao a rede nao sobe (FATAL no `setrlimit`). RED: escrever
    /// a diretiva sempre deixa a rede fora do ar sob `LimitMEMLOCK=8M`.
    #[test]
    fn mlock_do_openvpn_so_quando_ele_sobe() {
        let m = 100 * 1024 * KIB;
        assert!(!openvpn_aguenta_mlock(
            Some(8192 * KIB),
            Some(8192 * KIB),
            false
        ));
        assert!(openvpn_aguenta_mlock(
            Some(8192 * KIB),
            Some(8192 * KIB),
            true
        ));
        assert!(openvpn_aguenta_mlock(Some(8192 * KIB), Some(m), false));
        assert!(openvpn_aguenta_mlock(None, None, false));
        assert!(openvpn_aguenta_mlock(Some(8192 * KIB), None, false));
    }

    #[test]
    fn le_os_campos_do_status() {
        let s = "Name:\tphxvpn\nVmLck:\t   12345 kB\nCapEff:\t0000000000004000\n";
        assert_eq!(campo_status(s, "VmLck:"), 12345);
        assert_eq!(campo_status(s, "VmRSS:"), 0);
        #[cfg(target_os = "linux")]
        {
            assert!(so::tem_cap_em(s, so::CAP_IPC_LOCK));
            assert!(!so::tem_cap_em(s, so::CAP_SYS_RESOURCE));
            assert!(!so::tem_cap_em("CapEff:\tzz\n", 0));
        }
    }

    /// Prova contra o sistema operacional, num processo FILHO (o `mlockall`
    /// e do processo inteiro e nao se desfaz no meio da suite): o filho
    /// trava, e o `VmLck` do `/proc` dele diz se travou. Com permissao, tem
    /// de travar (RED: tirar a chamada deixa VmLck 0); sem permissao, NAO
    /// pode travar (RED: tirar a guarda trava com limite finito).
    #[cfg(target_os = "linux")]
    #[test]
    fn filho_trava_e_o_proc_confirma() {
        if std::env::var("PHXVPN_TESTE_MLOCK_FILHO").is_ok() {
            let r = travar("teste");
            let (macio, duro) = so::limite();
            println!(
                "RESULTADO travado={} vmlck={} decisao={:?}",
                matches!(r, Resultado::Travado { .. }),
                vmlck_kib(),
                decidir(macio, duro, so::tem_cap(so::CAP_IPC_LOCK))
            );
            return;
        }
        let eu = std::env::current_exe().unwrap();
        let (macio, duro) = so::limite();
        let decisao = decidir(macio, duro, so::tem_cap(so::CAP_IPC_LOCK));
        let saida = std::process::Command::new(eu)
            .args([
                "memoria::testes::filho_trava_e_o_proc_confirma",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("PHXVPN_TESTE_MLOCK_FILHO", "1")
            .output()
            .unwrap();
        let texto = String::from_utf8_lossy(&saida.stdout);
        let linha = texto
            .lines()
            // O harness escreve «test ... ... » na mesma linha, antes.
            .find_map(|l| l.split_once("RESULTADO ").map(|(_, r)| r))
            .unwrap_or_else(|| panic!("o filho nao respondeu: {texto}"));
        let vmlck: u64 = linha
            .split("vmlck=")
            .nth(1)
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok())
            .unwrap();
        if decisao == Decisao::NaoTravar {
            assert!(linha.contains("travado=false") && vmlck == 0, "{linha}");
        } else {
            assert!(linha.contains("travado=true"), "{linha}");
            assert!(vmlck > 1024, "VmLck do filho: {vmlck} KiB -- {linha}");
        }
    }
}
