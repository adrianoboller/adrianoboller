//! Motor de terminal do IDE do PhxClaw.
//!
//! Um PTY com um processo dentro (bash, hx), o emulador VT do alacritty_terminal lendo a
//! saida, e a grade de celulas exposta por DIFERENCA: quem desenha recebe so as linhas que
//! mudaram desde a ultima entrega. A tela do IDE e uma grade de ate ~200x60; mandar a grade
//! inteira a cada tecla seria mandar dezenas de KB de JSON por eco de caractere.
//!
//! Ciclo de vida: `Terminal::abrir` sobe o processo; soltar o `Terminal` fecha o PTY e mata
//! o filho -- e o mata mesmo que ele ignore o SIGHUP, porque o Drop do PTY do alacritty
//! espera o filho sem prazo, e um filho surdo travaria quem soltou.

pub mod cores;
pub mod teclas;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg, State};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty::{self, Pty, Shell};
use alacritty_terminal::vte::ansi::{CursorShape, Rgb};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

pub use teclas::Tecla;

/// O processo que roda dentro do terminal.
#[derive(Debug, Clone, Default)]
pub struct Programa {
    pub programa: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tamanho {
    pub colunas: u16,
    pub linhas: u16,
}

impl Tamanho {
    /// Um terminal de 0 coluna derruba o emulador (indice `colunas - 1`); de 1000 vira
    /// alocacao de grade gigante por um numero errado vindo da tela.
    fn limitado(self) -> Self {
        Self {
            colunas: self.colunas.clamp(2, 1000),
            linhas: self.linhas.clamp(1, 500),
        }
    }

