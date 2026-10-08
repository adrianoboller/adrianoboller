//! Pedidos 709 (F1) e 711 (F3): a marca do `COMMIT` que so esperava o fecho
//! da janela de durabilidade nao pode, no arranque, regravar por cima de
//! escrita posterior -- nem acrescentar ao diario eventos que nenhum cliente
//! fez. Provado contra o sistema operacional, com `SIGKILL`.
//!
//! # O defeito
//!
//! O `COMMIT` grava a marca, faz a passada e, com a janela aberta, deixa a
//! marca nas pendentes ate o `fsync` do lote. Uma escrita SOLTA na mesma
//! linha, depois do `COMMIT`, nao tem bilhete. O `SIGKILL` antes do fecho da
//! janela deixa o dado das duas no cache do nucleo -- ele sobrevive a morte do
//! processo -- e a marca no disco. O arranque reaplicava a marca sem condicao
//! e o valor do `COMMIT` VOLTAVA por cima do da solta, com um evento novo de
//! carimbo do arranque (no bidirecional, esse evento ganha o «mais recente
//! vence» do par).
//!
//! # Por que nao ha gancho
//!
//! A morte e ENTRE pedidos: a resposta da solta ja chegou, e a trava esta
//! solta. O `kill` de fora basta (`docs/propostas/e0-provas-dos-furos.md` §0.3).
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use phxsql_core::json::Json;
use phxsql_core::value::Value;
use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "recuperacao-nao-volta";

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn pedir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        Json::analisar(&r).unwrap()
    }

    fn exigir(&mut self, corpo: &str) -> Json {
        let r = self.pedir(corpo);
        assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
        r
    }
}

/// Sobe o `phxsqld` com a janela PARADA: sem ela a marca sairia no fecho do
/// lote antes do `kill`, e nao haveria o que reaplicar (E0 §0.4).
fn subir(dir: &Path, vez: u32) -> (Filho, u16) {
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
        .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()));
    for gancho in [
        "PHXSQL_TESTE_PARAR_NO_COMMIT",
        "PHXSQL_TESTE_PARAR_NO_REG",
        "PHXSQL_TESTE_PARAR_NO_REG_DO_COMMIT",
    ] {
        cmd.env_remove(gancho);
    }
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta)
}

fn criar(porta: u16) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"criar_database","database":"loja""#);
    b.exigir(
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"nome","tipo":"Str(20)"}],
           "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
    );
}

fn alterar(id: u64, rowid: u64, nome: &str) -> String {
    format!(
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":{rowid},
           "valores":{{"id":{id},"nome":"{nome}"}}"#
    )
}

fn inserir(id: u64, nome: &str) -> String {
    format!(
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id},"nome":"{nome}"}}"#
    )
}

fn excluir(rowid: u64) -> String {
    format!(
        r#""op":"excluir","database":"loja","tabela":"clientes","rowid":{rowid},"motivo":"teste""#
    )
}

fn restaurar(rowid: u64) -> String {
    format!(
        r#""op":"restaurar","database":"loja","tabela":"clientes","rowid":{rowid},"motivo":"teste""#
    )
}

/// Um `COMMIT` com as escritas `corpos`, numa ligacao propria.
fn commit(porta: u16, corpos: &[String]) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"begin","database":"loja""#);
    for c in corpos {
        b.exigir(c);
    }
    let r = b.exigir(r#""op":"commit""#);
    assert_eq!(
        r.campo("resultado")
            .map(|x| x.texto_ou("transaction_state", ""))
            .unwrap_or_default(),
        "COMMITTED",
        "{}",
        r.escrever()
    );
}

fn marcas(dados: &Path) -> Vec<PathBuf> {
    let Ok(ls) = std::fs::read_dir(dados.join("loja")) else {
        return Vec::new();
    };
    ls.filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("transacao_") && n.ends_with(".tx"))
        })
        .collect()
}

