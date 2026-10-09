//! Frente F3 do motor de fluxo: a galeria de modelos, o rascunho x publicada e o git dos
//! fluxos (`fluxo_modelos`, `fluxo_versoes`, `fluxo_git`).
//!
//! Cada guarda abaixo tem o defeito que a motivou e o RED medido (o defeito reposto, o teste
//! caindo) no comentario do teste.

mod comum_fluxo;

use chrono::Utc;
use comum_fluxo::*;
use phxclaw_agent::TaskStatus;
use phxclaw_agent::agenda::ScheduleSpec;
use phxclaw_agent::api::disparar_agenda_com_handles;
use phxclaw_agent::fluxo_git::{self, Ambiente, Efeito};
use phxclaw_agent::fluxo_modelos;
use phxclaw_agent::fluxo_versoes;
use phxclaw_agent::fluxos;
use phxclaw_agent::gatilhos::{GatilhoDeWebhook, Gatilhos};
use phxclaw_test_support::pulado;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn fluxo_eco(texto: &str) -> Value {
    json!({"nome":"f","passos":[{"id":"a","ferramenta":"eco","args":{"texto":texto}}]})
}

/// O que o passo `a` produziu na execucao que o gatilho criou.
async fn saida_do_disparo(s: &phxclaw_agent::api::ApiState, id: &str) -> (String, String) {
    let t = ate_o_estado(s, id, &[TaskStatus::Completed, TaskStatus::Failed]).await;
    assert_eq!(t.status, TaskStatus::Completed, "{t:#?}");
    let r = relatorio(&t);
    (passo(&r, "a").saida.clone(), r.fluxo_sha256.clone())
}

fn gatilho_para(arq: &Path) -> Gatilhos {
    Gatilhos {
        arquivos: vec![],
        webhooks: vec![GatilhoDeWebhook {
            nome: "g".into(),
            objetivo: String::new(),
            fluxo: Some(arq.to_string_lossy().into_owned()),
            segredo: None,
            segredo_formulario: None,
        }],
    }
}

async fn disparo(base: &str) -> String {
    let r = reqwest::Client::new()
        .post(format!("{base}/v1/triggers/g"))
        .bearer_auth(TOKEN)
        .body("{}")
        .send()
        .await
        .unwrap();
    let status = r.status();
    let v: Value = r.json().await.unwrap();
    assert_eq!(status, 202, "{v}");
    v["id"].as_str().unwrap().to_string()
}

// ---------------------------------------------------------------- galeria

/// A galeria tem pelo menos oito modelos, e cada um passa pela validacao do motor de fluxo
/// (`fluxos::ler`, a mesma de qualquer arquivo) depois de serializado como `usar` o grava.
#[test]
fn galeria_tem_oito_modelos_validos_pelo_motor() {
    let g = fluxo_modelos::galeria().unwrap();
    assert!(g.len() >= 8, "so {} modelos", g.len());
    for m in &g {
        let texto = serde_json::to_string_pretty(&m.fluxo).unwrap();
        let f = fluxos::ler(&texto).unwrap_or_else(|e| panic!("{}: {e}", m.nome));
        assert_eq!(f.nome, m.nome);
        assert!(!m.descricao.trim().is_empty(), "{}", m.nome);
        assert!(!m.etiquetas.is_empty(), "{}: sem etiqueta", m.nome);
        // as etiquetas do cabecalho chegam ao fluxo, para `listar --etiqueta` achar o modelo
        for e in &m.etiquetas {
            assert!(f.etiquetas.contains(e), "{}: etiqueta {e}", m.nome);
        }
    }
    // os nomes sao unicos
    let mut nomes: Vec<_> = g.iter().map(|m| m.nome.clone()).collect();
    nomes.sort();
    nomes.dedup();
    assert_eq!(nomes.len(), g.len());
}

/// Modelo novo na pasta `modelos/fluxos/` que ninguem registrou na lista embutida reprova:
/// a galeria que existe so no disco nao chega ao binario instalado.
#[test]
fn galeria_embutida_e_a_pasta() {
    let pasta = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modelos/fluxos");
    let mut no_disco: Vec<String> = std::fs::read_dir(&pasta)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_string_lossy()
                .strip_suffix(".json")
                .map(str::to_string)
        })
        .collect();
    no_disco.sort();
    let mut embutidos: Vec<String> = fluxo_modelos::nomes()
        .into_iter()
        .map(str::to_string)
        .collect();
    embutidos.sort();
    assert_eq!(embutidos, no_disco);
}

fn tem_chave_de_segredo(v: &Value) -> bool {
    match v {
        Value::Object(o) => o
            .iter()
            .any(|(k, x)| phxclaw_types::segredo::nome_de_segredo(k) || tem_chave_de_segredo(x)),
        Value::Array(a) => a.iter().any(tem_chave_de_segredo),
        _ => false,
    }
}

/// Nenhum arquivo da galeria traz credencial: nem pela forma do valor (motor unico
/// `phxclaw_types::segredo`), nem campo com nome de segredo.
///
/// RED medido: a guarda `texto_tem_credencial(texto)` e a `variavel_parece_segredo` retiradas
/// de `ler_modelo` -- `modelo_com_credencial_e_recusado` cai (as seis variantes passam a ser
/// aceitas). Esta varredura roda sobre os arquivos do DISCO, nao sobre o que o parser aceita.
#[test]
fn nenhum_modelo_da_galeria_traz_valor_de_credencial() {
    let pasta = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modelos/fluxos");
    let mut n = 0;
    for e in std::fs::read_dir(&pasta).unwrap().flatten() {
        let texto = std::fs::read_to_string(e.path()).unwrap();
        assert!(
            !phxclaw_types::segredo::texto_tem_credencial(&texto),
            "{:?} traz credencial pela forma",
            e.path()
        );
        let v: Value = serde_json::from_str(&texto).unwrap();
        assert!(
            !tem_chave_de_segredo(&v["fluxo"]),
            "{:?} tem campo com nome de segredo no fluxo",
            e.path()
        );
        // o cabecalho so declara NOMES de credencial, e nome nao e valor
        for c in v["modelo"]["credenciais"].as_array().unwrap() {
            let c = c.as_str().unwrap();
            assert!(
                c.len() <= 64
                    && c.chars().all(|x| x.is_ascii_lowercase()
                        || x.is_ascii_digit()
                        || x == '-'
                        || x == '_'),
                "credencial {c:?}"
            );
        }
        n += 1;
    }
    assert!(n >= 8);
}

