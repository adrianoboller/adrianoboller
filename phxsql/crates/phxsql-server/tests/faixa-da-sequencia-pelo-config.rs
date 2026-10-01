//! Pedido 290: o `inicio` da faixa da `Sequence` chega do `config.json` ao
//! motor, e a prova e pelo SOQUETE.
//!
//! # O defeito que esta bateria repoe
//!
//! O `passo` entrou no `PSCH` v10 com porta de producao (`criar_tabela` com
//! `passo_da_sequencia`), e o `inicio` -- quem diz em que faixa ESTE servidor
//! numera -- nao: `no::definir_inicio_da_sequencia` tinha zero chamadores fora
//! de teste (medido pelo DBA em 23/09/2026). Todo `phxsqld` rodava com inicio
//! 0, entao vinte caixas com `passo = 20` numeravam 20, 40, 60... identicas
//! entre si: a colisao ficava vinte vezes mais esparsa e continuava 100%.
//!
//! Arquivo proprio pelo mesmo motivo do `cache-paginas-pelo-config.rs`: o
//! inicio e um global do PROCESSO, e ligar isto dentro do binario da
//! biblioteca faria a faixa de um teste virar a de outro na mesma corrida.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};
use phxsql_store::no;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

const TOKEN: &str = "faixa-da-sequencia";

/// Sobe o servidor com o trecho `replicacao` dado (vazio = sem a secao).
fn subir(base: &Path, replicacao: &str) -> (Arc<Servidor>, u16) {
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:0", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              {replicacao}
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": false }} }}"#,
        base = base.join("dados").display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return (s, porta);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let f = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    f.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let mut escrita = f.try_clone().unwrap();
    let mut leitor = BufReader::new(f);
    writeln!(escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
    let mut r = String::new();
    leitor.read_line(&mut r).unwrap();
    Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
}

fn ok(porta: u16, corpo: &str) -> Json {
    let j = pedir(porta, corpo);
    assert!(j.booleano_ou("ok", false), "{corpo}: {}", j.escrever());
    j
}

/// Cria `loja.vendas` com a `Sequence` na faixa de `passo` e insere `quantas`
/// linhas. Devolve os numeros que o motor deu, na ordem.
fn numerar(porta: u16, passo: u64, quantas: usize) -> Vec<u64> {
    ok(porta, r#""op":"criar_database","database":"loja""#);
    ok(
        porta,
        &format!(
            r#""op":"criar_tabela","database":"loja","tabela":"vendas",
               "passo_da_sequencia":{passo},
               "colunas":[{{"nome":"id","tipo":"Sequence"}},
                          {{"nome":"nome","tipo":"Str(20)"}}],
               "indices":[{{"nome":"porId","colunas":["id"],"unico":true}}]"#
        )
        .replace('\n', " "),
    );
    inserir(porta, quantas)
}

fn inserir(porta: u16, quantas: usize) -> Vec<u64> {
    for _ in 0..quantas {
        ok(
            porta,
            r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"nome":"x"}"#,
        );
    }
    lidos(porta)
}

fn lidos(porta: u16) -> Vec<u64> {
    let r = ok(
        porta,
        r#""op":"varrer","database":"loja","tabela":"vendas","limite":100"#,
    );
    let linhas = r
        .campo("resultado")
        .and_then(|x| x.campo("linhas"))
        .and_then(Json::lista)
        .unwrap_or_else(|| panic!("varrer sem linhas: {}", r.escrever()));
    linhas
        .iter()
        .map(|l| {
            l.campo("id")
                .and_then(Json::inteiro)
                .unwrap_or_else(|| panic!("linha sem id: {}", l.escrever())) as u64
        })
        .collect()
}

/// **O campo do config CHEGA ao motor.** Faixa 1 de 2: 1, 3, 5.
///
/// # O vermelho
///
/// Sem a chamada a `no::definir_inicio_da_sequencia` no `Servidor::novo`, o
/// servidor numera na faixa 0 -- 2, 4, 6 -- com o `config.json` dizendo 1.
#[test]
fn o_inicio_do_config_poe_o_servidor_na_faixa_dele() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    // A faixa do processo comeca na OUTRA classe: sem isto o teste vizinho,
    // que deixa 7 (classe 1), fazia este passar com o defeito reposto --
    // medido na primeira prova da guarda `faixa-do-config-nao-lida`.
    no::definir_inicio_da_sequencia(0);
    let d = DirTemp::novo("faixa-chega");
    let (_s, porta) = subir(&d.0, r#""replicacao": { "inicio_da_sequencia": 1 },"#);
    assert_eq!(numerar(porta, 2, 3), vec![1, 3, 5]);
    let cfg = ok(porta, r#""op":"config""#);
    assert_eq!(
        cfg.campo("resultado")
            .and_then(|r| r.campo("replicacao"))
            .and_then(|r| r.campo("inicio_da_sequencia"))
            .and_then(Json::inteiro),
        Some(1),
        "o `config` nao mostra a faixa que o servidor usa: {}",
        cfg.escrever()
    );
}

/// **O comportamento VELHO.** Sem o campo, a faixa e 0 -- inclusive quando o
/// processo carregava outra de antes: o arranque DEFINE, nao herda.
#[test]
fn sem_o_campo_a_faixa_e_zero_e_nao_a_que_sobrou_no_processo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    no::definir_inicio_da_sequencia(7);
    let d = DirTemp::novo("faixa-padrao");
    let (_s, porta) = subir(&d.0, "");
    assert_eq!(numerar(porta, 2, 3), vec![2, 4, 6]);
    assert_eq!(no::inicio_da_sequencia(), 0);
}

/// Tabela sem faixa (passo 1) numera 1, 2, 3 com QUALQUER inicio: a faixa so
/// existe onde a tabela a pediu.
#[test]
fn tabela_sem_faixa_numera_como_sempre_com_qualquer_inicio() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = DirTemp::novo("faixa-passo-um");
    let (_s, porta) = subir(&d.0, r#""replicacao": { "inicio_da_sequencia": 5 },"#);
    assert_eq!(numerar(porta, 1, 3), vec![1, 2, 3]);
}

/// **Trocar o inicio de um no que ja numerou RECUSA**, nomeando o campo -- em
/// vez de reusar numero calado (a segunda metade da decisao do dono).
#[test]
fn subir_com_outra_faixa_sobre_tabela_ja_numerada_recusa_nomeando_o_campo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = DirTemp::novo("faixa-troca");
    // O primeiro servidor fica no ar, parado: so o segundo recebe pedido, e
    // e o arranque dele que redefine a faixa do processo.
    let (_primeiro, porta) = subir(&d.0, r#""replicacao": { "inicio_da_sequencia": 1 },"#);
    assert_eq!(numerar(porta, 2, 2), vec![1, 3]);
    let (_s, porta) = subir(&d.0, r#""replicacao": { "inicio_da_sequencia": 0 },"#);
    let r = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"nome":"y"}"#,
    );
    let erro = r.texto_ou("erro", "").to_string();
    assert!(
        !r.booleano_ou("ok", true) && erro.contains("replicacao.inicio_da_sequencia"),
        "a troca de faixa tinha de recusar nomeando o campo: {}",
        r.escrever()
    );
}

