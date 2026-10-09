//! O nome que tem cara de segredo: a lista UNICA de toda a base.
//!
//! Mora no crate mais baixo porque havia dois motores com duas listas -- o
//! `gravacao::chave_secreta` do agente (que redige gravacao e recusa variavel, pin, campo de
//! formulario e entrada de fluxo) e o `carga::nome_de_segredo` do config-runtime (que recusa
//! segredo no `config.json`) -- e elas divergiam: `openai_key`, `access_key` e `key` eram
//! segredo para um e nao para o outro (medido em 09/10). Duas listas divergem no dia em que
//! alguem acrescenta um nome numa so; a uniao das duas e o lado que nao vaza.

/// Os nomes cujo valor inteiro e segredo. Compara-se o ultimo segmento da chave pontuada
/// (`canais.telegram.bot_token` -> `bot_token`), inteiro ou pelo sufixo depois de `_` (o `-`
/// vale como `_`), nunca substring: `max_tokens` e `input_tokens` sao contagem, nao
/// credencial; `bot_token` e `openai_key` sao.
pub const NOMES_DE_SEGREDO: &[&str] = &[
    "password",
    "passwd",
    "senha",
    "secret",
    "segredo",
    "token",
    "key",
    "api_key",
    "apikey",
    "authorization",
    "cookie",
    "set_cookie",
    "private_key",
    "chave_privada",
    "client_secret",
    "credential",
    "credencial",
    "bearer",
];

/// O nome tem cara de segredo?
pub fn nome_de_segredo(chave: &str) -> bool {
    let ultimo = chave
        .rsplit('.')
        .next()
        .unwrap_or(chave)
        .to_ascii_lowercase()
        .replace('-', "_");
    NOMES_DE_SEGREDO
        .iter()
        .any(|s| ultimo == *s || ultimo.ends_with(&format!("_{s}")))
}

/// Os prefixos de chave de provedor: a lista UNICA. Havia uma no broker (11, que redige texto)
/// e outra no `config.json` (20, que recusa valor), e a entrada externa do motor de fluxo usou
/// a segunda sem a regra do corpo: `sk-SK` (locale) virava credencial. Medido em 09/10.
pub const PREFIXOS_DE_CREDENCIAL: &[&str] = &[
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxs-",
    "xapp-",
    "sk-",
    "sk_live_",
    "rk_live_",
    "AIza",
    "AKIA",
    "ya29.",
    "xai-",
    "-----BEGIN",
];

/// A PALAVRA tem forma de credencial? Prefixo de provedor seguido de corpo (pelo menos 12
/// caracteres de token depois dele: «sk-» numa frase, `sk-SK` ou `sk-telecom` nao sao chave),
/// ou JWT de tres partes. Olha-se o pedaco depois do ultimo `:` ou `=`, para pegar
/// `token:ghp_…` e `x-access-token:ghs_…`. Sem entropia: um SHA de commit tem 40 hex de alta
/// entropia e nao e segredo.
pub fn palavra_parece_credencial(palavra: &str) -> bool {
    let nucleo = palavra.rsplit([':', '=']).next().unwrap_or(palavra);
    let nucleo = nucleo.trim_end_matches(['.', '!', '?']);
    let corpo_ok = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    let com_prefixo = PREFIXOS_DE_CREDENCIAL
        .iter()
        .any(|p| nucleo.starts_with(p) && nucleo.len() >= p.len() + 12 && corpo_ok(nucleo));
    let jwt = nucleo.starts_with("eyJ") && nucleo.split('.').count() == 3 && nucleo.len() >= 30;
    com_prefixo || jwt
}

/// O TEXTO traz credencial pela FORMA, sem olhar nome de campo nem entropia: bloco PEM,
/// URL com senha (`esquema://usuario:senha@host`), cabecalho `Basic`/`Bearer` com valor, ou
/// alguma palavra com forma de credencial. E o criterio para dado de TERCEIROS (corpo de
/// webhook), em que `key` e a chave do Jira e `next_page_token` e paginacao.
pub fn texto_tem_credencial(texto: &str) -> bool {
    if texto.contains("-----BEGIN ") {
        return true;
    }
    if url_com_senha(texto) {
        return true;
    }
    if !valores_basic_bearer(texto).is_empty() {
        return true;
    }
    texto.split(e_separador).any(palavra_parece_credencial)
}

/// O que separa uma palavra da outra quando se procura credencial: UM so, usado aqui e na
/// tarja do broker (`scrub_secret_like`). Eram dois, e o do broker nao cortava em `[ ] { } & /
/// @`: `[ghp_…]`, `x=ghp_…&y=1` e `https://x-access-token:ghp_…@github.com` eram recusados na
/// entrada do fluxo e passavam em claro na gravacao (medido em 09/10). `:` e `=` NAO separam:
/// a palavra corta no ultimo deles dentro de `palavra_parece_credencial`.
pub fn e_separador(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\''
                | ','
                | ';'
                | '('
                | ')'
                | '<'
                | '>'
                | '`'
                | '['
                | ']'
                | '{'
                | '}'
                | '&'
                | '/'
                | '@'
                | '\\'
        )
}

