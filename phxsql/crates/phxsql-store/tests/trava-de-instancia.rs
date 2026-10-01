//! **Pedido 635: a trava de instancia, provada por PROCESSOS reais.**
//!
//! Teste unitario nao prova trava entre processos -- o `flock` e por
//! descritor, e dentro de um processo o registro reaproveita a posse de
//! proposito. Aqui o binario deste teste se reexecuta como FILHO (o papel
//! vai pela variavel `PHX_TRAVA_PAPEL`), e o pai e o outro processo.
//!
//! O que se prova, cada um contra o sistema operacional:
//!
//! * o segundo gravador e RECUSADO, com `INSTANCIA_OCUPADA` e o pid de quem
//!   segura -- e a tabela dele nao ganha linha nenhuma;
//! * ler continua livre enquanto o outro grava;
//! * o primeiro saiu, o segundo abre;
//! * `kill -9` no primeiro NAO deixa trava eterna: o nucleo solta;
//! * a raiz que gravou e ficou OCIOSA continua segurando (o `phxsqld` entre
//!   dois pedidos), e solta quando morre.
#![cfg(unix)]

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use phxsql_core::error::PhxError;
use phxsql_core::{Column, ColumnType, IndexColumn, IndexDef, Schema, Value};
use phxsql_store::table::{SemEscrever, Table};
use phxsql_store::Instancia;

const PAPEL: &str = "PHX_TRAVA_PAPEL";
const PASTA: &str = "PHX_TRAVA_PASTA";

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("c{i}"))]
}

/// O corpo do FILHO. Fora do papel, nao faz nada: e so a porta pela qual o
/// pai reexecuta este binario.
#[test]
fn papel_do_filho() {
    let Some(papel) = std::env::var_os(PAPEL) else {
        return;
    };
    let pasta = std::path::PathBuf::from(std::env::var_os(PASTA).unwrap());
    // `_segura` vive ate o fim da funcao: e a posse da trava.
    let _segura: Box<dyn std::any::Any> = match papel.to_str().unwrap() {
        "tabela" => {
            let mut t = Table::abrir(&pasta, "clientes").unwrap();
            t.inserir(&linha(100)).unwrap();
            t.sincronizar().unwrap();
            Box::new(t)
        }
        "raiz" => {
            // A raiz grava e fica OCIOSA: a tabela fecha, a raiz fica.
            let inst = Instancia::nova(&pasta).unwrap();
            let db = inst.garantir_database("loja").unwrap();
            let mut t = db.abrir_tabela(None, "clientes").unwrap();
            t.inserir(&linha(200)).unwrap();
            t.sincronizar().unwrap();
            drop(t);
            drop(db);
            Box::new(inst)
        }
        outro => panic!("papel desconhecido: {outro}"),
    };
    // Direto no descritor, e nao por `println!`: o `println!` passa pela
    // captura do harness, e o pai esperaria para sempre.
    let mut saida = std::io::stdout();
    writeln!(saida, "SEGURANDO {}", std::process::id()).unwrap();
    saida.flush().unwrap();
    // Segura ate o pai fechar a entrada (ou mata-lo).
    let mut resto = String::new();
    let _ = std::io::stdin().read_line(&mut resto);
}

/// O filho que segura, e a saida dele -- que fica ABERTA ate ele sair: o
/// harness dele ainda escreve «ok» no fim, e com a ponta fechada ele morreria
/// de `BrokenPipe` e sairia como falha.
struct Filho {
    processo: Child,
    saida: BufReader<std::process::ChildStdout>,
}

