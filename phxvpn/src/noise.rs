//! O aperto de mao do modo P2P: `Noise_IKpsk2_25519_ChaChaPoly_SHA256`.
//!
//! ```text
//! <- s
//! ...
//! -> e, es, s, ss
//! <- e, ee, se, psk
//! ```
//!
//! # Por que IK com psk2 (e de onde vem cada peca)
//!
//! Decisao do papel J (pesquisa P2P, 23/09/2026), no molde do WireGuard:
//! * **IK** -- quem inicia ja sabe a estatica do par (veio no rol da rede), e
//!   manda a propria cifrada na primeira mensagem. Um ida-e-volta, e o
//!   iniciador ja se identifica.
//! * **psk2** -- a chave pre-compartilhada entra no FIM da segunda mensagem.
//!   Ela sai da senha da rede: sem a senha, nem par valido fecha o aperto.
//!   Defesa em profundidade, nao a porta: quebrar exige tambem a estatica.
//!
//! Onde diverge do WireGuard, e a restricao que causou: SHA-256 no lugar do
//! BLAKE2s, porque so se usa o que o `phxsql-core` ja tem conferido contra
//! vetor. O preco e nao falar com cliente WireGuard oficial.
//!
//! O estado simetrico e o `phxsql_core::fio::Simetrico` -- o MESMO motor do
//! aperto NX do PhxSql, nao uma copia. E a composicao inteira e conferida
//! contra o vetor de interoperabilidade oficial (cacophony) no teste abaixo.

use phxsql_core::fio::Simetrico;
use phxsql_core::x25519;

pub const NOME: &[u8] = b"Noise_IKpsk2_25519_ChaChaPoly_SHA256";

/// O prologo do phxvpn: amarra o aperto a este protocolo e versao.
pub const PROLOGO: &[u8] = b"phxvpn-p2p-v1";

pub type R<T> = Result<T, String>;

/// As chaves de transporte e a transcricao, iguais dos dois lados.
pub struct Sessao {
    pub envio: [u8; 32],
    pub recepcao: [u8; 32],
    pub transcricao: [u8; 32],
}

fn dh(privada: &[u8; 32], publica: &[u8; 32]) -> R<[u8; 32]> {
    x25519::segredo(privada, publica).map_err(|e| e.to_string())
}

fn para32(b: &[u8]) -> R<[u8; 32]> {
    b.try_into()
        .map_err(|_| "chave de tamanho errado no aperto".to_string())
}

/// O lado que comeca: conhece a propria estatica e a do par.
pub struct Iniciador {
    s: Simetrico,
    estatica: [u8; 32],
    efemera: [u8; 32],
    psk: [u8; 32],
}

impl Iniciador {
    /// Monta a mensagem 1 (`e, es, s, ss` + carga). Devolve o estado para
    /// fechar com a mensagem 2.
    pub fn comecar(
        prologo: &[u8],
        estatica: [u8; 32],
        dele: &[u8; 32],
        psk: [u8; 32],
        carga: &[u8],
    ) -> R<(Iniciador, Vec<u8>)> {
        Iniciador::comecar_com(prologo, estatica, x25519::gerar_privada(), dele, psk, carga)
    }

    fn comecar_com(
        prologo: &[u8],
        estatica: [u8; 32],
        efemera: [u8; 32],
        dele: &[u8; 32],
        psk: [u8; 32],
        carga: &[u8],
    ) -> R<(Iniciador, Vec<u8>)> {
        let mut s = Simetrico::iniciar(NOME, prologo);
        s.misturar_hash(dele); // pre-mensagem `<- s`
        let e_pub = x25519::chave_publica(&efemera);
        let mut m = e_pub.to_vec();
        s.misturar_hash(&e_pub);
        // Em padrao com psk, o token `e` tambem mistura a chave (secao 9.2).
        s.misturar_chave(&e_pub);
        s.misturar_chave(&dh(&efemera, dele)?); // es
        m.extend(s.cifrar_e_hash(&x25519::chave_publica(&estatica))); // s
        s.misturar_chave(&dh(&estatica, dele)?); // ss
        m.extend(s.cifrar_e_hash(carga));
        Ok((
            Iniciador {
                s,
                estatica,
                efemera,
                psk,
            },
            m,
        ))
    }

