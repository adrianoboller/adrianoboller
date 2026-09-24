//! O disco que recusa, contra o SISTEMA OPERACIONAL -- pedidos 509, 512, 522
//! e 533.
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
//! disco-que-recusa inserir  DIR N         insere e fecha pelo `Drop` (o `fechar`)
//! disco-que-recusa inserir-e-cai DIR N   insere e cai SEM `Drop` (`abort`): o
//!                                       controle do pedido 522
//! disco-que-recusa fecho2x  DIR SINAL   fecho 1; espera SINAL existir; fecho 2
//!                                       numa tabela reaberta. ABORTA=1 poe o
//!                                       gancho do servidor (o abort na recusa)
//! disco-que-recusa conferir DIR         o byte 52, o indice e o diario
//!                                       (SO_LER=1: sem gravar nada)
//! disco-que-recusa criar533 DIR N1      `clientes` (10) e N1 `filhas` do
//!                                       cliente 1, com FK conferida; janela
//!                                       FECHADA (byte 52 em 0 no disco)
//! disco-que-recusa janela533 DIR N2 M   abre a janela seguinte das filhas e sai
//!                                       pelo `Drop`, sem fecho: M=inserir poe
//!                                       N2 filhas do cliente 2; M=mover passa
//!                                       N2 filhas do 1 ao 2 no MESMO slot
//! disco-que-recusa conferir533 DIR      o byte 52 das filhas, as filhas de
//!                                       cada cliente pelo indice, e o
//!                                       `excluir` do cliente 2 (o pai com
//!                                       filhas: a regra primordial)
//! ```
//!
//! Os tres de 533 sao a emulacao do papel C (C4) e do J (C4', `nada` e
//! `move`) levada ao disco de verdade: quem decide o que chegou ao disco e o
//! roteiro (`bancada/catastrofes/queda.py`, com `sync_file_range`, `fsync` e
//! `FS_IOC_SHUTDOWN`), e nao um `std::fs::write` do arquivo velho por cima.

use phxsql_core::schema::ForeignKey;
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

/// A mae do pedido 533: dez clientes, a chave pelo `id`.
fn clientes() -> Schema {
    Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

/// A filha, com a FK conferida -- e o indice `porCliente` que a mae consulta
/// para responder «alguem aponta para mim?» antes de se deixar excluir.
fn filhas() -> Schema {
    Schema::new(
        "filhas",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )
    .conferindo(true)])
    .unwrap()
}

/// O texto do erro numa linha so, curto: o roteiro guarda uma linha por passo.
fn curto(e: impl std::fmt::Display) -> String {
    let s = e.to_string().replace('\n', " ");
    s.chars().take(110).collect()
}

/// Quantas filhas o INDICE diz que o cliente `c` tem.
fn quantas(f: &mut Table, c: i64) -> String {
    match f.buscar("porCliente", &[Value::Int(c)]) {
        Ok(v) => v.len().to_string(),
        Err(e) => format!("ERRO {}", curto(e)),
    }
}

fn conferir533(dir: &Path) {
    let cru = std::fs::read(dir.join("filhas.ndx")).unwrap_or_default();
    println!("byte52={}", cru.get(52).copied().unwrap_or(255));
    let mut f = match Table::abrir(dir, "filhas") {
        Ok(t) => t,
        Err(e) => return println!("abrir=ERRO {}", curto(e)),
    };
    let (c1, c2) = (quantas(&mut f, 1), quantas(&mut f, 2));
    println!(
        "vivas={} precisa_reconstruir={} filhas_c1={c1} filhas_c2={c2}",
        f.registros(),
        f.indice_precisa_reconstruir()
    );
    match f.verificar() {
        Ok(_) => println!("verificar=OK"),
        Err(e) => println!("verificar=ERRO {}", curto(e)),
    }
    drop(f);
    let mut m = Table::abrir(dir, "clientes").unwrap();
    let r = m.buscar("porId", &[Value::Int(2)]).unwrap();
    match m.excluir(r[0]) {
        Ok(ok) => println!("excluir_cliente_2=OK {ok}"),
        Err(e) => println!("excluir_cliente_2=ERRO {}", curto(e)),
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
        // O controle do pedido 522: a mesma carga, e o processo cai antes de
        // qualquer `Drop` -- o `fechar` nao roda, e o byte 52 fica no que a
        // primeira escrita deixou.
        "inserir-e-cai" => {
            let n: i64 = a[3].parse().unwrap();
            let mut t = Table::abrir(dir, "pedidos").unwrap();
            let ok = (1..=n).filter(|&id| t.inserir(&linha(id)).is_ok()).count();
            println!("inserir ok={ok} de {n}, e cai sem Drop");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            std::process::abort();
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
        "criar533" => {
            let n1: i64 = a[3].parse().unwrap();
            let mut m = Table::criar(dir, clientes()).unwrap();
            for i in 1..=10 {
                m.inserir(&[Value::Int(i)]).unwrap();
            }
            m.sincronizar().unwrap();
            let mut f = Table::criar(dir, filhas()).unwrap();
            for i in 1..=n1 {
                f.inserir(&[Value::Int(i), Value::Int(1)]).unwrap();
            }
            f.sincronizar().unwrap();
            println!("criar533 filhas_c1={n1}, janela fechada");
        }
        // A janela seguinte, que sai pelo `Drop` -- o `fechar` leva as
        // paginas ao nucleo e deixa o 1, sem `fsync`, como o servidor entre
        // dois fechos. O que chega ao disco depois disso e do roteiro.
        "janela533" => {
            let n2: i64 = a[3].parse().unwrap();
            let mut f = Table::abrir(dir, "filhas").unwrap();
            let base = f.registros() as i64;
            let mut ok = 0;
            if a[4] == "mover" {
                let rs = f.buscar("porCliente", &[Value::Int(1)]).unwrap();
                for r in rs.iter().take(n2 as usize) {
                    let l = f.ler(*r).unwrap().unwrap();
                    ok += usize::from(f.atualizar(*r, &[l[0].clone(), Value::Int(2)]).is_ok());
                }
            } else {
                for i in 1..=n2 {
                    ok += usize::from(f.inserir(&[Value::Int(base + i), Value::Int(2)]).is_ok());
                }
            }
            println!("janela533 {} ok={ok} de {n2}, sem fecho", a[4]);
        }
        "conferir533" => conferir533(dir),
        outro => {
            eprintln!("modo desconhecido: {outro}");
            std::process::exit(2);
        }
    }
}
