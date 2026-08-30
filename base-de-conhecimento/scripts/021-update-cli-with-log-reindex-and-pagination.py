# Update CLI with log, reindex and pagination
# 27/08 18:28

p='crates/phxsql-cli/src/main.rs'
s=open(p).read()

s=s.replace('''//! phxsql demo      <dir>                       cria um cadastroClientes de exemplo
//! phxsql info      <dir> <tabela>              esquema, contagens e tamanho dos 4 arquivos
//! phxsql verificar <dir> <tabela>              confere CRC de tudo e a coerencia dos indices
//! phxsql listar    <dir> <tabela> [opcoes]     mostra as linhas
//!     --indice <nome>   percorre na ordem do indice, em vez da ordem de digitacao
//!     --max <n>         limita a quantidade de linhas (padrao 20; 0 = todas)
//! ```''','''//! phxsql demo      <dir> [--paginado]          cria um cadastroClientes de exemplo
//! phxsql info      <dir> <tabela>              esquema, contagens e volumes
//! phxsql verificar <dir> <tabela>              confere CRC de tudo e a coerencia dos indices
//! phxsql reindex   <dir> <tabela>              recria o .ndx do zero a partir do .reg
//! phxsql listar    <dir> <tabela> [opcoes]     mostra as linhas
//!     --indice <nome>   percorre na ordem do indice, em vez da ordem de digitacao
//!     --max <n>         limita a quantidade de linhas (padrao 20; 0 = todas)
//! phxsql log       <dir> <tabela> [opcoes]     mostra o diario de alteracoes
//!     --rowid <n>       so os eventos desse registro
//!     --max <n>         limita a quantidade (padrao 20; 0 = todos)
//! ```''')

s=s.replace('''use phxsql_core::datahora::{data_iso, hora_iso};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};''','''use phxsql_core::datahora::{data_iso, hora_iso};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};''')
s=s.replace('''use phxsql_core::{Result, EXT_BIN, EXT_MEMO, EXT_NDX, EXT_REG};
use phxsql_store::table::Table;''','''use phxsql_core::{Result, EXT_BIN, EXT_MEMO, EXT_NDX, EXT_REG};
use phxsql_store::log::EXT_LOG;
use phxsql_store::table::Table;''')

s=s.replace('''const USO: &str = "\\
phxsql -- motor de dados PhxSql (.reg + .ndx + .bin + .memo)

USO:
  phxsql demo      <dir>
  phxsql info      <dir> <tabela>
  phxsql verificar <dir> <tabela>
  phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>]
";''','''const USO: &str = "\\
phxsql -- motor de dados PhxSql (.reg + .ndx + .bin + .memo + .log)

USO:
  phxsql demo      <dir> [--paginado]
  phxsql info      <dir> <tabela>
  phxsql verificar <dir> <tabela>
  phxsql reindex   <dir> <tabela>
  phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>]
  phxsql log       <dir> <tabela> [--rowid <n>] [--max <n>]
";''')

s=s.replace('''        "demo" => exigir(&args, 2).and_then(|_| demo(Path::new(&args[1]))),
        "info" => exigir(&args, 3).and_then(|_| info(Path::new(&args[1]), &args[2])),
        "verificar" => exigir(&args, 3).and_then(|_| verificar(Path::new(&args[1]), &args[2])),
        "listar" => exigir(&args, 3).and_then(|_| listar(&args)),''','''        "demo" => exigir(&args, 2)
            .and_then(|_| demo(Path::new(&args[1]), args.iter().any(|a| a == "--paginado"))),
        "info" => exigir(&args, 3).and_then(|_| info(Path::new(&args[1]), &args[2])),
        "verificar" => exigir(&args, 3).and_then(|_| verificar(Path::new(&args[1]), &args[2])),
        "reindex" => exigir(&args, 3).and_then(|_| reindex(Path::new(&args[1]), &args[2])),
        "listar" => exigir(&args, 3).and_then(|_| listar(&args)),
        "log" => exigir(&args, 3).and_then(|_| mostrar_log(&args)),''')

s=s.replace('''fn esquema_demo() -> Result<Schema> {
    Schema::new(''','''fn esquema_demo(paginado: bool) -> Result<Schema> {
    let esquema = Schema::new(''')
