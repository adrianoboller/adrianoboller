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
//! phxsql conferir-pacote [<dir>]               confere um pacote de download
//! phxsql reparar   <dir> <tabela>              confere .reg contra .bkp e conserta
//! ```

use std::collections::BTreeMap;
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

use phxsql_core::carga;
use phxsql_core::datahora::{data_iso, hora_iso};
use phxsql_core::hash::{para_hex, sha256};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_core::Result;
use phxsql_store::catalogo::Instancia;
use phxsql_store::table::{Salto, Table, Visao};

const USO: &str = "\
phxsql -- motor de dados PhxSql (.reg + .ndx + .bin + .memo + .log)

USO:
  phxsql demo      <dir> [--paginado]
  phxsql info      <dir> <tabela>
  phxsql verificar <dir> <tabela>
  phxsql reindex   <dir> <tabela>
  phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>] [--pular <n>]
  phxsql log       <dir> <tabela> [--rowid <n>] [--max <n>]
  phxsql bancos    <base>
  phxsql tabelas   <base> <database>
  phxsql backup    <base> <destino> [--zip] [--database <n>] [--admin <n>]
  phxsql conferir-backup <destino>
  phxsql conferir-pacote [<dir>]
  phxsql reparar   <dir> <tabela>
  phxsql importar  <dir> <tabela> <arquivo> [--formato csv|txt|json|xml|html]
                                            [--seguir] [--conferir]
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
        // Sem argumento vale o diretorio corrente: quem baixou o zip entra
        // nele e roda `./phxsql conferir-pacote`, sem digitar caminho.
        "conferir-pacote" => conferir_pacote(Path::new(args.get(1).map_or(".", |s| s.as_str()))),
        "reparar" => exigir(&args, 3).and_then(|_| reparar(Path::new(&args[1]), &args[2])),
        "importar" => exigir(&args, 4).and_then(|_| importar(&args)),
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
        esquema.com_paginacao(Paginacao::nova(2, 99)?)?
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
    let mut pular = 0u64;

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
            "--pular" if i + 1 < args.len() => {
                pular = args[i + 1].parse().unwrap_or(0);
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

    // O teto entra na LEITURA, e nao depois dela. Ler a tabela inteira para
    // mostrar vinte linhas custava a tabela inteira -- numa de 200 mil linhas
    // com memo, segundos de espera para uma tela que cabe num terminal.
    let teto = if max == 0 { 0 } else { max as u64 };
    let (rowids, salto) = match &indice {
        // Por indice a ordem e a da CHAVE, e nao a do arquivo: ali nao ha
        // conta que leve a posicao N, e a lista de rowids ja veio inteira.
        Some(nome_idx) => {
            let todos = t.varrer_indice(nome_idx)?;
            let corte: Vec<u64> = todos
                .into_iter()
                .skip(pular as usize)
                .take(if max == 0 { usize::MAX } else { max })
                .collect();
            (corte, None)
        }
        None => {
            let (r, como) = t.pagina_por_posicao(pular, teto, Visao::Ativas)?;
            (r, Some(como))
        }
    };

    let total = t.contar(Visao::Ativas);
    diga!(
        "{} linha(s) de {total}, na ordem {}{}",
        rowids.len(),
        match &indice {
            Some(n) => format!("do indice {n}"),
            None => "de digitacao (.reg)".to_string(),
        },
        match salto {
            // Vale dizer: e a diferenca entre vinte leituras e `pular` delas.
            Some(Salto::Bissecao) => ", achada por bisseccao no rownum".to_string(),
            Some(Salto::Passo) if pular > 0 => format!(", andando {pular} linha(s)"),
            _ => String::new(),
        }
    );
    diga!("{:>8}  {}", "rowid", colunas.join(" | "));

    for rowid in &rowids {
        if let Some(linha) = t.ler(*rowid)? {
            let campos: Vec<String> = linha
                .iter()
                .zip(tipos.iter())
                .map(|(v, ty)| mostrar(v, ty))
                .collect();
            diga!("{rowid:>8}  {}", campos.join(" | "));
        }
    }
    let vistas = pular + rowids.len() as u64;
    if max != 0 && vistas < total {
        diga!(
            "... (+{} linha(s); use --pular {vistas} para a proxima pagina, ou --max 0 para tudo)",
            total - vistas
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
        (Value::Uuid(u), _) => u.to_string(),
        // Trinta e dois bytes viram 64 digitos, e a coluna do terminal nao
        // comporta. Mostra as pontas, que e o que o olho usa para reconhecer
        // um hash.
        (Value::Uuid256(u), _) => {
            let t = u.to_string();
            format!("{}…{}", &t[..10], &t[54..])
        }
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

/// Nome do manifesto que o `empacotar.sh` poe dentro de cada pacote.
const MANIFESTO_PACOTE: &str = "MANIFESTO.sha256";

/// Confere um pacote de download (fontes, Linux ou Windows) contra o
/// `MANIFESTO.sha256` que veio dentro dele.
///
/// # Por que o conferidor e o proprio programa
///
/// O pacote precisa ser conferivel NA MAQUINA DE QUEM BAIXOU, e essa maquina
/// pode nao ter `sha256sum` -- o Windows nao tem. O SHA-256 deste projeto ja
/// existe, ja e conferido contra os vetores do FIPS 180-4 e viaja dentro do
/// proprio pacote: o conferidor mais portatil possivel e o binario que ja
/// esta ali do lado.
///
/// O formato do manifesto e o do `sha256sum` de proposito (`<hex>  <arquivo>`),
/// para quem tiver a ferramenta poder conferir por um segundo caminho, que nao
/// depende deste codigo.
///
/// # Arquivo A MAIS tambem e divergencia
///
/// Conferencia de hash so olha o que o manifesto LISTA. Quem acrescenta um
/// arquivo ao pacote nao mexe em nenhuma linha do manifesto e passaria batido
/// -- que e exatamente o jeito de entregar um binario a mais junto do pacote
/// legitimo. E a mesma regra que o `backup.json` ja seguia.
fn conferir_pacote(dir: &Path) -> Result<()> {
    let texto = std::fs::read_to_string(dir.join(MANIFESTO_PACOTE)).map_err(|e| {
        phxsql_core::PhxError::NaoEncontrado(format!(
            "{} nao tem {MANIFESTO_PACOTE}: {e}",
            dir.display()
        ))
    })?;

    let mut esperado: BTreeMap<String, String> = BTreeMap::new();
    for (n, linha) in texto.lines().enumerate() {
        if linha.trim().is_empty() {
            continue;
        }
        let (hex, nome) = linha.split_once("  ").ok_or_else(|| {
            phxsql_core::PhxError::Corrompido(format!(
                "{MANIFESTO_PACOTE}, linha {}: esperava `<sha256>  <arquivo>`",
                n + 1
            ))
        })?;
        esperado.insert(nome.trim().to_string(), hex.trim().to_ascii_lowercase());
    }

    let mut no_disco = Vec::new();
    arquivos_sob(dir, dir, &mut no_disco)?;
    no_disco.sort();

    let mut divergencias = Vec::new();
    let mut bytes = 0u64;
    let mut conferidos = 0usize;

    for nome in &no_disco {
        if nome == MANIFESTO_PACOTE {
            continue;
        }
        let dados = std::fs::read(dir.join(nome))?;
        bytes += dados.len() as u64;
        match esperado.get(nome) {
            None => divergencias.push(format!("A MAIS  {nome} -- nao esta no manifesto")),
            Some(hex) => {
                if *hex != para_hex(&sha256(&dados)) {
                    divergencias.push(format!("DIFERE  {nome} -- o SHA-256 nao bate"));
                } else {
                    conferidos += 1;
                }
            }
        }
    }

    for nome in esperado.keys() {
        if !no_disco.iter().any(|n| n == nome) {
            divergencias.push(format!(
                "FALTA   {nome} -- esta no manifesto e nao no pacote"
            ));
        }
    }

    diga!(
        "{} arquivos no manifesto, {bytes} bytes no pacote",
        esperado.len()
    );
    if divergencias.is_empty() {
        diga!("INTEGRO -- os {conferidos} arquivos batem com o SHA-256 do manifesto.");
        return Ok(());
    }
    diga!();
    diga!("{} DIVERGENCIA(S):", divergencias.len());
    for d in &divergencias {
        diga!("  {d}");
    }
    Err(phxsql_core::PhxError::Corrompido(
        "o pacote nao confere com o manifesto".into(),
    ))
}

/// Lista, recursivamente, os arquivos sob `dir`, com o caminho relativo a
/// `raiz` e sempre com barra normal -- o manifesto e o mesmo nos dois sistemas.
fn arquivos_sob(dir: &Path, raiz: &Path, saida: &mut Vec<String>) -> Result<()> {
    let mut entradas: Vec<_> = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entradas.sort_by_key(|e| e.file_name());
    for e in entradas {
        let caminho = e.path();
        if caminho.is_dir() {
            arquivos_sob(&caminho, raiz, saida)?;
        } else {
            let rel = caminho.strip_prefix(raiz).unwrap_or(&caminho);
            saida.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

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

/// `phxsql importar <dir> <tabela> <arquivo>` -- carga em lote de um arquivo.
///
/// Le pelo MESMO caminho do servidor: `phxsql_core::carga`. Uma segunda
/// implementacao do leitor aqui divergiria da do servidor no primeiro caso
/// esquisito -- e caso esquisito e o que carga de arquivo tem de sobra.
///
/// `--conferir` le e mostra o que entendeu sem gravar nada. `--seguir` pula a
/// linha ruim em vez de parar; sem ele, para na primeira -- porque **nao ha
/// transacao**, e uma carga que para na linha 700 e mais facil de consertar do
/// que uma que gravou 999 com uma faltando no meio.
fn importar(args: &[String]) -> Result<()> {
    let dir = Path::new(&args[1]);
    let tabela = &args[2];
    let arquivo = &args[3];
    let seguir = args.iter().any(|a| a == "--seguir");
    let so_conferir = args.iter().any(|a| a == "--conferir");

    let texto = std::fs::read_to_string(arquivo)?;
    let formato = match valor_da_opcao(args, "--formato") {
        Some(f) => carga::Formato::de_texto(&f)?,
        None => carga::adivinhar(&texto),
    };
    let c = carga::ler(&texto, formato)?;

    let mut t = Table::abrir(dir, tabela)?;
    let esquema = t.esquema().clone();

    diga!("arquivo .... {arquivo}");
    diga!(
        "formato .... {} ({})",
        formato.nome(),
        if valor_da_opcao(args, "--formato").is_some() {
            "escolhido"
        } else {
            "adivinhado"
        }
    );
    diga!("colunas .... {}", c.colunas.join(", "));
    diga!("linhas ..... {}", c.linhas.len());

    let desconhecidas: Vec<&String> = c
        .colunas
        .iter()
        .filter(|n| esquema.coluna_por_nome(n).is_none())
        .collect();
    if !desconhecidas.is_empty() {
        return Err(phxsql_core::PhxError::Esquema(format!(
            "a tabela {tabela} nao tem a(s) coluna(s): {}",
            desconhecidas
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    if so_conferir {
        diga!();
        diga!("-- amostra (as 5 primeiras, como o leitor entendeu) --");
        for (i, l) in c.linhas.iter().take(5).enumerate() {
            diga!("{:>4}  {}", i + 1, l.join(" | "));
        }
        diga!();
        diga!("nada foi gravado (--conferir)");
        return Ok(());
    }

    let inicio = std::time::Instant::now();
    let mut linhas = Vec::with_capacity(c.linhas.len());
    let mut recusadas: Vec<(usize, String)> = Vec::new();
    for i in 0..c.linhas.len() {
        match carga::linha_de_texto(&c, i, &esquema) {
            Ok(l) => linhas.push(l),
            Err(e) => {
                recusadas.push((i, e.to_string()));
                if !seguir {
                    break;
                }
            }
        }
    }
    let lote = t.inserir_lote(&linhas, !seguir)?;
    t.sincronizar()?;
    for (i, e) in &lote.recusadas {
        recusadas.push((*i, e.clone()));
    }
    let ms = inicio.elapsed().as_millis().max(1);

    diga!();
    diga!(
        "gravadas ... {} em {ms} ms ({} linhas/s)",
        lote.rowids.len(),
        lote.rowids.len() as u128 * 1000 / ms
    );
    if let (Some(a), Some(b)) = (lote.rowids.first(), lote.rowids.last()) {
        diga!("rowid ...... {a} a {b}");
    }
    if !recusadas.is_empty() {
        diga!("recusadas .. {}", recusadas.len());
        for (i, e) in recusadas.iter().take(20) {
            diga!("  linha {:>5}: {e}", i + 1);
        }
        // O `inserir_lote` NAO entra em transacao -- e recusado dentro de
        // uma, e a razao esta na §3.4 do `docs/TRANSACOES.md`: ele ja e
        // atomico sozinho, e empilha-lo linha a linha estouraria o teto de
        // memoria com uma carga que nao precisava de transacao nenhuma.
        //
        // Entao o que entrou antes do erro FICOU, e dizer aqui e melhor que
        // quem rodou descobrir contando as linhas depois.
        diga!();
        diga!(
            "ATENCAO: a carga nao entra em transacao -- as linhas gravadas \
               antes do erro ficaram gravadas."
        );
    }
    Ok(())
}

/// O valor de uma opcao `--nome valor` na linha de comando.
fn valor_da_opcao(args: &[String], nome: &str) -> Option<String> {
    args.iter()
        .position(|a| a == nome)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::path::PathBuf;

    fn temp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phxpac-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Monta um pacote de mentira com o mesmo formato que o `empacotar.sh`
    /// grava: `<sha256>  <caminho>`, uma linha por arquivo, caminho relativo.
    fn pacote(nome: &str) -> PathBuf {
        let d = temp(nome);
        std::fs::create_dir_all(d.join("demonstracao")).unwrap();
        std::fs::write(d.join("phxsqld"), b"o servidor").unwrap();
        std::fs::write(d.join("MANUAL.txt"), b"o manual").unwrap();
        std::fs::write(d.join("demonstracao/config.json"), b"{}").unwrap();

        let mut linhas = Vec::new();
        for rel in ["MANUAL.txt", "demonstracao/config.json", "phxsqld"] {
            let dados = std::fs::read(d.join(rel)).unwrap();
            linhas.push(format!("{}  {rel}", para_hex(&sha256(&dados))));
        }
        std::fs::write(d.join(MANIFESTO_PACOTE), linhas.join("\n") + "\n").unwrap();
        d
    }

    #[test]
    fn pacote_intacto_confere() {
        let d = pacote("intacto");
        conferir_pacote(&d).unwrap();
    }

    /// A prova que importa: UM byte trocado tem de reprovar. Sem ela, o
    /// conferidor e decoracao -- e um conferidor que aprova tudo e pior que
    /// nenhum, porque quem baixou acha que conferiu.
    #[test]
    fn um_byte_trocado_reprova() {
        let d = pacote("um-byte");
        let mut dados = std::fs::read(d.join("phxsqld")).unwrap();
        dados[0] ^= 0x01;
        std::fs::write(d.join("phxsqld"), &dados).unwrap();

        let e = conferir_pacote(&d).unwrap_err();
        assert!(e.to_string().contains("nao confere"), "{e}");
    }

    /// Mesmo tamanho, conteudo diferente: o hash e o unico que pega isto, e e
    /// o caso realista -- quem troca um binario nao muda o tamanho de graca.
    #[test]
    fn mesmo_tamanho_conteudo_outro_reprova() {
        let d = pacote("mesmo-tamanho");
        std::fs::write(d.join("MANUAL.txt"), b"o MANUAL").unwrap();
        assert!(conferir_pacote(&d).is_err());
    }

    /// Arquivo A MAIS nao mexe em nenhuma linha do manifesto: quem so confere
    /// hash de quem esta listado nunca o ve.
    #[test]
    fn arquivo_a_mais_reprova() {
        let d = pacote("a-mais");
        std::fs::write(d.join("brinde.sh"), b"rm -rf /").unwrap();
        let e = conferir_pacote(&d).unwrap_err();
        assert!(e.to_string().contains("nao confere"), "{e}");
    }

    #[test]
    fn arquivo_que_falta_reprova() {
        let d = pacote("faltando");
        std::fs::remove_file(d.join("demonstracao/config.json")).unwrap();
        assert!(conferir_pacote(&d).is_err());
    }

    #[test]
    fn pacote_sem_manifesto_avisa_em_vez_de_aprovar() {
        let d = temp("sem-manifesto");
        std::fs::write(d.join("phxsqld"), b"o servidor").unwrap();
        let e = conferir_pacote(&d).unwrap_err();
        assert!(e.to_string().contains(MANIFESTO_PACOTE), "{e}");
    }

    #[test]
    fn manifesto_com_linha_torta_avisa() {
        let d = pacote("linha-torta");
        std::fs::write(d.join(MANIFESTO_PACOTE), "isto nao e um manifesto\n").unwrap();
        let e = conferir_pacote(&d).unwrap_err();
        assert!(e.to_string().contains("linha 1"), "{e}");
    }
}
