//! Prova real nos dois sentidos, medida durante a escrita (02/10/2026):
//! em `fluxos.rs`, a morte por porta do `se` matava TODAS as portas junto com a
//! principal, e `fluxo_de_erro` saia `pulado` na porta `erro` de `saida_de_erro`;
//! `continuar_em_erro` e `fluxo_de_erro` reprovavam, e passaram com o alcance da morte
//! restrito a porta nao disparada. Os demais testes nasceram verdes junto do codigo e
//! NAO tiveram o defeito reposto um a um — divida registrada em SPRINTS.md (SP000035),
//! a fechar na onda 2.
//! PHX Flow Engine, onda 1 (SP000035): itens em vez de texto, nos de controle do motor
//! (`se`, `juntar`, `lote`, `parar_com_erro`), erro tratado, tetos, expressoes por caminho
//! JSON e fluxo de erro. Um teste por id da onda, mais o do comportamento VELHO: o fluxo
//! que so sabia texto continua lendo e rodando igual.
//!
//! A ferramenta `eco` e o modelo roteirizado sao os unicos dubles: cada chamada continua
//! passando pelo `Agent::call_tool`, e e isso que o teste de capacidade negada confere.

use phxclaw_agent::fluxos;
use phxclaw_agent::*;
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, Tool, ToolContext, ToolError,
    ToolOutput, ToolSpec,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-fluxo-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `eco`: devolve `texto` (texto cru, ou JSON quando nao e texto), depois de dormir
/// `dorme_ms`; com `falhar` nao vazio, falha com essa mensagem. Conta as chamadas.
struct Eco {
    chamadas: AtomicUsize,
}

impl Tool for Eco {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "eco".into(),
            description: "eco".into(),
            parameters: json!({"type":"object"}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            self.chamadas.fetch_add(1, Ordering::SeqCst);
            if let Some(ms) = args.get("dorme_ms").and_then(Value::as_u64) {
                tokio::time::sleep(Duration::from_millis(ms)).await;
            }
            // `falhar` vazio nao falha: e o que deixa uma passada por item falhar e a
            // vizinha nao, com o motivo vindo do proprio item (`{{lotes[0].f}}`).
            if let Some(m) = args
                .get("falhar")
                .and_then(Value::as_str)
                .filter(|m| !m.is_empty())
            {
                return Err(ToolError::Failed(m.into()));
            }
            Ok(ToolOutput::text(match args.get("texto") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            }))
        })
    }
}

/// Modelo roteirizado pelo objetivo: `[[eco:X]]` responde X; `[[falha]]` cai;
/// `[[lento:MS]]` dorme antes de responder.
struct PorObjetivo;

impl Llm for PorObjetivo {
    fn id(&self) -> String {
        "por-objetivo".into()
    }
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        _tools: &'a [ToolSpec],
        _o: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            let obj = messages
                .iter()
                .find(|m| m.role == Role::User)
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let marca = |nome: &str| {
                let i = obj.find(&format!("[[{nome}"))?;
                let resto = &obj[i + 2 + nome.len()..];
                let fim = resto.find("]]")?;
                Some(resto[..fim].trim_start_matches(':').to_string())
            };
            if let Some(ms) = marca("lento") {
                tokio::time::sleep(Duration::from_millis(ms.parse().unwrap())).await;
            }
            if marca("falha").is_some() {
                return Err(LlmError::Transport("provedor fora (roteiro)".into()));
            }
            Ok(ScriptedLlm::text(
                &marca("eco").unwrap_or_else(|| "nada".into()),
            ))
        })
    }
}

fn agente(caps: &[&str]) -> (Agent, Arc<Eco>) {
    let eco = Arc::new(Eco {
        chamadas: AtomicUsize::new(0),
    });
    let tools: Vec<Arc<dyn Tool>> =
        vec![eco.clone(), Arc::new(WriteFileTool), Arc::new(ReadFileTool)];
    let a = Agent::new(
        Arc::new(PorObjetivo),
        tools,
        AgentConfig::default().grant(caps),
        TaskStore::new(tmp("motor")).unwrap(),
    );
    (a, eco)
}

