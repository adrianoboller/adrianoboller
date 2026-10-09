//! Os cofres externos (`cofres/`): HashiCorp Vault KV v2, AWS Secrets Manager (SigV4), Azure
//! Key Vault e GCP Secret Manager contra servidores FALSOS locais (axum) -- sem rede externa
//! --, e a SigV4 e o RS256 da casa contra os vetores oficiais (`tests/dados/cofres`).
//!
//! Contra os servicos REAIS: NAO MEDIDO (sem contas de teste). O binario `vault` nao esta
//! instalado neste ambiente (09/10/2026): o Vault real tambem nao foi medido.
//!
//! Prova real: os testes marcados «RED medido» tiveram o defeito reposto de verdade (linha
//! marcada `// REPOSTO`, recompilada, vista cair pelo motivo certo) e o conserto voltou.

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use phxclaw_agent::canais::jwt::assinar_rs256;
use phxclaw_agent::canais::rsa::{ChavePrivada, verificar_pkcs1_sha256_sem_politica};
use phxclaw_agent::cofres::{
    self, CfgAws, CfgAzure, CfgGcp, CfgVault, ConfigCofres, MetodoVault, sigv4,
};
use phxclaw_agent::fluxo_http;
use phxclaw_secret_broker::ReferenciaExterna;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ------------------------------------------------------------------ apoio

struct Pasta(PathBuf);

impl std::ops::Deref for Pasta {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmp(nome: &str) -> Pasta {
    let d = std::env::temp_dir().join(format!(
        "phx-cofre-{nome}-{}",
        phxclaw_types::new_uuid_v7().simple()
    ));
    std::fs::create_dir_all(&d).unwrap();
    Pasta(d)
}

fn dados() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dados/cofres")
}

fn refe(
    cofre: &str,
    caminho: &str,
    campo: Option<&str>,
    versao: Option<&str>,
) -> ReferenciaExterna {
    ReferenciaExterna {
        cofre: cofre.into(),
        caminho: caminho.into(),
        campo: campo.map(str::to_string),
        versao: versao.map(str::to_string),
    }
}

/// Todo byte de todo arquivo sob `d`, como texto (para procurar valor vazado).
fn tudo_em(d: &Path) -> String {
    let mut s = String::new();
    let mut pilha = vec![d.to_path_buf()];
    while let Some(p) = pilha.pop() {
        for e in std::fs::read_dir(&p).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if let Ok(b) = std::fs::read(&p) {
                s.push_str(&String::from_utf8_lossy(&b));
            }
        }
    }
    s
}

async fn servir(app: Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    base
}

fn resposta(status: u16, v: Value) -> Response {
    (StatusCode::from_u16(status).unwrap(), axum::Json(v)).into_response()
}

fn cabecalho(h: &HeaderMap, k: &str) -> String {
    h.get(k)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn form(corpo: &[u8]) -> BTreeMap<String, String> {
    url::form_urlencoded::parse(corpo).into_owned().collect()
}

fn config(liberar: &str) -> ConfigCofres {
    ConfigCofres {
        liberar: vec![liberar.to_string()],
        ..Default::default()
    }
}

// ------------------------------------------------------------------ vetores oficiais

/// Um pedido lido de um `.req`.
struct Req {
    metodo: String,
    caminho: String,
    query: Vec<(String, String)>,
    cabecalhos: Vec<(String, String)>,
    corpo: Vec<u8>,
}

/// Um `.req` da suite: linha de pedido, cabecalhos (a linha que comeca com branco continua a
/// anterior) e, depois da linha em branco, o corpo.
fn ler_req(t: &str) -> Req {
    let (cabeca, corpo) = match t.split_once("\n\n") {
        Some((c, b)) => (c, b.as_bytes().to_vec()),
        None => (t, vec![]),
    };
    let mut linhas = cabeca.lines();
    let primeira = linhas.next().unwrap();
    let metodo = primeira.split(' ').next().unwrap().to_string();
    let alvo = primeira[metodo.len() + 1..primeira.rfind(" HTTP/").unwrap()].to_string();
    let (caminho, q) = alvo.split_once('?').unwrap_or((&alvo, ""));
    let query = q
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (k.to_string(), v.to_string())
        })
        .collect();
    let mut cabecalhos: Vec<(String, String)> = Vec::new();
    for l in linhas {
        if l.starts_with([' ', '\t']) {
            let ultimo = cabecalhos.last_mut().unwrap();
            ultimo.1.push(' ');
            ultimo.1.push_str(l.trim());
        } else {
            let (k, v) = l.split_once(':').unwrap();
            cabecalhos.push((k.to_string(), v.to_string()));
        }
    }
    Req {
        metodo,
        caminho: caminho.to_string(),
        query,
        cabecalhos,
        corpo,
    }
}

/// A SigV4 da casa contra a suite oficial da AWS: a requisicao canonica, o texto a assinar e
/// o `Authorization`, byte a byte, nos 31 casos.
///
/// RED medido em 09/10: com `caminho_canonico` sem a normalizacao de `.` e `..`
/// (`// REPOSTO` em `sigv4.rs`), 12 conferencias cairam -- creq, sts e authz de
/// `get-relative`, `get-relative-relative`, `get-slash-dot-slash` e `get-slash-pointless-dot`.
#[test]
fn sigv4_confere_byte_a_byte_com_a_suite_oficial() {
    let mut casos = Vec::new();
    let mut pilha = vec![dados().join("aws4")];
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if p.extension().is_some_and(|x| x == "req") {
                casos.push(p);
            }
        }
    }
    casos.sort();
    assert_eq!(casos.len(), 31, "a suite inteira tem de estar la");
    let mut falhas = Vec::new();
    for req in &casos {
        let ler = |ext: &str| std::fs::read_to_string(req.with_extension(ext)).unwrap();
        let Req {
            metodo,
            caminho,
            query,
            cabecalhos,
            corpo,
        } = ler_req(&ler("req"));
        let data_hora = cabecalhos
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("x-amz-date"))
            .map(|(_, v)| v.trim().to_string())
            .unwrap();
        let p = sigv4::Pedido {
            metodo: &metodo,
            caminho: &caminho,
            query: &query,
            cabecalhos: &cabecalhos,
            corpo: &corpo,
        };
        let (canonica, _) = sigv4::requisicao_canonica(&p);
        let esc = sigv4::escopo(&data_hora, "us-east-1", "service");
        let sts = sigv4::texto_a_assinar(&data_hora, &esc, &canonica);
        let authz = sigv4::autorizacao(
            &p,
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "service",
            &data_hora,
        )
        .unwrap();
        let nome = req.file_stem().unwrap().to_string_lossy().to_string();
        for (o_que, nosso, oficial) in [
            ("creq", canonica, ler("creq")),
            ("sts", sts, ler("sts")),
            ("authz", authz, ler("authz")),
        ] {
            if nosso != oficial {
                falhas.push(format!(
                    "{nome}.{o_que}:\n{nosso}\n--- oficial ---\n{oficial}"
                ));
            }
        }
    }
    assert!(
        falhas.is_empty(),
        "{} falhas:\n{}",
        falhas.len(),
        falhas.join("\n\n")
    );
}

