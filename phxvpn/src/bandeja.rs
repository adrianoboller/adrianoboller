//! O icone do phxvpn na bandeja do Windows (ao lado do relogio).
//!
//! Duplo clique abre a janela; o botao direito mostra o menu «Abrir phxvpn /
//! Sair». Com `phxvpnw.exe --bandeja` (o que o «abrir com o sistema» usa), o
//! programa sobe so aqui, sem abrir a janela.
//!
//! So APIs do sistema por FFI (`user32`, `shell32`, `gdi32`, `kernel32`), sem
//! arquivo de recurso: o icone e pintado em codigo -- um circulo no vermelhao
//! da marca (#FF4D10) --, porque compilar um `.rc` pediria ferramenta de fora.
//!
//! O laco de mensagens roda na thread que criou a janela (regra do Windows);
//! a tela local roda em outra.

use crate::tap::largo;
use std::ffi::c_void;
use std::sync::OnceLock;

type Hwnd = *mut c_void;
type Handle = *mut c_void;

const WM_DESTROY: u32 = 0x0002;
const WM_COMMAND: u32 = 0x0111;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_BANDEJA: u32 = 0x8001; // WM_APP + 1
const NIM_ADD: u32 = 0;
const NIM_DELETE: u32 = 2;
const NIF_MESSAGE: u32 = 0x1;
const NIF_ICON: u32 = 0x2;
const NIF_TIP: u32 = 0x4;
const NIF_INFO: u32 = 0x10;
const MF_STRING: u32 = 0;
const MF_SEPARATOR: u32 = 0x800;
const TPM_RETURNCMD: u32 = 0x100;
const TPM_NONOTIFY: u32 = 0x80;
const ITEM_ABRIR: usize = 1;
const ITEM_SAIR: usize = 2;

#[repr(C)]
struct ClasseJanela {
    tamanho: u32,
    estilo: u32,
    procedimento: extern "system" fn(Hwnd, u32, usize, isize) -> isize,
    extra_classe: i32,
    extra_janela: i32,
    instancia: Handle,
    icone: Handle,
    cursor: Handle,
    fundo: Handle,
    menu: *const u16,
    nome: *const u16,
    icone_pequeno: Handle,
}

#[repr(C)]
struct Mensagem {
    hwnd: Hwnd,
    mensagem: u32,
    w: usize,
    l: isize,
    hora: u32,
    x: i32,
    y: i32,
    privado: u32,
}

#[repr(C)]
struct DadosIcone {
    tamanho: u32,
    hwnd: Hwnd,
    id: u32,
    bandeiras: u32,
    mensagem: u32,
    icone: Handle,
    dica: [u16; 128],
    estado: u32,
    mascara_estado: u32,
    info: [u16; 256],
    versao: u32,
    titulo_info: [u16; 64],
    bandeiras_info: u32,
    guid: [u8; 16],
    icone_balao: Handle,
}

#[repr(C)]
struct InfoIcone {
    e_icone: i32,
    x: u32,
    y: u32,
    mascara: Handle,
    cor: Handle,
}

#[repr(C)]
struct Ponto {
    x: i32,
    y: i32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(c: *const ClasseJanela) -> u16;
    fn CreateWindowExW(
        estilo_ext: u32,
        classe: *const u16,
        titulo: *const u16,
        estilo: u32,
        x: i32,
        y: i32,
        l: i32,
        a: i32,
        pai: Hwnd,
        menu: Handle,
        inst: Handle,
        param: *mut c_void,
    ) -> Hwnd;
    fn DefWindowProcW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn GetMessageW(m: *mut Mensagem, h: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(m: *const Mensagem) -> i32;
    fn DispatchMessageW(m: *const Mensagem) -> isize;
    fn PostQuitMessage(codigo: i32);
    fn CreatePopupMenu() -> Handle;
    fn AppendMenuW(menu: Handle, bandeiras: u32, id: usize, texto: *const u16) -> i32;
    fn TrackPopupMenu(
        menu: Handle,
        bandeiras: u32,
        x: i32,
        y: i32,
        r: i32,
        h: Hwnd,
        ret: *const c_void,
    ) -> i32;
    fn DestroyMenu(menu: Handle) -> i32;
    fn SetForegroundWindow(h: Hwnd) -> i32;
    fn GetCursorPos(p: *mut Ponto) -> i32;
    fn CreateIconIndirect(i: *const InfoIcone) -> Handle;
}

#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(acao: u32, dados: *mut DadosIcone) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateBitmap(l: i32, a: i32, planos: u32, bits: u32, dados: *const c_void) -> Handle;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(nome: *const u16) -> Handle;
}

