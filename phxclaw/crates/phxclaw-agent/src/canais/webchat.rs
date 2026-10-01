//! Webchat: uma pagina simples que conversa com o agente pelo mesmo laco dos outros canais.
//!
//! `GET /webchat` serve a pagina (sem segredo nenhum dentro dela); `POST /webchat/mensagem`
//! e `GET /webchat/respostas` pedem `Authorization: Bearer <chave>`, a chave guardada no
//! broker. A conversa e o nome que a pessoa digita, e so as da lista falam -- a chave diz
//! quem entra, a lista do `Canal` diz em que conversa. A pagina nao confere a lista de
//! novo: seria a segunda copia da decisao, e a que divergisse viraria a porta.
//!
//! As respostas vao para uma SEGUNDA caixa, a de saida, que a pagina le por `desde`: o
//! agente nao segura conexao aberta esperando a tarefa, e a pagina que recarrega nao perde
//! o que ja tinha chegado.

use super::caixa::Caixa;
use super::cripto::iguais;
use super::http::Credencial;
use super::{Entrada, Mensagem, Provedor, Unidade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub struct Webchat {
    entrada: Arc<Caixa>,
    saida: Arc<Caixa>,
    chave: Credencial,
}

impl Webchat {
    pub fn novo(entrada: Arc<Caixa>, saida: Arc<Caixa>, chave: Credencial) -> Self {
        Self {
            entrada,
            saida,
            chave,
        }
    }

    fn autorizado(&self, h: &HeaderMap) -> bool {
        let dada = h
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("")
            .trim()
            .to_string();
        !dada.is_empty()
            && self
                .chave
                .com("verify", |c| Ok(iguais(dada.as_bytes(), c.as_bytes())))
                .unwrap_or(false)
    }
}

impl Provedor for Webchat {
    fn nome(&self) -> &str {
        "webchat"
    }

    fn limite(&self) -> (usize, Unidade) {
        (16_000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.entrada
            .ler_desde(cursor, Duration::from_secs(espera_seg))
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let id = phxclaw_types::new_uuid_v7().to_string();
        self.saida.anexar(vec![(
            Mensagem {
                conversa: conversa.into(),
                autor: "agente".into(),
                id: id.clone(),
                texto: Some(texto.into()),
            },
            Value::Null,
        )])?;
        Ok(id)
    }
}

#[derive(Deserialize)]
struct Pedido {
    conversa: String,
    texto: String,
}

#[derive(Deserialize)]
struct Desde {
    conversa: String,
    #[serde(default)]
    desde: u64,
}

pub fn rotas(w: Arc<Webchat>) -> Router {
    Router::new()
        .route("/webchat", get(pagina))
        .route("/webchat/mensagem", post(mensagem))
        .route("/webchat/respostas", get(respostas))
        .with_state(w)
}

async fn pagina() -> Html<&'static str> {
    Html(PAGINA)
}

async fn mensagem(State(w): State<Arc<Webchat>>, h: HeaderMap, Json(p): Json<Pedido>) -> Response {
    let conversa = p.conversa.trim().to_string();
    if p.texto.trim().is_empty() || p.texto.len() > 20_000 {
        return (StatusCode::BAD_REQUEST, "texto vazio ou maior que 20000").into_response();
    }
    let r = tokio::task::spawn_blocking(move || {
        if !w.autorizado(&h) {
            return Err((
                StatusCode::UNAUTHORIZED,
                "chave ausente ou invalida".to_string(),
            ));
        }
        w.entrada
            .anexar(vec![(
                Mensagem {
                    conversa: conversa.clone(),
                    autor: conversa,
                    id: phxclaw_types::new_uuid_v7().to_string(),
                    texto: Some(p.texto),
                },
                Value::Null,
            )])
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
    })
    .await;
    match r {
        Ok(Ok(_)) => (StatusCode::ACCEPTED, Json(json!({"aceita": true}))).into_response(),
        Ok(Err(e)) => e.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn respostas(
    State(w): State<Arc<Webchat>>,
    h: HeaderMap,
    Query(q): Query<Desde>,
) -> Response {
    let r = tokio::task::spawn_blocking(move || {
        if !w.autorizado(&h) {
            return Err((StatusCode::UNAUTHORIZED, "chave ausente ou invalida"));
        }
        Ok(w.saida.da_conversa(&q.conversa, q.desde))
    })
    .await;
    match r {
        Ok(Ok(v)) => Json(json!(
            v.into_iter()
                .map(|(seq, texto)| json!({"seq": seq, "texto": texto}))
                .collect::<Vec<_>>()
        ))
        .into_response(),
        Ok(Err(e)) => e.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

const PAGINA: &str = r#"<!doctype html>
<html lang="pt-BR"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>PhxClaw — conversa</title>
<style>
body{font-family:system-ui,sans-serif;background:#010418;color:#e8ecf8;margin:0;display:flex;flex-direction:column;height:100vh}
header{padding:.6rem 1rem;border-bottom:1px solid #223}
#log{flex:1;overflow:auto;padding:1rem;white-space:pre-wrap}
.eu{color:#9cf}.agente{color:#e8ecf8}
form{display:flex;gap:.5rem;padding:.6rem 1rem;border-top:1px solid #223}
input,textarea{background:#0b1030;color:#e8ecf8;border:1px solid #334;padding:.4rem}
textarea{flex:1;height:3rem}
</style></head><body>
<header>Chave <input id="chave" type="password" size="18"> Conversa <input id="conversa" value="web" size="10"></header>
<div id="log" aria-live="polite"></div>
<form id="f"><textarea id="texto" placeholder="Escreva e aperte Enviar"></textarea><button>Enviar</button></form>
<script>
const $=id=>document.getElementById(id);
$('chave').value=sessionStorage.getItem('chave')||'';
let desde=0;
const cab=()=>({'Authorization':'Bearer '+$('chave').value,'Content-Type':'application/json'});
function linha(quem,t){const d=document.createElement('div');d.className=quem;d.textContent=(quem==='eu'?'você: ':'agente: ')+t;$('log').appendChild(d);$('log').scrollTop=1e9;}
$('f').onsubmit=async e=>{e.preventDefault();sessionStorage.setItem('chave',$('chave').value);
 const t=$('texto').value.trim();if(!t)return;
 const r=await fetch('/webchat/mensagem',{method:'POST',headers:cab(),body:JSON.stringify({conversa:$('conversa').value,texto:t})});
 if(r.ok){linha('eu',t);$('texto').value='';}else{linha('agente','(recusado: '+r.status+')');}};
setInterval(async()=>{if(!$('chave').value)return;
 const r=await fetch('/webchat/respostas?conversa='+encodeURIComponent($('conversa').value)+'&desde='+desde,{headers:cab()});
 if(!r.ok)return;for(const m of await r.json()){linha('agente',m.texto);desde=m.seq+1;}},2000);
</script></body></html>"#;