    /// Fecha com a mensagem 2. Devolve a sessao e a carga do par.
    pub fn terminar(mut self, m2: &[u8]) -> R<(Sessao, Vec<u8>)> {
        if m2.len() < 32 + 16 {
            return Err("mensagem 2 do aperto curta demais".into());
        }
        let re = para32(&m2[..32])?;
        self.s.misturar_hash(&re);
        self.s.misturar_chave(&re);
        self.s.misturar_chave(&dh(&self.efemera, &re)?); // ee
        self.s.misturar_chave(&dh(&self.estatica, &re)?); // se
        self.s.misturar_chave_e_hash(&self.psk); // psk
        let carga = self
            .s
            .decifrar_e_hash(&m2[32..])
            .map_err(|_| "o par nao fechou o aperto (chave ou senha da rede)".to_string())?;
        let (envio, recepcao) = self.s.dividir();
        Ok((
            Sessao {
                envio,
                recepcao,
                transcricao: self.s.transcricao(),
            },
            carga,
        ))
    }
}

/// O que o respondedor aprende da mensagem 1 antes de decidir aceitar: a
/// estatica de quem chama. A decisao (esta no rol?) e de quem chama esta
/// funcao, e ela vem ANTES de montar a resposta.
pub struct Chamada {
    s: Simetrico,
    re: [u8; 32],
    pub estatica_dele: [u8; 32],
    pub carga: Vec<u8>,
}

/// Le a mensagem 1 com a estatica propria.
pub fn ler_chamada(prologo: &[u8], estatica: &[u8; 32], m1: &[u8]) -> R<Chamada> {
    if m1.len() < 32 + 48 + 16 {
        return Err("mensagem 1 do aperto curta demais".into());
    }
    let mut s = Simetrico::iniciar(NOME, prologo);
    s.misturar_hash(&x25519::chave_publica(estatica));
    let re = para32(&m1[..32])?;
    s.misturar_hash(&re);
    s.misturar_chave(&re);
    s.misturar_chave(&dh(estatica, &re)?); // es
    let rs = s
        .decifrar_e_hash(&m1[32..80])
        .map_err(|_| "mensagem 1 nao abre com esta chave".to_string())?;
    let estatica_dele = para32(&rs)?;
    s.misturar_chave(&dh(estatica, &estatica_dele)?); // ss
    let carga = s
        .decifrar_e_hash(&m1[80..])
        .map_err(|_| "carga da mensagem 1 nao confere".to_string())?;
    Ok(Chamada {
        s,
        re,
        estatica_dele,
        carga,
    })
}

impl Chamada {
    /// Responde (`e, ee, se, psk` + carga) e fecha a sessao do lado de ca.
    pub fn responder(self, psk: [u8; 32], carga: &[u8]) -> R<(Sessao, Vec<u8>)> {
        self.responder_com(x25519::gerar_privada(), psk, carga)
    }

    fn responder_com(
        mut self,
        efemera: [u8; 32],
        psk: [u8; 32],
        carga: &[u8],
    ) -> R<(Sessao, Vec<u8>)> {
        let e_pub = x25519::chave_publica(&efemera);
        let mut m = e_pub.to_vec();
        self.s.misturar_hash(&e_pub);
        self.s.misturar_chave(&e_pub);
        self.s.misturar_chave(&dh(&efemera, &self.re)?); // ee
        self.s.misturar_chave(&dh(&efemera, &self.estatica_dele)?); // se
        self.s.misturar_chave_e_hash(&psk);
        m.extend(self.s.cifrar_e_hash(carga));
        // Quem responde recebe o que o iniciador envia: as chaves trocam.
        let (do_iniciador, do_respondedor) = self.s.dividir();
        Ok((
            Sessao {
                envio: do_respondedor,
                recepcao: do_iniciador,
                transcricao: self.s.transcricao(),
            },
            m,
        ))
    }
}

