//! O phxvpn como serviço do Windows: o gerenciador de serviços (SCM) sobe o
//! `phxvpn.exe servico-rodar <unidade> <comando...>` com a máquina, como
//! SYSTEM e sem ninguém logado, e o religa se cair (5 s, como o
//! `Restart=on-failure` do systemd).
//!
//! Só APIs do sistema (`advapi32`), por FFI.

#![cfg(windows)]

use crate::tap::largo;
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};

pub type R<T> = Result<T, String>;

type Sc = *mut c_void;

#[repr(C)]
struct Estado {
    tipo: u32,
    atual: u32,
    aceita: u32,
    saida_win32: u32,
    saida_propria: u32,
    ponto: u32,
    espera: u32,
}

#[repr(C)]
struct EntradaTabela {
    nome: *const u16,
    principal: Option<unsafe extern "system" fn(u32, *mut *mut u16)>,
}

#[repr(C)]
struct Descricao {
    texto: *const u16,
}

#[repr(C)]
struct Acao {
    tipo: u32,
    espera_ms: u32,
}

#[repr(C)]
struct AcoesDeFalha {
    zerar_apos_s: u32,
    msg_reinicio: *const u16,
    comando: *const u16,
    n: u32,
    acoes: *const Acao,
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenSCManagerW(maquina: *const u16, base: *const u16, acesso: u32) -> Sc;
    fn CreateServiceW(
        scm: Sc,
        nome: *const u16,
        exibicao: *const u16,
        acesso: u32,
        tipo: u32,
        inicio: u32,
        erro: u32,
        binario: *const u16,
        grupo: *const u16,
        marca: *mut u32,
        dependencias: *const u16,
        conta: *const u16,
        senha: *const u16,
    ) -> Sc;
    fn OpenServiceW(scm: Sc, nome: *const u16, acesso: u32) -> Sc;
    fn StartServiceW(s: Sc, n: u32, args: *const *const u16) -> i32;
    fn ControlService(s: Sc, controle: u32, estado: *mut Estado) -> i32;
    fn DeleteService(s: Sc) -> i32;
    fn CloseServiceHandle(s: Sc) -> i32;
    fn ChangeServiceConfig2W(s: Sc, nivel: u32, info: *const c_void) -> i32;
    fn StartServiceCtrlDispatcherW(tabela: *const EntradaTabela) -> i32;
    fn RegisterServiceCtrlHandlerExW(
        nome: *const u16,
        tratador: unsafe extern "system" fn(u32, u32, *mut c_void, *mut c_void) -> u32,
        contexto: *mut c_void,
    ) -> isize;
    fn SetServiceStatus(h: isize, estado: *const Estado) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn SetStdHandle(qual: u32, h: *mut c_void) -> i32;
}

const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
const STD_ERROR_HANDLE: u32 = -12i32 as u32;

const SC_MANAGER_ALL_ACCESS: u32 = 0xF003F;
const SERVICE_ALL_ACCESS: u32 = 0xF01FF;
const SERVICE_WIN32_OWN_PROCESS: u32 = 0x10;
const SERVICE_AUTO_START: u32 = 2;
const SERVICE_ERROR_NORMAL: u32 = 1;
const SERVICE_STOPPED: u32 = 1;
const SERVICE_STOP_PENDING: u32 = 3;
const SERVICE_RUNNING: u32 = 4;
const SERVICE_ACCEPT_STOP: u32 = 1;
const SERVICE_ACCEPT_SHUTDOWN: u32 = 4;
const SERVICE_CONTROL_STOP: u32 = 1;
const SERVICE_CONTROL_INTERROGATE: u32 = 4;
const SERVICE_CONTROL_SHUTDOWN: u32 = 5;
const SERVICE_CONFIG_DESCRIPTION: u32 = 1;
const SERVICE_CONFIG_FAILURE_ACTIONS: u32 = 2;
const SC_ACTION_RESTART: u32 = 1;
const ERROR_SERVICE_SPECIFIC_ERROR: u32 = 1066;
const ERROR_SERVICE_EXISTS: i32 = 1073;

fn erro(onde: &str) -> String {
    format!("{onde}: {}", std::io::Error::last_os_error())
}

struct Alca(Sc);
impl Drop for Alca {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: alca aberta pelo SCM, fechada uma vez.
            unsafe { CloseServiceHandle(self.0) };
        }
    }
}