static URL: OnceLock<String> = OnceLock::new();
static JANELA: OnceLock<usize> = OnceLock::new();

/// Os pixels do icone (32x32, BGRA): circulo vermelhao com borda escura e
/// canto transparente. Puro, testavel em qualquer sistema.
pub fn pixels_do_icone() -> Vec<u8> {
    let mut p = vec![0u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            let r = (dx * dx + dy * dy).sqrt();
            let i = (y * 32 + x) * 4;
            let (b, g, rr, a) = if r <= 12.5 {
                (0x10, 0x4D, 0xFF, 0xFF) // #FF4D10
            } else if r <= 15.0 {
                (0x18, 0x04, 0x01, 0xFF) // #010418, a borda da marca
            } else {
                (0, 0, 0, 0)
            };
            p[i..i + 4].copy_from_slice(&[b, g, rr, a]);
        }
    }
    p
}

fn icone() -> Handle {
    let cor_px = pixels_do_icone();
    let mascara_px = [0u8; 32 * 32 / 8];
    // SAFETY: buffers do tamanho que CreateBitmap le (32x32 a 32 e a 1 bit).
    unsafe {
        let cor = CreateBitmap(32, 32, 1, 32, cor_px.as_ptr() as *const c_void);
        let mascara = CreateBitmap(32, 32, 1, 1, mascara_px.as_ptr() as *const c_void);
        CreateIconIndirect(&InfoIcone {
            e_icone: 1,
            x: 0,
            y: 0,
            mascara,
            cor,
        })
    }
}

fn copiar<const N: usize>(texto: &str) -> [u16; N] {
    let mut a = [0u16; N];
    for (i, c) in texto.encode_utf16().take(N - 1).enumerate() {
        a[i] = c;
    }
    a
}

fn dados(h: Hwnd) -> DadosIcone {
    DadosIcone {
        tamanho: std::mem::size_of::<DadosIcone>() as u32,
        hwnd: h,
        id: 1,
        bandeiras: 0,
        mensagem: WM_BANDEJA,
        icone: std::ptr::null_mut(),
        dica: [0; 128],
        estado: 0,
        mascara_estado: 0,
        info: [0; 256],
        versao: 0,
        titulo_info: [0; 64],
        bandeiras_info: 0,
        guid: [0; 16],
        icone_balao: std::ptr::null_mut(),
    }
}

fn abrir() {
    if let Some(u) = URL.get() {
        crate::mesa::abrir_janela(u);
    }
}

fn sair(h: Hwnd) {
    let mut d = dados(h);
    // SAFETY: estrutura valida; remove o icone antes de o processo acabar
    // (sem isso o icone «fantasma» fica ate o mouse passar por cima).
    unsafe { Shell_NotifyIconW(NIM_DELETE, &mut d) };
    std::process::exit(0);
}

