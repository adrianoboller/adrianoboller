//! AWS Signature Version 4 (o `AWS4-HMAC-SHA256`), escrita aqui sobre o HMAC-SHA256 da casa
//! (`canais::cripto`, conferido contra a RFC 4231) para assinar o `GetSecretValue` do AWS
//! Secrets Manager (`cofres/aws.rs`).
//!
//! Conferida byte a byte -- requisicao canonica, texto a assinar e `Authorization` -- contra
//! a suite oficial `aws-sig-v4-test-suite` (31 casos, `tests/dados/aws4`).
//!
//! Onde diverge de um SDK, e por que:
//! - **o caminho entra CRU e se codifica uma vez**, como na suite: quem chama passa o caminho
//!   decodificado. O Secrets Manager so usa `/`, e a regra «codificar duas vezes» dos SDKs
//!   existe para caminho que ja chega codificado na linha do pedido -- aqui nao chega;
//! - **todo cabecalho passado e assinado**: quem chama escolhe o que assina. Nao ha lista de
//!   cabecalhos a pular (o `user-agent` dos SDKs), porque o cliente HTTP da casa poe o dele
//!   depois e ele nao entra na lista que se passa aqui;
//! - **sem pre-assinatura por query** (`X-Amz-Signature` na URL): a credencial nunca vai em
//!   URL, que acaba em log.

use crate::canais::cripto::{hex, hmac_sha256, sha256};

pub const ALGORITMO: &str = "AWS4-HMAC-SHA256";

/// Um pedido como se assina: metodo, caminho cru, pares de query crus (decodificados),
/// cabecalhos na ordem (valor cru; o de varias linhas ja unido) e o corpo.
pub struct Pedido<'a> {
    pub metodo: &'a str,
    pub caminho: &'a str,
    pub query: &'a [(String, String)],
    pub cabecalhos: &'a [(String, String)],
    pub corpo: &'a [u8],
}

/// URI-encode da SigV4: so os nao reservados da RFC 3986 (`A-Z a-z 0-9 - . _ ~`) ficam; o
/// resto vira `%XX` maiusculo, byte a byte do UTF-8.
pub fn codificar(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// O caminho canonico: segmentos vazios e `.` saem, `..` sobe um, cada segmento se codifica,
/// e a barra final do original fica (fora a raiz).
pub fn caminho_canonico(caminho: &str) -> String {
    let mut pilha: Vec<&str> = Vec::new();
    for seg in caminho.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                pilha.pop();
            }
            s => pilha.push(s),
        }
    }
    if pilha.is_empty() {
        return "/".into();
    }
    let mut c: String = pilha.iter().map(|s| format!("/{}", codificar(s))).collect();
    if caminho.ends_with('/') {
        c.push('/');
    }
    c
}

/// A query canonica: chave e valor codificados, ordenados pela chave e depois pelo valor.
pub fn query_canonica(query: &[(String, String)]) -> String {
    let mut pares: Vec<(String, String)> = query
        .iter()
        .map(|(k, v)| (codificar(k), codificar(v)))
        .collect();
    pares.sort();
    pares
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Os cabecalhos canonicos (`nome:valor\n` ordenados) e a lista assinada (`a;b;c`). Nome em
/// minusculas; valor sem espaco nas pontas e com cada sequencia de brancos virando UM
/// espaco; nome repetido junta os valores por virgula, na ordem em que vieram.
pub fn cabecalhos_canonicos(cabecalhos: &[(String, String)]) -> (String, String) {
    let mut por_nome: Vec<(String, Vec<String>)> = Vec::new();
    for (k, v) in cabecalhos {
        let k = k.trim().to_ascii_lowercase();
        let v = v.split_whitespace().collect::<Vec<_>>().join(" ");
        match por_nome.iter_mut().find(|(n, _)| *n == k) {
            Some((_, vs)) => vs.push(v),
            None => por_nome.push((k, vec![v])),
        }
    }
    por_nome.sort_by(|a, b| a.0.cmp(&b.0));
    let bloco: String = por_nome
        .iter()
        .map(|(k, vs)| format!("{k}:{}\n", vs.join(",")))
        .collect();
    let assinados = por_nome
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");
    (bloco, assinados)
}

/// A requisicao canonica e a lista de cabecalhos assinados.
pub fn requisicao_canonica(p: &Pedido) -> (String, String) {
    let (bloco, assinados) = cabecalhos_canonicos(p.cabecalhos);
    (
        format!(
            "{}\n{}\n{}\n{bloco}\n{assinados}\n{}",
            p.metodo.to_ascii_uppercase(),
            caminho_canonico(p.caminho),
            query_canonica(p.query),
            hex(&sha256(p.corpo))
        ),
        assinados,
    )
}

/// `AAAAMMDD/regiao/servico/aws4_request`.
pub fn escopo(data_hora: &str, regiao: &str, servico: &str) -> String {
    format!("{}/{regiao}/{servico}/aws4_request", &data_hora[..8])
}

pub fn texto_a_assinar(data_hora: &str, escopo: &str, canonica: &str) -> String {
    format!(
        "{ALGORITMO}\n{data_hora}\n{escopo}\n{}",
        hex(&sha256(canonica.as_bytes()))
    )
}

/// A chave derivada: HMAC encadeado de data, regiao, servico e `aws4_request`.
pub fn chave_de_assinatura(segredo: &str, data: &str, regiao: &str, servico: &str) -> Vec<u8> {
    let k = hmac_sha256(format!("AWS4{segredo}").as_bytes(), data.as_bytes());
    let k = hmac_sha256(&k, regiao.as_bytes());
    let k = hmac_sha256(&k, servico.as_bytes());
    hmac_sha256(&k, b"aws4_request")
}

/// O valor do `Authorization`. `data_hora` e o `X-Amz-Date` (`AAAAMMDDTHHMMSSZ`), que tem de
/// estar entre os cabecalhos do pedido -- e o que a AWS confere.
pub fn autorizacao(
    p: &Pedido,
    chave_id: &str,
    segredo: &str,
    regiao: &str,
    servico: &str,
    data_hora: &str,
) -> Result<String, String> {
    if data_hora.len() != 16 || !data_hora.is_ascii() {
        return Err("X-Amz-Date fora da forma AAAAMMDDTHHMMSSZ".into());
    }
    let (canonica, assinados) = requisicao_canonica(p);
    let esc = escopo(data_hora, regiao, servico);
    let sts = texto_a_assinar(data_hora, &esc, &canonica);
    let chave = chave_de_assinatura(segredo, &data_hora[..8], regiao, servico);
    let assinatura = hex(&hmac_sha256(&chave, sts.as_bytes()));
    Ok(format!(
        "{ALGORITMO} Credential={chave_id}/{esc}, SignedHeaders={assinados}, Signature={assinatura}"
    ))
}
