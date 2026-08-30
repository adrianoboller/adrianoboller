# Clean CLI warnings and add database commands
# 27/08 18:31

p='crates/phxsql-cli/src/main.rs'
s=open(p).read()
s=s.replace("use phxsql_core::{Result, EXT_BIN, EXT_MEMO, EXT_NDX, EXT_REG};\nuse phxsql_store::log::EXT_LOG;\nuse phxsql_store::table::Table;","use phxsql_core::Result;\nuse phxsql_store::catalogo::Instancia;\nuse phxsql_store::table::Table;")
# remove a funcao morta
import re
s=re.sub(r"fn tamanho\(dir: &Path, nome: &str, ext: &str\) -> \(PathBuf, u64\) \{.*?\n\}\n\n", "", s, flags=re.S)
s=s.replace("use std::path::{Path, PathBuf};","use std::path::Path;")

s=s.replace('''  phxsql log       <dir> <tabela> [--rowid <n>] [--max <n>]
";''','''  phxsql log       <dir> <tabela> [--rowid <n>] [--max <n>]
  phxsql bancos    <base>
  phxsql tabelas   <base> <database>
";''')
s=s.replace('''//! phxsql log       <dir> <tabela> [opcoes]     mostra o diario de alteracoes
//!     --rowid <n>       so os eventos desse registro
//!     --max <n>         limita a quantidade (padrao 20; 0 = todos)
//! ```''','''//! phxsql log       <dir> <tabela> [opcoes]     mostra o diario de alteracoes
//!     --rowid <n>       so os eventos desse registro
//!     --max <n>         limita a quantidade (padrao 20; 0 = todos)
//! phxsql bancos    <base>                      lista os databases da raiz
//! phxsql tabelas   <base> <database>           lista as tabelas, com schema
//! ```''')
s=s.replace('''        "log" => exigir(&args, 3).and_then(|_| mostrar_log(&args)),''','''        "log" => exigir(&args, 3).and_then(|_| mostrar_log(&args)),
        "bancos" => exigir(&args, 2).and_then(|_| bancos(Path::new(&args[1]))),
        "tabelas" => exigir(&args, 3).and_then(|_| tabelas(Path::new(&args[1]), &args[2])),''')

s=s.replace('''fn reindex(dir: &Path, nome: &str) -> Result<()> {''','''fn bancos(base: &Path) -> Result<()> {
    let inst = Instancia::nova(base)?;
    let lista = inst.databases()?;
    if lista.is_empty() {
        println!("nenhum database em {}", base.display());
        return Ok(());
    }
    println!("{} databases em {}", lista.len(), base.display());
    for nome in &lista {
        let db = inst.abrir_database(nome)?;
        let raiz = db.tabelas(None)?.len();
        let schemas = db.schemas()?;
        println!(
            "  {nome:<20} {raiz} tabelas na raiz, {} schemas",
            schemas.len()
        );
    }
    Ok(())
}

fn tabelas(base: &Path, database: &str) -> Result<()> {
    let inst = Instancia::nova(base)?;
    let db = inst.abrir_database(database)?;

    println!("database {database} em {}", db.caminho().display());
    let raiz = db.tabelas(None)?;
    println!("\\ntabelas da raiz ({}):", raiz.len());
    for t in &raiz {
        println!("  {t}");
    }
    for s in db.schemas()? {
        let ts = db.tabelas(Some(&s))?;
        println!("\\nschema {s} ({} tabelas):", ts.len());
        for t in &ts {
            println!("  {s}.{t}");
        }
    }
    Ok(())
}

fn reindex(dir: &Path, nome: &str) -> Result<()> {''')
open(p,'w').write(s)