fn scm() -> R<Alca> {
    // SAFETY: maquina e base nulas = esta maquina, base ativa.
    let h = unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS) };
    if h.is_null() {
        return Err(erro(
            "abrir o gerenciador de servicos (rode como administrador)",
        ));
    }
    Ok(Alca(h))
}

/// Cria (ou recria) o servico e o liga. `binario`: a linha inteira, com o
/// executavel entre aspas.
pub fn instalar(
    nome: &str,
    exibicao: &str,
    descricao: &str,
    binario: &str,
    iniciar: bool,
) -> R<()> {
    let m = scm()?;
    let (n, e, b) = (largo(nome), largo(exibicao), largo(binario));
    // SAFETY: textos terminados em zero, vivos durante a chamada.
    let mut s = unsafe {
        CreateServiceW(
            m.0,
            n.as_ptr(),
            e.as_ptr(),
            SERVICE_ALL_ACCESS,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_AUTO_START,
            SERVICE_ERROR_NORMAL,
            b.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(), // LocalSystem
            std::ptr::null(),
        )
    };
    if s.is_null() && std::io::Error::last_os_error().raw_os_error() == Some(ERROR_SERVICE_EXISTS) {
        remover(nome)?;
        // SAFETY: idem.
        s = unsafe {
            CreateServiceW(
                m.0,
                n.as_ptr(),
                e.as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_AUTO_START,
                SERVICE_ERROR_NORMAL,
                b.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
    }
    if s.is_null() {
        return Err(erro("criar o servico"));
    }
    let s = Alca(s);
    let d = largo(descricao);
    let desc = Descricao { texto: d.as_ptr() };
    // Caiu: religa em 5 s, tres vezes; o contador zera num dia bom.
    let acoes = [
        Acao {
            tipo: SC_ACTION_RESTART,
            espera_ms: 5000,
        },
        Acao {
            tipo: SC_ACTION_RESTART,
            espera_ms: 5000,
        },
        Acao {
            tipo: SC_ACTION_RESTART,
            espera_ms: 5000,
        },
    ];
    let falhas = AcoesDeFalha {
        zerar_apos_s: 86_400,
        msg_reinicio: std::ptr::null(),
        comando: std::ptr::null(),
        n: acoes.len() as u32,
        acoes: acoes.as_ptr(),
    };
    // SAFETY: estruturas vivas durante as chamadas.
    unsafe {
        ChangeServiceConfig2W(
            s.0,
            SERVICE_CONFIG_DESCRIPTION,
            &desc as *const _ as *const c_void,
        );
        ChangeServiceConfig2W(
            s.0,
            SERVICE_CONFIG_FAILURE_ACTIONS,
            &falhas as *const _ as *const c_void,
        );
    }
    // SAFETY: servico aberto acima; sem argumentos.
    if iniciar && unsafe { StartServiceW(s.0, 0, std::ptr::null()) } == 0 {
        return Err(erro("ligar o servico"));
    }
    Ok(())
}

pub fn remover(nome: &str) -> R<()> {
    let m = scm()?;
    let n = largo(nome);
    // SAFETY: nome terminado em zero.
    let s = unsafe { OpenServiceW(m.0, n.as_ptr(), SERVICE_ALL_ACCESS) };
    if s.is_null() {
        return Err(erro(&format!("abrir o servico {nome}")));
    }
    let s = Alca(s);
    let mut e = Estado {
        tipo: 0,
        atual: 0,
        aceita: 0,
        saida_win32: 0,
        saida_propria: 0,
        ponto: 0,
        espera: 0,
    };
    // SAFETY: servico aberto; parar pode falhar se ja parado (ignora).
    unsafe { ControlService(s.0, SERVICE_CONTROL_STOP, &mut e) };
    // SAFETY: idem.
    if unsafe { DeleteService(s.0) } == 0 {
        return Err(erro("apagar o servico"));
    }
    Ok(())
}

// ---------------------------------------------------- dentro do servico ----

type Trabalho = Box<dyn FnOnce() -> R<()> + Send>;

static NOME: OnceLock<Vec<u16>> = OnceLock::new();
static TRABALHO: Mutex<Option<Trabalho>> = Mutex::new(None);
static ALCA_ESTADO: AtomicIsize = AtomicIsize::new(0);

fn relatar(estado: u32, falhou: bool) {
    let e = Estado {
        tipo: SERVICE_WIN32_OWN_PROCESS,
        atual: estado,
        aceita: if estado == SERVICE_RUNNING {
            SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
        } else {
            0
        },
        saida_win32: if falhou {
            ERROR_SERVICE_SPECIFIC_ERROR
        } else {
            0
        },
        saida_propria: u32::from(falhou),
        ponto: 0,
        espera: if estado == SERVICE_STOP_PENDING {
            3000
        } else {
            0
        },
    };
    // SAFETY: alca registrada no ServiceMain; estrutura viva na chamada.
    unsafe { SetServiceStatus(ALCA_ESTADO.load(Ordering::SeqCst), &e) };
}

unsafe extern "system" fn tratador(
    controle: u32,
    _t: u32,
    _d: *mut c_void,
    _c: *mut c_void,
) -> u32 {
    match controle {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            relatar(SERVICE_STOP_PENDING, false);
            // A placa e os soquetes fecham com o processo; o que o no
            // grava (pares, fichas) ja foi gravado na hora em que mudou.
            relatar(SERVICE_STOPPED, false);
            std::process::exit(0);
        }
        SERVICE_CONTROL_INTERROGATE => 0,
        _ => 120, // ERROR_CALL_NOT_IMPLEMENTED
    }
}

unsafe extern "system" fn principal(_argc: u32, _argv: *mut *mut u16) {
    let nome = NOME.get().expect("nome do servico");
    // SAFETY: nome estatico terminado em zero; tratador com a assinatura do Windows.
    let h = unsafe { RegisterServiceCtrlHandlerExW(nome.as_ptr(), tratador, std::ptr::null_mut()) };
    ALCA_ESTADO.store(h, Ordering::SeqCst);
    relatar(SERVICE_RUNNING, false);
    let t = TRABALHO.lock().unwrap_or_else(|e| e.into_inner()).take();
    // Panico tambem para o servico COM a noticia ao SCM, em vez de deixar
    // o processo sumir com o estado ainda dizendo RUNNING.
    let r = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| t.map(|f| f()))) {
        Ok(r) => r.unwrap_or(Ok(())),
        Err(_) => Err("panico (ver o registro)".to_string()),
    };
    if let Err(e) = &r {
        eprintln!("phxvpn: servico parou: {e}");
    }
    relatar(SERVICE_STOPPED, r.is_err());
}