s=s.replace('''            IndexDef::new(
                "porCidadeLimite",
                vec![IndexColumn::asc(2), IndexColumn::desc(3)],
            ),
        ],
    )
}''','''            IndexDef::new(
                "porCidadeLimite",
                vec![IndexColumn::asc(2), IndexColumn::desc(3)],
            ),
        ],
    )?
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fkCidade",
        vec![2],
        "cidades",
        vec!["nome".to_string()],
    )
    .ao_excluir(AcaoRi::Restringir)])?;

    Ok(if paginado {
        // Dois registros por arquivo, ate 99 arquivos: so para a demo mostrar
        // varios volumes com poucas linhas.
        esquema.com_paginacao(Paginacao::nova(2, 99)?)
    } else {
        esquema
    })
}''')

s=s.replace('''fn demo(dir: &Path) -> Result<()> {
    let mut t = Table::criar(dir, esquema_demo()?)?;''','''fn demo(dir: &Path, paginado: bool) -> Result<()> {
    let mut t = Table::criar(dir, esquema_demo(paginado)?)?;''')

s=s.replace('''    t.sincronizar()?;
    println!(
        "criada a tabela cadastroClientes em {} com {} registros",
        dir.display(),
        t.registros()
    );
    for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO] {
        println!("  cadastroClientes.{ext}");
    }
    Ok(())
}''','''    t.sincronizar()?;
    println!(
        "criada a tabela cadastroClientes em {} com {} registros",
        dir.display(),
        t.registros()
    );
    listar_arquivos(dir, "cadastroClientes");
    Ok(())
}

/// Lista os arquivos da tabela em disco, com e sem sufixo de volume.
fn listar_arquivos(dir: &Path, nome: &str) -> u64 {
    let mut total = 0u64;
    let mut arquivos: Vec<(String, u64)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s == nome || s.starts_with(&format!("{nome}_")))
                .unwrap_or(false)
        })
        .map(|e| {
            let tam = e.metadata().map(|m| m.len()).unwrap_or(0);
            (e.file_name().to_string_lossy().to_string(), tam)
        })
        .collect();
    arquivos.sort();
    for (nome_arq, tam) in &arquivos {
        total += tam;
        println!("  {nome_arq:<34} {tam:>12} bytes");
    }
    println!("  {:<34} {total:>12} bytes", "TOTAL");
    total
}

fn reindex(dir: &Path, nome: &str) -> Result<()> {
    let mut t = Table::abrir(dir, nome)?;
    println!("recriando o .ndx de {nome} a partir do .reg...");
    let indices = t.reindexar()?;
    t.sincronizar()?;
    for (idx, qtd) in &indices {
        println!("  indice {idx}: {qtd} chaves reconstruidas");
    }
    println!("pronto.");
    Ok(())
}

fn mostrar_log(args: &[String]) -> Result<()> {
    let dir = Path::new(&args[1]);
    let nome = &args[2];
    let mut rowid: Option<u64> = None;
    let mut max = 20u64;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--rowid" if i + 1 < args.len() => {
                rowid = args[i + 1].parse().ok();
                i += 2;
            }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or(20);
                i += 2;
            }
            outro => {
                return Err(phxsql_core::PhxError::Esquema(format!(
                    "opcao desconhecida: {outro}"
                )))
            }
        }
    }

    let mut t = Table::abrir(dir, nome)?;
    let total = t.eventos()?;
    let eventos = match rowid {
        Some(r) => t.historico(r)?,
        None => t.diario(0, 0)?,
    };

    println!("{} eventos no diario de {nome}", total);
    if let Some(r) = rowid {
        println!("filtrando o registro {r}: {} eventos", eventos.len());
    }
    println!(
        "{:<23}  {:<10}  {:>8}  {:>7}  {:>8}",
        "quando", "operacao", "rowid", "versao", "usuario"
    );
    let mostrados: Vec<_> = if max == 0 {
        eventos.iter().collect()
    } else {
        eventos.iter().rev().take(max as usize).rev().collect()
    };
    for e in mostrados {
        println!(
            "{:<23}  {:<10}  {:>8}  {:>7}  {:>8}",
            e.instante_iso(),
            e.operacao.nome(),
            e.rowid,
            e.versao,
            e.usuario
        );
    }
    if max != 0 && eventos.len() as u64 > max {
        println!("... (mostrando os {max} mais recentes de {})", eventos.len());
    }
    Ok(())
}''')

