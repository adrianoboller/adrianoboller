//! TLS 1.3 (RFC 8446), a parte que nao depende de rede: o cronograma de
//! chaves (§7.1), a transcricao, o `Finished` (§4.4.4) e a camada de registro
//! protegida (§5.2-5.3).
//!
//! # Por que existe
//!
//! TLS no transporte, escrito nesta casa (decisao do dono, 30/09/2026, pedido
//! 572). O aperto de mao e a porta vem por cima disto; aqui fica o que se
//! confere contra vetor sem precisar de soquete nenhum.
//!
//! # Como se sabe que esta certo
//!
//! O cronograma inteiro -- `early`, `handshake`, `master`, os quatro segredos
//! de trafego, o `exporter`, o `resumption`, as chaves e IVs e os dois
//! `Finished` -- confere byte a byte contra o traco da secao 3 da RFC 8448
//! («Simple 1-RTT Handshake»). As constantes do teste foram EXTRAIDAS do texto
//! oficial por script, com o tamanho de cada uma conferido contra o que a RFC
//! declara, e nao digitadas.
//!
//! O traco da RFC 8448 usa `TLS_AES_128_GCM_SHA256`, e por isso o registro
//! protegido dele so se confere quando o AES-GCM entrar (T5 do pedido 572). O
//! cronograma nao depende da cifra -- so do tamanho da chave --, e e por isso
//! que ele ja se confere inteiro aqui.

use crate::error::{PhxError, Result};
use crate::hash::{hmac_sha256, sha256, Sha256};

/// Tamanho do resumo (SHA-256): todos os conjuntos que esta casa oferece o
/// usam.
pub const RESUMO: usize = 32;

/// `HKDF-Expand-Label` (RFC 8446 §7.1).
///
/// ```text
/// struct { uint16 length; opaque label<7..255> = "tls13 " + Label;
///          opaque context<0..255>; } HkdfLabel;
/// ```
pub fn expandir_rotulo(segredo: &[u8; RESUMO], rotulo: &str, contexto: &[u8], saida: &mut [u8]) {
    let rotulo = rotulo.as_bytes();
    let mut info = Vec::with_capacity(4 + 6 + rotulo.len() + contexto.len());
    info.extend_from_slice(&(saida.len() as u16).to_be_bytes());
    info.push((6 + rotulo.len()) as u8);
    info.extend_from_slice(b"tls13 ");
    info.extend_from_slice(rotulo);
    info.push(contexto.len() as u8);
    info.extend_from_slice(contexto);
    // Os tamanhos pedidos aqui sao constantes do protocolo (<= 32 bytes), bem
    // abaixo do teto de 255 blocos do HKDF: um erro so viria de uso errado.
    crate::hkdf::expandir(segredo, &info, saida)
        .expect("HKDF-Expand-Label com tamanho de protocolo nao estoura o HKDF");
}

/// `Derive-Secret(Secret, Label, Messages)`, com o resumo das mensagens ja
/// calculado.
pub fn derivar_segredo(
    segredo: &[u8; RESUMO],
    rotulo: &str,
    resumo: &[u8; RESUMO],
) -> [u8; RESUMO] {
    let mut s = [0u8; RESUMO];
    expandir_rotulo(segredo, rotulo, resumo, &mut s);
    s
}

/// O `Early Secret` sem PSK: `HKDF-Extract(0, 0)`.
pub fn segredo_early() -> [u8; RESUMO] {
    crate::hkdf::extrair(&[], &[0u8; RESUMO])
}

/// O `Handshake Secret`, a partir do `early` e do segredo do (EC)DHE.
pub fn segredo_handshake(early: &[u8; RESUMO], ecdhe: &[u8]) -> [u8; RESUMO] {
    let sal = derivar_segredo(early, "derived", &sha256(b""));
    crate::hkdf::extrair(&sal, ecdhe)
}

/// O `Master Secret`, a partir do `handshake`.
pub fn segredo_master(handshake: &[u8; RESUMO]) -> [u8; RESUMO] {
    let sal = derivar_segredo(handshake, "derived", &sha256(b""));
    crate::hkdf::extrair(&sal, &[0u8; RESUMO])
}

