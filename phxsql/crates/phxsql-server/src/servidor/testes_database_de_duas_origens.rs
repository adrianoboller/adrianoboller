//! A guarda do pedido 406: duas origens nao entregam o mesmo nome de database.
//!
//! O nivel ESTATICO (listas declaradas que se cruzam) mora no
//! `Config::validar()` e tem prova la. Aqui esta o DINAMICO -- o que so a
//! descoberta sabe: lista vazia quer dizer «todos os databases daquela
//! origem», e quais sao eles so a origem diz, ja conectada.
use super::*;
use std::collections::BTreeMap;

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("dbs-406-{rotulo}"))
}

/// A configuracao das origens descritas, ANTES de virar servidor.
///
/// Ela sai separada porque o `Config::validar()` nao roda em
/// `Config::de_json` -- so em `Config::ler` e no `gravar_a_arvore` da
/// tela. Um teste que pula o `validar()` prova metade da guarda e
/// carimba a outra metade sem olhar: foi assim que a recusa estatica
/// passou a derrubar o no de cluster sem nenhum teste acusar.
fn config(d: &std::path::Path, origens: &str, cluster: &str) -> Config {
    let txt = format!(
        r#"{{"token":"t","somente_leitura":true,
                 "replicacao":{{"papel":"replica","id_servidor":"central",
                   "origens":[{origens}]}}{cluster}}}"#
    );
    let mut c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
    c.base = d.to_path_buf();
    c.log_acessos = d.join("acessos.log");
    c.blacklist = d.join("blacklist.json");
    c.dblink = d.join("dblink.json");
    c.jobs = d.join("jobs.json");
    c
}

/// Um servidor com as origens descritas, sem subir thread nenhuma: o que
/// se prova aqui e o FILTRO, e ele nao precisa de rede.
///
/// O `validar()` roda aqui dentro para que nenhum teste deste modulo
/// exercite um arranjo que o servidor de verdade recusaria no arranque:
/// filtro provado sobre configuracao que nao sobe e prova de caminho que
/// ninguem percorre.
fn servidor(d: &std::path::Path, origens: &str, cluster: &str) -> Arc<Servidor> {
    let c = config(d, origens, cluster);
    c.validar()
        .unwrap_or_else(|e| panic!("o arranjo do teste nao sobe: {e}"));
    Servidor::novo(c).unwrap()
}

fn origem(nome: &str, databases: &str) -> String {
    format!(
        r#"{{"nome":"{nome}","host":"10.9.9.9","porta":5000,"token":"t",
                 "databases":[{databases}]}}"#
    )
}

/// As recusas anotadas para uma origem, como `replicacao_estado` as mostra.
fn recusas(s: &Servidor, origem: &str) -> BTreeMap<String, String> {
    s.estado_replicacao
        .lock()
        .unwrap()
        .get(origem)
        .map(|e| e.recusas.clone())
        .unwrap_or_default()
}

/// A origem de lista VAZIA anuncia o `loja` que e da declarada: ela perde
/// SO o `loja`, e as dezenove certas dela andam. Uma origem repetida nao
/// pode derrubar o resto -- e por isso a recusa e um filtro, e nao um
/// `Err` da rodada.
///
/// O arranjo e UMA declarada contra UMA vazia de proposito: desde o
/// conserto do ALTO-3 duas listas vazias nao sobem
/// (`Config::recusar_duas_origens_sem_databases`), e um teste que
/// exercita um arranjo que nao sobe mais prova um caminho que ninguem
/// percorre.
#[test]
fn a_segunda_origem_perde_so_o_database_repetido() {
    let d = dir("repetido");
    let s = servidor(
        &d,
        &format!(
            "{},{}",
            origem("alfa", "\"loja\",\"caixa01\""),
            origem("beta", "")
        ),
        "",
    );
    s.ligar_guarda_de_databases();

    let de_alfa = s.so_os_databases_desta_origem("alfa", vec!["loja".into(), "caixa01".into()]);
    assert_eq!(de_alfa, vec!["loja".to_string(), "caixa01".to_string()]);

    let de_beta = s.so_os_databases_desta_origem(
        "beta",
        vec!["loja".into(), "caixa02".into(), "caixa03".into()],
    );
    assert_eq!(
        de_beta,
        vec!["caixa02".to_string(), "caixa03".to_string()],
        "o repetido tinha de sair E o resto tinha de ficar"
    );

    // A recusa NOMEIA as duas origens e o database -- nada de erro cru.
    let r = recusas(&s, "beta");
    let motivo = r.get("loja").expect("sem a recusa do loja em beta");
    assert!(motivo.contains("loja"), "{motivo}");
    assert!(
        motivo.contains("alfa"),
        "a recusa nao nomeou a dona: {motivo}"
    );
    assert_eq!(r.len(), 1, "recusou mais do que o repetido: {r:?}");
    // E a origem que ficou com o nome nao recebe recusa nenhuma.
    assert!(recusas(&s, "alfa").is_empty());
    std::fs::remove_dir_all(&d).unwrap();
}

