//! O decisor restrito (R1): forma fechada, confianca sempre, o incerto sobe, e a decisao
//! nunca concede permissao no portao.

use phxclaw_agent::ScriptedLlm;
use phxclaw_agent::decisao::*;
use phxclaw_agent::regras::Decisao as NoPortao;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;

fn modelo(respostas: &[&str]) -> Arc<dyn Decisor> {
    Arc::new(DecisorPorModelo {
        llm: Arc::new(ScriptedLlm::new(
            respostas.iter().map(|t| ScriptedLlm::text(t)).collect(),
        )),
    })
}

fn tipos() -> Questao {
    Questao::escolha(
        "tipo da tarefa",
        &["codigo", "pesquisa"],
        "corrija o teste do cargo",
    )
}

#[tokio::test]
async fn modelo_fora_das_opcoes_e_sem_decisao_nunca_palpite() {
    // «código» com acento nao e a opcao «codigo»: aproximar seria palpite.
    for fora in [
        r#"{"value": "código", "confidence": 0.99}"#,
        r#"{"value": "outra", "confidence": 0.99}"#,
        r#"{"value": true, "confidence": 0.99}"#,
        r#"{"value": "codigo"}"#,
        r#"{"value": "codigo", "confidence": 1.5}"#,
        "acho que e codigo",
    ] {
        let d = modelo(&[fora]).decidir(&tipos()).await;
        assert!(matches!(d, Desfecho::SemDecisao { .. }), "{fora}: {d:?}");
    }
    let d = modelo(&["```json\n{\"value\": \"codigo\", \"confidence\": 0.8}\n```"])
        .decidir(&tipos())
        .await;
    let Desfecho::Decidido(x) = d else {
        panic!("dentro das opcoes tem de decidir: {d:?}")
    };
    assert_eq!(x.valor, Valor::Escolha("codigo".into()));
    assert_eq!(x.confianca, 0.8);
}

#[tokio::test]
async fn nota_e_predicado_conferem_a_forma() {
    let q = Questao::nota("risco", "rm -rf /");
    assert!(matches!(
        modelo(&[r#"{"value": 1.2, "confidence": 0.9}"#])
            .decidir(&q)
            .await,
        Desfecho::SemDecisao { .. }
    ));
    assert!(matches!(
        modelo(&[r#"{"value": 0.7, "confidence": 0.9}"#])
            .decidir(&q)
            .await,
        Desfecho::Decidido(_)
    ));
    let p = Questao::predicado("apaga dado?", "rm -rf /");
    assert!(matches!(
        modelo(&[r#"{"value": "sim", "confidence": 0.9}"#])
            .decidir(&p)
            .await,
        Desfecho::SemDecisao { .. }
    ));
}

#[tokio::test]
async fn regras_em_conflito_nao_decidem() {
    let r = DecisorDeRegras {
        nome: "t".into(),
        regras: vec![
            RegraDeDecisao::de_json(
                &json!({"palavras": ["cargo"], "valor": "codigo", "confianca": 0.9}),
            )
            .unwrap(),
            RegraDeDecisao::de_json(
                &json!({"palavras": ["teste"], "valor": "pesquisa", "confianca": 0.9}),
            )
            .unwrap(),
        ],
    };
    assert!(matches!(
        r.decidir(&tipos()).await,
        Desfecho::SemDecisao { .. }
    ));
    let q = Questao::escolha("tipo", &["codigo", "pesquisa"], "rode o CARGO build");
    assert!(matches!(r.decidir(&q).await, Desfecho::Decidido(_)));
}

#[tokio::test]
async fn abaixo_do_limiar_sobe_ao_proximo_e_depois_a_pessoa() {
    let barato = modelo(&[r#"{"value": "codigo", "confidence": 0.4}"#]);
    let forte = modelo(&[r#"{"value": "pesquisa", "confidence": 0.95}"#]);
    let e = Escada {
        degraus: vec![
            Degrau {
                decisor: barato.clone(),
                limiar: 0.7,
            },
            Degrau {
                decisor: forte,
                limiar: 0.7,
            },
        ],
        pessoa: true,
    };
    let r = e.decidir(&tipos()).await;
    assert_eq!(r.trilha.len(), 2, "o incerto tem de subir: {r:?}");
    let Encaminhamento::Decidido(x) = &r.fim else {
        panic!("{r:?}")
    };
    assert_eq!(x.valor, Valor::Escolha("pesquisa".into()));

    // Os dois incertos: a pessoa no fim, e a resposta dela pela mesma conferencia.
    let e = Escada {
        degraus: vec![Degrau {
            decisor: modelo(&[r#"{"value": "codigo", "confidence": 0.4}"#]),
            limiar: 0.7,
        }],
        pessoa: true,
    };
    let r = e.decidir(&tipos()).await;
    assert!(matches!(r.fim, Encaminhamento::Pessoa { .. }), "{r:?}");
    assert!(matches!(
        resposta_da_pessoa(&tipos(), "2"),
        Desfecho::Decidido(Decidido { valor: Valor::Escolha(ref s), .. }) if s == "pesquisa"
    ));
    assert!(matches!(
        resposta_da_pessoa(&tipos(), "talvez"),
        Desfecho::SemDecisao { .. }
    ));
    let sem_pessoa = Escada {
        degraus: vec![],
        pessoa: false,
    };
    assert_eq!(
        sem_pessoa.decidir(&tipos()).await.fim,
        Encaminhamento::SemDecisao
    );
}

#[test]
fn decisao_nao_libera_capacidade_negada() {
    let concedidas: BTreeSet<String> = ["fs.read".to_string()].into();
    let liberar = Encaminhamento::Decidido(Decidido {
        valor: Valor::Escolha("permitir".into()),
        confianca: 1.0,
        decisor: "modelo:x".into(),
    });
    // Capacidade NAO concedida: a decisao «permitir» e recusada e o portao nega.
    let v = capacidade_sob_decisao(&concedidas, "shell.exec", None, &liberar);
    assert_eq!(v.decisao, NoPortao::Negar);
    assert!(
        v.recusa.is_some(),
        "liberar a negada tem de ser recusado: {v:?}"
    );
    // Regra que manda perguntar: «permitir» nao a afrouxa.
    let v = sobre_o_portao(NoPortao::Perguntar, NoPortao::Permitir);
    assert_eq!(v.decisao, NoPortao::Perguntar);
    let v = sobre_o_portao(NoPortao::Negar, NoPortao::Perguntar);
    assert_eq!(v.decisao, NoPortao::Negar);
    assert!(
        v.recusa.is_some(),
        "afrouxar tem de ser recusado com motivo"
    );
    // E endurece: concedida + decisao de cuidado vira perguntar; + negar vira negar.
    let cuidado = Encaminhamento::Decidido(Decidido {
        valor: Valor::Predicado(true),
        confianca: 0.9,
        decisor: "regras:t".into(),
    });
    let v = capacidade_sob_decisao(&concedidas, "fs.read", None, &cuidado);
    assert_eq!(v.decisao, NoPortao::Perguntar);
    assert_eq!(
        sobre_o_portao(NoPortao::Permitir, NoPortao::Negar).decisao,
        NoPortao::Negar
    );
    // Sem decisao: o comportamento de hoje.
    let v = capacidade_sob_decisao(&concedidas, "fs.read", None, &Encaminhamento::SemDecisao);
    assert_eq!(v.decisao, NoPortao::Permitir);
}
