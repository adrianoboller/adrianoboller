# Fix the CLI backup function and test zip output
# 27/08 21:21

p='crates/phxsql-cli/src/main.rs'
linhas=open(p).read().split('\n')
novo = '''fn backup(args: &[String], base: &str, destino: &str) -> phxsql_core::error::Result<()> {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let opcao = |nome: &str| -> Option<String> {
        let i = args.iter().position(|a| a == nome)?;
        args.get(i + 1).filter(|v| !v.starts_with("--")).cloned()
    };

    if args.iter().any(|a| a == "--zip") {
        let banco = opcao("--database").unwrap_or_default();
        let quem = opcao("--admin").unwrap_or_else(|| "manual".into());
        let (arquivo, r) = phxsql_store::backup::executar_zip(
            Path::new(base),
            Path::new(destino),
            &banco,
            &quem,
            agora,
        )?;
        let pct = if r.bytes > 0 {
            100 - (r.comprimido * 100 / r.bytes).min(100)
        } else {
            0
        };
        diga!("{}", arquivo.display());
        diga!(
            "{} arquivos, {} bytes -> {} bytes ({pct}% menor)",
            r.arquivos.len(),
            r.bytes,
            r.comprimido
        );
        diga!();
        diga!("O manifesto vai dentro do zip. Confira com  unzip -t <arquivo>,");
        diga!("ou extraia e rode  phxsql conferir-backup <pasta extraida>.");
        return Ok(());
    }

    let r = phxsql_store::backup::executar(
        Path::new(base),
        Path::new(destino),
        &phxsql_core::datahora::instante_iso(agora),
    )?;
    diga!("copiados {} arquivos, {} bytes", r.arquivos.len(), r.bytes);
    diga!(
        "manifesto em {}/{}",
        destino,
        phxsql_store::backup::MANIFESTO
    );
    diga!();
    diga!("Confira com:  phxsql conferir-backup {destino}");
    Ok(())
}'''
# substitui as linhas 587..606 (indice 586..605)
assert linhas[586].startswith('fn backup(base:'), linhas[586]
assert linhas[605] == '}', linhas[605]
linhas[586:606] = novo.split('\n')
open(p,'w').write('\n'.join(linhas))
