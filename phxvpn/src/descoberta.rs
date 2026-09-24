//! Descoberta na LAN: membros da mesma rede no mesmo segmento se acham sem
//! endereco configurado e sem repasse.
//!
//! # O anuncio
//!
//! ```text
//! [0x20, 0, 0, 0] | nonce 24 | XChaCha20-Poly1305( carimbo_ms 8 | publica 32 | zeros ) | etiqueta 16
//! ```
//!
//! Tamanho FIXO (`TAM_ANUNCIO`), mandado a cada `ANUNCIAR_A_CADA` por
//! broadcast na porta UDP da propria rede -- o mesmo soquete do tunel, entao
//! o endereco de origem do anuncio JA e o endereco onde o aperto Noise chega.
//! A chave e `HKDF(PSK da rede, "anuncio")`: quem nao tem a senha ve um
//! datagrama de tamanho fixo e bytes sorteados, sem nome de rede nem chave de
//! membro; e nao consegue forjar um que alguem aceite.
//!
//! A autenticacao e a etiqueta Poly1305 do selo, nao um HMAC separado: o selo
//! ja autentica cabecalho (dado associado) e conteudo, e um HMAC ao lado seria
//! uma segunda conferencia da mesma coisa. XChaCha porque o nonce e sorteado.
//!
//! # A resposta, e por que nao vira amplificador
//!
//! Nao ha pacote de resposta proprio: quem ouve o anuncio de um par CONHECIDO
//! (esta na lista -- ou no rol assinado) e sem sessao viva anota o endereco e
//! manda um INICIO do Noise, que e o que ja mandaria. Entao:
//! - so quem tem a senha provoca resposta (anuncio sem etiqueta valida morre);
//! - repeticao de anuncio gravado morre no carimbo (monotonico por chave e
//!   dentro de `JANELA_S`);
//! - a resposta (INICIO, ate 196 bytes) nunca passa do pedido (`TAM_ANUNCIO`,
//!   200) -- ha teste que trava isso;
//! - o INICIO ja tem teto proprio (um a cada 5 s por par, `REPETIR_APERTO`),
//!   e aqui ainda ha teto por origem e teto global ANTES de abrir o selo.
//!
//! # Desligada custa zero
//!
//! O interruptor e a PRIMEIRA coisa que `abrir` olha: desligada, o datagrama
//! morre sem abrir selo, sem mexer em mapa, sem relogio.

use phxsql_core::cifra::{xabrir, xselar};
use phxsql_core::senha::bytes_aleatorios;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const TIPO_ANUNCIO: u8 = 0x20;
const CABECALHO: [u8; 4] = [TIPO_ANUNCIO, 0, 0, 0];
/// Maior que o maior INICIO (196 bytes, com ficha de convite e apelido): a resposta
/// nunca e maior que o pedido.
pub const TAM_ANUNCIO: usize = 200;
const CLARO: usize = TAM_ANUNCIO - 4 - 24 - 16;
pub const ANUNCIAR_A_CADA: Duration = Duration::from_secs(5);
/// Carimbo aceito ate tantos segundos de diferenca do relogio local.
pub const JANELA_S: u64 = 300;
/// Anuncios abertos por segundo, no total e por IP de origem. Uma LAN com
/// dezenas de membros anunciando a cada 5 s fica longe dos dois.
const TETO_POR_S: u32 = 50;
const TETO_POR_ORIGEM_POR_S: u32 = 4;
const TETO_ORIGENS: usize = 1024;

fn agora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn chave_do_anuncio(psk: &[u8; 32]) -> [u8; 32] {
    let mut k = [0u8; 32];
    phxsql_core::hkdf::derivar(b"phxvpn-descoberta-v1", psk, b"anuncio", &mut k).expect("32 bytes");
    k
}

pub struct Descoberta {
    chave: [u8; 32],
    ligada: bool,
    ultimo_envio: Option<Instant>,
    /// Ultimo carimbo aceito por chave de membro.
    vistos: HashMap<[u8; 32], u64>,
    janela: Instant,
    na_janela: u32,
    por_origem: HashMap<IpAddr, (Instant, u32)>,
    /// Anuncios que chegaram a abrir o selo (os testes provam o «custa zero»).
    pub abertos: u64,
}

