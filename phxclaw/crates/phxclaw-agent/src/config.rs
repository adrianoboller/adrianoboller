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
//! Fase 2 (SP000020): os modulos leem por aqui -- `texto_de`, `inteiro_de`, `lista_de`,
//! `caminho_de`, `segredo_do_ambiente` e `por_variavel` --, e a catraca de
//! `tools/config_catalogo.py` reprova a leitura solta que voltar. O nome da variavel que
//! uma mensagem cita sai de `variavel(chave)`, do catalogo, nunca digitado no modulo.
//!
//! Os perfis (`perfis`/`perfil_ativo` do arquivo da pasta) entram pelo MESMO caminho: a
//! vista os lista, `perfil_criar`/`perfil_usar` gravam pela `carga::gravar_com`, e a CLI
//! `config perfil` e as rotas `/v1/config/perfis` chamam estas funcoes -- nao ha uma
//! segunda regra para o que e um perfil valido.

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
pub const ARQUIVO: &str = carga::ARQUIVO;

/// A pasta do agente quando ninguem passa `--pasta`: `PHXCLAW_HOME`, ou `var/agente` -- a
/// MESMA conta do leitor de processo do config-runtime, que os crates sem o agente usam.
pub fn pasta_padrao() -> PathBuf {
    carga::pasta_do_processo()
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
    let Some(raiz) = raiz_do_projeto() else {
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

/// A configuracao efetiva da pasta (ambiente > projeto confiado > perfil ativo > pasta >
/// padrao).
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
    // Os crates abaixo do agente (navegador, busca, pontes) leem pelo config-runtime: a
    // pasta fixada tem de ser a mesma, senao `--pasta X` valeria para o agente e nao para
    // o Chromium que ele lanca.
    carga::fixar_pasta_do_processo(pasta);
    let mut g = estado();
    if g.carregada.as_ref().is_some_and(|(p, _)| p != pasta) {
        g.carregada = None;
    }
    g.fixada = Some(pasta.to_path_buf());
}

/// Fixa a pasta do processo e carrega. Sem fixar, vale `pasta_padrao()` na leitura.
pub fn iniciar(pasta: &Path) -> Result<(), String> {
    let c = carregar(pasta).map_err(|e| carga::em_texto(&e))?;
    carga::fixar_pasta_do_processo(pasta);
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

// --- os leitores de quem nao tem como devolver erro -----------------------------------

/// A configuracao para o leitor que nao devolve erro (construtor `-> Self`, campo
/// `Option`): arquivo invalido vira UM aviso no stderr por processo e a chave vale `None`.
/// A recusa de verdade acontece na porta -- o `main.rs` carrega antes de despachar
/// qualquer comando que nao seja `config` --, entao aqui so chega quem usa o crate como
/// biblioteca. Seguir calado nao e opcao: o aviso diz a chave e o arquivo.
fn tolerante() -> Option<Arc<Configuracao>> {
    match configuracao() {
        Ok(c) => Some(c),
        Err(e) => {
            static AVISADO: OnceLock<()> = OnceLock::new();
            AVISADO.get_or_init(|| eprintln!("aviso: config.json: {e}"));
            None
        }
    }
}

pub fn texto_de(chave: &str) -> Option<String> {
    tolerante()?.texto(chave).filter(|v| !v.trim().is_empty())
}

pub fn caminho_de(chave: &str) -> Option<PathBuf> {
    texto_de(chave).map(PathBuf::from)
}

pub fn inteiro_de(chave: &str) -> Option<i64> {
    tolerante()?.inteiro(chave)
}

pub fn booleano_de(chave: &str) -> Option<bool> {
    tolerante()?.booleano(chave)
}

pub fn lista_de(chave: &str) -> Option<Vec<String>> {
    tolerante()?.lista(chave)
}

/// O nome da variavel de ambiente da chave, para a mensagem que diz ao operador o que
/// definir. Sai do catalogo: chave fora dele e erro de programacao.
pub fn variavel(chave: &str) -> &'static str {
    catalogo_do_config::por_chave(chave)
        .map(|c| c.variavel.as_str())
        .unwrap_or_else(|| panic!("chave fora do catalogo: {chave}"))
}

/// Um segredo do catalogo, SO do ambiente (o broker e de quem tem a pasta: `chaves.rs`).
/// `valor` devolve `None` para segredo de proposito; quem precisa do texto le por aqui,
/// e a leitura fica no modulo de configuracao, nao espalhada.
pub fn segredo_do_ambiente(chave: &str) -> Option<String> {
    let c = catalogo_do_config::por_chave(chave)
        .unwrap_or_else(|| panic!("chave fora do catalogo: {chave}"));
    debug_assert!(c.segredo(), "{chave} nao e segredo no catalogo");
    ambiente(&c.variavel).map(|v| v.trim().to_string())
}

/// O leitor por NOME DE VARIAVEL, na forma de texto do ambiente, para quem recebe o nome
/// montado em tempo de execucao (`canais::ligar`, `PHXCLAW_<CANAL>_<CHAVE>`): resolve a
/// chave pelo catalogo e le pelo ponto unico -- lista volta unida pelo separador dela,
/// booleano como `true`/`false`. Segredo le o ambiente. Nome fora do catalogo: `None`.
pub fn por_variavel(var: &str) -> Option<String> {
    use phxclaw_config_runtime::agente::Tipo;
    let c = catalogo_do_config::por_variavel(var)?;
    if c.segredo() {
        return ambiente(&c.variavel);
    }
    let cfg = tolerante()?;
    match c.tipo {
        Tipo::Lista(sep) => cfg
            .lista(&c.chave)
            .filter(|l| !l.is_empty())
            .map(|l| l.join(&sep.to_string())),
        _ => cfg.texto(&c.chave).filter(|v| !v.trim().is_empty()),
    }
}

/// A pasta do projeto em que o agente trabalha: `PHXCLAW_PROJETO` (`agente.projeto`, so
/// ambiente), ou a pasta corrente. Le o ambiente direto porque e ela que LOCALIZA o
/// `.phxclaw/config.json` do projeto: passar pelo `valor` carregaria a configuracao para
/// saber onde carregar a configuracao.
pub fn raiz_do_projeto() -> Option<PathBuf> {
    ambiente("PHXCLAW_PROJETO")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
}

/// Esquece a configuracao carregada (depois de gravar, a proxima leitura rele). A pasta
/// fixada FICA: e ela que a proxima leitura rele.
pub(crate) fn esquecer(pasta: &Path) {
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
        "perfis": perfis_de(&cfg),
        "chaves": chaves,
    }))
}

