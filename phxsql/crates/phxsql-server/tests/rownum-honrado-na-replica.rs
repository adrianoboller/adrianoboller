//! A replica fiel HONRA o `rownum` que vem na imagem -- pedido 309, a via (b)
//! do 291, decidida pelo papel J em 01/10/2026 (convergencia dos quatro: PG
//! logico, MySQL/MariaDB em RBR e a changeset do SQLite aplicam o valor que
//! veio, sem gerar outro).
//!
//! # O que se mede, e por que pelo soquete
//!
//! A via (a) (pedido 291) parou de abrir buraco NOVO no `rownum` do source. O
//! buraco HISTORICO -- gravado antes dela -- continuava la, e a replica, que
//! numerava pela ordem de chegada DELA, o fechava: o source dizia `1,2,4` e a
//! replica `1,2,3`, para sempre. O PITR reaplica o diario pelo mesmo
//! `aplicar_evento` e renumerava igual. So os dois processos conversando
//! mostram o que a replica de verdade grava.
//!
//! O buraco se fabrica como um source anterior ao `eeb9925` o deixaria: com o
//! servidor parado, o contador do `rownum` no cabecalho do `.reg` (bytes
//! 92..100, CRC refeito) pula um numero. A proxima insercao sai com 4.
//!
//! E o irmao que trava o comportamento velho: no bidirecional o `rownum` e
//! LOCAL por desenho (cada servidor tem a sua ordem de digitacao), e quem
//! puxa de um par com buraco continua numerando `1,2,3`.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "rownum-honrado";
const ESPERA: Duration = Duration::from_secs(20);

/// Um servidor no ar, parado no fim -- inclusive quando o teste cai no meio.
struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let _ = tentar(self.porta, r#""op":"servico_parar""#);
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
    // O escape escrito dos outros testes de replicacao: o que esta bateria
    // mede e o `rownum`, nao o portao da cifra do fio.
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = papel;
    c.replicacao.id_servidor = id.into();
    c.replicacao.imagem_da_linha = true;
    c
}

fn origem(porta: u16) -> Origem {
    Origem {
        nome: "fonte".into(),
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
    }
}

fn subir(mut c: Config) -> NoAr {
    let (ouvinte, porta) = comum::ouvinte_reservado();
    c.bind = format!("127.0.0.1:{porta}");
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr { _s: s, porta }
}

fn tentar(porta: u16, corpo: &str) -> Option<Json> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().ok()?;
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .ok()?;
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).ok()?;
    Json::analisar(&resposta).ok()
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = tentar(porta, corpo).unwrap_or_else(|| panic!("{corpo}: sem resposta de {porta}"));
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

fn esperar<F: FnMut() -> bool>(o_que: &str, mut f: F) {
    let ate = Instant::now() + ESPERA;
    while Instant::now() < ate {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{o_que} nao aconteceu em {} s", ESPERA.as_secs());
}

/// As linhas de `database.clientes`, na ordem do rowid. Vazia enquanto a
/// tabela nao existe deste lado.
fn linhas(porta: u16, database: &str) -> Vec<Json> {
    tentar(
        porta,
        &format!(r#""op":"varrer","database":"{database}","tabela":"clientes","max":100"#),
    )
    .and_then(|r| r.campo("resultado").cloned())
    .and_then(|r| {
        r.campo("linhas")
            .and_then(Json::lista)
            .map(<[Json]>::to_vec)
    })
    .unwrap_or_default()
}

fn rownums(porta: u16, database: &str) -> Vec<i64> {
    linhas(porta, database)
        .iter()
        .map(|l| l.inteiro_ou("rownum", -1))
        .collect()
}

fn criar_clientes(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"nome","tipo":"Str(20)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
}

fn inserir(porta: u16, id: i64) {
    exigir(
        porta,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id},"nome":"c{id}"}}"#
        ),
    );
}

/// O buraco HISTORICO, como um source anterior a via (a) o deixaria: o
/// contador do `rownum` no cabecalho do volume 1 pula para `proximo`, com o
/// CRC do cabecalho refeito. So com o servidor parado.
fn fabricar_buraco(base: &Path, proximo: u64) {
    let caminho = base.join("loja").join("clientes.reg");
    let mut bytes =
        std::fs::read(&caminho).unwrap_or_else(|e| panic!("nao li {}: {e}", caminho.display()));
    let cab_len = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    let atual = u64::from_le_bytes(bytes[92..100].try_into().unwrap());
    assert!(
        proximo > atual,
        "o buraco tem de pular para a frente ({atual})"
    );
    bytes[92..100].copy_from_slice(&proximo.to_le_bytes());
    let crc = phxsql_core::crc::crc32(&bytes[..cab_len - 4]);
    bytes[cab_len - 4..cab_len].copy_from_slice(&crc.to_le_bytes());
    std::fs::write(&caminho, &bytes).unwrap();
}

/// O source com `1,2,4`: duas linhas, o buraco, e a terceira. Devolve o
/// servidor no ar, ja reaberto sobre o `.reg` com o buraco.
fn source_com_buraco(base: &Path, papel: Papel) -> NoAr {
    {
        let s = subir(config(base, "fonte-com-buraco", papel));
        criar_clientes(s.porta);
        inserir(s.porta, 1);
        inserir(s.porta, 2);
    }
    fabricar_buraco(base, 4);
    let s = subir(config(base, "fonte-com-buraco", papel));
    inserir(s.porta, 3);
    assert_eq!(
        rownums(s.porta, "loja"),
        vec![1, 2, 4],
        "a premissa caiu: o source nao ficou com o buraco"
    );
    s
}

