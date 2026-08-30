# Add key generation to the CLI
# 27/08 20:44

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
s=s.replace('''    if args.iter().any(|a| a == "--senha") {
        return gerar_senha(&args);
    }''','''    if args.iter().any(|a| a == "--senha") {
        return gerar_senha(&args);
    }

    if args.iter().any(|a| a == "--gerar-chave") {
        return gerar_chave();
    }''')
s=s.replace('''fn gerar_senha(args: &[String]) -> ExitCode {''','''/// Um par de chaves Ed25519.
///
/// A privada sai UMA vez, aqui, e o servidor nunca a ve. Se ela se perder,
/// nao ha como recuperar -- gera-se outra e troca-se a publica no
/// `config.json`. E o preco de o servidor nao guardar nada que assine.
fn gerar_chave() -> ExitCode {
    let privada = phxsql_core::ed25519::gerar_privada();
    let publica = phxsql_core::ed25519::chave_publica(&privada);
    println!("# Guarde a chave PRIVADA fora do servidor. Ela nao aparece de novo.");
    println!("chave_privada = {}", phxsql_core::hash::para_hex(&privada));
    println!();
    println!("# Esta linha vai no config.json, dentro do usuario:");
    println!(
        "\\"chave_publica\\": \\"{}\\"",
        phxsql_core::hash::para_hex(&publica)
    );
    ExitCode::SUCCESS
}

fn gerar_senha(args: &[String]) -> ExitCode {''')
open(p,'w').write(s)