/// A entrada e a assinatura do Apendice A.2 da RFC 7515.
const RFC_CARGA: &[u8] =
    b"{\"iss\":\"joe\",\r\n \"exp\":1300819380,\r\n \"http://example.com/is_root\":true}";
const RFC_JWS: &str = "eyJhbGciOiJSUzI1NiJ9.eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ.cC4hiUPoj9Eetdgtv3hF80EGrhuB__dzERat0XF9g2VtQgr9PJbu3XOiZj5RZmh7AAuHIm4Bh-0Qc_lF5YKt_O8W2Fp5jujGbds9uJdbF9CUAr7t1dnZcAcQjbKBYNX4BAynRFdiuB--f_nZLgrnbyTyWzO75vRK5h6xBArLIARNPvkSjtQBMHlb1L07Qe7K0GarZRmB_eSN9383LcOLn6_dO--xi12jzDwusC-eOkHWEsqtFZESc6BfI7noOPqvhJ1phCnvWh6IeYI2w9QOYEUipUTI8np6LbgGY9Fs98rqVt5AXLIhWkWywlVmtVrBp0igcN_IoypGlUPQGe77Rw";
/// O `n` da chave do apendice (base64url), para o servidor falso do Google conferir.
const RFC_N: &str = "ofgWCuLjybRlzo0tZWJjNiuSfb4p4fAkd_wWJcyQoTbji9k0l8W26mPddxHmfHQp-Vaw-4qPCJrcS2mJPMEzP1Pt0Bm4d4QlL-yRT-SFd2lZS-pCgNMsD1W_YpRPEwOWvG6b32690r2jZ47soMZo9wGzjb_7OMg0LOL-bSf63kpaSHSXndS5z5rexMdbBYUsLA9e-KXBdQOS-UTo7WTBEMa2R2CapHg665xsmtdVMTBQY4uDZlxvb3qCo5ZwKh9kG4LT6_I5IhlJH7aGhyxXFvUK-DWNmoudF8NAco9_h9iaGNj8q2ethFkMLs91kzk2PAcDTW9gb54h4FRWyuXpoQ";

fn pem(nome: &str) -> String {
    std::fs::read_to_string(dados().join(nome)).unwrap()
}

