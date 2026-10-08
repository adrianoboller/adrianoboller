//! TLS 1.3 do lado do servidor (RFC 8446): o aperto de mao e o fluxo
//! protegido que sai dele.
//!
//! # Por que existe
//!
//! Pedido 572, T4: TLS no transporte escrito nesta casa (decisao do dono,
//! 30/09/2026). O cronograma de chaves e o registro protegido estao em
//! [`crate::tls13`], conferidos contra a RFC 8448; aqui fica o que depende de
//! soquete, e por isso se confere contra clientes de verdade (`openssl
//! s_client`, `curl`) e nao contra vetor.
//!
//! # O que se oferece, e o que nao
//!
//! - So TLS 1.3. Cliente que nao oferece a `0x0304` recebe `protocol_version`.
//! - Conjunto `TLS_CHACHA20_POLY1305_SHA256`. O AES-128-GCM, que a §9.1 obriga,
//!   e o T5: o AES do `phxzip` nao e de tempo constante e nao serve para chave
//!   de sessao.
//! - Troca por X25519 ou P-256; a assinatura e `ecdsa_secp256r1_sha256`, a
//!   unica que um certificado desta casa carrega.
//! - `HelloRetryRequest` quando o cliente nao mandou chave de um grupo nosso
//!   mas os anuncia.
//! - Nada de PSK, retomada, 0-RTT ou certificado de cliente: o cliente que
//!   pede isso recebe o aperto completo (a extensao se ignora, como a §4.2
//!   manda fazer com o que nao se entende).
//!
//! # Tetos antes da credencial
//!
//! Tudo aqui acontece antes de qualquer senha: o `ClientHello` inteiro tem
//! teto ([`TETO_CLIENT_HELLO`]) e nenhum registro passa de 2^14 + 256 bytes.
//! E a licao do pedido 434: leitura sem teto antes da credencial e memoria que
//! qualquer um aloca.

use std::io::{Read, Write};

use crate::error::{PhxError, Result};
use crate::hash::iguais_em_tempo_constante;
use crate::tls13::{self, tipo, Conjunto, Protecao, Transcricao, MAX_CLARO, RESUMO};

mod cliente;
pub use cliente::{conectar, pino_de_texto, pino_em_texto, Confianca, OpcoesCliente};

/// O maior `ClientHello` aceito. Um navegador com chave pos-quantica manda
/// uns 1,8 KiB; o teto da folga de sobra sem deixar um estranho alocar 16 MiB
/// (o maximo que o campo de 24 bits permite).
pub const TETO_CLIENT_HELLO: usize = 32 * 1024;

/// A maior mensagem de aperto de mao que se aceita do cliente depois do
/// `ClientHello` (o `Finished` tem 36 bytes; o `KeyUpdate`, 5).
const TETO_MENSAGEM: usize = 1024;

const X25519: u16 = 0x001d;
const SECP256R1: u16 = 0x0017;
const ECDSA_SECP256R1_SHA256: u16 = 0x0403;
const TLS13: u16 = 0x0304;

mod ext {
    pub const SERVER_NAME: u16 = 0;
    pub const GRUPOS: u16 = 10;
    pub const ASSINATURAS: u16 = 13;
    pub const ALPN: u16 = 16;
    pub const COOKIE: u16 = 44;
    pub const VERSOES: u16 = 43;
    pub const CHAVES: u16 = 51;
}

mod hs {
    pub const CLIENT_HELLO: u8 = 1;
    pub const SERVER_HELLO: u8 = 2;
    pub const NEW_SESSION_TICKET: u8 = 4;
    pub const EXTENSOES: u8 = 8;
    pub const CERTIFICADO: u8 = 11;
    pub const PEDIDO_DE_CERTIFICADO: u8 = 13;
    pub const VERIFICACAO: u8 = 15;
    pub const FINISHED: u8 = 20;
    pub const KEY_UPDATE: u8 = 24;
    pub const MESSAGE_HASH: u8 = 254;
}

/// Os alertas que esta casa manda (§6), dos dois lados.
mod alerta {
    pub const CLOSE_NOTIFY: u8 = 0;
    pub const UNEXPECTED_MESSAGE: u8 = 10;
    pub const BAD_RECORD_MAC: u8 = 20;
    pub const RECORD_OVERFLOW: u8 = 22;
    pub const HANDSHAKE_FAILURE: u8 = 40;
    pub const BAD_CERTIFICATE: u8 = 42;
    pub const UNSUPPORTED_CERTIFICATE: u8 = 43;
    pub const ILLEGAL_PARAMETER: u8 = 47;
    pub const DECODE_ERROR: u8 = 50;
    pub const DECRYPT_ERROR: u8 = 51;
    pub const PROTOCOL_VERSION: u8 = 70;
    pub const INTERNAL_ERROR: u8 = 80;
    pub const MISSING_EXTENSION: u8 = 109;
    pub const UNSUPPORTED_EXTENSION: u8 = 110;
}

/// O `random` do `HelloRetryRequest`: o SHA-256 de "HelloRetryRequest"
/// (§4.1.3). E ele que diz ao cliente que aquele `ServerHello` e um pedido.
const RANDOM_HRR: [u8; 32] = [
    0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8, 0x91,
    0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8, 0x33, 0x9c,
];

/// Quem o servidor diz ser: a cadeia de certificados (DER, o da folha
/// primeiro) e a chave privada P-256 da folha.
pub struct Identidade {
    cadeia: Vec<Vec<u8>>,
    privada: [u8; 32],
}

impl std::fmt::Debug for Identidade {
    // A privada nunca aparece: nem em log, nem em mensagem de erro.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identidade")
            .field("certificados", &self.cadeia.len())
            .finish_non_exhaustive()
    }
}

impl Identidade {
    pub fn nova(cadeia: Vec<Vec<u8>>, privada: [u8; 32]) -> Result<Identidade> {
        if cadeia.is_empty() {
            return Err(PhxError::Esquema("TLS: identidade sem certificado".into()));
        }
        if crate::p256::chave_publica(&privada).is_none() {
            return Err(PhxError::Esquema(
                "TLS: chave privada P-256 invalida".into(),
            ));
        }
        Ok(Identidade { cadeia, privada })
    }

    /// Um certificado autoassinado novo, para os `nomes` dados.
    pub fn autoassinada(nomes: &[&str]) -> Result<Identidade> {
        let privada = crate::p256::gerar_privada();
        let mut serie = [0u8; 16];
        crate::cifra::sortear(&mut serie);
        serie[0] &= 0x7f;
        serie[0] |= 0x01;
        let validade = crate::x509::Validade::de_agora_por_anos(1);
        let cert = crate::x509::cert_tls_p256(nomes, &validade, &serie, &privada)
            .ok_or_else(|| PhxError::Esquema("TLS: nao consegui assinar o certificado".into()))?;
        Identidade::nova(vec![cert], privada)
    }

    /// O certificado da folha, em DER.
    pub fn certificado(&self) -> &[u8] {
        &self.cadeia[0]
    }

    /// O pino desta identidade: `SHA-256` do SPKI da folha -- o que o
    /// cliente que confia por pino ([`Confianca::Pino`]) tem de trazer.
    pub fn pino(&self) -> Result<[u8; 32]> {
        Ok(crate::hash::sha256(crate::x509::spki_do_certificado(
            &self.cadeia[0],
        )?))
    }
}

/// Falha do aperto: o alerta que o cliente recebe e o erro que fica aqui.
struct Falha(u8, PhxError);

type Aperto<T> = std::result::Result<T, Falha>;

fn falha<T>(alerta: u8, texto: &str) -> Aperto<T> {
    Err(Falha(alerta, PhxError::Corrompido(format!("TLS: {texto}"))))
}

fn de_io<T>(r: std::io::Result<T>) -> Aperto<T> {
    r.map_err(|e| Falha(alerta::INTERNAL_ERROR, PhxError::Io(e)))
}

fn de_phx<T>(alerta: u8, r: Result<T>) -> Aperto<T> {
    r.map_err(|e| Falha(alerta, e))
}

// ------------------------------------------------------------ leitura ----

/// Leitor de bytes do aperto de mao; todo tamanho vem conferido contra o que
/// sobra, e o que nao cabe vira `decode_error`.
struct Leitor<'a> {
    b: &'a [u8],
}

impl<'a> Leitor<'a> {
    fn bytes(&mut self, n: usize) -> Aperto<&'a [u8]> {
        if self.b.len() < n {
            return falha(alerta::DECODE_ERROR, "mensagem truncada");
        }
        let (a, r) = self.b.split_at(n);
        self.b = r;
        Ok(a)
    }
    fn u8(&mut self) -> Aperto<u8> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Aperto<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn vetor8(&mut self) -> Aperto<&'a [u8]> {
        let n = self.u8()? as usize;
        self.bytes(n)
    }
    fn vetor16(&mut self) -> Aperto<&'a [u8]> {
        let n = self.u16()? as usize;
        self.bytes(n)
    }
    fn fim(&self) -> Aperto<()> {
        if self.b.is_empty() {
            Ok(())
        } else {
            falha(alerta::DECODE_ERROR, "sobra depois da mensagem")
        }
    }
}