/// As faixas (em bytes) dos VALORES de `Basic` (8+ caracteres) e `Bearer` (16+), sem
/// diferenciar maiuscula: `Authorization: Bearer …`, `authorization: bearer …` de um `curl -H`
/// ou so `Bearer …` num JSON. Uma conta so, para recusar (`texto_tem_credencial`) e para
/// tarjar (`tarjar_basic_bearer`, usada pelo broker): eram duas regras, e quatro corpos que a
/// entrada do fluxo recusava passavam em claro na tarja (medido em 09/10).
fn valores_basic_bearer(texto: &str) -> Vec<std::ops::Range<usize>> {
    // O valor se corta pelo ALFABETO de cada um, nao por um delimitador: caractere colado no
    // fim (crase de markdown, `]`, `>`, `}`, `&`, `\"` de JSON escapado, ponto final) nao pode
    // invalidar o valor inteiro -- medido em 09/10, cortar pelo delimitador deixava 59 de 136
    // credenciais reais passarem em claro nesses contextos.
    let base64 = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=');
    let token =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~' | '+' | '/' | '=');
    let minusculo = texto.to_ascii_lowercase();
    let mut faixas = Vec::new();
    for marca in ["basic", "bearer"] {
        let mut de = 0usize;
        while let Some(i) = minusculo[de..].find(marca) {
            let fim_marca = de + i + marca.len();
            de = fim_marca;
            // Depois da marca, pelo menos um espaco (ou tab), e quantos houver: «basically»
            // nao e marca; «Basic\t…» e «Bearer  …» sao.
            let brancos: usize = texto[fim_marca..]
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .map(char::len_utf8)
                .sum();
            if brancos == 0 {
                continue;
            }
            let ini = fim_marca + brancos;
            let alfabeto = if marca == "basic" { base64 } else { token };
            let tam: usize = texto[ini..]
                .chars()
                .take_while(|c| alfabeto(*c))
                .map(char::len_utf8)
                .sum();
            let valor = &texto[ini..ini + tam];
            let vale = if marca == "basic" {
                valor_basic(valor)
            } else {
                valor_bearer(valor)
            };
            if vale {
                faixas.push(ini..ini + tam);
            }
        }
    }
    faixas.sort_by_key(|f| f.start);
    faixas
}

/// O valor de `Basic` e SEMPRE base64 de `usuario:senha`: base64 valido (alfabeto, tamanho
/// multiplo de 4) que decodifica para texto com `:`. «basic validation» e «basic
/// authentication» sao ingles comum (medido em 09/10: a regra «8+ caracteres» tarjava a
/// memoria «basic validation» para sempre).
fn valor_basic(v: &str) -> bool {
    v.len() >= 8
        && v.len().is_multiple_of(4)
        && base64_decodifica(v).is_some_and(|b| b.contains(&b':'))
}

/// O valor de `Bearer` e um token: caracteres de token (RFC 6750 b64token) e 16+, com digito
/// ou com 20+ caracteres. «the bearer of this certificate» e «bearer responsibilities» (16
/// letras, sem digito) sao prosa; um token opaco real quase sempre tem digito, e palavra de
/// 20+ letras quase nao existe.
fn valor_bearer(v: &str) -> bool {
    let token = v
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~' | '+' | '/' | '='));
    token && v.len() >= 16 && (v.chars().any(|c| c.is_ascii_digit()) || v.len() >= 20)
}

/// Base64 padrao (com `=`), so para decidir se o valor do `Basic` e o que diz ser. Escrito
/// aqui: o crate de tipos nao tem dependencia de base64.
fn base64_decodifica(v: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let b = v.as_bytes();
    if !b.len().is_multiple_of(4) {
        return None;
    }
    let mut saida = Vec::with_capacity(b.len() / 4 * 3);
    for (n, bloco) in b.chunks(4).enumerate() {
        let ultimo = n + 1 == b.len() / 4;
        let pad = bloco.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !ultimo) {
            return None;
        }
        let mut acc = 0u32;
        for &c in &bloco[..4 - pad] {
            acc = (acc << 6) | val(c)?;
        }
        acc <<= 6 * pad as u32;
        let bytes = [(acc >> 16) as u8, (acc >> 8) as u8, acc as u8];
        saida.extend_from_slice(&bytes[..3 - pad]);
    }
    Some(saida)
}

/// O texto com o valor de todo `Basic`/`Bearer` trocado por `tarja`, pela mesma conta de
/// `texto_tem_credencial`.
pub fn tarjar_basic_bearer(texto: &str, tarja: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut ultimo = 0usize;
    for f in valores_basic_bearer(texto) {
        if f.start < ultimo {
            continue;
        }
        saida.push_str(&texto[ultimo..f.start]);
        saida.push_str(tarja);
        ultimo = f.end;
    }
    saida.push_str(&texto[ultimo..]);
    saida
}

