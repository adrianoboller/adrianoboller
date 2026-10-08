//! Pedido 712 (F8): a restauracao completa as marcas de transacao NO PALCO,
//! antes de reconstruir o indice e antes de o database entrar na raiz --
//! provado contra o processo de verdade.
//!
//! # O defeito
//!
//! A marca `.tx` viaja no backup de proposito (`backup.rs`, doc do modulo): e
//! ela que completa o `COMMIT` que a copia pegou no meio. Mas o palco da
//! restauracao so reconstruia o indice marcado -- a ordem do 522 invertida --,
//! e o database entrava na raiz COM a marca de pe:
//!
//! - **F8b**: a copia fria de um servidor que caiu no meio da passada mostrava
//!   a venda PELA METADE logo depois da restauracao, ate o proximo arranque
//!   do servidor de destino;
//! - **F8a**: a marca do `COMMIT` que so esperava a janela ficava no destino,
//!   e o proximo arranque dele a completava por cima do que se escreveu desde
//!   a restauracao (o F1 pela copia).
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;

const TOKEN: &str = "marca-que-viaja";
const ITENS: u64 = 5;

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn pedir(&mut self, corpo: &str) -> Option<Json> {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").ok()?;
        let mut r = String::new();
        match self.leitor.read_line(&mut r) {
            Ok(n) if n > 0 => Some(Json::analisar(&r).unwrap()),
            _ => None,
        }
    }

    fn exigir(&mut self, corpo: &str) -> Json {
        let r = self.pedir(corpo).expect("a conexao caiu sem resposta");
        assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
        r
    }
}

/// Sobe o `phxsqld` com a janela PARADA (a marca fica pendente ate o
/// `SIGKILL`) e, quando pedido, com o gancho do 702.
fn subir(dir: &Path, vez: u32, parar_no_commit: Option<u64>) -> (Filho, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }},
                "recursos": {{ "durabilidade": "por_lote", "lote_operacoes": 1000000,
                               "lote_milissegundos": 600000 }},
                "replicacao": {{ "papel": "source", "id_servidor": "caixa01",
                                 "imagem_da_linha": true }}
            }}"#,
            base = dir.join("dados").display().to_string(),
            chave = dir.join("chave-do-fio.hex").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join(format!("stderr-{vez}.txt"));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_phxsqld"));
    cmd.arg("--config")
        .arg(&config)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
        .env_remove("PHXSQL_TESTE_PARAR_NO_COMMIT");
    if let Some(n) = parar_no_commit {
        cmd.env("PHXSQL_TESTE_PARAR_NO_COMMIT", n.to_string());
    }
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta)
}

fn criar_as_tabelas(porta: u16) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"criar_database","database":"loja""#);
    for tabela in ["vendas", "itens", "pagamentos"] {
        b.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Str(20)"}}],
               "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}}]"#
        ));
    }
}

/// A venda: uma linha em `vendas`, `ITENS` em `itens`, uma em `pagamentos`,
/// num `COMMIT` so. Devolve se o `COMMIT` respondeu.
fn vender(porta: u16) -> bool {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"begin","database":"loja""#);
    b.exigir(r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"id":1,"venda":"1"}"#);
    for i in 1..=ITENS {
        b.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"itens","linha":{{"id":{i},"venda":"1"}}"#
        ));
    }
    b.exigir(
        r#""op":"inserir","database":"loja","tabela":"pagamentos","linha":{"id":1,"venda":"1"}"#,
    );
    b.pedir(r#""op":"commit""#).is_some()
}

fn contar(porta: u16, database: &str, tabela: &str) -> usize {
    let mut b = Ligacao::nova(porta);
    let r = b.exigir(&format!(
        r#""op":"varrer","database":"{database}","tabela":"{tabela}","max":5000"#
    ));
    r.campo("resultado")
        .and_then(|r| r.campo("linhas"))
        .and_then(Json::lista)
        .map(<[Json]>::len)
        .unwrap_or(0)
}

fn marcas(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|ls| {
            ls.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("transacao_") && n.ends_with(".tx"))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn zips(dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if p.extension().is_some_and(|x| x == "zip") {
                v.push(p);
            }
        }
    }
    v
}