fn fluxo(v: Value) -> fluxos::Fluxo {
    fluxos::ler(&v.to_string()).unwrap()
}

fn passo<'a>(r: &'a fluxos::Relatorio, id: &str) -> &'a fluxos::Resultado {
    r.passos
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("passo {id} nao esta no relatorio: {r:#?}"))
}

/// O fluxo ANTIGO -- so texto, `{{id}}` inteiro, tarefa + ferramenta -- continua lendo e
/// rodando igual: a saida em texto e a mesma, o arquivo gravado e o mesmo, e cada passo
/// ganha um unico item de texto sem que ninguem tenha pedido.
#[tokio::test]
async fn comportamento_velho_fluxo_de_texto_roda_igual() {
    let (a, _) = agente(&["fs.write", "fs.read"]);
    let f = fluxo(json!({"nome":"velho","max_paralelo":4,"passos":[
        {"id":"grava","depende":["a","b"],"ferramenta":"write_file",
         "args":{"path":"junta.txt","content":"{{a}}+{{b}}"}},
        {"id":"a","tarefa":"[[eco:alfa]]"},
        {"id":"b","ferramenta":"eco","args":{"texto":"42"}},
        {"id":"le","depende":["grava"],"ferramenta":"read_file","args":{"path":"junta.txt"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "le").saida, "alfa+42");
    assert_eq!(
        passo(&r, "b").itens,
        vec![json!("42")],
        "texto nao vira numero"
    );
    assert_eq!(passo(&r, "a").itens, vec![json!("alfa")]);
    assert_eq!(
        std::fs::read_to_string(a.store.workdir(&r.tarefa).join("junta.txt")).unwrap(),
        "alfa+42"
    );
    // o relatorio antigo (sem `itens`) continua sendo lido pela retomada
    let velho: fluxos::Resultado = serde_json::from_value(json!({
        "id":"x","estado":"ok","saida":"texto","tarefa":null,"tentativas":1
    }))
    .unwrap();
    assert!(velho.itens.is_empty() && velho.portas.is_empty());
}

/// Nos de todo tipo ligados por conexoes com porta nomeada: ferramenta, agente e `se`;
/// a conexao `cond:verdadeiro` so dispara quando a porta tem itens, e porta que nao existe
/// ou `se` sem porta sao recusados na leitura.
#[tokio::test]
async fn nos_e_conexoes() {
    let (a, _) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"nos","passos":[
        {"id":"fonte","ferramenta":"eco","args":{"texto":{"status":"ok","n":1}}},
        {"id":"cond","depende":["fonte"],"se":{"caminho":"status","operador":"igual","valor":"ok"}},
        {"id":"sim","depende":["cond:verdadeiro"],"tarefa":"[[eco:deu {{cond.n}}]]"},
        {"id":"nao","depende":["cond:falso"],"ferramenta":"eco","args":{"texto":"nunca"}},
        {"id":"depois_do_nao","depende":["nao"],"ferramenta":"eco","args":{"texto":"nunca"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "sim").saida, "deu 1");
    assert_eq!(passo(&r, "nao").estado, "pulado");
    assert_eq!(
        passo(&r, "depois_do_nao").estado,
        "pulado",
        "pulo e transitivo"
    );
    let cond = passo(&r, "cond");
    assert_eq!(cond.portas["verdadeiro"].len(), 1);
    assert!(cond.portas["falso"].is_empty());

    let casos = [
        (
            json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco"},{"id":"b","depende":["a:erro"],"ferramenta":"eco"}]}),
            "porta 'erro' de 'a', que nao existe",
        ),
        (
            json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco"},
                {"id":"c","depende":["a"],"se":{"caminho":"","operador":"existe"}},
                {"id":"b","depende":["c"],"ferramenta":"eco"}]}),
            "diga a porta",
        ),
        (
            json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco","se":{"caminho":"","operador":"existe"}}]}),
            "exatamente um",
        ),
        (
            json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco"},
                {"id":"c","depende":["a"],"se":{"caminho":"","operador":"parecido"}}]}),
            "operador 'parecido' desconhecido",
        ),
    ];
    for (f, esperado) in casos {
        let e = fluxos::ler(&f.to_string()).unwrap_err();
        assert!(e.contains(esperado), "{f}: {e}");
    }
}