    fn janela(self) -> WindowSize {
        // A largura em pixel so serve a quem pergunta (TIOCGWINSZ); a tela desenha por
        // celula, entao vai uma celula nominal.
        WindowSize {
            num_lines: self.linhas,
            num_cols: self.colunas,
            cell_width: 8,
            cell_height: 16,
        }
    }
}

/// Um pedaco de linha com o mesmo estilo. `x` e a coluna onde comeca; cada `char` do
/// texto ocupa uma coluna, exceto em trecho isolado (caractere largo ou com acento
/// combinante), que vem sozinho para a tela nao desalinhar o resto da linha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trecho {
    pub x: u16,
    pub texto: String,
    /// 0xRRGGBB.
    pub frente: u32,
    pub fundo: u32,
    /// Bits: 1 negrito, 2 italico, 4 sublinhado, 8 riscado.
    pub estilo: u8,
    /// Colunas que o trecho ocupa quando isolado (2 para caractere largo).
    pub largura: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinhaGrade {
    pub y: u16,
    pub trechos: Vec<Trecho>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub forma: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Saida {
    pub codigo: Option<i32>,
}

/// O que a tela recebe: as linhas que mudaram, ou todas quando `completa`.
#[derive(Debug, Clone, Serialize)]
pub struct Atualizacao {
    pub colunas: u16,
    pub linhas: u16,
    pub completa: bool,
    pub linhas_alteradas: Vec<LinhaGrade>,
    pub cursor: Option<Cursor>,
    pub fundo: u32,
    pub frente: u32,
    pub titulo: Option<String>,
    pub encerrado: Option<Saida>,
}

enum Aviso {
    Acordar,
    Responder(Vec<u8>),
    Cor(usize, Arc<dyn Fn(Rgb) -> String + Sync + Send + 'static>),
    TamanhoPedido(Arc<dyn Fn(WindowSize) -> String + Sync + Send + 'static>),
    Titulo(Option<String>),
    Parar,
}

/// O ouvinte do emulador so repassa: `send_event` pode ser chamado com a grade travada
/// pelo laco de leitura, e travar de novo aqui (para responder uma consulta de cor) seria
/// um deadlock. Quem trava e responde e o fio observador.
#[derive(Clone)]
struct Ouvinte {
    avisos: mpsc::Sender<Aviso>,
    saiu: Arc<AtomicBool>,
    codigo: Arc<Mutex<Option<i32>>>,
}

impl EventListener for Ouvinte {
    fn send_event(&self, event: Event) {
        let aviso = match event {
            Event::Wakeup => Aviso::Acordar,
            Event::PtyWrite(s) => Aviso::Responder(s.into_bytes()),
            Event::ColorRequest(i, f) => Aviso::Cor(i, f),
            Event::TextAreaSizeRequest(f) => Aviso::TamanhoPedido(f),
            Event::Title(t) => Aviso::Titulo(Some(t)),
            Event::ResetTitle => Aviso::Titulo(None),
            Event::ChildExit(status) => {
                if let Ok(mut c) = self.codigo.lock() {
                    *c = status.code();
                }
                // Marcado aqui, no fio que colheu o filho, e nao no observador: o Drop
                // decide por esta marca se ainda pode mandar sinal para o pid, e um pid
                // ja colhido pode ter sido reusado por outro processo.
                self.saiu.store(true, Ordering::SeqCst);
                Aviso::Acordar
            }
            _ => return,
        };
        let _ = self.avisos.send(aviso);
    }
}

struct Compartilhado {
    term: Arc<FairMutex<Term<Ouvinte>>>,
    anterior: Mutex<Vec<Vec<Trecho>>>,
    titulo: Mutex<Option<String>>,
    titulo_mudou: AtomicBool,
    /// Largura da ultima entrega; 0 antes da primeira, para ela sair completa.
    largura: AtomicU64,
    saiu: Arc<AtomicBool>,
    codigo: Arc<Mutex<Option<i32>>>,
}

type Laco = EventLoop<Pty, Ouvinte>;

pub struct Terminal {
    comp: Arc<Compartilhado>,
    laco: EventLoopSender,
    fio_laco: Option<JoinHandle<(Laco, State)>>,
    fio_observador: Option<JoinHandle<()>>,
    avisos: mpsc::Sender<Aviso>,
    pid: u32,
}

static PROXIMA_JANELA: AtomicU64 = AtomicU64::new(1);

impl Terminal {
    /// Sobe `programa` num PTY de `tamanho`. `ao_mudar` e chamado de um fio proprio a cada
    /// lote de saida, com so o que mudou (a primeira chamada vem completa).
    pub fn abrir<F>(programa: Programa, tamanho: Tamanho, ao_mudar: F) -> io::Result<Self>
    where
        F: Fn(Atualizacao) + Send + 'static,
    {
        let tamanho = tamanho.limitado();
        let (tx, rx) = mpsc::channel();
        let saiu = Arc::new(AtomicBool::new(false));
        let codigo = Arc::new(Mutex::new(None));
        let ouvinte = Ouvinte {
            avisos: tx.clone(),
            saiu: saiu.clone(),
            codigo: codigo.clone(),
        };

        let mut env: HashMap<String, String> = programa.env.into_iter().collect();
        // Sem TERM o filho herdaria o do processo do PhxClaw (vazio numa janela grafica, ou
        // "dumb"), e o hx desenharia sem cor. O emulador e compativel com xterm-256color.
        env.entry("TERM".into())
            .or_insert_with(|| "xterm-256color".into());
        env.entry("COLORTERM".into())
            .or_insert_with(|| "truecolor".into());
        let opcoes = tty::Options {
            shell: Some(Shell::new(programa.programa, programa.args)),
            working_directory: programa.cwd,
            drain_on_exit: true,
            env,
            #[cfg(windows)]
            escape_args: true,
        };
        let janela = PROXIMA_JANELA.fetch_add(1, Ordering::Relaxed);
        let pty = tty::new(&opcoes, tamanho.janela(), janela)?;
        #[cfg(unix)]
        let pid = pty.child().id();
        #[cfg(not(unix))]
        let pid = 0;

        let config = Config {
            // O protocolo de teclado do kitty desligado: a tela manda tecla no formato
            // xterm (`teclas.rs`), e anunciar o kitty faria o hx esperar o outro formato.
            kitty_keyboard: false,
            ..Config::default()
        };
        let dims = TermSize::new(usize::from(tamanho.colunas), usize::from(tamanho.linhas));
        let term = Arc::new(FairMutex::new(Term::new(config, &dims, ouvinte.clone())));
        let laco = EventLoop::new(term.clone(), ouvinte, pty, true, false)?;
        let remetente = laco.channel();
        let fio_laco = laco.spawn();

        let comp = Arc::new(Compartilhado {
            term,
            anterior: Mutex::new(Vec::new()),
            titulo: Mutex::new(None),
            titulo_mudou: AtomicBool::new(false),
            largura: AtomicU64::new(0),
            saiu,
            codigo,
        });
        let fio_observador = {
            let comp = comp.clone();
            let remetente = remetente.clone();
            std::thread::Builder::new()
                .name("phxclaw-terminal-grade".into())
                .spawn(move || observar(comp, remetente, rx, ao_mudar))?
        };
        // A primeira grade vai mesmo sem saida nenhuma: a tela precisa do tamanho e do
        // fundo antes de o programa escrever o primeiro byte.
        let _ = tx.send(Aviso::Acordar);
        Ok(Self {
            comp,
            laco: remetente,
            fio_laco: Some(fio_laco),
            fio_observador: Some(fio_observador),
            avisos: tx,
            pid,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn encerrado(&self) -> Option<Saida> {
        self.comp.saiu.load(Ordering::SeqCst).then(|| Saida {
            codigo: self.comp.codigo.lock().ok().and_then(|c| *c),
        })
    }

    fn enviar(&self, bytes: Vec<u8>) -> io::Result<()> {
        // O canal do laco continua aberto depois do fim do filho (o laco morto fica guardado
        // no JoinHandle ate o Drop), entao so a marca de saida diz que ninguem vai ler.
        if self.comp.saiu.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "terminal encerrado",
            ));
        }
        if bytes.is_empty() {
            return Ok(());
        }
        self.laco
            .send(Msg::Input(Cow::Owned(bytes)))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "terminal encerrado"))
    }

    /// Bytes crus para o processo. Quem digita volta a ver o fim da tela, como em qualquer
    /// terminal: escrever olhando o historico rolado e escrever as cegas.
    pub fn escrever(&self, bytes: &[u8]) -> io::Result<()> {
        self.ao_fundo();
        self.enviar(bytes.to_vec())
    }

    /// Uma tecla do navegador, traduzida conforme o modo atual do terminal.
    pub fn tecla(&self, t: &Tecla) -> io::Result<bool> {
        let app = self.comp.term.lock().mode().contains(TermMode::APP_CURSOR);
        match teclas::codificar(t, app) {
            Some(b) => self.escrever(&b).map(|_| true),
            None => Ok(false),
        }
    }

    /// Texto colado, delimitado se o programa pediu colagem delimitada.
    pub fn colar(&self, texto: &str) -> io::Result<()> {
        let delimitada = self
            .comp
            .term
            .lock()
            .mode()
            .contains(TermMode::BRACKETED_PASTE);
        self.escrever(&teclas::colar(texto, delimitada))
    }

    fn ao_fundo(&self) {
        let mut term = self.comp.term.lock();
        if term.grid().display_offset() != 0 {
            term.scroll_display(Scroll::Bottom);
            drop(term);
            let _ = self.avisos.send(Aviso::Acordar);
        }
    }

    /// Rola o historico (positivo sobe). Sem efeito na tela alternativa, que nao tem
    /// historico.
    pub fn rolar(&self, linhas: i32) {
        self.comp.term.lock().scroll_display(Scroll::Delta(linhas));
        let _ = self.avisos.send(Aviso::Acordar);
    }

    /// Muda o emulador e o PTY juntos: so o PTY faria o programa desenhar em 120 colunas
    /// numa grade de 80; so o emulador deixaria o `tput cols` mentindo.
    pub fn redimensionar(&self, tamanho: Tamanho) -> io::Result<()> {
        let tamanho = tamanho.limitado();
        self.comp.term.lock().resize(TermSize::new(
            usize::from(tamanho.colunas),
            usize::from(tamanho.linhas),
        ));
        self.laco
            .send(Msg::Resize(tamanho.janela()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "terminal encerrado"))?;
        // O bash parado no prompt nao escreve nada ao mudar de tamanho; sem este aviso a
        // tela ficaria com a grade velha ate a proxima tecla.
        let _ = self.avisos.send(Aviso::Acordar);
        Ok(())
    }

    pub fn tamanho(&self) -> Tamanho {
        let term = self.comp.term.lock();
        Tamanho {
            colunas: term.columns() as u16,
            linhas: term.screen_lines() as u16,
        }
    }

    /// A grade inteira, sem mexer na diferenca que o observador acompanha.
    pub fn grade(&self) -> Atualizacao {
        let (linhas, at) = self.comp.retrato();
        let mut at = at;
        at.completa = true;
        at.linhas_alteradas = linhas
            .into_iter()
            .enumerate()
            .map(|(y, trechos)| LinhaGrade {
                y: y as u16,
                trechos,
            })
            .collect();
        at
    }

    /// O texto visivel, uma linha por linha da tela, sem os espacos do fim.
    pub fn texto(&self) -> String {
        let term = self.comp.term.lock();
        let grid = term.grid();
        let deslocado = grid.display_offset() as i32;
        let mut saida = Vec::with_capacity(term.screen_lines());
        for y in 0..term.screen_lines() as i32 {
            let linha = &grid[Line(y - deslocado)];
            let mut s = String::new();
            for x in 0..term.columns() {
                let cel = &linha[Column(x)];
                if cel
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                s.push(cel.c);
            }
            saida.push(s.trim_end().to_string());
        }
        saida.join("\n")
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.laco.send(Msg::Shutdown);
        #[cfg(unix)]
        if !self.comp.saiu.load(Ordering::SeqCst) {
            encerrar_grupo(self.pid);
        }
        // Soltar o laco solta o PTY: o Drop dele manda SIGHUP e colhe o filho (que a esta
        // altura ja morreu, pelo encerrar_grupo), e fecha o descritor mestre.
        if let Some(f) = self.fio_laco.take() {
            let _ = f.join();
        }
        let _ = self.avisos.send(Aviso::Parar);
        if let Some(f) = self.fio_observador.take() {
            let _ = f.join();
        }
    }
}

/// SIGHUP no grupo do filho (o PTY o pos em sessao propria, entao o grupo e o pid), e
/// SIGKILL se em meio segundo ele nao morreu. Um filho com `trap '' HUP` sobreviveria ao
/// SIGHUP do alacritty, e o `wait` sem prazo dele travaria quem fechou o terminal.
#[cfg(unix)]
fn encerrar_grupo(pid: u32) {
    let grupo = -(pid as libc::pid_t);
    // SAFETY: kill so le os dois inteiros; o pid e o do filho que este terminal criou e
    // que ainda nao foi colhido (a marca `saiu` foi conferida antes), entao nao ha reuso.
    unsafe { libc::kill(grupo, libc::SIGHUP) };
    for _ in 0..50 {
        if !vivo(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // SAFETY: idem.
    unsafe { libc::kill(grupo, libc::SIGKILL) };
}

/// Zumbi conta como morto: ja nao roda, so espera ser colhido pelo Drop do PTY.
#[cfg(unix)]
fn vivo(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat
            .rsplit_once(')')
            .and_then(|(_, resto)| resto.split_whitespace().next())
            .is_some_and(|estado| estado != "Z" && estado != "X"),
        // Sem /proc (macOS): pergunta ao kernel se o pid existe.
        // SAFETY: sinal 0 nao entrega nada, so confere existencia.
        Err(_) => unsafe { libc::kill(pid as libc::pid_t, 0) == 0 },
    }
}

fn observar<F>(
    comp: Arc<Compartilhado>,
    laco: EventLoopSender,
    rx: mpsc::Receiver<Aviso>,
    ao_mudar: F,
) where
    F: Fn(Atualizacao),
{
    while let Ok(primeiro) = rx.recv() {
        let mut acordar = false;
        let mut parar = false;
        let tratar = |a: Aviso, acordar: &mut bool, parar: &mut bool| match a {
            Aviso::Acordar => *acordar = true,
            Aviso::Parar => *parar = true,
            Aviso::Responder(b) => {
                let _ = laco.send(Msg::Input(Cow::Owned(b)));
            }
            Aviso::Cor(i, f) => {
                let rgb = cores::por_indice(comp.term.lock().colors(), i);
                let _ = laco.send(Msg::Input(Cow::Owned(f(rgb).into_bytes())));
            }
            Aviso::TamanhoPedido(f) => {
                let t = {
                    let term = comp.term.lock();
                    Tamanho {
                        colunas: term.columns() as u16,
                        linhas: term.screen_lines() as u16,
                    }
                };
                let _ = laco.send(Msg::Input(Cow::Owned(f(t.janela()).into_bytes())));
            }
            Aviso::Titulo(t) => {
                if let Ok(mut atual) = comp.titulo.lock() {
                    *atual = t;
                }
                comp.titulo_mudou.store(true, Ordering::SeqCst);
                *acordar = true;
            }
        };
        let era_acordar = matches!(primeiro, Aviso::Acordar);
        tratar(primeiro, &mut acordar, &mut parar);
        if era_acordar && !parar {
            // Junta a rajada: um `ls` grande acorda o observador dezenas de vezes em
            // milissegundos, e uma entrega por rajada basta para a tela.
            std::thread::sleep(Duration::from_millis(4));
        }
        while let Ok(a) = rx.try_recv() {
            tratar(a, &mut acordar, &mut parar);
        }
        if parar {
            break;
        }
        if acordar {
            ao_mudar(comp.diferenca());
        }
    }
}

impl Compartilhado {
    /// Todas as linhas visiveis em trechos, mais o cabecalho da atualizacao.
    fn retrato(&self) -> (Vec<Vec<Trecho>>, Atualizacao) {
        let term = self.term.lock();
        let conteudo = term.renderable_content();
        let paleta = conteudo.colors;
        let fundo = cores::resolver(
            paleta,
            alacritty_terminal::vte::ansi::Color::Named(
                alacritty_terminal::vte::ansi::NamedColor::Background,
            ),
        );
        let frente = cores::resolver(
            paleta,
            alacritty_terminal::vte::ansi::Color::Named(
                alacritty_terminal::vte::ansi::NamedColor::Foreground,
            ),
        );
        let deslocado = conteudo.display_offset as i32;
        let colunas = term.columns();
        let n_linhas = term.screen_lines();
        let cursor = match conteudo.cursor.shape {
            CursorShape::Hidden => None,
            forma => {
                let y = conteudo.cursor.point.line.0 + deslocado;
                (y >= 0 && (y as usize) < n_linhas).then_some(Cursor {
                    x: conteudo.cursor.point.column.0 as u16,
                    y: y as u16,
                    forma: match forma {
                        CursorShape::Underline => "sublinhado",
                        CursorShape::Beam => "barra",
                        CursorShape::HollowBlock => "oco",
                        _ => "bloco",
                    },
                })
            }
        };
        let grid = term.grid();
        let mut linhas = Vec::with_capacity(n_linhas);
        for y in 0..n_linhas as i32 {
            let linha = &grid[Line(y - deslocado)];
            let mut trechos: Vec<Trecho> = Vec::new();
            let mut ultimo_isolado = false;
            for x in 0..colunas {
                let cel = &linha[Column(x)];
                if cel
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                let mut fg = cores::resolver(paleta, cel.fg);
                let mut bg = cores::resolver(paleta, cel.bg);
                if cel.flags.contains(Flags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if cel.flags.contains(Flags::DIM) {
                    fg = Rgb {
                        r: (u16::from(fg.r) * 2 / 3) as u8,
                        g: (u16::from(fg.g) * 2 / 3) as u8,
                        b: (u16::from(fg.b) * 2 / 3) as u8,
                    };
                }
                if cel.flags.contains(Flags::HIDDEN) {
                    fg = bg;
                }
                let estilo = u8::from(cel.flags.contains(Flags::BOLD))
                    | (u8::from(cel.flags.contains(Flags::ITALIC)) << 1)
                    | (u8::from(cel.flags.intersects(Flags::ALL_UNDERLINES)) << 2)
                    | (u8::from(cel.flags.contains(Flags::STRIKEOUT)) << 3);
                let (frente_c, fundo_c) = (cores::para_u32(fg), cores::para_u32(bg));
                let largo = cel.flags.contains(Flags::WIDE_CHAR);
                let combinante = cel.zerowidth().is_some_and(|z| !z.is_empty());
                let isolado = largo || combinante;
                let mut texto = String::new();
                texto.push(cel.c);
                if let Some(z) = cel.zerowidth() {
                    texto.extend(z.iter());
                }
                let junta = !isolado
                    && !ultimo_isolado
                    && trechos.last().is_some_and(|t| {
                        t.frente == frente_c && t.fundo == fundo_c && t.estilo == estilo
                    });
                if junta {
                    let t = trechos.last_mut().expect("conferido acima");
                    t.texto.push_str(&texto);
                    t.largura += 1;
                } else {
                    trechos.push(Trecho {
                        x: x as u16,
                        texto,
                        frente: frente_c,
                        fundo: fundo_c,
                        estilo,
                        largura: if largo { 2 } else { 1 },
                    });
                }
                ultimo_isolado = isolado;
            }
            // Espaco no fim com fundo padrao e o que a tela ja pinta ao limpar a linha:
            // manda-lo seria a maior parte do JSON de uma tela de shell.
            let fundo_u = cores::para_u32(fundo);
            while let Some(t) = trechos.last_mut() {
                if t.fundo != fundo_u
                    || t.estilo & 0b1100 != 0
                    || t.largura != t.texto.chars().count() as u16
                {
                    break;
                }
                let aparado = t.texto.trim_end_matches(' ').len();
                if aparado == 0 {
                    trechos.pop();
                    continue;
                }
                let tirados = t.texto.len() - aparado;
                t.texto.truncate(aparado);
                t.largura -= tirados as u16;
                break;
            }
            linhas.push(trechos);
        }
        let titulo_mudou = self.titulo_mudou.swap(false, Ordering::SeqCst);
        let at = Atualizacao {
            colunas: colunas as u16,
            linhas: n_linhas as u16,
            completa: false,
            linhas_alteradas: Vec::new(),
            cursor,
            fundo: cores::para_u32(fundo),
            frente: cores::para_u32(frente),
            titulo: if titulo_mudou {
                Some(
                    self.titulo
                        .lock()
                        .ok()
                        .and_then(|t| t.clone())
                        .unwrap_or_default(),
                )
            } else {
                None
            },
            encerrado: self.saiu.load(Ordering::SeqCst).then(|| Saida {
                codigo: self.codigo.lock().ok().and_then(|c| *c),
            }),
        };
        (linhas, at)
    }

    fn diferenca(&self) -> Atualizacao {
        let (linhas, mut at) = self.retrato();
        let mut anterior = self.anterior.lock().unwrap_or_else(|e| e.into_inner());
        // Mudou a largura, a linha antiga nao serve de base: o emulador refluiu tudo.
        let largura_antiga = self.largura.swap(u64::from(at.colunas), Ordering::SeqCst);
        let completa = anterior.len() != linhas.len() || largura_antiga != u64::from(at.colunas);
        at.completa = completa;
        at.linhas_alteradas = linhas
            .iter()
            .enumerate()
            .filter(|(y, l)| completa || anterior.get(*y) != Some(*l))
            .map(|(y, l)| LinhaGrade {
                y: y as u16,
                trechos: l.clone(),
            })
            .collect();
        *anterior = linhas;
        at
    }
}