/// O bloco dos perfis da vista: o ativo (e de onde veio) e cada perfil com as chaves que
/// ele sobrepoe. Segredo nunca entra num perfil (a carga recusa), entao os valores podem
/// aparecer.
fn perfis_de(cfg: &Configuracao) -> Value {
    let lista: Vec<Value> = cfg
        .pasta
        .perfis
        .iter()
        .map(|(nome, chaves)| {
            json!({
                "nome": nome,
                "chaves": chaves,
                "ativo": cfg.perfil_ativo.as_ref().is_some_and(|(a, _)| a == nome),
            })
        })
        .collect();
    json!({
        "ativo": cfg.perfil_ativo.as_ref().map(|(n, _)| n.clone()),
        "origem_do_ativo": cfg.perfil_ativo.as_ref().map(|(_, o)| o.nome()),
        "lista": lista,
    })
}

/// `GET /v1/config/perfis` e `config perfil listar`: so o bloco dos perfis.
pub fn perfis(pasta: &Path) -> Result<Value, Vec<Erro>> {
    Ok(perfis_de(&carregar(pasta)?))
}

/// O que se faz com um perfil, pela CLI ou pela API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcaoDePerfil {
    /// Cria vazio, ou copiando as chaves da base.
    Criar { nome: String, copiar_base: bool },
    /// Ativa (`None` desativa).
    Usar(Option<String>),
}

