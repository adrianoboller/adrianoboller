//! As rotas da tela «Fluxos» (o editor em grafo do PWA): listar, ler, validar, gravar e
//! disparar os fluxos da pasta `fluxos/` do agente -- a MESMA pasta do `phxclaw fluxo
//! listar` e do sub-fluxo (`Montagem::raiz_do_agente().join("fluxos")`).
//!
//! Nada aqui decide o que e um fluxo valido: toda leitura e toda gravacao passam pelo
//! `fluxos::ler` (o motor), e o disparo pelo `api::criar_fluxo_ate` (o mesmo caminho do
//! webhook, da agenda e do gatilho, com o corte `--ate` da CLI). A tela desenha; o motor
//! julga. Uma segunda validacao aqui seria a segunda copia da regra, e a que alguem
//! esqueceria de atualizar.
//!
//! - `GET  /v1/fluxos`: os fluxos da pasta (validos e invalidos, os dois com o nome
//!   relativo), pelo `fluxos::listar`.
//! - `GET  /v1/fluxos/arquivo?nome=REL`: o texto cru, a revisao (sha256 dos bytes) e o
//!   veredito do motor -- o invalido tambem abre, para ser consertado.
//! - `POST /v1/fluxos/validar` `{"texto": ...}`: so o veredito e a assinatura; nao toca
//!   disco.
//! - `PUT  /v1/fluxos/arquivo?nome=REL` `{"texto": ...}` com `If-Match: <revisao>`: grava
//!   SO arquivo que ja existe, SO texto que o motor aceita, e SO sobre a revisao lida
//!   (409 com a atual, senao a gravacao de outra aba se perderia calada).
//! - `POST /v1/fluxos/rodar` `{"nome": REL, "ate": PASSO?}`: a tarefa do fluxo (202 + id),
//!   carimbada com o projeto do cabecalho `X-PhxClaw-Projeto` quando ha usuarios.
//!
//! Com usuarios (`rbac.rs`), gravar e rodar sao do `admin`: o arquivo gravado aqui e o que
//! o webhook, a agenda e o poll rodam com as credenciais do operador.
//!
//! A posicao dos nos mora no campo `ui` do proprio JSON. O `Fluxo` nao tem esse campo:
//! o serde o ignora na leitura e a `assinatura` (que serializa o struct) nunca o ve --
//! mover um no nao invalida uma execucao parada numa espera.

use crate::api::{ApiState, auth};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};

/// Teto do texto de um fluxo: 64 passos com argumentos generosos cabem com folga, e um
/// corpo maior que isso e engano ou abuso, nao fluxo.
pub const MAX_BYTES_FLUXO: usize = 1024 * 1024;

pub fn rotas() -> Router<ApiState> {
    Router::new()
        .route("/v1/fluxos", get(listar))
        .route("/v1/fluxos/arquivo", get(ler).put(gravar))
        .route("/v1/fluxos/validar", post(validar))
        .route("/v1/fluxos/rodar", post(rodar))
}

type Erro = (StatusCode, Json<Value>);

fn erro(code: StatusCode, msg: impl Into<String>) -> Erro {
    (code, Json(json!({"error": msg.into()})))
}

/// A pasta dos fluxos do agente: irma da raiz das tarefas, como na `Montagem`.
pub fn pasta_dos_fluxos(s: &ApiState) -> PathBuf {
    let raiz = s.store.root();
    raiz.parent().unwrap_or(raiz).join("fluxos")
}

/// O nome relativo, conferido contra o que o `fluxos::listar` enxerga: `ARQ.json` ou
/// `PASTA/ARQ.json` (um nivel, como a listagem), nenhum componente comecando com `.`, nunca
/// o arquivo de pins, e o caminho real (depois dos links) dentro da pasta e com a MESMA
/// forma. `None` diz «nao e um fluxo daqui» sem dizer se o arquivo existe.
///
/// O ponto e a guarda que importa: a pasta `.ARQ.versoes/` mora ao lado do rascunho, e o
/// `indice.json` e o `v0001.json` dela sao `.json` validos -- um fluxo que tambem e indice
/// (o `Fluxo` ignora campo desconhecido) gravado ali pela tela trocaria a versao PUBLICADA,
/// a que o webhook, a agenda e o poll rodam, sem passar pelo `publicar`. A conferencia se
/// repete no caminho canonico porque um link `x.json -> .x.versoes/indice.json` dentro da
/// pasta passaria pelo `starts_with` e chegaria ao mesmo lugar.
pub(crate) fn arquivo_do_fluxo(pasta: &Path, nome: &str) -> Option<PathBuf> {
    fn forma_de_fluxo(r: &Path) -> bool {
        let partes: Vec<_> = r.components().collect();
        (1..=2).contains(&partes.len())
            && partes.iter().all(|c| match c {
                Component::Normal(n) => !n.to_string_lossy().starts_with('.'),
                _ => false,
            })
            && r.to_str()
                .is_some_and(|t| t.ends_with(".json") && !t.ends_with(".pins.json"))
    }
    // A barra invertida nao separa nada no Unix, mas separa no Windows: o mesmo nome nao
    // pode ser um arquivo la e uma descida de pasta aqui.
    if nome.contains('\\') || !forma_de_fluxo(Path::new(nome)) {
        return None;
    }
    let base = std::fs::canonicalize(pasta).ok()?;
    let alvo = std::fs::canonicalize(base.join(nome)).ok()?;
    let dentro = alvo.strip_prefix(&base).ok()?;
    (forma_de_fluxo(dentro) && alvo.is_file()).then_some(alvo)
}

