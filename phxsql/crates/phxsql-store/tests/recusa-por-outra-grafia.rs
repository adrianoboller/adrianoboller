//! A recusa do `fsync` alcanca o MESMO diretorio por qualquer grafia -- pedido
//! 523.
//!
//! # O defeito, medido pelo papel C
//!
//! O 509 guarda o diretorio cujo `fsync` foi recusado, e a chave era a
//! absoluta LEXICA: a relativa e a absoluta casavam, mas o mesmo diretorio
//! por um symlink (`link/`) ou por `dir/../dir` nao. O diario recusado por
//! `real/` sincronizava Ok aberto por `link/`, e pela tabela no symlink o
//! `reindexar` e o fecho baixavam o byte 52 no proprio processo da recusa
//! (`docs/propostas/parecer-dba-509-512-2026-09-24.md`, P2). So a biblioteca:
//! o servidor cai na primeira recusa.
//!
//! # Por que este arquivo troca o diretorio de trabalho
//!
//! O caso relativo so existe contra um diretorio de trabalho, e ele e do
//! PROCESSO. Este binario e so destes testes, e os que trocam apontam todos
//! para o MESMO lugar (`std::env::temp_dir()`) -- o molde de
//! `grafia-do-diretorio-nao-divide-a-familia.rs`.
//!
//! So com `debug_assertions`: a recusa e forjada pela arma do `sincronia`.
#![cfg(all(debug_assertions, unix))]

mod comum;
use comum::DirTemp;

use phxsql_core::paginacao::Paginacao;
use phxsql_core::{Column, ColumnType, IndexColumn, IndexDef, Schema, Value};
use phxsql_store::log::{LogFile, Operacao};
use phxsql_store::sincronia::falha_de_teste::{self, Onde};
use phxsql_store::table::Table;
use std::path::{Path, PathBuf};

/// `d/real`, e `d/link` apontando para ele.
fn real_e_link(d: &Path) -> (PathBuf, PathBuf) {
    let real = d.join("real");
    std::fs::create_dir(&real).unwrap();
    let link = d.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    (real, link)
}

/// Um diario em `dir`, recusado ali pela arma: o que o 509 viu.
fn diario_recusado(dir: &Path) {
    let mut l = LogFile::criar(dir, "t", Paginacao::DESLIGADA).unwrap();
    l.sincronizar().unwrap();
    for rowid in 1..=50 {
        l.registrar(Operacao::Inclusao, rowid, 1).unwrap();
    }
    falha_de_teste::armar(dir, Onde::Fsync, 1);
    assert!(l.sincronizar().is_err(), "premissa: a recusa forjada");
}

/// O mesmo diario reaberto por `grafia`, com uma linha nova: `Ok` e o defeito.
fn sincroniza_por(grafia: &Path) -> Result<(), String> {
    let mut l = LogFile::abrir(grafia, "t", Paginacao::DESLIGADA).unwrap();
    l.registrar(Operacao::Inclusao, 51, 1).unwrap();
    l.sincronizar().map_err(|e| e.to_string())
}

/// As tres grafias do mesmo diretorio `d/real`.
fn tres_grafias(d: &Path) -> [(&'static str, PathBuf); 3] {
    let (real, link) = real_e_link(d);
    [
        ("real/", real.clone()),
        ("link/", link),
        ("real/../real/", real.join("..").join("real")),
    ]
}

/// **523: recusado por QUALQUER grafia, o diario recusa por TODAS.**
///
/// A matriz inteira, e nao so «recusa por `real/`»: a grafia com `..`
/// escapava num sentido so. Recusada por `real/`, a `real/../real/t.log`
/// ainda COMECA com `real/` componente a componente e casava por acaso;
/// recusada por `real/../real/`, a `real/t.log` nao comeca com ela. Um
/// diretorio novo por linha, porque a recusa de um nao pode segurar a do
/// outro.
#[test]
fn a_recusa_alcanca_o_mesmo_diretorio_por_toda_grafia() {
    let mut passaram = Vec::new();
    for recusa in 0..3 {
        let d = DirTemp::novo(&format!("523-grafias-{recusa}"));
        let grafias = tres_grafias(&d);
        diario_recusado(&grafias[recusa].1);
        for (nome, grafia) in &grafias {
            if !sincroniza_por(grafia).is_err_and(|e| e.contains("pedido 509")) {
                passaram.push(format!(
                    "recusado por {} e Ok por {nome}",
                    grafias[recusa].0
                ));
            }
        }
    }
    assert!(
        passaram.is_empty(),
        "{passaram:?}: a recusa e por GRAFIA, e o nucleo pode ter descartado o \
         que nao foi ao disco"
    );
}

fn esquema() -> Schema {
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

/// **523, pela tabela: o `reindexar` e o fecho pelo symlink nao baixam o
/// byte 52.** E o dano que o papel C mediu: a marca de sujo em 0 manda a
/// proxima abertura confiar num `.ndx` cujas paginas o nucleo pode ter
/// perdido.
#[test]
fn a_tabela_pelo_symlink_nao_baixa_a_marca() {
    let d = DirTemp::novo("523-tabela");
    let (real, link) = real_e_link(&d);
    let mut t = Table::criar(&real, esquema()).unwrap();
    t.sincronizar().unwrap();
    for id in 1..=500i64 {
        t.inserir(&[Value::Int(id), Value::Str(format!("cliente {id}"))])
            .unwrap();
    }
    falha_de_teste::armar(&real, Onde::Fsync, 1);
    assert!(t.sincronizar().is_err(), "premissa: a recusa forjada");
    drop(t);

    let mut t = Table::abrir(&link, "pedidos").unwrap();
    if t.indice_precisa_reconstruir() {
        let _ = t.reindexar();
    }
    let r = t.sincronizar().map_err(|e| e.to_string());
    drop(t);
    let byte_52 = std::fs::read(real.join("pedidos.ndx")).unwrap()[52];
    assert!(
        r.is_err() && byte_52 == 1,
        "pela tabela no symlink o fecho respondeu {r:?} e o byte 52 ficou em \
         {byte_52}: a marca de sujo baixou sobre um diretorio cujo fsync foi \
         recusado"
    );
}

/// **O que ja segurava continua: relativo <-> absoluto, nos dois sentidos.**
#[test]
fn relativo_e_absoluto_continuam_casando() {
    let a = DirTemp::novo("523-rel-abs");
    let b = DirTemp::novo("523-abs-rel");
    let pai = a.parent().expect("o temporario tem pai");
    std::env::set_current_dir(pai).expect("entrar no pai do temporario");
    let relativa = |d: &DirTemp| PathBuf::from(d.file_name().expect("o temporario tem nome"));
    // A absoluta sai do `current_dir` de VERDADE: onde `/tmp` for symlink, o
    // `temp_dir` e o `getcwd` diferem, e o caso mediria outra coisa.
    let absoluta = |d: &DirTemp| {
        std::env::current_dir()
            .expect("cwd legivel")
            .join(relativa(d))
    };

    diario_recusado(&relativa(&a));
    assert!(
        sincroniza_por(&absoluta(&a)).is_err(),
        "recusado pela relativa, sincronizou Ok pela absoluta"
    );
    diario_recusado(&absoluta(&b));
    assert!(
        sincroniza_por(&relativa(&b)).is_err(),
        "recusado pela absoluta, sincronizou Ok pela relativa"
    );
}
