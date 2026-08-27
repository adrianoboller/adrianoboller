//! `phxsql` -- ferramenta de linha de comando do PhxSql.
//!
//! ```text
//! phxsql demo      <dir> [--paginado]          cria um cadastroClientes de exemplo
//! phxsql info      <dir> <tabela>              esquema, contagens e volumes
//! phxsql verificar <dir> <tabela>              confere CRC de tudo e a coerencia dos indices
//! phxsql reindex   <dir> <tabela>              recria o .ndx do zero a partir do .reg
//! phxsql listar    <dir> <tabela> [opcoes]     mostra as linhas
//!     --indice <nome>   percorre na ordem do indice, em vez da ordem de digitacao
//!     --max <n>         limita a quantidade de linhas (padrao 20; 0 = todas)
//! phxsql log       <dir> <tabela> [opcoes]     mostra o diario de alteracoes
//!     --rowid <n>       so os eventos desse registro
//!     --max <n>         limita a quantidade (padrao 20; 0 = todos)
//! phxsql bancos    <base>                      lista os databases da raiz
//! phxsql tabelas   <base> <database>           lista as tabelas, com schema
//! phxsql backup    <base> <destino> [opcoes]   copia com manifesto SHA-256
//!     --zip                 um arquivo Banco_Admin_Data_HoraMin.zip
//!     --database <nome>     so esse banco
//!     --admin <nome>        o nome que entra no arquivo
//! phxsql conferir-backup <destino>             le a copia de volta e confere
//! ```

use std::io::Write;
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
}

use phxsql_core::datahora::{data_iso, hora_iso};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_core::Result;
use phxsql_store::catalogo::Instancia;
use phxsql_store::table::Table;

const USO: &str = "\
phxsql -- motor de dados PhxSql (.reg + .ndx + .bin + .memo + .log)

