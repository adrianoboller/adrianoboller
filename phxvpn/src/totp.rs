//! TOTP (RFC 6238) e HOTP (RFC 4226): o codigo de 6 digitos do aplicativo
//! autenticador, para o segundo fator do painel e da conexao OpenVPN.
//!
//! # Por que HMAC-SHA1, 6 digitos e 30 s
//!
//! Porque e o que os aplicativos calculam de fato. O Google Authenticator e
//! parentes ignoram `algorithm=` e `digits=` do URI; um segredo cadastrado com
//! SHA-256 ou 8 digitos gera, no telefone, codigos que nunca batem. O HMAC nao
//! depende da resistencia a colisao, que e o que caiu no SHA-1.
//!
//! # A janela e o reuso
//!
//! Aceita o passo atual e um de cada lado (relogio do telefone adiantado ou
//! atrasado ate 30 s), e so um passo MAIOR que o ultimo aceito (RFC 6238
//! §5.2: o verificador nao aceita a segunda apresentacao do mesmo codigo).
//! Quem ve o codigo por cima do ombro nao o reusa dentro dos 90 s.
//!
//! Conferido contra os vetores do apendice D da RFC 4226 e do apendice B da
//! RFC 6238 (a coluna SHA-1).

use phxsql_core::hash::iguais_em_tempo_constante;
use phxsql_core::sha1::hmac_sha1;

/// Duracao de um passo, em segundos.
pub const PERIODO: u64 = 30;
/// Digitos do codigo.
pub const DIGITOS: u32 = 6;
/// Passos aceitos de cada lado do atual.
pub const JANELA: u64 = 1;
/// Bytes do segredo: 160 bits, o tamanho do HMAC-SHA1 (RFC 4226 §4 R6).
pub const SEGREDO_LEN: usize = 20;

/// HOTP (RFC 4226 §5.3): HMAC, truncamento dinamico e os `digitos` finais.
pub fn hotp(segredo: &[u8], contador: u64, digitos: u32) -> u32 {
    let h = hmac_sha1(segredo, &contador.to_be_bytes());
    let o = (h[19] & 0x0f) as usize;
    let binario = u32::from_be_bytes([h[o] & 0x7f, h[o + 1], h[o + 2], h[o + 3]]);
    binario % 10u32.pow(digitos)
}

/// TOTP (RFC 6238 §4): o HOTP do passo `t / PERIODO`.
pub fn totp(segredo: &[u8], unix: u64, digitos: u32) -> u32 {
    hotp(segredo, unix / PERIODO, digitos)
}

/// O codigo como o aplicativo mostra: com os zeros a esquerda.
pub fn formatar(codigo: u32, digitos: u32) -> String {
    format!("{codigo:0width$}", width = digitos as usize)
}

/// Confere `codigo` no instante `unix`. Devolve o passo que bateu, se ele e
/// maior que `ultimo` (o ultimo passo ja aceito); quem chama grava o passo.
///
/// Compara os tres candidatos sempre, em tempo constante: o tempo nao diz
/// qual passo bateu nem se algum bateu.
pub fn conferir(segredo: &[u8], codigo: &str, unix: u64, ultimo: u64) -> Option<u64> {
    let codigo = codigo.trim();
    if codigo.len() != DIGITOS as usize || !codigo.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let atual = unix / PERIODO;
    let mut achado = None;
    for passo in atual.saturating_sub(JANELA)..=atual + JANELA {
        let esperado = formatar(hotp(segredo, passo, DIGITOS), DIGITOS);
        if iguais_em_tempo_constante(esperado.as_bytes(), codigo.as_bytes())
            && passo > ultimo
            && achado.is_none()
        {
            achado = Some(passo);
        }
    }
    achado
}

pub fn agora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------- base32 ----