fn lista_u16(b: &[u8]) -> Aperto<Vec<u16>> {
    if !b.len().is_multiple_of(2) {
        return falha(alerta::DECODE_ERROR, "lista de u16 com tamanho impar");
    }
    Ok(b.chunks(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect())
}

/// O que interessa de um `ClientHello`.
#[derive(Default)]
struct Ola {
    sessao: Vec<u8>,
    conjuntos: Vec<u16>,
    versoes: Vec<u16>,
    grupos: Vec<u16>,
    chaves: Vec<(u16, Vec<u8>)>,
    assinaturas: Vec<u16>,
    alpn: Vec<Vec<u8>>,
    nome: Option<String>,
}

fn analisar_ola(corpo: &[u8]) -> Aperto<Ola> {
    let mut l = Leitor { b: corpo };
    let mut o = Ola::default();
    l.u16()?; // legacy_version: a versao de verdade vem na extensao.
    l.bytes(32)?;
    let sessao = l.vetor8()?;
    if sessao.len() > 32 {
        return falha(
            alerta::ILLEGAL_PARAMETER,
            "legacy_session_id com mais de 32 bytes",
        );
    }
    o.sessao = sessao.to_vec();
    o.conjuntos = lista_u16(l.vetor16()?)?;
    if !l.vetor8()?.contains(&0) {
        return falha(alerta::ILLEGAL_PARAMETER, "compressao sem o nulo");
    }
    let mut extensoes = Leitor { b: l.vetor16()? };
    l.fim()?;
    let mut vistas: Vec<u16> = Vec::new();
    while !extensoes.b.is_empty() {
        let t = extensoes.u16()?;
        let dados = extensoes.vetor16()?;
        // §4.2: a mesma extensao duas vezes e erro, nao «vale a ultima».
        if vistas.contains(&t) {
            return falha(alerta::ILLEGAL_PARAMETER, "extensao repetida");
        }
        vistas.push(t);
        let mut d = Leitor { b: dados };
        match t {
            ext::VERSOES => {
                o.versoes = lista_u16(d.vetor8()?)?;
                d.fim()?;
            }
            ext::GRUPOS => {
                o.grupos = lista_u16(d.vetor16()?)?;
                d.fim()?;
            }
            ext::ASSINATURAS => {
                o.assinaturas = lista_u16(d.vetor16()?)?;
                d.fim()?;
            }
            ext::CHAVES => {
                let mut v = Leitor { b: d.vetor16()? };
                d.fim()?;
                while !v.b.is_empty() {
                    let g = v.u16()?;
                    let k = v.vetor16()?;
                    if o.chaves.iter().any(|(x, _)| *x == g) {
                        return falha(alerta::ILLEGAL_PARAMETER, "key_share com grupo repetido");
                    }
                    o.chaves.push((g, k.to_vec()));
                }
            }
            ext::ALPN => {
                let mut v = Leitor { b: d.vetor16()? };
                d.fim()?;
                while !v.b.is_empty() {
                    o.alpn.push(v.vetor8()?.to_vec());
                }
            }
            ext::SERVER_NAME => {
                // So o primeiro nome do tipo host_name (0); o resto se ignora.
                let mut v = Leitor { b: d.vetor16()? };
                if v.u8()? == 0 {
                    o.nome = String::from_utf8(v.vetor16()?.to_vec()).ok();
                }
            }
            _ => {}
        }
    }
    Ok(o)
}

// ------------------------------------------------------------ escrita ----

fn vetor8(v: &[u8]) -> Vec<u8> {
    let mut r = vec![v.len() as u8];
    r.extend_from_slice(v);
    r
}

fn vetor16(v: &[u8]) -> Vec<u8> {
    let mut r = (v.len() as u16).to_be_bytes().to_vec();
    r.extend_from_slice(v);
    r
}

fn vetor24(v: &[u8]) -> Vec<u8> {
    let n = v.len() as u32;
    let mut r = n.to_be_bytes()[1..].to_vec();
    r.extend_from_slice(v);
    r
}

fn mensagem(tipo_hs: u8, corpo: &[u8]) -> Vec<u8> {
    let mut r = vec![tipo_hs];
    r.extend_from_slice(&vetor24(corpo));
    r
}

fn extensao(t: u16, dados: &[u8]) -> Vec<u8> {
    let mut r = t.to_be_bytes().to_vec();
    r.extend_from_slice(&vetor16(dados));
    r
}

fn server_hello(random: &[u8; 32], sessao: &[u8], conjunto: Conjunto, chave_ext: &[u8]) -> Vec<u8> {
    let mut c = vec![3, 3];
    c.extend_from_slice(random);
    c.extend_from_slice(&vetor8(sessao));
    c.extend_from_slice(&conjunto.id().to_be_bytes());
    c.push(0);
    let mut e = extensao(ext::VERSOES, &TLS13.to_be_bytes());
    e.extend_from_slice(&extensao(ext::CHAVES, chave_ext));
    c.extend_from_slice(&vetor16(&e));
    mensagem(hs::SERVER_HELLO, &c)
}

/// A frase que a assinatura do servidor cobre (§4.4.3).
fn conteudo_da_verificacao(resumo: &[u8; RESUMO]) -> Vec<u8> {
    let mut m = vec![0x20u8; 64];
    m.extend_from_slice(b"TLS 1.3, server CertificateVerify");
    m.push(0);
    m.extend_from_slice(resumo);
    m
}

// ------------------------------------------------------------ registro ----

/// O fio cru e a protecao corrente de cada sentido. Durante o aperto a
/// protecao muda duas vezes; depois dele, so por `KeyUpdate`.
struct Registro<S> {
    fluxo: S,
    envio: Option<Protecao>,
    recebimento: Option<Protecao>,
    /// Bytes de aperto de mao ja recebidos e ainda nao consumidos.
    pendente: Vec<u8>,
}

impl<S: Read + Write> Registro<S> {
    fn ler_bruto(&mut self) -> Aperto<Option<([u8; 5], Vec<u8>)>> {
        let mut cab = [0u8; 5];
        match self.fluxo.read(&mut cab[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => {}
            Err(e) => return Err(Falha(alerta::INTERNAL_ERROR, PhxError::Io(e))),
        }
        de_io(self.fluxo.read_exact(&mut cab[1..]))?;
        // O tipo e a versao se conferem ANTES do tamanho: quem fala HTTP cru
        // numa porta TLS manda "GET /", e o "T /" viraria um tamanho de 8 KiB
        // que o servidor ficaria esperando ate o prazo -- sem o cliente
        // receber resposta nenhuma.
        if !(20..=23).contains(&cab[0]) || cab[1] != 3 {
            return falha(alerta::UNEXPECTED_MESSAGE, "isto nao e um registro TLS");
        }
        let n = u16::from_be_bytes([cab[3], cab[4]]) as usize;
        if n > MAX_CLARO + 256 {
            return falha(alerta::RECORD_OVERFLOW, "registro maior que 2^14 + 256");
        }
        let mut corpo = vec![0u8; n];
        de_io(self.fluxo.read_exact(&mut corpo))?;
        Ok(Some((cab, corpo)))
    }

    /// O proximo registro ja aberto: `(tipo verdadeiro, conteudo)`. O
    /// `change_cipher_spec` de compatibilidade (§D.4) se engole aqui, e so
    /// enquanto o aperto nao terminou.
    fn ler(&mut self, aceita_ccs: bool) -> Aperto<Option<(u8, Vec<u8>)>> {
        loop {
            let Some((cab, corpo)) = self.ler_bruto()? else {
                return Ok(None);
            };
            if cab[0] == 20 {
                if aceita_ccs && corpo == [1] {
                    continue;
                }
                return falha(
                    alerta::UNEXPECTED_MESSAGE,
                    "change_cipher_spec fora de hora",
                );
            }
            return match self.recebimento.as_mut() {
                None => {
                    if corpo.len() > MAX_CLARO {
                        return falha(alerta::RECORD_OVERFLOW, "registro claro maior que 2^14");
                    }
                    Ok(Some((cab[0], corpo)))
                }
                Some(p) => {
                    if cab[0] != tipo::DADOS {
                        return falha(
                            alerta::UNEXPECTED_MESSAGE,
                            "registro claro depois das chaves",
                        );
                    }
                    let (t, c) = p
                        .abrir(&cab, &corpo)
                        .map_err(|e| Falha(alerta_da_abertura(&e), e))?;
                    Ok(Some((t, c)))
                }
            };
        }
    }

    fn escrever(&mut self, t: u8, conteudo: &[u8]) -> Aperto<()> {
        for pedaco in conteudo.chunks(MAX_CLARO) {
            let registro = match self.envio.as_mut() {
                Some(p) => de_phx(alerta::INTERNAL_ERROR, p.selar(t, pedaco))?,
                None => {
                    let mut r = vec![t, 3, 3];
                    r.extend_from_slice(&(pedaco.len() as u16).to_be_bytes());
                    r.extend_from_slice(pedaco);
                    r
                }
            };
            de_io(self.fluxo.write_all(&registro))?;
        }
        Ok(())
    }

    /// A proxima mensagem de aperto de mao inteira (cabecalho incluido), com
    /// o teto `teto` sobre o corpo.
    fn mensagem(&mut self, teto: usize) -> Aperto<Vec<u8>> {
        loop {
            if self.pendente.len() >= 4 {
                let n =
                    u32::from_be_bytes([0, self.pendente[1], self.pendente[2], self.pendente[3]])
                        as usize;
                if n > teto {
                    return falha(alerta::DECODE_ERROR, "mensagem de aperto acima do teto");
                }
                if self.pendente.len() >= 4 + n {
                    let resto = self.pendente.split_off(4 + n);
                    return Ok(std::mem::replace(&mut self.pendente, resto));
                }
            }
            match self.ler(true)? {
                None => return falha(alerta::DECODE_ERROR, "conexao fechou no meio do aperto"),
                Some((tipo::HANDSHAKE, c)) => {
                    if c.is_empty() {
                        return falha(alerta::UNEXPECTED_MESSAGE, "registro de aperto vazio");
                    }
                    self.pendente.extend_from_slice(&c);
                }
                Some((tipo::ALERTA, a)) => {
                    // O codigo do alerta vai no erro: do lado cliente e a
                    // unica pista de por que o servidor alheio recusou.
                    return Err(Falha(
                        alerta::CLOSE_NOTIFY,
                        PhxError::Corrompido(format!(
                            "TLS: o par abortou o aperto com o alerta {}",
                            a.get(1).copied().unwrap_or(0)
                        )),
                    ));
                }
                Some(_) => return falha(alerta::UNEXPECTED_MESSAGE, "dado antes do fim do aperto"),
            }
        }
    }

    /// §5.1: mensagem de aperto nao atravessa troca de chave.
    fn nada_pendente(&self) -> Aperto<()> {
        if self.pendente.is_empty() {
            Ok(())
        } else {
            falha(
                alerta::UNEXPECTED_MESSAGE,
                "mensagem atravessando a troca de chave",
            )
        }
    }

    fn alertar(&mut self, a: u8) {
        let nivel = if a == alerta::CLOSE_NOTIFY { 1 } else { 2 };
        let _ = self.escrever(tipo::ALERTA, &[nivel, a]);
        let _ = self.fluxo.flush();
    }
}

/// §5.2: registro que nao abre e `bad_record_mac`; o grande demais e
/// `record_overflow`.
fn alerta_da_abertura(e: &PhxError) -> u8 {
    match e {
        PhxError::LimiteExcedido(_) => alerta::RECORD_OVERFLOW,
        _ => alerta::BAD_RECORD_MAC,
    }
}

// ------------------------------------------------------------ aperto ----

/// A escolha do grupo e o segredo compartilhado.
#[derive(Clone)]
enum Troca {
    X25519([u8; 32]),
    P256([u8; 32]),
}

impl Troca {
    fn nova(grupo: u16) -> Troca {
        if grupo == X25519 {
            Troca::X25519(crate::x25519::gerar_privada())
        } else {
            Troca::P256(crate::p256::gerar_privada())
        }
    }

    fn grupo(&self) -> u16 {
        match self {
            Troca::X25519(_) => X25519,
            Troca::P256(_) => SECP256R1,
        }
    }

    fn publica(&self) -> Vec<u8> {
        match self {
            Troca::X25519(k) => crate::x25519::chave_publica(k).to_vec(),
            Troca::P256(k) => crate::p256::chave_publica(k)
                .expect("privada sorteada pela propria p256 e valida")
                .to_vec(),
        }
    }

    fn segredo(&self, do_par: &[u8]) -> Aperto<[u8; 32]> {
        match self {
            Troca::X25519(k) => {
                let Ok(p) = <[u8; 32]>::try_from(do_par) else {
                    return falha(alerta::ILLEGAL_PARAMETER, "chave X25519 sem 32 bytes");
                };
                de_phx(alerta::ILLEGAL_PARAMETER, crate::x25519::segredo(k, &p))
            }
            Troca::P256(k) => match crate::p256::ecdh(k, do_par) {
                Some(s) => Ok(s),
                None => falha(alerta::ILLEGAL_PARAMETER, "ponto P-256 do par invalido"),
            },
        }
    }
}

/// Os grupos na ordem da nossa preferencia.
const GRUPOS: [u16; 2] = [X25519, SECP256R1];

/// Os conjuntos na ordem da nossa preferencia: o ChaCha20 primeiro, porque o
/// AES desta casa e de tempo constante e por isso mais lento (`crate::aes`).
/// A §4.1.1 deixa a escolha ao servidor.
const CONJUNTOS: [Conjunto; 2] = [Conjunto::Chacha20Poly1305Sha256, Conjunto::Aes128GcmSha256];

/// Confere o `ClientHello` e devolve o conjunto escolhido.
fn conferir_ola(o: &Ola) -> Aperto<Conjunto> {
    if !o.versoes.contains(&TLS13) {
        return falha(alerta::PROTOCOL_VERSION, "o cliente nao oferece TLS 1.3");
    }
    let Some(conjunto) = CONJUNTOS
        .into_iter()
        .find(|c| o.conjuntos.contains(&c.id()))
    else {
        return falha(
            alerta::HANDSHAKE_FAILURE,
            "o cliente nao oferece TLS_CHACHA20_POLY1305_SHA256 nem TLS_AES_128_GCM_SHA256",
        );
    };
    if !o.assinaturas.contains(&ECDSA_SECP256R1_SHA256) {
        return falha(
            alerta::HANDSHAKE_FAILURE,
            "o cliente nao aceita ecdsa_secp256r1_sha256",
        );
    }
    Ok(conjunto)
}

/// O que o aperto deixa para o fluxo: o negociado e os tres segredos que
/// sobrevivem a ele (os dois de trafego e o do exportador, §7.5).
pub(crate) struct Segredos {
    pub(crate) negociado: Negociado,
    pub(crate) c_ap: [u8; RESUMO],
    pub(crate) s_ap: [u8; RESUMO],
    pub(crate) exp: [u8; RESUMO],
}

/// O que o aperto negociou, para quem usa o fluxo.
#[derive(Debug, Clone, Default)]
pub struct Negociado {
    /// O protocolo de aplicacao (ALPN) escolhido, se o cliente ofereceu um
    /// que o servidor aceita.
    pub alpn: Option<Vec<u8>>,
    /// O nome que o cliente pediu (SNI).
    pub nome: Option<String>,
    /// O grupo da troca de chaves (`0x001d` X25519, `0x0017` P-256).
    pub grupo: u16,
    /// O conjunto de cifra do registro.
    pub conjunto: Conjunto,
    /// Do lado CLIENTE: o pino do servidor, `SHA-256` do SPKI do certificado
    /// que ele mostrou e cuja chave assinou o aperto. Quem conectou sem pino
    /// (primeiro contato) le daqui o que anotar. Do lado servidor, `None`.
    pub pino: Option<[u8; 32]>,
}

fn apertar<S: Read + Write>(
    r: &mut Registro<S>,
    id: &Identidade,
    alpn_aceitos: &[&[u8]],
) -> Aperto<Segredos> {
    let mut transcricao = Transcricao::default();
    let m = r.mensagem(TETO_CLIENT_HELLO)?;
    if m[0] != hs::CLIENT_HELLO {
        return falha(
            alerta::UNEXPECTED_MESSAGE,
            "o primeiro nao foi um ClientHello",
        );
    }
    let mut ola = analisar_ola(&m[4..])?;
    let conjunto = conferir_ola(&ola)?;
    transcricao.acrescentar(&m);
    let mut ccs_enviado = false;

    let escolhida = GRUPOS
        .iter()
        .find_map(|g| ola.chaves.iter().find(|(x, _)| x == g).cloned());
    let (grupo, chave_cliente) = match escolhida {
        Some(e) => e,
        None => {
            // Nenhuma chave nossa, mas talvez um grupo nosso: pede de novo.
            let Some(&g) = GRUPOS.iter().find(|g| ola.grupos.contains(g)) else {
                return falha(
                    alerta::HANDSHAKE_FAILURE,
                    "nenhum grupo em comum (X25519, P-256)",
                );
            };
            r.nada_pendente()?;
            // §4.4.1: a transcricao passa a comecar pelo resumo do primeiro
            // ClientHello, embrulhado numa `message_hash`.
            let resumo_ch1 = transcricao.resumo();
            transcricao = Transcricao::default();
            transcricao.acrescentar(&mensagem(hs::MESSAGE_HASH, &resumo_ch1));
            let hrr = server_hello(&RANDOM_HRR, &ola.sessao, conjunto, &g.to_be_bytes());
            transcricao.acrescentar(&hrr);
            r.escrever(tipo::HANDSHAKE, &hrr)?;
            if !ola.sessao.is_empty() {
                r.escrever(20, &[1])?;
                ccs_enviado = true;
            }
            de_io(r.fluxo.flush())?;
            let m2 = r.mensagem(TETO_CLIENT_HELLO)?;
            if m2[0] != hs::CLIENT_HELLO {
                return falha(
                    alerta::UNEXPECTED_MESSAGE,
                    "depois do HRR nao veio ClientHello",
                );
            }
            let ola2 = analisar_ola(&m2[4..])?;
            // §4.1.4: o conjunto do HRR e o do ServerHello tem de ser o mesmo.
            if conferir_ola(&ola2)? != conjunto {
                return falha(
                    alerta::ILLEGAL_PARAMETER,
                    "o segundo ClientHello mudou o conjunto de cifra",
                );
            }
            if ola2.sessao != ola.sessao {
                return falha(
                    alerta::ILLEGAL_PARAMETER,
                    "o segundo ClientHello trocou a sessao",
                );
            }
            transcricao.acrescentar(&m2);
            // §4.1.2: o segundo traz UMA chave, do grupo pedido.
            let chave = match ola2.chaves.as_slice() {
                [(x, k)] if *x == g => k.clone(),
                _ => {
                    return falha(
                        alerta::ILLEGAL_PARAMETER,
                        "o segundo ClientHello nao trouxe a chave do grupo pedido",
                    )
                }
            };
            ola = ola2;
            (g, chave)
        }
    };
    r.nada_pendente()?;

    let troca = Troca::nova(grupo);
    let compartilhado = troca.segredo(&chave_cliente)?;
    let mut random = [0u8; 32];
    crate::cifra::sortear(&mut random);
    let mut chave_ext = troca.grupo().to_be_bytes().to_vec();
    chave_ext.extend_from_slice(&vetor16(&troca.publica()));
    let sh = server_hello(&random, &ola.sessao, conjunto, &chave_ext);
    transcricao.acrescentar(&sh);
    r.escrever(tipo::HANDSHAKE, &sh)?;
    if !ola.sessao.is_empty() && !ccs_enviado {
        r.escrever(20, &[1])?;
    }

    let early = tls13::segredo_early();
    let segredo_hs = tls13::segredo_handshake(&early, &compartilhado);
    let t_sh = transcricao.resumo();
    let c_hs = tls13::derivar_segredo(&segredo_hs, "c hs traffic", &t_sh);
    let s_hs = tls13::derivar_segredo(&segredo_hs, "s hs traffic", &t_sh);
    r.envio = Some(Protecao::de_segredo_com(&s_hs, conjunto));
    r.recebimento = Some(Protecao::de_segredo_com(&c_hs, conjunto));

    // EncryptedExtensions: so o ALPN, e so se houver um em comum.
    let alpn = ola
        .alpn
        .iter()
        .find(|p| alpn_aceitos.contains(&p.as_slice()))
        .cloned();
    let mut ee = Vec::new();
    if let Some(p) = &alpn {
        ee = extensao(ext::ALPN, &vetor16(&vetor8(p)));
    }
    let mut voo = mensagem(hs::EXTENSOES, &vetor16(&ee));
    transcricao.acrescentar(&voo);

    let mut lista = Vec::new();
    for c in &id.cadeia {
        lista.extend_from_slice(&vetor24(c));
        lista.extend_from_slice(&vetor16(&[]));
    }
    let mut corpo_cert = vetor8(&[]);
    corpo_cert.extend_from_slice(&vetor24(&lista));
    let cert = mensagem(hs::CERTIFICADO, &corpo_cert);
    transcricao.acrescentar(&cert);
    voo.extend_from_slice(&cert);

    let Some((rr, ss)) =
        crate::p256::assinar(&id.privada, &conteudo_da_verificacao(&transcricao.resumo()))
    else {
        return falha(
            alerta::INTERNAL_ERROR,
            "nao consegui assinar o CertificateVerify",
        );
    };
    let mut corpo_cv = ECDSA_SECP256R1_SHA256.to_be_bytes().to_vec();
    corpo_cv.extend_from_slice(&vetor16(&crate::x509::der_ecdsa(&rr, &ss)));
    let cv = mensagem(hs::VERIFICACAO, &corpo_cv);
    transcricao.acrescentar(&cv);
    voo.extend_from_slice(&cv);

    let fin = mensagem(
        hs::FINISHED,
        &tls13::verify_data(&s_hs, &transcricao.resumo()),
    );
    transcricao.acrescentar(&fin);
    voo.extend_from_slice(&fin);
    r.escrever(tipo::HANDSHAKE, &voo)?;
    de_io(r.fluxo.flush())?;

    let t_sf = transcricao.resumo();
    let master = tls13::segredo_master(&segredo_hs);
    let c_ap = tls13::derivar_segredo(&master, "c ap traffic", &t_sf);
    let s_ap = tls13::derivar_segredo(&master, "s ap traffic", &t_sf);
    let exp = tls13::derivar_segredo(&master, "exp master", &t_sf);

    let m = r.mensagem(TETO_MENSAGEM)?;
    if m[0] != hs::FINISHED {
        return falha(
            alerta::UNEXPECTED_MESSAGE,
            "o cliente nao mandou o Finished",
        );
    }
    let esperado = tls13::verify_data(&c_hs, &t_sf);
    if !iguais_em_tempo_constante(&m[4..], &esperado) {
        return falha(alerta::DECRYPT_ERROR, "o Finished do cliente nao confere");
    }
    r.nada_pendente()?;

    let negociado = Negociado {
        alpn,
        nome: ola.nome,
        grupo,
        conjunto,
        pino: None,
    };
    Ok(Segredos {
        negociado,
        c_ap,
        s_ap,
        exp,
    })
}

/// Faz o aperto de mao do servidor sobre `fluxo` e devolve o fluxo
/// protegido. Em falha, o cliente recebe o alerta da §6 antes do erro voltar.
///
/// `alpn_aceitos` e a lista dos protocolos de aplicacao que o servidor fala,
/// na ordem da preferencia do CLIENTE (a §3.2 da RFC 7301 deixa a escolha ao
/// servidor; escolher o primeiro do cliente que esta na lista e o que os
/// servidores comuns fazem).
pub fn aceitar<S: Read + Write>(
    fluxo: S,
    id: &Identidade,
    alpn_aceitos: &[&[u8]],
) -> Result<FluxoTls<S>> {
    let mut r = Registro {
        fluxo,
        envio: None,
        recebimento: None,
        pendente: Vec::new(),
    };
    match apertar(&mut r, id, alpn_aceitos) {
        Ok(Segredos {
            negociado,
            c_ap,
            s_ap,
            exp,
        }) => Ok(FluxoTls {
            r: Registro {
                fluxo: r.fluxo,
                envio: Some(Protecao::de_segredo_com(&s_ap, negociado.conjunto)),
                recebimento: Some(Protecao::de_segredo_com(&c_ap, negociado.conjunto)),
                pendente: Vec::new(),
            },
            segredo_envio: s_ap,
            segredo_recebimento: c_ap,
            segredo_exportador: exp,
            claro: Vec::new(),
            pos: 0,
            fechado: false,
            cliente: false,
            negociado,
        }),
        Err(Falha(a, e)) => {
            if a != alerta::CLOSE_NOTIFY {
                r.alertar(a);
            }
            Err(e)
        }
    }
}

/// Um [`FluxoTls`] para quem le por um `BufReader` e escreve por outra
/// ponta -- o formato dos clientes desta casa (`replica::Cliente`, o
/// `Remoto`, o driver ODBC), que nasceram com o soquete partido em dois.
///
/// O registro protegido tem estado (a sequencia de cada sentido) e nao se
/// parte: as duas pontas sao a MESMA conexao atras de uma trava. A trava nao
/// custa disputa, porque esses clientes sao pedido-resposta numa thread so;
/// ela existe para o tipo continuar `Send` (a replica muda de thread com o
/// cliente dentro), que o `Rc` do `fio_dados` do servidor nao seria.
pub struct Compartilhado<S>(std::sync::Arc<std::sync::Mutex<FluxoTls<S>>>);

impl<S> Clone for Compartilhado<S> {
    fn clone(&self) -> Self {
        Compartilhado(std::sync::Arc::clone(&self.0))
    }
}

impl<S: Read + Write> Compartilhado<S> {
    pub fn novo(fluxo: FluxoTls<S>) -> Compartilhado<S> {
        Compartilhado(std::sync::Arc::new(std::sync::Mutex::new(fluxo)))
    }

    /// O fluxo, para o que nao e ler nem escrever (prazo, vinculo, adeus).
    /// Trava envenenada nao derruba: o estado do registro so muda depois de
    /// cada operacao inteira, e um panico no meio de uma deixa a conexao
    /// inutil de qualquer jeito -- o proximo `read` diz isso.
    pub fn com<R>(&self, f: impl FnOnce(&mut FluxoTls<S>) -> R) -> R {
        let mut g = self.0.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut g)
    }
}

impl<S: Read + Write> Read for Compartilhado<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.com(|t| t.read(buf))
    }
}

