//! NIP-44 versao 2: a cifra das mensagens diretas do Nostr (o NIP-17 a usa duas vezes,
//! no selo e no embrulho). Escrita aqui a partir da especificacao, sem crate: ECDH
//! secp256k1 (`bip340::ecdh_x`), HKDF-SHA256 (RFC 5869) sobre o HMAC de `cripto`, ChaCha20
//! (RFC 8439) e o padding proprio do NIP. Conferida contra o `nip44.vectors.json` oficial
//! (sha256 `269ed0f6...`, o que a especificacao publica) e contra os vetores da RFC 8439 e
//! da RFC 5869.
//!
//! O NIP-04 (AES-CBC, sem MAC) NAO existe aqui, nem para ler: e legado e maleavel, e o
//! OpenJarvis, que e a referencia deste canal, nao le DM nenhuma (ele so publica kind 4
//! com o texto em claro). Aceitar ler o 04 seria abrir a porta que o 44 fechou.
//!
//! Teto proprio: 65.535 bytes de texto. A especificacao ganhou depois um prefixo estendido
//! de 6 bytes para textos maiores, mas o `encrypt_msg_lengths` do arquivo oficial de
//! vetores ainda recusa 65.536, e a propria especificacao manda cada implementacao impor
//! o seu teto antes de decodificar o base64. O agente responde em pedacos de 2.000.

use super::bip340;
use super::cripto::{hmac_sha256, iguais};
use base64::Engine;

pub const VERSAO: u8 = 2;
/// Maior texto que se cifra ou se aceita decifrar.
pub const TETO_TEXTO: usize = 65_535;
/// versao (1) + nonce (32) + mac (32) em volta do texto acolchoado.
const MOLDURA: usize = 65;
/// Menor carga decodificada: moldura + 2 de prefixo + 32 de padding minimo.
const MIN_DADOS: usize = MOLDURA + 2 + 32;
/// Maior carga decodificada no teto da casa: moldura + 2 + 65.536.
const MAX_DADOS: usize = MOLDURA + 2 + 65_536;
/// Os mesmos limites em caracteres de base64, conferidos ANTES de decodificar.
const MIN_BASE64: usize = 132;
const MAX_BASE64: usize = MAX_DADOS.div_ceil(3) * 4;

/// Por que uma carga foi recusada. Separado por motivo porque o teste de cada guarda
/// confere o motivo, e nao so «deu erro»: sem isso, tirar a conferencia da versao passaria
/// despercebido, ja que a carga cairia adiante no MAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recusa {
    VersaoDesconhecida,
    Tamanho,
    Base64,
    Mac,
    Padding,
    Utf8,
    Chave,
}

impl std::fmt::Display for Recusa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Recusa::VersaoDesconhecida => "nip44: versao de cifra desconhecida",
            Recusa::Tamanho => "nip44: tamanho fora dos limites",
            Recusa::Base64 => "nip44: base64 invalido",
            Recusa::Mac => "nip44: MAC nao confere",
            Recusa::Padding => "nip44: padding invalido",
            Recusa::Utf8 => "nip44: texto nao e UTF-8",
            Recusa::Chave => "nip44: chave invalida",
        })
    }
}

impl From<Recusa> for String {
    fn from(r: Recusa) -> String {
        r.to_string()
    }
}

// ---------------------------------------------------------------- ChaCha20 (RFC 8439)

fn quarto(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

/// Um bloco de 64 bytes de fluxo (RFC 8439 §2.3).
fn bloco(chave: &[u8; 32], contador: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let le = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let mut ini = [0u32; 16];
    ini[..4].copy_from_slice(&[0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);
    for i in 0..8 {
        ini[4 + i] = le(&chave[i * 4..]);
    }
    ini[12] = contador;
    for i in 0..3 {
        ini[13 + i] = le(&nonce[i * 4..]);
    }
    let mut s = ini;
    for _ in 0..10 {
        quarto(&mut s, 0, 4, 8, 12);
        quarto(&mut s, 1, 5, 9, 13);
        quarto(&mut s, 2, 6, 10, 14);
        quarto(&mut s, 3, 7, 11, 15);
        quarto(&mut s, 0, 5, 10, 15);
        quarto(&mut s, 1, 6, 11, 12);
        quarto(&mut s, 2, 7, 8, 13);
        quarto(&mut s, 3, 4, 9, 14);
    }
    let mut out = [0u8; 64];
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&s[i].wrapping_add(ini[i]).to_le_bytes());
    }
    out
}

