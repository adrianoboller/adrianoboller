//! TLS 1.3 do lado do CLIENTE (RFC 8446): o espelho do [`super::aceitar`].
//!
//! # Por que existe
//!
//! Pedido 572, fatia T6b-1 (`docs/propostas/plano-tls13-572.md` §6): o
//! servidor ja falava TLS 1.3 desde 30/09, mas nenhum ponto desta casa
//! conseguia ABRIR uma conexao TLS -- replica, cluster, DbLink, `phxsqlcmd` e
//! ODBC continuam no Noise. Este modulo e o aperto do cliente, e mora DENTRO
//! do `tls` de proposito: o registro, a transcricao, o `KeyUpdate`, os
//! alertas e o fluxo de aplicacao sao os MESMOS do servidor (`Registro`,
//! `FluxoTls`). Um segundo registro para o cliente seria a decisao de como
//! ler um registro escrita duas vezes -- a que alguem consertasse num lado
//! ficaria torta no outro.
//!
//! # Confianca: pino, nao cadeia
//!
//! Decisao do plano (§4, H3): entre PhxSql e PhxSql a confianca e o PINO --
//! `SHA-256` do SPKI do certificado, a forma do `--pinnedpubkey` do curl e o
//! mesmo contrato do `chave_do_fio` do Noise. Sem pino, o cliente conecta,
//! confere que o servidor TEM a privada do certificado que mostrou, e
//! devolve o pino em [`super::Negociado::pino`] para quem quiser anotar
//! (primeiro contato). Cadeia e nome (RFC 5280/9525) e RSA sao a fatia T6c:
//! servidor com certificado RSA recebe `unsupported_certificate` nomeando o
//! que veio, nunca um «aceito sem conferir» calado.
//!
//! # O que se oferece
//!
//! `TLS_CHACHA20_POLY1305_SHA256` e `TLS_AES_128_GCM_SHA256`; X25519 (a chave
//! que vai no primeiro `ClientHello`) e P-256 (por `HelloRetryRequest`, com
//! eco do `cookie`); assinaturas `ecdsa_secp256r1_sha256` e `ed25519`. Nada
//! de PSK, 0-RTT ou retomada: o `NewSessionTicket` se descarta.
//! `CertificateRequest` se responde com `Certificate` vazio (§4.4.2) -- o
//! servidor decide se aceita cliente sem certificado.

use std::io::{Read, Write};

use super::{
    alerta, conteudo_da_verificacao, de_io, ext, extensao, falha, hs, mensagem, vetor16, vetor24,
    vetor8, Aperto, Falha, FluxoTls, Leitor, Negociado, Registro, Troca, CONJUNTOS,
    ECDSA_SECP256R1_SHA256, GRUPOS, RANDOM_HRR, TLS13, X25519,
};
use crate::error::{PhxError, Result};
use crate::hash::{iguais_em_tempo_constante, sha256};
use crate::tls13::{self, tipo, Conjunto, Protecao, Transcricao, MAX_CLARO, RESUMO};
use crate::x509::ChavePublica;

/// `ed25519` (RFC 8446 §4.2.3).
const ED25519: u16 = 0x0807;

/// As assinaturas que este cliente sabe CONFERIR, e por isso as unicas que
/// anuncia: anunciar RSA sem saber conferir RSA convidaria o servidor a
/// mandar o que so se aceitaria sem conferir.
const ASSINATURAS: [u16; 2] = [ECDSA_SECP256R1_SHA256, ED25519];

/// A maior mensagem `Certificate` aceita. Uma cadeia publica de tres
/// certificados RSA fica em 4-6 KiB; o teto da folga sem deixar o servidor
/// alheio fazer este processo alocar 16 MiB (o campo e de 24 bits) -- a
/// licao do 434 vale para quem le, dos dois lados.
const TETO_CERTIFICADO: usize = 64 * 1024;

/// Como o cliente decide se confia no servidor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confianca {
    /// So o servidor cujo SPKI tem este `SHA-256`.
    Pino([u8; 32]),
    /// Qualquer chave, desde que o servidor prove ter a privada dela; o pino
    /// visto volta em [`Negociado::pino`]. E o primeiro contato do Noise
    /// (TOFU), e o `sslmode=require` do PostgreSQL: cifra contra quem escuta,
    /// nao contra quem se poe no meio.
    AnotarNoPrimeiroContato,
}

/// O que o cliente pede.
#[derive(Clone, Debug)]
pub struct OpcoesCliente<'a> {
    /// O nome do servidor. Vai no SNI quando e nome (IP literal nao vai: a
    /// RFC 6066 §3 proibe).
    pub nome: Option<&'a str>,
    /// Os protocolos de aplicacao oferecidos (ALPN), na ordem da preferencia.
    pub alpn: &'a [&'a [u8]],
    pub confianca: Confianca,
}

/// `sha256//<base64>` -> pino. A forma do `--pinnedpubkey` do curl, para o
/// mesmo pino servir aos dois.
pub fn pino_de_texto(texto: &str) -> Result<[u8; 32]> {
    let Some(b64) = texto.trim().strip_prefix("sha256//") else {
        return Err(PhxError::Esquema(
            "TLS: o pino se escreve sha256//<base64 do SHA-256 do SPKI>".into(),
        ));
    };
    crate::base64::decodificar(b64)?
        .try_into()
        .map_err(|_| PhxError::Esquema("TLS: o pino nao tem 32 bytes".into()))
}

/// O inverso de [`pino_de_texto`].
pub fn pino_em_texto(pino: &[u8; 32]) -> String {
    format!("sha256//{}", crate::base64::codificar(pino))
}

// ------------------------------------------------------------ oferta ----

/// Quem monta os `ClientHello` e sorteia as chaves deles. Em producao e a
/// [`OfertaNova`]; o teste do traco da RFC 8448 injeta o `ClientHello` e as
/// privadas do traco, porque a transcricao so confere byte a byte se o
/// primeiro byte for o mesmo.
pub(super) trait Oferta {
    fn primeiro(&mut self) -> (Vec<u8>, Troca);
    /// Depois do HRR: o grupo pedido (ou `None`, quando o HRR so trouxe o
    /// cookie) e o cookie a ecoar.
    fn segundo(&mut self, grupo: Option<u16>, cookie: Option<&[u8]>) -> (Vec<u8>, Troca);
}

pub(super) struct OfertaNova<'a> {
    op: &'a OpcoesCliente<'a>,
    random: [u8; 32],
    troca: Option<Troca>,
}

impl<'a> OfertaNova<'a> {
    pub(super) fn nova(op: &'a OpcoesCliente<'a>) -> OfertaNova<'a> {
        let mut random = [0u8; 32];
        crate::cifra::sortear(&mut random);
        OfertaNova {
            op,
            random,
            troca: None,
        }
    }
}

impl Oferta for OfertaNova<'_> {
    fn primeiro(&mut self) -> (Vec<u8>, Troca) {
        // Uma chave so, X25519: e o grupo que todo servidor TLS 1.3 fala, e
        // a P-256 vem por HRR quando o servidor a prefere -- mandar as duas
        // custaria um P-256 a cada conexao para servir ao caso raro.
        let troca = Troca::nova(X25519);
        let ch = ola_do_cliente(&self.random, &troca, None, self.op);
        self.troca = Some(troca.clone());
        (ch, troca)
    }

    fn segundo(&mut self, grupo: Option<u16>, cookie: Option<&[u8]>) -> (Vec<u8>, Troca) {
        // §4.1.2: sem key_share no HRR, a chave do segundo e a do primeiro.
        let troca = match (grupo, self.troca.take()) {
            (None, Some(t)) => t,
            (Some(g), _) => Troca::nova(g),
            (None, None) => Troca::nova(X25519),
        };
        let ch = ola_do_cliente(&self.random, &troca, cookie, self.op);
        (ch, troca)
    }
}