fn menu(h: Hwnd) {
    // SAFETY: menu criado e destruido aqui; textos vivos durante a chamada.
    unsafe {
        let m = CreatePopupMenu();
        let abrir_t = largo("Abrir phxvpn");
        let sair_t = largo("Sair");
        AppendMenuW(m, MF_STRING, ITEM_ABRIR, abrir_t.as_ptr());
        AppendMenuW(m, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(m, MF_STRING, ITEM_SAIR, sair_t.as_ptr());
        let mut p = Ponto { x: 0, y: 0 };
        GetCursorPos(&mut p);
        // Sem o SetForegroundWindow o menu nao fecha ao clicar fora (regra
        // documentada do TrackPopupMenu para icone de bandeja).
        SetForegroundWindow(h);
        let escolha = TrackPopupMenu(
            m,
            TPM_RETURNCMD | TPM_NONOTIFY,
            p.x,
            p.y,
            0,
            h,
            std::ptr::null(),
        );
        DestroyMenu(m);
        match escolha as usize {
            ITEM_ABRIR => abrir(),
            ITEM_SAIR => sair(h),
            _ => {}
        }
    }
}

extern "system" fn procedimento(h: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_BANDEJA => {
            match (l as u32) & 0xFFFF {
                WM_LBUTTONDBLCLK => abrir(),
                WM_RBUTTONUP => menu(h),
                _ => {}
            }
            0
        }
        WM_COMMAND => 0,
        WM_DESTROY => {
            // SAFETY: sem argumentos alem do codigo.
            unsafe { PostQuitMessage(0) };
            0
        }
        // SAFETY: repassa o que nao e nosso ao procedimento padrao.
        _ => unsafe { DefWindowProcW(h, msg, w, l) },
    }
}

/// Poe o icone na bandeja e roda o laco de mensagens ate «Sair».
pub fn rodar(url: String) -> Result<(), String> {
    let _ = URL.set(url);
    let nome = largo("phxvpn-bandeja");
    // SAFETY: estruturas validas e vivas durante as chamadas; a janela nunca
    // e mostrada (so recebe as mensagens do icone).
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let classe = ClasseJanela {
            tamanho: std::mem::size_of::<ClasseJanela>() as u32,
            estilo: 0,
            procedimento,
            extra_classe: 0,
            extra_janela: 0,
            instancia: inst,
            icone: std::ptr::null_mut(),
            cursor: std::ptr::null_mut(),
            fundo: std::ptr::null_mut(),
            menu: std::ptr::null(),
            nome: nome.as_ptr(),
            icone_pequeno: std::ptr::null_mut(),
        };
        if RegisterClassExW(&classe) == 0 {
            return Err(format!(
                "RegisterClassExW: {}",
                std::io::Error::last_os_error()
            ));
        }
        let titulo = largo("phxvpn");
        let h = CreateWindowExW(
            0,
            nome.as_ptr(),
            titulo.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            inst,
            std::ptr::null_mut(),
        );
        if h.is_null() {
            return Err(format!(
                "CreateWindowExW: {}",
                std::io::Error::last_os_error()
            ));
        }
        let _ = JANELA.set(h as usize);
        let mut d = dados(h);
        d.bandeiras = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_INFO;
        d.icone = icone();
        d.dica = copiar("phxvpn -- redes virtuais");
        d.titulo_info = copiar("phxvpn");
        d.info = copiar("O phxvpn esta na bandeja. Duplo clique abre a janela.");
        if Shell_NotifyIconW(NIM_ADD, &mut d) == 0 {
            return Err("Shell_NotifyIconW recusou o icone (sem area de notificacao?)".into());
        }
        let mut m = std::mem::zeroed::<Mensagem>();
        while GetMessageW(&mut m, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&m);
            DispatchMessageW(&m);
        }
        let mut d = dados(h);
        Shell_NotifyIconW(NIM_DELETE, &mut d);
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn icone_tem_o_vermelhao_no_meio_e_canto_transparente() {
        let p = pixels_do_icone();
        let px = |x: usize, y: usize| &p[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
        assert_eq!(px(16, 16), &[0x10, 0x4D, 0xFF, 0xFF], "BGRA de #FF4D10");
        assert_eq!(px(0, 0)[3], 0, "o canto tem de ser transparente");
        assert_eq!(
            std::mem::size_of::<DadosIcone>(),
            if cfg!(target_pointer_width = "64") {
                976
            } else {
                956
            }
        );
    }
}
