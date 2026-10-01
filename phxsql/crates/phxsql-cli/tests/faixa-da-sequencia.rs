//! Pedido 615: a faixa da `Sequence` chega a CLI pelo mesmo motor do
//! servidor, e a prova e pelo BINARIO -- um processo novo, que nao herda a
//! faixa de ninguem.
//!
//! # O defeito que esta bateria repoe
//!
//! O servidor le `replicacao.inicio_da_sequencia` e declara a faixa no
//! `Servidor::novo` (pedido 290). A CLI recebe so a pasta e abria com o zero
//! implicito: a tabela que um no da faixa 1 numerou recusava ABRIR por ela --
//! nem `info`, nem `listar`, nem `verificar`. E nao havia como dizer a faixa.
//!
//! Prova pelo binario porque a faixa e um global do PROCESSO: dentro do
//! processo de teste, o «nao declarado» de um teste seria o declarado do
//! vizinho.

use std::path::{Path, PathBuf};
use std::process::Command;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::{no, Table};

/// Diretorio que se apaga no `Drop` (pedido 150). Mora no
/// `CARGO_TARGET_TMPDIR`, e nao em `/tmp`: o lixo de uma corrida que caia no
/// meio fica dentro do `target`, que o zelador ja sabe limpar.
struct DirTemp(PathBuf);

impl DirTemp {
    fn novo(rotulo: &str) -> DirTemp {
        let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("phxsql-cli-faixa-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        DirTemp(d)
    }
}

impl Drop for DirTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Uma tabela `numerada` com `passo = 2`, numerada pelo no da faixa 1: 1, 3.
/// O contador fica em 5 -- fora da faixa zero, que e o caso que recusava.
fn tabela_da_faixa_1(dir: &Path) {
    no::definir_inicio_da_sequencia(1);
    let esq = Schema::new(
        "numerada",
        vec![
            Column::new("id", ColumnType::Sequence),
            Column::new("nome", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_passo_da_sequencia(2)
    .unwrap();
    let mut t = Table::criar(dir, esq).unwrap();
    for _ in 0..2 {
        t.inserir(&[Value::Null, Value::Str("x".into())]).unwrap();
    }
    t.sincronizar().unwrap();
}

fn phxsql(args: &[&str]) -> (bool, String) {
    let saida = Command::new(env!("CARGO_BIN_EXE_phxsql"))
        .args(args)
        .output()
        .expect("o binario phxsql nao rodou");
    let texto = format!(
        "{}{}",
        String::from_utf8_lossy(&saida.stdout),
        String::from_utf8_lossy(&saida.stderr)
    );
    (saida.status.success(), texto)
}

/// **Sem declarar, a tabela de outra faixa ABRE para ler.**
///
/// # O vermelho
///
/// Com a conferencia da abertura valendo para quem nao declarou (o
/// comportamento de ate 01/10/2026), o `info` recusa: «fora da faixa deste
/// servidor (0 de 2)».
#[test]
fn sem_declarar_a_tabela_de_outra_faixa_abre_para_ler() {
    let d = DirTemp::novo("ler");
    tabela_da_faixa_1(&d.0);
    let dir = d.0.to_str().unwrap();
    let (ok, texto) = phxsql(&["info", dir, "numerada"]);
    assert!(ok, "o info recusou a tabela da faixa 1: {texto}");
    let (ok, texto) = phxsql(&["listar", dir, "numerada"]);
    assert!(ok, "o listar recusou a tabela da faixa 1: {texto}");
    assert!(texto.contains('3'), "o listar nao mostrou o 3: {texto}");
}

/// **Sem declarar, GRAVAR recusa -- e nao numera na faixa zero.**
///
/// # O vermelho
///
/// Com a abertura liberada e a numeracao sem a conferencia, a carga daria o
/// numero 6: a faixa 0, que e a de outro no -- a colisao que a faixa existe
/// para impedir, calada.
#[test]
fn sem_declarar_gravar_na_tabela_de_outra_faixa_recusa_e_diz_como() {
    let d = DirTemp::novo("gravar");
    tabela_da_faixa_1(&d.0);
    let csv = d.0.join("carga.csv");
    std::fs::write(&csv, "nome\ny\n").unwrap();
    let dir = d.0.to_str().unwrap();
    let (ok, texto) = phxsql(&["importar", dir, "numerada", csv.to_str().unwrap()]);
    assert!(
        !ok || texto.contains("recusadas"),
        "a carga gravou sem a faixa declarada: {texto}"
    );
    assert!(
        texto.contains("--inicio-da-sequencia"),
        "a recusa nao diz como declarar a faixa: {texto}"
    );
    no::definir_inicio_da_sequencia(1);
    let t = Table::abrir(&d.0, "numerada").unwrap();
    assert_eq!(t.registros(), 2, "a recusa deixou linha gravada");
}

/// **Com a bandeira, a faixa chega ao motor**: grava 5, e a faixa errada
/// recusa a abertura como no servidor.
///
/// # O vermelho
///
/// Sem a `declarar_a_faixa` no `main`, a bandeira cai num comando que nao a
/// conhece, e a carga recusa como o nao declarado.
#[test]
fn a_bandeira_declara_a_faixa_pelo_mesmo_motor_do_servidor() {
    let d = DirTemp::novo("bandeira");
    tabela_da_faixa_1(&d.0);
    let csv = d.0.join("carga.csv");
    std::fs::write(&csv, "nome\ny\n").unwrap();
    let dir = d.0.to_str().unwrap();
    let (ok, texto) = phxsql(&[
        "importar",
        dir,
        "numerada",
        csv.to_str().unwrap(),
        "--inicio-da-sequencia",
        "1",
    ]);
    assert!(
        ok && !texto.contains("recusadas"),
        "a carga com a faixa 1 recusou: {texto}"
    );
    let (ok, texto) = phxsql(&["--inicio-da-sequencia", "1", "listar", dir, "numerada"]);
    assert!(ok && texto.contains('5'), "a faixa 1 nao deu o 5: {texto}");

    // A faixa declarada errada recusa abrir: e a mesma conferencia do
    // servidor, chegando pelo mesmo motor.
    let (ok, texto) = phxsql(&["info", dir, "numerada", "--inicio-da-sequencia", "0"]);
    assert!(!ok, "a faixa 0 abriu a tabela da faixa 1: {texto}");
    assert!(texto.contains("fora da faixa"), "{texto}");

    let (ok, texto) = phxsql(&["info", dir, "numerada", "--inicio-da-sequencia", "x"]);
    assert!(!ok && texto.contains("inteiro"), "{texto}");
}