impl<S: Read + Write> Write for Compartilhado<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.com(|t| t.write(buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.com(|t| t.flush())
    }
}

/// O soquete de um CLIENTE desta casa (`replica::Cliente`, o driver ODBC):
/// as duas pontas do [`ComPrazo`] em claro, ou o MESMO fluxo TLS dos dois
/// lados -- pedido 572, T6b-2.
///
/// Mora aqui, e nao em cada cliente, porque a passagem para o TLS e uma
/// decisao so (conferir o buffer vazio, uma terceira ponta com o mesmo prazo,
/// o aperto pelo pino, o rearme por ponta unica) e os clientes nasceram com o
/// mesmo formato: `BufReader` sobre uma ponta, escrita pela outra. Duas copias
/// dela divergiriam no dia em que uma ganhasse um conserto.
///
/// [`ComPrazo`]: crate::prazo::ComPrazo
pub enum FioDeCliente {
    Claro(crate::prazo::ComPrazo),
    Tls(Compartilhado<crate::prazo::ComPrazo>),
}

impl Read for FioDeCliente {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            FioDeCliente::Claro(c) => c.read(b),
            FioDeCliente::Tls(t) => t.read(b),
        }
    }
}

impl Write for FioDeCliente {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            FioDeCliente::Claro(c) => c.write(b),
            FioDeCliente::Tls(t) => t.write(b),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            FioDeCliente::Claro(c) => c.flush(),
            FioDeCliente::Tls(t) => t.flush(),
        }
    }
}