/// O `ClientHello` (§4.1.2). O segundo, depois do HRR, e o MESMO com a chave
/// trocada e o cookie ecoado -- por isso uma funcao so monta os dois.
pub(super) fn ola_do_cliente(
    random: &[u8; 32],
    troca: &Troca,
    cookie: Option<&[u8]>,
    op: &OpcoesCliente,
) -> Vec<u8> {
    let mut c = vec![3, 3];
    c.extend_from_slice(random);
    // Sessao vazia: sem o modo de compatibilidade (§D.4), nenhum
    // `change_cipher_spec` precisa ir.
    c.extend_from_slice(&vetor8(&[]));
    let ids: Vec<u8> = CONJUNTOS
        .iter()
        .flat_map(|c| c.id().to_be_bytes())
        .collect();
    c.extend_from_slice(&vetor16(&ids));
    c.extend_from_slice(&vetor8(&[0]));

    let mut e = Vec::new();
    if let Some(n) = op.nome.filter(|n| n.parse::<std::net::IpAddr>().is_err()) {
        let mut nome = vec![0u8];
        nome.extend_from_slice(&vetor16(n.as_bytes()));
        e.extend_from_slice(&extensao(ext::SERVER_NAME, &vetor16(&nome)));
    }
    e.extend_from_slice(&extensao(ext::VERSOES, &vetor8(&TLS13.to_be_bytes())));
    let grupos: Vec<u8> = GRUPOS.iter().flat_map(|g| g.to_be_bytes()).collect();
    e.extend_from_slice(&extensao(ext::GRUPOS, &vetor16(&grupos)));
    let ass: Vec<u8> = ASSINATURAS.iter().flat_map(|a| a.to_be_bytes()).collect();
    e.extend_from_slice(&extensao(ext::ASSINATURAS, &vetor16(&ass)));
    let mut chave = troca.grupo().to_be_bytes().to_vec();
    chave.extend_from_slice(&vetor16(&troca.publica()));
    e.extend_from_slice(&extensao(ext::CHAVES, &vetor16(&chave)));
    if !op.alpn.is_empty() {
        let lista: Vec<u8> = op.alpn.iter().flat_map(|p| vetor8(p)).collect();
        e.extend_from_slice(&extensao(ext::ALPN, &vetor16(&lista)));
    }
    if let Some(k) = cookie {
        e.extend_from_slice(&extensao(ext::COOKIE, &vetor16(k)));
    }
    c.extend_from_slice(&vetor16(&e));
    mensagem(hs::CLIENT_HELLO, &c)
}

// ------------------------------------------------------------ leitura ----

/// O que interessa de um `ServerHello` (ou de um HRR, que tem a mesma forma).
#[derive(Default)]
struct OlaDoServidor {
    random: [u8; 32],
    sessao: Vec<u8>,
    conjunto: u16,
    versao: Option<u16>,
    /// No `ServerHello`: grupo e chave. No HRR: so o grupo (chave vazia).
    chave: Option<(u16, Vec<u8>)>,
    cookie: Option<Vec<u8>>,
}

fn analisar_ola_do_servidor(corpo: &[u8]) -> Aperto<OlaDoServidor> {
    let mut l = Leitor { b: corpo };
    let mut o = OlaDoServidor::default();
    l.u16()?;
    o.random.copy_from_slice(l.bytes(32)?);
    let hrr = o.random == RANDOM_HRR;
    o.sessao = l.vetor8()?.to_vec();
    o.conjunto = l.u16()?;
    if l.u8()? != 0 {
        return falha(alerta::ILLEGAL_PARAMETER, "compressao diferente de nula");
    }
    let mut extensoes = Leitor { b: l.vetor16()? };
    l.fim()?;
    let mut vistas: Vec<u16> = Vec::new();
    while !extensoes.b.is_empty() {
        let t = extensoes.u16()?;
        let mut d = Leitor {
            b: extensoes.vetor16()?,
        };
        if vistas.contains(&t) {
            return falha(alerta::ILLEGAL_PARAMETER, "extensao repetida");
        }
        vistas.push(t);
        match t {
            ext::VERSOES => {
                o.versao = Some(d.u16()?);
                d.fim()?;
            }
            ext::CHAVES if hrr => {
                o.chave = Some((d.u16()?, Vec::new()));
                d.fim()?;
            }
            ext::CHAVES => {
                let g = d.u16()?;
                o.chave = Some((g, d.vetor16()?.to_vec()));
                d.fim()?;
            }
            ext::COOKIE if hrr => {
                let k = d.vetor16()?;
                if k.is_empty() {
                    return falha(alerta::DECODE_ERROR, "cookie vazio");
                }
                o.cookie = Some(k.to_vec());
                d.fim()?;
            }
            // §4.1.3/§4.2: o ServerHello so traz o que o cliente ofereceu, e
            // fora estas tres nada aqui se oferece no ServerHello (PSK nao).
            _ => {
                return falha(
                    alerta::UNSUPPORTED_EXTENSION,
                    "o ServerHello trouxe extensao que nao se ofereceu",
                )
            }
        }
    }
    Ok(o)
}

/// O que vale para o HRR e para o `ServerHello` (§4.1.3, §4.1.4).
fn conferir_ola_do_servidor(o: &OlaDoServidor) -> Aperto<Conjunto> {
    if o.versao != Some(TLS13) {
        return falha(
            alerta::PROTOCOL_VERSION,
            "o servidor nao respondeu em TLS 1.3",
        );
    }
    if !o.sessao.is_empty() {
        return falha(
            alerta::ILLEGAL_PARAMETER,
            "o servidor ecoou uma sessao que nao se mandou",
        );
    }
    match Conjunto::de_id(o.conjunto).filter(|c| CONJUNTOS.contains(c)) {
        Some(c) => Ok(c),
        None => falha(
            alerta::ILLEGAL_PARAMETER,
            "o servidor escolheu um conjunto que nao se ofereceu",
        ),
    }
}

/// Le a lista de extensoes do `EncryptedExtensions` e devolve o ALPN.
fn analisar_extensoes_cifradas(corpo: &[u8], op: &OpcoesCliente) -> Aperto<Option<Vec<u8>>> {
    let mut l = Leitor { b: corpo };
    let mut e = Leitor { b: l.vetor16()? };
    l.fim()?;
    let mut alpn = None;
    let mut vistas: Vec<u16> = Vec::new();
    while !e.b.is_empty() {
        let t = e.u16()?;
        let dados = e.vetor16()?;
        if vistas.contains(&t) {
            return falha(alerta::ILLEGAL_PARAMETER, "extensao repetida");
        }
        vistas.push(t);
        if t == ext::ALPN {
            // RFC 7301 §3.1: UM protocolo, e um dos que se ofereceram.
            let mut d = Leitor { b: dados };
            let mut v = Leitor { b: d.vetor16()? };
            d.fim()?;
            let p = v.vetor8()?.to_vec();
            v.fim()?;
            if op.alpn.is_empty() {
                return falha(
                    alerta::UNSUPPORTED_EXTENSION,
                    "o servidor escolheu ALPN sem ninguem oferecer",
                );
            }
            if !op.alpn.contains(&p.as_slice()) {
                return falha(
                    alerta::ILLEGAL_PARAMETER,
                    "o servidor escolheu um ALPN que nao se ofereceu",
                );
            }
            alpn = Some(p);
        }
        // O resto (o eco do SNI, a lista de grupos do servidor, o
        // record_size_limit do traco da RFC 8448) e informativo e se ignora.
    }
    Ok(alpn)
}

