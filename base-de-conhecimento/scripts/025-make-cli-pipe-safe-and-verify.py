# Make CLI pipe-safe and verify
# 27/08 18:32

p='crates/phxsql-cli/src/main.rs'
s=open(p).read()

s=s.replace('''use std::path::Path;
use std::process::ExitCode;''','''use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

/// Escreve uma linha na saida padrao ignorando erro de escrita.
///
/// O `println!` da biblioteca padrao entra em panico quando a saida fecha --
/// e e exatamente o que acontece em `phxsql listar ... | head`. Uma ferramenta
/// de linha de comando tem de sobreviver a isso em silencio.
macro_rules! diga {
    () => {{
        let _ = writeln!(std::io::stdout());
    }};
    ($($arg:tt)*) => {{
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}''')
s=s.replace("println!(","diga!(")
s=s.replace('        print!("{USO}");','        diga!("{USO}");')
s=s.replace('    print!("{USO}");','    diga!("{USO}");')
open(p,'w').write(s)