impl FioDeCliente {
    /// `true` quando a conversa passa por TLS.
    pub fn tls(&self) -> bool {
        matches!(self, FioDeCliente::Tls(_))
    }

    /// O `tls-exporter` (RFC 9266) quando e TLS -- o vinculo que o
    /// `amarrar_canal` do login e a prova do pulso usam no lugar da
    /// transcricao do Noise.
    pub fn vinculo_do_canal(&self) -> Option<[u8; 32]> {
        match self {
            FioDeCliente::Tls(t) => Some(t.com(|f| f.vinculo_do_canal())),
            FioDeCliente::Claro(_) => None,
        }
    }

    /// Recomeca o total por pedido (pedido 578): nas duas pontas em claro, ou
    /// na unica do TLS -- a do `leitor` e a mesma.
    pub fn rearmar(leitor: &mut FioDeCliente, escrita: &mut FioDeCliente) {
        match (leitor, escrita) {
            (FioDeCliente::Claro(l), FioDeCliente::Claro(e)) => crate::prazo::rearmar(l, e),
            (_, FioDeCliente::Tls(t)) => t.com(|f| f.fio_mut().rearmar()),
            (FioDeCliente::Tls(_), FioDeCliente::Claro(_)) => {}
        }
    }

    /// Passa a conexao para TLS 1.3, conferindo o servidor pelo `pino`.
    ///
    /// So por pino, de proposito: entre dois PhxSql nao ha autoridade a
    /// consultar, e TLS sem conferir quem responde protegeria da escuta
    /// passiva e de nada mais. Nada lido pode estar no `BufReader`: o TLS
    /// comeca no primeiro byte, e um byte em claro guardado ali seria lido
    /// como se tivesse vindo pelo tunel.
    pub fn passar_a_tls(
        leitor: &mut std::io::BufReader<FioDeCliente>,
        escrita: &mut FioDeCliente,
        pino: [u8; 32],
    ) -> Result<()> {
        if !leitor.buffer().is_empty() {
            return Err(PhxError::Esquema(
                "o TLS so comeca numa conexao em que nada foi lido".into(),
            ));
        }
        let FioDeCliente::Claro(e) = &*escrita else {
            return Err(PhxError::Esquema(
                "esta conexao ja esta em TLS: uma cifra por conexao".into(),
            ));
        };
        let mut uma = e.clonar()?;
        uma.rearmar();
        let op = OpcoesCliente {
            // SNI nao vai: o pino ja diz quem se espera, e o host (as vezes
            // um IP, que a RFC 6066 §3 proibe no SNI) nao acrescenta.
            nome: None,
            alpn: &[],
            confianca: Confianca::Pino(pino),
        };
        let t = Compartilhado::novo(conectar(uma, &op)?);
        *leitor = std::io::BufReader::new(FioDeCliente::Tls(t.clone()));
        *escrita = FioDeCliente::Tls(t);
        Ok(())
    }