/// Confere a composicao inteira contra o vetor oficial de interoperabilidade
/// (cacophony, `vectors/cacophony.txt`, `Noise_IKpsk2_25519_ChaChaPoly_SHA256`):
/// as duas mensagens do aperto, o hash da transcricao e a primeira mensagem de
/// transporte tem de sair BYTE A BYTE iguais.
///
/// Publico de proposito: o `autoteste` do console roda isto na maquina do
/// cliente -- o binario que ele tem prova que a cifra e a da norma.
pub fn autoteste() -> R<()> {
    use phxsql_core::cifra::{abrir, selar};
    use phxsql_core::fio::nonce_do_contador;
    use phxsql_core::hash::{de_hex, para_hex};
    let hx = |s: &str| de_hex(s).ok_or_else(|| "vetor com hex torto".to_string());
    let h32 = |s: &str| -> R<[u8; 32]> { para32(&hx(s)?) };
    let igual = |o_que: &str, veio: &[u8], esperado: &str| -> R<()> {
        if para_hex(veio) == esperado {
            Ok(())
        } else {
            Err(format!("vetor cacophony: {o_que} nao confere"))
        }
    };
    let prologo = hx("4a6f686e2047616c74")?;
    let psk = h32("54686973206973206d7920417573747269616e20706572737065637469766521")?;
    let i_s = h32("e61ef9919cde45dd5f82166404bd08e38bceb5dfdfded0a34c8df7ed542214d1")?;
    let i_e = h32("893e28b9dc6ca8d611ab664754b8ceb7bac5117349a4439a6b0569da977c464a")?;
    let r_s = h32("4a3acbfdb163dec651dfa3194dece676d437029c62a408b4c5ea9114246e4893")?;
    let r_e = h32("bbdb4cdbd309f1a1f2e1456967fe288cadd6f712d65dc7b7793d5e63da6b375b")?;
    let r_pub = x25519::chave_publica(&r_s);
    igual(
        "chave publica",
        &r_pub,
        "31e0303fd6418d2f8c0e78b91f22e8caed0fbe48656dcf4767e4834f701b8f62",
    )?;
    let (ini, m1) = Iniciador::comecar_com(
        &prologo,
        i_s,
        i_e,
        &r_pub,
        psk,
        &hx("4c756477696720766f6e204d69736573")?,
    )?;
    igual("mensagem 1", &m1, "ca35def5ae56cec33dc2036731ab14896bc4c75dbb07a61f879f8e3afa4c79442ec9b09893d0f510791784c10cbc959f25b1766e0def6e301d14fbca1c7790ac829b8b3674f5f649a5f0e98479662cbfbf2b2c47cd4b09fcd266cd29d7cb675f1808849707847840f6d178ec4d3733aa")?;
    let chamada = ler_chamada(&prologo, &r_s, &m1)?;
    let (sr, m2) = chamada.responder_com(r_e, psk, &hx("4d757272617920526f746862617264")?)?;
    igual("mensagem 2", &m2, "95ebc60d2b1fa672c1f46a8aa265ef51bfe38e7ccb39ec5be34069f1448088439a1b3cebf680b2c74217fcb5eba4ff58a9468cd90c4aca6194f57479b379a7")?;
    let (si, _) = ini.terminar(&m2)?;
    let hh = "8310f86394dc0dabb40beb8210031556db4403ab1202db7034c526232147a700";
    igual("transcricao (iniciador)", &si.transcricao, hh)?;
    igual("transcricao (respondedor)", &sr.transcricao, hh)?;
    let (c, tag) = selar(
        &si.envio,
        &nonce_do_contador(0),
        &[],
        &hx("462e20412e20486179656b")?,
    );
    let mut m3 = c.clone();
    m3.extend_from_slice(&tag);
    igual(
        "transporte",
        &m3,
        "a8fde7a0accec190cd306c5950d4fd8e04a205ec288aa747d8b347",
    )?;
    abrir(&sr.recepcao, &nonce_do_contador(0), &[], &c, &tag)
        .map(|_| ())
        .map_err(|_| "vetor cacophony: o respondedor nao abre o transporte".to_string())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ikpsk2_confere_contra_o_vetor_cacophony() {
        autoteste().unwrap();
    }

    #[test]
    fn senha_da_rede_errada_nao_fecha() {
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let (ini, m1) =
            Iniciador::comecar(PROLOGO, a, &x25519::chave_publica(&b), [1; 32], b"").unwrap();
        let (_, m2) = ler_chamada(PROLOGO, &b, &m1)
            .unwrap()
            .responder([2; 32], b"")
            .unwrap();
        assert!(ini.terminar(&m2).is_err());
    }

    #[test]
    fn chamada_para_outra_chave_nao_abre() {
        let (a, b, c) = (
            x25519::gerar_privada(),
            x25519::gerar_privada(),
            x25519::gerar_privada(),
        );
        let (_, m1) =
            Iniciador::comecar(PROLOGO, a, &x25519::chave_publica(&b), [1; 32], b"").unwrap();
        assert!(ler_chamada(PROLOGO, &c, &m1).is_err());
    }
}
