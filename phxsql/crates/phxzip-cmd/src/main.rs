//! `phxzipcmd` -- o PhxZip no terminal. Toda a logica mora em `lib.rs`; este
//! arquivo so liga a saida padrao e o codigo de saida.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = std::io::stdout().lock();
    let mut err = std::io::stderr().lock();
    ExitCode::from(phxzip_cmd::rodar(&args, &mut out, &mut err))
}
