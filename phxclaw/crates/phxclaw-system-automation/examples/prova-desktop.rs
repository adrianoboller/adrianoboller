//! Prova do portao `desktop_os_automation_e2e` num desktop de verdade: teclado, mouse,
//! captura e shell governado, pelos mesmos tipos que o agente usa (`ShellExecutor`,
//! `EnigoInputProvider`, `capturar_monitor_principal`).
//!
//! - politica: a padrao NEGA lancar programa, e a lista de negados recusa `cmd.exe`;
//! - captura: a tela primaria vira imagem, e a imagem nao e de uma cor so;
//! - mouse: o cursor vai ao centro e o SO devolve a mesma posicao;
//! - teclado + shell (Windows): o Bloco de Notas abre pelo executor governado, recebe um
//!   codigo aleatorio pelo teclado, salva pelo Ctrl+S e o arquivo gravado traz o codigo;
//! - teclado + shell (Linux): um xterm abre pelo executor governado com um `read` esperando,
//!   recebe o codigo pelo teclado e o grava num arquivo, que tem de trazer o codigo.
//!
//! Sai 0 so com todas as checagens verdes; imprime `placar: N/M`. Nao mexa no teclado
//! nem no mouse enquanto roda (uns 15 s).

use phxclaw_system_automation::{
    AutomationError, EnigoInputProvider, ExecutionPolicy, InputAction, InputProvider,
    LaunchRequest, ShellExecutor, capturar_monitor_principal,
};
use std::time::Duration;

fn main() {
    let mut placar: Vec<(&str, bool, String)> = vec![];

    // politica: desligada por padrao, e a lista de negados vale com ela ligada
    let padrao = ShellExecutor::new(ExecutionPolicy::default());
    let negado = ShellExecutor::new(ExecutionPolicy {
        enabled: true,
        denied_programs: vec!["cmd.exe".into()],
        ..ExecutionPolicy::default()
    });
    let ok = matches!(
        padrao.launch(&LaunchRequest::new("notepad.exe", vec![])),
        Err(AutomationError::Disabled)
    ) && matches!(
        negado.launch(&LaunchRequest::new("CMD.EXE", vec![])),
        Err(AutomationError::DeniedProgram(_))
    );
    placar.push(("politica", ok, "padrao nega; cmd.exe negado".into()));

    // captura
    match capturar_monitor_principal() {
        Ok(img) => {
            let primeiro = img.get_pixel(0, 0);
            let diferentes = img.pixels().filter(|p| *p != primeiro).count();
            let _ = img.save("prova-desktop-captura.png");
            placar.push((
                "captura",
                img.width() > 0 && diferentes > 0,
                format!(
                    "{}x{}, {diferentes} pixels diferentes do primeiro; prova-desktop-captura.png",
                    img.width(),
                    img.height()
                ),
            ));
        }
        Err(e) => placar.push(("captura", false, e.to_string())),
    }

    // mouse
    let mut entrada = match EnigoInputProvider::new() {
        Ok(e) => e,
        Err(e) => {
            placar.push(("mouse", false, e));
            return fim(placar);
        }
    };
    let (x, y) = capturar_monitor_principal()
        .map(|i| (i.width() as i32 / 2, i.height() as i32 / 2))
        .unwrap_or((200, 200));
    let r = entrada.apply(&InputAction::MoveMouse { x, y });
    std::thread::sleep(Duration::from_millis(300));
    let onde = entrada.posicao_do_mouse();
    placar.push((
        "mouse",
        r.is_ok() && onde == Ok((x, y)),
        format!("pedido ({x},{y}), SO diz {onde:?}"),
    ));

    #[cfg(any(windows, target_os = "linux"))]
    placar.push(teclado_e_shell(&mut entrada));
    fim(placar)
}