/// XOR de `dados` com o fluxo a partir do bloco `contador` (RFC 8439 §2.4). O NIP-44
/// comeca do 0.
pub fn chacha20(chave: &[u8; 32], nonce: &[u8; 12], contador: u32, dados: &mut [u8]) {
    for (i, pedaco) in dados.chunks_mut(64).enumerate() {
        let k = bloco(chave, contador.wrapping_add(i as u32), nonce);
        for (b, x) in pedaco.iter_mut().zip(k.iter()) {
            *b ^= x;
        }
    }
}

// ---------------------------------------------------------------- HKDF-SHA256 (RFC 5869)

/// HKDF-Extract: PRK = HMAC(sal, IKM). O sal e a CHAVE do HMAC -- trocar a ordem compila e
/// derruba os vetores.
pub fn hkdf_extrair(sal: &[u8], ikm: &[u8]) -> [u8; 32] {
    hmac_sha256(sal, ikm)
        .try_into()
        .expect("HMAC-SHA256 tem 32 bytes")
}

/// HKDF-Expand: T(i) = HMAC(PRK, T(i-1) | info | i), ate `l` bytes (no maximo 255 * 32).
pub fn hkdf_expandir(prk: &[u8; 32], info: &[u8], l: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(l);
    let mut t: Vec<u8> = Vec::new();
    let mut i = 1u8;
    while out.len() < l {
        let mut msg = t.clone();
        msg.extend_from_slice(info);
        msg.push(i);
        t = hmac_sha256(prk, &msg);
        out.extend_from_slice(&t);
        i = i.wrapping_add(1);
    }
    out.truncate(l);
    out
}

// ---------------------------------------------------------------- NIP-44 v2

/// A chave de conversa entre A e B: `conv(a, B) == conv(b, A)`. Ela e segredo (decifra
/// tudo entre os dois): quem a calcula a usa e a deixa morrer, nunca a guarda.
pub fn chave_de_conversa(segredo: &[u8; 32], pk: &[u8; 32]) -> Result<[u8; 32], Recusa> {
    let x = bip340::ecdh_x(segredo, pk).map_err(|_| Recusa::Chave)?;
    Ok(hkdf_extrair(b"nip44-v2", &x))
}

/// (chave do ChaCha, nonce do ChaCha, chave do HMAC) da mensagem: HKDF-Expand da chave de
/// conversa com o nonce de 32 bytes como `info`, 76 bytes partidos em 32/12/32.
pub fn chaves_da_mensagem(conv: &[u8; 32], nonce: &[u8; 32]) -> ([u8; 32], [u8; 12], [u8; 32]) {
    let k = hkdf_expandir(conv, nonce, 76);
    let mut ck = [0u8; 32];
    let mut cn = [0u8; 12];
    let mut hk = [0u8; 32];
    ck.copy_from_slice(&k[..32]);
    cn.copy_from_slice(&k[32..44]);
    hk.copy_from_slice(&k[44..76]);
    (ck, cn, hk)
}

/// Tamanho do texto acolchoado (`calc_padded_len`): minimo 32; ate 256, multiplos de 32;
/// depois, multiplos de um oitavo da proxima potencia de 2.
pub fn tamanho_acolchoado(n: usize) -> usize {
    if n <= 32 {
        return 32;
    }
    // 1 << (floor(log2(n - 1)) + 1)
    let proxima = 1usize << (usize::BITS - (n - 1).leading_zeros());
    let pedaco = if proxima <= 256 { 32 } else { proxima / 8 };
    pedaco * ((n - 1) / pedaco + 1)
}

fn acolchoar(texto: &[u8]) -> Result<Vec<u8>, Recusa> {
    if texto.is_empty() || texto.len() > TETO_TEXTO {
        return Err(Recusa::Tamanho);
    }
    let mut v = Vec::with_capacity(2 + tamanho_acolchoado(texto.len()));
    v.extend_from_slice(&(texto.len() as u16).to_be_bytes());
    v.extend_from_slice(texto);
    v.resize(2 + tamanho_acolchoado(texto.len()), 0);
    Ok(v)
}