/// **F8b.** A copia fria (a da CLI, `phxsql backup --zip`) de um servidor que
/// caiu no meio da passada, restaurada noutro servidor. Vermelho medido antes
/// do conserto: `(1, 2, 0)` -- a venda pela metade visivel logo depois da
/// restauracao -- e a marca no destino.
#[test]
fn a_copia_de_um_servidor_caido_restaura_a_venda_inteira() {
    let caido = DirTemp::novo("viaja-caido");
    let (filho, porta) = subir(&caido.0, 1, None);
    criar_as_tabelas(porta);
    drop(filho);
    let (filho, porta) = subir(&caido.0, 2, Some(3));
    assert!(
        !vender(porta),
        "o COMMIT respondeu: o gancho nao matou o processo"
    );
    let ate = Instant::now() + Duration::from_secs(20);
    while !std::fs::read_to_string(caido.0.join("stderr-2.txt"))
        .unwrap_or_default()
        .contains("teste: parado no meio da passada do COMMIT")
    {
        assert!(Instant::now() < ate, "o processo nao parou na passada");
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(filho);
    assert_eq!(
        marcas(&caido.0.join("dados").join("loja")).len(),
        1,
        "premissa: a marca da venda em voo tinha de estar no disco caido"
    );

    // A copia fria, direto pelo store -- o caminho da CLI.
    let copias = caido.0.join("copias");
    let (zip, _) = phxsql_store::backup::executar_zip(
        &caido.0.join("dados"),
        &copias,
        "loja",
        "teste",
        phxsql_server::agora_ms(),
    )
    .unwrap();
    phxsql_store::backup::finalizar_zip(&zip).unwrap();
    let zip = zips(&copias).pop().expect("a copia nao gerou .zip");

    let destino = DirTemp::novo("viaja-destino");
    let (filho, porta) = subir(&destino.0, 1, None);
    Ligacao::nova(porta).exigir(&format!(
        r#""op":"restaurar_backup","origem":"{}","database":"loja""#,
        zip.display()
    ));
    let retrato = (
        contar(porta, "loja", "vendas"),
        contar(porta, "loja", "itens"),
        contar(porta, "loja", "pagamentos"),
    );
    let ficou = marcas(&destino.0.join("dados").join("loja"));
    drop(filho);
    assert_eq!(
        retrato,
        (1, ITENS as usize, 1),
        "a restauracao entregou a venda pela metade: a marca nao se completou no palco"
    );
    assert!(
        ficou.is_empty(),
        "a marca entrou na raiz do destino: {ficou:?}"
    );
}

/// **F8a.** A marca do `COMMIT` que so esperava a janela vai no backup e nao
/// pode chegar ao destino. Vermelho medido antes do conserto: a marca no
/// destino logo depois da restauracao.
#[test]
fn a_marca_pendente_do_backup_nao_entra_no_destino() {
    let base = DirTemp::novo("viaja-pendente");
    let (filho, porta) = subir(&base.0, 1, None);
    criar_as_tabelas(porta);
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"id":1,"venda":"0"}"#);
    b.exigir(r#""op":"begin","database":"loja""#);
    b.exigir(
        r#""op":"atualizar","database":"loja","tabela":"vendas","rowid":1,
           "valores":{"id":1,"venda":"a"}"#,
    );
    b.exigir(r#""op":"commit""#);
    assert_eq!(
        marcas(&base.0.join("dados").join("loja")).len(),
        1,
        "premissa: a marca do COMMIT tinha de ficar pendente"
    );
    let copias = base.0.join("copias");
    b.exigir(&format!(
        r#""op":"backup","destino":"{}","database":"loja","zip":true"#,
        copias.display()
    ));
    let zip = zips(&copias).pop().expect("o backup nao gerou .zip");
    b.exigir(&format!(
        r#""op":"restaurar_backup","origem":"{}","database":"loja2""#,
        zip.display()
    ));
    let ficou = marcas(&base.0.join("dados").join("loja2"));
    let r = b.exigir(r#""op":"ler","database":"loja2","tabela":"vendas","rowid":1"#);
    drop(b);
    drop(filho);
    assert!(
        ficou.is_empty(),
        "a marca do COMMIT entrou na raiz com o database restaurado: {ficou:?}"
    );
    assert_eq!(
        r.campo("resultado").map(|l| l.texto_ou("venda", "")),
        Some("a"),
        "{}",
        r.escrever()
    );
}
