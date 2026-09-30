//! Pedido 508: o separador de volume troca de `_` para `#`, e o disco do
//! binario anterior migra UMA vez, na abertura da raiz.
//!
//! O leiaute velho se monta aqui pelo caminho mais honesto que ha sem o
//! binario anterior: grava-se com o motor de hoje e desfaz-se a troca a mao --
//! cada `#` do NOME volta a ser `_`, o `.pag` volta a dizer `clientes_A.reg`,
//! e a marca do formato sai. O conteudo dos arquivos e byte a byte o que o
//! motor grava; so o nome muda, que e exatamente o que o 508 mudou. (A trilha
//! `.lgpd` gravada de verdade pelo binario anterior -- `tests/fixtures/` --
//! passa pela mesma migracao em `tests/trilha-lgpd.rs`.)

mod comum;

use std::path::{Path, PathBuf};

use comum::DirTemp;
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::{Database, Instancia};
use phxsql_store::separador::MARCA_FORMATO_VOLUMES;

fn esquema(nome: &str, paginacao: Option<Paginacao>) -> Schema {
    let e = Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(20)).obrigatoria(),
        ],
        vec![],
    )
    .unwrap();
    match paginacao {
        Some(p) => e.com_paginacao(p).unwrap(),
        None => e,
    }
}

fn gravar(db: &Database, schema: Option<&str>, e: Schema, linhas: &[(i64, &str)]) {
    let mut t = db.criar_tabela(schema, e).unwrap();
    for (id, nome) in linhas {
        t.inserir(&[Value::Int(*id), Value::Str((*nome).into())])
            .unwrap();
    }
    t.sincronizar().unwrap();
}

/// As linhas de uma tabela, na ordem de digitacao do motor.
fn linhas(db: &Database, qualificado: &str) -> Vec<(i64, String)> {
    let mut t = db.abrir_qualificada(qualificado).unwrap();
    t.varrer()
        .unwrap()
        .into_iter()
        .map(|(_, l)| match (&l[0], &l[1]) {
            (Value::Int(i), Value::Str(s)) => (*i, s.clone()),
            outro => panic!("linha inesperada {outro:?}"),
        })
        .collect()
}

const VENDAS: &[(i64, &str)] = &[(1, "a"), (2, "b"), (3, "c"), (4, "d"), (5, "e")];
const VENDAS_2024: &[(i64, &str)] = &[(100, "x"), (101, "y"), (102, "z")];
const CLIENTES: &[(i64, &str)] = &[(1, "Zeca"), (2, "Ana"), (3, "0800"), (4, "Bruno")];
const ITENS: &[(i64, &str)] = &[(7, "p"), (8, "q"), (9, "r")];

/// Monta a base de hoje: `vendas` paginada (3 digitos, 3 volumes), a tabela
/// `vendas_2024` SEM paginacao ao lado dela -- nome que so o binario de antes
/// do 368 aceitava --, `clientes` por letra (com `.pag`) e, num schema,
/// `itens` com sufixo de 4 digitos.
fn montar(base: &Path) {
    let inst = Instancia::nova(base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    gravar(
        &db,
        None,
        esquema("vendas", Some(Paginacao::nova(2, 99).unwrap())),
        VENDAS,
    );
    gravar(&db, None, esquema("vendas_2024", None), VENDAS_2024);
    gravar(
        &db,
        None,
        esquema("clientes", Some(Paginacao::por_letra(100, 1).unwrap())),
        CLIENTES,
    );
    gravar(
        &db,
        Some("arq"),
        esquema(
            "itens",
            Some(Paginacao::nova(2, 99).unwrap().com_digitos(4).unwrap()),
        ),
        ITENS,
    );
}

/// Desfaz a troca do 508 no NOME: cada `#` volta a `_`, o `.pag` volta a
/// escrever o nome velho, e a marca do formato sai -- o disco fica como o
/// binario anterior o deixaria.
fn para_o_leiaute_velho(dir: &Path) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let caminho = e.path();
        if caminho.is_dir() {
            para_o_leiaute_velho(&caminho);
            continue;
        }
        let nome = e.file_name().to_string_lossy().into_owned();
        if nome == MARCA_FORMATO_VOLUMES {
            std::fs::remove_file(&caminho).unwrap();
            continue;
        }
        if nome.ends_with(".pag") {
            let texto = std::fs::read_to_string(&caminho).unwrap();
            std::fs::write(&caminho, texto.replace('#', "_")).unwrap();
        }
        if nome.contains('#') {
            std::fs::rename(&caminho, dir.join(nome.replace('#', "_"))).unwrap();
        }
    }
}