/// O RS256 da casa reproduz o JWS do Apendice A.2 da RFC 7515 byte a byte, com a chave em
/// PKCS#8 e em PKCS#1; e a assinatura que nao confere com a chave publica nao sai.
///
/// RED medido em 09/10: com a subtracao final do Montgomery desligada (`// REPOSTO` em
/// `rsa.rs`, `mascara & 0`), a assinatura saiu errada e o auto-conferir a recusou -- este
/// teste e o do GCP cairam com «a assinatura RSA nao conferiu com a chave publica».
#[test]
fn rs256_reproduz_o_vetor_da_rfc7515_a2() {
    for arq in ["rfc7515_a2_pkcs8.pem", "rfc7515_a2_pkcs1.pem"] {
        let chave = ChavePrivada::de_pem(&pem(arq)).unwrap();
        assert_eq!(chave.bits(), 2048);
        assert_eq!(
            assinar_rs256(br#"{"alg":"RS256"}"#, RFC_CARGA, &chave).unwrap(),
            RFC_JWS,
            "{arq}"
        );
        assert!(
            !format!("{chave:?}").contains("Eq5x"),
            "o Debug nao mostra d"
        );
    }
    // `d` errado: a assinatura nao confere com `e` e nao sai.
    let n = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(RFC_N)
        .unwrap();
    let ruim = ChavePrivada::de_componentes(&n, &[1, 0, 1], &[0x12, 0x34, 0x56]).unwrap();
    let e = ruim.assinar(b"x").unwrap_err();
    assert!(e.contains("nao conferiu"), "{e}");
    // PEM que nao e chave privada RSA: erro sem conteudo.
    let e = ChavePrivada::de_pem("-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----")
        .unwrap_err();
    assert!(!e.contains("AAAA"), "{e}");
}

// ------------------------------------------------------------------ Vault falso

const VAULT_TOKEN: &str = "hvs.TOKEN-VAULT-teste-7Qx2";
const VAULT_SECRET_ID: &str = "sid-SEGREDO-approle-55k";
const VAULT_V1: &str = "senha-VAULT-v1-AAA111";
const VAULT_V2: &str = "senha-VAULT-v2-BBB222";

#[derive(Default)]
struct Vault {
    leituras: AtomicUsize,
    logins: AtomicUsize,
    emitidos: Mutex<Vec<String>>,
    namespaces: Mutex<Vec<String>>,
    revogar: AtomicBool,
}

async fn vault_falso(
    State(e): State<Arc<Vault>>,
    metodo: Method,
    uri: Uri,
    h: HeaderMap,
    corpo: Bytes,
) -> Response {
    e.namespaces
        .lock()
        .unwrap()
        .push(cabecalho(&h, "x-vault-namespace"));
    let caminho = uri.path();
    if metodo == Method::POST && caminho == "/v1/auth/approle/login" {
        let v: Value = serde_json::from_slice(&corpo).unwrap_or_default();
        if v["role_id"] != "papel-1" || v["secret_id"] != VAULT_SECRET_ID {
            return resposta(400, json!({"errors": ["invalid role or secret ID"]}));
        }
        let n = e.logins.fetch_add(1, Ordering::SeqCst);
        let t = format!("hvs.APPROLE-emitido-{n}");
        e.emitidos.lock().unwrap().push(t.clone());
        e.revogar.store(false, Ordering::SeqCst);
        return resposta(
            200,
            json!({"auth": {"client_token": t, "lease_duration": 600}}),
        );
    }
    let token = cabecalho(&h, "x-vault-token");
    let valido = token == VAULT_TOKEN
        || (!e.revogar.load(Ordering::SeqCst) && e.emitidos.lock().unwrap().contains(&token));
    if !valido {
        // O Vault que ecoa o pedido no erro: o token nao pode voltar ao chamador.
        return resposta(
            403,
            json!({"errors": [format!("permission denied for token {token}")]}),
        );
    }
    e.leituras.fetch_add(1, Ordering::SeqCst);
    let versao = uri
        .query()
        .and_then(|q| q.strip_prefix("version="))
        .map(|v| v.parse::<u64>().unwrap());
    match caminho {
        "/v1/secret/data/app/db" => {
            let (senha, ver) = match versao {
                Some(1) => (VAULT_V1, 1),
                _ => (VAULT_V2, 2),
            };
            resposta(
                200,
                json!({"data": {"data": {"usuario": "app", "senha": senha}, "metadata": {"version": ver}}}),
            )
        }
        "/v1/secret/data/app/unico" => resposta(
            200,
            json!({"data": {"data": {"token": "UNICO-valor-9z"}, "metadata": {"version": 1}}}),
        ),
        "/v1/secret/data/app/apagado" => resposta(
            404,
            json!({"data": {"data": null, "metadata": {"version": 3, "deletion_time": "2026-10-01T00:00:00Z"}}}),
        ),
        "/v1/secret/data/app/redir" => Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header("location", "http://127.0.0.1:9/roubo")
            .body(axum::body::Body::empty())
            .unwrap(),
        _ => resposta(404, json!({"errors": []})),
    }
}

async fn subir_vault() -> (String, Arc<Vault>) {
    let e = Arc::new(Vault::default());
    let base = servir(Router::new().fallback(vault_falso).with_state(e.clone())).await;
    (base, e)
}

fn cfg_vault(base: &str, metodo: MetodoVault) -> ConfigCofres {
    ConfigCofres {
        vault: Some(CfgVault {
            url: base.to_string(),
            namespace: Some("time-a".into()),
            montagem: "secret".into(),
            metodo,
            approle_montagem: "approle".into(),
            role_id: Some("papel-1".into()),
        }),
        ..config(base)
    }
}

/// Vault com token guardado: versao, campo, o campo unico sem `campo`, o caminho com varios
/// campos sem `campo` (erro com as CHAVES, nao os valores), a versao apagada, o 403 e o 3xx
/// -- e nenhum erro carrega o token, nem quando o Vault o ecoa.
///
/// RED medido em 09/10: com o corpo de erro do Vault posto no motivo (`// REPOSTO` em
/// `vault.rs`, `format!("{}: {}", motivo_do_status(..), v["errors"])`), o 403 devolveu
/// «permission denied for token hvs.…» e o teste caiu na asserção do token.
#[tokio::test]
async fn vault_token_le_versao_e_campo_e_diz_o_erro_sem_o_valor() {
    let (base, e) = subir_vault().await;
    let raiz = tmp("vault");
    cofres::VAULT_TOKEN.guardar(&raiz, VAULT_TOKEN).unwrap();
    let c = cofres::montar(&raiz, &cfg_vault(&base, MetodoVault::Token)).unwrap();
    let ler = |caminho: &str, campo: Option<&str>, versao: Option<&str>| {
        let r = refe("vault", caminho, campo, versao);
        let c = &c;
        async move { c.resolver("db", &r).await }
    };
    assert_eq!(
        ler("app/db", Some("senha"), None).await.unwrap().0.expose(),
        VAULT_V2
    );
    assert_eq!(
        ler("app/db", Some("senha"), Some("1"))
            .await
            .unwrap()
            .0
            .expose(),
        VAULT_V1
    );
    assert_eq!(
        ler("app/unico", None, None).await.unwrap().0.expose(),
        "UNICO-valor-9z"
    );
    let erro = ler("app/db", None, Some("2"))
        .await
        .unwrap_err()
        .to_string();
    assert!(erro.contains("usuario") && erro.contains("senha"), "{erro}");
    assert!(!erro.contains(VAULT_V2), "{erro}");
    let erro = ler("app/db", Some("nada"), Some("2"))
        .await
        .unwrap_err()
        .to_string();
    assert!(erro.contains("\"nada\" nao existe"), "{erro}");
    let erro = ler("app/apagado", None, None)
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(
        erro,
        "cofre vault: credencial db: a versao pedida foi apagada no Vault (404)"
    );
    let erro = ler("app/redir", None, None).await.unwrap_err().to_string();
    assert!(erro.contains("redirecionamento"), "{erro}");
    let erro = ler("app/../db", None, None).await.unwrap_err().to_string();
    assert!(erro.contains("caminho invalido"), "{erro}");
    let erro = ler("app/db", None, Some("dois"))
        .await
        .unwrap_err()
        .to_string();
    assert!(erro.contains("numero inteiro"), "{erro}");
    assert!(e.namespaces.lock().unwrap().iter().all(|n| n == "time-a"));
    // Token errado: o Vault responde 403 ecoando o token, e o erro sai sem ele.
    cofres::VAULT_TOKEN
        .guardar(&raiz, "hvs.ERRADO-ecoado-123")
        .unwrap();
    // Outra referencia: a de antes esta no cache e nem chegaria ao Vault.
    let erro = ler("app/db", Some("usuario"), Some("1"))
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(erro, "cofre vault: credencial db: permissao negada (403)");
    assert!(!erro.contains("ERRADO-ecoado"), "{erro}");
}

/// AppRole: um login so para varias leituras (o `client_token` fica em memoria), e o 403 de
/// um token revogado antes do prazo faz UM login novo. O token e o secret_id nunca vao ao
/// disco.
#[tokio::test]
async fn vault_approle_entra_uma_vez_e_renova_no_403() {
    let (base, e) = subir_vault().await;
    let raiz = tmp("approle");
    cofres::VAULT_SECRET_ID
        .guardar(&raiz, VAULT_SECRET_ID)
        .unwrap();
    let mut cfg = cfg_vault(&base, MetodoVault::Approle);
    cfg.cache_segundos = 0;
    let c = cofres::montar(&raiz, &cfg).unwrap();
    let r = refe("vault", "app/db", Some("senha"), None);
    for _ in 0..3 {
        assert_eq!(c.resolver("db", &r).await.unwrap().0.expose(), VAULT_V2);
    }
    assert_eq!(
        e.logins.load(Ordering::SeqCst),
        1,
        "um login para tres leituras"
    );
    e.revogar.store(true, Ordering::SeqCst);
    assert_eq!(c.resolver("db", &r).await.unwrap().0.expose(), VAULT_V2);
    assert_eq!(
        e.logins.load(Ordering::SeqCst),
        2,
        "o 403 fez um login novo"
    );
    let disco = tudo_em(&raiz);
    assert!(
        !disco.contains("APPROLE-emitido"),
        "o client_token foi ao disco"
    );
    assert!(
        !disco.contains(VAULT_SECRET_ID),
        "o secret_id foi ao disco em claro"
    );
    // Sem role_id, a configuracao nem monta.
    let mut sem = cfg_vault(&base, MetodoVault::Approle);
    sem.vault.as_mut().unwrap().role_id = None;
    let erro = cofres::montar(&raiz, &sem).err().unwrap();
    assert!(erro.contains("role_id"), "{erro}");
}

/// O cache curto: a segunda leitura nao vai ao cofre; com prazo zero, vai sempre.
#[tokio::test]
async fn cache_curto_serve_a_segunda_leitura_e_zero_desliga() {
    let (base, e) = subir_vault().await;
    let raiz = tmp("cache");
    cofres::VAULT_TOKEN.guardar(&raiz, VAULT_TOKEN).unwrap();
    let r = refe("vault", "app/db", Some("senha"), None);
    let c = cofres::montar(&raiz, &cfg_vault(&base, MetodoVault::Token)).unwrap();
    assert!(!c.resolver("db", &r).await.unwrap().1);
    assert!(
        c.resolver("db", &r).await.unwrap().1,
        "a segunda veio do cache"
    );
    assert_eq!(e.leituras.load(Ordering::SeqCst), 1);
    let mut sem_cache = cfg_vault(&base, MetodoVault::Token);
    sem_cache.cache_segundos = 0;
    let c = cofres::montar(&raiz, &sem_cache).unwrap();
    c.resolver("db", &r).await.unwrap();
    c.resolver("db", &r).await.unwrap();
    assert_eq!(e.leituras.load(Ordering::SeqCst), 3);
    assert_eq!(c.em_cache(), 0);
}

/// O que a rede da casa recusa antes de sair: HTTP sem TLS fora do loopback, URL com senha,
/// e o destino interno nao liberado (o Vault falso em 127.0.0.1 sem `cofres.liberar`).
#[tokio::test]
async fn rede_da_casa_recusa_sem_tls_e_interno_nao_liberado() {
    let raiz = tmp("rede");
    let mut cfg = cfg_vault("http://vault.exemplo:8200", MetodoVault::Token);
    let erro = cofres::montar(&raiz, &cfg).err().unwrap();
    assert!(erro.contains("so HTTPS"), "{erro}");
    cfg.vault.as_mut().unwrap().url = "https://eu:senha@vault.exemplo".into();
    let erro = cofres::montar(&raiz, &cfg).err().unwrap();
    assert!(
        erro.contains("usuario/senha") && !erro.contains("senha@"),
        "{erro}"
    );
    let (base, e) = subir_vault().await;
    cofres::VAULT_TOKEN.guardar(&raiz, VAULT_TOKEN).unwrap();
    let mut sem_liberar = cfg_vault(&base, MetodoVault::Token);
    sem_liberar.liberar.clear();
    let c = cofres::montar(&raiz, &sem_liberar).unwrap();
    let erro = c
        .resolver("db", &refe("vault", "app/db", Some("senha"), None))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        erro.contains("interno") || erro.contains("127.0.0.1"),
        "{erro}"
    );
    assert_eq!(e.leituras.load(Ordering::SeqCst), 0, "nada chegou ao Vault");
}