impl Descoberta {
    pub fn nova(psk: &[u8; 32], ligada: bool) -> Descoberta {
        Descoberta {
            chave: chave_do_anuncio(psk),
            ligada,
            ultimo_envio: None,
            vistos: HashMap::new(),
            janela: Instant::now(),
            na_janela: 0,
            por_origem: HashMap::new(),
            abertos: 0,
        }
    }

    pub fn ligada(&self) -> bool {
        self.ligada
    }

    /// O anuncio deste no, se ja deu a hora (ou `None`).
    pub fn anuncio_se_ja_e_hora(&mut self, minha_publica: &[u8; 32]) -> Option<Vec<u8>> {
        if !self.ligada
            || self
                .ultimo_envio
                .is_some_and(|t| t.elapsed() < ANUNCIAR_A_CADA)
        {
            return None;
        }
        self.ultimo_envio = Some(Instant::now());
        Some(anuncio(&self.chave, minha_publica, agora_ms()))
    }

    /// Confere um anuncio que chegou de `de`; devolve a publica do membro
    /// que anunciou. Nao diz nada sobre ser membro -- isso e de quem chama.
    pub fn abrir(&mut self, dado: &[u8], de: SocketAddr) -> Option<[u8; 32]> {
        if !self.ligada {
            return None;
        }
        if dado.len() != TAM_ANUNCIO || dado[..4] != CABECALHO {
            return None;
        }
        if !self.dentro_do_teto(de.ip()) {
            return None;
        }
        self.abertos += 1;
        let nonce: [u8; 24] = dado[4..28].try_into().ok()?;
        let fim = TAM_ANUNCIO - 16;
        let tag: [u8; 16] = dado[fim..].try_into().ok()?;
        let claro = xabrir(&self.chave, &nonce, &CABECALHO, &dado[28..fim], &tag).ok()?;
        let carimbo = u64::from_be_bytes(claro[..8].try_into().ok()?);
        let publica: [u8; 32] = claro[8..40].try_into().ok()?;
        if agora_ms().abs_diff(carimbo) > JANELA_S * 1000 {
            return None;
        }
        // Repeticao: um anuncio gravado e reenviado (de outro IP, forjado)
        // mudaria o endereco do par para onde o atacante quer.
        if self.vistos.get(&publica).is_some_and(|&v| carimbo <= v) {
            return None;
        }
        if self.vistos.len() >= crate::rol::TETO_MEMBROS {
            self.vistos.clear();
        }
        self.vistos.insert(publica, carimbo);
        Some(publica)
    }

    fn dentro_do_teto(&mut self, ip: IpAddr) -> bool {
        if self.janela.elapsed() >= Duration::from_secs(1) {
            self.janela = Instant::now();
            self.na_janela = 0;
        }
        self.na_janela += 1;
        if self.na_janela > TETO_POR_S {
            return false;
        }
        if self.por_origem.len() >= TETO_ORIGENS {
            self.por_origem
                .retain(|_, (t, _)| t.elapsed() < Duration::from_secs(1));
        }
        let (t, n) = self.por_origem.entry(ip).or_insert((Instant::now(), 0));
        if t.elapsed() >= Duration::from_secs(1) {
            *t = Instant::now();
            *n = 0;
        }
        *n += 1;
        *n <= TETO_POR_ORIGEM_POR_S
    }
}

fn anuncio(chave: &[u8; 32], publica: &[u8; 32], carimbo_ms: u64) -> Vec<u8> {
    let mut claro = [0u8; CLARO];
    claro[..8].copy_from_slice(&carimbo_ms.to_be_bytes());
    claro[8..40].copy_from_slice(publica);
    let nonce: [u8; 24] = bytes_aleatorios(24).try_into().expect("24");
    let (c, tag) = xselar(chave, &nonce, &CABECALHO, &claro);
    let mut p = CABECALHO.to_vec();
    p.extend_from_slice(&nonce);
    p.extend_from_slice(&c);
    p.extend_from_slice(&tag);
    p
}