/// A linha `rowid` de `clientes`, lida do disco com o servidor parado:
/// `(nome, excluida_suave, versao, eventos_no_diario, maior_carimbo_do_rowid)`.
fn retrato(dados: &Path, rowid: u64) -> (String, bool, u64, u64, i64) {
    let inst = Instancia::nova(dados).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    let mut t = db.abrir_qualificada("clientes").unwrap();
    let linha = t.ler(rowid).unwrap().expect("a linha sumiu do .reg");
    let nome = match &linha[1] {
        Value::Str(s) => s.clone(),
        outro => panic!("nome inesperado: {outro:?}"),
    };
    let i = t
        .esquema()
        .coluna_softdeleted()
        .expect("sem exclusao suave");
    let suave = matches!(linha[i], Value::Bool(true));
    let versao = t.versao(rowid).unwrap().unwrap_or(0);
    let total = t.eventos().unwrap();
    let maior = t
        .diario(0, total)
        .unwrap()
        .iter()
        .filter(|e| e.rowid == rowid)
        .map(|e| e.carimbo)
        .max()
        .unwrap_or(0);
    (nome, suave, versao, total, maior)
}

fn agora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// O roteiro comum: `antes` sem transacao, o `COMMIT`, a solta `depois`, e o
/// `SIGKILL` com a janela aberta. Devolve o retrato de antes da morte, o de
/// depois do arranque e o instante da morte.
type Retrato = (String, bool, u64, u64, i64);

fn roteiro(
    rotulo: &str,
    antes: &[String],
    no_commit: &[String],
    depois: &[String],
    rowid: u64,
) -> (Retrato, Retrato, i64, DirTemp) {
    let base = DirTemp::novo(rotulo);
    let dados = base.0.join("dados");
    let (filho, porta) = subir(&base.0, 1);
    criar(porta);
    let mut b = Ligacao::nova(porta);
    for c in antes {
        b.exigir(c);
    }
    if !no_commit.is_empty() {
        commit(porta, no_commit);
        // Premissa (E0 §0.5): sem a marca pendente nao ha o que reaplicar, e
        // a prova passaria por engano.
        assert_eq!(
            marcas(&dados).len(),
            1,
            "a marca do COMMIT nao ficou pendente: a janela nao estava parada"
        );
    }
    for c in depois {
        b.exigir(c);
    }
    if !no_commit.is_empty() {
        assert_eq!(marcas(&dados).len(), 1, "a solta drenou a marca do COMMIT");
    }
    drop(b);
    let morte = agora_ms();
    drop(filho);
    let antes_da_volta = retrato(&dados, rowid);
    let (filho, _) = subir(&base.0, 2);
    drop(filho);
    let depois_da_volta = retrato(&dados, rowid);
    (antes_da_volta, depois_da_volta, morte, base)
}

/// **F1, o caso principal (pedido 709).** Vermelho medido antes do conserto:
/// a linha volta a «a», com um evento de carimbo do arranque.
#[test]
fn a_marca_pendente_nao_volta_o_valor_por_cima_da_solta() {
    let (antes, depois, morte, _base) = roteiro(
        "f1-alt",
        &[inserir(1, "0")],
        &[alterar(1, 1, "a")],
        &[alterar(1, 1, "b")],
        1,
    );
    assert_eq!(antes.0, "b", "premissa: a solta tinha gravado «b»");
    assert_eq!(
        depois.0, "b",
        "o arranque reaplicou o COMMIT por cima da escrita posterior: a linha voltou a {:?}",
        depois.0
    );
    assert_eq!(
        depois.4, antes.4,
        "o arranque gravou um evento novo na linha (carimbo {}, morte em {morte})",
        depois.4
    );
    assert_eq!(depois.3, antes.3, "o diario ganhou eventos no arranque");
    assert_eq!(depois.2, antes.2, "a versao do slot andou no arranque");
}

