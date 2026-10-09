//! Pedido 729: o retrato da replica nao para a escrita a copia inteira --
//! provado PELO SOQUETE, medindo quanto um `inserir` espera enquanto o
//! retrato copia.
//!
//! # Por que um binario proprio
//!
//! A prova arma `PHXSQL_TESTE_PAUSA_NA_COPIA_DO_RETRATO_MS` (so existe em
//! debug), um gancho do PROCESSO: num binario com outros testes, todo retrato
//! deles pausaria junto.
//!
//! # O que se prova
//!
//! A pausa fica DENTRO da copia. Com a copia sob a ficha exclusiva (o defeito
//! do 729), o escritor de outro database espera a pausa inteira; com a copia
//! em duas passadas do backup, ele espera so a fase 2 -- o portao fechado
//! enquanto se recopia o que mudou.

mod comum;
use comum::DirTemp;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::replica::Cliente;
use phxsql_server::{Config, Papel, Servidor};

const TOKEN: &str = "retrato-sem-a-trava";
const PAUSA_MS: u64 = 1_500;

fn subir(base: &Path) -> (Arc<Servidor>, u16) {
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
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "origem".into();
    c.replicacao.imagem_da_linha = true;
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    (s, porta)
}

fn conectar(porta: u16) -> Cliente {
    Cliente::conectar("127.0.0.1", porta, TOKEN, Duration::from_secs(30)).unwrap()
}

fn exigir(c: &mut Cliente, campos: Vec<(&str, Json)>) -> Json {
    c.pedir(campos).unwrap()
}

fn criar(c: &mut Cliente, database: &str, tabela: &str) {
    exigir(
        c,
        vec![
            ("op", Json::texto_de("criar_database")),
            ("database", Json::texto_de(database)),
        ],
    );
    let corpo = format!(
        r#"{{"op":"criar_tabela","database":"{database}","tabela":"{tabela}",
             "motivo_obrigatorio":false,
             "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}}],
             "indices":[{{"nome":"pk","colunas":["id"],"unico":true,"primario":true}}]}}"#
    );
    let Json::Objeto(campos) = Json::analisar(&corpo).unwrap() else {
        unreachable!()
    };
    c.pedir(
        campos
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect(),
    )
    .unwrap();
}

fn inserir(c: &mut Cliente, database: &str, tabela: &str, id: i64) {
    exigir(
        c,
        vec![
            ("op", Json::texto_de("inserir")),
            ("database", Json::texto_de(database)),
            ("tabela", Json::texto_de(tabela)),
            ("linha", Json::objeto(vec![("id", Json::Numero(id as f64))])),
        ],
    );
}

/// **729:** enquanto o retrato de `loja` copia (com a pausa de 1,5 s dentro
/// da copia), um escritor de `caixa` grava em laco. A maior espera dele tem
/// de ficar longe da pausa; e o retrato diz quanto a escrita ficou parada.
///
/// Defeito reposto (a copia com a ficha exclusiva na mao): o escritor espera
/// a pausa inteira, ~1.500 ms, e o teste cai.
#[test]
fn o_escritor_nao_espera_a_copia_do_retrato() {
    std::env::set_var(
        "PHXSQL_TESTE_PAUSA_NA_COPIA_DO_RETRATO_MS",
        PAUSA_MS.to_string(),
    );
    let d = DirTemp::novo("retrato-sem-a-trava");
    let (_s, porta) = subir(&d);
    let mut admin = conectar(porta);
    criar(&mut admin, "loja", "vendas");
    criar(&mut admin, "caixa", "notas");
    for i in 0..200 {
        inserir(&mut admin, "loja", "vendas", i);
    }

    let tirou = std::thread::spawn(move || {
        let mut c = conectar(porta);
        let t0 = Instant::now();
        let r = c
            .pedir(vec![
                ("op", Json::texto_de("retrato_da_replica")),
                ("database", Json::texto_de("loja")),
            ])
            .unwrap();
        (r, t0.elapsed())
    });
    // O escritor comeca depois de a copia comecar, e grava ate ela acabar.
    std::thread::sleep(Duration::from_millis(200));
    let mut escritor = conectar(porta);
    let mut maior = Duration::ZERO;
    let mut id = 0;
    while !tirou.is_finished() {
        let t0 = Instant::now();
        inserir(&mut escritor, "caixa", "notas", id);
        maior = maior.max(t0.elapsed());
        id += 1;
    }
    let (r, total) = tirou.join().unwrap();
    let parada = r.inteiro_ou("escrita_parada_ms", -1);
    eprintln!(
        "retrato em {} ms; {id} insercoes; maior espera do escritor {} ms; \
         escrita_parada_ms {parada}",
        total.as_millis(),
        maior.as_millis()
    );
    assert!(
        total >= Duration::from_millis(PAUSA_MS),
        "a pausa nao entrou na copia: o retrato levou {total:?}"
    );
    assert!(
        maior < Duration::from_millis(PAUSA_MS / 2),
        "o escritor esperou {maior:?} pelo retrato: a copia esta com a trava na mao"
    );
    assert!(id > 1, "o escritor nao gravou durante o retrato");
    assert!(
        (0..(PAUSA_MS / 2) as i64).contains(&parada),
        "o retrato diz que a escrita ficou parada {parada} ms"
    );
}
