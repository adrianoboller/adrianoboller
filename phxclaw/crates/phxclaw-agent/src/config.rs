//! O ponto unico de leitura da configuracao do agente: `config::valor("modelo.padrao")`.
//!
//! O catalogo, a precedencia e a recusa moram no `phxclaw-config-runtime::agente`; aqui
//! fica o que so o agente sabe responder -- onde e a pasta, qual projeto esta CONFIADO, e
//! se um segredo esta no broker -- e as duas portas de fora (a CLI `phxclaw config` e a
//! rota `/v1/config`), que chamam as MESMAS `vista` e `definir`: a tela e o terminal nao
//! podem ter duas regras para o que se grava.
//!
//! O projeto so entra se a raiz dele foi confiada (`phxclaw projeto confiar`), pela mesma
//! lista das instrucoes do projeto: o `.phxclaw/config.json` de um repositorio clonado
//! poderia apontar `voz.whisper.bin` para um executavel do proprio repositorio.
//!
//! Fase 1: os modulos ainda leem o ambiente direto; a catraca de `tools/config_catalogo.py`
//! impede leitura nova fora daqui, e a fase 2 migra os leitores para `valor`.

use crate::api::ApiState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use phxclaw_config_runtime::agente::carga::{self, Alvo, Erro, Origem, Recusa};
use phxclaw_config_runtime::agente::{Chave, Configuracao, Natureza, catalogo, gerar};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

pub use phxclaw_config_runtime::agente as catalogo_do_config;

/// O nome do arquivo, na pasta do agente e em `.phxclaw/` do projeto.
pub const ARQUIVO: &str = "config.json";

/// A pasta do agente quando ninguem passa `--pasta`: `PHXCLAW_HOME`, ou `var/agente`.
pub fn pasta_padrao() -> PathBuf {
    ambiente("PHXCLAW_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("var/agente"))
}

fn ambiente(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

pub fn arquivo_da_pasta(pasta: &Path) -> PathBuf {
    pasta.join(ARQUIVO)
}

/// O arquivo do projeto e se ele vale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Projeto {
    Confiado(PathBuf),
    /// Ha projeto, mas a raiz nao foi confiada: o arquivo, se existir, e ignorado.
    NaoConfiado {
        arquivo: PathBuf,
        raiz: PathBuf,
    },
    Nenhum,
}

impl Projeto {
    pub fn arquivo_valido(&self) -> Option<&Path> {
        match self {
            Projeto::Confiado(p) => Some(p),
            _ => None,
        }
    }
}

/// O projeto de agora (`PHXCLAW_PROJETO` ou a pasta corrente, a mesma nocao da montagem),
/// julgado pela lista de confiados da pasta do agente.
pub fn projeto(pasta: &Path) -> Projeto {
    let Some(raiz) = crate::montagem::raiz_do_projeto() else {
        return Projeto::Nenhum;
    };
    let arquivo = raiz.join(".phxclaw").join(ARQUIVO);
    // A MESMA conta das instrucoes do projeto (`instrucoes::confiado`): confianca julgada
    // por uma raiz e leitura feita de outra era o achado M5.
    match crate::instrucoes::confiado(pasta, &raiz) {
        Some(_) => Projeto::Confiado(arquivo),
        None => Projeto::NaoConfiado {
            arquivo,
            raiz: crate::instrucoes::raiz_canonica(&raiz),
        },
    }
}

/// A configuracao efetiva da pasta (ambiente > projeto confiado > pasta > padrao).
pub fn carregar(pasta: &Path) -> Result<Configuracao, Vec<Erro>> {
    let p = projeto(pasta);
    carga::carregar(&ambiente, &arquivo_da_pasta(pasta), p.arquivo_valido())
}

// --- o ponto unico -------------------------------------------------------------------

type Carregada = (PathBuf, Arc<Configuracao>);

/// O estado do processo: a pasta FIXADA (o `--pasta` da CLI) e a configuracao carregada dela.
/// Ficam separadas porque o `esquecer` (depois de gravar) solta so a carregada: juntas, ele
/// apagava a pasta junto, e a leitura seguinte recarregava da `pasta_padrao()` -- o PUT
/// gravava em `X/config.json` e o processo passava a ler `var/agente` (prova F, 01/10).
#[derive(Default)]
struct Estado {
    fixada: Option<PathBuf>,
    carregada: Option<Carregada>,
}

fn atual() -> &'static Mutex<Estado> {
    static A: OnceLock<Mutex<Estado>> = OnceLock::new();
    A.get_or_init(|| Mutex::new(Estado::default()))
}

fn estado() -> std::sync::MutexGuard<'static, Estado> {
    atual().lock().unwrap_or_else(|p| p.into_inner())
}

/// Fixa a pasta do processo SEM carregar: a CLI chama no comeco de todo comando, e o
/// arquivo invalido continua erro so de quem le (o `phxclaw config` que o conserta nao pode
/// morrer por ele).
pub fn fixar_pasta(pasta: &Path) {
    let mut g = estado();
    if g.carregada.as_ref().is_some_and(|(p, _)| p != pasta) {
        g.carregada = None;
    }
    g.fixada = Some(pasta.to_path_buf());
}