# info: volumes, paginacao, FK
s=s.replace('''fn info(dir: &Path, nome: &str) -> Result<()> {
    let t = Table::abrir(dir, nome)?;
    let esq = t.esquema();''','''fn info(dir: &Path, nome: &str) -> Result<()> {
    let mut t = Table::abrir(dir, nome)?;
    let esq = t.esquema().clone();''')

s=s.replace('''    println!("\\narquivos:");
    let mut total = 0u64;
    for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO] {
        let (c, tam) = tamanho(dir, nome, ext);
        total += tam;
        println!(
            "  {:<28} {:>12} bytes",
            c.file_name().unwrap().to_string_lossy(),
            tam
        );
    }
    println!("  {:<28} {total:>12} bytes", "TOTAL");
''','''    let pag = esq.paginacao();
    if pag.ligada() {
        println!(
            "paginada: {} registros por arquivo x {} arquivos = capacidade {}",
            pag.registros_por_arquivo,
            pag.max_arquivos,
            pag.capacidade()
        );
        let (r, b, m, l) = t.volumes_por_arquivo();
        println!(
            "volumes : .reg {}  .bin {}  .memo {}  .log {}",
            r.len(),
            b.len(),
            m.len(),
            l.len()
        );
    } else {
        println!("paginada: nao (arquivo unico por extensao)");
    }
    println!("eventos: {} no diario", t.eventos()?);

    println!("\\narquivos:");
    listar_arquivos(dir, nome);
''')

s=s.replace('''    println!("  {} paginas no .ndx", t.paginas_indice());

    let (bin, memo) = t.estatisticas_externas();''','''    println!("  {} paginas no .ndx", t.paginas_indice());

    if !esq.chaves_estrangeiras().is_empty() {
        println!("\\nchaves estrangeiras:");
        for fk in esq.chaves_estrangeiras() {
            let locais: Vec<&str> = fk
                .colunas
                .iter()
                .map(|c| esq.colunas()[*c].nome.as_str())
                .collect();
            println!(
                "  {:<18} ({}) -> {}({})  ao excluir {:?}, ao alterar {:?}",
                fk.nome,
                locais.join(", "),
                fk.tabela_ref,
                fk.colunas_ref.join(", "),
                fk.ao_excluir,
                fk.ao_alterar
            );
        }
    }

    let (bin, memo) = t.estatisticas_externas()?;''')

s=s.replace('''    println!(
        "  .bin   {} blocos, {} bytes vivos, {} mortos ({:.1}% de desperdicio)",
        bin.blocos,
        bin.bytes_vivos,
        bin.bytes_mortos,
        bin.fragmentacao() * 100.0
    );
    println!(
        "  .memo  {} blocos, {} bytes vivos, {} mortos ({:.1}% de desperdicio)",
        memo.blocos,
        memo.bytes_vivos,
        memo.bytes_mortos,
        memo.fragmentacao() * 100.0
    );
    Ok(())
}''','''    println!(
        "  .bin   {} volumes, {} blocos, {} bytes vivos, {} mortos ({:.1}% de desperdicio)",
        bin.volumes,
        bin.blocos,
        bin.bytes_vivos,
        bin.bytes_mortos,
        bin.fragmentacao() * 100.0
    );
    println!(
        "  .memo  {} volumes, {} blocos, {} bytes vivos, {} mortos ({:.1}% de desperdicio)",
        memo.volumes,
        memo.blocos,
        memo.bytes_vivos,
        memo.bytes_mortos,
        memo.fragmentacao() * 100.0
    );
    Ok(())
}''')

s=s.replace('''    println!(
        "  .memo {} blocos vivos, {} mortos",
        r.blocos_memo.0, r.blocos_memo.1
    );
    Ok(())
}''','''    println!(
        "  .memo {} blocos vivos, {} mortos",
        r.blocos_memo.0, r.blocos_memo.1
    );
    println!("  .log  {} eventos conferidos", r.eventos);
    let (vr, vb, vm, vl) = r.volumes;
    println!("  volumes: .reg {vr}  .bin {vb}  .memo {vm}  .log {vl}");
    Ok(())
}''')
open(p,'w').write(s)
print("CLI atualizado")
