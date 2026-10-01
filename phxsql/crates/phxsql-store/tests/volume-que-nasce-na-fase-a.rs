//! **Pedido 427: o volume que NASCE durante a FASE A.**
//!
//! O retrato da FASE A fotografava so os volumes que existiam. Um vizinho que
//! escapasse do congelamento e enchesse o ultimo volume fazia nascer o
//! seguinte -- e nascer um volume nao toca nenhum dos que ja existiam, entao
//! o retrato batia e a FASE B renomeava os velhos por cima, deixando o novo
//! na geometria antiga.
//!
//! O vizinho entra por um GANCHO no meio da FASE A
//! (`Ponto::FaseADepoisDoRetrato`), e nao entre as duas chamadas: e o
//! intervalo de dentro da fase que o servidor roda fora da trava.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use comum::DirTemp;

use phxsql_core::error::PhxError;
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::ndx::panico_de_teste::{armar_gancho, desarmar, Ponto};
use phxsql_store::table::Table;

/// 30 linhas por volume e 90 linhas: tres volumes CHEIOS, e a linha 91 so
/// cabe num quarto, que nasce com ela.
const POR_VOLUME: u64 = 30;
const LINHAS: i64 = 90;

const NOME_COMPRIDO: &str =
    "fk_cliente_com_nome_comprido_de_proposito_para_estourar_a_folga_do_alinhamento";

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
    .com_paginacao(Paginacao::nova(POR_VOLUME, 9).unwrap())
    .unwrap()
}

fn cliente(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("cliente {i:04}"))]
}

fn base(rotulo: &str) -> (DirTemp, Table) {
    let d = DirTemp::novo(rotulo);
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    for i in 1..=LINHAS {
        t.inserir(&cliente(i)).unwrap();
    }
    t.sincronizar().unwrap();
    (d, t)
}

/// Os volumes do `.reg`, em ordem.
fn volumes(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.starts_with("clientes") && f.ends_with(".reg"))
        .collect();
    v.sort();
    v
}

fn novos(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".novo"))
        .collect()
}

type Foto = Vec<(u64, Option<std::time::SystemTime>)>;

fn fotografar(dir: &Path, nomes: &[String]) -> Foto {
    nomes
        .iter()
        .map(|n| {
            let m = std::fs::metadata(dir.join(n)).unwrap();
            (m.len(), m.modified().ok())
        })
        .collect()
}

/// O que o vizinho deixou nos volumes que JA existiam: quais mudaram de
/// tamanho e quais so de `mtime`. E a medida que decide as hipoteses.
#[derive(Debug, Default)]
struct Medida {
    mudaram_de_tamanho: Vec<String>,
    mudaram_so_de_mtime: Vec<String>,
}

/// Arma o vizinho: no meio da FASE A ele abre a tabela por fora do
/// congelamento e grava a linha 91, que faz nascer o volume 4.
///
/// Com `tique_grosso`, ele devolve o `mtime` de antes aos volumes velhos
/// depois de gravar: e o que um sistema de arquivos de tique de 2 s (FAT,
/// exFAT) ou de 1 s (HFS+, NFS) mostra quando a escrita cai no mesmo tique
/// do retrato. E SIMULACAO do tique, nao medida num FAT -- o que ela prova e
/// o que o retrato faz quando o `mtime` nao anda.
fn armar_vizinho(dir: &Path, tique_grosso: bool) -> Rc<RefCell<Option<Medida>>> {
    let medida = Rc::new(RefCell::new(None));
    let anotar = Rc::clone(&medida);
    let dir: PathBuf = dir.to_path_buf();
    armar_gancho(Ponto::FaseADepoisDoRetrato, move || {
        let velhos = volumes(&dir);
        let antes = fotografar(&dir, &velhos);
        let mut outro = Table::abrir(&dir, "clientes").unwrap();
        assert_eq!(outro.inserir(&cliente(LINHAS + 1)).unwrap(), 91);
        outro.sincronizar().unwrap();
        drop(outro);
        let depois = fotografar(&dir, &velhos);
        let mut m = Medida::default();
        for ((nome, a), d) in velhos.iter().zip(&antes).zip(&depois) {
            if a.0 != d.0 {
                m.mudaram_de_tamanho.push(nome.clone());
            } else if a.1 != d.1 {
                m.mudaram_so_de_mtime.push(nome.clone());
            }
        }
        if tique_grosso {
            for (nome, a) in velhos.iter().zip(&antes) {
                let f = std::fs::File::options()
                    .write(true)
                    .open(dir.join(nome))
                    .unwrap();
                f.set_modified(a.1.unwrap()).unwrap();
            }
        }
        *anotar.borrow_mut() = Some(m);
    });
    medida
}

/// A medida que matou a hipotese «nascer nao toca os existentes» e o volume
/// que nasceu, conferidos juntos: sem os dois o teste do cinto nao estaria
/// testando o 427.
fn conferir_o_cenario(dir: &Path, medida: &Rc<RefCell<Option<Medida>>>) {
    let m = medida.borrow();
    let m = m.as_ref().expect("o vizinho nao rodou no meio da FASE A");
    assert!(
        m.mudaram_de_tamanho.is_empty(),
        "um volume velho mudou de TAMANHO -- o retrato o pegaria em qualquer \
         sistema de arquivos, e este teste nao provaria o 427: {m:?}"
    );
    assert_eq!(
        m.mudaram_so_de_mtime,
        vec!["clientes#001.reg".to_string()],
        "o unico sinal do nascimento nos volumes velhos devia ser o `mtime` do \
         volume 1, onde moram os contadores: {m:?}"
    );
    assert_eq!(
        volumes(dir).len(),
        4,
        "o volume 4 nao nasceu: {:?}",
        volumes(dir)
    );
}