fn relativo(pasta: &Path, p: &Path) -> String {
    p.strip_prefix(pasta)
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

/// O veredito do motor sobre um texto: `fluxos::ler` (serde + `validar`).
fn veredito(texto: &str) -> Value {
    match crate::fluxos::ler(texto) {
        Ok(f) => json!({
            "valido": true,
            "erro": null,
            "nome": f.nome,
            "passos": f.passos.len(),
            "assinatura": crate::fluxos::assinatura(&f),
        }),
        Err(e) => json!({"valido": false, "erro": e}),
    }
}

async fn listar(State(s): State<ApiState>, h: HeaderMap) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let pasta = pasta_dos_fluxos(&s);
    let v = tokio::task::spawn_blocking(move || {
        let (achados, erros) = crate::fluxos::listar(&pasta, None, None);
        let prefixo = format!("{}/", pasta.display());
        json!({
            "fluxos": achados.iter().map(|f| json!({
                "nome": f.nome,
                "arquivo": relativo(&pasta, &f.arquivo),
                "pasta": f.pasta,
                "etiquetas": f.etiquetas,
                "passos": f.passos,
            })).collect::<Vec<_>>(),
            // O `listar` devolve «CAMINHO: motivo»; a tela recebe o nome relativo, que e
            // o que ela pede de volta no `arquivo?nome=`.
            "invalidos": erros.iter().map(|e| {
                let e = e.strip_prefix(&prefixo).unwrap_or(e);
                let (arq, motivo) = e.split_once(": ").unwrap_or((e, ""));
                json!({"arquivo": arq, "erro": motivo})
            }).collect::<Vec<_>>(),
        })
    })
    .await
    .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(v))
}

#[derive(Deserialize)]
struct Consulta {
    nome: String,
}

async fn ler(
    State(s): State<ApiState>,
    h: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let pasta = pasta_dos_fluxos(&s);
    let nome = q.nome.clone();
    let lido = tokio::task::spawn_blocking(move || {
        let arq = arquivo_do_fluxo(&pasta, &nome)?;
        std::fs::read(arq).ok()
    })
    .await
    .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let Some(bytes) = lido else {
        return Err(erro(
            StatusCode::NOT_FOUND,
            format!("{}: nao e um fluxo da pasta fluxos/", q.nome),
        ));
    };
    if bytes.len() > MAX_BYTES_FLUXO {
        return Err(erro(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("{}: maior que {MAX_BYTES_FLUXO} bytes", q.nome),
        ));
    }
    let texto = String::from_utf8_lossy(&bytes).into_owned();
    let mut v = veredito(&texto);
    v["arquivo"] = json!(q.nome);
    v["revisao"] = json!(crate::fluxos::sha256_hex(&bytes));
    v["texto"] = json!(texto);
    Ok(Json(v))
}

#[derive(Deserialize)]
struct Texto {
    texto: String,
}

async fn validar(
    State(s): State<ApiState>,
    h: HeaderMap,
    Json(c): Json<Texto>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    if c.texto.len() > MAX_BYTES_FLUXO {
        return Err(erro(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("fluxo maior que {MAX_BYTES_FLUXO} bytes"),
        ));
    }
    Ok(Json(veredito(&c.texto)))
}