// ------------------------------------------------------------------ AWS falsa

const AWS_ID: &str = "AKIDTESTE0000000001";
const AWS_SEGREDO: &str = "segredo/AWS+teste-XYZ0123456789abcdef";
const AWS_SESSAO: &str = "sessao-AWS-token-temporario-777";
const AWS_ATUAL: &str = r#"{"usuario":"app","senha":"senha-AWS-atual-CCC333"}"#;
const AWS_ANTERIOR: &str = "senha-AWS-anterior-DDD444";

#[derive(Default)]
struct Aws {
    pedidos: AtomicUsize,
    assinados: Mutex<Vec<String>>,
}

/// O falso confere a assinatura como a AWS: recalcula com o segredo conhecido sobre o que
/// CHEGOU (os quatro cabecalhos que o Secrets Manager exige assinados, mais o token de
/// sessao se veio) e compara com o `Authorization`.
async fn aws_falsa(State(e): State<Arc<Aws>>, h: HeaderMap, corpo: Bytes) -> Response {
    e.pedidos.fetch_add(1, Ordering::SeqCst);
    let authz = cabecalho(&h, "authorization");
    let mut exigidos = vec!["content-type", "host", "x-amz-date", "x-amz-target"];
    if h.contains_key("x-amz-security-token") {
        exigidos.push("x-amz-security-token");
    }
    let cab: Vec<(String, String)> = exigidos
        .iter()
        .map(|k| (k.to_string(), cabecalho(&h, k)))
        .collect();
    let data = cabecalho(&h, "x-amz-date");
    let esperado = sigv4::autorizacao(
        &sigv4::Pedido {
            metodo: "POST",
            caminho: "/",
            query: &[],
            cabecalhos: &cab,
            corpo: &corpo,
        },
        AWS_ID,
        AWS_SEGREDO,
        "us-east-1",
        "secretsmanager",
        &data,
    );
    e.assinados.lock().unwrap().push(authz.clone());
    if esperado.ok().as_deref() != Some(authz.as_str())
        || cabecalho(&h, "x-amz-target") != "secretsmanager.GetSecretValue"
    {
        return resposta(
            400,
            json!({"__type": "InvalidSignatureException", "message": format!("recebi {authz}")}),
        );
    }
    let v: Value = serde_json::from_slice(&corpo).unwrap_or_default();
    match (
        v["SecretId"].as_str(),
        v["VersionId"].as_str(),
        v["VersionStage"].as_str(),
    ) {
        (Some("app/db"), None, None) | (Some("app/db"), Some("v-2"), None) => resposta(
            200,
            json!({"Name": "app/db", "VersionId": "v-2", "SecretString": AWS_ATUAL}),
        ),
        (Some("app/db"), None, Some("AWSPREVIOUS")) | (Some("app/db"), Some("v-1"), None) => {
            resposta(
                200,
                json!({"Name": "app/db", "VersionId": "v-1", "SecretString": AWS_ANTERIOR}),
            )
        }
        (Some("app/bin"), _, _) => resposta(
            200,
            json!({"Name": "app/bin", "SecretBinary": base64::engine::general_purpose::STANDARD.encode("BIN-valor-AWS-55")}),
        ),
        _ => resposta(
            400,
            json!({"__type": "com.amazonaws.secretsmanager#ResourceNotFoundException", "message": format!("Secrets Manager can't find {authz}")}),
        ),
    }
}