/// `TLS-Exporter(label, context_value, key_length)` (§7.5):
///
/// ```text
/// HKDF-Expand-Label(Derive-Secret(exporter_master_secret, label, ""),
///                   "exporter", Hash(context_value), key_length)
/// ```
///
/// Sem contexto e com contexto vazio dao o MESMO valor no TLS 1.3 -- a §7.5
/// acabou com a distincao do 1.2, e e por isso que nao ha `Option` aqui.
/// Recusa tamanho acima do teto do HKDF (255 blocos) em vez de estourar: o
/// tamanho e de quem chama, e nao uma constante do protocolo.
pub fn exportar(
    exp_master: &[u8; RESUMO],
    rotulo: &str,
    contexto: &[u8],
    saida: &mut [u8],
) -> Result<()> {
    // O rotulo vai num `opaque label<7..255>` junto do prefixo `tls13 `.
    if rotulo.len() > 255 - 6 || saida.len() > 255 * RESUMO {
        return Err(PhxError::LimiteExcedido(
            "exportador TLS: rotulo ou tamanho acima do que a RFC 8446 §7.1 codifica".into(),
        ));
    }
    let segredo = derivar_segredo(exp_master, rotulo, &sha256(b""));
    expandir_rotulo(&segredo, "exporter", &sha256(contexto), saida);
    Ok(())
}

/// Chave e IV de um sentido do trafego (§7.3).
pub struct ChavesDeTrafego {
    pub chave: Vec<u8>,
    pub iv: [u8; 12],
}

/// `[sender]_write_key` e `[sender]_write_iv` de um segredo de trafego.
pub fn chaves_de_trafego(segredo: &[u8; RESUMO], tamanho_da_chave: usize) -> ChavesDeTrafego {
    let mut chave = vec![0u8; tamanho_da_chave];
    expandir_rotulo(segredo, "key", &[], &mut chave);
    let mut iv = [0u8; 12];
    expandir_rotulo(segredo, "iv", &[], &mut iv);
    ChavesDeTrafego { chave, iv }
}

/// O `verify_data` do `Finished` (§4.4.4): HMAC da transcricao com a
/// `finished_key` do segredo de trafego do aperto de mao.
pub fn verify_data(segredo_base: &[u8; RESUMO], resumo: &[u8; RESUMO]) -> [u8; RESUMO] {
    let mut chave = [0u8; RESUMO];
    expandir_rotulo(segredo_base, "finished", &[], &mut chave);
    hmac_sha256(&chave, resumo)
}

/// A transcricao do aperto de mao: o SHA-256 corrente das mensagens, com o
/// resumo parcial sem encerrar o calculo (o `Sha256` se clona).
#[derive(Clone, Default)]
pub struct Transcricao(Sha256);

impl Transcricao {
    pub fn acrescentar(&mut self, mensagem: &[u8]) {
        self.0.atualizar(mensagem);
    }

    pub fn resumo(&self) -> [u8; RESUMO] {
        self.0.clone().finalizar()
    }
}

/// Tipo de conteudo do registro (§5.1).
pub mod tipo {
    pub const ALERTA: u8 = 21;
    pub const HANDSHAKE: u8 = 22;
    pub const DADOS: u8 = 23;
}

/// O maior texto claro de um registro (§5.1): 2^14.
pub const MAX_CLARO: usize = 1 << 14;

/// Os conjuntos de cifra que esta casa oferece. Os dois usam SHA-256, entao o
/// cronograma de chaves e o mesmo; muda o tamanho da chave e a AEAD.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Conjunto {
    /// `TLS_AES_128_GCM_SHA256` (0x1301), o que a §9.1 obriga a ter.
    Aes128GcmSha256,
    #[default]
    /// `TLS_CHACHA20_POLY1305_SHA256` (0x1303), o preferido: o AES desta casa
    /// e de tempo constante, e por isso lento (ver `crate::aes`).
    Chacha20Poly1305Sha256,
}

impl Conjunto {
    pub fn id(self) -> u16 {
        match self {
            Conjunto::Aes128GcmSha256 => 0x1301,
            Conjunto::Chacha20Poly1305Sha256 => 0x1303,
        }
    }

    pub fn de_id(id: u16) -> Option<Conjunto> {
        match id {
            0x1301 => Some(Conjunto::Aes128GcmSha256),
            0x1303 => Some(Conjunto::Chacha20Poly1305Sha256),
            _ => None,
        }
    }

    fn tamanho_da_chave(self) -> usize {
        match self {
            Conjunto::Aes128GcmSha256 => 16,
            Conjunto::Chacha20Poly1305Sha256 => 32,
        }
    }
}