fn modelo_com(credenciais: Value, ferramenta: Value, variaveis: Value) -> String {
    json!({
        "modelo": {"nome":"teste-modelo","descricao":"d","etiquetas":["t"],"credenciais":credenciais},
        "fluxo": {"nome":"teste-modelo","variaveis":variaveis,"passos":[
            {"id":"a","ferramenta":ferramenta["nome"],"args":ferramenta["args"]}]}
    })
    .to_string()
}

/// O modelo que traz o VALOR de uma credencial e recusado, de seis jeitos: chave de provedor
/// num argumento, nome de segredo como campo (mesmo com valor inofensivo), variavel com forma
/// de segredo, URL com senha, Bearer no cabecalho e JWT.
#[test]
fn modelo_com_credencial_e_recusado() {
    let ok = modelo_com(
        json!([]),
        json!({"nome":"read_file","args":{"path":"a.txt"}}),
        json!({}),
    );
    assert!(
        fluxo_modelos::ler_modelo(&ok).is_ok(),
        "{:?}",
        fluxo_modelos::ler_modelo(&ok)
    );
    let casos = [
        modelo_com(
            json!([]),
            json!({"nome":"read_file","args":{"path":"a.txt","nota":"ghp_0123456789abcdefABCDEF"}}),
            json!({}),
        ),
        modelo_com(
            json!([]),
            json!({"nome":"read_file","args":{"path":"a.txt","api_key":"qualquer"}}),
            json!({}),
        ),
        modelo_com(
            json!([]),
            json!({"nome":"read_file","args":{"path":"a.txt"}}),
            json!({"senha":"x"}),
        ),
        modelo_com(
            json!([]),
            json!({"nome":"read_file","args":{"path":"https://u:senha123@host/x"}}),
            json!({}),
        ),
        modelo_com(
            json!([]),
            json!({"nome":"read_file","args":{"path":"a.txt","h":"Authorization: Bearer sk-0123456789abcdef"}}),
            json!({}),
        ),
        // o VALOR no lugar do nome, na lista de credenciais
        modelo_com(
            json!(["ghp_0123456789abcdefABCDEF"]),
            json!({"nome":"read_file","args":{"path":"a.txt"}}),
            json!({}),
        ),
    ];
    for (i, c) in casos.iter().enumerate() {
        let e = fluxo_modelos::ler_modelo(c).unwrap_err();
        assert!(
            e.contains("credencial") || e.contains("segredo"),
            "caso {i} recusado por outro motivo: {e}"
        );
    }
}

/// O modelo que USA ferramenta com credencial e nao declara o nome dela mente por omissao e
/// e recusado; declarando, passa. A galeria inteira cumpre a regra.
#[test]
fn modelo_declara_a_credencial_que_a_ferramenta_exige() {
    let args = json!({"nome":"send_email","args":{"to":["a@b.c"],"subject":"s","body":"b"}});
    let sem = modelo_com(json!([]), args.clone(), json!({}));
    let e = fluxo_modelos::ler_modelo(&sem).unwrap_err();
    assert!(e.contains("smtp"), "{e}");
    let com = modelo_com(json!(["smtp"]), args, json!({}));
    assert!(fluxo_modelos::ler_modelo(&com).is_ok());
    for m in fluxo_modelos::galeria().unwrap() {
        for c in fluxo_modelos::credenciais_das_ferramentas(&m.fluxo) {
            assert!(m.credenciais.iter().any(|d| d == c), "{}: {c}", m.nome);
        }
    }
}

/// `usar` grava o fluxo do modelo (so o fluxo, com as etiquetas) onde a pessoa mandou, o
/// motor o le de volta, e nao sobrescreve o que ja existe.
#[test]
fn usar_copia_o_modelo_para_o_projeto_e_nao_sobrescreve() {
    let d = tmp("usar");
    let destino = d.join("meu.json");
    let m = fluxo_modelos::usar("triagem-por-prioridade", &destino).unwrap();
    let f = fluxos::ler_arquivo(&destino).unwrap();
    assert_eq!(f.nome, m.nome);
    assert!(f.etiquetas.contains(&"triagem".to_string()));
    let texto = std::fs::read_to_string(&destino).unwrap();
    assert!(
        !texto.contains("\"modelo\""),
        "o cabecalho nao vai para o projeto"
    );
    // por cima: recusa e o arquivo nao muda
    std::fs::write(&destino, texto.replace("triagem-por-prioridade", "editado")).unwrap();
    let e = fluxo_modelos::usar("triagem-por-prioridade", &destino).unwrap_err();
    assert!(e.contains("ja existe"), "{e}");
    assert!(
        std::fs::read_to_string(&destino)
            .unwrap()
            .contains("editado")
    );
    // modelo que nao existe lista os que existem
    let e = fluxo_modelos::usar("nao-existe", &d.join("x.json")).unwrap_err();
    assert!(e.contains("ponte-n8n"), "{e}");
    assert!(!d.join("x.json").exists());
    // o fluxo copiado aparece na listagem pela etiqueta do modelo
    let outro = d.join("outro");
    std::fs::create_dir_all(&outro).unwrap();
    fluxo_modelos::usar("ponte-n8n", &outro.join("p.json")).unwrap();
    let (achados, erros) = fluxos::listar(&outro, Some("n8n"), None);
    assert!(
        erros.is_empty() && achados.len() == 1,
        "{achados:?} {erros:?}"
    );
}

// ---------------------------------------------------------------- publicada x rascunho

/// O GATILHO roda a versao publicada, nao o rascunho: publica "v1", edita o rascunho para
/// "rascunho", dispara o webhook e a execucao devolve "v1"; o hash do relatorio e o da versao.
///
/// RED medido: `fluxo_versoes::criar_fluxo_publicado` passando o caminho do rascunho a
/// `criar_fluxo_com` (marcado `// REPOSTO`) -- a execucao devolve "rascunho" e o teste cai.
#[tokio::test]
async fn gatilho_roda_a_publicada_e_nao_o_rascunho() {
    let raiz = tmp("gatilho-publicada");
    let s = estado(&raiz);
    let arq = gravar(&raiz, "f", &fluxo_eco("v1"));
    let v1 = fluxo_versoes::publicar(&arq, "primeira").unwrap();
    assert_eq!(v1.numero, 1);
    gravar(&raiz, "f", &fluxo_eco("rascunho"));
    let base = servir(&s, gatilho_para(&arq)).await;

    let (saida, sha) = saida_do_disparo(&s, &disparo(&base).await).await;
    assert_eq!(saida, "v1", "o gatilho rodou o rascunho");
    assert_eq!(
        sha, v1.sha256,
        "o hash do relatorio e o da versao publicada"
    );
    assert_eq!(
        fluxo_versoes::versao_do_sha(&arq, &sha).unwrap(),
        Some(1),
        "o relatorio aponta a versao"
    );

    // publicar o rascunho muda o que o gatilho roda, sem reiniciar o servidor
    let v2 = fluxo_versoes::publicar(&arq, "segunda").unwrap();
    assert_eq!(v2.numero, 2);
    let (saida, sha) = saida_do_disparo(&s, &disparo(&base).await).await;
    assert_eq!(saida, "rascunho");
    assert_eq!(sha, v2.sha256);
}

