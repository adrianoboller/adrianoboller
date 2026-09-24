//! Difusao na rede P2P: broadcast e multicast da placa replicados aos pares.
//!
//! Jogos de LAN e descoberta de servico (SSDP 239.255.255.250, mDNS
//! 224.0.0.251, NetBIOS e o broadcast de jogo na porta X) so funcionam se o
//! pacote que a aplicacao manda a «todo mundo» chega a todo mundo. Antes
//! disto o no descartava tudo o que nao ia ao IP de um par: medido, 0 de 15
//! broadcasts/multicasts chegavam (`docs/propostas/lacunas-openvpn-fonte-*`,
//! M2). E o nicho do Radmin.
//!
//! # O que replica
//!
//! Pacote IPv4 da PROPRIA placa, com origem no PROPRIO IP virtual, para
//! `255.255.255.255`, para o broadcast da sub-rede da VPN, ou para
//! `224.0.0.0/4`. Cada copia vai cifrada na sessao de cada par, como o
//! unicast -- par sem sessao pronta nao ganha aperto por causa de um anuncio
//! (o tique ja abre sessao com quem tem endereco).
//!
//! # Decisoes, e de onde vieram
//!
//! - **Sem laco por construcao.** Replica-se so o que tem origem no IP deste
//!   no. O que chegou de um par e entregue a placa com a origem DELE, e se o
//!   sistema o devolvesse a placa (nao devolve: broadcast nao se roteia), a
//!   origem alheia o barraria aqui. Nao ha tabela de «ja vi», que teria teto
//!   e prazo e erraria nas bordas.
//! - **Escopo de enlace ATRAVESSA.** `224.0.0.0/24` (mDNS, LLMNR) e o proprio
//!   `255.255.255.255` sao de enlace, e a VPN e o enlace para as aplicacoes:
//!   o ZeroTier emula um switch Ethernet e leva tudo (`Switch.cpp`, ramo
//!   `to.isMulticast()`), e o Radmin existe para isso. O TTL 1 de quem manda
//!   continua valendo: a copia nao passa por roteador nenhum.
//! - **IGMP morre.** Nao ha roteador multicast na malha para ouvi-lo, e
//!   replicar relatorio de grupo a N pares so gasta banda.
//! - **IPv6.** `ff01::/16` (escopo de interface) nunca sai da maquina, pela
//!   RFC 4291 §2.7. O resto de `ff00::/8` fica classificado mas NAO replica:
//!   o tunel P2P ainda nao leva IPv6 (a entrada confere origem IPv4), e
//!   replicar o que o outro lado joga fora seria banda por nada. No dia em
//!   que levar, e aqui que muda, com a origem conferida contra o IPv6 do no.
//! - **Tetos contra amplificacao**, por no de origem: `TETO_PACOTES_POR_S` e
//!   `TETO_BYTES_POR_S` num balde de fichas (rajada de ate um segundo), e
//!   `TETO_TAMANHO`. Na saida protegem o uplink de quem manda (cada pacote
//!   vira N); na ENTRADA, por par, protegem quem recebe de um membro com
//!   binario alterado que ignore o proprio teto. O ZeroTier limita o numero
//!   de destinatarios (`multicastLimit`) e nao a taxa; aqui a malha inteira
//!   ja e o conjunto de destinatarios, entao o que cresce sem teto e a taxa.
//! - **Desliga por rede** (`"difusao": false` no `.p2p`, `--sem-difusao`),
//!   como o `enableBroadcast` do ZeroTier. Desligada, nao sai E nao entra: a
//!   escolha vale mesmo que o outro lado a tenha ligada.
//! - **O Tailscale nao repassa** broadcast nem multicast (referencia da
//!   pauta, nao conferida no fonte nesta rodada).
//!
//! # Windows
//!
//! O TAP-Windows6 em modo TUN NAO entrega broadcast nem multicast IPv4 ao
//! programa: `txpath.c` so enfileira o quadro cujo cabecalho Ethernet e o do
//! par ponto-a-ponto («Only accept directed packets, not broadcasts»). Entao
//! um no Windows RECEBE a difusao dos outros (a escrita vira quadro dirigido
//! ao adaptador) mas nao ORIGINA -- originar pede o TAP em modo Ethernet,
//! outra frente. Nada disto rodou num Windows ainda.