/// Fixa a pasta do processo e carrega. Sem fixar, vale `pasta_padrao()` na leitura.
pub fn iniciar(pasta: &Path) -> Result<(), String> {
    let c = carregar(pasta).map_err(|e| carga::em_texto(&e))?;
    let mut g = estado();
    g.fixada = Some(pasta.to_path_buf());
    g.carregada = Some((pasta.to_path_buf(), Arc::new(c)));
    Ok(())
}

/// A configuracao do processo. Arquivo invalido e ERRO, nao «vale o padrao»: seguir com o
/// padrao calado faria uma restricao escrita no arquivo deixar de valer sem ninguem ver.
pub fn configuracao() -> Result<Arc<Configuracao>, String> {
    let mut g = estado();
    let pasta = g.fixada.clone().unwrap_or_else(pasta_padrao);
    if let Some((p, c)) = g.carregada.as_ref()
        && *p == pasta
    {
        return Ok(c.clone());
    }
    let c = Arc::new(carregar(&pasta).map_err(|e| carga::em_texto(&e))?);
    g.carregada = Some((pasta, c.clone()));
    Ok(c)
}

/// O valor efetivo da chave (`None`: nao definida e sem padrao; segredo e sempre `None`).
pub fn valor(chave: &str) -> Result<Option<Value>, String> {
    Ok(configuracao()?.valor(chave).cloned())
}

pub fn texto(chave: &str) -> Result<Option<String>, String> {
    Ok(configuracao()?.texto(chave))
}

/// Esquece a configuracao carregada (depois de gravar, a proxima leitura rele). A pasta
/// fixada FICA: e ela que a proxima leitura rele.
fn esquecer(pasta: &Path) {
    let mut g = estado();
    if g.carregada.as_ref().is_some_and(|(p, _)| p == pasta) {
        g.carregada = None;
    }
}

// --- segredo: so a presenca ----------------------------------------------------------

/// O segredo esta no ambiente ou guardado no broker? O broker so se abre se a chave-mestra
/// ja existe: perguntar nao pode criar um cofre vazio (a mesma regra de `chaves.rs`).
pub fn segredo_presente(pasta: &Path, c: &Chave) -> bool {
    if ambiente(&c.variavel).is_some() {
        return true;
    }
    let Natureza::Segredo {
        referencia: Some(r),
        ..
    } = &c.natureza
    else {
        return false;
    };
    let dir = pasta.join(r.subpasta);
    if !dir.join("segredos/master.key").exists() {
        return false;
    }
    crate::canais::broker_em(&dir)
        .and_then(|b| crate::canais::segredo_guardado(&b, &r.nome, r.espaco))
        .is_ok_and(|d| d.is_some())
}

// --- a vista (GET e `config mostrar`) e a gravacao (PUT e `config definir`) -----------

fn motivo_nao_editavel(c: &Chave, origem: Origem) -> Option<String> {
    match &c.natureza {
        Natureza::Segredo { comando, .. } => Some(format!("segredo: guarde com `{comando}`")),
        Natureza::Ambiente { motivo } => Some(format!("só ambiente: {motivo}")),
        Natureza::Config if origem == Origem::Ambiente => Some("vem do ambiente".into()),
        Natureza::Config => None,
    }
}