/// Lista de itens: um passo que recebe N itens roda uma vez por item com `por_item`, e
/// recebe a lista inteira sem ele; `{{lista}}` sozinho num argumento passa a LISTA a
/// ferramenta, nao um texto achatado.
#[tokio::test]
async fn execucao_por_item() {
    let (a, eco) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"itens","passos":[
        {"id":"lista","ferramenta":"eco","args":{"texto":[{"n":1},{"n":2},{"n":3}]}},
        {"id":"cada","depende":["lista"],"por_item":true,"ferramenta":"eco",
         "args":{"texto":"item {{lista.n}}"}},
        {"id":"todos","depende":["lista"],"ferramenta":"eco","args":{"texto":"segundo={{lista[1].n}}"}},
        {"id":"inteira","depende":["lista"],"ferramenta":"eco","args":{"texto":"{{lista}}"}},
        {"id":"agente_por_item","depende":["cada"],"por_item":true,"tarefa":"[[eco:visto {{cada}}]]"}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "lista").itens.len(), 3);
    assert_eq!(
        passo(&r, "cada").itens,
        vec![json!("item 1"), json!("item 2"), json!("item 3")]
    );
    assert_eq!(passo(&r, "todos").saida, "segundo=2");
    assert_eq!(
        passo(&r, "inteira").itens.len(),
        3,
        "a lista chegou como lista"
    );
    assert_eq!(passo(&r, "agente_por_item").itens.len(), 3);
    assert_eq!(passo(&r, "agente_por_item").itens[2], json!("visto item 3"));
    // 1 (lista) + 3 (cada) + 1 (todos) + 1 (inteira): tudo pelo portao
    assert_eq!(eco.chamadas.load(Ordering::SeqCst), 6);
    let filhas = a
        .store
        .list()
        .unwrap()
        .into_iter()
        .filter(|t| t.parent.as_deref() == Some(r.tarefa.as_str()))
        .count();
    assert_eq!(filhas, 3, "uma tarefa filha por item");
}

