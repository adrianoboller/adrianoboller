//! `phxsql` -- ferramenta de linha de comando do PhxSql.
//!
//! ```text
//! phxsql demo      <dir>                       cria um cadastroClientes de exemplo
//! phxsql info      <dir> <tabela>              esquema, contagens e tamanho dos 4 arquivos
//! phxsql verificar <dir> <tabela>              confere CRC de tudo e a coerencia dos indices
//! phxsql listar    <dir> <tabela> [opcoes]     mostra as linhas
//!     --indice <nome>   percorre na ordem do indice, em vez da ordem de digitacao
//!     --max <n>         limita a quantidade de linhas (padrao 20; 0 = todas)
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use phxsql_core::datahora::{data_iso, hora_iso};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_core::{Result, EXT_BIN, EXT_MEMO, EXT_NDX, EXT_REG};
use phxsql_store::table::Table;

const USO: &str = "\
phxsql -- motor de dados PhxSql (.reg + .ndx + .bin + .memo)

USO:
  phxsql demo      <dir>
  phxsql info      <dir> <tabela>
  phxsql verificar <dir> <tabela>
  phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>]
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print!("{USO}");
        return ExitCode::SUCCESS;
    }

    let resultado = match args[0].as_str() {
        "demo" => exigir(&args, 2).and_then(|_| demo(Path::new(&args[1]))),
        "info" => exigir(&args, 3).and_then(|_| info(Path::new(&args[1]), &args[2])),
        "verificar" => exigir(&args, 3).and_then(|_| verificar(Path::new(&args[1]), &args[2])),
        "listar" => exigir(&args, 3).and_then(|_| listar(&args)),
        outro => {
            eprintln!("comando desconhecido: {outro}\n");
            print!("{USO}");
            return ExitCode::FAILURE;
        }
    };

    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::FAILURE
        }
    }
}

fn exigir(args: &[String], n: usize) -> Result<()> {
    if args.len() < n {
        return Err(phxsql_core::PhxError::Esquema(format!(
            "faltam argumentos para `{}`",
            args[0]
        )));
    }
    Ok(())
}

fn esquema_demo() -> Result<Schema> {
    Schema::new(
        "cadastroClientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
            Column::new(
                "limite",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
            Column::new("cadastro", ColumnType::Date),
            Column::new("foto", ColumnType::Bin),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porNome", vec![IndexColumn::asc(1).sem_caixa()]),
            IndexDef::new(
                "porCidadeLimite",
                vec![IndexColumn::asc(2), IndexColumn::desc(3)],
            ),
        ],
    )
}

fn demo(dir: &Path) -> Result<()> {
    let mut t = Table::criar(dir, esquema_demo()?)?;
    // O limite e Decimal(15,2): os valores ja vao escalados por 100,
    // entao 150_000 significa R$ 1.500,00.
    let amostra = [
        (1i64, "Adriano Boller", "Blumenau", 150_000i128),
        (2, "Marcia Alves", "Joinville", 32_000),
        (3, "Zuleica Prado", "Blumenau", 89_000),
        (4, "Beatriz Nunes", "Itajai", 4_500),
        (5, "Carlos Menezes", "Blumenau", 270_000),
    ];
    for (id, nome, cidade, limite) in amostra {
        t.inserir(&[
            Value::Int(id),
            Value::Str(nome.into()),
            Value::Str(cidade.into()),
            Value::Decimal(limite),
            Value::Date(20_000),
            Value::Null,
            Value::Memo(format!("Ficha cadastral de {nome}.")),
        ])?;
    }
    t.sincronizar()?;
    println!(
        "criada a tabela cadastroClientes em {} com {} registros",
        dir.display(),
        t.registros()
    );
    for ext in [EXT_REG, EXT_NDX, EXT_BIN, EXT_MEMO] {
        println!("  cadastroClientes.{ext}");
    }
    Ok(())
}

fn tamanho(dir: &Path, nome: &str, ext: &str) -> (PathBuf, u64) {
    let c = dir.join(format!("{nome}.{ext}"));
    let tam = std::fs::metadata(&c).map(|m| m.len()).unwrap_or(0);
    (c, tam)
}