/// A folha do `Certificate` (§4.4.2), em DER. O resto da cadeia so serviria
/// a quem confere cadeia (T6c).
fn folha_do_certificado(corpo: &[u8]) -> Aperto<Vec<u8>> {
    let mut l = Leitor { b: corpo };
    if !l.vetor8()?.is_empty() {
        return falha(
            alerta::ILLEGAL_PARAMETER,
            "certificate_request_context nao vazio no certificado do servidor",
        );
    }
    let n = {
        let b = l.bytes(3)?;
        u32::from_be_bytes([0, b[0], b[1], b[2]]) as usize
    };
    let mut lista = Leitor { b: l.bytes(n)? };
    l.fim()?;
    if lista.b.is_empty() {
        // §4.4.2.4: servidor sem certificado e `decode_error`.
        return falha(alerta::DECODE_ERROR, "o servidor nao mandou certificado");
    }
    let n = {
        let b = lista.bytes(3)?;
        u32::from_be_bytes([0, b[0], b[1], b[2]]) as usize
    };
    let folha = lista.bytes(n)?.to_vec();
    lista.vetor16()?;
    Ok(folha)
}

/// Confere uma assinatura do `CertificateVerify` contra o SPKI da folha.
/// `Err` com o alerta: algoritmo nao suportado e uma coisa, assinatura que
/// nao bate e outra.
pub(super) fn conferir_assinatura(
    alg: u16,
    spki: &[u8],
    conteudo: &[u8],
    assinatura: &[u8],
) -> Aperto<()> {
    let chave = match crate::x509::chave_do_spki(spki) {
        Ok(c) => c,
        Err(e) => return Err(Falha(alerta::BAD_CERTIFICATE, e)),
    };
    if let ChavePublica::Outra(oid) = &chave {
        return Err(Falha(
            alerta::UNSUPPORTED_CERTIFICATE,
            PhxError::Corrompido(format!(
                "TLS: o certificado do servidor tem chave de algoritmo {} \
                 -- este cliente confere P-256 e Ed25519; RSA e P-384 sao \
                 a fatia T6c do pedido 572",
                oid.iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(".")
            )),
        ));
    }
    if !ASSINATURAS.contains(&alg) {
        return falha(
            alerta::ILLEGAL_PARAMETER,
            "o CertificateVerify usou uma assinatura que nao se ofereceu",
        );
    }
    let ok = match (alg, &chave) {
        (ECDSA_SECP256R1_SHA256, ChavePublica::P256(q)) => {
            match crate::x509::ecdsa_de_der(assinatura) {
                Some((r, s)) => crate::p256::verificar(q, conteudo, &r, &s),
                None => false,
            }
        }
        (ED25519, ChavePublica::Ed25519(q)) => match <&[u8; 64]>::try_from(assinatura) {
            Ok(a) => crate::ed25519::conferir(q, conteudo, a),
            Err(_) => false,
        },
        _ => {
            return falha(
                alerta::ILLEGAL_PARAMETER,
                "o algoritmo do CertificateVerify nao casa com a chave do certificado",
            )
        }
    };
    if ok {
        Ok(())
    } else {
        falha(
            alerta::DECRYPT_ERROR,
            "a assinatura do CertificateVerify nao confere com o certificado",
        )
    }
}

// ------------------------------------------------------------ aperto ----

type Conferidor<'a> = &'a dyn Fn(u16, &[u8], &[u8], &[u8]) -> Aperto<()>;

/// A primeira mensagem de aperto depois do ServerHello que nao e de um tipo
/// esperado vira `unexpected_message` com o nome do que se esperava.
fn exigir(m: &[u8], t: u8, nome: &str) -> Aperto<()> {
    if m[0] == t {
        Ok(())
    } else {
        Err(Falha(
            alerta::UNEXPECTED_MESSAGE,
            PhxError::Corrompido(format!("TLS: esperava {nome}, veio o tipo {}", m[0])),
        ))
    }
}

