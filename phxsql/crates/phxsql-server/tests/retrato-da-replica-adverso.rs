//! O retrato da replica (pedido 706) contra quem o usa por fora -- pedidos
//! 726, 727, 729 e 731, da revisao SEC de 08/10/2026
//! (`docs/propostas/sec-revisao-20261008.md`, A1, A2, M2 e M4). Tudo PELO
//! SOQUETE: o que se quer saber e o que outra conexao consegue ler, apagar ou
//! travar, e isso nao aparece numa chamada de funcao.
//!
//! - **726:** a origem falsa manda `tabela` com caminho absoluto (ou `..` no
//!   schema) e a replica nao pode escrever nem apagar fora do database.
//! - **727:** a sessao B, com `replicar` so em `loja` e fio em claro, nao le o
//!   pedaco do retrato de `rh` que a sessao A tirou, nem o solta.
//! - **729:** duas replicas tiram retrato ao mesmo tempo e as duas terminam; e
//!   ha teto para o laco de pedidos.
//! - **731:** a conexao que cai leva o retrato junto, o arranque apaga o que
//!   uma vida anterior deixou, e o backup nao o copia.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::{Column, ColumnType, DadoPessoal, IndexColumn, IndexDef, Schema, Value};
use phxsql_server::replica::Cliente;
use phxsql_server::{Config, Origem, Papel, Servidor};
use phxsql_store::Table;

const TOKEN: &str = "retrato-adverso";
const SENHA: &str = "senha-do-teste-do-retrato";
/// O valor marcado de `rh.fichas`: e ele que nao pode sair por B.
const SEGREDO: &str = "cpf-do-rh-727";

struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
    com_login: bool,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let c = if self.com_login {
            entrar(self.porta, "root", false)
        } else {
            Cliente::conectar("127.0.0.1", self.porta, TOKEN, Duration::from_secs(5))
        };
        if let Ok(mut c) = c {
            let _ = c.pedir(vec![("op", Json::texto_de("servico_parar"))]);
        }
    }
}

fn config(base: &Path, id: &str, papel: Papel) -> Config {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = papel;
    c.replicacao.id_servidor = id.into();
    c.replicacao.imagem_da_linha = true;
    c
}

fn subir(c: Config) -> NoAr {
    let com_login = !c.cadastro.usuarios.is_empty();
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr {
        _s: s,
        porta,
        com_login,
    }
}

/// O cadastro da origem: `root` (supervisor) e `bia`, com `replicar` SO em
/// `loja` -- a sessao B do achado A2.
fn cadastro() -> phxsql_server::usuarios::Cadastro {
    let h = phxsql_core::senha::cifrar_com(SENHA, 1);
    let texto = format!(
        r#"{{ "usuarios": [
              {{ "login": "root", "senha_hash": "{h}", "supervisor": true }},
              {{ "login": "bia", "senha_hash": "{h}",
                 "bases": {{ "loja": {{ "replicar": true }} }} }} ] }}"#
    );
    phxsql_server::usuarios::Cadastro::de_json(&Json::analisar(&texto).unwrap()).unwrap()
}

