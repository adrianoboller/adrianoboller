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
use std::time::{Duration, SystemTime};

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

/// O relogio do sistema de arquivos que o vizinho enfrenta.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Relogio {
    /// O ext4 desta casa, sem mexer em nada.
    Fino,
    /// O `mtime` NAO anda com a escrita: o vizinho devolve o valor de antes,
    /// seja ele qual for. E o adversario do 427 -- mais forte que qualquer
    /// tique, porque apaga ate a escrita que cai longe da anterior -- e por
    /// isso o que isola o cinto dos AUSENTES: so a existencia denuncia.
    Parado,
    /// **O tique grosso de verdade** (pedido 634): a escrita que cai no MESMO
    /// tique do `mtime` anterior nao o muda; a que cai em outro tique muda.
    /// O tique simulado e de uma hora para o teste nao depender de em que
    /// milissegundo ele roda -- o FAT tem 2 s, e o que importa e a regra, nao
    /// a largura. E SIMULACAO, nao medida num FAT.
    Grosso,
}

/// Uma hora: a largura do tique do [`Relogio::Grosso`].
const TIQUE: Duration = Duration::from_secs(3600);

/// Arma o vizinho: no meio da FASE A ele abre a tabela por fora do
/// congelamento e roda `escrever` nela; depois o `relogio` decide o que o
/// `mtime` dos volumes velhos mostra.
fn armar(
    dir: &Path,
    relogio: Relogio,
    escrever: impl FnOnce(&mut Table) + 'static,
) -> Rc<RefCell<Option<Medida>>> {
    let medida = Rc::new(RefCell::new(None));
    let anotar = Rc::clone(&medida);
    let dir: PathBuf = dir.to_path_buf();
    armar_gancho(Ponto::FaseADepoisDoRetrato, move || {
        let velhos = volumes(&dir);
        let antes = fotografar(&dir, &velhos);
        let mut outro = Table::abrir(&dir, "clientes").unwrap();
        escrever(&mut outro);
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
        for ((nome, a), d) in velhos.iter().zip(&antes).zip(&depois) {
            let (Some(a), Some(d)) = (a.1, d.1) else {
                continue;
            };
            let mesmo_tique = match relogio {
                Relogio::Fino => false,
                Relogio::Parado => true,
                Relogio::Grosso => d.duration_since(a).is_ok_and(|x| x < TIQUE),
            };
            if mesmo_tique {
                let f = std::fs::File::options()
                    .write(true)
                    .open(dir.join(nome))
                    .unwrap();
                f.set_modified(a).unwrap();
            }
        }
        *anotar.borrow_mut() = Some(m);
    });
    medida
}

