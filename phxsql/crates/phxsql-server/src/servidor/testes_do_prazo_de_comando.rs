//! Pedido 765, fatia P2: o prazo de COMANDO, dentro e fora de transacao,
//! pelo MESMO relogio do `STATEMENT TIMEOUT` (`Atividade::prazo_ate_ms`) e
//! pelo mesmo ponto de cancelamento (`Atividade::siga`).
//!
//! O RED de cada teste esta escrito acima dele: qual linha, tirada, o
//! derruba. A camada de protecao fica LIGADA em todos -- nada aqui e
//! comando da lista de perigo, so leitura.

use super::*;

const IP: &str = "127.0.0.1";
const LINHAS: u32 = 20_000;

fn p(t: &str) -> Json {
    Json::analisar(t).unwrap()
}

fn servidor(nome: &str, prazo_ms: u64, so_observa: bool) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("prazo-de-comando-{nome}"));
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        // A pagina inteira numa varredura so: o prazo tem de morder no meio
        // de UM pedido, e o teto de fabrica (1.000) caberia em 1 ms.
        max_linhas: 50_000,
        ..Config::default()
    };
    c.protecao.prazo_comando_ms = prazo_ms;
    c.protecao.prazo_comando_so_observa = so_observa;
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &p(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"b","tabela":"c",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"nome","tipo":"Str(20)"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                           "primario":true}]}"#),
        &dono,
    )
    .unwrap();
    for inicio in (1..=LINHAS).step_by(5_000) {
        let lote: Vec<String> = (inicio..inicio + 5_000)
            .map(|i| format!(r#"{{"id":{i},"nome":"n{i}"}}"#))
            .collect();
        s.executar(
            "inserir_lote",
            &p(&format!(
                r#"{{"database":"b","tabela":"c","linhas":[{}]}}"#,
                lote.join(",")
            )),
            &dono,
        )
        .unwrap();
    }
    (s, dir)
}

/// O pedido como a conexao o faz: a atividade amarrada (e nela que mora o
/// relogio) e o `despachar` (e la que o prazo de comando se arma).
fn pede(s: &Servidor, chave: &str, sessao: &mut Sessao, corpo: &str) -> Result<Json> {
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar(chave, "dados", IP, 1, agora)
        .expect("a telemetria nasce ligada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    a.comecou_pedido("varrer", "root", "b", "c", agora);
    let linha = format!(r#"{{"token":"t",{corpo}}}"#);
    let r = s.despachar(&linha, sessao, IP).2;
    a.terminou_pedido("root");
    r
}

const VARRER: &str = r#""op":"varrer","database":"b","tabela":"c","max":50000"#;

fn linhas_lidas(r: &Json) -> usize {
    r.campo("linhas")
        .and_then(Json::lista)
        .map(<[Json]>::len)
        .unwrap_or(0)
}

fn prazos_estourados(s: &Servidor) -> Vec<crate::ocorrencias::Ocorrencia> {
    use crate::ocorrencias::Carta;
    s.ocorrencias
        .correio()
        .retirar(std::time::Duration::from_millis(1), |c| {
            matches!(c, Carta::Ocorrencia(o) if o.alarme == crate::aquario::Alarme::PrazoEstourado)
        })
        .into_iter()
        .filter_map(|c| match c {
            Carta::Ocorrencia(o) => Some(o),
            _ => None,
        })
        .collect()
}

/// Proteger: a varredura de 20.000 linhas com prazo de 1 ms e CANCELADA no
/// meio, e a trava de dados fica livre para o segundo cliente -- que chega
/// enquanto ela corre e termina. O mesmo pedido sem prazo le tudo.
///
/// RED: sem o `armar_prazo_de_comando` do `despachar`, a varredura le as
/// 20.000 e o `Cancelado` nao vem.
#[test]
fn o_prazo_de_comando_cancela_a_varredura_e_solta_a_trava() {
    let (s, _d) = servidor("proteger", 1, false);
    let copia = Arc::clone(&s);
    let longa =
        std::thread::spawn(move || pede(&copia, "dados:longa", &mut Sessao::default(), VARRER));
    // O segundo cliente: uma leitura de UMA linha, por outra conexao.
    let r = pede(
        &s,
        "dados:curta",
        &mut Sessao::default(),
        r#""op":"ler","database":"b","tabela":"c","rowid":1"#,
    );
    assert!(r.is_ok(), "o segundo cliente nao leu: {r:?}");
    let r = longa.join().unwrap();
    match &r {
        Err(PhxError::Cancelado(m)) => {
            assert!(m.contains("passou do prazo de comando do servidor"), "{m}");
        }
        outro => panic!(
            "a varredura nao foi cancelada: {}",
            outro
                .as_ref()
                .map(|j| format!("{} linhas", linhas_lidas(j)))
                .unwrap_or_else(|e| e.to_string())
        ),
    }
    assert!(
        s.dados.try_write().is_ok(),
        "a trava de dados ficou presa depois do cancelamento"
    );
    // A ocorrencia do estouro, uma, com o rotulo do comando.
    let o = prazos_estourados(&s);
    assert_eq!(o.len(), 1, "{o:?}");

    // O comportamento VELHO: sem prazo, a mesma varredura le tudo.
    let (s, _d) = servidor("sem-prazo", 0, false);
    let r = pede(&s, "dados:longa", &mut Sessao::default(), VARRER).unwrap();
    assert_eq!(linhas_lidas(&r), LINHAS as usize);
    assert!(prazos_estourados(&s).is_empty());
}

/// Observar: o mesmo prazo estourado deixa TERMINAR e emite UMA ocorrencia
/// `PrazoEstourado` -- nao uma por linha.
///
/// RED: com o ramo `ComandoObservado` do `siga` caindo no `Cancelado`, a
/// varredura e cancelada; sem o `sinal_em`, a ocorrencia nao sai.
#[test]
fn em_observar_a_varredura_termina_e_emite_uma_ocorrencia() {
    let (s, _d) = servidor("observar", 1, true);
    let r = pede(&s, "dados:longa", &mut Sessao::default(), VARRER);
    let r = r.unwrap_or_else(|e| panic!("observar cancelou: {e}"));
    assert_eq!(linhas_lidas(&r), LINHAS as usize);
    let o = prazos_estourados(&s);
    assert_eq!(o.len(), 1, "{o:?}");
}

/// O prazo e do PEDIDO: o `STATEMENT TIMEOUT` que uma transacao ja
/// confirmada deixou no relogio nao cancela a varredura seguinte da mesma
/// conexao.
///
/// RED (defeito que existia antes da P2): sem o `armar_prazo_de_comando` do
/// `despachar` -- que zera o relogio a cada pedido --, o prazo de 20 ms do
/// `ler` da transacao continuava armado e a varredura saia `Cancelado`
/// citando um STATEMENT TIMEOUT de uma transacao que ja tinha acabado.
#[test]
fn o_prazo_da_transacao_confirmada_nao_vaza_para_o_pedido_seguinte() {
    let (s, _d) = servidor("vazamento", 0, false);
    let mut sessao = Sessao {
        ligacao: 41,
        ..Sessao::default()
    };
    let chave = "dados:vazamento";
    pede(
        &s,
        chave,
        &mut sessao,
        r#""op":"begin","database":"b","statement_timeout":20"#,
    )
    .unwrap();
    pede(
        &s,
        chave,
        &mut sessao,
        r#""op":"ler","database":"b","tabela":"c","rowid":1"#,
    )
    .unwrap();
    pede(&s, chave, &mut sessao, r#""op":"commit""#).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(60));
    let r = pede(&s, chave, &mut sessao, VARRER);
    let r = r.unwrap_or_else(|e| panic!("o prazo da transacao vazou: {e}"));
    assert_eq!(linhas_lidas(&r), LINHAS as usize);
}

/// UM relogio so, e o do servidor e TETO: uma transacao que declara
/// `statement_timeout` de um minuto nao afrouxa o prazo de comando de 1 ms.
///
/// RED: com o `definir_prazo` gravando o prazo da transacao por cima (sem o
/// `teto_do_comando_ms`), a varredura dentro da transacao le as 20.000.
#[test]
fn a_transacao_nao_afrouxa_o_prazo_de_comando() {
    let (s, _d) = servidor("teto", 1, false);
    let mut sessao = Sessao {
        ligacao: 42,
        ..Sessao::default()
    };
    let chave = "dados:teto";
    pede(
        &s,
        chave,
        &mut sessao,
        r#""op":"begin","database":"b","statement_timeout":60000"#,
    )
    .unwrap();
    let r = pede(&s, chave, &mut sessao, VARRER);
    match &r {
        Err(PhxError::Cancelado(m)) => {
            assert!(m.contains("passou do prazo de comando do servidor"), "{m}")
        }
        outro => panic!(
            "a transacao afrouxou o prazo: {:?}",
            outro.as_ref().map(linhas_lidas)
        ),
    }
    let _ = pede(&s, chave, &mut sessao, r#""op":"rollback""#);
}
