//! Pedido 496, fatia F8: o tamanho do plano antes da primeira escrita, pelo
//! caminho real -- a op `sql`, com uma atividade amarrada como a conexao faz.
//!
//! As cartas ficam no correio porque nenhum teste daqui liga o carteiro: a
//! contagem le a fila, e nao o arquivo.

use super::*;
use crate::aquario::Alarme;
use crate::ocorrencias::Carta;

fn servidor(rotulo: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("plano-largo-{rotulo}"));
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    (Servidor::novo(c).unwrap(), dir)
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn sessao() -> Sessao {
    Sessao {
        ligacao: 77,
        ip: "127.0.0.1".into(),
        ..Sessao::default()
    }
}

/// `b.nums` (id unico, valor obrigatorio) com uma linha por `valores`, na
/// ordem, e ids de 1 em diante.
fn com_nums(s: &Arc<Servidor>, valores: &[i64]) {
    let ses = sessao();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"nums",
                "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                           {"nome":"valor","tipo":"Int4","obrigatoria":true}],
                "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    let linhas: Vec<Json> = valores
        .iter()
        .enumerate()
        .map(|(i, v)| {
            Json::objeto(vec![
                ("id", Json::de_i64(i as i64 + 1)),
                ("valor", Json::de_i64(*v)),
            ])
        })
        .collect();
    let lote = Json::objeto(vec![
        ("database", Json::texto_de("b")),
        ("tabela", Json::texto_de("nums")),
        ("linhas", Json::Lista(linhas)),
    ]);
    let r = s.executar("inserir_lote", &lote, &ses).unwrap();
    assert_eq!(r.inteiro_ou("gravadas", -1), valores.len() as i64);
}

/// Um pedido como a porta de dados o faz: a atividade entra e se amarra, o
/// corpo roda, e volta o desfecho com a mascara de alarmes da tarefa.
fn pedido_amarrado<R>(s: &Arc<Servidor>, corpo: impl FnOnce(&Arc<Servidor>) -> R) -> (R, u32) {
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar("dados:f8", "dados", "127.0.0.1", 1, agora)
        .expect("a telemetria nasce ligada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    a.comecou_pedido("sql", "root", "b", "nums", agora);
    let r = corpo(s);
    let mascara = a.alarmes();
    a.terminou_pedido("root");
    (r, mascara)
}

fn sql(s: &Arc<Servidor>, texto: &str) -> Result<Json> {
    s.executar(
        "sql",
        &Json::objeto(vec![
            ("database", Json::texto_de("b")),
            ("texto", Json::texto_de(texto)),
        ]),
        &sessao(),
    )
}

/// As ocorrencias de plano largo que esperam o carteiro neste servidor.
fn planos_largos(s: &Arc<Servidor>) -> Vec<crate::ocorrencias::Ocorrencia> {
    let mut vistas = Vec::new();
    for c in s.ocorrencias.correio().retirar(
        std::time::Duration::from_millis(1),
        |c| matches!(c, Carta::Ocorrencia(o) if o.alarme == Alarme::PlanoLargo),
    ) {
        if let Carta::Ocorrencia(o) = c {
            vistas.push(o);
        }
    }
    vistas
}

/// **O aceite do desenho, lado que alarma:** `UPDATE ... WHERE id > 0` em
/// 2.000 de 2.000 linhas gera UMA ocorrencia, com o bit na tarefa e a
/// tabela nomeada -- e o UPDATE acontece inteiro, porque so se observa.
#[test]
fn update_por_faixa_em_toda_a_tabela_gera_uma_ocorrencia() {
    let (s, _dir) = servidor("toda");
    com_nums(&s, &[1; 2000]);
    let (r, mascara) = pedido_amarrado(&s, |s| sql(s, "UPDATE nums SET valor = 7 WHERE id > 0"));
    assert_eq!(r.unwrap().inteiro_ou("afetadas", -1), 2000);
    assert_ne!(mascara & Alarme::PlanoLargo.bit(), 0, "o bit na tarefa");
    let vistas = planos_largos(&s);
    assert_eq!(vistas.len(), 1, "uma ocorrencia por plano: {vistas:?}");
    assert!(
        vistas[0]
            .tabelas
            .iter()
            .any(|(d, t)| d == "b" && t == "nums"),
        "{:?}",
        vistas[0].tabelas
    );
}

/// **O aceite do desenho, lado que cala:** 10 de 2.000 nao e plano largo.
#[test]
fn update_por_faixa_estreita_nao_gera_nada() {
    let (s, _dir) = servidor("estreita");
    com_nums(&s, &[1; 2000]);
    let (r, mascara) = pedido_amarrado(&s, |s| sql(s, "UPDATE nums SET valor = 7 WHERE id > 1990"));
    assert_eq!(r.unwrap().inteiro_ou("afetadas", -1), 10);
    assert_eq!(mascara & Alarme::PlanoLargo.bit(), 0);
    assert!(planos_largos(&s).is_empty());
}

/// **A resposta sai igual, byte a byte.** O MESMO texto, as MESMAS 1.000
/// linhas afetadas: numa tabela de 1.000 elas sao 100% (alarma); numa de
/// 2.001, 49,98% (nao alarma). Se o observador acrescentasse um campo, uma
/// nota ou mudasse uma contagem, as duas respostas difeririam.
#[test]
fn a_resposta_ao_cliente_nao_muda_com_o_alarme() {
    let texto = "UPDATE nums SET valor = 9 WHERE valor > 0";
    let (com, _d1) = servidor("resposta-com");
    com_nums(&com, &[1; 1000]);
    let (sem, _d2) = servidor("resposta-sem");
    let mut valores = vec![1; 1000];
    valores.extend(std::iter::repeat_n(0, 1001));
    com_nums(&sem, &valores);

    let (r_com, m_com) = pedido_amarrado(&com, |s| sql(s, texto));
    let (r_sem, m_sem) = pedido_amarrado(&sem, |s| sql(s, texto));
    assert_ne!(m_com & Alarme::PlanoLargo.bit(), 0, "1.000 de 1.000 alarma");
    assert_eq!(m_sem & Alarme::PlanoLargo.bit(), 0, "1.000 de 2.001 nao");
    assert_eq!(planos_largos(&com).len(), 1);
    assert!(planos_largos(&sem).is_empty());
    assert_eq!(r_com.unwrap().escrever(), r_sem.unwrap().escrever());
}

/// **O RED da F8: a ocorrencia sai ANTES da primeira escrita.** O `SET`
/// pede NULO numa coluna obrigatoria, e a PRIMEIRA linha ja recusa -- nada
/// e gravado. A ocorrencia tem de existir assim mesmo.
///
/// Vermelho: contar depois de aplicar (o `observar_a_faixa` movido para
/// depois do laco) nunca roda, porque o `?` da primeira linha sai antes --
/// e este teste cai com zero ocorrencias.
#[test]
fn a_ocorrencia_sai_antes_da_primeira_escrita() {
    let (s, _dir) = servidor("antes");
    com_nums(&s, &[1; 2000]);
    let (r, mascara) = pedido_amarrado(&s, |s| sql(s, "UPDATE nums SET valor = NULL WHERE id > 0"));
    assert!(r.is_err(), "a primeira linha tinha de recusar: {r:?}");
    let lida = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"nums","rowid":1}"#),
            &sessao(),
        )
        .unwrap();
    assert_eq!(
        lida.inteiro_ou("valor", -1),
        1,
        "nada foi gravado: {lida:?}"
    );
    assert_ne!(mascara & Alarme::PlanoLargo.bit(), 0);
    assert_eq!(planos_largos(&s).len(), 1);
}