/// O prefixo diz o tamanho, e o total TEM de ser o que o prefixo manda: o padding e
/// conferido, nao so descartado. Prefixo zero seria o formato estendido, acima do teto.
fn desacolchoar(acolchoado: &[u8]) -> Result<String, Recusa> {
    if acolchoado.len() < 2 {
        return Err(Recusa::Padding);
    }
    let n = u16::from_be_bytes([acolchoado[0], acolchoado[1]]) as usize;
    if n == 0 || acolchoado.len() != 2 + tamanho_acolchoado(n) {
        return Err(Recusa::Padding);
    }
    String::from_utf8(acolchoado[2..2 + n].to_vec()).map_err(|_| Recusa::Utf8)
}

/// HMAC-SHA256 sobre `nonce | cifrado`: o nonce entra como dado autenticado, e trocar o
/// nonce de uma carga legitima derruba o MAC.
fn mac(chave: &[u8; 32], nonce: &[u8; 32], cifrado: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(32 + cifrado.len());
    m.extend_from_slice(nonce);
    m.extend_from_slice(cifrado);
    hmac_sha256(chave, &m)
}

/// Cifra `texto` com a chave de conversa e um nonce de 32 bytes. O nonce vem de fora para
/// os vetores; no canal ele sai do CSPRNG (`cifrar_aleatorio`), nunca do conteudo.
pub fn cifrar(texto: &str, conv: &[u8; 32], nonce: &[u8; 32]) -> Result<String, Recusa> {
    let (ck, cn, hk) = chaves_da_mensagem(conv, nonce);
    let mut dados = acolchoar(texto.as_bytes())?;
    chacha20(&ck, &cn, 0, &mut dados);
    let m = mac(&hk, nonce, &dados);
    let mut carga = Vec::with_capacity(MOLDURA + dados.len());
    carga.push(VERSAO);
    carga.extend_from_slice(nonce);
    carga.extend_from_slice(&dados);
    carga.extend_from_slice(&m);
    Ok(base64::engine::general_purpose::STANDARD.encode(carga))
}

/// `cifrar` com nonce novo do CSPRNG do sistema.
pub fn cifrar_aleatorio(texto: &str, conv: &[u8; 32]) -> Result<String, String> {
    let mut nonce = [0u8; 32];
    getrandom::fill(&mut nonce).map_err(|e| format!("CSPRNG do sistema indisponivel: {e}"))?;
    Ok(cifrar(texto, conv, &nonce)?)
}