/// O texto com a senha de toda URL `esquema://usuario:senha@host` trocada por `tarja` (so na
/// autoridade), pela mesma conta de `url_com_senha`.
pub fn tarjar_url_com_senha(texto: &str, tarja: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(faixa) = senha_de_url(resto) {
        saida.push_str(&resto[..faixa.start]);
        saida.push_str(tarja);
        resto = &resto[faixa.end..];
    }
    saida.push_str(resto);
    saida
}

/// A faixa (em bytes) da primeira senha de URL do texto, se houver.
fn senha_de_url(texto: &str) -> Option<std::ops::Range<usize>> {
    let mut base = 0usize;
    while let Some(i) = texto[base..].find("://") {
        let ini_aut = base + i + 3;
        let depois = &texto[ini_aut..];
        let fim = depois
            .find(|c: char| matches!(c, '/' | '?' | '#') || c.is_whitespace() || c == '"')
            .unwrap_or(depois.len());
        let autoridade = &depois[..fim];
        if let Some(arroba) = autoridade.rfind('@')
            && let Some(dois) = autoridade[..arroba].find(':')
            && dois + 1 < arroba
        {
            return Some(ini_aut + dois + 1..ini_aut + arroba);
        }
        base = ini_aut;
    }
    None
}

/// `esquema://usuario:senha@host` com senha nao vazia, olhando so a AUTORIDADE (ate a
/// primeira `/`, `?` ou `#`): `http://h/x?u=eu:pw@y` e `https://h/perfil/@ana` nao contam.
fn url_com_senha(texto: &str) -> bool {
    senha_de_url(texto).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Os tres que dividiam os dois motores em 09/10, e os que nao sao segredo.
    #[test]
    fn a_uniao_das_duas_listas_e_contagem_nao_e_segredo() {
        for s in [
            "openai_key",
            "access_key",
            "key",
            "x.api_key",
            "bot-token",
            "canais.telegram.bot_token",
            "password",
            "Client_Secret",
        ] {
            assert!(nome_de_segredo(s), "{s}");
        }
        for n in [
            "max_tokens",
            "input_tokens",
            "chave",
            "monkey",
            "keys",
            "nome",
        ] {
            assert!(!nome_de_segredo(n), "{n}");
        }
    }

    /// A forma, sem nome e sem entropia. Os 400 errados e os 202 errados medidos em 09/10.
    #[test]
    fn credencial_pela_forma_e_so_pela_forma() {
        let ghp = "ghp_0123456789abcdefABCDEF";
        for sim in [
            format!("{{\"nota\":\"{ghp}\"}}"),
            format!("token:{ghp}"),
            format!("[{ghp}]"),
            format!("{{{ghp}}}"),
            format!("https://x-access-token:{ghp}@github.com/a/b"),
            "Bearer sk-0123456789abcdef".into(),
            "Authorization: Basic dXNlcjpwYXNzd29yZA==".into(),
            "https://eu:senha@busca.local:443/x".into(),
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.c2lnbmF0dXJh".into(),
            "-----BEGIN PRIVATE KEY-----\nMII".into(),
            // os contextos em que o valor vem colado a pontuacao (integrador, 09/10)
            "use `Bearer a1b2c3d4e5f6g7h8i9j0` no header".into(),
            "<Bearer a1b2c3d4e5f6g7h8i9j0>".into(),
            "[Bearer a1b2c3d4e5f6g7h8i9j0]".into(),
            "{Bearer a1b2c3d4e5f6g7h8i9j0}".into(),
            "headers=Bearer a1b2c3d4e5f6g7h8i9j0&x=1".into(),
            "{\\\"h\\\":\\\"Bearer a1b2c3d4e5f6g7h8i9j0\\\"}".into(),
            "(Authorization: Basic dXNlcjpwYXNzd29yZA==)".into(),
            "Basic dXNlcjpwYXNzd29yZA==.".into(),
            "Basic\tdXNlcjpwYXNzd29yZA==".into(),
            "Bearer   a1b2c3d4e5f6g7h8i9j0".into(),
            // token68 com `-`, `_`, `.` e `~`: trava o alfabeto do Bearer (cortado pelo base64,
            // sobra `7f3k` e o resto passa).
            "Bearer 7f3k-9QxZ_2mN8.pL4v~R6tY".into(),
        ] {
            assert!(texto_tem_credencial(&sim), "{sim}");
        }
        for nao in [
            "sk-SK",
            "sk-telecom",
            "PROJ-123",
            "fotos/a.jpg",
            "https://h/x?next_page_token=abc123",
            "9fceb02d0ae598e95dc970b74767f19372d61af8",
            "http://localhost:8888/x?u=eu:pw@y",
            "https://busca.local:443/perfil/@ana?q=a:b",
            "o portador (bearer) do titulo",
            "Add basic validation to the form",
            "basic authentication is deprecated",
            "Implement basic functionality",
            "a basic requirement",
            "Visual Basic is old",
            "the bearer of this certificate",
            "bearer responsibilities",
            "Basic abcdefgh",
        ] {
            assert!(!texto_tem_credencial(nao), "{nao}");
        }
    }
}