/// Servico nao tem console: a saida padrao e a de erro vao para
/// `%ProgramData%\\phxvpn\\<unidade>.log` (o `journalctl` daqui). O `std`
/// do Rust pergunta o `GetStdHandle` a cada escrita, entao trocar a alca
/// basta para todo `println!`/`eprintln!` do programa.
fn registro_em_arquivo(nome: &str) {
    use std::os::windows::io::IntoRawHandle;
    let dir = crate::servico::pasta_dados();
    let _ = std::fs::create_dir_all(&dir);
    let Ok(f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{nome}.log")))
    else {
        return;
    };
    let h = f.into_raw_handle();
    // SAFETY: alca de arquivo aberta acima; fica viva ate o processo sair.
    unsafe {
        SetStdHandle(STD_OUTPUT_HANDLE, h);
        SetStdHandle(STD_ERROR_HANDLE, h);
    }
    std::panic::set_hook(Box::new(|info| eprintln!("phxvpn: panico: {info}")));
}

/// Chamado pelo `phxvpn servico-rodar`: entrega o processo ao SCM, que
/// chama `principal` numa thread dele. Volta quando o servico para.
pub fn rodar(nome: &str, trabalho: Trabalho) -> R<()> {
    let _ = NOME.set(largo(nome));
    registro_em_arquivo(nome);
    *TRABALHO.lock().unwrap_or_else(|e| e.into_inner()) = Some(trabalho);
    let tabela = [
        EntradaTabela {
            nome: NOME.get().expect("nome").as_ptr(),
            principal: Some(principal),
        },
        EntradaTabela {
            nome: std::ptr::null(),
            principal: None,
        },
    ];
    // SAFETY: tabela terminada pela entrada nula, viva ate o retorno.
    if unsafe { StartServiceCtrlDispatcherW(tabela.as_ptr()) } == 0 {
        return Err(erro(
            "servico-rodar e para o gerenciador de servicos chamar (use phxvpn servico instalar)",
        ));
    }
    Ok(())
}