/// **F1-sup.** O `COMMIT` exclui (suave); a solta restaura. A linha nao pode
/// sumir de novo no arranque.
#[test]
fn a_exclusao_suave_do_commit_nao_some_de_novo_com_a_linha_restaurada() {
    let (antes, depois, morte, _base) = roteiro(
        "f1-sup",
        &[inserir(1, "0")],
        &[excluir(1)],
        &[restaurar(1)],
        1,
    );
    assert!(!antes.1, "premissa: a solta tinha restaurado a linha");
    assert!(
        !depois.1,
        "o arranque reaplicou a exclusao suave por cima da restauracao: a linha sumiu"
    );
    assert_eq!(
        depois.4, antes.4,
        "o arranque gravou um evento novo na linha (morte em {morte})"
    );
    assert_eq!(depois.3, antes.3, "o diario ganhou eventos no arranque");
}

/// **F1-rest.** O `COMMIT` restaura; a solta exclui (suave). A linha nao pode
/// voltar no arranque.
#[test]
fn a_restauracao_do_commit_nao_volta_a_linha_excluida_depois() {
    let (antes, depois, morte, _base) = roteiro(
        "f1-rest",
        &[inserir(1, "0"), excluir(1)],
        &[restaurar(1)],
        &[excluir(1)],
        1,
    );
    assert!(antes.1, "premissa: a solta tinha excluido a linha");
    assert!(
        depois.1,
        "o arranque reaplicou a restauracao por cima da exclusao: a linha voltou"
    );
    assert_eq!(
        depois.4, antes.4,
        "o arranque gravou um evento novo na linha (morte em {morte})"
    );
    assert_eq!(depois.3, antes.3, "o diario ganhou eventos no arranque");
}

/// **F3 (pedido 711).** Sem escrita posterior: completar a marca cuja passada
/// terminou grava zero bytes e zero eventos.
#[test]
fn completar_a_marca_de_passada_terminada_nao_acrescenta_eventos() {
    let (antes, depois, morte, _base) =
        roteiro("f3", &[inserir(1, "0")], &[alterar(1, 1, "a")], &[], 1);
    assert_eq!(antes.0, "a");
    assert_eq!(depois.0, "a");
    assert_eq!(
        depois.3,
        antes.3,
        "a recuperacao acrescentou {} evento(s) que nenhum cliente fez",
        depois.3 - antes.3
    );
    assert_eq!(depois.2, antes.2, "a versao do slot andou no arranque");
    assert_eq!(
        depois.4, antes.4,
        "o arranque gravou um evento novo na linha (morte em {morte})"
    );
}

/// **C0.** Sem `COMMIT`: o `SIGKILL` preserva a solta. Sem este verde, o
/// vermelho do F1 podia ser escrita perdida e nao reaplicacao.
#[test]
fn controle_o_sigkill_preserva_a_solta() {
    let (antes, depois, _morte, _base) = roteiro(
        "c0",
        &[inserir(1, "0"), alterar(1, 1, "a"), alterar(1, 1, "b")],
        &[],
        &[],
        1,
    );
    assert_eq!(antes.0, "b");
    assert_eq!(depois.0, "b");
    assert_eq!(depois.3, antes.3);
}

/// **C1.** O `COMMIT` INSERE e a solta altera: a inclusao ja era protegida
/// pelo slot consumido, e continua.
#[test]
fn controle_a_inclusao_do_commit_nao_volta() {
    let (antes, depois, morte, _base) = roteiro(
        "c1",
        &[inserir(1, "0")],
        &[inserir(2, "a")],
        &[alterar(2, 2, "b")],
        2,
    );
    assert_eq!(antes.0, "b");
    assert_eq!(depois.0, "b");
    assert_eq!(depois.3, antes.3);
    assert_eq!(
        depois.4, antes.4,
        "evento novo no arranque (morte em {morte})"
    );
}