/// `b.mae` com uma linha e `b.filha` com `filhas` linhas apontando para
/// ela, chave com cascata no alterar e indice dos dois lados.
fn com_cascata(s: &Arc<Servidor>, filhas: i64) {
    let ses = sessao();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &ses)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"mae",
                "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"mae","linha":{"id":1}}"#),
        &ses,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"filha",
                "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                           {"nome":"mae_id","tipo":"Int4"}],
                "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                           {"nome":"porMae","colunas":["mae_id"]}],
                "chaves_estrangeiras":[{"nome":"fk_mae","colunas":["mae_id"],
                                        "tabela_ref":"mae","colunas_ref":["id"],
                                        "ao_excluir":"restringir","ao_alterar":"cascata"}]}"#,
        ),
        &ses,
    )
    .unwrap();
    let linhas: Vec<Json> = (1..=filhas)
        .map(|i| Json::objeto(vec![("id", Json::de_i64(i)), ("mae_id", Json::de_i64(1))]))
        .collect();
    let lote = Json::objeto(vec![
        ("database", Json::texto_de("b")),
        ("tabela", Json::texto_de("filha")),
        ("linhas", Json::Lista(linhas)),
    ]);
    s.executar("inserir_lote", &lote, &ses).unwrap();
}

/// **A cascata larga (C10), solta:** trocar a chave da mae leva as 1.000
/// filhas -- 100% da filha. A ocorrencia nomeia a FILHA, que e a tabela
/// varrida, e nao a mae que o pedido nomeia.
#[test]
fn cascata_que_leva_a_filha_inteira_gera_ocorrencia() {
    let (s, _dir) = servidor("cascata");
    com_cascata(&s, 1000);
    let (r, mascara) = pedido_amarrado(&s, |s| sql(s, "UPDATE mae SET id = 2 WHERE id = 1"));
    assert_eq!(r.unwrap().inteiro_ou("afetadas", -1), 1);
    assert_ne!(mascara & Alarme::PlanoLargo.bit(), 0);
    let vistas = planos_largos(&s);
    assert_eq!(vistas.len(), 1, "{vistas:?}");
    assert!(
        vistas[0].tabelas.iter().any(|(_, t)| t == "filha"),
        "{:?}",
        vistas[0].tabelas
    );
}

/// **O irmao, dentro da transacao:** a mesma troca empilhada alarma no
/// `empilhar`, antes do COMMIT -- que e a primeira escrita.
#[test]
fn cascata_larga_empilhada_alarma_antes_do_commit() {
    let (s, _dir) = servidor("cascata-tx");
    com_cascata(&s, 1000);
    s.executar("begin", &pedido(r#"{"database":"b"}"#), &sessao())
        .unwrap();
    let (r, mascara) = pedido_amarrado(&s, |s| sql(s, "UPDATE mae SET id = 2 WHERE id = 1"));
    r.unwrap();
    assert_ne!(mascara & Alarme::PlanoLargo.bit(), 0);
    assert_eq!(planos_largos(&s).len(), 1, "antes do COMMIT");
    s.executar("commit", &Json::Nulo, &sessao()).unwrap();
}

/// O comportamento velho: cascata pequena nao alarma (999 filhas, 100%,
/// abaixo do piso).
#[test]
fn cascata_abaixo_do_piso_nao_alarma() {
    let (s, _dir) = servidor("cascata-pequena");
    com_cascata(&s, 999);
    let (r, mascara) = pedido_amarrado(&s, |s| sql(s, "UPDATE mae SET id = 2 WHERE id = 1"));
    r.unwrap();
    assert_eq!(mascara & Alarme::PlanoLargo.bit(), 0);
    assert!(planos_largos(&s).is_empty());
}