#[cfg(windows)]
fn teclado_e_shell(entrada: &mut EnigoInputProvider) -> (&'static str, bool, String) {
    let codigo = uuid::Uuid::now_v7().simple().to_string();
    let arquivo = std::env::temp_dir().join(format!("phxclaw-prova-{}.txt", &codigo[..12]));
    let executor = ShellExecutor::new(ExecutionPolicy {
        enabled: true,
        denied_programs: vec!["cmd.exe".into(), "powershell.exe".into()],
        ..ExecutionPolicy::default()
    });
    if let Err(e) = executor.launch(&LaunchRequest::new("notepad.exe", vec![])) {
        return ("teclado+shell", false, format!("lancar: {e}"));
    }
    let passo = |a: InputAction, e: &mut EnigoInputProvider, ms: u64| {
        let r = e.apply(&a);
        std::thread::sleep(Duration::from_millis(ms));
        r
    };
    std::thread::sleep(Duration::from_secs(3));
    let passos = [
        (
            InputAction::Text {
                text: codigo.clone(),
            },
            500,
        ),
        (
            InputAction::Hotkey {
                keys: vec!["ctrl".into(), "s".into()],
            },
            2000,
        ),
        (
            InputAction::Text {
                text: arquivo.display().to_string(),
            },
            500,
        ),
        (
            InputAction::Key {
                key: "enter".into(),
                state: "click".into(),
            },
            2000,
        ),
        (
            InputAction::Hotkey {
                keys: vec!["alt".into(), "f4".into()],
            },
            1000,
        ),
    ];
    for (a, ms) in passos {
        if let Err(e) = passo(a, entrada, ms) {
            return ("teclado+shell", false, format!("entrada: {e}"));
        }
    }
    let lido = std::fs::read_to_string(&arquivo).unwrap_or_default();
    let _ = std::fs::remove_file(&arquivo);
    (
        "teclado+shell",
        lido.contains(&codigo),
        format!(
            "Bloco de Notas pelo executor; {} gravado com o codigo: {}",
            arquivo.display(),
            lido.contains(&codigo)
        ),
    )
}

#[cfg(target_os = "linux")]
fn teclado_e_shell(entrada: &mut EnigoInputProvider) -> (&'static str, bool, String) {
    let codigo = uuid::Uuid::now_v7().simple().to_string();
    let arquivo = std::env::temp_dir().join(format!("phxclaw-prova-{}.txt", &codigo[..12]));
    let executor = ShellExecutor::new(ExecutionPolicy {
        enabled: true,
        denied_programs: vec!["sh".into(), "bash".into()],
        ..ExecutionPolicy::default()
    });
    // o sh roda DENTRO do xterm: o executor lanca so o xterm, que nao esta negado
    let lancado = executor.launch(&LaunchRequest::new(
        "xterm",
        vec![
            "-geometry".into(),
            "60x5+40+40".into(),
            "-e".into(),
            "sh".into(),
            "-c".into(),
            format!("read l; printf %s \"$l\" > '{}'", arquivo.display()),
        ],
    ));
    if let Err(e) = lancado {
        return ("teclado+shell", false, format!("lancar o xterm: {e}"));
    }
    std::thread::sleep(Duration::from_secs(2));
    // sem gerenciador de janelas o foco segue o ponteiro: o cursor vai para dentro do xterm
    let passos = [
        (InputAction::MoveMouse { x: 120, y: 70 }, 300),
        (
            InputAction::Text {
                text: codigo.clone(),
            },
            300,
        ),
        (
            InputAction::Key {
                key: "enter".into(),
                state: "click".into(),
            },
            1500,
        ),
    ];
    for (a, ms) in passos {
        if let Err(e) = entrada.apply(&a) {
            return ("teclado+shell", false, format!("entrada: {e}"));
        }
        std::thread::sleep(Duration::from_millis(ms));
    }
    let lido = std::fs::read_to_string(&arquivo).unwrap_or_default();
    let _ = std::fs::remove_file(&arquivo);
    (
        "teclado+shell",
        lido == codigo,
        format!(
            "xterm pelo executor; {} gravado com o codigo: {}",
            arquivo.display(),
            lido == codigo
        ),
    )
}

fn fim(placar: Vec<(&str, bool, String)>) {
    for (nome, ok, det) in &placar {
        println!("{} {nome}: {det}", if *ok { "OK  " } else { "FALHA" });
    }
    let verdes = placar.iter().filter(|p| p.1).count();
    println!("placar: {verdes}/{}", placar.len());
    std::process::exit(if verdes == placar.len() { 0 } else { 1 });
}