/// O irmao do gatilho: a AGENDA dispara a publicada, e o SUB-FLUXO tambem.
///
/// RED medido: o `publicada_do_disparo` do `agenda::due` devolvendo a copia sem trocar o
/// caminho (`// REPOSTO`) -- a agenda roda o rascunho; e o `ler_fluxo` do `subfluxo.rs`
/// voltando a `fluxos::ler_arquivo` -- o sub-fluxo roda o rascunho.
#[tokio::test]
async fn agenda_e_subfluxo_rodam_a_publicada() {
    let raiz = tmp("irmaos");
    let s = estado(&raiz);
    let arq = gravar(&raiz, "f", &fluxo_eco("v1"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    gravar(&raiz, "f", &fluxo_eco("rascunho"));

    s.agenda
        .lock()
        .unwrap()
        .add_fluxo(
            "noturno",
            &arq.to_string_lossy(),
            ScheduleSpec::EverySeconds(60),
            Utc::now() - chrono::Duration::minutes(2),
        )
        .unwrap();
    for h in disparar_agenda_com_handles(&s) {
        h.await.unwrap();
    }
    let id = s.agenda.lock().unwrap().items[0].last_task.clone().unwrap();
    let (saida, _) = saida_do_disparo(&s, &id).await;
    assert_eq!(saida, "v1", "a agenda rodou o rascunho");

    // o sub-fluxo, pela ferramenta `fluxo` com o nome na pasta de fluxos
    let pasta = raiz.join("fluxos");
    std::fs::create_dir_all(&pasta).unwrap();
    let filho = gravar(
        &pasta,
        "filho",
        &json!({"nome":"filho","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":"filho v1"}}]}),
    );
    fluxo_versoes::publicar(&filho, "").unwrap();
    gravar(
        &pasta,
        "filho",
        &json!({"nome":"filho","passos":[
        {"id":"a","ferramenta":"eco","args":{"texto":"filho rascunho"}}]}),
    );
    let b = banca_em(&raiz.join("sub"), Some(&pasta));
    let pai = fluxo(json!({"nome":"pai","passos":[
        {"id":"s","ferramenta":"fluxo","args":{"nome":"filho"}}]}));
    let r = fluxos::rodar(&b.a, &pai).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    assert_eq!(passo(&r, "s").itens, vec![json!("filho v1")]);
}

/// O comportamento VELHO: fluxo que nunca foi publicado e o publicado implicito -- o gatilho
/// roda o arquivo, editar vale na proxima execucao, e nenhuma pasta de versoes nasce.
///
/// RED medido: `ler_indice` tratando a ausencia da pasta de versoes como erro (`// REPOSTO`)
/// -- o gatilho do fluxo antigo passa a devolver 500 e este teste cai.
#[tokio::test]
async fn fluxo_sem_versao_e_publicado_implicito() {
    let raiz = tmp("implicito");
    let s = estado(&raiz);
    let arq = gravar(&raiz, "f", &fluxo_eco("antigo"));
    let base = servir(&s, gatilho_para(&arq)).await;
    assert_eq!(fluxo_versoes::ler_indice(&arq).unwrap(), None);
    assert_eq!(fluxo_versoes::caminho_publicado(&arq).unwrap(), arq);
    let (saida, _) = saida_do_disparo(&s, &disparo(&base).await).await;
    assert_eq!(saida, "antigo");
    gravar(&raiz, "f", &fluxo_eco("editado"));
    let (saida, _) = saida_do_disparo(&s, &disparo(&base).await).await;
    assert_eq!(saida, "editado", "sem publicar, o arquivo e o publicado");
    assert!(!fluxo_versoes::pasta_de(&arq).exists(), "ler nao cria nada");
    // o leitor de producao e o de rascunho concordam quando nunca se publicou
    assert_eq!(
        fluxos::assinatura(&fluxo_versoes::ler_publicado(&arq).unwrap()),
        fluxos::assinatura(&fluxos::ler_arquivo(&arq).unwrap())
    );
}

/// Publicar numera de 1 em 1, recusa rascunho igual a publicada, e o historico lista tudo.
#[test]
fn publicar_numera_e_recusa_rascunho_igual() {
    let raiz = tmp("numera");
    let arq = gravar(&raiz, "f", &fluxo_eco("a"));
    let sit = fluxo_versoes::situacao(&arq).unwrap();
    assert!(sit.indice.is_none() && !sit.rascunho_alterado());
    let v1 = fluxo_versoes::publicar(&arq, "  inicio ").unwrap();
    assert_eq!((v1.numero, v1.nota.as_str()), (1, "inicio"));
    let e = fluxo_versoes::publicar(&arq, "").unwrap_err();
    assert!(e.contains("nada a publicar"), "{e}");
    assert!(!fluxo_versoes::situacao(&arq).unwrap().rascunho_alterado());
    gravar(&raiz, "f", &fluxo_eco("b"));
    assert!(fluxo_versoes::situacao(&arq).unwrap().rascunho_alterado());
    let v2 = fluxo_versoes::publicar(&arq, "").unwrap();
    assert_eq!(v2.numero, 2);
    let i = fluxo_versoes::ler_indice(&arq).unwrap().unwrap();
    assert_eq!((i.publicada, i.versoes.len()), (2, 2));
    assert_ne!(i.versoes[0].sha256, i.versoes[1].sha256);
    // fluxo invalido nao e publicado, e nada nasce
    let ruim = gravar(&raiz, "ruim", &json!({"nome":"ruim","passos":[]}));
    assert!(fluxo_versoes::publicar(&ruim, "").is_err());
    assert!(!fluxo_versoes::pasta_de(&ruim).exists());
    // a nota nao guarda credencial
    gravar(&raiz, "f", &fluxo_eco("c"));
    assert!(fluxo_versoes::publicar(&arq, "ghp_0123456789abcdefABCDEF").is_err());
}