pub(super) fn apertar_cliente<S: Read + Write>(
    r: &mut Registro<S>,
    oferta: &mut dyn Oferta,
    op: &OpcoesCliente,
    conferir: Conferidor,
) -> Aperto<(Negociado, [u8; RESUMO], [u8; RESUMO])> {
    let mut transcricao = Transcricao::default();
    let (ch1, mut troca) = oferta.primeiro();
    // §5.1: o primeiro ClientHello pode ir com a versao de registro 0x0301,
    // para o intermediario velho que recusaria 0x0303 no primeiro byte.
    let mut reg = vec![tipo::HANDSHAKE, 3, 1];
    reg.extend_from_slice(&(ch1.len() as u16).to_be_bytes());
    reg.extend_from_slice(&ch1);
    de_io(r.fluxo.write_all(&reg))?;
    de_io(r.fluxo.flush())?;
    transcricao.acrescentar(&ch1);

    let m = r.mensagem(MAX_CLARO)?;
    exigir(&m, hs::SERVER_HELLO, "ServerHello")?;
    let mut sh = analisar_ola_do_servidor(&m[4..])?;
    let mut conjunto = conferir_ola_do_servidor(&sh)?;
    if sh.random == RANDOM_HRR {
        let grupo = sh.chave.as_ref().map(|(g, _)| *g);
        if let Some(g) = grupo {
            // §4.1.4: grupo que nao se anunciou, ou o da chave que ja foi.
            if !GRUPOS.contains(&g) || g == troca.grupo() {
                return falha(
                    alerta::ILLEGAL_PARAMETER,
                    "o HelloRetryRequest pediu um grupo invalido",
                );
            }
        } else if sh.cookie.is_none() {
            return falha(
                alerta::ILLEGAL_PARAMETER,
                "HelloRetryRequest que nao muda nada",
            );
        }
        r.nada_pendente()?;
        // §4.4.1: a transcricao recomeca pelo resumo do primeiro ClientHello.
        let resumo_ch1 = transcricao.resumo();
        transcricao = Transcricao::default();
        transcricao.acrescentar(&mensagem(hs::MESSAGE_HASH, &resumo_ch1));
        transcricao.acrescentar(&m);
        let (ch2, t2) = oferta.segundo(grupo, sh.cookie.as_deref());
        troca = t2;
        r.escrever(tipo::HANDSHAKE, &ch2)?;
        de_io(r.fluxo.flush())?;
        transcricao.acrescentar(&ch2);

        let m2 = r.mensagem(MAX_CLARO)?;
        exigir(&m2, hs::SERVER_HELLO, "ServerHello")?;
        let sh2 = analisar_ola_do_servidor(&m2[4..])?;
        if sh2.random == RANDOM_HRR {
            return falha(alerta::UNEXPECTED_MESSAGE, "segundo HelloRetryRequest");
        }
        if conferir_ola_do_servidor(&sh2)? != conjunto {
            return falha(
                alerta::ILLEGAL_PARAMETER,
                "o ServerHello mudou o conjunto do HelloRetryRequest",
            );
        }
        transcricao.acrescentar(&m2);
        sh = sh2;
    } else {
        transcricao.acrescentar(&m);
    }
    conjunto = Conjunto::de_id(sh.conjunto).unwrap_or(conjunto);
    let Some((grupo, chave_srv)) = sh.chave.take() else {
        return falha(alerta::MISSING_EXTENSION, "ServerHello sem key_share");
    };
    if grupo != troca.grupo() {
        return falha(
            alerta::ILLEGAL_PARAMETER,
            "o ServerHello respondeu num grupo de que nao se mandou chave",
        );
    }
    r.nada_pendente()?;
    let compartilhado = troca.segredo(&chave_srv)?;

    let segredo_hs = tls13::segredo_handshake(&tls13::segredo_early(), &compartilhado);
    let t_sh = transcricao.resumo();
    let c_hs = tls13::derivar_segredo(&segredo_hs, "c hs traffic", &t_sh);
    let s_hs = tls13::derivar_segredo(&segredo_hs, "s hs traffic", &t_sh);
    r.recebimento = Some(Protecao::de_segredo_com(&s_hs, conjunto));
    r.envio = Some(Protecao::de_segredo_com(&c_hs, conjunto));

    let m = r.mensagem(MAX_CLARO)?;
    exigir(&m, hs::EXTENSOES, "EncryptedExtensions")?;
    let alpn = analisar_extensoes_cifradas(&m[4..], op)?;
    transcricao.acrescentar(&m);

    let mut m = r.mensagem(TETO_CERTIFICADO)?;
    let mut pedido_de_certificado: Option<Vec<u8>> = None;
    if m[0] == hs::PEDIDO_DE_CERTIFICADO {
        let mut l = Leitor { b: &m[4..] };
        pedido_de_certificado = Some(l.vetor8()?.to_vec());
        l.vetor16()?;
        l.fim()?;
        transcricao.acrescentar(&m);
        m = r.mensagem(TETO_CERTIFICADO)?;
    }
    exigir(&m, hs::CERTIFICADO, "Certificate")?;
    let folha = folha_do_certificado(&m[4..])?;
    let spki = match crate::x509::spki_do_certificado(&folha) {
        Ok(s) => s.to_vec(),
        Err(e) => return Err(Falha(alerta::BAD_CERTIFICATE, e)),
    };
    let pino = sha256(&spki);
    if let Confianca::Pino(esperado) = op.confianca {
        if !iguais_em_tempo_constante(&pino, &esperado) {
            return Err(Falha(
                alerta::BAD_CERTIFICATE,
                PhxError::Autorizacao(format!(
                    "TLS: a chave do servidor nao e a do pino (veio {}) -- \
                     ou a chave do servidor mudou, ou ha alguem no meio",
                    pino_em_texto(&pino)
                )),
            ));
        }
    }
    transcricao.acrescentar(&m);

    let m = r.mensagem(TETO_CERTIFICADO)?;
    exigir(&m, hs::VERIFICACAO, "CertificateVerify")?;
    let mut l = Leitor { b: &m[4..] };
    let alg = l.u16()?;
    let assinatura = l.vetor16()?;
    l.fim()?;
    // Se o algoritmo foi oferecido, quem sabe e o conferidor: o oferecido e
    // o que ele sabe conferir.
    conferir(
        alg,
        &spki,
        &conteudo_da_verificacao(&transcricao.resumo()),
        assinatura,
    )?;
    transcricao.acrescentar(&m);

    let m = r.mensagem(super::TETO_MENSAGEM)?;
    exigir(&m, hs::FINISHED, "Finished")?;
    let esperado = tls13::verify_data(&s_hs, &transcricao.resumo());
    if !iguais_em_tempo_constante(&m[4..], &esperado) {
        return falha(alerta::DECRYPT_ERROR, "o Finished do servidor nao confere");
    }
    transcricao.acrescentar(&m);
    // §5.1: o Finished do servidor fecha a chave do aperto dele.
    r.nada_pendente()?;

    let t_sf = transcricao.resumo();
    let master = tls13::segredo_master(&segredo_hs);
    let c_ap = tls13::derivar_segredo(&master, "c ap traffic", &t_sf);
    let s_ap = tls13::derivar_segredo(&master, "s ap traffic", &t_sf);

    let mut voo = Vec::new();
    if let Some(contexto) = pedido_de_certificado {
        // §4.4.2: sem certificado de cliente, a lista vai vazia e o
        // CertificateVerify nao vai.
        let mut corpo = vetor8(&contexto);
        corpo.extend_from_slice(&vetor24(&[]));
        let c = mensagem(hs::CERTIFICADO, &corpo);
        transcricao.acrescentar(&c);
        voo.extend_from_slice(&c);
    }
    let fin = mensagem(
        hs::FINISHED,
        &tls13::verify_data(&c_hs, &transcricao.resumo()),
    );
    voo.extend_from_slice(&fin);
    r.escrever(tipo::HANDSHAKE, &voo)?;
    de_io(r.fluxo.flush())?;

    let negociado = Negociado {
        alpn,
        nome: op.nome.map(str::to_string),
        grupo,
        conjunto,
        pino: Some(pino),
    };
    Ok((negociado, c_ap, s_ap))
}