    /// O `close_notify`, para o `Drop` de quem e dono da conexao -- o
    /// servidor ve um adeus em vez de um corte.
    pub fn despedir(&self) {
        if let FioDeCliente::Tls(t) = self {
            t.com(|f| {
                let _ = f.despedir();
            });
        }
    }
}

/// O rotulo do vinculo ao canal (RFC 9266 §2).
pub const ROTULO_DO_VINCULO: &str = "EXPORTER-Channel-Binding";

// ------------------------------------------------------------ fluxo ----

/// O fluxo de aplicacao protegido. Le e escreve bytes claros; os registros,
/// o `KeyUpdate` e o `close_notify` ficam por dentro.
pub struct FluxoTls<S> {
    r: Registro<S>,
    segredo_envio: [u8; RESUMO],
    segredo_recebimento: [u8; RESUMO],
    /// O `exporter_master_secret` (§7.1). Guardado porque o vinculo ao canal
    /// (RFC 9266) e pedido DEPOIS do aperto, no `login`; fica fora do `Debug`
    /// como os outros segredos.
    segredo_exportador: [u8; RESUMO],
    claro: Vec<u8>,
    pos: usize,
    fechado: bool,
    /// O lado cliente aceita o `NewSessionTicket` depois do aperto (e o
    /// descarta: retomada nao entra, plano do 572 §2.3); o servidor nunca
    /// recebe um.
    cliente: bool,
    negociado: Negociado,
}

impl<S> std::fmt::Debug for FluxoTls<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FluxoTls")
            .field("negociado", &self.negociado)
            .finish_non_exhaustive()
    }
}

fn proximo_segredo(s: &[u8; RESUMO]) -> [u8; RESUMO] {
    let mut n = [0u8; RESUMO];
    tls13::expandir_rotulo(s, "traffic upd", &[], &mut n);
    n
}

fn io_de(Falha(_, e): Falha) -> std::io::Error {
    match e {
        PhxError::Io(e) => e,
        outro => std::io::Error::new(std::io::ErrorKind::InvalidData, outro.to_string()),
    }
}

impl<S: Read + Write> FluxoTls<S> {
    pub fn negociado(&self) -> &Negociado {
        &self.negociado
    }

    /// `TLS-Exporter` (RFC 8446 §7.5) desta conexao: os dois lados chegam ao
    /// mesmo valor so se ninguem terminou o TLS no meio.
    pub fn exportar(&self, rotulo: &str, contexto: &[u8], tamanho: usize) -> Result<Vec<u8>> {
        let mut saida = vec![0u8; tamanho];
        tls13::exportar(&self.segredo_exportador, rotulo, contexto, &mut saida)?;
        Ok(saida)
    }

    /// O vinculo ao canal `tls-exporter` da RFC 9266 §2: rotulo
    /// `EXPORTER-Channel-Binding`, contexto vazio, 32 bytes.
    ///
    /// E o que faz o papel da transcricao do Noise no `amarrar_canal` do
    /// login e na prova de identidade do pulso: a prova presa a ESTE valor nao
    /// vale numa conexao que outro terminou. O `tls-unique` do TLS 1.2 nao
    /// existe no 1.3 (RFC 9266 §1), e o `tls-server-end-point` so prende ao
    /// certificado, que o homem-no-meio com o mesmo certificado repetiria.
    pub fn vinculo_do_canal(&self) -> [u8; 32] {
        let mut v = [0u8; 32];
        tls13::exportar(&self.segredo_exportador, ROTULO_DO_VINCULO, &[], &mut v)
            .expect("rotulo e tamanho fixos cabem no HKDF");
        v
    }

    /// O fio cru por baixo (para prazo de leitura, endereco do par...).
    pub fn fio(&self) -> &S {
        &self.r.fluxo
    }

