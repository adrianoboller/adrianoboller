//! Provas contra o sistema operacional: PTY de verdade, bash de verdade, /proc de verdade.
//! Teste unitario nao prova que o filho morreu -- so o /proc prova.

use phxclaw_terminal::{Atualizacao, Programa, Tamanho, Tecla, Terminal};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn bash_limpo() -> Programa {
    Programa {
        programa: "bash".into(),
        args: vec!["--norc".into(), "--noprofile".into()],
        // Prompt fixo: o do usuario muda de maquina para maquina e pode conter o texto
        // que o teste procura.
        env: vec![("PS1".into(), "$ ".into())],
        ..Programa::default()
    }
}

fn abrir(p: Programa, colunas: u16) -> (Terminal, mpsc::Receiver<Atualizacao>) {
    let (tx, rx) = mpsc::channel();
    let t = Terminal::abrir(
        p,
        Tamanho {
            colunas,
            linhas: 24,
        },
        move |a| {
            let _ = tx.send(a);
        },
    )
    .expect("abrir o PTY");
    (t, rx)
}

fn esperar_linha(t: &Terminal, alvo: &str, prazo: Duration) -> String {
    let fim = Instant::now() + prazo;
    loop {
        let texto = t.texto();
        if texto.lines().any(|l| l.trim() == alvo) {
            return texto;
        }
        assert!(
            Instant::now() < fim,
            "linha {alvo:?} nao apareceu na grade em {prazo:?}; tela:\n{texto}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn processo_existe(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[test]
fn bash_calcula_e_o_resultado_aparece_na_grade() {
    let (t, _rx) = abrir(bash_limpo(), 80);
    t.escrever(b"echo phx$((40+2))\n").unwrap();
    // A linha do comando tem "phx$((40+2))", nao "phx42": so a saida do bash casa.
    let tela = esperar_linha(&t, "phx42", Duration::from_secs(10));
    assert!(tela.contains("echo phx$((40+2))"), "{tela}");
}

#[test]
fn redimensionar_muda_o_tput_cols_e_a_grade() {
    let (t, _rx) = abrir(bash_limpo(), 80);
    t.escrever(b"tput cols\n").unwrap();
    esperar_linha(&t, "80", Duration::from_secs(10));
    t.escrever(b"clear\n").unwrap();
    t.redimensionar(Tamanho {
        colunas: 123,
        linhas: 30,
    })
    .unwrap();
    assert_eq!(
        t.tamanho(),
        Tamanho {
            colunas: 123,
            linhas: 30
        }
    );
    t.escrever(b"tput cols; tput lines\n").unwrap();
    esperar_linha(&t, "123", Duration::from_secs(10));
    esperar_linha(&t, "30", Duration::from_secs(10));
    assert_eq!(t.grade().colunas, 123);
}

#[test]
fn soltar_mata_o_filho_e_o_proc_confirma() {
    let (t, _rx) = abrir(bash_limpo(), 80);
    let pid = t.pid();
    t.escrever(b"echo vivo\n").unwrap();
    esperar_linha(&t, "vivo", Duration::from_secs(10));
    assert!(
        processo_existe(pid),
        "o bash devia estar vivo antes de soltar"
    );
    drop(t);
    assert!(
        !processo_existe(pid),
        "/proc/{pid} ainda existe depois de soltar o terminal"
    );
}

/// Filho surdo ao SIGHUP: o Drop do PTY do alacritty esperaria para sempre. O prazo vem
/// de um fio a parte para o teste FALHAR (e nao travar a suite) se o conserto sair.
#[test]
fn filho_que_ignora_sighup_morre_mesmo_assim() {
    let p = Programa {
        programa: "bash".into(),
        args: vec![
            "--norc".into(),
            "-c".into(),
            "trap '' HUP; echo surdo; while :; do sleep 0.05; done".into(),
        ],
        ..Programa::default()
    };
    let (t, _rx) = abrir(p, 80);
    let pid = t.pid();
    esperar_linha(&t, "surdo", Duration::from_secs(10));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        drop(t);
        let _ = tx.send(());
    });
    assert!(
        rx.recv_timeout(Duration::from_secs(5)).is_ok(),
        "soltar o terminal travou: o filho surdo ao SIGHUP nao foi morto"
    );
    assert!(!processo_existe(pid), "/proc/{pid} ainda existe");
}

/// O programa pergunta a posicao do cursor (DSR 6) e espera a resposta: o emulador a
/// produz, mas quem a devolve ao PTY e este motor. Sem devolver, o `read` espera para
/// sempre -- e o hx, que consulta o terminal ao subir, ficaria parado do mesmo jeito.
#[test]
fn consulta_do_programa_ao_terminal_e_respondida() {
    let p = Programa {
        programa: "bash".into(),
        args: vec![
            "--norc".into(),
            "-c".into(),
            "printf '\\033[6n'; IFS= read -rsd R -t 5 pos; echo; echo \"pos=${pos#*[}\"; sleep 30"
                .into(),
        ],
        ..Programa::default()
    };
    let (t, _rx) = abrir(p, 80);
    esperar_linha(&t, "pos=1;1", Duration::from_secs(10));
}

#[test]
fn uma_tecla_muda_so_a_linha_do_prompt() {
    let (t, rx) = abrir(bash_limpo(), 80);
    t.escrever(b"echo pronto\n").unwrap();
    esperar_linha(&t, "pronto", Duration::from_secs(10));
    let primeira = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(primeira.completa, "a primeira entrega tem de vir completa");
    assert_eq!(primeira.linhas_alteradas.len(), 24);
    // Esvazia o que o echo produziu.
    std::thread::sleep(Duration::from_millis(300));
    while rx.try_recv().is_ok() {}
    t.tecla(&Tecla::simples("x")).unwrap();
    let at = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(!at.completa);
    assert_eq!(
        at.linhas_alteradas.len(),
        1,
        "uma tecla ecoada devia mudar uma linha so: {:?}",
        at.linhas_alteradas
    );
    let linha = &at.linhas_alteradas[0];
    let texto: String = linha.trechos.iter().map(|t| t.texto.as_str()).collect();
    assert_eq!(texto, "$ x");
    let cursor = at.cursor.expect("cursor visivel");
    assert_eq!((cursor.x, cursor.y), (3, linha.y));
}

#[test]
fn cor_e_estilo_chegam_resolvidos() {
    let (t, _rx) = abrir(bash_limpo(), 80);
    t.escrever(b"printf '\\033[1;38;2;1;2;3mRGB\\033[0m\\n'\n")
        .unwrap();
    esperar_linha(&t, "RGB", Duration::from_secs(10));
    let g = t.grade();
    let trecho = g
        .linhas_alteradas
        .iter()
        .flat_map(|l| &l.trechos)
        .find(|tr| tr.texto == "RGB")
        .expect("trecho RGB isolado pelo estilo");
    assert_eq!(trecho.frente, 0x010203);
    assert_eq!(trecho.estilo & 1, 1, "negrito");
    assert_eq!(g.fundo, 0x010418, "fundo da marca");
}

#[test]
fn saida_do_filho_chega_na_atualizacao_e_escrever_depois_falha() {
    let p = Programa {
        programa: "bash".into(),
        args: vec!["--norc".into(), "-c".into(), "exit 7".into()],
        ..Programa::default()
    };
    let (t, rx) = abrir(p, 80);
    let fim = Instant::now() + Duration::from_secs(10);
    let saida = loop {
        if let Ok(a) = rx.recv_timeout(Duration::from_millis(200))
            && let Some(s) = a.encerrado
        {
            break s;
        }
        assert!(Instant::now() < fim, "o fim do filho nao chegou");
    };
    assert_eq!(saida.codigo, Some(7));
    assert!(t.escrever(b"x").is_err());
}

/// O Helix sobe no PTY do motor e desenha a linha de estado (`NOR`) -- a prova de que o
/// `helix::programa` monta um hx que roda, e nao so um caminho que existe. Sem hx no
/// hospedeiro, nada a provar. Medido aqui porque `hx .` sob um pty de 0x0 (o `script` sem
/// tamanho) estoura no picker; o motor abre com tamanho real.
#[test]
fn o_helix_sobe_no_pty_e_desenha_a_linha_de_estado() {
    if phxclaw_terminal::helix::achar(None).is_none() {
        phxclaw_test_support::pulado::pular("hx", "hx ausente neste hospedeiro");
        return;
    }
    let dir = std::env::temp_dir().join(format!("phx-term-hx-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.rs"), "fn main() {}\n").unwrap();
    let p = phxclaw_terminal::helix::programa(&dir, None, vec![]).unwrap();
    let t = phxclaw_terminal::Terminal::abrir(
        p,
        phxclaw_terminal::Tamanho {
            colunas: 100,
            linhas: 30,
        },
        |_| {},
    )
    .unwrap();
    let fim = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut texto = String::new();
    while std::time::Instant::now() < fim {
        texto = t.texto();
        if phxclaw_terminal::helix::arquivo_corrente(&texto).is_some() || texto.contains("NOR") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(
        texto.contains("NOR"),
        "o Helix nao desenhou: encerrado={:?}\n{texto}",
        t.encerrado()
    );
    drop(t);
    let _ = std::fs::remove_dir_all(&dir);
}
