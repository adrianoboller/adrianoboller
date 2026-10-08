//! **Pedido 630, a janela da conta (02/10/2026).** A escrita local na replica
//! era contada DEPOIS de gravar. Entre uma coisa e outra a escrita ja estava no
//! diario e o contador ainda nao subira: a rodada da replica que caisse ali
//! (noutra thread) via o diario andado, chamava `por_que_nao_continua` com
//! zero escritas locais e gravava a recusa das DUAS causas -- guardada por
//! posicao, e o teste de soquete `escrita_local_sem_o_source_andar_...` lia
//! «escrita localmente» onde esperava «escrita LOCAL». Flocou uma vez em
//! centenas; aqui a janela e aberta de proposito pelo gancho.
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

fn replica_de_escrita_livre(rotulo: &str) -> (DirTemp, Arc<Servidor>) {
    let d = DirTemp::novo(&format!("janela-630-{rotulo}"));
    let txt = r#"{"token":"t",
            "replicacao":{"papel":"replica","id_servidor":"r",
              "origens":[{"nome":"fonte","host":"10.9.9.9","porta":5000,
                          "token":"t","databases":["loja"]}]}}"#;
    let mut c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
    c.base = d.to_path_buf();
    c.log_acessos = d.join("acessos.log");
    c.blacklist = d.join("blacklist.json");
    c.dblink = d.join("dblink.json");
    c.jobs = d.join("jobs.json");
    c.web.ligado = false;
    c.validar().unwrap();
    let s = Servidor::novo(c).unwrap();
    (d, s)
}

fn rodar(s: &Servidor, op: &str, corpo: &str) -> Result<Json> {
    let p = Json::analisar(&format!(r#"{{"op":"{op}",{corpo}}}"#)).unwrap();
    s.executar_e_contar_escrita_local(op, &p, &Sessao::default())
}

/// O preparo passa por `executar` e nao pela conta: o que se mede aqui e o
/// `inserir`, e o `criar_tabela` tambem e escrita que a conta alcanca.
fn criar_loja(s: &Servidor) {
    for (op, corpo) in [
        ("criar_database", r#""database":"loja""#),
        (
            "criar_tabela",
            r#""database":"loja","tabela":"clientes",
                   "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                   "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
        ),
    ] {
        let p = Json::analisar(&format!(r#"{{"op":"{op}",{corpo}}}"#)).unwrap();
        s.executar(op, &p, &Sessao::default()).unwrap();
    }
}

const INSERIR_99: &str = r#""database":"loja","tabela":"clientes","linha":{"id":99}"#;

/// **A prova da janela.** No instante em que a escrita ja gravou e a
/// operacao ainda nao devolveu, a causa que a replica NOMEIA tem de ser a
/// local. Defeito reposto (contar depois do `Ok`): o gancho le zero e a
/// frase das duas causas -- este teste cai.
#[test]
fn na_janela_entre_gravar_e_responder_a_causa_ja_e_a_escrita_local() {
    let (_d, s) = replica_de_escrita_livre("janela");
    criar_loja(&s);
    let lida = Rc::new(RefCell::new(String::new()));
    let l2 = Rc::clone(&lida);
    GANCHO_APOS_ESCRITA.with(|g| {
        *g.borrow_mut() = Some(Box::new(move |s: &Servidor| {
            *l2.borrow_mut() = s.por_que_nao_continua("loja/clientes", 3);
        }));
    });
    rodar(&s, "inserir", INSERIR_99).unwrap();
    GANCHO_APOS_ESCRITA.with(|g| *g.borrow_mut() = None);
    let causa = lida.borrow().clone();
    assert!(
        causa.contains("escrita LOCAL") && causa.contains("ACEITOU 1"),
        "na janela a replica nao sabia da propria escrita: {causa}"
    );
}

/// O irmao que impede o conserto de contar a mais: pedido que FALHA (tabela
/// que nao existe) nao deixa conta nem entrada no mapa -- o teto da revisao
/// SEC M2 -- e a escrita boa soma uma so vez.
#[test]
fn escrita_que_falha_nao_deixa_conta_nem_entrada_no_mapa() {
    let (_d, s) = replica_de_escrita_livre("falha");
    criar_loja(&s);
    for i in 0..5 {
        let r = rodar(
            &s,
            "inserir",
            &format!(r#""database":"loja","tabela":"nao_existe_{i}","linha":{{"id":1}}"#),
        );
        assert!(r.is_err(), "inserir em tabela inexistente tinha de falhar");
    }
    assert!(
        s.escritas_locais_na_replica.lock().unwrap().is_empty(),
        "a conta da escrita que falhou ficou no mapa"
    );
    rodar(&s, "inserir", INSERIR_99).unwrap();
    let m = s.escritas_locais_na_replica.lock().unwrap();
    assert_eq!(m.get("loja/clientes"), Some(&1));
    assert_eq!(m.len(), 1);
}
