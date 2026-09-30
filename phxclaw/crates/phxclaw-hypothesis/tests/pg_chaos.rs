//! Caos real: o motor decide hipoteses em laco enquanto alguem mata o PostgreSQL com
//! SIGKILL e o sobe de novo (tests/chaos/pg_kill_loop.sh). Ao fim, os invariantes:
//!
//! 1. DURABILIDADE: toda decisao que o motor recebeu como Ok esta no banco.
//! 2. ATOMICIDADE: o diario e o estado nunca divergem -- o numero de decisoes gravadas
//!    de cada hipotese e exatamente o que o current_status implica (0, 1 ou 2).
//!
//! Uma decisao que falhou no meio (conexao cortada) pode ou nao ter entrado; o que nao
//! pode e ter entrado pela metade. Por isso a conferencia e pelo banco, nao pelo placar.
//!
//! `PHXCLAW_E2E_RLS_URL=... PHXCLAW_CHAOS_SECONDS=40 cargo test -p phxclaw-hypothesis
//!  --test pg_chaos -- --ignored --nocapture`

use phxclaw_hypothesis::{HypothesisCore, HypothesisPersistenceContext};
use phxclaw_types::HypothesisStatus;
use postgres::{Client, NoTls};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TENANT: &str = "0190c4a0-0000-7000-8000-00000000c4a0";

fn conectar(url: &str, ate: Instant) -> Option<Client> {
    while Instant::now() < ate {
        if let Ok(c) = Client::connect(url, NoTls) {
            return Some(c);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

#[test]
#[ignore = "caos: requer PostgreSQL e o matador tests/chaos/pg_kill_loop.sh"]
fn decisoes_sobrevivem_a_sigkill_do_postgres() {
    let url = std::env::var("PHXCLAW_E2E_RLS_URL").expect("PHXCLAW_E2E_RLS_URL");
    let segundos: u64 = std::env::var("PHXCLAW_CHAOS_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let tenant: Uuid = TENANT.parse().unwrap();
    let ctx = HypothesisPersistenceContext {
        tenant_uuid: tenant,
        research_uuid: None,
        source_state_sha256: "ce".repeat(32),
    };
    let core = HypothesisCore;
    let fim = Instant::now() + Duration::from_secs(segundos);
    // O que o motor CONFIRMOU para cada hipotese: o ultimo estado que voltou Ok.
    let mut confirmado: BTreeMap<Uuid, &'static str> = BTreeMap::new();
    let (mut oks, mut erros, mut reconexoes) = (0u64, 0u64, 0u64);
    let mut c = conectar(&url, fim + Duration::from_secs(30)).expect("banco inicial");

    while Instant::now() < fim {
        let h = core.propose("caos", "sigkill", vec![], vec![], vec![]);
        let passos: [(Option<(HypothesisStatus, HypothesisStatus)>, &'static str); 3] = [
            (None, "proposed"),
            (
                Some((HypothesisStatus::Proposed, HypothesisStatus::Testing)),
                "testing",
            ),
            (
                Some((HypothesisStatus::Testing, HypothesisStatus::Supported)),
                "supported",
            ),
        ];
        for (transicao, nome) in passos {
            let r = match transicao {
                None => core.persist(&mut c, &ctx, &h),
                Some((de, para)) => {
                    core.record_decision(&mut c, tenant, h.uuid, de, para, "caos", "chaos")
                }
            };
            match r {
                Ok(()) => {
                    oks += 1;
                    confirmado.insert(h.uuid, nome);
                }
                Err(_) => {
                    erros += 1;
                    if c.is_closed() || c.simple_query("SELECT 1").is_err() {
                        reconexoes += 1;
                        match conectar(&url, Instant::now() + Duration::from_secs(60)) {
                            Some(novo) => c = novo,
                            None => panic!("banco nao voltou em 60 s"),
                        }
                    }
                    break; // hipotese abandonada no estado em que o banco a deixou
                }
            }
        }
    }

    let mut c = conectar(&url, Instant::now() + Duration::from_secs(60)).expect("banco final");
    c.simple_query(&format!(
        "SELECT set_config('phxclaw.tenant_uuid','{tenant}',false)"
    ))
    .unwrap();
    let linhas = c
        .query(
            "SELECT r.hypothesis_uuid, r.current_status,
                    (SELECT count(*) FROM phxclaw.hypothesis_decisions d
                      WHERE d.hypothesis_uuid = r.hypothesis_uuid AND d.actor = 'chaos')
               FROM phxclaw.hypothesis_records r WHERE r.tenant_uuid = $1",
            &[&tenant],
        )
        .unwrap();
    let no_banco: BTreeMap<Uuid, (String, i64)> = linhas
        .iter()
        .map(|l| (l.get(0), (l.get(1), l.get(2))))
        .collect();

    let mut perdidos = 0;
    for (h, estado) in &confirmado {
        match no_banco.get(h) {
            // O banco pode estar ADIANTE do confirmado (commit feito, resposta perdida),
            // nunca atras.
            Some((s, _)) if ordem(s) >= ordem(estado) => {}
            outro => {
                perdidos += 1;
                eprintln!("PERDIDO {h}: confirmado {estado}, banco {outro:?}");
            }
        }
    }
    let mut divergentes = 0;
    for (h, (s, n)) in &no_banco {
        if ordem(s) as i64 != *n {
            divergentes += 1;
            eprintln!("DIVERGE {h}: estado {s} com {n} decisoes no diario");
        }
    }
    eprintln!(
        "caos: {segundos} s, {oks} ok, {erros} erros, {reconexoes} reconexoes, {} hipoteses no banco, {perdidos} perdidas, {divergentes} divergentes",
        no_banco.len()
    );
    assert!(reconexoes > 0, "nenhuma queda aconteceu: o caos nao rodou");
    assert_eq!(perdidos, 0, "decisao confirmada sumiu depois do SIGKILL");
    assert_eq!(divergentes, 0, "diario e estado divergiram");
}

fn ordem(s: &str) -> usize {
    match s {
        "proposed" => 0,
        "testing" => 1,
        "supported" => 2,
        _ => 99,
    }
}