/// `se` por item: cada item vai para `verdadeiro` ou `falso`, os dois ramos rodam quando
/// os dois tem itens, e o ramo sem item e `pulado`.
#[tokio::test]
async fn ramificacao_if_switch() {
    let (a, _) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"se","passos":[
        {"id":"lista","ferramenta":"eco","args":{"texto":[{"v":5},{"v":15},{"v":25}]}},
        {"id":"grande","depende":["lista"],"se":{"caminho":"v","operador":"maior","valor":10}},
        {"id":"grandes","depende":["grande:verdadeiro"],"ferramenta":"eco","args":{"texto":"{{grande}}"}},
        {"id":"pequenos","depende":["grande:falso"],"ferramenta":"eco","args":{"texto":"{{grande}}"}},
        {"id":"nenhum","depende":["lista"],"se":{"caminho":"v","operador":"existe"}},
        {"id":"sem_v","depende":["nenhum:falso"],"ferramenta":"eco","args":{"texto":"x"}},
        {"id":"com_v","depende":["nenhum:verdadeiro"],"ferramenta":"eco","args":{"texto":"x"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(
        passo(&r, "grandes").itens,
        vec![json!({"v":15}), json!({"v":25})]
    );
    assert_eq!(passo(&r, "pequenos").itens, vec![json!({"v":5})]);
    assert_eq!(passo(&r, "sem_v").estado, "pulado");
    assert_eq!(passo(&r, "com_v").estado, "ok");
}

/// `juntar` nos tres modos: `append` concatena na ordem de `depende`, `chave` combina os
/// itens das duas entradas pelo campo, `ramo` fica com o ramo que sobreviveu ao `se` --
/// e so pula quando todos os ramos morreram.
#[tokio::test]
async fn juncao_merge() {
    let (a, _) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"juntar","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":[{"id":1,"x":"a"},{"id":2,"x":"b"}]}},
        {"id":"b","ferramenta":"eco","args":{"texto":[{"id":2,"y":"B"},{"id":3,"y":"C"}]}},
        {"id":"tudo","depende":["a","b"],"juntar":{"modo":"append"}},
        {"id":"casados","depende":["a","b"],"juntar":{"modo":"chave","chave":"id"}},
        {"id":"cond","depende":["a"],"se":{"caminho":"x","operador":"igual","valor":"zzz"}},
        {"id":"ramo_sim","depende":["cond:verdadeiro"],"ferramenta":"eco","args":{"texto":"sim"}},
        {"id":"ramo_nao","depende":["cond:falso"],"ferramenta":"eco","args":{"texto":"nao"}},
        {"id":"vivo","depende":["ramo_sim","ramo_nao"],"juntar":{"modo":"ramo"}},
        {"id":"so_morto","depende":["ramo_sim"],"juntar":{"modo":"append"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "tudo").itens.len(), 4);
    assert_eq!(passo(&r, "tudo").itens[0], json!({"id":1,"x":"a"}));
    assert_eq!(
        passo(&r, "casados").itens,
        vec![json!({"id":2,"x":"b","y":"B"})]
    );
    assert_eq!(passo(&r, "ramo_sim").estado, "pulado");
    assert_eq!(passo(&r, "vivo").itens, vec![json!("nao")]);
    assert_eq!(passo(&r, "so_morto").estado, "pulado");
    let e = fluxos::ler(
        &json!({"nome":"j","passos":[{"id":"a","ferramenta":"eco"},
            {"id":"j","depende":["a"],"juntar":{"modo":"chave","chave":"id"}}]})
        .to_string(),
    )
    .unwrap_err();
    assert!(e.contains("exatamente duas"), "{e}");
}

/// `lote`: a entrada vira itens-lote de N, e o passo seguinte com `por_item` faz uma
/// passada por lote -- o laco do n8n sem ciclo no grafo. E o teste central da onda:
/// `se` + `lote` + `continuar_em_erro` no mesmo fluxo, com a passada que falha nao
/// derrubando as outras.
///
/// Prova real (06/10): a versao anterior fazia TODAS as passadas falharem, e por isso
/// passava com o motor jogando fora as passadas boas quando uma falhava -- o contrario do
/// que este comentario afirma. Agora uma passada falha e as outras nao; com o
/// `parcial()` reposto como «so a primeira falha» (`fluxos.rs`, ramo `Continuar` usando
/// `item_de_erro`), `roda.itens` volta com 1 item e o teste cai (RED medido).
#[tokio::test]
async fn laco_lotes() {
    let (a, eco) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"lotes","passos":[
        {"id":"lista","ferramenta":"eco","args":{"texto":[1,2,3,4,5]}},
        {"id":"lotes","depende":["lista"],"lote":2},
        {"id":"passada","depende":["lotes"],"por_item":true,"ferramenta":"eco",
         "args":{"texto":"lote {{lotes}}"}},
        {"id":"primeiro","depende":["lotes"],"ferramenta":"eco","args":{"texto":"{{lotes[0][1]}}"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "lotes").itens.len(), 3);
    assert_eq!(passo(&r, "lotes").itens[2], json!([5]));
    assert_eq!(
        passo(&r, "passada").itens,
        vec![json!("lote [1,2]"), json!("lote [3,4]"), json!("lote [5]")]
    );
    assert_eq!(passo(&r, "primeiro").saida, "2");
    assert_eq!(eco.chamadas.load(Ordering::SeqCst), 1 + 3 + 1);

    // o fluxo central da onda: se + lote + continuar em erro, com UMA passada falhando
    // (o lote [4,5]) e as outras duas boas
    let central = |ao_errar: &str| {
        let mut v = json!({"nome":"central","passos":[
            {"id":"lista","ferramenta":"eco","args":{"texto":[
                {"n":1,"f":""},{"n":2,"f":""},{"n":3,"f":""},{"n":4,"f":"4 nao vale"},
                {"n":5,"f":""},{"n":6,"f":""},{"n":7,"f":""}]}},
            {"id":"par","depende":["lista"],"se":{"caminho":"n","operador":"contem","valor":"x"}},
            {"id":"lotes","depende":["par:falso"],"lote":3},
            {"id":"roda","depende":["lotes"],"por_item":true,"ao_errar":ao_errar,"ferramenta":"eco",
             "args":{"texto":"{{lotes[0].n}}","falhar":"{{lotes[0].f}}"}},
            {"id":"fim","depende":["roda"],"ferramenta":"eco","args":{"texto":"{{roda}}"}},
            {"id":"trata","depende":["roda:erro"],"ferramenta":"eco","args":{"texto":"{{roda}}"}}
        ]});
        // a porta `erro` so existe com saida_de_erro; nos outros, `trata` sai do fluxo
        if ao_errar != "saida_de_erro" {
            v["passos"].as_array_mut().unwrap().pop();
        }
        fluxo(v)
    };
    let (a, eco) = agente(&["fs.read"]);
    let r = fluxos::rodar(&a, &central("continuar")).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    let roda = passo(&r, "roda");
    assert_eq!(roda.estado, "continuou");
    assert_eq!(roda.itens.len(), 3, "as duas passadas boas ficaram: {roda:#?}");
    assert_eq!(roda.itens[0], json!("1"));
    assert_eq!(roda.itens[1]["erro"], json!("4 nao vale"));
    assert_eq!(roda.itens[1]["item"], json!(1));
    assert_eq!(roda.itens[2], json!("7"));
    assert_eq!(passo(&r, "fim").estado, "ok");
    // 1 (lista) + 3 passadas + 1 (fim)
    assert_eq!(eco.chamadas.load(Ordering::SeqCst), 5);

    // saida_de_erro: as boas pela principal, a que falhou pela porta `erro` -- e os DOIS
    // lados disparam
    let (a, _) = agente(&["fs.read"]);
    let r = fluxos::rodar(&a, &central("saida_de_erro")).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    let roda = passo(&r, "roda");
    assert_eq!(roda.itens, vec![json!("1"), json!("7")]);
    assert_eq!(roda.portas["erro"].len(), 1);
    assert_eq!(roda.portas["erro"][0]["erro"], json!("4 nao vale"));
    assert_eq!(passo(&r, "fim").estado, "ok");
    assert_eq!(passo(&r, "trata").estado, "ok");

    // parar (padrao): uma passada falha, o passo falha -- nao ha meio sucesso calado
    let (a, _) = agente(&["fs.read"]);
    let r = fluxos::rodar(&a, &central("parar")).await.unwrap();
    assert!(!r.sucesso);
    assert_eq!(passo(&r, "roda").estado, "falhou");
    assert_eq!(passo(&r, "roda").saida, "4 nao vale");
    assert_eq!(passo(&r, "fim").estado, "bloqueado");
}

/// `ao_errar`: `parar` (padrao) bloqueia os dependentes; `continuar` segue com o item de
/// erro na saida principal; `saida_de_erro` esvazia a principal (dependente pula) e manda
/// o item pela porta `erro`. Com sucesso, a porta `erro` e a que fica vazia. As tentativas
/// continuam valendo antes de qualquer um dos tres.
#[tokio::test]
async fn continuar_em_erro() {
    let (a, eco) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"erros","passos":[
        {"id":"para","ferramenta":"eco","tentativas":2,"args":{"falhar":"quebrou"}},
        {"id":"apos_para","depende":["para"],"ferramenta":"eco","args":{"texto":"x"}},
        {"id":"segue","ferramenta":"eco","ao_errar":"continuar","args":{"falhar":"tropecou"}},
        {"id":"apos_segue","depende":["segue"],"ferramenta":"eco","args":{"texto":"viu {{segue.erro}}"}},
        {"id":"desvia","ferramenta":"eco","ao_errar":"saida_de_erro","args":{"falhar":"desviou"}},
        {"id":"apos_desvia","depende":["desvia"],"ferramenta":"eco","args":{"texto":"x"}},
        {"id":"trata","depende":["desvia:erro"],"ferramenta":"eco","args":{"texto":"tratei {{desvia.erro}}"}},
        {"id":"bem","ferramenta":"eco","ao_errar":"saida_de_erro","args":{"texto":"ok"}},
        {"id":"apos_bem","depende":["bem"],"ferramenta":"eco","args":{"texto":"{{bem}}"}},
        {"id":"trata_bem","depende":["bem:erro"],"ferramenta":"eco","args":{"texto":"x"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(!r.sucesso, "o passo com `parar` falhou");
    let para = passo(&r, "para");
    assert_eq!((para.estado.as_str(), para.tentativas), ("falhou", 2));
    assert_eq!(passo(&r, "apos_para").estado, "bloqueado");
    assert_eq!(passo(&r, "segue").estado, "continuou");
    let viu = &passo(&r, "apos_segue").saida;
    assert!(viu.starts_with("viu ") && viu.contains("tropecou"), "{viu}");
    assert_eq!(passo(&r, "desvia").estado, "continuou");
    assert_eq!(passo(&r, "apos_desvia").estado, "pulado");
    let tratei = &passo(&r, "trata").saida;
    assert!(
        tratei.starts_with("tratei ") && tratei.contains("desviou"),
        "{tratei}"
    );
    assert_eq!(passo(&r, "apos_bem").saida, "ok");
    assert_eq!(passo(&r, "trata_bem").estado, "pulado");
    assert_eq!(
        eco.chamadas.load(Ordering::SeqCst),
        2 + 1 + 1 + 1 + 1 + 1 + 1
    );

    // sem o passo `parar`, o fluxo com erros tratados e um fluxo que deu certo
    let (a, _) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"tratado","passos":[
        {"id":"segue","ferramenta":"eco","ao_errar":"continuar","args":{"falhar":"tropecou"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(
        a.store.load(&r.tarefa).unwrap().status,
        TaskStatus::Completed
    );
}

/// Teto por passo (por tentativa) e por fluxo: estourar e falha do passo com o motivo
/// dizendo QUAL teto; o que nao chegou a rodar depois do teto do fluxo tambem diz isso.
#[tokio::test]
async fn timeout_execucao() {
    let (a, _) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"teto-passo","passos":[
        {"id":"lento","ferramenta":"eco","teto_ms":50,"tentativas":2,"args":{"dorme_ms":400,"texto":"x"}},
        {"id":"agente_lento","tarefa":"[[lento:400]] [[eco:x]]","teto_ms":50},
        {"id":"rapido","ferramenta":"eco","teto_ms":500,"args":{"texto":"x"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(!r.sucesso);
    let lento = passo(&r, "lento");
    assert_eq!((lento.estado.as_str(), lento.tentativas), ("falhou", 2));
    assert_eq!(lento.saida, "teto do passo (50 ms) estourou");
    assert_eq!(
        passo(&r, "agente_lento").saida,
        "teto do passo (50 ms) estourou"
    );
    assert_eq!(passo(&r, "rapido").estado, "ok");

    let (a, _) = agente(&["fs.read"]);
    let f = fluxo(
        json!({"nome":"teto-fluxo","teto_ms":200,"max_paralelo":1,"passos":[
            {"id":"um","ferramenta":"eco","args":{"dorme_ms":150,"texto":"x"}},
            {"id":"dois","depende":["um"],"ferramenta":"eco","args":{"dorme_ms":150,"texto":"x"}},
            {"id":"tres","depende":["dois"],"ferramenta":"eco","args":{"texto":"x"}}
        ]}),
    );
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(!r.sucesso);
    assert_eq!(passo(&r, "um").estado, "ok");
    assert_eq!(passo(&r, "dois").saida, "teto do fluxo (200 ms) estourou");
    let tres = passo(&r, "tres");
    assert_eq!(tres.estado, "falhou");
    assert!(
        tres.saida.contains("teto do fluxo (200 ms)"),
        "{}",
        tres.saida
    );
    let mae = a.store.load(&r.tarefa).unwrap();
    assert!(mae.error.unwrap().contains("teto do fluxo"));
    assert!(
        fluxos::ler(
            &json!({"nome":"z","teto_ms":0,"passos":[{"id":"a","ferramenta":"eco"}]}).to_string()
        )
        .unwrap_err()
        .contains("maior que zero")
    );
}

/// Expressoes por caminho JSON, sem JS: `{{p.campo.sub[0]}}` em texto e em argumento,
/// `{{p}}` inteiro continua valendo, caminho que nao existe falha o passo com o motivo, e
/// referencia a passo fora de `depende` continua recusada na leitura.
#[tokio::test]
async fn expressoes() {
    let (a, _) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"expr","passos":[
        {"id":"p","ferramenta":"eco","args":{"texto":{"campo":{"sub":[{"nome":"zero"},"um"]},"n":7}}},
        {"id":"texto","depende":["p"],"tarefa":"[[eco:{{p.campo.sub[0].nome}}/{{ p.campo.sub[1] }}/{{p.n}}]]"},
        {"id":"valor","depende":["p"],"ferramenta":"eco","args":{"texto":{"lista":"{{p.campo.sub}}","n":"{{p.n}}"}}},
        {"id":"inteiro","depende":["p"],"ferramenta":"eco","args":{"texto":"{{p}}"}},
        {"id":"nao_existe","depende":["p"],"tentativas":3,"ferramenta":"eco","args":{"texto":"{{p.campo.outro}}"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert_eq!(passo(&r, "texto").saida, "zero/um/7");
    assert_eq!(
        passo(&r, "valor").itens[0],
        json!({"lista":[{"nome":"zero"},"um"],"n":7}),
        "expressao sozinha vira o valor, com o tipo"
    );
    assert_eq!(passo(&r, "inteiro").itens[0]["n"], json!(7));
    let ne = passo(&r, "nao_existe");
    assert_eq!(ne.estado, "falhou");
    assert_eq!(ne.tentativas, 1, "erro de definicao nao se tenta de novo");
    assert!(
        ne.saida
            .contains("o caminho 'campo.outro' nao existe na saida de 'p'"),
        "{}",
        ne.saida
    );
    let e = fluxos::ler(
        &json!({"nome":"r","passos":[{"id":"a","ferramenta":"eco"},
            {"id":"b","ferramenta":"eco","args":{"texto":"{{a.x[0]}}"}}]})
        .to_string(),
    )
    .unwrap_err();
    assert!(e.contains("sem declarar 'a'"), "{e}");
}

/// `parar_com_erro` falha o fluxo com a mensagem montada, e o `fluxo_de_erro` roda pelo
/// mesmo portao com `{{erro}}` (passos falhos e motivos); o fluxo continua falho, e o passo
/// de erro nao roda quando o fluxo da certo. Passo de erro com `depende` e recusado.
#[tokio::test]
async fn fluxo_de_erro() {
    let (a, eco) = agente(&["fs.read", "fs.write"]);
    let f = fluxo(json!({"nome":"erro","fluxo_de_erro":"avisa","passos":[
        {"id":"p","ferramenta":"eco","args":{"texto":{"saldo":-3}}},
        {"id":"negativo","depende":["p"],"se":{"caminho":"saldo","operador":"menor","valor":0}},
        {"id":"para","depende":["negativo:verdadeiro"],"parar_com_erro":"saldo {{negativo.saldo}} nao serve"},
        {"id":"depois","depende":["para"],"ferramenta":"eco","args":{"texto":"nunca"}},
        {"id":"avisa","ferramenta":"write_file","args":{"path":"erro.txt","content":"{{erro.fluxo}}: {{erro.passos[0].passo}} -> {{erro.passos[0].motivo}}"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(!r.sucesso);
    assert_eq!(passo(&r, "para").estado, "falhou");
    assert_eq!(passo(&r, "para").saida, "saldo -3 nao serve");
    assert_eq!(passo(&r, "depois").estado, "bloqueado");
    assert_eq!(passo(&r, "avisa").estado, "ok");
    assert_eq!(
        std::fs::read_to_string(a.store.workdir(&r.tarefa).join("erro.txt")).unwrap(),
        "erro: para -> saldo -3 nao serve"
    );
    assert_eq!(a.store.load(&r.tarefa).unwrap().status, TaskStatus::Failed);
    assert_eq!(eco.chamadas.load(Ordering::SeqCst), 1);

    // fluxo que da certo: o passo de erro nao roda nem aparece como rodado
    let (a, _) = agente(&["fs.read", "fs.write"]);
    let f = fluxo(json!({"nome":"bem","fluxo_de_erro":"avisa","passos":[
        {"id":"p","ferramenta":"eco","args":{"texto":"ok"}},
        {"id":"avisa","ferramenta":"write_file","args":{"path":"erro.txt","content":"{{erro}}"}}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert!(r.passos.iter().all(|p| p.id != "avisa"));
    assert!(!a.store.workdir(&r.tarefa).join("erro.txt").exists());

    // o fluxo de erro como tarefa de agente, pelo laco unico
    let (a, _) = agente(&["fs.read"]);
    let f = fluxo(json!({"nome":"agente","fluxo_de_erro":"avisa","passos":[
        {"id":"p","ferramenta":"eco","args":{"falhar":"caiu"}},
        {"id":"avisa","tarefa":"[[eco:aviso: {{erro.passos[0].motivo}}]]"}
    ]}));
    let r = fluxos::rodar(&a, &f).await.unwrap();
    let aviso = &passo(&r, "avisa").saida;
    assert!(
        aviso.starts_with("aviso: ") && aviso.contains("caiu"),
        "{aviso}"
    );
    assert!(passo(&r, "avisa").tarefa.is_some());

    let casos = [
        (
            json!({"nome":"e","fluxo_de_erro":"x","passos":[{"id":"a","ferramenta":"eco"}]}),
            "nao existe",
        ),
        (
            json!({"nome":"e","fluxo_de_erro":"b","passos":[{"id":"a","ferramenta":"eco"},
                {"id":"b","depende":["a"],"ferramenta":"eco"}]}),
            "nao pode ter depende",
        ),
        (
            json!({"nome":"e","fluxo_de_erro":"b","passos":[{"id":"a","depende":["b"],"ferramenta":"eco"},
                {"id":"b","ferramenta":"eco"}]}),
            "ninguem pode depender",
        ),
        (
            json!({"nome":"e","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"{{erro}}"}}]}),
            "sem declarar 'erro'",
        ),
    ];
    for (f, esperado) in casos {
        let e = fluxos::ler(&f.to_string()).unwrap_err();
        assert!(e.contains(esperado), "{f}: {e}");
    }
}