/// **A tabela gravada pelo contador defeituoso tem saida pelo protocolo**
/// (parecer do DBA, NAO 290-b): `ajustar_sequencia` com `"pelo_maior": true`
/// realinha pelo maior valor gravado, e a insercao seguinte volta a valer.
///
/// O estado e o que a versao com o contador `v + 1` deixava no disco -- faixa
/// 1 de 2, numeros 1 e 3, contador em 4 --, produzido regravando o contador
/// com o CRC certo.
///
/// # O vermelho
///
/// Sem o braco `pelo_maior` o pedido cai no caminho de sempre, que exige
/// «proxima» e, se a recebesse, abriria a tabela e bateria na mesma recusa.
#[test]
fn a_tabela_do_contador_defeituoso_se_realinha_pelo_protocolo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let d = DirTemp::novo("faixa-realinha");
    let (_s, porta) = subir(&d.0, r#""replicacao": { "inicio_da_sequencia": 1 },"#);
    assert_eq!(numerar(porta, 2, 2), vec![1, 3]);
    gravar_contador_cru(&d.0.join("dados").join("loja"), "vendas", 4);

    let r = pedir(
        porta,
        r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"nome":"z"}"#,
    );
    let erro = r.texto_ou("erro", "").to_string();
    assert!(
        erro.contains("pelo_maior"),
        "a recusa tem de apontar a saida: {}",
        r.escrever()
    );

    let v = ok(
        porta,
        r#""op":"ajustar_sequencia","database":"loja","tabela":"vendas","pelo_maior":true"#,
    );
    let res = v.campo("resultado").expect("resultado");
    assert_eq!(res.inteiro_ou("maior", -1), 3, "{}", v.escrever());
    assert_eq!(res.inteiro_ou("proxima", -1), 5, "{}", v.escrever());
    assert_eq!(inserir(porta, 1), vec![1, 3, 5]);
}

/// Regrava o contador da sequencia (bytes 36..44 do cabecalho do volume 1) com
/// o CRC certo -- o ESTADO que o contador `v + 1` deixava no disco.
fn gravar_contador_cru(dir: &Path, nome: &str, valor: u64) {
    let caminho = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            n.starts_with(nome) && n.ends_with(".reg")
        })
        .expect("o .reg da tabela");
    let mut bytes = std::fs::read(&caminho).unwrap();
    // Tabela sem cifra: cabecalho de 128 bytes, CRC nos 4 ultimos.
    bytes[36..44].copy_from_slice(&valor.to_le_bytes());
    let crc = phxsql_core::crc::crc32(&bytes[..124]);
    bytes[124..128].copy_from_slice(&crc.to_le_bytes());
    std::fs::write(&caminho, bytes).unwrap();
}

/// Valor torto recusa o ARRANQUE, em vez de cair calado na faixa 0 -- que e a
/// faixa de outro no.
#[test]
fn inicio_torto_recusa_o_config() {
    for torto in ["-1", "\"2\"", "1.5", "true"] {
        let texto = format!(r#"{{"token":"t","replicacao":{{"inicio_da_sequencia":{torto}}}}}"#);
        let e = Config::de_json(&Json::analisar(&texto).unwrap())
            .err()
            .unwrap_or_else(|| panic!("{torto} foi aceito"))
            .to_string();
        assert!(e.contains("inicio_da_sequencia"), "{torto}: {e}");
    }
}