/// Para onde o anuncio vai: o broadcast dirigido de cada interface com
/// broadcast (Linux, `getifaddrs`) e o limitado `255.255.255.255`.
///
/// O limitado sozinho NAO basta no Linux: sem rota padrao ele da «Network is
/// unreachable» -- medido num par de `netns` ligados so por uma bridge, que e
/// exatamente a LAN sem roteador que esta descoberta existe para cobrir. No
/// Windows cada interface tem a rota `255.255.255.255` no proprio enlace, e o
/// limitado serve.
pub fn destinos(porta: u16) -> Vec<SocketAddr> {
    let mut v: Vec<SocketAddr> = broadcasts_das_interfaces()
        .into_iter()
        .map(|ip| SocketAddr::from((ip, porta)))
        .collect();
    v.push(SocketAddr::from((Ipv4Addr::BROADCAST, porta)));
    v
}

#[cfg(target_os = "linux")]
fn broadcasts_das_interfaces() -> Vec<Ipv4Addr> {
    // `struct ifaddrs` da glibc/musl: a mesma disposicao em 64 e 32 bits
    // (so ponteiros e um `unsigned int`).
    #[repr(C)]
    struct IfAddrs {
        prox: *mut IfAddrs,
        nome: *const i8,
        flags: u32,
        endereco: *const SockAddrIn,
        mascara: *const SockAddrIn,
        _broadcast: *const SockAddrIn,
        _dados: *mut u8,
    }
    #[repr(C)]
    struct SockAddrIn {
        familia: u16,
        porta: u16,
        ip: [u8; 4],
    }
    extern "C" {
        fn getifaddrs(lista: *mut *mut IfAddrs) -> i32;
        fn freeifaddrs(lista: *mut IfAddrs);
    }
    const AF_INET: u16 = 2;
    const IFF_UP: u32 = 0x1;
    const IFF_BROADCAST: u32 = 0x2;
    const IFF_LOOPBACK: u32 = 0x8;
    let mut v = Vec::new();
    let mut lista: *mut IfAddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs preenche `lista` com uma lista ligada que so e lida
    // aqui e devolvida inteira ao freeifaddrs; cada ponteiro e conferido
    // contra nulo antes de ser lido, e a familia antes do resto do sockaddr.
    unsafe {
        if getifaddrs(&mut lista) != 0 {
            return v;
        }
        let mut p = lista;
        while !p.is_null() {
            let i = &*p;
            let quer = i.flags & IFF_UP != 0
                && i.flags & IFF_BROADCAST != 0
                && i.flags & IFF_LOOPBACK == 0;
            // O broadcast sai de endereco | !mascara, e nao do `ifa_broadaddr`:
            // `ip addr add` sem `brd` deixa o campo com o PROPRIO endereco --
            // medido no netns da prova, o anuncio ia para 192.168.77.4 em vez
            // de .255 e ninguem o ouvia.
            if quer && !i.endereco.is_null() && !i.mascara.is_null() {
                let (e, m) = (&*i.endereco, &*i.mascara);
                if e.familia == AF_INET && m.familia == AF_INET {
                    let ip = Ipv4Addr::from(u32::from_be_bytes(e.ip) | !u32::from_be_bytes(m.ip));
                    if !v.contains(&ip) {
                        v.push(ip);
                    }
                }
            }
            p = i.prox;
        }
        freeifaddrs(lista);
    }
    v
}

