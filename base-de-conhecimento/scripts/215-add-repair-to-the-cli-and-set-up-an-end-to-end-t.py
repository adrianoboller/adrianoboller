# Add repair to the CLI and set up an end-to-end test
# 27/08 21:47

p='crates/phxsql-cli/src/main.rs'
s=open(p).read()
s=s.replace('''        "conferir-backup" => exigir(&args, 2).and_then(|_| conferir_backup(&args[1])),''',
'''        "conferir-backup" => exigir(&args, 2).and_then(|_| conferir_backup(&args[1])),
        "reparar" => exigir(&args, 3).and_then(|_| reparar(Path::new(&args[1]), &args[2])),''')
s=s.replace('''//! phxsql conferir-backup <destino>             le a copia de volta e confere''',
'''//! phxsql conferir-backup <destino>             le a copia de volta e confere
//! phxsql reparar   <dir> <tabela>              confere .reg contra .bkp e conserta''')
s=s.replace('''  phxsql conferir-backup <destino>
";''','''  phxsql conferir-backup <destino>
  phxsql reparar   <dir> <tabela>
";''')
s += '''

/// Confere o `.reg` contra o `.bkp` e conserta o que der.
///
/// Repara nos dois sentidos: registro ruim no principal volta do espelho, e
/// registro ruim no espelho e reescrito a partir do principal. O que estiver
/// ruim dos dois lados e CONTADO como perdido, nunca inventado.
fn reparar(dir: &Path, tabela: &str) -> Result<()> {
    let mut t = Table::abrir_espelhada(dir, tabela)?;
    let (conferidos, reparados, perdidos) = t.reparar()?;
    t.sincronizar()?;
    diga!("{conferidos} slots conferidos");
    diga!("{reparados} reparados");
    if perdidos == 0 {
        diga!("nenhum perdido -- a tabela esta integra.");
        return Ok(());
    }
    diga!("{perdidos} PERDIDOS: ruins nos dois lados.");
    diga!();
    diga!("Restaure do backup. O espelho e segunda chance, nao e backup:");
    diga!("ele mora no mesmo disco.");
    Err(phxsql_core::PhxError::Corrompido(format!(
        "{perdidos} registro(s) sem copia boa"
    )))
}
'''
open(p,'w').write(s)