    /// O fio cru por baixo, mutavel -- para rearmar o prazo de quem conversa
    /// (`prazo::ComPrazo`) a cada pedido, sem tocar no registro.
    pub fn fio_mut(&mut self) -> &mut S {
        &mut self.r.fluxo
    }

    /// Manda o `close_notify` (§6.1). Depois dele nada mais se escreve.
    pub fn despedir(&mut self) -> std::io::Result<()> {
        self.r
            .escrever(tipo::ALERTA, &[1, alerta::CLOSE_NOTIFY])
            .map_err(io_de)?;
        self.r.fluxo.flush()
    }

    fn pos_aperto(&mut self, mut msgs: Vec<u8>) -> Aperto<()> {
        while !msgs.is_empty() {
            if msgs.len() < 4 {
                return falha(alerta::DECODE_ERROR, "mensagem de aperto truncada");
            }
            let n = u32::from_be_bytes([0, msgs[1], msgs[2], msgs[3]]) as usize;
            // O ticket do servidor pode passar de 1 KiB; do cliente, nada
            // pos-aperto passa.
            let teto = if self.cliente {
                MAX_CLARO
            } else {
                TETO_MENSAGEM
            };
            if msgs.len() < 4 + n || n > teto {
                // §5.1 permite partir, mas nao ha mensagem pos-aperto do
                // cliente que precise; partir aqui e so custo de memoria.
                return falha(
                    alerta::DECODE_ERROR,
                    "mensagem pos-aperto partida ou grande",
                );
            }
            let resto = msgs.split_off(4 + n);
            match (msgs[0], &msgs[4..]) {
                (hs::KEY_UPDATE, [pedido @ (0 | 1)]) => {
                    // §4.6.3: o proximo registro do cliente ja vem com a chave nova.
                    self.segredo_recebimento = proximo_segredo(&self.segredo_recebimento);
                    self.r.recebimento = Some(Protecao::de_segredo_com(
                        &self.segredo_recebimento,
                        self.negociado.conjunto,
                    ));
                    if *pedido == 1 {
                        self.r
                            .escrever(tipo::HANDSHAKE, &mensagem(hs::KEY_UPDATE, &[0]))?;
                        self.segredo_envio = proximo_segredo(&self.segredo_envio);
                        self.r.envio = Some(Protecao::de_segredo_com(
                            &self.segredo_envio,
                            self.negociado.conjunto,
                        ));
                    }
                }
                // §4.6.1: o ticket se le e se joga fora -- quem nao retoma
                // nao precisa guardar segredo de sessao nenhum.
                (hs::NEW_SESSION_TICKET, _) if self.cliente => {}
                (hs::KEY_UPDATE, _) => {
                    return falha(alerta::ILLEGAL_PARAMETER, "KeyUpdate malformado")
                }
                _ => return falha(alerta::UNEXPECTED_MESSAGE, "mensagem pos-aperto inesperada"),
            }
            msgs = resto;
        }
        Ok(())
    }

    fn encher(&mut self) -> Aperto<()> {
        while self.pos >= self.claro.len() && !self.fechado {
            match self.r.ler(false)? {
                // Fim do fio sem `close_notify`: para quem le, e fim.
                None => self.fechado = true,
                Some((tipo::DADOS, c)) => {
                    self.claro = c;
                    self.pos = 0;
                }
                Some((tipo::ALERTA, a)) => {
                    self.fechado = true;
                    if a.get(1) != Some(&alerta::CLOSE_NOTIFY) {
                        return falha(alerta::CLOSE_NOTIFY, "o par mandou um alerta");
                    }
                }
                Some((tipo::HANDSHAKE, c)) => self.pos_aperto(c)?,
                Some(_) => {
                    return falha(alerta::UNEXPECTED_MESSAGE, "tipo de registro desconhecido")
                }
            }
        }
        Ok(())
    }
}

impl<S: Read + Write> Read for FluxoTls<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Err(Falha(a, e)) = self.encher() {
            if a != alerta::CLOSE_NOTIFY {
                self.r.alertar(a);
            }
            self.fechado = true;
            return Err(io_de(Falha(a, e)));
        }
        let disponivel = &self.claro[self.pos.min(self.claro.len())..];
        let n = disponivel.len().min(buf.len());
        buf[..n].copy_from_slice(&disponivel[..n]);
        self.pos += n;
        Ok(n)
    }
}