/// A AEAD de um sentido, ja com a chave.
enum Aead {
    ChaCha([u8; 32]),
    Aes(Box<crate::aes::Gcm>),
}

/// Um sentido do trafego protegido: a AEAD, o IV e o numero de sequencia,
/// que so anda.
///
/// # O nonce
///
/// `iv XOR (0^4 || seq em big-endian)` (§5.3). O `seq` nunca se repete no
/// mesmo sentido com a mesma chave -- e por isso ele e privado e so anda
/// dentro de [`Protecao::selar`] e [`Protecao::abrir`], e nao se escolhe por
/// fora.
pub struct Protecao {
    aead: Aead,
    iv: [u8; 12],
    seq: u64,
}

impl Protecao {
    /// A protecao de um segredo de trafego, com a cifra ChaCha20-Poly1305.
    pub fn de_segredo(segredo: &[u8; RESUMO]) -> Protecao {
        Protecao::de_segredo_com(segredo, Conjunto::Chacha20Poly1305Sha256)
    }

    /// A protecao de um segredo de trafego, com a cifra do conjunto dado.
    pub fn de_segredo_com(segredo: &[u8; RESUMO], conjunto: Conjunto) -> Protecao {
        let c = chaves_de_trafego(segredo, conjunto.tamanho_da_chave());
        let aead = match conjunto {
            Conjunto::Chacha20Poly1305Sha256 => {
                let mut k = [0u8; 32];
                k.copy_from_slice(&c.chave);
                Aead::ChaCha(k)
            }
            Conjunto::Aes128GcmSha256 => {
                let mut k = [0u8; 16];
                k.copy_from_slice(&c.chave);
                Aead::Aes(Box::new(crate::aes::Gcm::nova(&k)))
            }
        };
        Protecao {
            aead,
            iv: c.iv,
            seq: 0,
        }
    }

    fn nonce(&self) -> [u8; 12] {
        let mut n = self.iv;
        for (i, b) in self.seq.to_be_bytes().iter().enumerate() {
            n[4 + i] ^= b;
        }
        n
    }

    fn andar(&mut self) -> Result<()> {
        // 2^64 registros nao acontecem numa conexao; o que se recusa e o
        // contador dar a volta e repetir nonce.
        self.seq = self.seq.checked_add(1).ok_or_else(|| {
            PhxError::LimiteExcedido("TLS: o numero de sequencia do registro esgotou".into())
        })?;
        Ok(())
    }

    /// O registro inteiro (`17 03 03 len || cifrado || etiqueta`) de
    /// `conteudo` com o tipo verdadeiro `tipo` escondido dentro.
    pub fn selar(&mut self, tipo: u8, conteudo: &[u8]) -> Result<Vec<u8>> {
        if conteudo.len() > MAX_CLARO {
            return Err(PhxError::LimiteExcedido(format!(
                "TLS: registro de {} bytes passa de 2^14",
                conteudo.len()
            )));
        }
        let mut interno = Vec::with_capacity(conteudo.len() + 1);
        interno.extend_from_slice(conteudo);
        interno.push(tipo);
        // As duas AEAD daqui tem etiqueta de 16 bytes.
        let tamanho = interno.len() + crate::cifra::TAG_LEN;
        let cab = [tipo::DADOS, 3, 3, (tamanho >> 8) as u8, tamanho as u8];
        let nonce = self.nonce();
        let (cifrado, etiqueta) = match &self.aead {
            Aead::ChaCha(k) => crate::cifra::selar(k, &nonce, &cab, &interno),
            Aead::Aes(g) => g.selar(&nonce, &cab, &interno),
        };
        self.andar()?;
        let mut r = Vec::with_capacity(5 + tamanho);
        r.extend_from_slice(&cab);
        r.extend_from_slice(&cifrado);
        r.extend_from_slice(&etiqueta);
        Ok(r)
    }