/// `voltar` publica de novo a definicao antiga como versao NOVA (o historico so cresce), e o
/// gatilho passa a rodar aquela definicao; voltar para a que ja e a publicada e recusado.
#[tokio::test]
async fn voltar_a_uma_versao_e_o_gatilho_a_roda() {
    let raiz = tmp("voltar");
    let s = estado(&raiz);
    let arq = gravar(&raiz, "f", &fluxo_eco("um"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    gravar(&raiz, "f", &fluxo_eco("dois"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    let base = servir(&s, gatilho_para(&arq)).await;
    assert_eq!(saida_do_disparo(&s, &disparo(&base).await).await.0, "dois");

    let v3 = fluxo_versoes::voltar(&arq, 1).unwrap();
    assert_eq!((v3.numero, v3.voltou_a), (3, Some(1)));
    let i = fluxo_versoes::ler_indice(&arq).unwrap().unwrap();
    assert_eq!(
        (i.publicada, i.versoes.len()),
        (3, 3),
        "o historico so cresce"
    );
    assert_eq!(i.versoes[2].sha256, i.versoes[0].sha256);
    assert_eq!(saida_do_disparo(&s, &disparo(&base).await).await.0, "um");
    // o rascunho nao foi tocado
    assert!(std::fs::read_to_string(&arq).unwrap().contains("dois"));
    assert!(
        fluxo_versoes::voltar(&arq, 3)
            .unwrap_err()
            .contains("ja e a publicada")
    );
    assert!(
        fluxo_versoes::voltar(&arq, 1)
            .unwrap_err()
            .contains("ja e a publicada")
    );
    assert!(
        fluxo_versoes::voltar(&arq, 9)
            .unwrap_err()
            .contains("nao existe")
    );
    assert!(
        fluxo_versoes::voltar(&arq, 0)
            .unwrap_err()
            .contains("nao existe")
    );
}

/// Versao adulterada FECHA: o gatilho recusa em vez de cair no rascunho, e o indice de
/// formato futuro, sem indice ou com numeracao quebrada tambem.
///
/// RED medido: `caminho_publicado` voltando ao rascunho quando `ler_versao` falha
/// (`// REPOSTO`) -- o gatilho responde 202 rodando o rascunho, e o teste cai.
#[tokio::test]
async fn versao_adulterada_fecha_e_nao_cai_no_rascunho() {
    let raiz = tmp("adulterada");
    let s = estado(&raiz);
    let arq = gravar(&raiz, "f", &fluxo_eco("publicado"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    gravar(&raiz, "f", &fluxo_eco("rascunho"));
    let base = servir(&s, gatilho_para(&arq)).await;
    let v1 = fluxo_versoes::arquivo_da_versao(&arq, 1);
    let original = std::fs::read_to_string(&v1).unwrap();
    std::fs::write(&v1, original.replace("publicado", "trocado")).unwrap();
    let r = reqwest::Client::new()
        .post(format!("{base}/v1/triggers/g"))
        .bearer_auth(TOKEN)
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 500);
    let corpo: Value = r.json().await.unwrap();
    assert!(corpo["error"].as_str().unwrap().contains("hash"), "{corpo}");
    assert!(s.store.list().unwrap().is_empty(), "nenhuma tarefa nasceu");
    assert!(fluxo_versoes::ler_publicado(&arq).is_err());
    assert!(
        fluxo_versoes::voltar(&arq, 1).is_err(),
        "nao se volta a uma versao quebrada"
    );

    // restaurado o conteudo, volta a rodar a publicada
    std::fs::write(&v1, &original).unwrap();
    assert_eq!(
        saida_do_disparo(&s, &disparo(&base).await).await.0,
        "publicado"
    );

    // indice de formato futuro, e pasta sem indice
    let ind = fluxo_versoes::pasta_de(&arq).join("indice.json");
    let bom = std::fs::read_to_string(&ind).unwrap();
    std::fs::write(&ind, bom.replace("\"formato\": 1", "\"formato\": 9")).unwrap();
    assert!(
        fluxo_versoes::ler_indice(&arq)
            .unwrap_err()
            .contains("formato 9")
    );
    std::fs::remove_file(&ind).unwrap();
    assert!(fluxo_versoes::caminho_publicado(&arq).is_err());
    std::fs::write(&ind, bom.replace("\"numero\": 1", "\"numero\": 5")).unwrap();
    assert!(
        fluxo_versoes::ler_indice(&arq)
            .unwrap_err()
            .contains("numerada")
    );
}

/// `restaurar_rascunho` copia a versao para o arquivo, mas recusa quando o rascunho tem edicao
/// que nenhuma versao guarda (a unica operacao daqui que destroi o que a pessoa digitou).
#[test]
fn restaurar_rascunho_recusa_edicao_nao_guardada() {
    let raiz = tmp("restaurar");
    let arq = gravar(&raiz, "f", &fluxo_eco("um"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    gravar(&raiz, "f", &fluxo_eco("dois"));
    let e = fluxo_versoes::restaurar_rascunho(&arq, 1, false).unwrap_err();
    assert!(e.contains("nenhuma versao guarda"), "{e}");
    assert!(std::fs::read_to_string(&arq).unwrap().contains("dois"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    // agora o rascunho esta guardado (na v2): pode voltar ao conteudo da v1
    fluxo_versoes::restaurar_rascunho(&arq, 1, false).unwrap();
    assert!(std::fs::read_to_string(&arq).unwrap().contains("um"));
    gravar(&raiz, "f", &fluxo_eco("tres"));
    fluxo_versoes::restaurar_rascunho(&arq, 2, true).unwrap();
    assert!(std::fs::read_to_string(&arq).unwrap().contains("dois"));
    // o pin do rascunho velho nao fica duplicado com o da versao
    let com_pin = gravar(
        &raiz,
        "p",
        &json!({"nome":"p","passos":[{"id":"a","ferramenta":"eco","args":{"texto":"x"}}]}),
    );
    fluxos::pinar(&com_pin, "a", Some(json!(["PIN"]))).unwrap();
    fluxo_versoes::publicar(&com_pin, "").unwrap();
    fluxo_versoes::restaurar_rascunho(&com_pin, 1, false).unwrap();
    assert!(!fluxos::arquivo_de_pins(&com_pin).exists());
    let f = fluxos::ler_arquivo(&com_pin).unwrap();
    assert_eq!(f.passos[0].pin, Some(json!(["PIN"])));
}

/// A pasta de versoes e oculta: `listar` nao a toma por uma pasta de fluxos, e a versao
/// nao aparece como fluxo.
#[test]
fn listar_nao_toma_a_pasta_de_versoes_por_fluxo() {
    let raiz = tmp("listar");
    let arq = gravar(&raiz, "f", &fluxo_eco("a"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    assert!(fluxo_versoes::pasta_de(&arq).is_dir());
    let (achados, erros) = fluxos::listar(&raiz, None, None);
    assert!(erros.is_empty(), "{erros:?}");
    assert_eq!(achados.len(), 1);
}

/// Dois processos publicando ao mesmo tempo nao pegam o mesmo numero (a trava do indice).
/// Oito threads alternam `voltar` entre as versoes 1 e 2; cada `Ok` e exatamente uma versao
/// nova no indice.
///
/// RED medido: a `travar(...)` de `acrescentar` retirada (`// REPOSTO`) -- versoes se perdem
/// (menos `Ok` gravados que `Ok` contados) em 3 de 3 corridas; esta rodada e deterministica
/// para o verde e probabilistica para o vermelho.
#[test]
fn publicacoes_concorrentes_nao_repetem_numero() {
    let raiz = tmp("concorrente");
    let arq = gravar(&raiz, "f", &fluxo_eco("um"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    gravar(&raiz, "f", &fluxo_eco("dois"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    let barreira = std::sync::Arc::new(std::sync::Barrier::new(8));
    let mut hs = Vec::new();
    for k in 0..8u32 {
        let (arq, b) = (arq.clone(), barreira.clone());
        hs.push(std::thread::spawn(move || {
            b.wait();
            let mut oks = 0;
            for r in 0..6 {
                if fluxo_versoes::voltar(&arq, 1 + (k + r) % 2).is_ok() {
                    oks += 1;
                }
            }
            oks
        }));
    }
    let oks: usize = hs.into_iter().map(|h| h.join().unwrap()).sum();
    let i = fluxo_versoes::ler_indice(&arq).unwrap().unwrap();
    assert_eq!(i.versoes.len(), 2 + oks, "versao perdida na corrida");
    for v in &i.versoes {
        assert!(fluxo_versoes::arquivo_da_versao(&arq, v.numero).is_file());
    }
}

// ---------------------------------------------------------------- git dos fluxos

/// Um projeto com tres fluxos: um sem pasta, um em subpasta e um com pin ao lado.
fn projeto(raiz: &Path) -> PathBuf {
    let dir = raiz.join("fluxos");
    std::fs::create_dir_all(dir.join("vendas")).unwrap();
    gravar(&dir, "a", &fluxo_eco("a"));
    gravar(
        &dir.join("vendas"),
        "b",
        &json!({"nome":"b","etiquetas":["x"],"passos":[
        {"id":"z","ferramenta":"eco","args":{"texto":"b","b":2,"a":1}}]}),
    );
    let c = gravar(&dir, "c", &fluxo_eco("c"));
    fluxos::pinar(&c, "a", Some(json!(["PIN"]))).unwrap();
    dir
}

/// O arquivo e canonico: chaves em ordem, os mesmos bytes a cada exportacao (e nada e
/// reescrito quando nada mudou), com o ambiente dentro; o rascunho vai para `dev/` e a
/// publicada para `prod/`.
#[test]
fn exportar_e_canonico_e_estavel_e_separa_dev_de_prod() {
    let raiz = tmp("git-export");
    let dir = projeto(&raiz);
    let repo = raiz.join("repo");
    let a = fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    assert_eq!(a.escritos.len(), 3);
    assert!(a.escritos.iter().all(|e| e.mudou));
    for rel in ["dev/a.json", "dev/vendas/b.json", "dev/c.json"] {
        assert!(repo.join(rel).is_file(), "{rel}");
    }
    let texto = std::fs::read_to_string(repo.join("dev/vendas/b.json")).unwrap();
    // chaves em ordem em qualquer profundidade (a antes de b dentro dos args)
    let pos = |t: &str| texto.find(t).unwrap_or_else(|| panic!("{t}: {texto}"));
    assert!(pos("\"ambiente\"") < pos("\"fluxo\""));
    assert!(pos("\"fluxo\"") < pos("\"phxclaw_fluxo\""));
    assert!(pos("\"phxclaw_fluxo\"") < pos("\"sha256\""));
    assert!(pos("\"a\": 1") < pos("\"b\": 2"));
    assert!(texto.ends_with("}\n"));
    // o pin vai DENTRO do arquivo (o repositorio tem um arquivo por fluxo)
    assert!(
        std::fs::read_to_string(repo.join("dev/c.json"))
            .unwrap()
            .contains("PIN")
    );
    // de novo: mesmos bytes, nada gravado
    let antes = std::fs::read(repo.join("dev/a.json")).unwrap();
    let b = fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    assert!(b.escritos.iter().all(|e| !e.mudou), "{b:?}");
    assert_eq!(antes, std::fs::read(repo.join("dev/a.json")).unwrap());
    // prod: so a publicada. `a` publicado em "a", rascunho editado depois
    let arq_a = dir.join("a.json");
    fluxo_versoes::publicar(&arq_a, "").unwrap();
    gravar(&dir, "a", &fluxo_eco("a-editado"));
    let p = fluxo_git::exportar(&dir, &repo, Ambiente::Prod).unwrap();
    let pa = std::fs::read_to_string(repo.join("prod/a.json")).unwrap();
    assert!(pa.contains("\"versao\": 1") && pa.contains("\"a\""), "{pa}");
    assert!(!pa.contains("a-editado"), "o prod exportou o rascunho");
    let da = std::fs::read_to_string(repo.join("dev/a.json")).unwrap();
    fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    assert!(
        std::fs::read_to_string(repo.join("dev/a.json"))
            .unwrap()
            .contains("a-editado")
    );
    assert_ne!(
        da,
        std::fs::read_to_string(repo.join("dev/a.json")).unwrap()
    );
    // nunca publicado: versao nula (o arquivo e o publicado implicito)
    let pc = p
        .escritos
        .iter()
        .find(|e| e.nome == "f" && e.versao.is_none());
    assert!(pc.is_some(), "{p:?}");
}

/// O fluxo que saiu do projeto sai do repositorio (o `git diff` mostra a remocao), sem tocar
/// em arquivo que nao e pacote de fluxo; e um fluxo que nao le aborta ANTES de gravar.
#[test]
fn exportar_remove_o_que_saiu_e_aborta_sem_gravar() {
    let raiz = tmp("git-remove");
    let dir = projeto(&raiz);
    let repo = raiz.join("repo");
    fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    std::fs::write(repo.join("dev/LEIA-ME.json"), r#"{"nota":"minha"}"#).unwrap();
    std::fs::remove_file(dir.join("a.json")).unwrap();
    let r = fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    assert_eq!(r.removidos.len(), 1);
    assert!(!repo.join("dev/a.json").exists());
    assert!(
        repo.join("dev/LEIA-ME.json").exists(),
        "arquivo alheio foi apagado"
    );
    // um fluxo quebrado: nada e gravado, nem o que leria
    gravar(&dir, "quebrado", &json!({"nome":"q","passos":[]}));
    gravar(&dir, "novo", &fluxo_eco("novo"));
    let e = fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap_err();
    assert!(
        e.contains("quebrado.json") && e.contains("nada foi exportado"),
        "{e}"
    );
    assert!(!repo.join("dev/novo.json").exists());
}

/// Ida e volta: o dev de uma instancia chega na outra igual (hash a hash, pin incluido); o
/// prod PUBLICA no destino, e o gatilho do destino passa a rodar o que veio do git.
#[tokio::test]
async fn importar_prod_publica_e_o_gatilho_roda_o_que_veio_do_git() {
    let origem = tmp("git-origem");
    let dir = projeto(&origem);
    fluxo_versoes::publicar(&dir.join("a.json"), "").unwrap();
    gravar(&dir, "a", &fluxo_eco("a-v2"));
    fluxo_versoes::publicar(&dir.join("a.json"), "").unwrap();
    let repo = origem.join("repo");
    fluxo_git::exportar(&dir, &repo, Ambiente::Prod).unwrap();
    fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();

    // dev -> outra instancia: rascunhos iguais, pin preservado, nada publicado
    let destino = tmp("git-destino-dev");
    let r = fluxo_git::importar(&repo, Ambiente::Dev, &destino, false).unwrap();
    assert_eq!(r.len(), 3);
    assert!(
        r.iter()
            .all(|x| x.efeito == Efeito::Novo && x.versao.is_none()),
        "{r:?}"
    );
    for rel in ["a.json", "vendas/b.json", "c.json"] {
        assert_eq!(
            fluxos::assinatura(&fluxos::ler_arquivo(&dir.join(rel)).unwrap()),
            fluxos::assinatura(&fluxos::ler_arquivo(&destino.join(rel)).unwrap()),
            "{rel}"
        );
    }
    assert!(fluxos::arquivo_de_pins(&destino.join("c.json")).exists());
    // importar de novo o mesmo: tudo igual, nada gravado
    let r = fluxo_git::importar(&repo, Ambiente::Dev, &destino, false).unwrap();
    assert!(r.iter().all(|x| x.efeito == Efeito::Igual), "{r:?}");

    // prod -> instancia de producao: publica, e o gatilho roda a publicada (a-v2)
    let prod = tmp("git-destino-prod");
    let r = fluxo_git::importar(&repo, Ambiente::Prod, &prod, false).unwrap();
    let a = r.iter().find(|x| x.destino.ends_with("a.json")).unwrap();
    assert_eq!((a.efeito, a.versao), (Efeito::Novo, Some(1)));
    let s = estado(&prod);
    let base = servir(&s, gatilho_para(&prod.join("a.json"))).await;
    assert_eq!(saida_do_disparo(&s, &disparo(&base).await).await.0, "a-v2");
    // de novo: o prod ja roda isto
    let r = fluxo_git::importar(&repo, Ambiente::Prod, &prod, false).unwrap();
    assert!(r.iter().all(|x| x.efeito == Efeito::Igual), "{r:?}");
    let i = fluxo_versoes::ler_indice(&prod.join("a.json"))
        .unwrap()
        .unwrap();
    assert_eq!(i.versoes.len(), 1, "importar o mesmo nao cria versao");
}

/// A importacao confere TUDO antes de gravar: arquivo movido de ambiente, pacote editado sem
/// refazer o sha256 e rascunho diferente sem `--sobrescrever` nao deixam metade trocada.
///
/// RED medido: a verificacao do campo `ambiente` retirada de `importar` (`// REPOSTO`) -- o
/// arquivo do dev colocado em `prod/` e publicado, e o teste cai.
#[test]
fn importar_confere_antes_de_gravar() {
    let origem = tmp("git-confere");
    let dir = projeto(&origem);
    let repo = origem.join("repo");
    fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    let destino = tmp("git-confere-destino");

    // arquivo do dev que alguem moveu para prod/
    std::fs::create_dir_all(repo.join("prod")).unwrap();
    std::fs::copy(repo.join("dev/a.json"), repo.join("prod/a.json")).unwrap();
    let e = fluxo_git::importar(&repo, Ambiente::Prod, &destino, false).unwrap_err();
    assert!(e.contains("ambiente") && e.contains("dev"), "{e}");
    assert!(!destino.join("a.json").exists());

    // pacote editado a mao sem refazer o sha256
    let t = std::fs::read_to_string(repo.join("dev/c.json")).unwrap();
    std::fs::write(repo.join("dev/c.json"), t.replace("\"c\"", "\"hackeado\"")).unwrap();
    let e = fluxo_git::importar(&repo, Ambiente::Dev, &destino, false).unwrap_err();
    assert!(e.contains("sha256"), "{e}");
    assert!(
        !destino.join("a.json").exists(),
        "a e anterior a c na ordem, e nao pode ter sido gravado"
    );
    std::fs::write(repo.join("dev/c.json"), t).unwrap();

    // rascunho diferente: recusa tudo, e com o pedido troca
    fluxo_git::importar(&repo, Ambiente::Dev, &destino, false).unwrap();
    gravar(&destino, "a", &fluxo_eco("editado no destino"));
    gravar(
        &destino.join("vendas"),
        "b",
        &json!({"nome":"b","passos":[{"id":"z","ferramenta":"eco","args":{"texto":"editado"}}]}),
    );
    let e = fluxo_git::importar(&repo, Ambiente::Dev, &destino, false).unwrap_err();
    assert!(
        e.contains("2 rascunho(s)") && e.contains("nada foi importado"),
        "{e}"
    );
    assert!(
        std::fs::read_to_string(destino.join("a.json"))
            .unwrap()
            .contains("editado no destino")
    );
    let r = fluxo_git::importar(&repo, Ambiente::Dev, &destino, true).unwrap();
    assert_eq!(
        r.iter().filter(|x| x.efeito == Efeito::Atualizado).count(),
        2
    );
    assert!(
        std::fs::read_to_string(destino.join("a.json"))
            .unwrap()
            .contains("\"a\"")
    );
    assert!(
        !std::fs::read_to_string(destino.join("a.json"))
            .unwrap()
            .contains("editado")
    );

    // pasta do ambiente que nao existe
    let e =
        fluxo_git::importar(&origem.join("sem-repo"), Ambiente::Dev, &destino, false).unwrap_err();
    assert!(e.contains("nao existe"), "{e}");
    assert!(Ambiente::ler("homologacao").is_err());
}

/// No prod, o rascunho com edicao nao guardada NAO impede importar quando o prod ja roda o
/// que veio (a edicao nao e do prod), e quando difere o pedido explicito e exigido.
#[test]
fn prod_nao_perde_edicao_de_rascunho_calado() {
    let origem = tmp("git-prod-edicao");
    let dir = origem.join("fluxos");
    std::fs::create_dir_all(&dir).unwrap();
    let arq = gravar(&dir, "a", &fluxo_eco("v1"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    let repo = origem.join("repo");
    fluxo_git::exportar(&dir, &repo, Ambiente::Prod).unwrap();
    // o destino ja tem a mesma publicada e um rascunho editado
    let destino = tmp("git-prod-edicao-destino");
    fluxo_git::importar(&repo, Ambiente::Prod, &destino, false).unwrap();
    gravar(&destino, "a", &fluxo_eco("rascunho do destino"));
    let r = fluxo_git::importar(&repo, Ambiente::Prod, &destino, false).unwrap();
    assert_eq!(r[0].efeito, Efeito::Igual, "{r:?}");
    assert!(
        std::fs::read_to_string(destino.join("a.json"))
            .unwrap()
            .contains("rascunho do destino")
    );
    // o git traz uma versao nova: o rascunho do destino difere, e o pedido e exigido
    gravar(&dir, "a", &fluxo_eco("v2"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    fluxo_git::exportar(&dir, &repo, Ambiente::Prod).unwrap();
    let e = fluxo_git::importar(&repo, Ambiente::Prod, &destino, false).unwrap_err();
    assert!(e.contains("rascunho"), "{e}");
    let r = fluxo_git::importar(&repo, Ambiente::Prod, &destino, true).unwrap();
    assert_eq!((r[0].efeito, r[0].versao), (Efeito::Atualizado, Some(2)));
}

/// `registrar` commita pelo `GitTool` de escrita (sandbox + varredura), nao por um git
/// proprio: o commit aparece no `git log` de fora, o segundo registro sem mudanca diz que nao
/// havia o que registrar, e a edicao de um fluxo vira UM arquivo no diff.
#[tokio::test]
async fn registrar_commita_pelo_git_tool() {
    let Some(bwrap) = phxclaw_agent::arquivos::achar_bwrap() else {
        pulado::pular("bwrap", "sem bwrap");
        return;
    };
    let raiz = tmp("git-registrar");
    let dir = projeto(&raiz);
    let repo = raiz.join("repo");
    fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    let g = phxclaw_agent::git::GitTool::escrita(bwrap);
    let r = fluxo_git::registrar(&g, &repo, "fluxos: primeira exportacao")
        .await
        .unwrap();
    assert_eq!(r["commit"]["assunto"], "fluxos: primeira exportacao", "{r}");
    let fora = |args: &[&str]| {
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(args)
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    assert!(fora(&["ls-files"]).contains("dev/vendas/b.json"));
    // nada mudou: nada a registrar
    fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    let r = fluxo_git::registrar(&g, &repo, "de novo").await.unwrap();
    assert_eq!(r["nada_a_registrar"], true, "{r}");
    // editar UM fluxo muda UM arquivo
    gravar(&dir, "a", &fluxo_eco("a2"));
    fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    let r = fluxo_git::registrar(&g, &repo, "fluxos: a2").await.unwrap();
    assert!(r["commit"].is_object(), "{r}");
    assert_eq!(
        fora(&["show", "--stat", "--format=", "HEAD"])
            .matches("dev/")
            .count(),
        1
    );
    assert_eq!(fora(&["rev-list", "--count", "HEAD"]).trim(), "2");
}

// ---------------------------------------------------------------- correcao A (revisao de seguranca)

/// A1: a tela nao alcanca a pasta de versoes. Os dois `PUT` em `.f.versoes/...` (o indice e
/// o arquivo da versao) recebem 404 -- e o GET e o rodar tambem --, e a versao publicada
/// continua a original. O texto forjado e um fluxo valido que tambem le como indice sem o
/// `deny_unknown_fields`: o motor aceitaria o texto, e so o nome o barra.
///
/// RED medido: `arquivo_do_fluxo` aceitando componente com ponto (`// REPOSTO`, o
/// `forma_de_fluxo` sem o `starts_with('.')`) -- o PUT do indice responde 200 e o teste cai.
#[tokio::test]
async fn a_tela_nao_grava_na_pasta_de_versoes() {
    let raiz = tmp("tela-versoes");
    let s = estado(&raiz);
    let dir = raiz.join("fluxos");
    std::fs::create_dir_all(&dir).unwrap();
    let arq = gravar(&dir, "f", &fluxo_eco("original"));
    let v1 = fluxo_versoes::publicar(&arq, "").unwrap();
    let base = servir(&s, Gatilhos::default()).await;
    let http = reqwest::Client::new();
    let pasta = fluxo_versoes::pasta_de(&arq);
    let mut forjado = fluxo_eco("forjado");
    forjado["formato"] = json!(1);
    forjado["publicada"] = json!(1);
    forjado["versoes"] = json!([{"numero":1,"sha256":fluxos::assinatura(
        &fluxo(fluxo_eco("forjado"))),"em":"2026-10-09T00:00:00Z"}]);
    for nome in [".f.versoes/indice.json", ".f.versoes/v0001.json"] {
        let atual = std::fs::read(pasta.join(nome.rsplit('/').next().unwrap())).unwrap();
        let r = http
            .put(format!("{base}/v1/fluxos/arquivo"))
            .query(&[("nome", nome)])
            .bearer_auth(TOKEN)
            .header("If-Match", sha256(&atual))
            .json(&json!({"texto": forjado.to_string()}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 404, "PUT {nome}: {}", r.text().await.unwrap());
        let r = http
            .get(format!("{base}/v1/fluxos/arquivo"))
            .query(&[("nome", nome)])
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 404, "GET {nome}");
        let r = http
            .post(format!("{base}/v1/fluxos/rodar"))
            .bearer_auth(TOKEN)
            .json(&json!({"nome": nome}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 404, "rodar {nome}");
    }
    let f = fluxo_versoes::ler_publicado(&arq).unwrap();
    assert_eq!(fluxos::assinatura(&f), v1.sha256, "a publicada mudou");
    // o rascunho, pelo nome simples, continua abrindo e gravando
    let r = http
        .get(format!("{base}/v1/fluxos/arquivo"))
        .query(&[("nome", "f.json")])
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let lido: Value = r.json().await.unwrap();
    let r = http
        .put(format!("{base}/v1/fluxos/arquivo"))
        .query(&[("nome", "f.json")])
        .bearer_auth(TOKEN)
        .header("If-Match", lido["revisao"].as_str().unwrap())
        .json(&json!({"texto": fluxo_eco("rascunho novo").to_string()}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().await.unwrap());
}

/// O indice recusa campo desconhecido: um texto de fluxo nunca le como indice, mesmo que
/// alguem o ponha la por outro caminho.
///
/// RED medido: sem o `#[serde(deny_unknown_fields)]` do `Indice` (`// REPOSTO`) -- o fluxo
/// forjado le como indice e o teste cai.
#[test]
fn fluxo_no_lugar_do_indice_nao_le() {
    let raiz = tmp("indice-estrito");
    let arq = gravar(&raiz, "f", &fluxo_eco("original"));
    fluxo_versoes::publicar(&arq, "").unwrap();
    let ind = fluxo_versoes::pasta_de(&arq).join("indice.json");
    let bom: Value = serde_json::from_str(&std::fs::read_to_string(&ind).unwrap()).unwrap();
    let mut forjado = fluxo_eco("forjado");
    for k in ["formato", "publicada", "versoes"] {
        forjado[k] = bom[k].clone();
    }
    std::fs::write(&ind, forjado.to_string()).unwrap();
    let e = fluxo_versoes::ler_indice(&arq).unwrap_err();
    assert!(e.contains("indice invalido"), "{e}");
}

#[cfg(unix)]
fn link(alvo: &Path, onde: &Path) {
    std::os::unix::fs::symlink(alvo, onde).unwrap();
}

/// A pasta de versoes que e link (para uma pasta de versoes valida de outro lugar, ou
/// quebrado) e recusa: nem roda a versao de fora, nem cai no rascunho.
///
/// RED medido: `ler_indice` voltando ao `pasta.exists()` (`// REPOSTO`) -- o link valido
/// le a versao de fora e o teste cai; e o `ler_sem_link` sem a conferencia do link
/// (`// REPOSTO`) -- o indice e a versao de fora leem e o teste cai na ultima parte.
#[cfg(unix)]
#[test]
fn pasta_de_versoes_que_e_link_e_recusada() {
    let fora = tmp("versoes-fora");
    let outro = gravar(&fora, "f", &fluxo_eco("de fora"));
    fluxo_versoes::publicar(&outro, "").unwrap();
    let raiz = tmp("versoes-link");
    let arq = gravar(&raiz, "f", &fluxo_eco("rascunho"));
    link(
        &fluxo_versoes::pasta_de(&outro),
        &fluxo_versoes::pasta_de(&arq),
    );
    let e = fluxo_versoes::ler_publicado(&arq).unwrap_err();
    assert!(e.contains("link simbolico"), "{e}");
    assert!(fluxo_versoes::publicar(&arq, "").is_err());
    std::fs::remove_file(fluxo_versoes::pasta_de(&arq)).unwrap();
    link(&fora.join("nao-existe"), &fluxo_versoes::pasta_de(&arq));
    let e = fluxo_versoes::ler_publicado(&arq).unwrap_err();
    assert!(e.contains("link simbolico"), "{e}");
    // o indice e a versao que sao link, dentro de uma pasta de verdade, tambem: os dois
    // apontando para os de fora formam uma publicada coerente (hash bate) que nao e daqui
    std::fs::remove_file(fluxo_versoes::pasta_de(&arq)).unwrap();
    fluxo_versoes::publicar(&arq, "").unwrap();
    for nome in ["indice.json", "v0001.json"] {
        let aqui = fluxo_versoes::pasta_de(&arq).join(nome);
        std::fs::remove_file(&aqui).unwrap();
        link(&fluxo_versoes::pasta_de(&outro).join(nome), &aqui);
    }
    let e = fluxo_versoes::ler_publicado(&arq).unwrap_err();
    assert!(e.contains("link simbolico"), "{e}");
}

/// O repositorio de fluxos com link (arquivo ou pasta) e recusa no `importar`: o link
/// traria para o projeto -- e, no prod, publicaria -- um arquivo que nao esta no repositorio.
///
/// RED medido: `arquivos_de_pacote` voltando ao `is_file`/`is_dir` (`// REPOSTO`) -- o
/// pacote de fora e importado e o teste cai.
#[cfg(unix)]
#[test]
fn importar_recusa_link_no_repositorio() {
    let origem = tmp("git-link");
    let dir = projeto(&origem);
    let repo = origem.join("repo");
    fluxo_git::exportar(&dir, &repo, Ambiente::Dev).unwrap();
    let fora = tmp("git-link-fora");
    std::fs::rename(repo.join("dev/a.json"), fora.join("a.json")).unwrap();
    link(&fora.join("a.json"), &repo.join("dev/a.json"));
    let destino = tmp("git-link-destino");
    let e = fluxo_git::importar(&repo, Ambiente::Dev, &destino, false).unwrap_err();
    assert!(e.contains("link simbolico"), "{e}");
    assert!(!destino.join("a.json").exists());
    // a pasta do fluxo que e link
    std::fs::remove_file(repo.join("dev/a.json")).unwrap();
    std::fs::rename(fora.join("a.json"), repo.join("dev/a.json")).unwrap();
    std::fs::rename(repo.join("dev/vendas"), fora.join("vendas")).unwrap();
    link(&fora.join("vendas"), &repo.join("dev/vendas"));
    let e = fluxo_git::importar(&repo, Ambiente::Dev, &destino, false).unwrap_err();
    assert!(e.contains("link simbolico"), "{e}");
    assert!(!destino.join("vendas").exists());
}