/// **O defeito, executado** (antes so tinha sido lido): sem o cinto, a FASE B
/// troca os tres volumes velhos e deixa o que nasceu na geometria antiga. A
/// tabela NAO ABRE mais -- fail-stop, nao corrupcao calada, mas a linha 91 so
/// volta por backup.
///
/// Fica verde com e sem o conserto, de proposito: e ele que prova que o teste
/// de baixo tem um defeito real para pegar.
#[test]
fn sem_o_cinto_o_volume_que_nasceu_deixa_a_tabela_sem_abrir() {
    let (d, mut t) = base("nasce-sem-cinto");
    let medida = armar_vizinho(&d.0, true);
    let coluna = Column::new("situacao", ColumnType::Str(12));
    let pendente = t.acrescentar_coluna_fase_a(coluna, None).unwrap();
    desarmar();
    conferir_o_cenario(&d.0, &medida);

    t.acrescentar_coluna_fase_b(pendente).unwrap();
    drop(t);

    let e = Table::abrir(&d.0, "clientes")
        .err()
        .expect("o conjunto misturado abriu: a geometria do volume 4 foi aceita");
    assert!(
        matches!(e, PhxError::Corrompido(_)),
        "recusou de outro jeito: {e:?}"
    );
}

/// **O cinto, no caminho do `acrescentar_coluna`**: o retrato conta tambem os
/// volumes que AINDA NAO existem, e o que nasce no meio aborta a troca.
///
/// Reponha o defeito fazendo o `retratar` ignorar os ausentes: o
/// `conferir_retrato` passa e o `expect_err` cai.
#[test]
fn o_volume_que_nasce_na_fase_a_do_acrescentar_coluna_aborta_a_troca() {
    let (d, mut t) = base("nasce-alargar");
    let medida = armar_vizinho(&d.0, true);
    let pendente = t
        .acrescentar_coluna_fase_a(Column::new("situacao", ColumnType::Str(12)), None)
        .unwrap();
    desarmar();
    conferir_o_cenario(&d.0, &medida);

    let e = pendente
        .conferir_retrato()
        .expect_err("o retrato nao viu o volume que nasceu na FASE A");
    let texto = e.to_string();
    let nascido = volumes(&d.0).pop().unwrap();
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert!(
        texto.contains("ABORTADA") && texto.contains(&nascido),
        "a recusa nao diz o que houve nem qual arquivo nasceu ({nascido}): {texto}"
    );
    let pasta = d.0.display().to_string();
    assert!(!texto.contains(&pasta), "a recusa publica a pasta: {texto}");
    assert!(pendente.descartar() > 0, "o descarte nao apagou nada");
    drop(t);

    let mut depois = Table::abrir(&d.0, "clientes").unwrap();
    assert_eq!(depois.slots(), 91, "a linha do vizinho sumiu");
    assert!(depois.esquema().coluna_por_nome("situacao").is_none());
    for i in 1..=91u64 {
        assert!(depois.ler(i).unwrap().is_some(), "a linha {i} sumiu");
    }
    assert!(novos(&d.0).is_empty(), "sobrou `*.novo`: {:?}", novos(&d.0));
}

/// **O IRMAO: a regravacao do esquema** (`regravar_esquema_fase_a`), que tira o
/// retrato pela mesma funcao e confere dentro da propria FASE B.
#[test]
fn o_volume_que_nasce_na_fase_a_do_esquema_aborta_a_troca() {
    let (d, mut t) = base("nasce-esquema");
    let fks =
        vec![ForeignKey::new(NOME_COMPRIDO, vec![0], "outra", vec!["id".into()]).conferindo(false)];
    let medida = armar_vizinho(&d.0, true);
    let troca = t
        .preparar_chaves_estrangeiras(&fks)
        .unwrap()
        .expect("o nome nao forcou a reescrita");
    desarmar();
    conferir_o_cenario(&d.0, &medida);

    let recibo = t.conferir_chaves_que_nascem(&fks, None).unwrap();
    let e = t
        .redeclarar_depois_de_conferir(fks, recibo, Some(troca))
        .expect_err("a troca renomeou com um volume que o retrato nao viu");
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert!(novos(&d.0).is_empty(), "sobrou `*.novo`: {:?}", novos(&d.0));
    drop(t);

    let mut depois = Table::abrir(&d.0, "clientes").unwrap();
    assert!(depois.esquema().chaves_estrangeiras().is_empty());
    assert_eq!(depois.slots(), 91, "a linha do vizinho sumiu");
    for i in 1..=91u64 {
        assert!(depois.ler(i).unwrap().is_some(), "a linha {i} sumiu");
    }
}

/// **O comportamento VELHO, com o tique fino de verdade** (ext4 desta casa):
/// o `mtime` do volume 1 ja denunciava o nascimento, e continua denunciando.
/// E a metade da hipotese «tamanho e mtime bastam?» que sobrevive -- onde o
/// tique e fino.
#[test]
fn com_tique_fino_o_volume_1_ja_denunciava_e_continua() {
    let (d, mut t) = base("nasce-tique-fino");
    let medida = armar_vizinho(&d.0, false);
    let pendente = t
        .acrescentar_coluna_fase_a(Column::new("situacao", ColumnType::Str(12)), None)
        .unwrap();
    desarmar();
    conferir_o_cenario(&d.0, &medida);
    let e = pendente
        .conferir_retrato()
        .expect_err("o retrato nao viu o volume 1 mudar");
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    pendente.descartar();
}