use super::{No, MTU};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

/// Pacotes de difusao por segundo, por no de origem. Descoberta e anuncio
/// de jogo sao poucos por segundo; a rajada do mDNS ao ligar, dezenas.
pub const TETO_PACOTES_POR_S: u32 = 200;
/// Bytes de difusao por segundo, por no de origem (antes de replicar).
pub const TETO_BYTES_POR_S: u32 = 256 * 1024;
/// Maior pacote de difusao: o da placa nunca passa do MTU; na entrada, o que
/// passar veio de binario alterado.
pub const TETO_TAMANHO: usize = MTU as usize;
/// Aviso de corte no log, no maximo um por este prazo.
const AVISO_A_CADA: Duration = Duration::from_secs(60);

/// A sub-rede da VPN: o IP do no e o prefixo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subrede {
    pub ip: Ipv4Addr,
    pub prefixo: u8,
}

impl Subrede {
    /// Broadcast dirigido da sub-rede; `/31` e `/32` nao tem.
    pub fn broadcast(&self) -> Option<Ipv4Addr> {
        (self.prefixo <= 30).then(|| {
            let host = u32::MAX.checked_shr(self.prefixo as u32).unwrap_or(0);
            Ipv4Addr::from(u32::from(self.ip) | host)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motivo {
    /// Origem que nao e a esperada (na saida: o proprio IP).
    OrigemAlheia,
    Igmp,
    Grande,
    /// `ff01::/16`: escopo de interface.
    EscopoInterface,
    /// Multicast IPv6 enquanto o tunel nao leva IPv6.
    Ipv6NaoLevado,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classe {
    /// Nao e difusao: segue o caminho de sempre.
    Unicast,
    Difusao,
    Descartar(Motivo),
}

/// Classifica um pacote IP. `origem` e quem tem de ter mandado: na saida, o
/// proprio no; na entrada, o par autenticado.
pub fn classificar(pacote: &[u8], sub: Subrede, origem: Ipv4Addr) -> Classe {
    match pacote.first().map(|b| b >> 4) {
        Some(4) if pacote.len() >= 20 => {}
        Some(6) if pacote.len() >= 40 => {
            if pacote[24] != 0xff {
                return Classe::Unicast;
            }
            return match pacote[25] & 0x0f {
                0 | 1 => Classe::Descartar(Motivo::EscopoInterface),
                _ => Classe::Descartar(Motivo::Ipv6NaoLevado),
            };
        }
        _ => return Classe::Unicast,
    }
    let destino = Ipv4Addr::new(pacote[16], pacote[17], pacote[18], pacote[19]);
    let difusao =
        destino.is_broadcast() || destino.is_multicast() || sub.broadcast() == Some(destino);
    if !difusao {
        return Classe::Unicast;
    }
    let de = Ipv4Addr::new(pacote[12], pacote[13], pacote[14], pacote[15]);
    if de != origem {
        return Classe::Descartar(Motivo::OrigemAlheia);
    }
    if pacote[9] == 2 {
        return Classe::Descartar(Motivo::Igmp);
    }
    if pacote.len() > TETO_TAMANHO {
        return Classe::Descartar(Motivo::Grande);
    }
    Classe::Difusao
}

/// Balde de fichas de pacotes e de bytes: cheio vale um segundo de teto.
#[derive(Clone, Debug)]
pub struct Balde {
    pacotes: f64,
    bytes: f64,
    quando: Instant,
}

impl Balde {
    pub fn cheio(agora: Instant) -> Balde {
        Balde {
            pacotes: TETO_PACOTES_POR_S as f64,
            bytes: TETO_BYTES_POR_S as f64,
            quando: agora,
        }
    }

    /// Tira um pacote de `tamanho` bytes; `false` = acima do teto, cai.
    pub fn tirar(&mut self, tamanho: usize, agora: Instant) -> bool {
        let dt = agora.saturating_duration_since(self.quando).as_secs_f64();
        self.quando = self.quando.max(agora);
        let (tp, tb) = (TETO_PACOTES_POR_S as f64, TETO_BYTES_POR_S as f64);
        self.pacotes = (self.pacotes + dt * tp).min(tp);
        self.bytes = (self.bytes + dt * tb).min(tb);
        if self.pacotes < 1.0 || self.bytes < tamanho as f64 {
            return false;
        }
        self.pacotes -= 1.0;
        self.bytes -= tamanho as f64;
        true
    }
}

/// O que a difusao fez desde que o no ligou. As provas leem daqui.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contagem {
    /// Pacotes da placa que passaram o teto e foram replicados.
    pub replicados: u64,
    /// Copias cifradas mandadas (uma por par pronto).
    pub copias: u64,
    /// Da placa, acima do teto.
    pub cortados_saida: u64,
    /// Da placa, descartados pela classificacao ou com a difusao desligada.
    pub descartados_saida: u64,
    /// De pares, entregues a placa.
    pub aceitos: u64,
    /// De pares, acima do teto daquele par.
    pub cortados_entrada: u64,
    /// De pares, descartados pela classificacao ou com a difusao desligada.
    pub descartados_entrada: u64,
}

/// O que o `No` guarda da difusao. A parte imutavel (ligada, sub-rede) fica
/// fora da trava: o unicast so paga a classificacao, nunca a trava.
pub struct Difusao {
    pub(super) ligada: bool,
    pub(super) sub: Subrede,
    pub(super) mutavel: std::sync::Mutex<Mutavel>,
}

pub struct Mutavel {
    saida: Balde,
    /// Um balde por par (chave = IP virtual do par: limitado pelo numero de
    /// pares, que ja tem teto).
    entrada: HashMap<Ipv4Addr, Balde>,
    contagem: Contagem,
    ultimo_aviso: Option<Instant>,
}

impl Difusao {
    /// Sem `com_difusao`: desligada, e sem prefixo conhecido.
    pub fn desligada(ip: Ipv4Addr) -> Difusao {
        Difusao::nova(false, Subrede { ip, prefixo: 32 })
    }

    pub fn nova(ligada: bool, sub: Subrede) -> Difusao {
        Difusao {
            ligada,
            sub,
            mutavel: std::sync::Mutex::new(Mutavel {
                saida: Balde::cheio(Instant::now()),
                entrada: HashMap::new(),
                contagem: Contagem::default(),
                ultimo_aviso: None,
            }),
        }
    }

    fn avisar_corte(m: &mut Mutavel, de: &str) {
        if m.ultimo_aviso.is_some_and(|t| t.elapsed() < AVISO_A_CADA) {
            return;
        }
        m.ultimo_aviso = Some(Instant::now());
        eprintln!(
            "phxvpn: difusao {de} acima do teto ({TETO_PACOTES_POR_S} pacotes/s, {} KiB/s): o excedente cai",
            TETO_BYTES_POR_S / 1024
        );
    }
}

impl No {
    /// Liga (ou nao) a difusao, com o prefixo da sub-rede da placa.
    pub fn com_difusao(mut self, ligada: bool, prefixo: u8) -> No {
        self.difusao = Difusao::nova(
            ligada,
            Subrede {
                ip: self.ip,
                prefixo,
            },
        );
        self
    }

    pub fn difusao_contagem(&self) -> Contagem {
        self.difusao.mutavel.lock().expect("difusao").contagem
    }

    /// Gancho do `da_placa`. `true` = o pacote era difusao (replicado ou
    /// descartado) e o caminho do unicast nao deve ve-lo.
    pub(super) fn difusao_da_placa(&self, pacote: &[u8]) -> bool {
        let d = &self.difusao;
        // Portao antes do trabalho: desligada, o unicast segue sem nem
        // classificar -- o caminho de sempre ja descarta o que nao e de par.
        if !d.ligada {
            return false;
        }
        match classificar(pacote, d.sub, self.ip) {
            Classe::Unicast => return false,
            Classe::Descartar(_) => {
                d.mutavel
                    .lock()
                    .expect("difusao")
                    .contagem
                    .descartados_saida += 1;
                return true;
            }
            Classe::Difusao => {}
        }
        {
            let mut m = d.mutavel.lock().expect("difusao");
            if !m.saida.tirar(pacote.len(), Instant::now()) {
                m.contagem.cortados_saida += 1;
                Difusao::avisar_corte(&mut m, "da placa");
                return true;
            }
        }
        let mut saidas = Vec::new();
        let mut e = self.estado.lock().expect("estado");
        for par in e.pares.iter_mut() {
            let Some(via) = par.via else { continue };
            let Some(s) = par
                .atual
                .as_mut()
                .filter(|s| s.confirmada && !s.expirada() && !s.pede_novo_aperto())
            else {
                continue;
            };
            if let Ok(p) = s.selar(pacote) {
                // Sem `sem_resposta_desde`: anuncio nao pede resposta, e
                // marca-lo faria o par calado parecer surdo e refazer o
                // aperto a cada 15 s.
                par.ultimo_envio = Instant::now();
                saidas.push((via, par.publica, p));
            }
        }
        drop(e);
        {
            let mut m = d.mutavel.lock().expect("difusao");
            m.contagem.replicados += 1;
            m.contagem.copias += saidas.len() as u64;
        }
        for (via, chave, p) in saidas {
            self.mandar(via, &chave, &p);
        }
        true
    }

    /// Gancho do `receber_dados`, depois de a origem ser conferida: `true` =
    /// pode entrar na placa.
    pub(super) fn difusao_do_par(&self, claro: &[u8], ip_do_par: Ipv4Addr) -> bool {
        let d = &self.difusao;
        let classe = classificar(claro, d.sub, ip_do_par);
        if classe == Classe::Unicast {
            return true;
        }
        let mut m = d.mutavel.lock().expect("difusao");
        if classe != Classe::Difusao || !d.ligada {
            m.contagem.descartados_entrada += 1;
            return false;
        }
        let agora = Instant::now();
        let passa = m
            .entrada
            .entry(ip_do_par)
            .or_insert_with(|| Balde::cheio(agora))
            .tirar(claro.len(), agora);
        if passa {
            m.contagem.aceitos += 1;
        } else {
            m.contagem.cortados_entrada += 1;
            Difusao::avisar_corte(&mut m, &format!("de {ip_do_par}"));
        }
        passa
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const EU: Ipv4Addr = Ipv4Addr::new(10, 78, 0, 1);
    const SUB: Subrede = Subrede {
        ip: EU,
        prefixo: 24,
    };

    fn v4(origem: Ipv4Addr, destino: Ipv4Addr, protocolo: u8, tamanho: usize) -> Vec<u8> {
        let mut p = vec![0u8; tamanho.max(20)];
        p[0] = 0x45;
        p[9] = protocolo;
        p[12..16].copy_from_slice(&origem.octets());
        p[16..20].copy_from_slice(&destino.octets());
        p
    }

    fn v6(destino0: u8, destino1: u8) -> Vec<u8> {
        let mut p = vec![0u8; 48];
        p[0] = 0x60;
        p[24] = destino0;
        p[25] = destino1;
        p
    }

    #[test]
    fn broadcast_e_multicast_ipv4_da_propria_origem_replicam() {
        for d in [
            "255.255.255.255",
            "10.78.0.255",
            "224.0.0.251",
            "239.255.255.250",
            "224.0.0.252",
        ] {
            let p = v4(EU, d.parse().unwrap(), 17, 60);
            assert_eq!(classificar(&p, SUB, EU), Classe::Difusao, "{d}");
        }
    }

    #[test]
    fn unicast_e_broadcast_de_outra_subrede_nao_sao_difusao() {
        for d in ["10.78.0.2", "10.78.1.255", "192.168.0.255", "8.8.8.8"] {
            let p = v4(EU, d.parse().unwrap(), 17, 60);
            assert_eq!(classificar(&p, SUB, EU), Classe::Unicast, "{d}");
        }
        // Lixo e o que nao e IP seguem o caminho de sempre (que os descarta).
        assert_eq!(classificar(&[], SUB, EU), Classe::Unicast);
        assert_eq!(classificar(&[0x45; 10], SUB, EU), Classe::Unicast);
    }

    #[test]
    fn broadcast_dirigido_segue_o_prefixo() {
        let s16 = Subrede {
            ip: EU,
            prefixo: 16,
        };
        let p = v4(EU, "10.78.255.255".parse().unwrap(), 17, 60);
        assert_eq!(classificar(&p, s16, EU), Classe::Difusao);
        assert_eq!(classificar(&p, SUB, EU), Classe::Unicast);
        // /31 e /32 nao tem broadcast dirigido.
        for prefixo in [31, 32] {
            assert_eq!(Subrede { ip: EU, prefixo }.broadcast(), None);
        }
        assert_eq!(
            Subrede { ip: EU, prefixo: 0 }.broadcast(),
            Some(Ipv4Addr::BROADCAST)
        );
    }

    /// A guarda do laco: origem alheia nunca replica. Tirada a conferencia,
    /// um pacote que voltasse do sistema seria reenviado a malha.
    #[test]
    fn origem_alheia_nao_replica() {
        let alheio = Ipv4Addr::new(10, 78, 0, 2);
        let p = v4(alheio, Ipv4Addr::BROADCAST, 17, 60);
        assert_eq!(
            classificar(&p, SUB, EU),
            Classe::Descartar(Motivo::OrigemAlheia)
        );
        // Na entrada a origem esperada e o par: a mesma funcao aceita.
        assert_eq!(classificar(&p, SUB, alheio), Classe::Difusao);
    }

    #[test]
    fn igmp_e_grande_caem() {
        let igmp = v4(EU, "224.0.0.22".parse().unwrap(), 2, 40);
        assert_eq!(classificar(&igmp, SUB, EU), Classe::Descartar(Motivo::Igmp));
        let grande = v4(EU, Ipv4Addr::BROADCAST, 17, TETO_TAMANHO + 1);
        assert_eq!(
            classificar(&grande, SUB, EU),
            Classe::Descartar(Motivo::Grande)
        );
        let no_teto = v4(EU, Ipv4Addr::BROADCAST, 17, TETO_TAMANHO);
        assert_eq!(classificar(&no_teto, SUB, EU), Classe::Difusao);
    }

    #[test]
    fn ipv6_multicast_classificado_e_escopo_de_interface_morre() {
        assert_eq!(
            classificar(&v6(0xff, 0x01), SUB, EU),
            Classe::Descartar(Motivo::EscopoInterface)
        );
        assert_eq!(
            classificar(&v6(0xff, 0x02), SUB, EU),
            Classe::Descartar(Motivo::Ipv6NaoLevado)
        );
        assert_eq!(classificar(&v6(0xfd, 0x00), SUB, EU), Classe::Unicast);
    }

    #[test]
    fn balde_corta_acima_do_teto_de_pacotes_e_volta_com_o_tempo() {
        let t0 = Instant::now();
        let mut b = Balde::cheio(t0);
        let passaram = (0..1000).filter(|_| b.tirar(60, t0)).count();
        assert_eq!(passaram, TETO_PACOTES_POR_S as usize);
        // Meio segundo depois, metade do teto de volta.
        let t1 = t0 + Duration::from_millis(500);
        let depois = (0..1000).filter(|_| b.tirar(60, t1)).count();
        assert_eq!(depois, TETO_PACOTES_POR_S as usize / 2);
    }

    #[test]
    fn balde_corta_acima_do_teto_de_bytes() {
        let t0 = Instant::now();
        let mut b = Balde::cheio(t0);
        let passaram = (0..1000).filter(|_| b.tirar(TETO_TAMANHO, t0)).count();
        assert_eq!(passaram, TETO_BYTES_POR_S as usize / TETO_TAMANHO);
        assert!(passaram < TETO_PACOTES_POR_S as usize);
    }

    use crate::p2p::{psk_da_rede, ParConfig};
    use phxsql_core::x25519;
    use std::net::{SocketAddr, UdpSocket};

    struct Tres {
        a: No,
        b: No,
        c: No,
    }

    /// A conhece B e C pelo endereco; B e C conhecem A. `liga` = difusao de
    /// A, B e C.
    fn tres(liga: [bool; 3]) -> Tres {
        let k: Vec<[u8; 32]> = (0..3).map(|_| x25519::gerar_privada()).collect();
        let u: Vec<UdpSocket> = (0..3)
            .map(|_| {
                let u = UdpSocket::bind("127.0.0.1:0").unwrap();
                u.set_read_timeout(Some(Duration::from_millis(100)))
                    .unwrap();
                u
            })
            .collect();
        let e: Vec<SocketAddr> = u.iter().map(|u| u.local_addr().unwrap()).collect();
        let ip = |i: usize| Ipv4Addr::new(10, 78, 0, i as u8 + 1);
        let par = |i: usize, com_endereco: bool| ParConfig {
            publica: x25519::chave_publica(&k[i]),
            ip: ip(i),
            endereco: com_endereco.then_some(e[i]),
        };
        let psk = psk_da_rede("R", "s", 1_000);
        let mut u = u.into_iter();
        let a = No::novo(
            k[0],
            psk,
            ip(0),
            u.next().unwrap(),
            vec![par(1, true), par(2, true)],
        );
        let b = No::novo(k[1], psk, ip(1), u.next().unwrap(), vec![par(0, false)]);
        let c = No::novo(k[2], psk, ip(2), u.next().unwrap(), vec![par(0, false)]);
        Tres {
            a: a.com_difusao(liga[0], 24),
            b: b.com_difusao(liga[1], 24),
            c: c.com_difusao(liga[2], 24),
        }
    }

    fn pacote(origem: Ipv4Addr, destino: Ipv4Addr, carga: &[u8]) -> Vec<u8> {
        let mut p = v4(origem, destino, 17, 20);
        p.extend_from_slice(carga);
        p
    }

    /// Bombeia ate sair um pacote na placa, ou ate o soquete calar.
    fn ate_a_placa(no: &No) -> Option<Vec<u8>> {
        for _ in 0..16 {
            let mut buf = vec![0u8; 4096];
            let (n, de) = no.udp.recv_from(&mut buf).ok()?;
            if let Some(p) = no.da_rede(&buf[..n], de) {
                return Some(p);
            }
        }
        None
    }

    /// Esvazia o soquete de `no` (ate calar) e devolve o que foi a placa.
    fn drenar(no: &No) -> Vec<Vec<u8>> {
        let mut placa = Vec::new();
        let mut buf = vec![0u8; 4096];
        while let Ok((n, de)) = no.udp.recv_from(&mut buf) {
            placa.extend(no.da_rede(&buf[..n], de));
        }
        placa
    }

    /// Abre as sessoes A-B e A-C por um unicast de A a cada um, e gira ate
    /// a malha calar (listas de pares vao e voltam no meio).
    fn conectar(t: &Tres) {
        let ida_b = pacote(EU, Ipv4Addr::new(10, 78, 0, 2), b"oi");
        let ida_c = pacote(EU, Ipv4Addr::new(10, 78, 0, 3), b"oi");
        t.a.da_placa(&ida_b);
        t.a.da_placa(&ida_c);
        let (mut pb, mut pc) = (Vec::new(), Vec::new());
        for _ in 0..3 {
            pb.extend(drenar(&t.b));
            pc.extend(drenar(&t.c));
            drenar(&t.a);
        }
        assert_eq!((pb, pc), (vec![ida_b], vec![ida_c]));
    }

    /// A prova do caminho inteiro por UDP real: o broadcast da placa de A
    /// chega as placas de B e de C, cada um pela propria sessao -- e o que B
    /// devolve a propria placa com a origem de A NAO volta a malha.
    #[test]
    fn difusao_chega_aos_dois_pares_e_nao_faz_laco() {
        let t = tres([true; 3]);
        conectar(&t);
        for destino in [
            Ipv4Addr::BROADCAST,
            Ipv4Addr::new(10, 78, 0, 255),
            Ipv4Addr::new(239, 255, 255, 250),
        ] {
            let bc = pacote(EU, destino, b"quem joga?");
            t.a.da_placa(&bc);
            assert_eq!(ate_a_placa(&t.b).as_ref(), Some(&bc), "B {destino}");
            assert_eq!(ate_a_placa(&t.c).as_ref(), Some(&bc), "C {destino}");
        }
        let ca = t.a.difusao_contagem();
        assert_eq!((ca.replicados, ca.copias), (3, 6));
        assert_eq!(t.b.difusao_contagem().aceitos, 3);
        // O sistema de B devolvendo a placa o que veio de A: nao replica.
        t.b.da_placa(&pacote(EU, Ipv4Addr::BROADCAST, b"eco"));
        let cb = t.b.difusao_contagem();
        assert_eq!((cb.replicados, cb.copias, cb.descartados_saida), (0, 0, 1));
        assert_eq!(ate_a_placa(&t.a), None, "nada volta a placa de A");
    }

    /// Desligada num lado, 0 passa -- nos dois sentidos.
    #[test]
    fn desligada_nao_sai_nem_entra() {
        let t = tres([true, false, true]);
        conectar(&t);
        let bc = pacote(EU, Ipv4Addr::BROADCAST, b"x");
        t.a.da_placa(&bc);
        assert_eq!(ate_a_placa(&t.b), None, "B desligada recusa");
        assert_eq!(ate_a_placa(&t.c).as_ref(), Some(&bc), "C ligada aceita");
        assert_eq!(t.b.difusao_contagem().descartados_entrada, 1);
        let t = tres([false, true, true]);
        conectar(&t);
        t.a.da_placa(&bc);
        assert_eq!(t.a.difusao_contagem(), Contagem::default());
        assert_eq!(ate_a_placa(&t.b), None);
        assert_eq!(ate_a_placa(&t.c), None);
    }

    /// Rajada da placa acima do teto: o excedente cai antes de cifrar.
    #[test]
    fn rajada_da_placa_acima_do_teto_cai() {
        let t = tres([true; 3]);
        conectar(&t);
        let bc = pacote(EU, Ipv4Addr::BROADCAST, b"r");
        for _ in 0..(TETO_PACOTES_POR_S * 2) {
            t.a.da_placa(&bc);
        }
        let c = t.a.difusao_contagem();
        let teto = TETO_PACOTES_POR_S as u64;
        // A rajada leva milissegundos: volta no maximo uma ficha ou outra.
        assert!((teto..teto + 5).contains(&c.replicados), "{c:?}");
        assert_eq!(c.replicados + c.cortados_saida, 2 * teto);
        assert_eq!(c.copias, 2 * c.replicados);
    }

    /// Par com binario alterado que ignora o proprio teto: quem recebe corta.
    #[test]
    fn entrada_acima_do_teto_por_par_cai() {
        let t = tres([true; 3]);
        let de = Ipv4Addr::new(10, 78, 0, 2);
        let bc = pacote(de, Ipv4Addr::BROADCAST, b"r");
        let passaram = (0..TETO_PACOTES_POR_S * 2)
            .filter(|_| t.a.difusao_do_par(&bc, de))
            .count() as u64;
        let teto = TETO_PACOTES_POR_S as u64;
        assert!((teto..teto + 5).contains(&passaram), "{passaram}");
        // O balde e POR par: o outro par nao paga pelo primeiro.
        let outro = Ipv4Addr::new(10, 78, 0, 3);
        assert!(t
            .a
            .difusao_do_par(&pacote(outro, Ipv4Addr::BROADCAST, b"r"), outro));
        // Unicast nao passa pelo balde.
        let uni = pacote(de, EU, b"u");
        assert!((0..1000).all(|_| t.a.difusao_do_par(&uni, de)));
    }

    /// Relogio que volta (instante antigo) nao cria fichas.
    #[test]
    fn balde_nao_ganha_fichas_com_instante_velho() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_secs(1);
        let mut b = Balde::cheio(t1);
        let passaram = (0..1000).filter(|_| b.tirar(60, t0)).count();
        assert_eq!(passaram, TETO_PACOTES_POR_S as usize);
        // Se o instante velho tivesse recuado o relogio do balde, voltar a
        // `t1` daria um segundo inteiro de fichas de graca.
        assert!(!b.tirar(60, t1));
    }
}