/// AWS: assinatura SigV4 conferida pelo falso, `AWSCURRENT` sem versao, `VersionId`,
/// `estagio:AWSPREVIOUS`, binario, campo do JSON, token de sessao assinado, e o erro pelo
/// `__type` -- sem o `Authorization` que o falso ecoa na `message`.
///
/// RED medido em 09/10: com o `X-Amz-Target` mandado mas FORA da lista assinada
/// (`// REPOSTO` em `aws.rs`), o falso respondeu `InvalidSignatureException` e o teste caiu
/// na primeira leitura.
#[tokio::test]
async fn aws_assina_e_le_versao_estagio_binario_e_erro_pelo_codigo() {
    let e = Arc::new(Aws::default());
    let base = servir(Router::new().fallback(aws_falsa).with_state(e.clone())).await;
    let raiz = tmp("aws");
    cofres::AWS_SEGREDO.guardar(&raiz, AWS_SEGREDO).unwrap();
    let cfg = ConfigCofres {
        aws: Some(CfgAws {
            regiao: "us-east-1".into(),
            url: Some(base.clone()),
            chave_id: AWS_ID.into(),
        }),
        cache_segundos: 0,
        ..config(&base)
    };
    let c = cofres::montar(&raiz, &cfg).unwrap();
    let ler = |caminho: &str, campo: Option<&str>, versao: Option<&str>| {
        let r = refe("aws", caminho, campo, versao);
        let c = &c;
        async move { c.resolver("banco", &r).await.map(|v| v.0) }
    };
    assert_eq!(ler("app/db", None, None).await.unwrap().expose(), AWS_ATUAL);
    assert_eq!(
        ler("app/db", Some("senha"), None).await.unwrap().expose(),
        "senha-AWS-atual-CCC333"
    );
    assert_eq!(
        ler("app/db", None, Some("v-1")).await.unwrap().expose(),
        AWS_ANTERIOR
    );
    assert_eq!(
        ler("app/db", None, Some("estagio:AWSPREVIOUS"))
            .await
            .unwrap()
            .expose(),
        AWS_ANTERIOR
    );
    assert_eq!(
        ler("app/bin", None, None).await.unwrap().expose(),
        "BIN-valor-AWS-55"
    );
    let erro = ler("app/nada", None, None).await.unwrap_err().to_string();
    assert_eq!(
        erro,
        "cofre aws: credencial banco: HTTP 400: ResourceNotFoundException"
    );
    // O segredo da AWS nunca viajou: so a assinatura.
    let assinados = e.assinados.lock().unwrap().clone();
    assert!(assinados.iter().all(|a| {
        a.starts_with(&format!("AWS4-HMAC-SHA256 Credential={AWS_ID}/"))
            && a.contains("SignedHeaders=content-type;host;x-amz-date;x-amz-target,")
            && !a.contains(AWS_SEGREDO)
    }));
    // Com token de sessao: vai assinado.
    cofres::AWS_TOKEN_SESSAO.guardar(&raiz, AWS_SESSAO).unwrap();
    assert_eq!(ler("app/db", None, None).await.unwrap().expose(), AWS_ATUAL);
    let ultimo = e.assinados.lock().unwrap().last().cloned().unwrap();
    assert!(
        ultimo.contains(
            "SignedHeaders=content-type;host;x-amz-date;x-amz-security-token;x-amz-target,"
        ),
        "{ultimo}"
    );
    // Regiao estranha nem monta.
    let mut ruim = cfg.clone();
    ruim.aws.as_mut().unwrap().regiao = "us-east-1/../x".into();
    assert!(cofres::montar(&raiz, &ruim).is_err());
    assert!(
        !tudo_em(&raiz).contains("CCC333"),
        "valor lido foi ao disco"
    );
}

// ------------------------------------------------------------------ Azure falso

const AZ_CLIENTE: &str = "cliente-azure-1";
const AZ_SEGREDO: &str = "segredo-AZURE-cliente-QQQ999";
const AZ_VALOR: &str = "valor-AZURE-kv-EEE555";

#[derive(Default)]
struct Azure {
    tokens: AtomicUsize,
    leituras: AtomicUsize,
    validos: Mutex<Vec<String>>,
    formularios: Mutex<Vec<BTreeMap<String, String>>>,
}