pub(super) fn conectar_com<S: Read + Write>(
    fluxo: S,
    op: &OpcoesCliente,
    oferta: &mut dyn Oferta,
    conferir: Conferidor,
) -> Result<FluxoTls<S>> {
    let mut r = Registro {
        fluxo,
        envio: None,
        recebimento: None,
        pendente: Vec::new(),
    };
    match apertar_cliente(&mut r, oferta, op, conferir) {
        Ok((negociado, c_ap, s_ap)) => Ok(FluxoTls {
            r: Registro {
                fluxo: r.fluxo,
                envio: Some(Protecao::de_segredo_com(&c_ap, negociado.conjunto)),
                recebimento: Some(Protecao::de_segredo_com(&s_ap, negociado.conjunto)),
                pendente: Vec::new(),
            },
            segredo_envio: c_ap,
            segredo_recebimento: s_ap,
            claro: Vec::new(),
            pos: 0,
            fechado: false,
            cliente: true,
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

/// Faz o aperto de mao do CLIENTE sobre `fluxo` e devolve o fluxo protegido
/// -- o mesmo [`FluxoTls`] que o servidor devolve. Em falha, o servidor
/// recebe o alerta da §6 antes do erro voltar.
pub fn conectar<S: Read + Write>(fluxo: S, op: &OpcoesCliente) -> Result<FluxoTls<S>> {
    let mut oferta = OfertaNova::nova(op);
    conectar_com(fluxo, op, &mut oferta, &conferir_assinatura)
}

#[cfg(test)]
mod testes {
    // Provas do cliente TLS 1.3: os tracos das secoes 3 e 5 da RFC 8448 byte a
    // byte, e o fio de verdade contra o servidor desta casa e o `openssl
    // s_server`.

    use super::*;
    use crate::tls::SECP256R1;

    // Extraidas do texto oficial da RFC 8448 (o mesmo arquivo do T2) por script,
    // com o tamanho de cada uma conferido contra o declarado ("complete record
    // (N octets)"). R3 = secao 3 (1-RTT simples), R5 = secao 5 (HRR).
    const R3_CH: &str = "16030100c4010000c00303cb34ecb1e78163ba1c38c6dacb196a6dffa21a8d9912ec18a2ef6283024dece7000006130113031302010000910000000b0009000006736572766572ff01000100000a00140012001d0017001800190100010101020103010400230000003300260024001d002099381de560e4bd43d23d8e435a7dbafeb3c06e51c13cae4d5413691e529aaf2c002b0003020304000d0020001e040305030603020308040805080604010501060102010402050206020202002d00020101001c00024001";
    const R3_SH: &str = "160303005a020000560303a6af06a4121860dc5e6e60249cd34c95930c8ac5cb1434dac155772ed3e2692800130100002e00330024001d0020c9828876112095fe66762bdbf7c672e156d6cc253b833df1dd69b1b04e751f0f002b00020304";
    const R3_CFIN: &str = "170303003575ec4dc238cce60b298044a71e219c56cc77b0517fe9b93c7a4bfc44d87f38f80338ac98fc46deb384bd1caeacab6867d726c40546";
    const R3_NST: &str = "17030300de3a6b8f90414a97d6959c3487680de5134a2b240e6cffac116e95d41d6af8f6b580dcf3d11d63c758db289a015940252f55713e061dc13e078891a38efbcf5753ad8ef170ad3c7353d16d9da773b9ca7f2b9fa1b6c0d4a3d03f75e09c30ba1e62972ac46f75f7b981be63439b2999ce13064615139891d5e4c5b406f16e3fc181a77ca475840025db2f0a77f81b5ab05b94c01346755f69232c86519d86cbeeac87aac347d143f9605d64f650db4d023e70e952ca49fe5137121c74bc2697687e248746d6df353005f3bce18696129c8153556b3b6c6779b37bf15985684f";
    const R3_CDADOS: &str = "1703030043a23f7054b62c94d0affafe8228ba55cbefacea42f914aa66bcab3f2b9819a8a5b46b395bd54a9a20441e2b62974e1f5a6292a2977014bd1e3deae63aeebb21694915e4";
    const R3_CALERTA: &str = "1703030013c9872760655666b74d7ff1153efd6db6d0b0e3";
    const R5_CH1: &str = "16030100b4010000b00303b0b1c5a5aa37c5919f2ed1d5c6fff7fcb7849716945a2b8cee9258a346677b6f000006130113031302010000810000000b0009000006736572766572ff01000100000a00080006001d00170018003300260024001d0020e8e8e3f3b93a25ed97a14a7dcacb8a272c6288e585c6484d05262fcad062ad1f002b0003020304000d0020001e040305030603020308040805080604010501060102010402050206020202002d00020101001c00024001";
    const R5_HRR: &str = "16030300b0020000ac0303cf21ad74e59a6111be1d8c021e65b891c2a211167abb8c5e079e09e2c8a8339c001301000084003300020017002c0074007271dcd04bb88bc3189119398a00000000eefafc76c146b823b096f8aacad365dd0030953f4edf625636e5f21bb2e23fcc654b1b5b40318d10d137abcbb87574e36e8a1f025f7dfa5d6e50781b5eda4aa15b0c8be778257d16aa3030e9e7841dd9e4c0342267e8ca0caf571fb2b7cff0f934b0002b00020304";
    const R5_CH2: &str = "1603030200010001fc0303b0b1c5a5aa37c5919f2ed1d5c6fff7fcb7849716945a2b8cee9258a346677b6f000006130113031302010001cd0000000b0009000006736572766572ff01000100000a00080006001d001700180033004700450017004104a6da7392ec591e17abfd535964b99894d13befb221b3def2ebe3830eac8f0151812677c4d6d2237e85cf01d6910cfb83954e76ba7352830534159897e8065780002b0003020304000d0020001e040305030603020308040805080604010501060102010402050206020202002c0074007271dcd04bb88bc3189119398a00000000eefafc76c146b823b096f8aacad365dd0030953f4edf625636e5f21bb2e23fcc654b1b5b40318d10d137abcbb87574e36e8a1f025f7dfa5d6e50781b5eda4aa15b0c8be778257d16aa3030e9e7841dd9e4c0342267e8ca0caf571fb2b7cff0f934b0002d00020101001c00024001001500af00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    const R5_SH: &str = "160303007b020000770303bb341d847fd789c47c387172dc0c9bf147fccacb5043d86ca4c598d3ff571b9800130100004f003300450017004104583e054b7a66672ae020ad9d2686fcc85b5ad41a134a0f03ee72b893052bd85b4c8de6776f5b04ac07d83540eab3e3d9c547bc6528c4317d294686093a6cad7d002b00020304";
    const R5_VOO: &str = "170303029699bee20baf5b7fc727bfab6223928a381e6d0cf9c4da653f9d2a7b23f7de11cce842d5cf75631763450ffb8b0cc1d238e658af7a12adc86243114ab14a1da2fae42621ce483fb6242eabfaad52566b02b31d2eddedefeb80e66a9900d5f973b40c4fdf74719ecf1b68d7f9c3b6ceb903ca13dd1bb8f8187ae33417e1d152522c5822a1a03ad52c838c55953d610222874cce8e1790b229a2aa0b53c8d377ee720182951dc6181dc5d90bd1f0105ed1e84aa5f75957c6661897079e5ea5007449e3197bdc7c9beeedddeafdd844afa5c315ecfe65e576afe909812880620ec7048b42d7f5c78d76f299d6d82534bdd8f512febc0ed3814aca470cd8000d3e1cb9962b052fbb950df683a52c2ba77ed3713b122937a6e5170964e2ab7969dcd980b3db9b458da7603124d6dc005e4d6e04b4d0c4baf3275db827dbba0a6db09672171fc057b3851d7e026841e2978fbd2346bbefdd0376bb1108fe9acc92189f5650aa5e85d8e8c7b67ac510dba003d3d7e16350bb66d45013efd44c9b607c0d318c4c7d1a1f5cbc57e20611804e3787d7b4a4b5f08ed8fd70bdaeade02260b12ab842ef690b4a3ee7911e841b374ecd5ebbbc2a54d047b600336dd7d0c88b4bc10e58ee6cb656de7247fa20d8e91deb84628608cf80615b62e96c1491c7ac3755eb6901405d3474fe1ac79d106a0cee56c2577fc88480f96cb6b8c681b7b68b53c146093908f350888175bdfb0b1e31ad61e30ba0adfe6d223aa03c0783b5001a57587c328a9afcfcfb978d1cd4328f7d9d60530e630befd96c0c816ee20b0100768ae2a6df51fc68f172740a79af11398ee3be1252491fa9c693479e877f94ab7c5f8cad480203e6ab7b87dd71e8a0729113df17f5eee86ce108d1d72007ec1cd13c85a6c149621e77b7d78d805a30f0be030c315e54";
    const R5_CFIN: &str = "1703030035d74f1923c662fd34137c6f502f3dd2b93d951d1b3bc97e42afe23c31abea92fe91b474999e85e3b791ce252fe8c3e9f939a4120cb2";
    const R5_X_PRIV: &str = "0ed02f8e8117efc75ca7ac32aa7e34eda64cdc0ddad154a5e85289f959f63204";
    const R5_P_PRIV: &str = "ab5473467e19346ceb0a0414e41da21d4d2445bc3025afe97c4e8dc8d513da39";
    const R5_P_PUB: &str = "04a6da7392ec591e17abfd535964b99894d13befb221b3def2ebe3830eac8f0151812677c4d6d2237e85cf01d6910cfb83954e76ba7352830534159897e8065780";
    const R3_X_PRIV: &str = "49af42ba7f7994852d713ef2784bcbcaa7911de26adc5642cb634540e7ea5005";
    const R3_SRV_PUB: &str = "c9828876112095fe66762bdbf7c672e156d6cc253b833df1dd69b1b04e751f0f";
    const R3_VOO: &str = "17030302a2d1ff334a56f5bff6594a07cc87b580233f500f45e489e7f33af35edf7869fcf40aa40aa2b8ea73f848a7ca07612ef9f945cb960b4068905123ea78b111b429ba9191cd05d2a389280f526134aadc7fc78c4b729df828b5ecf7b13bd9aefb0e57f271585b8ea9bb355c7c79020716cfb9b1183ef3ab20e37d57a6b9d7477609aee6e122a4cf51427325250c7d0e509289444c9b3a648f1d71035d2ed65b0e3cdd0cbae8bf2d0b227812cbb360987255cc744110c453baa4fcd610928d809810e4b7ed1a8fd991f06aa6248204797e36a6a73b70a2559c09ead686945ba246ab66e5edd8044b4c6de3fcf2a89441ac66272fd8fb330ef8190579b3684596c960bd596eea520a56a8d650f563aad27409960dca63d3e688611ea5e22f4415cf9538d51a200c27034272968a264ed6540c84838d89f72c24461aad6d26f59ecaba9acbbb317b66d902f4f292a36ac1b639c637ce343117b659622245317b49eeda0c6258f100d7d961ffb138647e92ea330faeea6dfa31c7a84dc3bd7e1b7a6c7178af36879018e3f252107f243d243dc7339d5684c8b0378bf30244da8c87c843f5e56eb4c5e8280a2b48052cf93b16499a66db7cca71e4599426f7d461e66f99882bd89fc50800becca62d6c74116dbd2972fda1fa80f85df881edbe5a37668936b335583b599186dc5c6918a396fa48a181d6b6fa4f9d62d513afbb992f2b992f67f8afe67f76913fa388cb5630c8ca01e0c65d11c66a1e2ac4c85977b7c7a6999bbf10dc35ae69f5515614636c0b9b68c19ed2e31c0b3b66763038ebba42f3b38edc0399f3a9f23faa63978c317fc9fa66a73f60f0504de93b5b845e275592c12335ee340bbc4fddd502784016e4b3be7ef04dda49f4b440a30cb5d2af939828fd4ae3794e44f94df5a631ede42c1719bfdabf0253fe5175be898e750edc53370d2b";
    const R3_SDADOS: &str = "17030300432e937e11ef4ac740e538ad36005fc4a46932fc3225d05f82aa1b36e30efaf97d90e6dffc602dcb501a59a8fcc49c4bf2e5f0a21c0047c2abf332540dd032e167c2955d";
    const R3_SALERTA: &str = "1703030013b58fd67166ebf599d24720cfbe7efa7a8864a9";

    fn b(h: &str) -> Vec<u8> {
        crate::hash::de_hex(h).unwrap()
    }

    fn a32(h: &str) -> [u8; 32] {
        b(h).try_into().unwrap()
    }

    /// Um fio de mentira: entrega o que o servidor do traco mandou e guarda o
    /// que o cliente escreveu.
    struct Roteiro {
        entrada: std::io::Cursor<Vec<u8>>,
        saida: Vec<u8>,
    }

    impl Read for Roteiro {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.entrada.read(buf)
        }
    }

    impl Write for Roteiro {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.saida.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn roteiro(registros: &[&str]) -> Roteiro {
        Roteiro {
            entrada: std::io::Cursor::new(registros.iter().flat_map(|h| b(h)).collect()),
            saida: Vec::new(),
        }
    }

    /// A oferta do traco: o `ClientHello` e as privadas que a RFC publica. O
    /// segundo guarda o que o cliente pediu, para o teste conferir o eco.
    struct OfertaDoTraco {
        ch1: Vec<u8>,
        troca1: Troca,
        ch2: Option<(Vec<u8>, Troca)>,
        pedido: Option<(Option<u16>, Option<Vec<u8>>)>,
    }

    impl Oferta for OfertaDoTraco {
        fn primeiro(&mut self) -> (Vec<u8>, Troca) {
            (self.ch1.clone(), self.troca1.clone())
        }
        fn segundo(&mut self, grupo: Option<u16>, cookie: Option<&[u8]>) -> (Vec<u8>, Troca) {
            self.pedido = Some((grupo, cookie.map(<[u8]>::to_vec)));
            self.ch2
                .take()
                .expect("o traco nao tem segundo ClientHello")
        }
    }

    fn oferta_da_secao_3() -> OfertaDoTraco {
        OfertaDoTraco {
            ch1: b(R3_CH)[5..].to_vec(),
            troca1: Troca::X25519(a32(R3_X_PRIV)),
            ch2: None,
            pedido: None,
        }
    }

    /// O servidor do traco assina com RSA-PSS (0x0804), que esta fatia nao
    /// confere (T6c). O conferidor de mentira so aceita ESSE algoritmo e ESSE
    /// conteudo -- o resto do aperto (transcricao, chaves, Finished) e o que o
    /// traco prova.
    fn conferidor_do_traco(alg: u16, _spki: &[u8], conteudo: &[u8], _a: &[u8]) -> Aperto<()> {
        assert_eq!(alg, 0x0804, "o traco assina com rsa_pss_rsae_sha256");
        assert_eq!(&conteudo[..64], &[0x20u8; 64][..]);
        assert_eq!(&conteudo[64..98], b"TLS 1.3, server CertificateVerify\0");
        Ok(())
    }

    fn op(confianca: Confianca) -> OpcoesCliente<'static> {
        OpcoesCliente {
            nome: Some("server"),
            alpn: &[],
            confianca,
        }
    }

    /// O fluxo do cliente sobre o traco da §3, com o servidor mandando `voo`
    /// no lugar do voo cifrado oficial.
    fn cliente_da_secao_3(
        voo: &str,
        confianca: Confianca,
    ) -> (Result<FluxoTls<Roteiro>>, OfertaDoTraco) {
        let mut oferta = oferta_da_secao_3();
        let fio = roteiro(&[R3_SH, voo, R3_NST, R3_SDADOS, R3_SALERTA]);
        let r = conectar_com(fio, &op(confianca), &mut oferta, &conferidor_do_traco);
        (r, oferta)
    }

    #[test]
    fn o_traco_da_secao_3_da_rfc_8448_byte_a_byte() {
        let (r, _) = cliente_da_secao_3(R3_VOO, Confianca::AnotarNoPrimeiroContato);
        let mut t = r.expect("o cliente recusou o traco oficial");
        // O que o cliente pos no fio: o ClientHello e o Finished cifrado,
        // os dois registros exatamente como a RFC os imprime.
        let mut esperado = b(R3_CH);
        esperado.extend_from_slice(&b(R3_CFIN));
        assert_eq!(
            t.r.fluxo.saida, esperado,
            "o voo do cliente nao e o do traco"
        );
        assert_eq!(t.negociado().conjunto, Conjunto::Aes128GcmSha256);
        assert_eq!(t.negociado().grupo, X25519);

        // Dado nos dois sentidos, sob a chave de aplicacao: o NewSessionTicket
        // do servidor (o primeiro registro dele) se descarta no caminho.
        t.r.fluxo.saida.clear();
        let payload: Vec<u8> = (0u8..50).collect();
        t.write_all(&payload).unwrap();
        assert_eq!(
            t.r.fluxo.saida,
            b(R3_CDADOS),
            "o registro de dado do cliente"
        );
        let mut lido = vec![0u8; 50];
        t.read_exact(&mut lido)
            .expect("o ticket derrubou a leitura");
        assert_eq!(lido, payload);
        t.r.fluxo.saida.clear();
        t.despedir().unwrap();
        assert_eq!(t.r.fluxo.saida, b(R3_CALERTA), "o close_notify do cliente");
        // E o close_notify do servidor encerra a leitura sem erro.
        assert_eq!(t.read(&mut lido).unwrap(), 0);
    }

    #[test]
    fn o_pino_certo_passa_e_o_errado_recusa_com_bad_certificate() {
        let (r, _) = cliente_da_secao_3(R3_VOO, Confianca::AnotarNoPrimeiroContato);
        let pino = r
            .unwrap()
            .negociado()
            .pino
            .expect("o cliente nao anotou o pino");

        let (r, _) = cliente_da_secao_3(R3_VOO, Confianca::Pino(pino));
        r.expect("o pino certo foi recusado");

        let mut torto = pino;
        torto[31] ^= 1;
        let (r, _) = cliente_da_secao_3(R3_VOO, Confianca::Pino(torto));
        let e = r.expect_err("o pino errado passou").to_string();
        assert!(e.contains("nao e a do pino"), "{e}");
        assert!(
            e.contains(&pino_em_texto(&pino)),
            "a recusa nao diz o que veio: {e}"
        );
    }

    #[test]
    fn o_pino_vai_e_volta_pelo_texto_do_curl() {
        let p = [0xA5u8; 32];
        let t = pino_em_texto(&p);
        assert!(t.starts_with("sha256//"));
        assert_eq!(pino_de_texto(&t).unwrap(), p);
        assert!(pino_de_texto("sha1//AAAA").is_err());
        assert!(pino_de_texto("sha256//AAAA").is_err(), "pino curto passou");
    }

    /// O voo cifrado oficial, aberto com a chave do aperto do proprio traco,
    /// com `mexer` aplicado ao claro, e selado de novo.
    fn voo_adulterado(mexer: impl Fn(&mut Vec<u8>)) -> String {
        let comp = crate::x25519::segredo(&a32(R3_X_PRIV), &a32(R3_SRV_PUB)).unwrap();
        let hs = tls13::segredo_handshake(&tls13::segredo_early(), &comp);
        let mut tr = Transcricao::default();
        tr.acrescentar(&b(R3_CH)[5..]);
        tr.acrescentar(&b(R3_SH)[5..]);
        let s_hs = tls13::derivar_segredo(&hs, "s hs traffic", &tr.resumo());
        let reg = b(R3_VOO);
        let cab: [u8; 5] = reg[..5].try_into().unwrap();
        let (t, mut claro) = Protecao::de_segredo_com(&s_hs, Conjunto::Aes128GcmSha256)
            .abrir(&cab, &reg[5..])
            .unwrap();
        mexer(&mut claro);
        let novo = Protecao::de_segredo_com(&s_hs, Conjunto::Aes128GcmSha256)
            .selar(t, &claro)
            .unwrap();
        novo.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn finished_do_servidor_adulterado_recusa() {
        // Controle: abrir e selar de novo sem mexer reproduz o voo oficial.
        assert_eq!(voo_adulterado(|_| {}), R3_VOO);
        let voo = voo_adulterado(|c| {
            let n = c.len();
            c[n - 1] ^= 1;
        });
        let (r, _) = cliente_da_secao_3(&voo, Confianca::AnotarNoPrimeiroContato);
        let e = r.expect_err("aceitou o Finished adulterado").to_string();
        assert!(e.contains("o Finished do servidor nao confere"), "{e}");
    }

    #[test]
    fn o_certificado_rsa_se_recusa_nomeando_o_algoritmo() {
        // Pelo conferidor de PRODUCAO: o traco assina com RSA, e esta fatia nao
        // confere RSA -- a recusa diz o que veio, em vez de aceitar sem conferir.
        let mut oferta = oferta_da_secao_3();
        let fio = roteiro(&[R3_SH, R3_VOO]);
        let r = conectar_com(
            fio,
            &op(Confianca::AnotarNoPrimeiroContato),
            &mut oferta,
            &conferir_assinatura,
        );
        let e = r.expect_err("aceitou RSA sem conferir").to_string();
        assert!(e.contains("1.2.840.113549.1.1.1"), "{e}");
    }

    #[test]
    fn o_hrr_da_secao_5_com_eco_do_cookie_byte_a_byte() {
        let mut oferta = OfertaDoTraco {
            ch1: b(R5_CH1)[5..].to_vec(),
            troca1: Troca::X25519(a32(R5_X_PRIV)),
            ch2: Some((b(R5_CH2)[5..].to_vec(), Troca::P256(a32(R5_P_PRIV)))),
            pedido: None,
        };
        assert_eq!(
            crate::p256::chave_publica(&a32(R5_P_PRIV))
                .unwrap()
                .to_vec(),
            b(R5_P_PUB)
        );
        let fio = roteiro(&[R5_HRR, R5_SH, R5_VOO]);
        let t = conectar_com(
            fio,
            &op(Confianca::AnotarNoPrimeiroContato),
            &mut oferta,
            &conferidor_do_traco,
        )
        .expect("o cliente recusou o traco do HRR");
        let mut esperado = b(R5_CH1);
        esperado.extend_from_slice(&b(R5_CH2));
        esperado.extend_from_slice(&b(R5_CFIN));
        assert_eq!(
            t.r.fluxo.saida, esperado,
            "o voo do cliente nao e o do traco"
        );
        assert_eq!(t.negociado().grupo, SECP256R1);

        // O que o cliente pediu a oferta: P-256 e o cookie do HRR.
        let (grupo, cookie) = oferta.pedido.expect("o segundo ClientHello nao foi pedido");
        assert_eq!(grupo, Some(SECP256R1));
        let cookie = cookie.expect("o cookie do HRR nao chegou a oferta");
        let eco = extensao(ext::COOKIE, &vetor16(&cookie));
        let ch2 = b(R5_CH2);
        assert!(
            ch2.windows(eco.len()).any(|w| w == eco),
            "o traco nao ecoa assim"
        );
        // E o montador de PRODUCAO ecoa do mesmo jeito.
        let o = op(Confianca::AnotarNoPrimeiroContato);
        let meu = ola_do_cliente(&[0; 32], &Troca::P256(a32(R5_P_PRIV)), Some(&cookie), &o);
        assert!(
            meu.windows(eco.len()).any(|w| w == eco),
            "o montador nao ecoa o cookie"
        );
    }

    // ------------------------------------------------------ fio de verdade ----

    use std::net::{TcpListener, TcpStream};
    use std::process::{Child, Command, Stdio};
    use std::time::Duration;

    /// Servidor desta casa, de UMA conexao: devolve o que leu em maiusculas.
    fn servidor_eco(id: super::super::Identidade) -> (u16, std::thread::JoinHandle<Result<()>>) {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (fio, _) = ouvinte.accept()?;
            fio.set_read_timeout(Some(Duration::from_secs(20)))?;
            let mut t = super::super::aceitar(fio, &id, &[b"phx"])?;
            let mut b = [0u8; 64];
            let n = t.read(&mut b)?;
            t.write_all(&b[..n].to_ascii_uppercase())?;
            t.despedir()?;
            Ok(())
        });
        (porta, h)
    }

    fn fio(porta: u16) -> TcpStream {
        let f = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        f.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        f
    }

    #[test]
    fn conversa_com_o_servidor_desta_casa_pelo_pino() {
        let id = super::super::Identidade::autoassinada(&["localhost"]).unwrap();
        let pino = id.pino().unwrap();
        let (porta, h) = servidor_eco(id);
        let op = OpcoesCliente {
            nome: Some("localhost"),
            alpn: &[b"h2", b"phx"],
            confianca: Confianca::Pino(pino),
        };
        let mut t = conectar(fio(porta), &op).expect("o aperto com o servidor desta casa falhou");
        assert_eq!(t.negociado().alpn.as_deref(), Some(&b"phx"[..]));
        assert_eq!(t.negociado().conjunto, Conjunto::Chacha20Poly1305Sha256);
        t.write_all(b"ola, servidor").unwrap();
        let mut s = String::new();
        t.read_to_string(&mut s).unwrap();
        assert_eq!(s, "OLA, SERVIDOR");
        h.join().unwrap().expect("o servidor falhou");
    }

    #[test]
    fn certificado_de_uma_chave_assinado_por_outra_recusa() {
        // O servidor mostra o certificado de A e assina o aperto com a privada
        // de B: so a conferencia do CertificateVerify pega -- sem pino, nada
        // mais no aperto depende da chave do certificado.
        let a = super::super::Identidade::autoassinada(&["localhost"]).unwrap();
        let b_ = crate::p256::gerar_privada();
        let id = super::super::Identidade::nova(vec![a.certificado().to_vec()], b_).unwrap();
        let (porta, h) = servidor_eco(id);
        let op = OpcoesCliente {
            nome: Some("localhost"),
            alpn: &[],
            confianca: Confianca::AnotarNoPrimeiroContato,
        };
        let e = conectar(fio(porta), &op)
            .expect_err("aceitou a chave trocada")
            .to_string();
        assert!(e.contains("nao confere com o certificado"), "{e}");
        let es = h
            .join()
            .unwrap()
            .expect_err("o servidor nao viu a recusa")
            .to_string();
        assert!(
            es.contains("alerta 51"),
            "o servidor nao recebeu decrypt_error: {es}"
        );
    }

    /// `openssl s_server -www` numa porta livre, com o certificado e a chave
    /// dados; espera a porta abrir.
    struct SServer(Child, u16);

    impl Drop for SServer {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn s_server(cert: &std::path::Path, chave: &std::path::Path, extra: &[&str]) -> SServer {
        let porta = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let filho = Command::new("openssl")
            .args(["s_server", "-www", "-tls1_3", "-accept"])
            .arg(porta.to_string())
            .arg("-cert")
            .arg(cert)
            .arg("-key")
            .arg(chave)
            .args(extra)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let s = SServer(filho, porta);
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", porta)).is_ok() {
                return s;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("o openssl s_server nao abriu a porta {porta}");
    }

    use crate::tls::testes::Dir;

    fn dir(nome: &str) -> Dir {
        Dir::novo(&format!("cli-{nome}"))
    }

    /// Uma identidade P-256 desta casa em PEM, para o `openssl s_server`.
    fn identidade_p256(d: &Dir) -> (std::path::PathBuf, std::path::PathBuf, [u8; 32]) {
        let id = super::super::Identidade::autoassinada(&["localhost"]).unwrap();
        let cert = d.0.join("cert.pem");
        let chave = d.0.join("chave.pem");
        std::fs::write(
            &cert,
            crate::x509::para_pem(id.certificado(), "CERTIFICATE"),
        )
        .unwrap();
        std::fs::write(&chave, crate::x509::pem_da_chave_p256(&id.privada).unwrap()).unwrap();
        (cert, chave, id.pino().unwrap())
    }

    /// Conecta pelo pino, pede a pagina do `-www` e devolve o texto e o que se
    /// negociou.
    fn pedir_ao_s_server(porta: u16, pino: [u8; 32]) -> Result<(String, Negociado)> {
        let op = OpcoesCliente {
            nome: Some("localhost"),
            alpn: &[],
            confianca: Confianca::Pino(pino),
        };
        let mut t = conectar(fio(porta), &op)?;
        t.write_all(b"GET / HTTP/1.0\r\n\r\n")?;
        let mut v = Vec::new();
        t.read_to_end(&mut v)?;
        Ok((
            String::from_utf8_lossy(&v).into_owned(),
            t.negociado().clone(),
        ))
    }

    #[test]
    fn com_o_openssl_s_server_em_x25519_e_chacha() {
        let d = dir("x25519");
        let (cert, chave, pino) = identidade_p256(&d);
        let s = s_server(&cert, &chave, &[]);
        let (pagina, neg) = pedir_ao_s_server(s.1, pino).expect("o aperto com o openssl falhou");
        // O -www manda dois NewSessionTicket antes da pagina: chegar aqui prova
        // que o cliente os descarta.
        assert!(pagina.contains("HTTP/1.0 200 ok"), "{pagina}");
        assert!(pagina.contains("TLSv1.3"), "{pagina}");
        assert_eq!(neg.grupo, X25519);
        assert_eq!(neg.conjunto, Conjunto::Chacha20Poly1305Sha256);
    }

    #[test]
    fn com_o_openssl_s_server_so_de_p256_passa_pelo_hrr() {
        let d = dir("hrr");
        let (cert, chave, pino) = identidade_p256(&d);
        let s = s_server(&cert, &chave, &["-groups", "P-256"]);
        let (pagina, neg) = pedir_ao_s_server(s.1, pino).expect("o HRR do openssl falhou");
        assert!(pagina.contains("HTTP/1.0 200 ok"), "{pagina}");
        assert_eq!(neg.grupo, SECP256R1);
    }

    #[test]
    fn com_o_openssl_s_server_so_de_aes_128_gcm() {
        let d = dir("aes");
        let (cert, chave, pino) = identidade_p256(&d);
        let s = s_server(&cert, &chave, &["-ciphersuites", "TLS_AES_128_GCM_SHA256"]);
        let (pagina, neg) = pedir_ao_s_server(s.1, pino).expect("o AES com o openssl falhou");
        assert!(pagina.contains("TLS_AES_128_GCM_SHA256"), "{pagina}");
        assert_eq!(neg.conjunto, Conjunto::Aes128GcmSha256);
    }

    #[test]
    fn com_o_openssl_s_server_pedindo_certificado_do_cliente() {
        // `-verify 1` pede certificado sem exigir: o cliente responde a lista
        // vazia (§4.4.2) e o servidor segue.
        let d = dir("pedido");
        let (cert, chave, pino) = identidade_p256(&d);
        let s = s_server(&cert, &chave, &["-verify", "1"]);
        let (pagina, _) = pedir_ao_s_server(s.1, pino).expect("o pedido de certificado derrubou");
        assert!(pagina.contains("HTTP/1.0 200 ok"), "{pagina}");
        assert!(pagina.contains("no client certificate"), "{pagina}");
    }

    #[test]
    fn com_o_openssl_s_server_de_certificado_ed25519() {
        // A chave e o certificado vem do OPENSSL, nao desta casa: a conferencia
        // Ed25519 e o SPKI lido de certificado alheio.
        let d = dir("ed25519");
        let cert = d.0.join("cert.pem");
        let chave = d.0.join("chave.pem");
        let ok = Command::new("openssl")
            .args(["req", "-x509", "-newkey", "ed25519", "-nodes", "-days", "1"])
            .args(["-subj", "/CN=localhost", "-keyout"])
            .arg(&chave)
            .arg("-out")
            .arg(&cert)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(ok.success());
        let der = crate::x509::blocos_pem(&std::fs::read_to_string(&cert).unwrap(), "CERTIFICATE")
            .unwrap()
            .remove(0);
        let spki = crate::x509::spki_do_certificado(&der).unwrap();
        assert!(matches!(
            crate::x509::chave_do_spki(spki).unwrap(),
            ChavePublica::Ed25519(_)
        ));
        // O pino daqui e o do openssl: `pkey -pubout | sha256`.
        let pub_der = Command::new("openssl")
            .args(["pkey", "-pubout", "-outform", "DER", "-in"])
            .arg(&chave)
            .output()
            .unwrap()
            .stdout;
        assert_eq!(spki, pub_der.as_slice(), "o SPKI lido nao e o do openssl");
        let s = s_server(&cert, &chave, &[]);
        let (pagina, _) = pedir_ao_s_server(s.1, sha256(spki)).expect("o Ed25519 falhou");
        assert!(pagina.contains("HTTP/1.0 200 ok"), "{pagina}");
    }

    #[test]
    fn com_o_openssl_s_server_de_chave_rsa_a_recusa_vem_do_servidor() {
        // RSA nao se anuncia (nao se sabe conferir): o servidor que so tem RSA
        // nao acha assinatura em comum e aborta com handshake_failure (40).
        let d = dir("rsa");
        let cert = d.0.join("cert.pem");
        let chave = d.0.join("chave.pem");
        let ok = Command::new("openssl")
            .args([
                "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
            ])
            .args(["-subj", "/CN=localhost", "-keyout"])
            .arg(&chave)
            .arg("-out")
            .arg(&cert)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(ok.success());
        let s = s_server(&cert, &chave, &[]);
        let e = pedir_ao_s_server(s.1, [0; 32])
            .expect_err("conectou num servidor RSA")
            .to_string();
        assert!(e.contains("alerta 40"), "{e}");
    }
}