/// Sobe o filho no `papel` e espera ele dizer que segura. Devolve o pid.
///
/// O `allow`: o filho SAI desta funcao dentro do [`Filho`], e quem o colhe e
/// o `soltar` (ou o `kill` + `wait` do teste da queda) -- o lint nao segue o
/// processo para dentro da estrutura.
#[allow(clippy::zombie_processes)]
fn subir(papel: &str, pasta: &Path) -> (Filho, u32) {
    let eu = std::env::current_exe().unwrap();
    let mut filho = Command::new(eu)
        .args([
            "--exact",
            "papel_do_filho",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PAPEL, papel)
        .env(PASTA, pasta)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut leitor = BufReader::new(filho.stdout.take().unwrap());
    let mut l = String::new();
    loop {
        l.clear();
        if leitor.read_line(&mut l).unwrap_or(0) == 0 {
            // Colhe o filho antes de reprovar: zumbi nao ajuda ninguem.
            let _ = filho.kill();
            let _ = filho.wait();
            panic!("o filho saiu sem segurar a trava");
        }
        // O harness escreve «test papel_do_filho ... » na MESMA linha, antes.
        if let Some((_, pid)) = l.trim().split_once("SEGURANDO ") {
            let pid = pid.parse().unwrap();
            return (
                Filho {
                    processo: filho,
                    saida: leitor,
                },
                pid,
            );
        }
    }
}

fn soltar(mut filho: Filho) {
    drop(filho.processo.stdin.take());
    let mut resto = String::new();
    let _ = std::io::Read::read_to_string(&mut filho.saida, &mut resto);
    assert!(
        filho.processo.wait().unwrap().success(),
        "o filho falhou:\n{resto}"
    );
}

fn tabela_pronta(rotulo: &str) -> DirTemp {
    let d = DirTemp::novo(rotulo);
    let mut t = Table::criar(&d.0, esquema()).unwrap();
    t.inserir(&linha(1)).unwrap();
    t.sincronizar().unwrap();
    d
}

fn recusa_esperada(r: Result<Table, PhxError>, pid: u32) {
    let e = r
        .err()
        .expect("o SEGUNDO processo abriu para gravar a pasta que o primeiro grava");
    assert!(
        matches!(e, PhxError::InstanciaOcupada(_)),
        "recusou como {e:?}"
    );
    assert_eq!(e.codigo(), 4008);
    assert!(
        !e.adianta_repetir(),
        "repetir nao adianta: o outro fica de pe"
    );
    let texto = e.to_string();
    assert!(
        texto.contains(&format!("processo {pid}")),
        "a recusa nao diz quem segura (pid {pid}): {texto}"
    );
    assert!(
        !texto.contains("/tmp"),
        "a recusa publica o caminho: {texto}"
    );
}

/// **O defeito do 635, nos dois sentidos.** Com a trava, o segundo processo e
/// recusado; reponha o defeito (tire o `tomar` do `Table::abrir_com`) e o
/// `expect` da recusa cai -- o segundo abre e grava por cima dos contadores.
#[test]
fn o_segundo_gravador_e_recusado_e_ler_continua_livre() {
    let d = tabela_pronta("trava-segundo");
    let (filho, pid) = subir("tabela", &d.0);

    recusa_esperada(Table::abrir(&d.0, "clientes"), pid);
    // Criar tabela na pasta tambem e gravar.
    let mut outra = esquema();
    outra = Schema::new("outra", outra.colunas().to_vec(), vec![]).unwrap();
    recusa_esperada(Table::criar(&d.0, outra), pid);

    // Ler continua livre, com o outro gravando.
    match Table::abrir_para_ler(&d.0, "clientes").unwrap() {
        SemEscrever::Aberta(mut t) => {
            assert!(t.ler(1).unwrap().is_some(), "a leitura nao viu a linha 1")
        }
        SemEscrever::PrecisaEscrever(p) => panic!("a leitura pediu para escrever: {p}"),
    }

    // O primeiro sai; o segundo abre e ve a linha do primeiro.
    soltar(filho);
    let mut t = Table::abrir(&d.0, "clientes").expect("o primeiro saiu e a trava ficou");
    assert!(t.ler(2).unwrap().is_some(), "a linha do primeiro sumiu");
    assert_eq!(t.slots(), 2, "o recusado gravou alguma coisa");
    t.inserir(&linha(3)).unwrap();
}

/// **A queda nao deixa trava eterna**: `kill -9` (o `Child::kill` do Unix e
/// `SIGKILL`) no gravador, e o nucleo solta a trava com o processo.
#[test]
fn kill_9_no_gravador_nao_deixa_trava_eterna() {
    let d = tabela_pronta("trava-kill");
    let (mut filho, pid) = subir("tabela", &d.0);
    recusa_esperada(Table::abrir(&d.0, "clientes"), pid);
    filho.processo.kill().unwrap();
    filho.processo.wait().unwrap();
    let mut t = Table::abrir(&d.0, "clientes").expect("a trava sobreviveu ao kill -9");
    t.inserir(&linha(5)).unwrap();
}

/// **A raiz ociosa continua segurando** -- e o `phxsqld` entre dois pedidos:
/// a tabela fechou, a raiz que gravou nao. Reponha o defeito tirando o
/// `fixar_a_trava` do `Database::abrir_tabela` e o `expect` da recusa cai.
#[test]
fn a_raiz_que_gravou_segura_ociosa_e_solta_quando_morre() {
    let base = DirTemp::novo("trava-raiz");
    {
        let inst = Instancia::nova(&base.0).unwrap();
        let db = inst.criar_database("loja").unwrap();
        let mut t = db.criar_tabela(None, esquema()).unwrap();
        t.inserir(&linha(1)).unwrap();
        t.sincronizar().unwrap();
    }
    let (filho, pid) = subir("raiz", &base.0);
    recusa_esperada(Table::abrir(base.0.join("loja"), "clientes"), pid);
    soltar(filho);
    let mut t =
        Table::abrir(base.0.join("loja"), "clientes").expect("a raiz morreu e a trava ficou");
    assert!(t.ler(1).unwrap().is_some());
}

/// **O comportamento VELHO**: um processo so -- dois leitores juntos, e
/// depois duas tabelas gravaveis e uma criacao na mesma pasta ao mesmo
/// tempo, sem recusa nenhuma.
#[test]
fn um_processo_so_abre_quantas_quiser_na_mesma_pasta() {
    let d = tabela_pronta("trava-um-so");
    let l1 = Table::abrir_para_ler(&d.0, "clientes").unwrap();
    let l2 = Table::abrir_para_ler(&d.0, "clientes").unwrap();
    assert!(matches!(l1, SemEscrever::Aberta(_)));
    assert!(matches!(l2, SemEscrever::Aberta(_)));
    let mut a = Table::abrir(&d.0, "clientes").unwrap();
    let b = Table::abrir(&d.0, "clientes").unwrap();
    let _outra = Table::criar(
        &d.0,
        Schema::new("outra", esquema().colunas().to_vec(), vec![]).unwrap(),
    )
    .unwrap();
    a.inserir(&linha(7)).unwrap();
    a.sincronizar().unwrap();
    drop((a, b, l1, l2));
}
