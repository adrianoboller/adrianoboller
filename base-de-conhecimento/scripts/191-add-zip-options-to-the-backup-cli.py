# Add zip options to the backup CLI
# 27/08 21:20

# CLI: backup em zip tambem pela linha de comando
p='crates/phxsql-cli/src/main.rs'
s=open(p).read()
s=s.replace('''        "backup" => exigir(&args, 3).and_then(|_| backup(&args[1], &args[2])),''',
'''        "backup" => exigir(&args, 3).and_then(|_| backup(&args, &args[1], &args[2])),''')
s=s.replace('''fn backup(base: &str, destino: &str) -> phxsql_core::error::Result<()> {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let r = phxsql_store::backup::executar(
        Path::new(base),
        Path::new(destino),
        &phxsql_core::datahora::instante_iso(agora),
    )?;
    diga!("copiados {} arquivos, {} bytes", r.arquivos.len(), r.bytes);
    diga!("manifesto em {}/{}", destino, phxsql_store::backup::MANIFESTO);
    diga!();
    diga!("Confira com:  phxsql conferir-backup {destino}");
    Ok(())
}''','''fn backup(args: &[String], base: &str, destino: &str) -> phxsql_core::error::Result<()> {
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
        let (arquivo, r) =
            phxsql_store::backup::executar_zip(Path::new(base), Path::new(destino), &banco, &quem, agora)?;
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
        diga!("O manifesto vai dentro do zip. Confira com unzip -t, ou extraia e rode");
        diga!("phxsql conferir-backup na pasta extraida.");
        return Ok(());
    }

    let r = phxsql_store::backup::executar(
        Path::new(base),
        Path::new(destino),
        &phxsql_core::datahora::instante_iso(agora),
    )?;
    diga!("copiados {} arquivos, {} bytes", r.arquivos.len(), r.bytes);
    diga!("manifesto em {}/{}", destino, phxsql_store::backup::MANIFESTO);
    diga!();
    diga!("Confira com:  phxsql conferir-backup {destino}");
    Ok(())
}''')
s=s.replace('''//! phxsql backup    <base> <destino>            copia com manifesto SHA-256''',
'''//! phxsql backup    <base> <destino> [opcoes]   copia com manifesto SHA-256
//!     --zip                 um arquivo Banco_Admin_Data_HoraMin.zip
//!     --database <nome>     so esse banco
//!     --admin <nome>        o nome que entra no arquivo''')
s=s.replace('''phxsql backup    <base> <destino>        copia com manifesto SHA-256''',
'''phxsql backup    <base> <destino>        copia com manifesto SHA-256
    --zip                                um arquivo Banco_Admin_Data_HoraMin.zip
    --database <nome>                    so esse banco
    --admin <nome>                       o nome que entra no arquivo''')
open(p,'w').write(s)