/// Cria ou ativa um perfil no arquivo da PASTA (perfil e da maquina, nunca do projeto).
pub fn perfil(pasta: &Path, acao: AcaoDePerfil, if_match: Option<&str>) -> Result<String, Recusa> {
    static GRAVANDO: Mutex<()> = Mutex::new(());
    let _g = GRAVANDO.lock().unwrap_or_else(|p| p.into_inner());
    let arquivo = arquivo_da_pasta(pasta);
    let cfg = carregar(pasta).map_err(Recusa::Invalida)?;
    let alvo = Alvo {
        arquivo: &arquivo,
        atual: &cfg,
        esperada: if_match,
        perfil: None,
    };
    let r = match acao {
        AcaoDePerfil::Criar { nome, copiar_base } => carga::perfil_criar(alvo, &nome, copiar_base),
        AcaoDePerfil::Usar(nome) => carga::perfil_usar(alvo, nome.as_deref()),
    };
    esquecer(pasta);
    r
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Escopo {
    Pasta,
    Projeto,
    /// Um perfil do arquivo da pasta.
    Perfil(String),
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
    let arquivo = match (&escopo, projeto(pasta)) {
        (Escopo::Pasta | Escopo::Perfil(_), _) => arquivo_da_pasta(pasta),
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
            perfil: match &escopo {
                Escopo::Perfil(n) => Some(n.as_str()),
                _ => None,
            },
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
    let Some(if_match) = if_match_de(&h) else {
        return (
            StatusCode::PRECONDITION_REQUIRED,
            Json(json!({"error": "falta If-Match com a revisao lida no GET /v1/config"})),
        )
            .into_response();
    };
    let escopo = match (
        corpo.get("escopo").and_then(Value::as_str),
        corpo.get("perfil").and_then(Value::as_str),
    ) {
        (Some("pasta"), None) => Escopo::Pasta,
        (Some("projeto"), None) => Escopo::Projeto,
        (Some("perfil") | None, Some(n)) => Escopo::Perfil(n.to_string()),
        _ => {
            return recusa_http(Recusa::Invalida(vec![Erro {
                chave: "escopo".into(),
                motivo: "esperado \"pasta\", \"projeto\" ou \"perfil\" (com \"perfil\": nome)"
                    .into(),
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

fn if_match_de(h: &HeaderMap) -> Option<String> {
    h.get(axum::http::header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
}

async fn obter_perfis(State(s): State<ApiState>, h: HeaderMap) -> Response {
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    let pasta = pasta_da_api(&s);
    match tokio::task::spawn_blocking(move || perfis(&pasta)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => (StatusCode::UNPROCESSABLE_ENTITY, Json(erros_json(&e))).into_response(),
        Err(e) => recusa_http(Recusa::Falha(e.to_string())),
    }
}

/// `PUT /v1/config/perfis` com `{"criar": nome, "copiar_base": bool}` ou `{"usar": nome|null}`,
/// e o mesmo If-Match do `PUT /v1/config`.
async fn gravar_perfil(State(s): State<ApiState>, h: HeaderMap, corpo: Json<Value>) -> Response {
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    let Some(if_match) = if_match_de(&h) else {
        return (
            StatusCode::PRECONDITION_REQUIRED,
            Json(json!({"error": "falta If-Match com a revisao lida no GET /v1/config"})),
        )
            .into_response();
    };
    let acao = match (corpo.get("criar"), corpo.get("usar")) {
        (Some(Value::String(n)), None) => AcaoDePerfil::Criar {
            nome: n.clone(),
            copiar_base: corpo
                .get("copiar_base")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        (None, Some(Value::String(n))) => AcaoDePerfil::Usar(Some(n.clone())),
        (None, Some(Value::Null)) => AcaoDePerfil::Usar(None),
        _ => {
            return recusa_http(Recusa::Invalida(vec![Erro {
                chave: "perfil".into(),
                motivo: "esperado {\"criar\": nome} ou {\"usar\": nome|null}".into(),
            }]));
        }
    };
    let pasta = pasta_da_api(&s);
    match tokio::task::spawn_blocking(move || perfil(&pasta, acao, Some(&if_match))).await {
        Ok(Ok(n)) => Json(json!({"revisao": n})).into_response(),
        Ok(Err(r)) => recusa_http(r),
        Err(e) => recusa_http(Recusa::Falha(e.to_string())),
    }
}

/// `GET`/`PUT /v1/config`, `GET`/`PUT /v1/config/perfis` e a sincronizacao
/// (`sincronizar.rs`), com o mesmo Bearer das outras rotas.
pub fn rotas() -> Router<ApiState> {
    Router::new()
        .route("/v1/config", get(obter).put(gravar))
        .route("/v1/config/perfis", get(obter_perfis).put(gravar_perfil))
        .merge(crate::sincronizar::rotas())
}