/// O contrato de `GET /v1/config`. Segredo NUNCA traz valor: so `segredo_presente`.
pub fn vista(pasta: &Path) -> Result<Value, Vec<Erro>> {
    let cfg = carregar(pasta)?;
    let p = projeto(pasta);
    let chaves: Vec<Value> = catalogo()
        .iter()
        .map(|c| {
            let e = cfg.efetivo(&c.chave).cloned().unwrap_or(carga::Efetivo {
                valor: None,
                origem: Origem::Ausente,
            });
            let mut m = gerar::entrada(c);
            let origem = match e.origem {
                // O contrato tem quatro origens; sem valor nem padrao, o valor e o padrao
                // (nulo).
                Origem::Ausente => "padrao",
                o => o.nome(),
            };
            m.insert(
                "valor".into(),
                if c.segredo() {
                    Value::Null
                } else {
                    e.valor.unwrap_or(Value::Null)
                },
            );
            m.insert("origem".into(), json!(origem));
            m.insert(
                "segredo_presente".into(),
                json!(c.segredo() && segredo_presente(pasta, c)),
            );
            let motivo = motivo_nao_editavel(c, e.origem);
            m.insert("editavel".into(), json!(motivo.is_none()));
            m.insert("motivo_nao_editavel".into(), json!(motivo));
            Value::Object(m)
        })
        .collect();
    let (arq_projeto, ignorado) = match &p {
        Projeto::Confiado(a) => (json!(a.display().to_string()), Value::Null),
        Projeto::NaoConfiado { arquivo, raiz } if arquivo.exists() => (
            Value::Null,
            json!(format!(
                "{} ignorado: projeto nao confiado (phxclaw projeto confiar {})",
                arquivo.display(),
                raiz.display()
            )),
        ),
        _ => (Value::Null, Value::Null),
    };
    Ok(json!({
        "revisao": cfg.revisao(),
        "arquivos": {
            "pasta": arquivo_da_pasta(pasta).display().to_string(),
            "projeto": arq_projeto,
            "projeto_ignorado": ignorado,
        },
        "chaves": chaves,
    }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escopo {
    Pasta,
    Projeto,
}

/// Grava `mudancas` (`null` remove) no arquivo do escopo. `if_match`: a revisao que o
/// cliente viu (`None`: a de agora). A gravacao e a do config-runtime (revisao, historico,
/// troca atomica); este cadeado so serializa conferir-e-gravar dentro do processo.
pub fn definir(
    pasta: &Path,
    escopo: Escopo,
    mudancas: &Map<String, Value>,
    if_match: Option<&str>,
) -> Result<String, Recusa> {
    static GRAVANDO: Mutex<()> = Mutex::new(());
    let _g = GRAVANDO.lock().unwrap_or_else(|p| p.into_inner());
    let arquivo = match (escopo, projeto(pasta)) {
        (Escopo::Pasta, _) => arquivo_da_pasta(pasta),
        (Escopo::Projeto, Projeto::Confiado(a)) => a,
        (Escopo::Projeto, Projeto::NaoConfiado { raiz, .. }) => {
            return Err(Recusa::Invalida(vec![Erro {
                chave: "escopo".into(),
                motivo: format!(
                    "projeto {} nao confiado: rode `phxclaw projeto confiar {}` antes",
                    raiz.display(),
                    raiz.display()
                ),
            }]));
        }
        (Escopo::Projeto, Projeto::Nenhum) => {
            return Err(Recusa::Invalida(vec![Erro {
                chave: "escopo".into(),
                motivo: "nenhum projeto (PHXCLAW_PROJETO ou a pasta corrente)".into(),
            }]));
        }
    };
    let cfg = carregar(pasta).map_err(Recusa::Invalida)?;
    let r = carga::definir(
        Alvo {
            arquivo: &arquivo,
            atual: &cfg,
            esperada: if_match,
        },
        mudancas,
    );
    esquecer(pasta);
    r
}

// --- a rota ----------------------------------------------------------------------------

fn pasta_da_api(s: &ApiState) -> PathBuf {
    let r = s.store.root();
    r.parent().unwrap_or(r).to_path_buf()
}

fn erros_json(e: &[Erro]) -> Value {
    json!({"erros": e.iter().map(|e| json!({"chave": e.chave, "motivo": e.motivo})).collect::<Vec<_>>()})
}

fn recusa_http(r: Recusa) -> Response {
    match r {
        Recusa::Conflito { atual } => {
            (StatusCode::CONFLICT, Json(json!({"revisao_atual": atual}))).into_response()
        }
        Recusa::Invalida(e) => {
            (StatusCode::UNPROCESSABLE_ENTITY, Json(erros_json(&e))).into_response()
        }
        Recusa::Falha(m) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": m}))).into_response()
        }
    }
}

async fn obter(State(s): State<ApiState>, h: HeaderMap) -> Response {
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    let pasta = pasta_da_api(&s);
    match tokio::task::spawn_blocking(move || vista(&pasta)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => (StatusCode::UNPROCESSABLE_ENTITY, Json(erros_json(&e))).into_response(),
        Err(e) => recusa_http(Recusa::Falha(e.to_string())),
    }
}

async fn gravar(State(s): State<ApiState>, h: HeaderMap, corpo: Json<Value>) -> Response {
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    let Some(if_match) = h
        .get(axum::http::header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
    else {
        return (
            StatusCode::PRECONDITION_REQUIRED,
            Json(json!({"error": "falta If-Match com a revisao lida no GET /v1/config"})),
        )
            .into_response();
    };
    let escopo = match corpo.get("escopo").and_then(Value::as_str) {
        Some("pasta") => Escopo::Pasta,
        Some("projeto") => Escopo::Projeto,
        _ => {
            return recusa_http(Recusa::Invalida(vec![Erro {
                chave: "escopo".into(),
                motivo: "esperado \"pasta\" ou \"projeto\"".into(),
            }]));
        }
    };
    let Some(valores) = corpo.get("valores").and_then(Value::as_object).cloned() else {
        return recusa_http(Recusa::Invalida(vec![Erro {
            chave: "valores".into(),
            motivo: "esperado objeto {\"chave\": valor|null}".into(),
        }]));
    };
    let pasta = pasta_da_api(&s);
    let r = tokio::task::spawn_blocking(move || definir(&pasta, escopo, &valores, Some(&if_match)))
        .await;
    match r {
        Ok(Ok(n)) => Json(json!({"revisao": n})).into_response(),
        Ok(Err(r)) => recusa_http(r),
        Err(e) => recusa_http(Recusa::Falha(e.to_string())),
    }
}

/// `GET /v1/config` e `PUT /v1/config`, com o mesmo Bearer das outras rotas.
pub fn rotas() -> Router<ApiState> {
    Router::new().route("/v1/config", get(obter).put(gravar))
}