#[cfg(not(target_os = "linux"))]
fn broadcasts_das_interfaces() -> Vec<Ipv4Addr> {
    Vec::new()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn de() -> SocketAddr {
        "192.168.77.2:51820".parse().unwrap()
    }

    #[test]
    fn anuncio_abre_com_a_senha_e_nao_diz_nada_sem() {
        let psk = [1u8; 32];
        let mut a = Descoberta::nova(&psk, true);
        let mut b = Descoberta::nova(&psk, true);
        let p = a.anuncio_se_ja_e_hora(&[7; 32]).unwrap();
        assert_eq!(p.len(), TAM_ANUNCIO);
        assert!(a.anuncio_se_ja_e_hora(&[7; 32]).is_none(), "cedo demais");
        assert_eq!(b.abrir(&p, de()), Some([7; 32]));
        // Nem a chave do membro nem nada reconhecivel aparece em claro.
        assert!(!p.windows(32).any(|w| w == [7; 32]));
        let mut outra = Descoberta::nova(&[2; 32], true);
        assert_eq!(outra.abrir(&p, de()), None, "senha errada");
    }

    #[test]
    fn anuncio_adulterado_e_recusado() {
        let mut b = Descoberta::nova(&[1; 32], true);
        let p = anuncio(&chave_do_anuncio(&[1; 32]), &[7; 32], agora_ms());
        for i in [0usize, 5, 40, TAM_ANUNCIO - 1] {
            let mut t = p.clone();
            t[i] ^= 1;
            assert_eq!(b.abrir(&t, de()), None, "byte {i}");
        }
    }

    /// Anuncio gravado e reenviado nao vale de novo, nem de outro IP.
    #[test]
    fn anuncio_repetido_e_recusado() {
        let mut b = Descoberta::nova(&[1; 32], true);
        let p = anuncio(&chave_do_anuncio(&[1; 32]), &[7; 32], agora_ms());
        assert_eq!(b.abrir(&p, de()), Some([7; 32]));
        assert_eq!(b.abrir(&p, "192.168.77.9:51820".parse().unwrap()), None);
    }

    #[test]
    fn anuncio_fora_da_janela_e_recusado() {
        let mut b = Descoberta::nova(&[1; 32], true);
        let velho = agora_ms() - (JANELA_S + 5) * 1000;
        let p = anuncio(&chave_do_anuncio(&[1; 32]), &[7; 32], velho);
        assert_eq!(b.abrir(&p, de()), None);
    }

    /// Desligada: nem o selo se abre. E o teto por origem corta ANTES do selo.
    #[test]
    fn desligada_nao_abre_nada_e_teto_vem_antes_do_selo() {
        let psk = [1u8; 32];
        let p = anuncio(&chave_do_anuncio(&psk), &[7; 32], agora_ms());
        let mut d = Descoberta::nova(&psk, false);
        assert!(d.anuncio_se_ja_e_hora(&[7; 32]).is_none());
        assert_eq!(d.abrir(&p, de()), None);
        assert_eq!(d.abertos, 0, "desligada abriu selo");
        let mut l = Descoberta::nova(&psk, true);
        for _ in 0..20 {
            l.abrir(&p, de());
        }
        assert_eq!(l.abertos as u32, TETO_POR_ORIGEM_POR_S);
    }

    /// A resposta a um anuncio e um INICIO; ele nunca pode ser maior.
    #[test]
    fn resposta_nunca_maior_que_o_pedido() {
        use crate::{noise, transporte};
        let (ka, kb) = (
            phxsql_core::x25519::gerar_privada(),
            phxsql_core::x25519::gerar_privada(),
        );
        let pb = phxsql_core::x25519::chave_publica(&kb);
        let mut carga = transporte::carimbo_agora().to_vec();
        carga.extend_from_slice(&[9; 16]); // ficha de convite
        carga.extend_from_slice(&[b'x'; crate::rol::TETO_NOME]); // apelido
        let (_, m1) = noise::Iniciador::comecar(noise::PROLOGO, ka, &pb, [3; 32], &carga).unwrap();
        let ini = transporte::embrulhar_inicio(1, &m1, &pb, Some(&[1; 16]));
        assert!(
            ini.len() <= TAM_ANUNCIO,
            "INICIO de {} bytes > anuncio de {TAM_ANUNCIO}",
            ini.len()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn destinos_tem_o_limitado_e_nao_tem_loopback() {
        let d = destinos(51820);
        assert!(d.contains(&"255.255.255.255:51820".parse().unwrap()));
        assert!(!d.iter().any(|a| a.ip().is_loopback()));
    }
}