/// Lista declarada ganha de lista vazia, e ganha ANTES de qualquer thread
/// subir. Sem esta regra o vencedor seria a thread que chegasse primeiro e
/// mudaria a cada arranque -- o database local receberia linhas de um
/// source hoje e de outro amanha, que e a divergencia de rowid que a
/// guarda existe para impedir. Aqui `curitiba` nem roda: quem pede e so
/// `saopaulo`, e `Z` ja e de `curitiba`.
#[test]
fn a_lista_declarada_ganha_da_lista_vazia() {
    let d = dir("declarada");
    let s = servidor(
        &d,
        &format!("{},{}", origem("curitiba", "\"Z\""), origem("saopaulo", "")),
        "",
    );
    s.ligar_guarda_de_databases();

    let de_sp = s.so_os_databases_desta_origem("saopaulo", vec!["Z".into(), "W".into()]);
    assert_eq!(de_sp, vec!["W".to_string()]);
    let motivo = recusas(&s, "saopaulo")
        .get("Z")
        .cloned()
        .expect("sem a recusa do Z");
    assert!(motivo.contains("curitiba"), "{motivo}");
    std::fs::remove_dir_all(&d).unwrap();
}

/// O recado sai UMA vez, e nao a cada rodada: a recusa e estavel, e
/// repeti-la afogaria o log de quem tem dezenove bases certas.
#[test]
fn o_recado_da_recusa_nao_se_repete_a_cada_rodada() {
    let d = dir("uma-vez");
    let s = servidor(
        &d,
        &format!("{},{}", origem("alfa", "\"loja\""), origem("beta", "")),
        "",
    );
    s.ligar_guarda_de_databases();
    s.so_os_databases_desta_origem("alfa", vec!["loja".into()]);
    for _ in 0..5 {
        let r = s.so_os_databases_desta_origem("beta", vec!["loja".into(), "b".into()]);
        assert_eq!(r, vec!["b".to_string()], "o filtro mudou entre rodadas");
    }
    // Cinco rodadas, uma recusa anotada -- e UM avisado, que e o
    // mecanismo: quem ja ouviu nao ouve de novo.
    assert_eq!(recusas(&s, "beta").len(), 1);
    let donos = s.dono_do_database.lock().unwrap();
    let dono = donos.get("loja").expect("ninguem ficou com o loja");
    assert_eq!(dono.origem, "alfa");
    assert_eq!(
        dono.avisadas.iter().cloned().collect::<Vec<_>>(),
        vec!["beta".to_string()],
        "o recado nao passou pelo registro de quem ja ouviu: ele sairia \
             de novo a cada rodada"
    );
    drop(donos);
    std::fs::remove_dir_all(&d).unwrap();
}

/// **O teste do comportamento VELHO.** Com uma origem so -- o par 1<->1,
/// que e a configuracao mais comum que existe -- a guarda nem liga: o
/// mapa fica VAZIO (nenhum `lock`, nenhuma reivindicacao) e a lista volta
/// inteira. Guarda nova entra pedida, nao imposta, e instrumentacao
/// desligada custa zero.
#[test]
fn com_uma_origem_so_a_guarda_nem_liga() {
    let d = dir("par");
    let s = servidor(&d, &origem("master", "\"loja\""), "");
    s.ligar_guarda_de_databases();
    assert!(!s.ha_varias_origens.load(Ordering::Relaxed));
    assert!(
        s.dono_do_database.lock().unwrap().is_empty(),
        "a guarda reivindicou num servidor de uma origem so"
    );
    let tudo = s.so_os_databases_desta_origem("master", vec!["loja".into(), "x".into()]);
    assert_eq!(tudo, vec!["loja".to_string(), "x".to_string()]);
    std::fs::remove_dir_all(&d).unwrap();
}

/// Com `cluster` a guarda NAO liga, e nao e detalhe: ali quem puxa e um
/// laco so, do master CORRENTE, e o nome da origem muda a cada eleicao
/// (`cluster:<id>`). Um dono guardado por nome recusaria ao master novo o
/// database do master velho -- a promocao inteira parando por causa de
/// uma guarda que nao era para ela.
#[test]
fn com_cluster_a_guarda_nao_liga() {
    let d = dir("cluster");
    let cluster = r#","cluster":{"id":"no1","janela_inatividade_s":30,
              "nos":[{"id":"no1","endereco":"127.0.0.1","porta":5399},
                     {"id":"no2","endereco":"127.0.0.1","porta":5398}]}"#;
    let origens = format!(
        "{},{}",
        origem("um", "\"loja\""),
        origem("dois", "\"loja\"")
    );
    // O nivel ESTATICO tem de se desligar pelo mesmo crivo: sem esta
    // linha o teste carimbava a metade dinamica e nao via que o
    // `validar()` passara a recusar o mesmo arquivo -- um no com origens
    // sobrando de antes deixava de subir, e a tela de configuracao
    // deixava de gravar o proprio config que estava no disco.
    let c = config(&d, &origens, cluster);
    c.validar()
        .unwrap_or_else(|e| panic!("o no de cluster deixou de subir: {e}"));
    let s = Servidor::novo(c).unwrap();
    s.ligar_guarda_de_databases();
    assert!(!s.ha_varias_origens.load(Ordering::Relaxed));
    // Dois "masters" seguidos com nomes diferentes ficam os dois com o
    // `loja`: e o que a promocao precisa.
    for quem in ["cluster:no1", "cluster:no2"] {
        assert_eq!(
            s.so_os_databases_desta_origem(quem, vec!["loja".into()]),
            vec!["loja".to_string()]
        );
    }
    std::fs::remove_dir_all(&d).unwrap();
}