USO:
  phxsql demo      <dir> [--paginado]
  phxsql info      <dir> <tabela>
  phxsql verificar <dir> <tabela>
  phxsql reindex   <dir> <tabela>
  phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>]
  phxsql log       <dir> <tabela> [--rowid <n>] [--max <n>]
  phxsql bancos    <base>
  phxsql tabelas   <base> <database>
  phxsql backup    <base> <destino> [--zip] [--database <n>] [--admin <n>]
  phxsql conferir-backup <destino>
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        diga!("{USO}");
        return ExitCode::SUCCESS;
    }

    let resultado = match args[0].as_str() {
        "demo" => exigir(&args, 2)
            .and_then(|_| demo(Path::new(&args[1]), args.iter().any(|a| a == "--paginado"))),
        "info" => exigir(&args, 3).and_then(|_| info(Path::new(&args[1]), &args[2])),
        "verificar" => exigir(&args, 3).and_then(|_| verificar(Path::new(&args[1]), &args[2])),
        "reindex" => exigir(&args, 3).and_then(|_| reindex(Path::new(&args[1]), &args[2])),
        "listar" => exigir(&args, 3).and_then(|_| listar(&args)),
        "log" => exigir(&args, 3).and_then(|_| mostrar_log(&args)),
        "bancos" => exigir(&args, 2).and_then(|_| bancos(Path::new(&args[1]))),
        "tabelas" => exigir(&args, 3).and_then(|_| tabelas(Path::new(&args[1]), &args[2])),
        "backup" => exigir(&args, 3).and_then(|_| backup(&args, &args[1], &args[2])),
        "conferir-backup" => exigir(&args, 2).and_then(|_| conferir_backup(&args[1])),
        outro => {
            eprintln!("comando desconhecido: {outro}\n");
            diga!("{USO}");
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

fn esquema_demo(paginado: bool) -> Result<Schema> {
    let esquema = Schema::new(
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
}

fn demo(dir: &Path, paginado: bool) -> Result<()> {
    let mut t = Table::criar(dir, esquema_demo(paginado)?)?;
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
    diga!(
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
        diga!("  {nome_arq:<34} {tam:>12} bytes");
    }
    diga!("  {:<34} {total:>12} bytes", "TOTAL");
    total
}

fn bancos(base: &Path) -> Result<()> {
    let inst = Instancia::nova(base)?;
    let lista = inst.databases()?;
    if lista.is_empty() {
        diga!("nenhum database em {}", base.display());
        return Ok(());
    }
    diga!("{} databases em {}", lista.len(), base.display());
    for nome in &lista {
        let db = inst.abrir_database(nome)?;
        let raiz = db.tabelas(None)?.len();
        let schemas = db.schemas()?;
        diga!(
            "  {nome:<20} {raiz} tabelas na raiz, {} schemas",
            schemas.len()
        );
    }
    Ok(())
}

fn tabelas(base: &Path, database: &str) -> Result<()> {
    let inst = Instancia::nova(base)?;
    let db = inst.abrir_database(database)?;

    diga!("database {database} em {}", db.caminho().display());
    let raiz = db.tabelas(None)?;
    diga!("\ntabelas da raiz ({}):", raiz.len());
    for t in &raiz {
        diga!("  {t}");
    }
    for s in db.schemas()? {
        let ts = db.tabelas(Some(&s))?;
        diga!("\nschema {s} ({} tabelas):", ts.len());
        for t in &ts {
            diga!("  {s}.{t}");
        }
    }
    Ok(())
}

fn reindex(dir: &Path, nome: &str) -> Result<()> {
    let mut t = Table::abrir(dir, nome)?;
    diga!("recriando o .ndx de {nome} a partir do .reg...");
    let indices = t.reindexar()?;
    t.sincronizar()?;
    for (idx, qtd) in &indices {
        diga!("  indice {idx}: {qtd} chaves reconstruidas");
    }
    diga!("pronto.");
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

    diga!("{} eventos no diario de {nome}", total);
    if let Some(r) = rowid {
        diga!("filtrando o registro {r}: {} eventos", eventos.len());
    }
    diga!(
        "{:<23}  {:<10}  {:>8}  {:>7}  {:>8}",
        "quando",
        "operacao",
        "rowid",
        "versao",
        "usuario"
    );
    let mostrados: Vec<_> = if max == 0 {
        eventos.iter().collect()
    } else {
        eventos.iter().rev().take(max as usize).rev().collect()
    };
    for e in mostrados {
        diga!(
            "{:<23}  {:<10}  {:>8}  {:>7}  {:>8}",
            e.instante_iso(),
            e.operacao.nome(),
            e.rowid,
            e.versao,
            e.usuario
        );
    }
    if max != 0 && eventos.len() as u64 > max {
        diga!(
            "... (mostrando os {max} mais recentes de {})",
            eventos.len()
        );
    }
    Ok(())
}

fn info(dir: &Path, nome: &str) -> Result<()> {
    let mut t = Table::abrir(dir, nome)?;
    let esq = t.esquema().clone();

    diga!("tabela : {}", esq.nome());
    diga!("local  : {}", dir.display());
    diga!(
        "linhas : {} ativas / {} slots ja usados",
        t.registros(),
        t.slots()
    );

    let pag = esq.paginacao();
    if pag.ligada() {
        diga!(
            "paginada: {} registros por arquivo x {} arquivos = capacidade {}",
            pag.registros_por_arquivo,
            pag.max_arquivos,
            pag.capacidade()
        );
        let (r, b, m, l) = t.volumes_por_arquivo();
        diga!(
            "volumes : .reg {}  .bin {}  .memo {}  .log {}",
            r.len(),
            b.len(),
            m.len(),
            l.len()
        );
    } else {
        diga!("paginada: nao (arquivo unico por extensao)");
    }
    diga!("eventos: {} no diario", t.eventos()?);

    diga!("\narquivos:");
    listar_arquivos(dir, nome);

    diga!("\ncolunas:");
    for (i, c) in esq.colunas().iter().enumerate() {
        diga!(
            "  {i:>2}  {:<14} {:<24} {}",
            c.nome,
            format!("{:?}", c.ty),
            if c.nullable { "" } else { "NOT NULL" }
        );
    }

    diga!("\nindices:");
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
        diga!(
            "  {:<18} {:<10} chave {:>4} bytes  {:>8} chaves  ({})",
            d.nome,
            if d.unico { "UNICO" } else { "" },
            d.key_len,
            d.qtd_chaves,
            cols.join(", ")
        );
    }
    diga!("  {} paginas no .ndx", t.paginas_indice());

    if !esq.chaves_estrangeiras().is_empty() {
        diga!("\nchaves estrangeiras:");
        for fk in esq.chaves_estrangeiras() {
            let locais: Vec<&str> = fk
                .colunas
                .iter()
                .map(|c| esq.colunas()[*c].nome.as_str())
                .collect();
            diga!(
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

    let (bin, memo) = t.estatisticas_externas()?;
    diga!("\nexternos:");
    diga!(
        "  .bin   {} volumes, {} blocos, {} bytes vivos, {} mortos ({:.1}% de desperdicio)",
        bin.volumes,
        bin.blocos,
        bin.bytes_vivos,
        bin.bytes_mortos,
        bin.fragmentacao() * 100.0
    );
    diga!(
        "  .memo  {} volumes, {} blocos, {} bytes vivos, {} mortos ({:.1}% de desperdicio)",
        memo.volumes,
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
    diga!("tabela {} INTEGRA", r.tabela);
    diga!("  {} registros em {} slots", r.registros, r.slots);
    for (idx, qtd) in &r.indices {
        diga!("  indice {idx}: {qtd} chaves, ordenacao conferida");
    }
    diga!(
        "  .bin  {} blocos vivos, {} mortos",
        r.blocos_bin.0,
        r.blocos_bin.1
    );
    diga!(
        "  .memo {} blocos vivos, {} mortos",
        r.blocos_memo.0,
        r.blocos_memo.1
    );
    diga!("  .log  {} eventos conferidos", r.eventos);
    let (vr, vb, vm, vl) = r.volumes;
    diga!("  volumes: .reg {vr}  .bin {vb}  .memo {vm}  .log {vl}");
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

    diga!(
        "{} linhas, na ordem {}",
        rowids.len(),
        match &indice {
            Some(n) => format!("do indice {n}"),
            None => "de digitacao (.reg)".to_string(),
        }
    );
    diga!("{:>8}  {}", "rowid", colunas.join(" | "));

    for rowid in rowids.iter().take(if max == 0 { usize::MAX } else { max }) {
        if let Some(linha) = t.ler(*rowid)? {
            let campos: Vec<String> = linha
                .iter()
                .zip(tipos.iter())
                .map(|(v, ty)| mostrar(v, ty))
                .collect();
            diga!("{rowid:>8}  {}", campos.join(" | "));
        }
    }
    if max != 0 && rowids.len() > max {
        diga!(
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

/// Copia os dados e escreve o manifesto.
///
/// Feito de fora do servidor, com o servidor PARADO -- e o unico jeito de a
/// linha de comando garantir que ninguem escreve durante a copia. Com o
/// servidor no ar, use a operacao "backup" pelo protocolo: la a trava de
/// dados e segurada de verdade.
fn backup(args: &[String], base: &str, destino: &str) -> phxsql_core::error::Result<()> {
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