/// `rh.fichas` com coluna marcada e `loja.vendas` sem, gravadas antes de o
/// servidor subir.
fn encher(base: &Path) {
    let rh = base.join("rh");
    std::fs::create_dir_all(&rh).unwrap();
    let nome = Column::new("nome", ColumnType::Str(40))
        .obrigatoria()
        .com_dado_pessoal(DadoPessoal::Pessoal);
    let esquema = Schema::new(
        "fichas",
        vec![Column::new("id", ColumnType::Int8).obrigatoria(), nome],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    let mut t = Table::criar(&rh, esquema).unwrap();
    for i in 1..=3 {
        t.inserir(&[Value::Int(i), Value::Str(format!("{SEGREDO}-{i}"))])
            .unwrap();
    }
    t.sincronizar().unwrap();

    let loja = base.join("loja");
    std::fs::create_dir_all(&loja).unwrap();
    let esquema = Schema::new(
        "vendas",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("total", ColumnType::Int8),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    let mut t = Table::criar(&loja, esquema).unwrap();
    for i in 1..=3 {
        t.inserir(&[Value::Int(i), Value::Int(i * 10)]).unwrap();
    }
    t.sincronizar().unwrap();
}

fn origem_com_cadastro(base: &Path) -> NoAr {
    encher(base);
    let mut c = config(base, "origem", Papel::Source);
    c.cadastro = cadastro();
    subir(c)
}

/// Uma conexao autenticada; `cifrada` pede o tunel antes do login.
fn entrar(porta: u16, login: &str, cifrada: bool) -> phxsql_core::error::Result<Cliente> {
    let mut c = Cliente::conectar("127.0.0.1", porta, TOKEN, Duration::from_secs(10))?;
    if cifrada {
        c.cifrar(None)?;
    }
    c.autenticar(login, "", SENHA)?;
    Ok(c)
}

fn tirar(c: &mut Cliente, database: &str) -> phxsql_core::error::Result<Json> {
    c.pedir(vec![
        ("op", Json::texto_de("retrato_da_replica")),
        ("database", Json::texto_de(database)),
    ])
}

fn pedaco(
    c: &mut Cliente,
    database: &str,
    id: i64,
    indice: u64,
) -> phxsql_core::error::Result<Json> {
    c.pedir(vec![
        ("op", Json::texto_de("retrato_da_replica")),
        ("database", Json::texto_de(database)),
        ("id", Json::Numero(id as f64)),
        ("indice", Json::de_u64(indice)),
        ("offset", Json::de_u64(0)),
    ])
}

fn soltar(c: &mut Cliente, database: &str, id: i64) -> phxsql_core::error::Result<Json> {
    c.pedir(vec![
        ("op", Json::texto_de("retrato_da_replica")),
        ("database", Json::texto_de(database)),
        ("id", Json::Numero(id as f64)),
        ("soltar", Json::Bool(true)),
    ])
}

/// O indice do `.reg` na lista do retrato.
fn indice_do_reg(r: &Json) -> u64 {
    r.campo("arquivos")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .position(|a| a.texto_ou("arquivo", "").ends_with(".reg"))
        .expect("o retrato nao trouxe .reg") as u64
}

fn retratos_na_raiz(base: &Path) -> Vec<String> {
    std::fs::read_dir(base)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with(".retrato-"))
        .collect()
}

fn esperar(o_que: &str, mut ok: impl FnMut() -> bool) {
    let ate = Instant::now() + Duration::from_secs(20);
    while !ok() {
        assert!(Instant::now() < ate, "nao aconteceu em 20 s: {o_que}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

// ------------------------------------------------------------------- 727

/// **727 (A2):** A tira o retrato de `rh` pelo tunel; B, com `replicar` so em
/// `loja` e em claro, pede o pedaco do `.reg` de `rh` com o id de A (pondo
/// `loja` no pedido, que e o que o portao confere) e manda `soltar`.
///
/// Defeito reposto (o retrato sem amarracao): B recebe os bytes crus de `rh`
/// com o valor marcado, em claro, e o `soltar` dele apaga o retrato de A --
/// o pedaco seguinte de A cai.
#[test]
fn o_retrato_de_uma_sessao_nao_se_le_nem_se_solta_por_outra() {
    let d = DirTemp::novo("retrato-727");
    let origem = origem_com_cadastro(&d);
    let mut a = entrar(origem.porta, "root", true).unwrap();
    let r = tirar(&mut a, "rh").expect("A, supervisor e cifrada, nao tirou o retrato de rh");
    let id = r.inteiro_ou("id", 0);
    let reg = indice_do_reg(&r);
    assert!(id > 0);

    let mut b = entrar(origem.porta, "bia", false).unwrap();
    let lido = pedaco(&mut b, "loja", id, reg);
    let vazou = lido
        .as_ref()
        .map(|j| j.texto_ou("bytes", "").to_string())
        .unwrap_or_default();
    let segredo_hex: String = SEGREDO.bytes().map(|b| format!("{b:02x}")).collect();
    assert!(
        !vazou.contains(&segredo_hex),
        "B leu o .reg de rh, com o dado marcado, em claro, pelo id de A"
    );
    assert!(lido.is_err(), "o pedaco alheio nao foi recusado: {lido:?}");

    let solto = soltar(&mut b, "loja", id);
    assert!(
        solto.is_err(),
        "o soltar alheio nao foi recusado: {solto:?}"
    );
    let ainda = pedaco(&mut a, "rh", id, reg);
    assert!(
        ainda.is_ok(),
        "o soltar de B apagou o retrato de A: {:?}",
        ainda.err()
    );
}

/// **727, o irmao:** o dono do retrato le e solta o proprio -- o portao novo
/// nao recusa tudo. E o id deixou de ser o relogio: dois retratos seguidos
/// nao sao vizinhos.
#[test]
fn o_dono_le_e_solta_o_proprio_retrato_e_o_id_nao_e_o_relogio() {
    let d = DirTemp::novo("retrato-727-dono");
    let origem = origem_com_cadastro(&d);
    let mut a = entrar(origem.porta, "bia", false).unwrap();
    let r = tirar(&mut a, "loja").unwrap();
    let id = r.inteiro_ou("id", 0);
    let reg = indice_do_reg(&r);
    let p = pedaco(&mut a, "loja", id, reg).expect("o dono nao leu o proprio pedaco");
    assert!(!p.texto_ou("bytes", "").is_empty());
    soltar(&mut a, "loja", id).expect("o dono nao soltou o proprio retrato");
    assert!(
        retratos_na_raiz(&d).is_empty(),
        "{:?}",
        retratos_na_raiz(&d)
    );
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    assert!(
        (id - agora).abs() > 86_400_000,
        "o id {id} e o relogio ({agora}): adivinha-se varrendo a janela"
    );
}

// ------------------------------------------------------------------- 729

/// **729 (M2):** duas replicas atrasadas tiram o retrato do mesmo database ao
/// mesmo tempo -- e as duas leem ate o fim.
///
/// Defeito reposto (o slot global): o retrato de B apaga o de A, e o pedaco
/// de A cai em «nao ha retrato».
#[test]
fn duas_replicas_atrasadas_terminam_as_duas() {
    let d = DirTemp::novo("retrato-729-duas");
    let origem = origem_com_cadastro(&d);
    let mut a = entrar(origem.porta, "root", true).unwrap();
    let mut b = entrar(origem.porta, "root", true).unwrap();
    let ra = tirar(&mut a, "loja").unwrap();
    let rb = tirar(&mut b, "loja").unwrap();
    let (ia, ib) = (ra.inteiro_ou("id", 0), rb.inteiro_ou("id", 0));
    pedaco(&mut a, "loja", ia, indice_do_reg(&ra)).expect("o retrato de B apagou o de A");
    pedaco(&mut b, "loja", ib, indice_do_reg(&rb)).expect("B nao leu o proprio");
}

/// **729, o laco:** cada conexao segura um retrato; a quinta ao mesmo tempo
/// ouve o teto, e nao poe uma quinta copia do database na raiz.
///
/// Defeito reposto (sem teto): as cinco tiram, e cada uma e uma copia inteira
/// feita com a trava global na mao.
#[test]
fn o_laco_de_retratos_tem_teto() {
    let d = DirTemp::novo("retrato-729-teto");
    let origem = origem_com_cadastro(&d);
    let mut conexoes = Vec::new();
    let mut aceitos = 0;
    let mut recusa = String::new();
    for _ in 0..5 {
        let mut c = entrar(origem.porta, "root", true).unwrap();
        match tirar(&mut c, "loja") {
            Ok(_) => aceitos += 1,
            Err(e) => recusa = e.to_string(),
        }
        conexoes.push(c);
    }
    assert_eq!(
        aceitos, 4,
        "o teto nao segurou: {aceitos} retratos ao mesmo tempo"
    );
    assert!(recusa.contains("teto"), "a recusa nao diz o teto: {recusa}");
    // A mesma conexao pedindo de novo troca o dela, e nao conta duas vezes.
    tirar(&mut conexoes[0], "loja").expect("a conexao nao trocou o proprio retrato");
}

// ------------------------------------------------------------------- 731

/// **731 (M4):** a replica que morre no meio, sem `soltar`, nao deixa a copia
/// na raiz.
///
/// Defeito reposto (sem limpeza ao cair): a copia de `rh`, com o valor
/// marcado, fica na raiz ate o proximo retrato -- que pode nunca vir.
#[test]
fn a_conexao_que_cai_leva_o_retrato_junto() {
    let d = DirTemp::novo("retrato-731-queda");
    let origem = origem_com_cadastro(&d);
    let mut a = entrar(origem.porta, "root", true).unwrap();
    tirar(&mut a, "rh").unwrap();
    assert!(
        !retratos_na_raiz(&d).is_empty(),
        "o retrato nem foi para a raiz"
    );
    drop(a);
    esperar(
        "a copia do retrato sair da raiz com a conexao caida",
        || retratos_na_raiz(&d).is_empty(),
    );
}

/// **731, o arranque:** a copia que uma vida anterior deixou (o processo
/// morreu com o retrato no ar) sai antes de a porta abrir.
#[test]
fn o_arranque_apaga_o_retrato_de_uma_vida_anterior() {
    let d = DirTemp::novo("retrato-731-arranque");
    std::fs::write(d.join(".retrato-servido-123-0-0"), SEGREDO).unwrap();
    std::fs::write(d.join(".retrato-recebido-0"), SEGREDO).unwrap();
    let _origem = origem_com_cadastro(&d);
    assert!(
        retratos_na_raiz(&d).is_empty(),
        "sobrou: {:?}",
        retratos_na_raiz(&d)
    );
}

/// **731, o backup:** medido antes do conserto -- o `listar` do backup
/// percorre a raiz inteira, e a copia do retrato ia para o backup junto.
#[test]
fn o_backup_nao_leva_o_retrato() {
    let d = DirTemp::novo("retrato-731-backup");
    let destino = DirTemp::novo("retrato-731-backup-destino");
    let origem = origem_com_cadastro(&d);
    let mut a = entrar(origem.porta, "root", true).unwrap();
    tirar(&mut a, "rh").unwrap();
    assert!(!retratos_na_raiz(&d).is_empty());
    let alvo = destino.join("copia");
    a.pedir(vec![
        ("op", Json::texto_de("backup")),
        ("destino", Json::texto_de(alvo.display().to_string())),
    ])
    .expect("o backup recusou");
    let mut achados = Vec::new();
    let mut pilha = vec![alvo.clone()];
    while let Some(p) = pilha.pop() {
        for e in std::fs::read_dir(&p).unwrap().flatten() {
            if e.path().is_dir() {
                pilha.push(e.path());
            } else if e.file_name().to_string_lossy().starts_with(".retrato-") {
                achados.push(e.path());
            }
        }
    }
    assert!(achados.is_empty(), "o backup levou o retrato: {achados:?}");
}

// ------------------------------------------------------------------- 726

/// A origem FALSA do achado A1: anuncia `base` acima da replica (forca o
/// retrato) e responde o retrato com a `tabela` que o teste escolher.
struct OrigemFalsa {
    porta: u16,
    soltou: Arc<AtomicBool>,
}

fn origem_falsa(tabela: &'static str, conteudo: &'static [u8]) -> OrigemFalsa {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let soltou = Arc::new(AtomicBool::new(false));
    let s = Arc::clone(&soltou);
    std::thread::spawn(move || {
        for c in ouvinte.incoming() {
            let Ok(c) = c else { return };
            let s = Arc::clone(&s);
            std::thread::spawn(move || {
                let mut escrita = c.try_clone().unwrap();
                for linha in BufReader::new(c).lines() {
                    let Ok(linha) = linha else { return };
                    let Ok(p) = Json::analisar(&linha) else {
                        return;
                    };
                    let hex: String = conteudo.iter().map(|b| format!("{b:02x}")).collect();
                    let resultado = match p.texto_ou("op", "") {
                        "posicao" => r#"{"tabelas":{"produtos":{"eventos":9,"base":5}},
                                 "imagem_da_linha":true,"tx_no_diario":true,
                                 "id_servidor":"falsa"}"#
                            .to_string(),
                        "retrato_da_replica" if p.booleano_ou("soltar", false) => {
                            s.store(true, Ordering::SeqCst);
                            r#"{"soltou":true}"#.to_string()
                        }
                        "retrato_da_replica" if p.campo("indice").is_some() => {
                            let offset = p.inteiro_ou("offset", 0);
                            let bytes = if offset == 0 { hex.as_str() } else { "" };
                            format!(r#"{{"id":7,"offset":{offset},"bytes":"{bytes}"}}"#)
                        }
                        "retrato_da_replica" => format!(
                            r#"{{"database":"loja","id":7,"eventos":{{}},
                                 "arquivos":[{{"tabela":{tabela:?},"arquivo":"produtos.reg",
                                               "bytes":{}}}]}}"#,
                            conteudo.len()
                        ),
                        _ => "{}".to_string(),
                    };
                    let resposta =
                        format!(r#"{{"ok":true,"resultado":{resultado}}}"#).replace('\n', " ");
                    if writeln!(escrita, "{resposta}").is_err() {
                        return;
                    }
                }
            });
        }
    });
    OrigemFalsa { porta, soltou }
}

fn replica_da_falsa(base: &Path, porta: u16) -> NoAr {
    let mut c = config(base, "replica", Papel::Replica);
    c.somente_leitura = true;
    c.replicacao.origens = vec![Origem {
        nome: "falsa".into(),
        host: "127.0.0.1".into(),
        porta,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
        pino_tls: String::new(),
        espelho: false,
    }];
    subir(c)
}

/// Espera a replica soltar o retrato e da a troca o tempo de acontecer;
/// devolve o conteudo do arquivo vigiado.
fn depois_da_troca(falsa: &OrigemFalsa, vigiado: &PathBuf) -> Vec<u8> {
    esperar("a replica pedir o retrato e solta-lo", || {
        falsa.soltou.load(Ordering::SeqCst)
    });
    std::thread::sleep(Duration::from_millis(1_500));
    std::fs::read(vigiado).unwrap_or_default()
}

/// **726 (A1):** a origem falsa manda `tabela` com caminho ABSOLUTO para o
/// database `vizinho` desta mesma replica. `Path::join` com caminho absoluto
/// substitui a base, e o `produtos.reg` do vizinho seria apagado e trocado.
///
/// A variante `..` do achado nao se alcanca: o `separar_qualificado` corta no
/// PRIMEIRO ponto, entao o schema nunca e `..` (o `"...produtos"` vira schema
/// nenhum), e o `..` com barra cai na tabela, que ja passava pelo
/// `validar_nome`. O caminho absoluto e a unica fuga -- e e o que se prova.
///
/// Defeito reposto (o schema sem `validar_nome`): o arquivo do vizinho vira o
/// conteudo da origem falsa.
#[test]
fn o_retrato_com_caminho_absoluto_nao_escreve_fora_do_database() {
    let d = DirTemp::novo("retrato-726-absoluto");
    let vizinho = d.join("vizinho");
    // `Box::leak`: a origem falsa vive numa thread solta, e o caminho dela e
    // o deste teste.
    let alvo: &'static str = Box::leak(format!("{}.produtos", vizinho.display()).into_boxed_str());
    assert!(
        !vizinho.display().to_string().contains('.'),
        "o caminho temporario tem ponto, e o teste nao separaria schema e tabela onde quer"
    );
    let falsa = origem_falsa(alvo, b"ATACANTE");
    let _replica = replica_da_falsa(&d, falsa.porta);
    std::fs::create_dir_all(&vizinho).unwrap();
    let vigiado = vizinho.join("produtos.reg");
    std::fs::write(&vigiado, b"ORIGINAL").unwrap();
    let depois = depois_da_troca(&falsa, &vigiado);
    assert_eq!(
        depois, b"ORIGINAL",
        "o retrato da origem falsa reescreveu o database vizinho"
    );
}