/// O vizinho do 427: grava a linha 91, que faz nascer o volume 4.
fn armar_vizinho(dir: &Path, relogio: Relogio) -> Rc<RefCell<Option<Medida>>> {
    armar(dir, relogio, |outro| {
        assert_eq!(outro.inserir(&cliente(LINHAS + 1)).unwrap(), 91);
    })
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
    let medida = armar_vizinho(&d.0, Relogio::Parado);
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
    let medida = armar_vizinho(&d.0, Relogio::Parado);
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
    // Pedido 657: «ABORTADA» sai de tres recusas do `reg.rs`; so a do
    // retrato com o volume que NASCEU diz «nasceu».
    assert!(
        texto.contains("nasceu enquanto a reescrita montava o arquivo novo"),
        "{texto}"
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
    let medida = armar_vizinho(&d.0, Relogio::Parado);
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
    // Pedido 657: o `Conflito` sai de varias recusas; a do retrato com o
    // volume que nasceu e a unica com esta frase.
    assert!(
        e.to_string()
            .contains("nasceu enquanto a reescrita montava o arquivo novo"),
        "{e}"
    );
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
    let medida = armar_vizinho(&d.0, Relogio::Fino);
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

// ---------------------------------------------------------------------------
// Pedido 634: a ATUALIZACAO no lugar com tique grosso.
// ---------------------------------------------------------------------------

/// A linha 45 mora no volume 2. Atualiza-la nao muda tamanho nenhum nem byte
/// nenhum do cabecalho (medido no 427): o `mtime` e o unico sinal.
const ALVO: u64 = 45;

fn atualizar_o_alvo(outro: &mut Table) {
    outro
        .atualizar(ALVO, &[Value::Int(ALVO as i64), Value::Str("NOVO".into())])
        .unwrap();
}

fn nome_do_alvo(dir: &Path) -> Value {
    let mut t = Table::abrir(dir, "clientes").unwrap();
    t.ler(ALVO).unwrap().expect("a linha 45 sumiu")[1].clone()
}

fn mtimes(dir: &Path) -> Vec<SystemTime> {
    fotografar(dir, &volumes(dir))
        .into_iter()
        .map(|(_, m)| m.unwrap())
        .collect()
}

/// **O defeito do 634, no caminho do `acrescentar_coluna`.** Sem o selo, o
/// tique grosso apaga o unico sinal da atualizacao, o retrato bate, e a FASE B
/// troca o volume 2 pelo `*.novo` copiado ANTES dela: a linha 45 volta de
/// «NOVO» para «cliente 0045», sem erro.
///
/// Reponha o defeito tirando o `selar` do `retratar_existentes`: o
/// `conferir_retrato` passa e o `expect_err` cai.
#[test]
fn a_atualizacao_na_fase_a_com_tique_grosso_aborta_a_troca_634() {
    let (d, mut t) = base("selo-alargar");
    let medida = armar(&d.0, Relogio::Grosso, atualizar_o_alvo);
    let pendente = t
        .acrescentar_coluna_fase_a(Column::new("situacao", ColumnType::Str(12)), None)
        .unwrap();
    desarmar();
    let m = medida.borrow_mut().take().expect("o vizinho nao rodou");
    assert!(
        m.mudaram_de_tamanho.is_empty(),
        "a atualizacao mudou tamanho -- o retrato a pegaria sem o selo: {m:?}"
    );
    assert!(
        m.mudaram_so_de_mtime
            .contains(&"clientes#002.reg".to_string()),
        "a atualizacao da linha 45 nao tocou o volume 2: {m:?}"
    );

    let e = pendente
        .conferir_retrato()
        .expect_err("o retrato nao viu a atualizacao no tique grosso: a troca a desfaria");
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert!(e.to_string().contains("ABORTADA"), "{e}");
    // Pedido 657: «ABORTADA» sai de tres recusas; a do retrato que viu o
    // volume MUDAR (e nao nascer, nem o `*.novo` trocado) e esta.
    assert!(
        e.to_string()
            .contains("mudou enquanto a reescrita montava o arquivo novo"),
        "{e}"
    );
    pendente.descartar();
    drop(t);
    assert_eq!(nome_do_alvo(&d.0), Value::Str("NOVO".into()));
}

/// **O IRMAO**: a regravacao do esquema tira o retrato pela mesma funcao e
/// confere dentro da propria FASE B.
#[test]
fn a_atualizacao_na_fase_a_do_esquema_com_tique_grosso_aborta_a_troca_634() {
    let (d, mut t) = base("selo-esquema");
    let fks =
        vec![ForeignKey::new(NOME_COMPRIDO, vec![0], "outra", vec!["id".into()]).conferindo(false)];
    let _medida = armar(&d.0, Relogio::Grosso, atualizar_o_alvo);
    let troca = t
        .preparar_chaves_estrangeiras(&fks)
        .unwrap()
        .expect("o nome nao forcou a reescrita");
    desarmar();
    let recibo = t.conferir_chaves_que_nascem(&fks, None).unwrap();
    let e = t
        .redeclarar_depois_de_conferir(fks, recibo, Some(troca))
        .expect_err("a troca renomeou por cima da atualizacao");
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert!(
        e.to_string()
            .contains("mudou enquanto a reescrita montava o arquivo novo"),
        "{e}"
    );
    drop(t);
    assert_eq!(nome_do_alvo(&d.0), Value::Str("NOVO".into()));
}

/// O selo e VISIVEL entre as fases, e a troca abortada devolve a cada volume
/// o `mtime` que ele tinha -- quem copia por data nao herda 1980.
#[test]
fn a_troca_abortada_devolve_o_mtime_original_634() {
    let (d, mut t) = base("selo-devolve");
    let originais = mtimes(&d.0);
    let pendente = t
        .acrescentar_coluna_fase_a(Column::new("situacao", ColumnType::Str(12)), None)
        .unwrap();
    let ano_1980 = SystemTime::UNIX_EPOCH + Duration::from_secs(315_619_200);
    assert_eq!(
        mtimes(&d.0),
        vec![ano_1980; originais.len()],
        "o retrato nao selou os volumes"
    );
    pendente.conferir_retrato().unwrap();
    pendente.descartar();
    assert_eq!(mtimes(&d.0), originais, "o mtime original nao voltou");
}

/// O comportamento VELHO: sem ninguem escrever, a troca acontece e o volume
/// trocado nao fica com a data de 1980.
#[test]
fn sem_vizinho_a_troca_acontece_e_nao_fica_em_1980_634() {
    let (d, mut t) = base("selo-troca");
    let pendente = t
        .acrescentar_coluna_fase_a(Column::new("situacao", ColumnType::Str(12)), None)
        .unwrap();
    pendente.conferir_retrato().unwrap();
    t.acrescentar_coluna_fase_b(pendente).unwrap();
    let ano_1981 = SystemTime::UNIX_EPOCH + Duration::from_secs(347_155_200);
    for m in mtimes(&d.0) {
        assert!(m > ano_1981, "volume trocado ficou com a sentinela: {m:?}");
    }
    drop(t);
    assert_eq!(
        nome_do_alvo(&d.0),
        Value::Str(format!("cliente {:04}", ALVO))
    );
}
