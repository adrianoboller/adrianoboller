//! Roda um programa no motor de terminal e imprime a grade em JSON, no mesmo formato do
//! evento `terminal_grade` do desktop. Serve a prova da tela: o stub do Tauri no roteiro do
//! Playwright reproduz uma grade que SAIU DESTE MOTOR, e nao uma inventada a mao.
//!
//! cargo run -p phxclaw-terminal --example retrato -- COLUNAS LINHAS ESPERA_MS ENTRADA PROGRAMA [ARGS...]
//! (ENTRADA aceita \n como Enter; vazia para nao digitar nada)

use phxclaw_terminal::{Programa, Tamanho, Terminal};
use std::time::Duration;

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 5 {
        return Err("uso: retrato COLUNAS LINHAS ESPERA_MS ENTRADA PROGRAMA [ARGS...]".into());
    }
    let num = |i: usize| a[i].parse::<u64>().map_err(|e| format!("{}: {e}", a[i]));
    let tamanho = Tamanho {
        colunas: num(0)? as u16,
        linhas: num(1)? as u16,
    };
    let espera = Duration::from_millis(num(2)?);
    let entrada = a[3].replace("\\n", "\n");
    let t = Terminal::abrir(
        Programa {
            programa: a[4].clone(),
            args: a[5..].to_vec(),
            cwd: std::env::current_dir().ok(),
            env: vec![("PS1".into(), "phxclaw$ ".into())],
        },
        tamanho,
        |_| {},
    )
    .map_err(|e| e.to_string())?;
    std::thread::sleep(espera / 2);
    t.escrever(entrada.as_bytes()).map_err(|e| e.to_string())?;
    std::thread::sleep(espera / 2);
    println!(
        "{}",
        serde_json::to_string(&t.grade()).map_err(|e| e.to_string())?
    );
    Ok(())
}