fn nomes(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// O leiaute velho montado, com a premissa conferida: se o desfazer nao
/// produzisse os nomes antigos, a prova de migracao passaria por engano.
fn base_velha(rotulo: &str) -> (DirTemp, PathBuf) {
    let d = DirTemp::novo(rotulo);
    let base = d.0.join("base");
    montar(&base);
    para_o_leiaute_velho(&base);
    let loja = base.join("loja");
    let n = nomes(&loja);
    for esperado in [
        "vendas_001.reg",
        "vendas_003.reg",
        "vendas_2024.reg",
        "clientes_A.reg",
        "clientes_0.reg",
        "clientes_001.log",
    ] {
        assert!(
            n.contains(&esperado.to_string()),
            "premissa: {esperado} em {n:?}"
        );
    }
    assert!(!n.iter().any(|x| x.contains('#')), "premissa: {n:?}");
    assert!(nomes(&loja.join("arq")).contains(&"itens_0001.reg".to_string()));
    (d, base)
}

/// Depois da migracao, tudo le as mesmas linhas -- e os nomes sao os novos.
fn conferir_migrada(base: &Path) {
    let inst = Instancia::nova(base).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    assert_eq!(
        db.todas_as_tabelas().unwrap(),
        vec!["arq.itens", "clientes", "vendas", "vendas_2024"]
    );
    let dono = |v: &[(i64, &str)]| -> Vec<(i64, String)> {
        v.iter().map(|(i, s)| (*i, s.to_string())).collect()
    };
    assert_eq!(linhas(&db, "vendas"), dono(VENDAS));
    assert_eq!(linhas(&db, "vendas_2024"), dono(VENDAS_2024));
    assert_eq!(linhas(&db, "arq.itens"), dono(ITENS));
    // Por letra, a ordem e a dos baldes: A, B, Z, 0.
    assert_eq!(
        linhas(&db, "clientes"),
        dono(&[(2, "Ana"), (4, "Bruno"), (1, "Zeca"), (3, "0800")])
    );

    let loja = base.join("loja");
    let n = nomes(&loja);
    for novo in [
        "vendas#001.reg",
        "vendas#003.reg",
        "clientes#A.reg",
        "clientes#001.log",
    ] {
        assert!(n.contains(&novo.to_string()), "{novo} faltou em {n:?}");
    }
    // A tabela de nome ambiguo ficou com o nome dela: o cabecalho disse que
    // `vendas_2024.reg` nao e o volume 2024 de `vendas`.
    assert!(n.contains(&"vendas_2024.reg".to_string()), "{n:?}");
    assert!(!n.contains(&"vendas#2024.reg".to_string()), "{n:?}");
    assert!(!n
        .iter()
        .any(|x| x.starts_with("vendas_0") || x.starts_with("clientes_")));
    assert!(nomes(&loja.join("arq")).contains(&"itens#0001.reg".to_string()));
    // A marca, nos dois diretorios.
    assert!(loja.join(MARCA_FORMATO_VOLUMES).is_file());
    assert!(loja.join("arq").join(MARCA_FORMATO_VOLUMES).is_file());
    // E o `.pag` regerado: o descritor diz o arquivo que existe.
    let pag = std::fs::read_to_string(loja.join("clientes.pag")).unwrap();
    assert!(pag.contains("\"clientes#A.reg\""), "{pag}");
    assert!(!pag.contains("clientes_A.reg"), "{pag}");
}

/// **O disco do binario anterior abre, migra e le as mesmas linhas** -- e a
/// tabela `vendas_2024` de antes do 368 NAO vira o volume 2024 de `vendas`.
///
/// **Defeito reposto** (a migracao decidindo pelo NOME, como o catalogo
/// anterior: digitos depois do `_` sao volume): `vendas_2024.reg` vira
/// `vendas#2024.reg`, a tabela `vendas_2024` some e `vendas` ganha um volume
/// alheio -- a listagem e a leitura caem.
#[test]
fn o_leiaute_velho_abre_migra_e_le_as_mesmas_linhas() {
    let (_d, base) = base_velha("508-migra");
    conferir_migrada(&base);
    // Idempotente: a segunda abertura nao renomeia nada e le igual.
    let antes = nomes(&base.join("loja"));
    conferir_migrada(&base);
    assert_eq!(nomes(&base.join("loja")), antes);
}

/// O volume nos DOIS nomes ao mesmo tempo nao e um estado que a migracao
/// produz: ela para como corrompido, sem renomear nada, em vez de escolher.
#[test]
fn o_mesmo_volume_nos_dois_nomes_para_como_corrompido() {
    let (_d, base) = base_velha("508-dois");
    let loja = base.join("loja");
    std::fs::copy(loja.join("vendas_002.reg"), loja.join("vendas#002.reg")).unwrap();
    let antes = nomes(&loja);
    let (feitos, falhas) = phxsql_store::separador::migrar_base(&base);
    assert_eq!(feitos, 0);
    assert_eq!(falhas.len(), 1, "{falhas:?}");
    assert!(falhas[0].contains("nos dois"), "{falhas:?}");
    assert_eq!(nomes(&loja), antes, "renomeou alguma coisa antes de parar");
    // E a abertura do database recusa com o motivo, em vez de listar errado.
    let inst = Instancia::nova(&base).unwrap();
    let Err(e) = inst.abrir_database("loja") else {
        panic!("o database abriu com o volume nos dois nomes");
    };
    assert!(e.to_string().contains("nos dois"), "{e}");
}

/// O aviso que a sonda escreve ao parar entre dois `rename`s.
const AVISO_508: &str = "SONDA-508: parada entre dois renomes do separador";

/// **A queda entre dois `rename`s, antes da marca, contra o sistema
/// operacional**: o filho e este mesmo binario de teste, parado pela pausa do
/// `panico_de_teste` na SEGUNDA passagem -- um volume ja com `#`, o resto com
/// `_`, e a marca ainda por gravar -- e morto por `Child::kill()` (SIGKILL,
/// sem desenrolar nada). A reabertura completa a migracao e le tudo.
///
/// **Defeito reposto** (a marca gravada ANTES dos `rename`s): o filho morre
/// com o diretorio marcado e meio migrado, a reabertura nao migra o resto, e
/// a tabela que ficou com o nome velho some -- `conferir_migrada` cai.
#[cfg(all(unix, debug_assertions))]
#[test]
fn a_queda_entre_dois_renomes_completa_na_reabertura() {
    use std::io::BufRead as _;
    let (_d, base) = base_velha("508-queda");
    let mut filho = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "sonda_508_para_entre_dois_renomes",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("PHX_SONDA_508", &base)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // O erro padrao do filho e lido numa thread: a pausa nao fecha o cano, e
    // ler aqui mesmo penduraria o teste se o aviso nunca viesse.
    let erro = filho.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for linha in std::io::BufReader::new(erro).lines().map_while(|l| l.ok()) {
            let _ = tx.send(linha);
        }
    });
    let prazo = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut visto = String::new();
    let parou = loop {
        let falta = prazo.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(falta) {
            Ok(l) if l.contains(AVISO_508) => break true,
            Ok(l) => {
                visto.push_str(&l);
                visto.push('\n');
            }
            Err(_) => break false,
        }
    };
    filho.kill().unwrap();
    let _ = filho.wait();
    assert!(parou, "a sonda nao parou entre os renomes:\n{visto}");

    // O estado do meio, pelo sistema de arquivos: um ja novo, o resto velho,
    // e nenhuma marca.
    let loja = base.join("loja");
    let n = nomes(&loja);
    let novos = n.iter().filter(|x| x.contains('#')).count();
    let velhos = n
        .iter()
        .filter(|x| x.starts_with("vendas_0") || x.starts_with("clientes_"))
        .count();
    eprintln!("508 queda: {novos} renomeado(s), {velhos} ainda com `_`");
    assert_eq!(novos, 1, "{n:?}");
    assert!(velhos > 0, "{n:?}");
    assert!(
        !loja.join(MARCA_FORMATO_VOLUMES).exists(),
        "a marca veio antes da hora"
    );

    conferir_migrada(&base);
}

/// A sonda da prova de cima: arma a pausa na segunda passagem pelo ponto
/// entre os renomes e abre a raiz. Nao volta -- o pai a mata.
#[cfg(debug_assertions)]
#[test]
#[ignore = "sonda: roda so reexecutada por a_queda_entre_dois_renomes_completa_na_reabertura"]
fn sonda_508_para_entre_dois_renomes() {
    use phxsql_store::ndx::panico_de_teste::{armar_pausa, Ponto};
    let Some(base) = std::env::var_os("PHX_SONDA_508") else {
        return;
    };
    armar_pausa(Ponto::EntreRenomesDoSeparador, 2, AVISO_508);
    let _ = Instancia::nova(&base);
    panic!("a pausa nao disparou: a migracao terminou sem passar duas vezes pelo ponto");
}