fn info(dir: &Path, nome: &str) -> Result<()> {
    let t = Table::abrir(dir, nome)?;
    let esq = t.esquema();

    println!("tabela : {}", esq.nome());
    println!("local  : {}", dir.display());
    println!(
        "linhas : {} ativas / {} slots ja usados",
        t.registros(),
        t.slots()
    );

    println!("\narquivos:");
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

    println!("\ncolunas:");
    for (i, c) in esq.colunas().iter().enumerate() {
        println!(
            "  {i:>2}  {:<14} {:<24} {}",
            c.nome,
            format!("{:?}", c.ty),
            if c.nullable { "" } else { "NOT NULL" }
        );
    }

    println!("\nindices:");
    for (d, def) in t.descritores_indices().iter().zip(esq.indices()) {
        let cols: Vec<String> = def
            .colunas
            .iter()
            .map(|ic| {
                let mut s = esq.colunas()[ic.coluna].nome.clone();
                if ic.desc {
                    s.push_str(" DESC");
                }
                if ic.nocase {
                    s.push_str(" NOCASE");
                }
                s
            })
            .collect();
        println!(
            "  {:<18} {:<10} chave {:>4} bytes  {:>8} chaves  ({})",
            d.nome,
            if d.unico { "UNICO" } else { "" },
            d.key_len,
            d.qtd_chaves,
            cols.join(", ")
        );
    }
    println!("  {} paginas no .ndx", t.paginas_indice());

    let (bin, memo) = t.estatisticas_externas();
    println!("\nexternos:");
    println!(
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
}

fn verificar(dir: &Path, nome: &str) -> Result<()> {
    let mut t = Table::abrir(dir, nome)?;
    let r = t.verificar()?;
    println!("tabela {} INTEGRA", r.tabela);
    println!("  {} registros em {} slots", r.registros, r.slots);
    for (idx, qtd) in &r.indices {
        println!("  indice {idx}: {qtd} chaves, ordenacao conferida");
    }
    println!(
        "  .bin  {} blocos vivos, {} mortos",
        r.blocos_bin.0, r.blocos_bin.1
    );
    println!(
        "  .memo {} blocos vivos, {} mortos",
        r.blocos_memo.0, r.blocos_memo.1
    );
    Ok(())
}

fn listar(args: &[String]) -> Result<()> {
    let dir = Path::new(&args[1]);
    let nome = &args[2];
    let mut indice: Option<String> = None;
    let mut max = 20usize;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--indice" if i + 1 < args.len() => {
                indice = Some(args[i + 1].clone());
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
    let colunas: Vec<String> = t
        .esquema()
        .colunas()
        .iter()
        .map(|c| c.nome.clone())
        .collect();
    let tipos: Vec<ColumnType> = t.esquema().colunas().iter().map(|c| c.ty).collect();

    let rowids: Vec<u64> = match &indice {
        Some(nome_idx) => t.varrer_indice(nome_idx)?,
        None => t.varrer()?.into_iter().map(|(r, _)| r).collect(),
    };

    println!(
        "{} linhas, na ordem {}",
        rowids.len(),
        match &indice {
            Some(n) => format!("do indice {n}"),
            None => "de digitacao (.reg)".to_string(),
        }
    );
    println!("{:>8}  {}", "rowid", colunas.join(" | "));

    for rowid in rowids.iter().take(if max == 0 { usize::MAX } else { max }) {
        if let Some(linha) = t.ler(*rowid)? {
            let campos: Vec<String> = linha
                .iter()
                .zip(tipos.iter())
                .map(|(v, ty)| mostrar(v, ty))
                .collect();
            println!("{rowid:>8}  {}", campos.join(" | "));
        }
    }
    if max != 0 && rowids.len() > max {
        println!(
            "... (+{} linhas; use --max 0 para tudo)",
            rowids.len() - max
        );
    }
    Ok(())
}

/// Formata um valor para exibicao, usando o tipo da coluna para dar sentido
/// aos inteiros de data, hora e decimal.
fn mostrar(v: &Value, ty: &ColumnType) -> String {
    match (v, ty) {
        (Value::Null, _) => "NULL".to_string(),
        (Value::Date(d), _) => data_iso(*d),
        (Value::Time(c), _) => hora_iso(*c),
        (Value::Decimal(n), ColumnType::Decimal { escala, .. }) => decimal_texto(*n, *escala),
        (Value::Str(s), _) => s.clone(),
        (Value::Memo(s), _) => {
            if s.chars().count() > 40 {
                let corte: String = s.chars().take(37).collect();
                format!("{corte}...")
            } else {
                s.clone()
            }
        }
        (Value::Bin(b), _) => format!("<{} bytes>", b.len()),
        (Value::Bool(b), _) => b.to_string(),
        (Value::Int(n), _) => n.to_string(),
        (Value::UInt(n), _) => n.to_string(),
        (Value::Real(n), _) => format!("{n}"),
        (Value::Decimal(n), _) => n.to_string(),
        (Value::DateTime(n), _) => n.to_string(),
    }
}

fn decimal_texto(valor: i128, escala: u8) -> String {
    if escala == 0 {
        return valor.to_string();
    }
    let divisor = 10i128.pow(escala as u32);
    let sinal = if valor < 0 { "-" } else { "" };
    let a = valor.unsigned_abs();
    let inteiro = a / divisor.unsigned_abs();
    let fracao = a % divisor.unsigned_abs();
    format!(
        "{sinal}{inteiro}.{fracao:0>width$}",
        width = escala as usize
    )
}