/// **A prova real do 309, replica fiel.** O source tem `1,2,4`; a replica
/// tem de ter `1,2,4` -- e a linha inteira igual, que e o que o retrato
/// SHA-256 de cada linha compara.
///
/// **Medido** (01/10/2026) com o defeito reposto -- `honrar_rownum` nunca
/// ligado no `reaplicar_evento_do_proprio_diario`: a replica terminava com
/// `[1, 2, 3]` contra `[1, 2, 4]` do source.
///
/// E a quarta linha, inserida no source depois, sai 5 nos dois: o contador
/// da replica andou para depois do numero honrado.
#[test]
fn a_replica_fiel_honra_o_buraco_historico_do_source() {
    let dir_s = DirTemp::novo("rownum-fonte");
    let dir_r = DirTemp::novo("rownum-replica");
    let s = source_com_buraco(&dir_s, Papel::Source);

    let mut cr = config(&dir_r, "replica-do-rownum", Papel::Replica);
    cr.somente_leitura = true;
    cr.replicacao.origens = vec![origem(s.porta)];
    let r = subir(cr);
    esperar("as tres linhas na replica", || {
        linhas(r.porta, "loja").len() == 3
    });
    assert_eq!(
        rownums(r.porta, "loja"),
        vec![1, 2, 4],
        "a replica renumerou o buraco historico do source"
    );

    inserir(s.porta, 4);
    esperar("a quarta linha na replica", || {
        linhas(r.porta, "loja").len() == 4
    });
    assert_eq!(rownums(s.porta, "loja"), vec![1, 2, 4, 5]);
    assert_eq!(
        rownums(r.porta, "loja"),
        vec![1, 2, 4, 5],
        "o contador da replica nao andou para depois do numero honrado"
    );
    // A linha inteira, coluna por coluna, inclusive as de sistema.
    let ls: Vec<String> = linhas(s.porta, "loja").iter().map(Json::escrever).collect();
    let lr: Vec<String> = linhas(r.porta, "loja").iter().map(Json::escrever).collect();
    assert_eq!(ls, lr, "a replica fiel divergiu do source");
}

/// **O mesmo, pelo PITR**: restaurar um backup tirado antes da terceira linha
/// e reaplicar o diario vivo ate agora tem de devolver o `1,2,4` do
/// original. Ele reaplica pelo mesmo `reaplicar_evento_do_proprio_diario`, e
/// renumerava igual -- medido com o defeito reposto: `[1, 2, 3]`.
#[test]
fn o_pitr_devolve_o_buraco_do_original() {
    let dir = DirTemp::novo("rownum-pitr");
    // O destino do backup fica FORA da raiz de dados: o servidor recusa
    // copia que se misturaria com o banco vivo.
    let base = dir.join("dados");
    let copias = dir.join("copias");
    let zip;
    {
        let s = subir(config(&base, "pitr-do-rownum", Papel::Isolado));
        criar_clientes(s.porta);
        inserir(s.porta, 1);
        inserir(s.porta, 2);
        let r = exigir(
            s.porta,
            &format!(
                r#""op":"backup","destino":"{}","database":"loja","zip":true"#,
                copias.display()
            ),
        );
        zip = r.texto_ou("arquivo", "").to_string();
        assert!(!zip.is_empty(), "{}", r.escrever());
    }
    fabricar_buraco(&base, 4);
    let s = subir(config(&base, "pitr-do-rownum", Papel::Isolado));
    inserir(s.porta, 3);
    assert_eq!(rownums(s.porta, "loja"), vec![1, 2, 4], "a premissa caiu");

    std::thread::sleep(Duration::from_millis(5));
    let r = exigir(
        s.porta,
        &format!(
            r#""op":"restaurar_backup","origem":"{zip}","database":"restaurada","ate_ms":{}"#,
            phxsql_server::agora_ms()
        ),
    );
    let pitr = r.campo("pitr").map(Json::escrever).unwrap_or_default();
    assert!(pitr.contains("\"reaplicados\":1"), "{}", r.escrever());
    assert_eq!(
        rownums(s.porta, "restaurada"),
        vec![1, 2, 4],
        "o PITR renumerou o buraco do original"
    );
}

/// **O comportamento velho, que tem de ficar: o bidirecional numera aqui.**
/// Cada servidor do par tem a sua ordem de digitacao, e o `rownum` e dela
/// (`REPLICACAO.md` §12). Quem puxa de um par com `1,2,4` grava `1,2,3`.
///
/// Sem este teste, um conserto que ligasse o `honrar_rownum` no
/// `inserir_replicado` passaria no de cima -- e misturaria as duas fontes de
/// numeracao no mesmo `.reg`.
#[test]
fn o_bidirecional_continua_numerando_na_ordem_daqui() {
    let dir_a = DirTemp::novo("rownum-bidi-a");
    let dir_b = DirTemp::novo("rownum-bidi-b");
    let a = source_com_buraco(&dir_a, Papel::Multi);

    let mut cb = config(&dir_b, "bidi-do-rownum-b", Papel::Multi);
    cb.replicacao.origens = vec![origem(a.porta)];
    let b = subir(cb);
    esperar("as tres linhas em B", || linhas(b.porta, "loja").len() == 3);
    assert_eq!(
        rownums(b.porta, "loja"),
        vec![1, 2, 3],
        "o bidirecional passou a honrar o rownum do outro servidor"
    );
}