impl<S: Read + Write> Write for FluxoTls<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = buf.len().min(MAX_CLARO);
        self.r.escrever(tipo::DADOS, &buf[..n]).map_err(io_de)?;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.r.fluxo.flush()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::process::{Command, Stdio};
    use std::time::Duration;

    /// Diretorio do teste que se apaga no `Drop`: o core nao tem
    /// `apoio_teste`.
    /// Tambem e o guarda dos testes do cliente (`tls::cliente::testes`):
    /// um segundo guarda la seria um segundo `temp_dir` fora do catalogo.
    pub(super) struct Dir(pub(super) std::path::PathBuf);
    impl Dir {
        pub(super) fn novo(nome: &str) -> Dir {
            let d = std::env::temp_dir().join(format!("phx-tls-{nome}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Dir(d)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    type Resultado = std::result::Result<(Negociado, String), String>;

    /// Um servidor de UMA conexao: aperta a mao, le um pedido HTTP e responde
    /// `ola!`. Devolve a porta e o que o servidor viu.
    fn servir(id: Identidade) -> (u16, std::thread::JoinHandle<Resultado>) {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (fio, _) = ouvinte.accept().map_err(|e| e.to_string())?;
            fio.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
            let mut t = aceitar(fio, &id, &[b"http/1.1"]).map_err(|e| e.to_string())?;
            let mut pedido = Vec::new();
            let mut b = [0u8; 512];
            while !pedido.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = t.read(&mut b).map_err(|e| e.to_string())?;
                if n == 0 {
                    return Err("fim antes do pedido".into());
                }
                pedido.extend_from_slice(&b[..n]);
            }
            t.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nola!\n")
                .map_err(|e| e.to_string())?;
            t.despedir().map_err(|e| e.to_string())?;
            Ok((
                t.negociado().clone(),
                String::from_utf8_lossy(&pedido).into_owned(),
            ))
        });
        (porta, h)
    }

    fn com_certificado(nome: &str) -> (Dir, Identidade, std::path::PathBuf) {
        let d = Dir::novo(nome);
        let id = Identidade::autoassinada(&["localhost", "127.0.0.1"]).unwrap();
        let pem = d.0.join("cert.pem");
        std::fs::write(&pem, crate::x509::para_pem(id.certificado(), "CERTIFICATE")).unwrap();
        (d, id, pem)
    }

    /// `openssl s_client` com `extra`, mandando `linhas` uma a uma, com uma
    /// pausa. Os comandos de uma letra (`K`) so valem SEM `-ign_eof`; por
    /// isso a entrada fica aberta ate o servidor `h` terminar, e so entao
    /// fecha -- senao o fim da entrada derrubaria a conexao antes da resposta.
    fn s_client(
        porta: u16,
        pem: &std::path::Path,
        extra: &[&str],
        linhas: &[&str],
        h: std::thread::JoinHandle<Resultado>,
    ) -> (String, Resultado) {
        let mut filho = Command::new("openssl")
            .args(["s_client", "-connect", &format!("127.0.0.1:{porta}")])
            .args(["-tls1_3", "-verify_return_error", "-CAfile"])
            .arg(pem)
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut entrada = filho.stdin.take().unwrap();
        for l in linhas {
            std::thread::sleep(Duration::from_millis(400));
            let _ = entrada.write_all(l.as_bytes());
        }
        let visto = h.join().unwrap();
        std::thread::sleep(Duration::from_millis(200));
        drop(entrada);
        let s = filho.wait_with_output().unwrap();
        let saida = format!(
            "{}{}",
            String::from_utf8_lossy(&s.stdout),
            String::from_utf8_lossy(&s.stderr)
        );
        (saida, visto)
    }

    const PEDIDO: &str = "GET / HTTP/1.0\r\nHost: x\r\n\r\n";

    #[test]
    fn o_curl_confere_o_certificado_e_recebe_a_resposta() {
        let (_d, id, pem) = com_certificado("curl");
        let (porta, h) = servir(id);
        let s = Command::new("curl")
            .args(["-sS", "--max-time", "20", "--tlsv1.3"])
            .args(["--tls13-ciphers", "TLS_CHACHA20_POLY1305_SHA256"])
            .arg("--cacert")
            .arg(&pem)
            .arg(format!("https://127.0.0.1:{porta}/"))
            .output()
            .unwrap();
        let (neg, pedido) = h.join().unwrap().expect("o servidor falhou");
        assert_eq!(
            String::from_utf8_lossy(&s.stdout),
            "ola!\n",
            "{}",
            String::from_utf8_lossy(&s.stderr)
        );
        assert!(pedido.starts_with("GET / HTTP/1.1"), "{pedido}");
        assert_eq!(neg.alpn.as_deref(), Some(&b"http/1.1"[..]));
        assert_eq!(neg.grupo, X25519);
    }

    #[test]
    fn o_openssl_troca_por_p256_e_confere_o_nome() {
        let (_d, id, pem) = com_certificado("p256");
        let (porta, h) = servir(id);
        let (s, visto) = s_client(
            porta,
            &pem,
            &[
                "-groups",
                "P-256",
                "-verify_hostname",
                "localhost",
                "-servername",
                "localhost",
                "-alpn",
                "h2,http/1.1",
            ],
            &[PEDIDO],
            h,
        );
        let (neg, _) = visto.expect("o servidor falhou");
        assert!(s.contains("ola!"), "{s}");
        assert!(s.contains("Verify return code: 0 (ok)"), "{s}");
        assert!(s.contains("TLS_CHACHA20_POLY1305_SHA256"), "{s}");
        assert!(s.contains("ALPN protocol: http/1.1"), "{s}");
        assert_eq!(neg.grupo, SECP256R1);
        assert_eq!(neg.nome.as_deref(), Some("localhost"));
    }

    #[test]
    fn sem_chave_de_grupo_nosso_o_servidor_pede_de_novo() {
        // P-384 primeiro: o OpenSSL manda so a chave dela, e o X25519 so
        // aparece na lista de grupos -- o caminho do HelloRetryRequest.
        let (_d, id, pem) = com_certificado("hrr");
        let (porta, h) = servir(id);
        let (s, visto) = s_client(
            porta,
            &pem,
            &["-groups", "P-384:X25519", "-msg"],
            &[PEDIDO],
            h,
        );
        let (neg, _) = visto.expect("o servidor falhou");
        assert!(s.contains("ola!"), "{s}");
        assert_eq!(neg.grupo, X25519);
        assert_eq!(s.matches("ClientHello").count(), 2, "sem HRR: {s}");
    }

    #[test]
    fn a_troca_de_chave_no_meio_da_conexao_continua_legivel() {
        // `K` pede ao servidor que troque a dele tambem (§4.6.3).
        let (_d, id, pem) = com_certificado("keyupdate");
        let (porta, h) = servir(id);
        let (s, visto) = s_client(porta, &pem, &["-msg"], &["K\n", PEDIDO], h);
        visto.expect("o servidor falhou");
        assert!(s.contains("ola!"), "{s}");
        assert!(
            s.matches("KeyUpdate").count() >= 2,
            "o servidor nao respondeu o KeyUpdate: {s}"
        );
    }

    #[test]
    fn cliente_sem_o_nosso_conjunto_recebe_o_alerta_e_nao_a_conexao() {
        // AES-256 so: nenhum dos dois que esta casa fala.
        let (_d, id, pem) = com_certificado("sem-conjunto");
        let (porta, h) = servir(id);
        let (s, visto) = s_client(
            porta,
            &pem,
            &["-ciphersuites", "TLS_AES_256_GCM_SHA384"],
            &[PEDIDO],
            h,
        );
        let e = visto.expect_err("aceitou sem conjunto em comum");
        assert!(e.contains("AES_128_GCM"), "{e}");
        assert!(s.contains("handshake failure"), "{s}");
        assert!(!s.contains("ola!"));
    }

    #[test]
    fn cliente_so_de_aes_128_gcm_conversa_pelo_aes() {
        // O que a §9.1 obriga: o cliente que so fala AES-128-GCM, com o curl
        // conferindo a cadeia -- e troca de chave no meio, que re-deriva a
        // chave AES pelo conjunto negociado.
        let (_d, id, pem) = com_certificado("so-aes");
        let (porta, h) = servir(id);
        let (s, visto) = s_client(
            porta,
            &pem,
            &["-ciphersuites", "TLS_AES_128_GCM_SHA256", "-msg"],
            &["K\n", PEDIDO],
            h,
        );
        let (neg, _) = visto.expect("o servidor falhou");
        assert!(s.contains("ola!"), "{s}");
        assert!(s.contains("TLS_AES_128_GCM_SHA256"), "{s}");
        assert!(s.matches("KeyUpdate").count() >= 2, "{s}");
        assert_eq!(neg.conjunto, Conjunto::Aes128GcmSha256);

        let (_d2, id, pem) = com_certificado("so-aes-curl");
        let (porta, h) = servir(id);
        let c = Command::new("curl")
            .args(["-sS", "--max-time", "20", "--tlsv1.3"])
            .args(["--tls13-ciphers", "TLS_AES_128_GCM_SHA256"])
            .arg("--cacert")
            .arg(&pem)
            .arg(format!("https://127.0.0.1:{porta}/"))
            .output()
            .unwrap();
        let (neg, _) = h.join().unwrap().expect("o servidor falhou com o curl");
        assert_eq!(String::from_utf8_lossy(&c.stdout), "ola!\n");
        assert_eq!(neg.conjunto, Conjunto::Aes128GcmSha256);
    }

    #[test]
    fn com_os_dois_oferecidos_o_servidor_prefere_o_chacha() {
        let (_d, id, pem) = com_certificado("preferencia");
        let (porta, h) = servir(id);
        let (s, visto) = s_client(
            porta,
            &pem,
            &[
                "-ciphersuites",
                "TLS_AES_128_GCM_SHA256:TLS_CHACHA20_POLY1305_SHA256",
            ],
            &[PEDIDO],
            h,
        );
        let (neg, _) = visto.expect("o servidor falhou");
        assert!(s.contains("ola!"), "{s}");
        assert_eq!(neg.conjunto, Conjunto::Chacha20Poly1305Sha256);
    }

    /// O que o `openssl` imprime como `Keying material:` -- em minusculas,
    /// para comparar com o `para_hex` desta casa.
    fn material_exportado(saida: &str) -> Option<String> {
        saida
            .lines()
            .find_map(|l| l.trim().strip_prefix("Keying material: "))
            .map(str::to_ascii_lowercase)
    }

    /// Um servidor de UMA conexao que manda o proprio `tls-exporter` em hex,
    /// para o `openssl s_client` comparar com o que ele exportou.
    fn servir_o_vinculo(id: Identidade) -> (u16, std::thread::JoinHandle<Resultado>) {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (fio, _) = ouvinte.accept().map_err(|e| e.to_string())?;
            fio.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
            let mut t = aceitar(fio, &id, &[]).map_err(|e| e.to_string())?;
            let v = crate::hash::para_hex(&t.vinculo_do_canal());
            t.write_all(format!("vinculo={v}\n").as_bytes())
                .map_err(|e| e.to_string())?;
            t.despedir().map_err(|e| e.to_string())?;
            Ok((t.negociado().clone(), v))
        });
        (porta, h)
    }

    #[test]
    fn o_vinculo_do_canal_e_o_tls_exporter_que_o_openssl_exporta() {
        // RFC 9266 §2 pela RFC 8446 §7.5: rotulo `EXPORTER-Channel-Binding`,
        // contexto vazio, 32 bytes. Nao ha traco oficial deste valor (a RFC
        // 8448 para no `exp master`, conferido no `tls13`), entao o vetor e o
        // do OpenSSL na MESMA conexao -- nos dois conjuntos e nos dois grupos,
        // porque o HRR muda a transcricao de que o `exp master` sai.
        for (nome, extra) in [
            ("vinc-chacha", vec![]),
            (
                "vinc-aes-hrr",
                vec![
                    "-ciphersuites",
                    "TLS_AES_128_GCM_SHA256",
                    "-groups",
                    "P-384:P-256",
                ],
            ),
        ] {
            let (_d, id, pem) = com_certificado(nome);
            let (porta, h) = servir_o_vinculo(id);
            let mut args = vec!["-keymatexport", ROTULO_DO_VINCULO, "-keymatexportlen", "32"];
            args.extend(extra);
            let (s, visto) = s_client(porta, &pem, &args, &[], h);
            let (_, nosso) = visto.expect("o servidor falhou");
            assert!(s.contains(&format!("vinculo={nosso}")), "{s}");
            assert_eq!(
                material_exportado(&s).as_deref(),
                Some(nosso.as_str()),
                "{nome}: o exportador desta casa diverge do OpenSSL\n{s}"
            );
        }
    }

    #[test]
    fn cliente_de_tls_1_2_recebe_protocol_version() {
        let (_d, id, _) = com_certificado("tls12");
        let (porta, h) = servir(id);
        let s = Command::new("openssl")
            .args([
                "s_client",
                "-connect",
                &format!("127.0.0.1:{porta}"),
                "-tls1_2",
            ])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let e = h.join().unwrap().expect_err("aceitou TLS 1.2");
        assert!(e.contains("1.3"), "{e}");
        let saida = String::from_utf8_lossy(&s.stderr);
        assert!(saida.contains("protocol version"), "{saida}");
    }

    #[test]
    fn http_em_claro_recebe_o_alerta_na_hora() {
        let (_d, id, _) = com_certificado("http-cru");
        let (porta, h) = servir(id);
        let mut c = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        c.write_all(b"GET /saude HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut resposta = [0u8; 7];
        c.read_exact(&mut resposta)
            .expect("o servidor ficou esperando em vez de alertar");
        assert_eq!(resposta, [21, 3, 3, 0, 2, 2, alerta::UNEXPECTED_MESSAGE]);
        let e = h.join().unwrap().expect_err("aceitou HTTP cru");
        assert!(e.contains("registro TLS"), "{e}");
    }

    #[test]
    fn client_hello_acima_do_teto_se_recusa_sem_alocar() {
        let (_d, id, _) = com_certificado("teto");
        let (porta, h) = servir(id);
        let mut c = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        // Um registro de aperto anunciando um ClientHello de 16 MiB.
        c.write_all(&[22, 3, 1, 0, 4, 1, 0xff, 0xff, 0xff]).unwrap();
        let e = h.join().unwrap().expect_err("aceitou o anuncio");
        assert!(e.contains("teto"), "{e}");
        let mut resposta = [0u8; 7];
        c.read_exact(&mut resposta).unwrap();
        assert_eq!(resposta, [21, 3, 3, 0, 2, 2, alerta::DECODE_ERROR]);
    }

    // ---------------------------------------------------- cliente cru ----
    //
    // O que nenhum cliente honesto faz: mandar o `Finished` errado, ou uma
    // mensagem atravessando a troca de chave. So um cliente escrito aqui
    // chega nesses caminhos -- e o caminho BOM dele e o que prova que o
    // vermelho do adulterado vem da conferencia, e nao de um cliente torto.

    #[derive(Clone, Copy, PartialEq)]
    enum Desvio {
        Nenhum,
        FinishedAdulterado,
        SobraNoClientHello,
    }

    fn registro_cru(c: &mut TcpStream) -> ([u8; 5], Vec<u8>) {
        let mut cab = [0u8; 5];
        c.read_exact(&mut cab).unwrap();
        let mut corpo = vec![0u8; u16::from_be_bytes([cab[3], cab[4]]) as usize];
        c.read_exact(&mut corpo).unwrap();
        (cab, corpo)
    }

    /// Devolve o alerta que o servidor mandou (nivel, descricao), ou a
    /// resposta HTTP quando nao houve alerta.
    fn cliente_cru(porta: u16, desvio: Desvio) -> std::result::Result<String, [u8; 2]> {
        let mut c = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        let privada = crate::x25519::gerar_privada();
        let mut corpo = vec![3, 3];
        corpo.extend_from_slice(&[7u8; 32]);
        corpo.extend_from_slice(&vetor8(&[]));
        corpo.extend_from_slice(&vetor16(
            &Conjunto::Chacha20Poly1305Sha256.id().to_be_bytes(),
        ));
        corpo.extend_from_slice(&vetor8(&[0]));
        let mut chave = X25519.to_be_bytes().to_vec();
        chave.extend_from_slice(&vetor16(&crate::x25519::chave_publica(&privada)));
        let mut e = extensao(ext::VERSOES, &vetor8(&TLS13.to_be_bytes()));
        e.extend_from_slice(&extensao(ext::GRUPOS, &vetor16(&X25519.to_be_bytes())));
        e.extend_from_slice(&extensao(
            ext::ASSINATURAS,
            &vetor16(&ECDSA_SECP256R1_SHA256.to_be_bytes()),
        ));
        e.extend_from_slice(&extensao(ext::CHAVES, &vetor16(&chave)));
        corpo.extend_from_slice(&vetor16(&e));
        let ch = mensagem(hs::CLIENT_HELLO, &corpo);
        let mut transcricao = Transcricao::default();
        transcricao.acrescentar(&ch);
        let mut enviado = ch.clone();
        if desvio == Desvio::SobraNoClientHello {
            // O comeco de um Finished grudado no ClientHello, no mesmo registro.
            enviado.extend_from_slice(&[hs::FINISHED, 0, 0, 32]);
        }
        let mut reg = vec![22, 3, 1];
        reg.extend_from_slice(&(enviado.len() as u16).to_be_bytes());
        reg.extend_from_slice(&enviado);
        c.write_all(&reg).unwrap();

        let (cab, sh) = registro_cru(&mut c);
        if cab[0] == tipo::ALERTA {
            return Err([sh[0], sh[1]]);
        }
        transcricao.acrescentar(&sh);
        // O servidor desta casa poe o key_share por ultimo: a chave e o fim.
        let publica: [u8; 32] = sh[sh.len() - 32..].try_into().unwrap();
        let comp = crate::x25519::segredo(&privada, &publica).unwrap();
        let segredo_hs = tls13::segredo_handshake(&tls13::segredo_early(), &comp);
        let t_sh = transcricao.resumo();
        let c_hs = tls13::derivar_segredo(&segredo_hs, "c hs traffic", &t_sh);
        let s_hs = tls13::derivar_segredo(&segredo_hs, "s hs traffic", &t_sh);
        let mut de_la = Protecao::de_segredo(&s_hs);
        let mut daqui = Protecao::de_segredo(&c_hs);

        let mut voo = Vec::new();
        loop {
            let (cab, corpo) = registro_cru(&mut c);
            let (t, claro) = de_la.abrir(&cab, &corpo).unwrap();
            assert_eq!(t, tipo::HANDSHAKE);
            voo.extend_from_slice(&claro);
            let mut resto = &voo[..];
            let mut viu_finished = false;
            while resto.len() >= 4 {
                let n = u32::from_be_bytes([0, resto[1], resto[2], resto[3]]) as usize;
                if resto.len() < 4 + n {
                    break;
                }
                viu_finished |= resto[0] == hs::FINISHED;
                resto = &resto[4 + n..];
            }
            if viu_finished {
                break;
            }
        }
        transcricao.acrescentar(&voo);
        let t_sf = transcricao.resumo();
        let mut fin = tls13::verify_data(&c_hs, &t_sf);
        if desvio == Desvio::FinishedAdulterado {
            fin[0] ^= 1;
        }
        c.write_all(
            &daqui
                .selar(tipo::HANDSHAKE, &mensagem(hs::FINISHED, &fin))
                .unwrap(),
        )
        .unwrap();

        let master = tls13::segredo_master(&segredo_hs);
        let mut app_daqui =
            Protecao::de_segredo(&tls13::derivar_segredo(&master, "c ap traffic", &t_sf));
        let mut app_de_la =
            Protecao::de_segredo(&tls13::derivar_segredo(&master, "s ap traffic", &t_sf));
        if desvio == Desvio::FinishedAdulterado {
            // O alerta vem ainda sob a chave do aperto: o servidor nao chegou
            // a trocar.
            let (cab, corpo) = registro_cru(&mut c);
            let (t, a) = de_la.abrir(&cab, &corpo).unwrap();
            assert_eq!(t, tipo::ALERTA);
            return Err([a[0], a[1]]);
        }
        c.write_all(&app_daqui.selar(tipo::DADOS, PEDIDO.as_bytes()).unwrap())
            .unwrap();
        let (cab, corpo) = registro_cru(&mut c);
        let (t, resposta) = app_de_la.abrir(&cab, &corpo).unwrap();
        assert_eq!(t, tipo::DADOS);
        Ok(String::from_utf8(resposta).unwrap())
    }

    #[test]
    fn o_cliente_cru_honesto_conversa() {
        let (_d, id, _) = com_certificado("cru-honesto");
        let (porta, h) = servir(id);
        let r = cliente_cru(porta, Desvio::Nenhum);
        h.join()
            .unwrap()
            .expect("o servidor falhou com o cliente honesto");
        assert!(r.expect("recebeu alerta").ends_with("ola!\n"));
    }

    #[test]
    fn finished_adulterado_recebe_decrypt_error_e_nenhum_dado() {
        let (_d, id, _) = com_certificado("cru-finished");
        let (porta, h) = servir(id);
        let r = cliente_cru(porta, Desvio::FinishedAdulterado);
        let e = h
            .join()
            .unwrap()
            .expect_err("aceitou o Finished adulterado");
        assert!(e.contains("Finished"), "{e}");
        assert_eq!(r, Err([2, alerta::DECRYPT_ERROR]));
    }

    #[test]
    fn mensagem_atravessando_a_troca_de_chave_e_recusada() {
        let (_d, id, _) = com_certificado("cru-sobra");
        let (porta, h) = servir(id);
        let r = cliente_cru(porta, Desvio::SobraNoClientHello);
        let e = h.join().unwrap().expect_err("aceitou a sobra");
        assert!(e.contains("atravessando"), "{e}");
        assert_eq!(r, Err([2, alerta::UNEXPECTED_MESSAGE]));
    }
}
