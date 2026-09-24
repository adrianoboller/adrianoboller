//! O disco que recusa, contra o SISTEMA OPERACIONAL -- pedidos 509 e 512.
//!
//! O executor da `bancada/catastrofes/`, que o chama dentro de um `unshare -m`
//! com tmpfs pequeno ou ext4 sobre loop com provisionamento fino. Sozinho ele
//! so grava num diretorio; quem faz o disco recusar e o roteiro.
//!
//! E o arnes do Apendice A do parecer do papel C
//! (`docs/propostas/parecer-dba-496-catastrofes-2026-09-24.md`), trazido para
//! dentro do repositorio: roteiro que resolveu algo nao morre com a sessao.
//! Uma diferenca, e ela e a que importa: o `fecho2x` faz os dois fechos no
//! MESMO processo, como o servidor os faz -- o parecer os rodava em dois
//! processos, e a recusa que fica no processo nao atravessa um `exec`.
//!
//! ```text
//! disco-que-recusa enospc   DIR MAX     insere ate o disco encher, e fecha
//!                                       no mesmo punho (SEM_SYNC=1: so o Drop)
//! disco-que-recusa criar    DIR
//! disco-que-recusa inserir  DIR N
//! disco-que-recusa fecho2x  DIR SINAL   fecho 1; espera SINAL existir; fecho 2
//!                                       numa tabela reaberta. ABORTA=1 poe o
//!                                       gancho do servidor (o abort na recusa)
//! disco-que-recusa conferir DIR         o byte 52, o indice e o diario
//!                                       (SO_LER=1: sem gravar nada)
//! ```

use phxsql_core::{Column, ColumnType, IndexColumn, IndexDef, Schema, Value};
use phxsql_store::log::Operacao;
use phxsql_store::table::Table;
use std::path::Path;

fn esquema() -> Schema {
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(id: i64) -> Vec<Value> {
    vec![
        Value::Int(id),
        Value::Str(format!("cliente numero {id:08}")),
        Value::Str("Blumenau".into()),
    ]
}

fn conferir(dir: &Path) {
    let cru = std::fs::read(dir.join("pedidos.ndx")).unwrap_or_default();
    println!("byte52={}", cru.get(52).copied().unwrap_or(255));
    let mut t = match Table::abrir(dir, "pedidos") {
        Ok(t) => t,
        Err(e) => return println!("abrir=ERRO {e}"),
    };
    let eventos = t.diario(0, 0).unwrap_or_default();
    let inclusoes = eventos
        .iter()
        .filter(|e| e.operacao == Operacao::Inclusao)
        .count();
    println!(
        "vivas={} inclusoes_no_log={} precisa_reconstruir={}",
        t.registros(),
        inclusoes,
        t.indice_precisa_reconstruir()
    );
    match t.buscar("porId", &[Value::Int(5)]) {
        Ok(r) => println!("buscar=OK {r:?}"),
        Err(e) => println!("buscar=ERRO {e}"),
    }
    if ligado("SO_LER") {
        return;
    }
    match t.verificar() {
        Ok(r) => println!("verificar=OK registros={}", r.registros),
        Err(e) => println!("verificar=ERRO {e}"),
    }
}

/// A variavel existe e nao e vazia: o roteiro passa `SEM_SYNC=""` quando quer
/// o comportamento de sempre, e «existe» sozinho o leria como ligado.
fn ligado(nome: &str) -> bool {
    std::env::var_os(nome).is_some_and(|v| !v.is_empty())
}

fn fecho(rotulo: &str, dir: &Path) {
    match Table::abrir(dir, "pedidos").and_then(|mut t| t.sincronizar()) {
        Ok(()) => println!("{rotulo}=OK"),
        Err(e) => println!("{rotulo}=ERRO {e}"),
    }
}

/// O gancho do servidor, sem o servidor: o `phxsqld` nao roda aqui dentro.
fn abortar(caminho: &Path, e: &std::io::Error) {
    println!("gancho=ABORTA {} ({e})", caminho.display());
    use std::io::Write;
    let _ = std::io::stdout().flush();
    std::process::abort();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("uso: disco-que-recusa MODO DIR [..] -- ver o cabecalho do fonte");
        std::process::exit(2);
    }
    let dir = Path::new(&a[2]);
    if ligado("ABORTA") {
        phxsql_store::sincronia::ao_recusar(abortar);
    }
    match a[1].as_str() {
        "enospc" => {
            let max: i64 = a[3].parse().unwrap();
            let mut t = Table::criar(dir, esquema()).unwrap();
            t.ligar_imagem_no_diario(true);
            t.sincronizar().unwrap();
            let (mut ok, mut erros, mut seguidos) = (0u64, 0u64, 0u32);
            for id in 1..=max {
                if t.inserir(&linha(id)).is_ok() {
                    ok += 1;
                    seguidos = 0;
                    continue;
                }
                erros += 1;
                seguidos += 1;
                if seguidos >= 40 {
                    break;
                }
            }
            println!("insercoes ok={ok} erros={erros}");
            if !ligado("SEM_SYNC") {
                match t.sincronizar() {
                    Ok(()) => println!("fecho1=OK"),
                    Err(e) => println!("fecho1=ERRO {e}"),
                }
            }
            // O segundo fecho e o `Drop`, no mesmo punho.
        }
        "criar" => {
            let mut t = Table::criar(dir, esquema()).unwrap();
            t.sincronizar().unwrap();
        }
        "inserir" => {
            let n: i64 = a[3].parse().unwrap();
            let mut t = Table::abrir(dir, "pedidos").unwrap();
            let ok = (1..=n).filter(|&id| t.inserir(&linha(id)).is_ok()).count();
            println!("inserir ok={ok} de {n}, sem fsync");
        }
        "fecho2x" => {
            let sinal = Path::new(&a[3]);
            fecho("fecho1", dir);
            use std::io::Write;
            let _ = std::io::stdout().flush();
            while !sinal.exists() {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            fecho("fecho2", dir);
        }
        "conferir" => conferir(dir),
        outro => {
            eprintln!("modo desconhecido: {outro}");
            std::process::exit(2);
        }
    }
}
