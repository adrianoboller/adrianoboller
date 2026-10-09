//! **Pedido 769:** a transacao que a replica nao aplicaria inteira e
//! recusada (pedidos 685 e 686) -- e agora a recusa e tambem o alarme
//! `transacao_acima_do_teto`, pelo produtor unico, nos DOIS irmaos que fazem
//! a mesma conta: o `COMMIT` e a carga fora de transacao.
//!
//! # Por que pelo soquete, e num binario so
//!
//! O teto injetado (`definir_teto_da_transacao_para_teste`) e do PROCESSO:
//! num teste unitario ele recusaria o COMMIT dos testes vizinhos. Aqui o
//! processo e so deste `#[test]`. E a prova le a ocorrencia pela op
//! `ocorrencias`, do `ocorrencias.log` que o carteiro gravou -- o caminho que
//! o administrador usa.
//!
//! Um servidor por irmao: o silencio das ocorrencias e por (alarme, usuario,
//! IP) e por servidor, e os dois pedidos daqui teriam a mesma chave.
//!
//! RED: tirar o `sinal` do `servico_marca_01` derruba a metade do COMMIT;
//! tirar o do `servico_escrita_01`, a da carga.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "alarme-do-teto";

struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let mut l = Ligacao::nova(self.porta);
        let _ = l.pedir(r#""op":"servico_parar""#);
    }
}

fn subir(base: &std::path::Path) -> NoAr {
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
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr { _s: s, porta }
}

/// Uma conexao que FICA -- a transacao vive na sessao.
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
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }

    fn exigir(&mut self, corpo: &str) -> Json {
        let j = self.pedir(corpo);
        assert!(j.booleano_ou("ok", false), "{corpo} -> {}", j.escrever());
        j
    }
}

fn com_tabela(porta: u16) -> Ligacao {
    let mut l = Ligacao::nova(porta);
    l.exigir(r#""op":"criar_database","database":"loja""#);
    l.exigir(
        r#""op":"criar_tabela","database":"loja","tabela":"itens",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}]"#,
    );
    l
}

fn recusada_por_limite(r: &Json) {
    assert!(
        !r.booleano_ou("ok", true) && r.escrever().contains("LIMITE_EXCEDIDO"),
        "premissa: o teto 1 tinha de recusar por limite: {}",
        r.escrever()
    );
}

/// As ocorrencias `transacao_acima_do_teto` gravadas, esperando o carteiro
/// (que acorda na hora em que a carta entra, mas noutra thread).
fn ocorrencias_do_teto(porta: u16) -> Vec<Json> {
    let ate = Instant::now() + Duration::from_secs(10);
    loop {
        let r =
            Ligacao::nova(porta).exigir(r#""op":"ocorrencias","alarme":"transacao_acima_do_teto""#);
        let linhas: Vec<Json> = r
            .campo("resultado")
            .and_then(|r| r.campo("linhas"))
            .and_then(Json::lista)
            .map(<[Json]>::to_vec)
            .unwrap_or_default();
        if !linhas.is_empty() || Instant::now() > ate {
            return linhas;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_recusa_pelo_teto_e_alarme_no_commit_e_na_carga() {
    let base_commit = DirTemp::novo("alarme-teto-commit");
    let base_carga = DirTemp::novo("alarme-teto-carga");
    let commit = subir(&base_commit.0);
    let carga = subir(&base_carga.0);
    let mut lc = com_tabela(commit.porta);
    let mut lg = com_tabela(carga.porta);
    assert!(
        ocorrencias_do_teto_sem_esperar(commit.porta).is_empty(),
        "premissa: o servidor novo nao tem ocorrencia do teto"
    );

    phxsql_store::log::definir_teto_da_transacao_para_teste(1);

    // O COMMIT.
    lc.exigir(r#""op":"begin","database":"loja""#);
    lc.exigir(r#""op":"inserir","database":"loja","tabela":"itens","linha":{"id":1}"#);
    recusada_por_limite(&lc.pedir(r#""op":"commit""#));

    // A carga, fora de transacao.
    recusada_por_limite(&lg.pedir(
        r#""op":"inserir_lote","database":"loja","tabela":"itens","linhas":[{"id":1},{"id":2}]"#,
    ));

    phxsql_store::log::definir_teto_da_transacao_para_teste(0);

    let no_commit = ocorrencias_do_teto(commit.porta);
    assert_eq!(
        no_commit.len(),
        1,
        "o COMMIT acima do teto nao virou ocorrencia"
    );
    let na_carga = ocorrencias_do_teto(carga.porta);
    assert_eq!(
        na_carga.len(),
        1,
        "a carga acima do teto nao virou ocorrencia"
    );
    for o in no_commit.iter().chain(&na_carga) {
        assert_eq!(o.texto_ou("gravidade", ""), "vermelho", "{}", o.escrever());
        assert_eq!(o.texto_ou("grupo", ""), "replica", "{}", o.escrever());
    }
    drop((lc, lg));
}

fn ocorrencias_do_teto_sem_esperar(porta: u16) -> Vec<Json> {
    Ligacao::nova(porta)
        .exigir(r#""op":"ocorrencias","alarme":"transacao_acima_do_teto""#)
        .campo("resultado")
        .and_then(|r| r.campo("linhas"))
        .and_then(Json::lista)
        .map(<[Json]>::to_vec)
        .unwrap_or_default()
}