async fn gravar(
    State(s): State<ApiState>,
    h: HeaderMap,
    Query(q): Query<Consulta>,
    Json(c): Json<Texto>,
) -> Response {
    if let Err(e) = auth(&s, &h) {
        return e.into_response();
    }
    let Some(if_match) = h
        .get(header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
    else {
        return erro(
            StatusCode::PRECONDITION_REQUIRED,
            "falta If-Match com a revisao lida no GET /v1/fluxos/arquivo",
        )
        .into_response();
    };
    if c.texto.len() > MAX_BYTES_FLUXO {
        return erro(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("fluxo maior que {MAX_BYTES_FLUXO} bytes"),
        )
        .into_response();
    }
    // O motor julga ANTES do disco: texto que ele recusaria nao chega ao arquivo que a
    // agenda, o gatilho e o sub-fluxo leem.
    let v = veredito(&c.texto);
    if v["valido"] != json!(true) {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(v)).into_response();
    }
    let pasta = pasta_dos_fluxos(&s);
    let nome = q.nome.clone();
    let r = tokio::task::spawn_blocking(move || -> Result<String, Erro> {
        let arq = arquivo_do_fluxo(&pasta, &nome).ok_or_else(|| {
            erro(
                StatusCode::NOT_FOUND,
                format!("{nome}: nao e um fluxo da pasta fluxos/ (so se grava o que ja existe)"),
            )
        })?;
        let atual = std::fs::read(&arq)
            .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let revisao_atual = crate::fluxos::sha256_hex(&atual);
        if revisao_atual != if_match {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "o arquivo mudou depois da leitura",
                    "revisao_atual": revisao_atual,
                })),
            ));
        }
        phxclaw_types::arquivo::gravar_atomico(&arq, c.texto.as_bytes())
            .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        Ok(crate::fluxos::sha256_hex(c.texto.as_bytes()))
    })
    .await;
    match r {
        Ok(Ok(revisao)) => {
            let mut v = v;
            v["revisao"] = json!(revisao);
            Json(v).into_response()
        }
        Ok(Err(e)) => e.into_response(),
        Err(e) => erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct Disparo {
    nome: String,
    #[serde(default)]
    ate: Option<String>,
    /// Orcamento desta execucao (tokens, dinheiro), no teto global; ausente = o padrao.
    #[serde(default)]
    orcamento: Option<crate::tarefa::Orcamento>,
}

async fn rodar(
    State(s): State<ApiState>,
    h: HeaderMap,
    acesso: Option<axum::Extension<crate::rbac::Acesso>>,
    Json(d): Json<Disparo>,
) -> Result<Response, Erro> {
    auth(&s, &h)?;
    // O projeto e o que o portao resolveu (cabecalho), como no `POST /v1/tasks`: a tarefa
    // do fluxo nasce carimbada, senao o membro do projeto nao a enxergaria e o filtro do
    // `listar` a trataria como de ninguem.
    let projeto = acesso.and_then(|a| a.0.projeto);
    let pasta = pasta_dos_fluxos(&s);
    let Some(arq) = arquivo_do_fluxo(&pasta, &d.nome) else {
        return Err(erro(
            StatusCode::NOT_FOUND,
            format!("{}: nao e um fluxo da pasta fluxos/", d.nome),
        ));
    };
    let ate = d
        .ate
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty());
    let c = crate::api::criar_fluxo_ate(
        &s,
        &arq.to_string_lossy(),
        vec![],
        |_| Ok(()),
        ate,
        projeto,
        d.orcamento,
    )
    .map_err(|r| erro(r.status, r.erro))?;
    Ok((StatusCode::ACCEPTED, Json(json!({"id": c.id}))).into_response())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_fora_da_pasta_nao_e_fluxo() {
        let d = std::env::temp_dir().join(format!("phx-fluxos-tela-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let pasta = d.join("fluxos");
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("a.json"), "{}").unwrap();
        std::fs::write(pasta.join("a.pins.json"), "{}").unwrap();
        std::fs::write(d.join("fora.json"), "{}").unwrap();
        let versoes = pasta.join(".a.versoes");
        std::fs::create_dir_all(versoes.join("fundo")).unwrap();
        std::fs::write(versoes.join("indice.json"), "{}").unwrap();
        std::fs::write(versoes.join("v0001.json"), "{}").unwrap();
        std::fs::create_dir_all(pasta.join("vendas/fundo")).unwrap();
        std::fs::write(pasta.join("vendas/b.json"), "{}").unwrap();
        std::fs::write(pasta.join("vendas/fundo/c.json"), "{}").unwrap();
        std::fs::write(pasta.join(".oculto.json"), "{}").unwrap();
        assert!(arquivo_do_fluxo(&pasta, "a.json").is_some());
        // a subpasta de um nivel e o que o `fluxos::listar` mostra: a tela tem de abri-la
        assert!(arquivo_do_fluxo(&pasta, "vendas/b.json").is_some());
        for nome in [
            "../fora.json",
            "/etc/passwd",
            "a.pins.json",
            "a",
            "",
            "./a.json",
            ".a.versoes/indice.json",
            ".a.versoes/v0001.json",
            "vendas/../.a.versoes/indice.json",
            ".oculto.json",
            "vendas/fundo/c.json",
            "vendas\\b.json",
        ] {
            assert!(arquivo_do_fluxo(&pasta, nome).is_none(), "{nome}");
        }
        // o link de nome inocente que aponta para dentro da pasta de versoes
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(versoes.join("indice.json"), pasta.join("x.json")).unwrap();
            assert!(
                arquivo_do_fluxo(&pasta, "x.json").is_none(),
                "link para o indice"
            );
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn o_campo_ui_nao_muda_o_veredito_nem_a_assinatura() {
        let sem = r#"{"nome":"x","passos":[{"id":"a","tarefa":"t"}]}"#;
        let com = r#"{"nome":"x","passos":[{"id":"a","tarefa":"t"}],"ui":{"posicoes":{"a":{"x":10,"y":20}}}}"#;
        let (a, b) = (veredito(sem), veredito(com));
        assert_eq!(a["valido"], json!(true));
        assert_eq!(a["assinatura"], b["assinatura"]);
    }
}
