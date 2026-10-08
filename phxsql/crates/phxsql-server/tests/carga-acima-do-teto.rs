//! **Pedido 686: a carga fora de transacao acima do teto e RECUSADA antes de
//! gravar -- e a que cabe chega inteira na replica.**
//!
//! # O que existia
//!
//! O 685 pos o teto no COMMIT. Os tres nomes da carga -- `inserir_lote`,
//! `importar` e `carga`, que sao o mesmo `op_inserir_lote` -- gravam o lote
//! inteiro numa tomada so da trava, e a tomada e UMA transacao para a
//! replica (o id de transacao do 676). Sem passar pelo teto, a carga grande
//! era aceita na origem e chegava a replica em PEDACOS.
//!
//! # A prova, com um teto de teste
//!
//! Como a do 685: o tamanho `S` da carga sai da propria recusa (sonda com
//! teto 1); com teto `S - 1` (a carga e o teto + 1) os tres nomes recusam e
//! nada e gravado; com `S + 1` a carga grava e chega ao central inteira, com
//! `transacoes_em_pedacos` zero. A coluna `nota` e `Memo` para a conta passar
//! pela previsao da linha (o tamanho da imagem depende do valor).
//!
//! Um `#[test]` so neste arquivo, porque o teto injetado e do processo.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "carga-acima-do-teto";
const ESPERA: Duration = Duration::from_secs(30);
const LINHAS: usize = 600;

struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let _ = pedir(self.porta, r#""op":"servico_parar""#);
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
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr { _s: s, porta }
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    let corpo = corpo.replace('\n', " ");
    writeln!(escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
    let mut r = String::new();
    leitor.read_line(&mut r).unwrap();
    Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let j = pedir(porta, corpo);
    assert!(j.booleano_ou("ok", false), "{corpo} -> {}", j.escrever());
    j
}

fn contar(porta: u16) -> usize {
    pedir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"itens","max":5000"#,
    )
    .campo("resultado")
    .and_then(|r| r.campo("linhas"))
    .and_then(Json::lista)
    .map_or(0, <[Json]>::len)
}

/// A carga de [`LINHAS`] linhas pelo nome `op`: lista JSON no
/// `inserir_lote`, CSV colado nos outros dois -- o mesmo conteudo.
fn a_carga(porta: u16, op: &str, base: usize) -> Json {
    let corpo = if op == "inserir_lote" {
        let linhas: Vec<String> = (1..=LINHAS)
            .map(|i| format!(r#"{{"id":{},"nota":"item {i:04}"}}"#, base + i))
            .collect();
        format!(
            r#""op":"{op}","database":"loja","tabela":"itens","linhas":[{}]"#,
            linhas.join(",")
        )
    } else {
        let mut csv = String::from("id;nota\\n");
        for i in 1..=LINHAS {
            csv.push_str(&format!("{};item {i:04}\\n", base + i));
        }
        format!(r#""op":"{op}","database":"loja","tabela":"itens","formato":"csv","texto":"{csv}""#)
    };
    pedir(porta, &corpo)
}

/// O tamanho que a recusa diz: o numero depois de «chega a».
fn bytes_da_recusa(r: &Json) -> usize {
    assert!(
        !r.booleano_ou("ok", true),
        "a carga acima do teto foi ACEITA: {}",
        r.escrever()
    );
    assert!(
        r.escrever().contains("LIMITE_EXCEDIDO"),
        "a recusa nao e de limite: {}",
        r.escrever()
    );
    let texto = r.texto_ou("erro", "").to_string();
    assert!(
        texto.contains("divida a carga") && texto.contains("nada foi gravado"),
        "a recusa nao diz o que fazer: {texto}"
    );
    let depois = texto
        .split_once("chega a ")
        .map(|(_, d)| d)
        .unwrap_or_else(|| panic!("a recusa nao diz o tamanho: {texto:?}"));
    depois
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap()
}

fn replica_de(base: &Path, porta: u16) -> NoAr {
    let mut c = config(base, "central", Papel::Replica);
    c.somente_leitura = true;
    c.replicacao.origens = vec![Origem {
        nome: "caixa01".into(),
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
        espelho: false,
    }];
    subir(c)
}

/// **A prova real do 686.** Defeito reposto (a conta do teto tirada do
/// `op_inserir_lote` -- a guarda `carga-acima-do-teto-aceita`): com o teto em
/// `S - 1` a carga grava as 600 linhas, e o teste cai na asercao da recusa.
#[test]
fn a_carga_acima_do_teto_e_recusada_antes_de_gravar_e_a_que_cabe_chega_inteira() {
    let base_o = DirTemp::novo("carga-teto-origem");
    let base_c = DirTemp::novo("carga-teto-central");
    let caixa = subir(config(&base_o.0, "caixa01", Papel::Source));
    exigir(caixa.porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        caixa.porta,
        r#""op":"criar_tabela","database":"loja","tabela":"itens",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"nota","tipo":"Memo"}]"#,
    );

    // A sonda: com teto 1, a recusa diz quanto a carga custa.
    phxsql_store::log::definir_teto_da_transacao_para_teste(1);
    let s = bytes_da_recusa(&a_carga(caixa.porta, "inserir_lote", 0));
    assert!(
        s > LINHAS * phxsql_store::log::CUSTO_DO_EVENTO,
        "tamanho {s}"
    );

    // Teto + 1: os tres nomes recusam com o MESMO tamanho, e nada se grava.
    phxsql_store::log::definir_teto_da_transacao_para_teste(s - 1);
    for op in ["inserir_lote", "importar", "carga"] {
        assert_eq!(
            bytes_da_recusa(&a_carga(caixa.porta, op, 0)),
            s,
            "{op} mediu a mesma carga diferente"
        );
        assert_eq!(
            contar(caixa.porta),
            0,
            "{op} acima do teto deixou linha gravada na origem"
        );
    }

    // Teto - 1: grava, e chega inteira.
    phxsql_store::log::definir_teto_da_transacao_para_teste(s + 1);
    let r = a_carga(caixa.porta, "carga", 0);
    assert!(
        r.booleano_ou("ok", false),
        "a carga de {s} bytes com teto {} foi recusada: {}",
        s + 1,
        r.escrever()
    );
    assert_eq!(contar(caixa.porta), LINHAS);

    let central = replica_de(&base_c.0, caixa.porta);
    let ate = Instant::now() + ESPERA;
    loop {
        let n = contar(central.porta);
        if n == LINHAS {
            break;
        }
        assert!(
            n == 0,
            "a carga apareceu PELA METADE no central: {n} de {LINHAS}"
        );
        assert!(
            Instant::now() < ate,
            "a carga nao chegou ao central em {} s",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let estado = exigir(central.porta, r#""op":"replicacao_estado""#);
    let em_pedacos = estado
        .campo("resultado")
        .unwrap_or(&estado)
        .campo("origens")
        .and_then(|o| o.campo("caixa01"))
        .and_then(|o| o.campo("transacoes_em_pedacos"))
        .and_then(Json::inteiro)
        .unwrap_or_else(|| panic!("sem transacoes_em_pedacos: {}", estado.escrever()));
    assert_eq!(
        em_pedacos, 0,
        "a carga que a origem aceitou abaixo do teto chegou EM PEDACOS"
    );

    phxsql_store::log::definir_teto_da_transacao_para_teste(0);
    drop(central);
    drop(caixa);
}
