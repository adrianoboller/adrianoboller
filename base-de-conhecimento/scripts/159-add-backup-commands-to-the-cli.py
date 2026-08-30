# Add backup commands to the CLI
# 27/08 20:55

p='crates/phxsql-cli/src/main.rs'
s=open(p).read()
s=s.replace('''        "tabelas" => exigir(&args, 3).and_then(|_| tabelas(Path::new(&args[1]), &args[2])),''',
'''        "tabelas" => exigir(&args, 3).and_then(|_| tabelas(Path::new(&args[1]), &args[2])),
        "backup" => exigir(&args, 3).and_then(|_| backup(&args[1], &args[2])),
        "conferir-backup" => exigir(&args, 2).and_then(|_| conferir_backup(&args[1])),''')
s=s.replace('''//! phxsql tabelas   <base> <database>           lista as tabelas, com schema''',
'''//! phxsql tabelas   <base> <database>           lista as tabelas, com schema
//! phxsql backup    <base> <destino>            copia com manifesto SHA-256
//! phxsql conferir-backup <destino>             le a copia de volta e confere''')
s=s.replace('''phxsql tabelas   <base> <database>       lista as tabelas, com schema''',
'''phxsql tabelas   <base> <database>       lista as tabelas, com schema
phxsql backup    <base> <destino>        copia com manifesto SHA-256
phxsql conferir-backup <destino>         le a copia de volta e confere''')
s += '''

/// Copia os dados e escreve o manifesto.
///
/// Feito de fora do servidor, com o servidor PARADO -- e o unico jeito de a
/// linha de comando garantir que ninguem escreve durante a copia. Com o
/// servidor no ar, use a operacao "backup" pelo protocolo: la a trava de
/// dados e segurada de verdade.
fn backup(base: &str, destino: &str) -> phxsql_core::error::Result<()> {
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
}

fn conferir_backup(destino: &str) -> phxsql_core::error::Result<()> {
    let r = phxsql_store::backup::conferir(Path::new(destino))?;
    diga!("{} arquivos, {} bytes", r.arquivos.len(), r.bytes);
    if r.ok() {
        diga!("INTEGRO -- cada arquivo bate com o SHA-256 do manifesto.");
        return Ok(());
    }
    diga!();
    diga!("{} DIVERGENCIA(S):", r.divergencias.len());
    for d in &r.divergencias {
        diga!("  {d}");
    }
    Err(phxsql_core::error::PhxError::Corrompido(
        "o backup nao confere com o manifesto".into(),
    ))
}
'''
open(p,'w').write(s)