async fn azure_falso(
    State(e): State<Arc<Azure>>,
    metodo: Method,
    uri: Uri,
    h: HeaderMap,
    corpo: Bytes,
) -> Response {
    if metodo == Method::POST && uri.path() == "/tenant-x/oauth2/v2.0/token" {
        let f = form(&corpo);
        e.formularios.lock().unwrap().push(f.clone());
        if f.get("grant_type").map(String::as_str) != Some("client_credentials")
            || f.get("client_id").map(String::as_str) != Some(AZ_CLIENTE)
            || f.get("client_secret").map(String::as_str) != Some(AZ_SEGREDO)
            || f.get("scope").map(String::as_str) != Some("https://vault.azure.net/.default")
        {
            return resposta(401, json!({"error": "invalid_client"}));
        }
        let n = e.tokens.fetch_add(1, Ordering::SeqCst);
        let t = format!("az-acesso-{n}");
        *e.validos.lock().unwrap() = vec![t.clone()];
        return resposta(
            200,
            json!({"access_token": t, "expires_in": 3600, "token_type": "Bearer"}),
        );
    }
    let auth = cabecalho(&h, "authorization");
    let token = auth.strip_prefix("Bearer ").unwrap_or("");
    if !e.validos.lock().unwrap().iter().any(|v| v == token) {
        return resposta(
            401,
            json!({"error": {"code": "Unauthorized", "message": auth}}),
        );
    }
    if uri.query() != Some("api-version=7.4") {
        return resposta(400, json!({"error": {"code": "BadParameter"}}));
    }
    e.leituras.fetch_add(1, Ordering::SeqCst);
    match uri.path() {
        "/secrets/banco" => resposta(200, json!({"value": AZ_VALOR, "id": "x/secrets/banco/v9"})),
        "/secrets/banco/v1" => resposta(200, json!({"value": "{\"senha\":\"azure-v1-FFF666\"}"})),
        _ => resposta(
            404,
            json!({"error": {"code": "SecretNotFound", "message": format!("nao achei, {auth}")}}),
        ),
    }
}

/// Azure: client credentials pelo MESMO pedido de token do `oauth.rs`, o acesso em memoria
/// (um token para varias leituras), o 401 que esquece e renova uma vez, versao, campo, e o
/// erro pelo `error.code` -- sem o Bearer que o falso ecoa.
#[tokio::test]
async fn azure_token_em_memoria_renova_no_401_e_le_versao() {
    let e = Arc::new(Azure::default());
    let base = servir(Router::new().fallback(azure_falso).with_state(e.clone())).await;
    let raiz = tmp("azure");
    cofres::AZURE_SEGREDO.guardar(&raiz, AZ_SEGREDO).unwrap();
    let cfg = ConfigCofres {
        azure: Some(CfgAzure {
            url: base.clone(),
            tenant: "tenant-x".into(),
            cliente_id: AZ_CLIENTE.into(),
            token_url: Some(format!("{base}/tenant-x/oauth2/v2.0/token")),
        }),
        cache_segundos: 0,
        ..config(&base)
    };
    let c = cofres::montar(&raiz, &cfg).unwrap();
    let r = refe("azure", "banco", None, None);
    for _ in 0..2 {
        assert_eq!(c.resolver("kv", &r).await.unwrap().0.expose(), AZ_VALOR);
    }
    assert_eq!(
        e.tokens.load(Ordering::SeqCst),
        1,
        "um token para duas leituras"
    );
    // O servico invalida o acesso: 401, um token novo, e a leitura passa.
    e.validos.lock().unwrap().clear();
    assert_eq!(c.resolver("kv", &r).await.unwrap().0.expose(), AZ_VALOR);
    assert_eq!(e.tokens.load(Ordering::SeqCst), 2);
    let v1 = refe("azure", "banco", Some("senha"), Some("v1"));
    assert_eq!(
        c.resolver("kv", &v1).await.unwrap().0.expose(),
        "azure-v1-FFF666"
    );
    let erro = c
        .resolver("kv", &refe("azure", "outro", None, None))
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(
        erro,
        "cofre azure: credencial kv: nao encontrado (404): SecretNotFound"
    );
    assert!(!erro.contains("az-acesso"), "{erro}");
    // Segredo do cliente errado: o erro do token sai sem ele.
    cofres::AZURE_SEGREDO
        .guardar(&raiz, "segredo-AZURE-errado-123")
        .unwrap();
    let c = cofres::montar(&raiz, &cfg).unwrap();
    let erro = c.resolver("kv", &r).await.unwrap_err().to_string();
    assert!(
        erro.contains("invalid_client") && !erro.contains("errado-123"),
        "{erro}"
    );
    assert!(
        !tudo_em(&raiz).contains("az-acesso"),
        "o acesso foi ao disco"
    );
}

// ------------------------------------------------------------------ GCP falso

const GCP_EMAIL: &str = "agente@projeto-x.iam.gserviceaccount.com";
const GCP_VALOR: &str = "valor-GCP-sm-GGG777";

#[derive(Default)]
struct Gcp {
    tokens: AtomicUsize,
    token_url: Mutex<String>,
    recusas: Mutex<Vec<String>>,
}