const ALFABETO: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Base32 (RFC 4648 §6), com o preenchimento `=`.
pub fn base32(dados: &[u8]) -> String {
    let mut s = String::new();
    for bloco in dados.chunks(5) {
        let mut b = [0u8; 5];
        b[..bloco.len()].copy_from_slice(bloco);
        let v = u64::from_be_bytes([0, 0, 0, b[0], b[1], b[2], b[3], b[4]]);
        let uteis = (bloco.len() * 8).div_ceil(5);
        for i in 0..8 {
            if i < uteis {
                s.push(ALFABETO[((v >> (35 - 5 * i)) & 31) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// Base32 sem o `=`, como o URI `otpauth://` leva (e como se digita).
pub fn base32_sem_preenchimento(dados: &[u8]) -> String {
    base32(dados).trim_end_matches('=').to_string()
}

/// Le base32 tolerando minusculas, espacos e a falta do `=` -- e como a
/// pessoa digita o segredo quando nao da para ler o QR.
pub fn de_base32(texto: &str) -> Option<Vec<u8>> {
    let mut bits: u64 = 0;
    let mut n = 0u32;
    let mut saida = Vec::new();
    for c in texto.bytes() {
        if c == b'=' || c == b' ' || c == b'-' {
            continue;
        }
        let v = ALFABETO.iter().position(|&a| a == c.to_ascii_uppercase())? as u64;
        bits = (bits << 5) | v;
        n += 5;
        if n >= 8 {
            n -= 8;
            saida.push((bits >> n) as u8);
            bits &= (1 << n) - 1;
        }
    }
    Some(saida)
}

// ------------------------------------------------------ URI e QR Code ----

/// Percent-encoding do que nao e nao-reservado (RFC 3986 §2.3).
fn escapar(t: &str) -> String {
    t.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// O URI que o aplicativo le do QR (formato «Key Uri» do Google
/// Authenticator): emissor no rotulo E no parametro, que e o que os
/// aplicativos mais velhos e os mais novos entendem.
pub fn uri(emissor: &str, conta: &str, segredo: &[u8]) -> String {
    format!(
        "otpauth://totp/{e}:{c}?secret={s}&issuer={e}&algorithm=SHA1&digits={DIGITOS}&period={PERIODO}",
        e = escapar(emissor),
        c = escapar(conta),
        s = base32_sem_preenchimento(segredo),
    )
}

/// O QR do URI em SVG (um `path` so, sem fonte nem imagem externa), com a
/// zona de silencio de 4 modulos que a norma pede.
pub fn qr_svg(texto: &str) -> Result<String, String> {
    use phxsql_core::qr::{gerar, Nivel};
    let q = gerar(texto.as_bytes(), Nivel::M).map_err(|e| e.to_string())?;
    let (n, borda) = (q.dim(), 4);
    let lado = n + 2 * borda;
    let mut d = String::new();
    for r in 0..n {
        for c in 0..n {
            if q.escuro(r, c) {
                d.push_str(&format!("M{} {}h1v1h-1z", c + borda, r + borda));
            }
        }
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {lado} {lado}\" \
         shape-rendering=\"crispEdges\"><rect width=\"{lado}\" height=\"{lado}\" fill=\"#fff\"/>\
         <path d=\"{d}\" fill=\"#000\"/></svg>"
    ))
}

// --------------------------------------------------- static-challenge ----

/// A senha que o cliente OpenVPN manda quando o perfil tem
/// `static-challenge`: `SCRV1:<base64 senha>:<base64 resposta>`. Devolve
/// (senha, resposta). Senha sem esse formato volta com resposta vazia --
/// perfil sem o desafio, e quem exige o codigo recusa.
pub fn scrv1(senha: &str) -> (String, String) {
    let de = |t: &str| phxsql_core::base64::decodificar_texto(t).ok();
    if let Some(resto) = senha.strip_prefix("SCRV1:") {
        if let Some((s, r)) = resto.split_once(':') {
            if let (Some(s), Some(r)) = (de(s), de(r)) {
                return (s, r);
            }
        }
    }
    (senha.to_string(), String::new())
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O segredo dos dois apendices: "12345678901234567890" em ASCII.
    const SEGREDO: &[u8] = b"12345678901234567890";

    /// RFC 4226, apendice D: os dez primeiros HOTP de 6 digitos.
    #[test]
    fn vetores_hotp_da_rfc_4226() {
        let esperados = [
            755224, 287082, 359152, 969429, 338314, 254676, 287922, 162583, 399871, 520489,
        ];
        for (c, e) in esperados.iter().enumerate() {
            assert_eq!(hotp(SEGREDO, c as u64, 6), *e, "contador {c}");
        }
    }

    /// RFC 6238, apendice B, coluna SHA-1 (8 digitos).
    #[test]
    fn vetores_totp_da_rfc_6238() {
        let casos = [
            (59u64, "94287082"),
            (1_111_111_109, "07081804"),
            (1_111_111_111, "14050471"),
            (1_234_567_890, "89005924"),
            (2_000_000_000, "69279037"),
            (20_000_000_000, "65353130"),
        ];
        for (t, e) in casos {
            assert_eq!(formatar(totp(SEGREDO, t, 8), 8), e, "T={t}");
        }
    }

    /// RFC 4648 §10.
    #[test]
    fn vetores_base32_da_rfc_4648() {
        let casos = [
            ("", ""),
            ("f", "MY======"),
            ("fo", "MZXQ===="),
            ("foo", "MZXW6==="),
            ("foob", "MZXW6YQ="),
            ("fooba", "MZXW6YTB"),
            ("foobar", "MZXW6YTBOI======"),
        ];
        for (claro, b32) in casos {
            assert_eq!(base32(claro.as_bytes()), b32);
            assert_eq!(de_base32(b32).unwrap(), claro.as_bytes());
            assert_eq!(
                de_base32(&b32.to_lowercase().replace('=', "")).unwrap(),
                claro.as_bytes()
            );
        }
        assert!(de_base32("MZ1W").is_none(), "1 nao e do alfabeto");
    }

    #[test]
    fn janela_de_um_passo_e_sem_reuso() {
        let t = 1_111_111_111u64;
        let passo = t / PERIODO;
        let cod = |p: u64| formatar(hotp(SEGREDO, p, DIGITOS), DIGITOS);
        assert_eq!(conferir(SEGREDO, &cod(passo), t, 0), Some(passo));
        assert_eq!(conferir(SEGREDO, &cod(passo - 1), t, 0), Some(passo - 1));
        assert_eq!(conferir(SEGREDO, &cod(passo + 1), t, 0), Some(passo + 1));
        assert_eq!(conferir(SEGREDO, &cod(passo - 2), t, 0), None, "fora");
        assert_eq!(conferir(SEGREDO, &cod(passo + 2), t, 0), None, "fora");
        // O mesmo codigo, depois de aceito: recusado.
        assert_eq!(conferir(SEGREDO, &cod(passo), t, passo), None, "reuso");
        // Um anterior ao ultimo aceito tambem.
        assert_eq!(conferir(SEGREDO, &cod(passo - 1), t, passo), None);
        // O seguinte, sim.
        assert_eq!(
            conferir(SEGREDO, &cod(passo + 1), t, passo),
            Some(passo + 1)
        );
        for ruim in ["", "12345", "1234567", "12a456", "１２３４５６"] {
            assert_eq!(conferir(SEGREDO, ruim, t, 0), None, "{ruim}");
        }
    }

    #[test]
    fn uri_escapa_e_leva_os_parametros() {
        let u = uri("phxvpn Empresa", "ana@x", b"12345678901234567890");
        assert_eq!(
            u,
            "otpauth://totp/phxvpn%20Empresa:ana%40x?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ\
             &issuer=phxvpn%20Empresa&algorithm=SHA1&digits=6&period=30"
        );
    }

    /// O QR se le de volta pelo leitor do nucleo: o que a tela mostra e o URI.
    #[test]
    fn qr_do_uri_volta_igual() {
        let u = uri("phxvpn Empresa Ltda", "joao.silva", &[7u8; SEGREDO_LEN]);
        let q = phxsql_core::qr::gerar(u.as_bytes(), phxsql_core::qr::Nivel::M).unwrap();
        assert_eq!(phxsql_core::qr::ler(&q).unwrap(), u.as_bytes());
        let svg = qr_svg(&u).unwrap();
        assert!(svg.starts_with("<svg") && svg.contains("<path d=\"M"));
        assert!(!svg.contains("GEZDG"), "o segredo nao vai em texto no SVG");
    }

    #[test]
    fn scrv1_separa_senha_e_codigo() {
        let b = phxsql_core::base64::codificar;
        let s = format!("SCRV1:{}:{}", b(b"senha:com:dois"), b(b"123456"));
        assert_eq!(scrv1(&s), ("senha:com:dois".into(), "123456".into()));
        assert_eq!(scrv1("so-senha"), ("so-senha".into(), String::new()));
        assert_eq!(scrv1("SCRV1:lixo"), ("SCRV1:lixo".into(), String::new()));
    }
}