/// Decifra uma carga. A ordem e a da especificacao e e ela que protege: tamanho e versao
/// antes do base64 (nada de decodificar um megabyte de lixo), MAC em tempo constante
/// ANTES de decifrar, padding conferido depois.
pub fn decifrar(carga: &str, conv: &[u8; 32]) -> Result<String, Recusa> {
    if carga.starts_with('#') {
        return Err(Recusa::VersaoDesconhecida);
    }
    if carga.len() < MIN_BASE64 || carga.len() > MAX_BASE64 {
        return Err(Recusa::Tamanho);
    }
    let dados = base64::engine::general_purpose::STANDARD
        .decode(carga)
        .map_err(|_| Recusa::Base64)?;
    if dados.len() < MIN_DADOS || dados.len() > MAX_DADOS {
        return Err(Recusa::Tamanho);
    }
    if dados[0] != VERSAO {
        return Err(Recusa::VersaoDesconhecida);
    }
    let nonce: [u8; 32] = dados[1..33].try_into().expect("32 bytes");
    let (cifrado, recebido) = dados[33..].split_at(dados.len() - 33 - 32);
    let (ck, cn, hk) = chaves_da_mensagem(conv, &nonce);
    if !iguais(&mac(&hk, &nonce, cifrado), recebido) {
        return Err(Recusa::Mac);
    }
    let mut claro = cifrado.to_vec();
    chacha20(&ck, &cn, 0, &mut claro);
    desacolchoar(&claro)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn de_hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    fn h32(s: &str) -> [u8; 32] {
        de_hex(s).try_into().unwrap()
    }

    fn hex(b: &[u8]) -> String {
        bip340::hex(b)
    }

    fn sha(b: &[u8]) -> String {
        hex(&super::super::cripto::sha256(b))
    }

    /// O arquivo oficial, copiado sem mexer de `paulmillr/nip44`; o sha256 e o que o
    /// proprio NIP-44 publica, entao vetor «ajustado» derruba este teste antes dos outros.
    fn vetores() -> Value {
        let txt = include_str!("../../tests/dados/nostr/nip44.vectors.json");
        assert_eq!(
            sha(txt.as_bytes()),
            "269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce25040"
        );
        serde_json::from_str::<Value>(txt).unwrap()["v2"].clone()
    }

    fn lista<'a>(v: &'a Value, caminho: &[&str]) -> &'a Vec<Value> {
        let mut x = v;
        for c in caminho {
            x = &x[*c];
        }
        x.as_array().unwrap()
    }

    // ------------------------------------------------ RFC 8439 e RFC 5869

    #[test]
    fn chacha20_contra_a_rfc_8439() {
        let chave: [u8; 32] = std::array::from_fn(|i| i as u8);
        // §2.3.2: o bloco serializado.
        let nonce: [u8; 12] = de_hex("000000090000004a00000000").try_into().unwrap();
        assert_eq!(
            hex(&bloco(&chave, 1, &nonce)),
            "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4e\
             d2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e"
        );
        // §2.4.2: o texto do «sunscreen», contador inicial 1, dois blocos e um resto.
        let nonce: [u8; 12] = de_hex("000000000000004a00000000").try_into().unwrap();
        let mut t = b"Ladies and Gentlemen of the class of '99: If I could offer you only one \
tip for the future, sunscreen would be it."
            .to_vec();
        chacha20(&chave, &nonce, 1, &mut t);
        assert_eq!(
            hex(&t),
            "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27afccfd9fae0b\
             f91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f0861d8\
             07ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab7793736\
             5af90bbf74a35be6b40b8eedf2785e42874d"
        );
        // A.2 #1: chave e nonce zerados, contador 0 -- o contador que o NIP-44 usa.
        let mut z = [0u8; 64];
        chacha20(&[0; 32], &[0; 12], 0, &mut z);
        assert_eq!(
            hex(&z),
            "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7\
             da41597c5157488d7724e03fb8d84a376a43b8f41518a11cc387b669b2ee6586"
        );
        // A.2 #3: contador 42.
        let mut j = de_hex(
            "2754776173206272696c6c69672c20616e642074686520736c6974687920746f\
             7665730a446964206779726520616e642067696d626c6520696e207468652077\
             6162653a0a416c6c206d696d737920776572652074686520626f726f676f7665\
             732c0a416e6420746865206d6f6d65207261746873206f757467726162652e",
        );
        let chave: [u8; 32] =
            h32("1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0");
        let nonce: [u8; 12] = de_hex("000000000000000000000002").try_into().unwrap();
        chacha20(&chave, &nonce, 42, &mut j);
        assert_eq!(
            hex(&j),
            "62e6347f95ed87a45ffae7426f27a1df5fb69110044c0d73118effa95b01e5cf\
             166d3df2d721caf9b21e5fb14c616871fd84c54f9d65b283196c7fe4f60553eb\
             f39c6402c42234e32a356b3e764312a61a5532055716ead6962568f87d3f3f77\
             04c6a8d1bcd1bf4d50d6154b6da731b187b58dfd728afa36757a797ac188d1"
        );
    }

    #[test]
    fn hkdf_contra_a_rfc_5869() {
        // Caso de teste 1 (A.1).
        let prk = hkdf_extrair(&de_hex("000102030405060708090a0b0c"), &[0x0b; 22]);
        assert_eq!(
            hex(&prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
        assert_eq!(
            hex(&hkdf_expandir(&prk, &de_hex("f0f1f2f3f4f5f6f7f8f9"), 42)),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    // ------------------------------------------------ vetores oficiais do NIP-44

    #[test]
    fn chave_de_conversa_contra_os_vetores() {
        let v = vetores();
        let casos = lista(&v, &["valid", "get_conversation_key"]);
        assert_eq!(casos.len(), 35);
        for c in casos {
            let k = chave_de_conversa(
                &h32(c["sec1"].as_str().unwrap()),
                &h32(c["pub2"].as_str().unwrap()),
            )
            .unwrap();
            assert_eq!(hex(&k), c["conversation_key"], "{}", c["note"]);
        }
    }

    #[test]
    fn chave_de_conversa_recusa_os_vetores_invalidos() {
        let v = vetores();
        let casos = lista(&v, &["invalid", "get_conversation_key"]);
        assert_eq!(casos.len(), 8);
        for c in casos {
            let r = chave_de_conversa(
                &h32(c["sec1"].as_str().unwrap()),
                &h32(c["pub2"].as_str().unwrap()),
            );
            assert_eq!(r, Err(Recusa::Chave), "{}", c["note"]);
        }
    }

    #[test]
    fn chaves_da_mensagem_contra_os_vetores() {
        let v = vetores();
        let m = &v["valid"]["get_message_keys"];
        let conv = h32(m["conversation_key"].as_str().unwrap());
        let casos = m["keys"].as_array().unwrap();
        assert!(!casos.is_empty());
        for c in casos {
            let (ck, cn, hk) = chaves_da_mensagem(&conv, &h32(c["nonce"].as_str().unwrap()));
            assert_eq!(hex(&ck), c["chacha_key"]);
            assert_eq!(hex(&cn), c["chacha_nonce"]);
            assert_eq!(hex(&hk), c["hmac_key"]);
        }
    }

    #[test]
    fn padding_contra_os_vetores() {
        let v = vetores();
        let casos = lista(&v, &["valid", "calc_padded_len"]);
        assert_eq!(casos.len(), 24);
        for c in casos {
            let (n, esperado) = (c[0].as_u64().unwrap(), c[1].as_u64().unwrap());
            assert_eq!(tamanho_acolchoado(n as usize) as u64, esperado, "{n}");
        }
    }

    /// A conversa inteira, como a especificacao manda usar o vetor: pub2 de sec2, conv de
    /// (sec1, pub2), cifra e confere a carga; pub1 de sec1, conv de (sec2, pub1), decifra.
    #[test]
    fn cifra_e_decifra_contra_os_vetores() {
        let v = vetores();
        let casos = lista(&v, &["valid", "encrypt_decrypt"]);
        assert_eq!(casos.len(), 10);
        for c in casos {
            let s = |k: &str| c[k].as_str().unwrap();
            let (sec1, sec2) = (h32(s("sec1")), h32(s("sec2")));
            let pub2 = bip340::chave_publica(&sec2).unwrap();
            let conv = chave_de_conversa(&sec1, &pub2).unwrap();
            assert_eq!(hex(&conv), s("conversation_key"));
            let carga = cifrar(s("plaintext"), &conv, &h32(s("nonce"))).unwrap();
            assert_eq!(carga, s("payload"));
            let pub1 = bip340::chave_publica(&sec1).unwrap();
            let conv2 = chave_de_conversa(&sec2, &pub1).unwrap();
            assert_eq!(conv, conv2);
            assert_eq!(decifrar(&carga, &conv2).unwrap(), s("plaintext"));
        }
    }

    #[test]
    fn mensagem_longa_contra_os_vetores() {
        let v = vetores();
        let casos = lista(&v, &["valid", "encrypt_decrypt_long_msg"]);
        assert_eq!(casos.len(), 3);
        for c in casos {
            let s = |k: &str| c[k].as_str().unwrap();
            let texto = s("pattern").repeat(c["repeat"].as_u64().unwrap() as usize);
            assert_eq!(sha(texto.as_bytes()), s("plaintext_sha256"));
            let conv = h32(s("conversation_key"));
            let carga = cifrar(&texto, &conv, &h32(s("nonce"))).unwrap();
            assert_eq!(sha(carga.as_bytes()), s("payload_sha256"));
            assert_eq!(decifrar(&carga, &conv).unwrap(), texto);
        }
    }

    #[test]
    fn recusa_tamanho_de_texto_fora_dos_limites() {
        let v = vetores();
        let casos = lista(&v, &["invalid", "encrypt_msg_lengths"]);
        assert_eq!(casos.len(), 4);
        for n in casos {
            let texto = "a".repeat(n.as_u64().unwrap() as usize);
            assert_eq!(
                cifrar(&texto, &[1; 32], &[2; 32]),
                Err(Recusa::Tamanho),
                "{n}"
            );
        }
        // O limite exato passa.
        assert!(cifrar(&"a".repeat(TETO_TEXTO), &[1; 32], &[2; 32]).is_ok());
    }

    /// Os 12 `invalid.decrypt`, cada um pelo motivo que a nota dele diz.
    fn recusa_por_nota(prefixo: &str, esperado: Recusa, quantos: usize) {
        let v = vetores();
        let mut n = 0;
        for c in lista(&v, &["invalid", "decrypt"]) {
            let nota = c["note"].as_str().unwrap();
            if !nota.starts_with(prefixo) {
                continue;
            }
            let conv = h32(c["conversation_key"].as_str().unwrap());
            let r = decifrar(c["payload"].as_str().unwrap(), &conv);
            assert_eq!(r, Err(esperado), "{nota}");
            n += 1;
        }
        assert_eq!(n, quantos, "{prefixo}");
    }

    #[test]
    fn recusa_versao_desconhecida() {
        recusa_por_nota("unknown encryption version", Recusa::VersaoDesconhecida, 2);
    }

    #[test]
    fn recusa_base64_invalido() {
        recusa_por_nota("invalid base64", Recusa::Base64, 1);
    }

    #[test]
    fn recusa_mac_invalido() {
        recusa_por_nota("invalid MAC", Recusa::Mac, 2);
    }

    #[test]
    fn recusa_padding_invalido() {
        recusa_por_nota("invalid padding", Recusa::Padding, 3);
    }

    #[test]
    fn recusa_carga_de_tamanho_fora_dos_limites() {
        recusa_por_nota("invalid payload length", Recusa::Tamanho, 4);
        // E acima do teto, antes de decodificar: base64 valido e grande demais.
        let grande = "A".repeat(MAX_BASE64 + 4);
        assert_eq!(decifrar(&grande, &[1; 32]), Err(Recusa::Tamanho));
        // O tamanho e conferido ANTES do base64: lixo grande ou curto e recusado pelo
        // tamanho, sem o decodificador sequer olhar (o motivo seria `Base64` se olhasse).
        assert_eq!(
            decifrar(&"!".repeat(MAX_BASE64 + 4), &[1; 32]),
            Err(Recusa::Tamanho)
        );
        assert_eq!(
            decifrar(&"!".repeat(MIN_BASE64 - 1), &[1; 32]),
            Err(Recusa::Tamanho)
        );
        // E de novo depois de decodificar: 132 caracteres com `==` sao 97 bytes, e o maior
        // base64 aceito pode trazer um byte a mais que o teto.
        let b64 = |d: &[u8]| base64::engine::general_purpose::STANDARD.encode(d);
        let mut curto = vec![VERSAO];
        curto.resize(MIN_DADOS - 2, 0);
        assert_eq!(b64(&curto).len(), MIN_BASE64);
        assert_eq!(decifrar(&b64(&curto), &[1; 32]), Err(Recusa::Tamanho));
        let mut longo = vec![VERSAO];
        longo.resize(MAX_DADOS + 1, 0);
        assert!(b64(&longo).len() <= MAX_BASE64);
        assert_eq!(decifrar(&b64(&longo), &[1; 32]), Err(Recusa::Tamanho));
    }

    /// Trocar o nonce de uma carga boa derruba o MAC, e mexer em um byte do cifrado tambem.
    /// Este teste NAO isola o nonce como AAD: o nonce tambem e o `info` do HKDF, entao
    /// trocado ele ja muda a chave do HMAC (repor o MAC sem o nonce passou aqui). Quem
    /// derruba a falta do AAD sao os vetores de `cifra_e_decifra_contra_os_vetores`.
    #[test]
    fn mac_cobre_o_nonce_e_o_cifrado() {
        let conv = [7u8; 32];
        let carga = cifrar("oi", &conv, &[9; 32]).unwrap();
        let mut d = base64::engine::general_purpose::STANDARD
            .decode(&carga)
            .unwrap();
        let b64 = |d: &[u8]| base64::engine::general_purpose::STANDARD.encode(d);
        let mut outro_nonce = d.clone();
        outro_nonce[5] ^= 1;
        assert_eq!(decifrar(&b64(&outro_nonce), &conv), Err(Recusa::Mac));
        d[40] ^= 1;
        assert_eq!(decifrar(&b64(&d), &conv), Err(Recusa::Mac));
    }
}