async fn gcp_falso(
    State(e): State<Arc<Gcp>>,
    metodo: Method,
    uri: Uri,
    h: HeaderMap,
    corpo: Bytes,
) -> Response {
    if metodo == Method::POST && uri.path() == "/token" {
        let f = form(&corpo);
        let jwt = f.get("assertion").cloned().unwrap_or_default();
        let partes: Vec<&str> = jwt.split('.').collect();
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let n = b64.decode(RFC_N).unwrap();
        let ok_ass = partes.len() == 3
            && verificar_pkcs1_sha256_sem_politica(
                &n,
                &[1, 0, 1],
                &jwt.as_bytes()[..partes[0].len() + 1 + partes[1].len()],
                &b64.decode(partes[2]).unwrap_or_default(),
            );
        let claims: Value = partes
            .get(1)
            .and_then(|p| b64.decode(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let cab: Value = partes
            .first()
            .and_then(|p| b64.decode(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let ok = ok_ass
            && f.get("grant_type").map(String::as_str)
                == Some("urn:ietf:params:oauth:grant-type:jwt-bearer")
            && !f.contains_key("client_id")
            && cab["alg"] == "RS256"
            && cab["kid"] == "chave-1"
            && claims["iss"] == GCP_EMAIL
            && claims["aud"] == *e.token_url.lock().unwrap()
            && claims["scope"] == "https://www.googleapis.com/auth/cloud-platform"
            && claims["exp"].as_i64().unwrap_or(0) - claims["iat"].as_i64().unwrap_or(0) == 3600;
        if !ok {
            e.recusas
                .lock()
                .unwrap()
                .push(format!("{cab} {claims} ass={ok_ass}"));
            return resposta(400, json!({"error": "invalid_grant"}));
        }
        let n = e.tokens.fetch_add(1, Ordering::SeqCst);
        return resposta(
            200,
            json!({"access_token": format!("gcp-acesso-{n}"), "expires_in": 3600}),
        );
    }
    if !cabecalho(&h, "authorization").starts_with("Bearer gcp-acesso-") {
        return resposta(401, json!({"error": {"status": "UNAUTHENTICATED"}}));
    }
    let dado = |t: &str| {
        resposta(
            200,
            json!({"name": "x", "payload": {"data": base64::engine::general_purpose::STANDARD.encode(t)}}),
        )
    };
    match uri.path() {
        "/v1/projects/projeto-x/secrets/banco/versions/latest:access" => dado(GCP_VALOR),
        "/v1/projects/projeto-x/secrets/banco/versions/3:access" => {
            dado(r#"{"senha":"gcp-v3-HHH888"}"#)
        }
        "/v1/projects/outro/secrets/banco/versions/latest:access" => dado("do-projeto-outro"),
        _ => resposta(
            404,
            json!({"error": {"status": "NOT_FOUND", "message": "x"}}),
        ),
    }
}

fn conta_gcp() -> String {
    json!({
        "type": "service_account",
        "project_id": "projeto-x",
        "private_key_id": "chave-1",
        "private_key": pem("rfc7515_a2_pkcs8.pem"),
        "client_email": GCP_EMAIL,
        "token_uri": "https://token-trocado.exemplo/token",
    })
    .to_string()
}

/// GCP: a conta de servico assina o JWT RS256 com o RSA da casa, o falso confere a
/// assinatura com a chave publica e as declaracoes (`aud` = o endpoint DECLARADO, nao o
/// `token_uri` trocado do JSON), e o Secret Manager devolve o valor; versao, campo, projeto
/// da configuracao sobre o da conta, e o erro pelo `error.status`.
#[tokio::test]
async fn gcp_conta_de_servico_assina_jwt_e_le_o_segredo() {
    let e = Arc::new(Gcp::default());
    let base = servir(Router::new().fallback(gcp_falso).with_state(e.clone())).await;
    *e.token_url.lock().unwrap() = format!("{base}/token");
    let raiz = tmp("gcp");
    cofres::GCP_CONTA.guardar(&raiz, &conta_gcp()).unwrap();
    let mut cfg = ConfigCofres {
        gcp: Some(CfgGcp {
            projeto: None,
            url: Some(base.clone()),
            token_url: Some(format!("{base}/token")),
        }),
        cache_segundos: 0,
        ..config(&base)
    };
    let c = cofres::montar(&raiz, &cfg).unwrap();
    let r = refe("gcp", "banco", None, None);
    assert_eq!(
        c.resolver("sm", &r).await.unwrap().0.expose(),
        GCP_VALOR,
        "recusas do falso: {:?}",
        e.recusas.lock().unwrap()
    );
    c.resolver("sm", &r).await.unwrap();
    assert_eq!(
        e.tokens.load(Ordering::SeqCst),
        1,
        "um token para duas leituras"
    );
    let v3 = refe("gcp", "banco", Some("senha"), Some("3"));
    assert_eq!(
        c.resolver("sm", &v3).await.unwrap().0.expose(),
        "gcp-v3-HHH888"
    );
    let erro = c
        .resolver("sm", &refe("gcp", "nada", None, None))
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(
        erro,
        "cofre gcp: credencial sm: nao encontrado (404): NOT_FOUND"
    );
    cfg.gcp.as_mut().unwrap().projeto = Some("outro".into());
    let c = cofres::montar(&raiz, &cfg).unwrap();
    assert_eq!(
        c.resolver("sm", &r).await.unwrap().0.expose(),
        "do-projeto-outro"
    );
    // Conta que nao e de servico: erro sem o conteudo.
    cofres::GCP_CONTA
        .guardar(
            &raiz,
            r#"{"type":"authorized_user","refresh_token":"rt-SEGREDO-1"}"#,
        )
        .unwrap();
    let erro = c.resolver("sm", &r).await.unwrap_err().to_string();
    assert!(
        erro.contains("service_account") && !erro.contains("rt-SEGREDO"),
        "{erro}"
    );
    assert!(
        !tudo_em(&raiz).contains("gcp-acesso"),
        "o acesso foi ao disco"
    );
}

// ------------------------------------------------------------------ no HTTP, de ponta a ponta

/// O no HTTP pede `"credencial": "db"` e nao sabe que ela mora no Vault: o broker de
/// `credenciais/` resolve pelo cofre (configurado pelas chaves `cofres.*` do `config.json`),
/// o valor vai no `Authorization`, a resposta que o ecoa sai tarjada, o erro que o ecoa
/// tambem, e nada -- evidencia, envelope, config -- guarda o valor em disco.
///
/// E o UNICO teste deste arquivo que fixa a configuracao do processo (`config::iniciar`).
///
/// RED medido em 09/10: com o valor posto no resumo da evidencia do `resolver_externo`
/// (`// REPOSTO` no `phxclaw-secret-broker`), o teste caiu em «senha-VAULT-v2-… em disco».
#[tokio::test]
async fn no_http_resolve_credencial_no_cofre_sem_saber_e_sem_vazar() {
    let (vault, ev) = subir_vault().await;
    let eco = servir(Router::new().fallback(|h: HeaderMap, uri: Uri| async move {
        let a = cabecalho(&h, "authorization");
        if uri.path() == "/falha" {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("falhou; recebi {a}"),
            )
                .into_response()
        } else {
            resposta(200, json!({"recebi": a}))
        }
    }))
    .await;
    let raiz = tmp("e2e");
    cofres::VAULT_TOKEN.guardar(&raiz, VAULT_TOKEN).unwrap();
    std::fs::write(
        raiz.join("config.json"),
        json!({"cofres": {"vault": {"url": vault, "namespace": "time-a"}, "liberar": [vault]}})
            .to_string(),
    )
    .unwrap();
    phxclaw_agent::config::iniciar(&raiz).unwrap();
    std::fs::write(
        raiz.join(fluxo_http::ARQUIVO),
        json!({
            "liberar": [eco],
            "credenciais": {"db": {
                "tipo": "bearer",
                "origens": [eco],
                "cofre": {"cofre": "vault", "caminho": "app/db", "campo": "senha"}
            }}
        })
        .to_string(),
    )
    .unwrap();
    let itens = fluxo_http::executar(
        &raiz,
        &json!({"url": format!("{eco}/ok"), "credencial": "db"}),
    )
    .await
    .unwrap();
    let texto = serde_json::to_string(&itens).unwrap();
    assert!(
        texto.contains("[REDACTED]") && !texto.contains(VAULT_V2),
        "{texto}"
    );
    assert_eq!(ev.leituras.load(Ordering::SeqCst), 1, "o Vault foi lido");
    let erro = fluxo_http::executar(
        &raiz,
        &json!({"url": format!("{eco}/falha"), "credencial": "db"}),
    )
    .await
    .unwrap_err();
    assert!(
        erro.contains("HTTP 500") && !erro.contains(VAULT_V2),
        "{erro}"
    );
    assert_eq!(
        ev.leituras.load(Ordering::SeqCst),
        1,
        "a segunda veio do cache curto"
    );
    // A credencial nao vai a origem que nao e dela.
    let erro = fluxo_http::executar(
        &raiz,
        &json!({"url": "https://outro.exemplo/x", "credencial": "db"}),
    )
    .await
    .unwrap_err();
    assert!(erro.contains("nao vale para"), "{erro}");
    // A evidencia registrou as leituras, sem valor; nada em disco tem o valor nem o token.
    let disco = tudo_em(&raiz);
    assert!(
        disco.contains("resolved_external"),
        "a evidencia nao registrou"
    );
    for vazou in [VAULT_V2, VAULT_TOKEN] {
        assert!(!disco.contains(vazou), "{vazou} em disco");
    }
    // Credencial oauth2 com cofre e recusada na declaracao.
    std::fs::write(
        raiz.join(fluxo_http::ARQUIVO),
        json!({"credenciais": {"x": {
            "tipo": "oauth2", "origens": [eco],
            "oauth2": {"token": format!("{eco}/t"), "cliente_id": "c", "concessao": "cliente"},
            "cofre": {"cofre": "vault", "caminho": "a"}
        }}})
        .to_string(),
    )
    .unwrap();
    let erro = fluxo_http::ConfigHttp::carregar(&raiz).unwrap_err();
    assert!(erro.contains("no oauth2"), "{erro}");
}

/// A configuracao dos cofres e so do operador: o `.phxclaw/config.json` de um projeto
/// confiado que aponta `cofres.vault.url` (e o `cofres.liberar`) para um servidor dele e
/// IGNORADO com aviso; a pasta do operador continua podendo.
///
/// RED medido em 09/10: sem as linhas `cofres.*` em `SO_DO_OPERADOR` (`// REPOSTO`), a URL do
/// projeto valeu e o teste caiu na primeira asserção.
#[test]
fn cofre_declarado_pelo_projeto_e_ignorado() {
    use phxclaw_config_runtime::agente::carga;
    let d = tmp("projeto");
    let pasta = d.join("pasta.json");
    let projeto = d.join("projeto.json");
    std::fs::write(&pasta, r#"{"cofres": {"aws": {"regiao": "us-east-1"}}}"#).unwrap();
    std::fs::write(
        &projeto,
        r#"{"cofres": {"vault": {"url": "https://vault.atacante.exemplo"}, "liberar": ["http://10.0.0.1"], "aws": {"url": "https://aws.atacante.exemplo"}}}"#,
    )
    .unwrap();
    let nada = |_: &str| None;
    let c = carga::carregar(&nada, &pasta, Some(&projeto)).unwrap();
    assert_eq!(c.texto("cofres.vault.url"), None);
    assert_eq!(c.texto("cofres.aws.url"), None);
    assert_eq!(
        c.lista("cofres.liberar").unwrap_or_default(),
        Vec::<String>::new()
    );
    assert_eq!(c.texto("cofres.aws.regiao").as_deref(), Some("us-east-1"));
    let ignoradas: Vec<&str> = c
        .ignoradas_do_projeto
        .iter()
        .map(|e| e.chave.as_str())
        .collect();
    for k in ["cofres.vault.url", "cofres.liberar", "cofres.aws.url"] {
        assert!(
            ignoradas.contains(&k),
            "{k} nao foi ignorada: {ignoradas:?}"
        );
    }
    for e in &c.ignoradas_do_projeto {
        assert!(!e.motivo.contains("atacante"), "o aviso ecoou o valor: {e}");
    }
    // Toda chave cofres.* de arquivo e so do operador.
    for k in phxclaw_config_runtime::agente::catalogo()
        .iter()
        .filter(|k| k.chave.starts_with("cofres."))
    {
        assert!(
            k.segredo() || k.so_do_operador().is_some(),
            "{} vale do projeto",
            k.chave
        );
    }
}

/// O broker sem cofres ligados diz isso, e o tipo que nao esta configurado tambem.
#[tokio::test]
async fn cofre_nao_configurado_diz_qual_e_quais_existem() {
    let raiz = tmp("sem");
    let c = cofres::montar(&raiz, &ConfigCofres::default()).unwrap();
    let erro = c
        .resolver("db", &refe("vault", "a", None, None))
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(
        erro,
        "cofre vault: credencial db: cofre nao configurado (configurados: nenhum)"
    );
}