    /// Abre um registro protegido: devolve o tipo verdadeiro e o conteudo, sem
    /// o enchimento de zeros. Etiqueta errada nao decifra nada.
    pub fn abrir(&mut self, cabecalho: &[u8; 5], corpo: &[u8]) -> Result<(u8, Vec<u8>)> {
        let tag_len = crate::cifra::TAG_LEN;
        if cabecalho[0] != tipo::DADOS || corpo.len() < tag_len + 1 {
            return Err(PhxError::Corrompido(
                "TLS: registro protegido malformado".into(),
            ));
        }
        if corpo.len() > MAX_CLARO + 256 {
            return Err(PhxError::LimiteExcedido(
                "TLS: registro protegido longo demais".into(),
            ));
        }
        let (cifrado, etiqueta) = corpo.split_at(corpo.len() - tag_len);
        let mut tag = [0u8; 16];
        tag.copy_from_slice(etiqueta);
        let nonce = self.nonce();
        let mut claro = match &self.aead {
            Aead::ChaCha(k) => crate::cifra::abrir(k, &nonce, cabecalho, cifrado, &tag)?,
            Aead::Aes(g) => g.abrir(&nonce, cabecalho, cifrado, &tag)?,
        };
        self.andar()?;
        // O tipo verdadeiro e o ultimo byte diferente de zero (§5.4).
        while let Some(&0) = claro.last() {
            claro.pop();
        }
        let tipo = claro
            .pop()
            .ok_or_else(|| PhxError::Corrompido("TLS: registro so de enchimento".into()))?;
        Ok((tipo, claro))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    // Extraidas do texto oficial da RFC 8448 §3 por script, com o tamanho de
    // cada uma conferido contra o declarado.
    const CLI_PRIV: &str = "49af42ba7f7994852d713ef2784bcbcaa7911de26adc5642cb634540e7ea5005";
    const CLI_PUB: &str = "99381de560e4bd43d23d8e435a7dbafeb3c06e51c13cae4d5413691e529aaf2c";
    const CLIENT_HELLO: &str = "010000c00303cb34ecb1e78163ba1c38c6dacb196a6dffa21a8d9912ec18a2ef6283024dece7000006130113031302010000910000000b0009000006736572766572ff01000100000a00140012001d0017001800190100010101020103010400230000003300260024001d002099381de560e4bd43d23d8e435a7dbafeb3c06e51c13cae4d5413691e529aaf2c002b0003020304000d0020001e040305030603020308040805080604010501060102010402050206020202002d00020101001c00024001";
    const SRV_PRIV: &str = "b1580eeadf6dd589b8ef4f2d5652578cc810e9980191ec8d058308cea216a21e";
    const SRV_PUB: &str = "c9828876112095fe66762bdbf7c672e156d6cc253b833df1dd69b1b04e751f0f";
    const SERVER_HELLO: &str = "020000560303a6af06a4121860dc5e6e60249cd34c95930c8ac5cb1434dac155772ed3e2692800130100002e00330024001d0020c9828876112095fe66762bdbf7c672e156d6cc253b833df1dd69b1b04e751f0f002b00020304";
    const EARLY: &str = "33ad0a1c607ec03b09e6cd9893680ce210adf300aa1f2660e1b22e10f170f92a";
    const ECDHE: &str = "8bd4054fb55b9d63fdfbacf9f04b9f0d35e6d63f537563efd46272900f89492d";
    const HANDSHAKE: &str = "1dc826e93606aa6fdc0aadc12f741b01046aa6b99f691ed221a9f0ca043fbeac";
    const C_HS: &str = "b3eddb126e067f35a780b3abf45e2d8f3b1a950738f52e9600746a0e27a55a21";
    const S_HS: &str = "b67b7d690cc16c4e75e54213cb2d37b4e9c912bcded9105d42befd59d391ad38";
    const MASTER: &str = "18df06843d13a08bf2a449844c5f8a478001bc4d4c627984d5a41da8d0402919";
    const S_HS_KEY: &str = "3fce516009c21727d0f2e4e86ee403bc";
    const S_HS_IV: &str = "5d313eb2671276ee13000b30";
    const SRV_VOO: &str = "080000240022000a00140012001d00170018001901000101010201030104001c00024001000000000b0001b9000001b50001b0308201ac30820115a003020102020102300d06092a864886f70d01010b0500300e310c300a06035504031303727361301e170d3136303733303031323335395a170d3236303733303031323335395a300e310c300a0603550403130372736130819f300d06092a864886f70d010101050003818d0030818902818100b4bb498f8279303d980836399b36c6988c0c68de55e1bdb826d3901a2461eafd2de49a91d015abbc9a95137ace6c1af19eaa6af98c7ced43120998e187a80ee0ccb0524b1b018c3e0b63264d449a6d38e22a5fda430846748030530ef0461c8ca9d9efbfae8ea6d1d03e2bd193eff0ab9a8002c47428a6d35a8d88d79f7f1e3f0203010001a31a301830090603551d1304023000300b0603551d0f0404030205a0300d06092a864886f70d01010b05000381810085aad2a0e5b9276b908c65f73a7267170618a54c5f8a7b337d2df7a594365417f2eae8f8a58c8f8172f9319cf36b7fd6c55b80f21a03015156726096fd335e5e67f2dbf102702e608ccae6bec1fc63a42a99be5c3eb7107c3c54e9b9eb2bd5203b1c3b84e0a8b2f759409ba3eac9d91d402dcc0cc8f8961229ac9187b42b4de100000f000084080400805a747c5d88fa9bd2e55ab085a61015b7211f824cd484145ab3ff52f1fda8477b0b7abc90db78e2d33a5c141a078653fa6bef780c5ea248eeaaa785c4f394cab6d30bbe8d4859ee511f602957b15411ac027671459e46445c9ea58c181e818e95b8c3fb0bf3278409d3be152a3da5043e063dda65cdf5aea20d53dfacd42f74f3140000209b9b141d906337fbd2cbdce71df4deda4ab42c309572cb7fffee5454b78f0718";
    const S_FINISHED: &str = "9b9b141d906337fbd2cbdce71df4deda4ab42c309572cb7fffee5454b78f0718";
    const C_AP: &str = "9e40646ce79a7f9dc05af8889bce6552875afa0b06df0087f792ebb7c17504a5";
    const S_AP: &str = "a11af9f05531f856ad47116b45a950328204b4f44bfb6b3a4b4f1f3fcb631643";
    const EXP: &str = "fe22f881176eda18eb8f44529e6792c50c9a3f89452f68d8ae311b4309d3cf50";
    const S_AP_KEY: &str = "9f02283b6c9c07efc26bb9f2ac92e356";
    const S_AP_IV: &str = "cf782b88dd83549aadf1e984";
    const C_FINISHED: &str = "a8ec436d677634ae525ac1fcebe11a039ec17694fac6e98527b642f2edd5ce61";
    const RES: &str = "7df235f2031d2a051287d02b0241b0bfdaf86cc856231f2d5aba46c434ec196c";

    fn b(h: &str) -> Vec<u8> {
        crate::hash::de_hex(h).unwrap()
    }

    // Os registros cifrados da secao 3 (AES-128-GCM), extraidos do texto
    // oficial pelo mesmo script, com o tamanho conferido contra o declarado.
    const REC_SRV_VOO: &str = "17030302a2d1ff334a56f5bff6594a07cc87b580233f500f45e489e7f33af35edf7869fcf40aa40aa2b8ea73f848a7ca07612ef9f945cb960b4068905123ea78b111b429ba9191cd05d2a389280f526134aadc7fc78c4b729df828b5ecf7b13bd9aefb0e57f271585b8ea9bb355c7c79020716cfb9b1183ef3ab20e37d57a6b9d7477609aee6e122a4cf51427325250c7d0e509289444c9b3a648f1d71035d2ed65b0e3cdd0cbae8bf2d0b227812cbb360987255cc744110c453baa4fcd610928d809810e4b7ed1a8fd991f06aa6248204797e36a6a73b70a2559c09ead686945ba246ab66e5edd8044b4c6de3fcf2a89441ac66272fd8fb330ef8190579b3684596c960bd596eea520a56a8d650f563aad27409960dca63d3e688611ea5e22f4415cf9538d51a200c27034272968a264ed6540c84838d89f72c24461aad6d26f59ecaba9acbbb317b66d902f4f292a36ac1b639c637ce343117b659622245317b49eeda0c6258f100d7d961ffb138647e92ea330faeea6dfa31c7a84dc3bd7e1b7a6c7178af36879018e3f252107f243d243dc7339d5684c8b0378bf30244da8c87c843f5e56eb4c5e8280a2b48052cf93b16499a66db7cca71e4599426f7d461e66f99882bd89fc50800becca62d6c74116dbd2972fda1fa80f85df881edbe5a37668936b335583b599186dc5c6918a396fa48a181d6b6fa4f9d62d513afbb992f2b992f67f8afe67f76913fa388cb5630c8ca01e0c65d11c66a1e2ac4c85977b7c7a6999bbf10dc35ae69f5515614636c0b9b68c19ed2e31c0b3b66763038ebba42f3b38edc0399f3a9f23faa63978c317fc9fa66a73f60f0504de93b5b845e275592c12335ee340bbc4fddd502784016e4b3be7ef04dda49f4b440a30cb5d2af939828fd4ae3794e44f94df5a631ede42c1719bfdabf0253fe5175be898e750edc53370d2b";
    const REC_SRV_DADOS: &str = "17030300432e937e11ef4ac740e538ad36005fc4a46932fc3225d05f82aa1b36e30efaf97d90e6dffc602dcb501a59a8fcc49c4bf2e5f0a21c0047c2abf332540dd032e167c2955d";
    const REC_SRV_ALERTA: &str = "1703030013b58fd67166ebf599d24720cfbe7efa7a8864a9";

    /// O registro do traco, byte a byte: o voo cifrado do servidor sob a
    /// chave do aperto, e o dado e o alerta sob a de aplicacao (o alerta e o
    /// TERCEIRO registro dela, e prova que a sequencia anda).
    #[test]
    fn os_registros_aes_gcm_da_rfc_8448() {
        let mut hs = Protecao::de_segredo_com(&a32(S_HS), Conjunto::Aes128GcmSha256);
        assert_eq!(
            hs.selar(tipo::HANDSHAKE, &b(SRV_VOO)).unwrap(),
            b(REC_SRV_VOO)
        );
        let mut ap = Protecao::de_segredo_com(&a32(S_AP), Conjunto::Aes128GcmSha256);
        // No traco, o primeiro registro do servidor sob esta chave e o
        // NewSessionTicket (sequencia 0); o dado e a 1 e o alerta, a 2.
        ap.selar(tipo::HANDSHAKE, &[0u8; 8]).unwrap();
        let payload: Vec<u8> = (0u8..50).collect();
        assert_eq!(ap.selar(tipo::DADOS, &payload).unwrap(), b(REC_SRV_DADOS));
        assert_eq!(ap.selar(tipo::ALERTA, &[1, 0]).unwrap(), b(REC_SRV_ALERTA));

        // E o outro sentido: o cliente do traco abre o que o servidor mandou.
        let mut le = Protecao::de_segredo_com(&a32(S_AP), Conjunto::Aes128GcmSha256);
        le.seq = 1;
        let r = b(REC_SRV_DADOS);
        let cab: [u8; 5] = r[..5].try_into().unwrap();
        assert_eq!(le.abrir(&cab, &r[5..]).unwrap(), (tipo::DADOS, payload));
    }

    fn a32(h: &str) -> [u8; 32] {
        b(h).try_into().unwrap()
    }

    #[test]
    fn o_ecdhe_do_traco_sai_do_x25519() {
        let s = crate::x25519::segredo(&a32(SRV_PRIV), &a32(CLI_PUB)).unwrap();
        assert_eq!(s.to_vec(), b(ECDHE));
        let do_cliente = crate::x25519::segredo(&a32(CLI_PRIV), &a32(SRV_PUB)).unwrap();
        assert_eq!(do_cliente, s, "os dois lados do traco nao concordam");
        assert_eq!(
            crate::x25519::chave_publica(&a32(SRV_PRIV)).to_vec(),
            b(SRV_PUB)
        );
    }

    #[test]
    fn o_cronograma_inteiro_da_rfc_8448() {
        let early = segredo_early();
        assert_eq!(early.to_vec(), b(EARLY), "early");
        let hs = segredo_handshake(&early, &b(ECDHE));
        assert_eq!(hs.to_vec(), b(HANDSHAKE), "handshake");

        let mut t = Transcricao::default();
        t.acrescentar(&b(CLIENT_HELLO));
        t.acrescentar(&b(SERVER_HELLO));
        let c_hs = derivar_segredo(&hs, "c hs traffic", &t.resumo());
        let s_hs = derivar_segredo(&hs, "s hs traffic", &t.resumo());
        assert_eq!(c_hs.to_vec(), b(C_HS), "c hs traffic");
        assert_eq!(s_hs.to_vec(), b(S_HS), "s hs traffic");

        let k = chaves_de_trafego(&s_hs, 16);
        assert_eq!(k.chave, b(S_HS_KEY), "chave do aperto");
        assert_eq!(k.iv.to_vec(), b(S_HS_IV), "iv do aperto");

        let master = segredo_master(&hs);
        assert_eq!(master.to_vec(), b(MASTER), "master");

        // O voo do servidor: EncryptedExtensions, Certificate,
        // CertificateVerify e Finished -- o Finished do servidor e o HMAC da
        // transcricao ATE o CertificateVerify.
        let voo = b(SRV_VOO);
        let finished_srv = &voo[voo.len() - 36..];
        let mut ate_cv = t.clone();
        ate_cv.acrescentar(&voo[..voo.len() - 36]);
        let vd = verify_data(&s_hs, &ate_cv.resumo());
        assert_eq!(vd.to_vec(), b(S_FINISHED), "Finished do servidor");
        assert_eq!(&finished_srv[4..], &vd[..], "o Finished dentro do voo");

        t.acrescentar(&voo);
        let c_ap = derivar_segredo(&master, "c ap traffic", &t.resumo());
        let s_ap = derivar_segredo(&master, "s ap traffic", &t.resumo());
        let exp = derivar_segredo(&master, "exp master", &t.resumo());
        assert_eq!(c_ap.to_vec(), b(C_AP), "c ap traffic");
        assert_eq!(s_ap.to_vec(), b(S_AP), "s ap traffic");
        assert_eq!(exp.to_vec(), b(EXP), "exp master");
        let k = chaves_de_trafego(&s_ap, 16);
        assert_eq!(k.chave, b(S_AP_KEY));
        assert_eq!(k.iv.to_vec(), b(S_AP_IV));

        let vd_c = verify_data(&c_hs, &t.resumo());
        assert_eq!(vd_c.to_vec(), b(C_FINISHED), "Finished do cliente");
        let mut fin = vec![20, 0, 0, 32];
        fin.extend_from_slice(&vd_c);
        t.acrescentar(&fin);
        let res = derivar_segredo(&master, "res master", &t.resumo());
        assert_eq!(res.to_vec(), b(RES), "res master");
    }

    /// §5.3: o numero de sequencia, em 64 bits big-endian, entra nos 8
    /// bytes de BAIXO do IV (os 4 primeiros ficam). Um teste de ida e volta
    /// nunca pega o nonce errado -- os dois lados erram igual (medido: o
    /// defeito reposto passa nele) --, entao o valor esta escrito a parte.
    #[test]
    fn o_nonce_e_o_iv_com_a_sequencia_nos_bytes_de_baixo() {
        let p = Protecao {
            aead: Aead::ChaCha([0; 32]),
            iv: [0xA0, 0xA1, 0xA2, 0xA3, 0, 0, 0, 0, 0, 0, 0, 0xFF],
            seq: 0x0102_0304_0506_0708,
        };
        assert_eq!(
            p.nonce(),
            [0xA0, 0xA1, 0xA2, 0xA3, 1, 2, 3, 4, 5, 6, 7, 0xF7]
        );
    }

    #[test]
    fn registro_protegido_vai_e_volta_e_recusa_adulterado() {
        let segredo = [7u8; 32];
        let mut a = Protecao::de_segredo(&segredo);
        let mut b_ = Protecao::de_segredo(&segredo);
        for msg in [&b"primeiro"[..], b"segundo, com outro nonce"] {
            let r = a.selar(tipo::DADOS, msg).unwrap();
            let cab: [u8; 5] = r[..5].try_into().unwrap();
            let (t, claro) = b_.abrir(&cab, &r[5..]).unwrap();
            assert_eq!((t, claro.as_slice()), (tipo::DADOS, msg));
        }
        // O mesmo texto em dois registros sai diferente: o nonce andou.
        let r1 = a.selar(tipo::DADOS, b"igual").unwrap();
        let r2 = a.selar(tipo::DADOS, b"igual").unwrap();
        assert_ne!(r1, r2, "o numero de sequencia nao andou -- nonce repetido");
        let mut r3 = a.selar(tipo::ALERTA, b"x").unwrap();
        r3[7] ^= 1;
        let cab: [u8; 5] = r3[..5].try_into().unwrap();
        let mut c = Protecao::de_segredo(&segredo);
        for _ in 0..4 {
            let r = c.selar(tipo::DADOS, b"").unwrap();
            let _ = r;
        }
        assert!(
            c.abrir(&cab, &r3[5..]).is_err(),
            "registro adulterado abriu"
        );
    }
}
