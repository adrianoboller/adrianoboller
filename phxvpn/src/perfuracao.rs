//! Perfuracao de NAT mediada pelo repasse (UDP hole punching).
//!
//! Dois membros atras de NATs diferentes ja conversam pelo repasse. O repasse
//! e o unico que VE o endereco publico de cada um (o IP:porta de onde chega o
//! REGISTRO), entao ele passa a servir de apresentador: conta a cada lado onde
//! o outro esta, e os dois disparam sondas um para o outro ao mesmo tempo. A
//! sonda de dentro abre o furo no NAT de quem manda; a do outro lado entra por
//! esse furo. A primeira que chegar autenticada muda o caminho do par para
//! `direto` -- e o repasse para de carregar o trafego.
//!
//! # Pacotes (alem dos do `repasse.rs`)
//!
//! ```text
//! APRESENTAR   [10,0,0,0] chave_do_par:32 carimbo:12 mac:32
//! APRESENTACAO [11,0,0,0] chave_do_par:32 familia:1 ip:16 porta:2 carimbo:12 mac:32
//! ```
//!
//! O `mac` e HMAC-SHA256 com o segredo `DH(no, repasse)` -- o MESMO que prova
//! o REGISTRO. O repasse guarda esse segredo desde o registro, entao conferir
//! um APRESENTAR custa um HMAC e nenhum Diffie-Hellman; o no calcula o dele
//! uma vez ao ligar. A APRESENTACAO devolve o carimbo do pedido: resposta que
//! nao casa com o pedido em aberto morre, e um terceiro nao planta endereco.
//!
//! # Anti-abuso: o repasse nao vira refletor, nem delator
//!
//! * O repasse so responde a quem esta registrado, no endereco registrado, e
//!   so manda para esse endereco -- nunca para um alvo que o pedido escolha.
//! * So apresenta quando o pedido e MUTUO: A pediu B e B pediu A. Quem sabe so
//!   a chave publica de alguem nao descobre o IP dele. E cada no so pede o par
//!   com quem ja tem sessao confirmada -- o que prova, pelo Noise com a PSK,
//!   que os dois sao da mesma rede. O repasse nao tem a PSK e nao precisa ter.
//! * O no so sonda o endereco de uma APRESENTACAO autenticada, de um par que
//!   ele mesmo pediu, e no maximo `SONDAS` vezes por rodada, com uma rodada a
//!   cada `RETENTAR`. Mesmo um repasse mentiroso so consegue apontar poucas
//!   sondas pequenas e cifradas por minuto a um endereco.
//!
//! # Onde diverge das referencias, e por que
//!
//! * **IKEv2 Mediation Extension (strongSwan, RFC-rascunho de Brunner)**: la o
//!   mediador troca listas de ENDPOINTs candidatos (locais, refletidos,
//!   repassados) e os pares fazem checagem de conectividade com par de
//!   candidatos, como o ICE. Aqui o candidato e um so -- o refletido que o
//!   repasse ja observa --, porque o caso que falta ao phxvpn e o de NATs
//!   diferentes; o da mesma LAN ja se resolve pelo endereco do convite. Menos
//!   candidato e menos endereco para o repasse revelar.
//! * **Tailscale (disco)**: la as sondas sao mensagens proprias (ping/pong
//!   cifrados com a chave do no), trocadas por um canal lateral do DERP. Aqui
//!   a sonda e um DADOS vazio da sessao que JA existe pelo repasse -- o mesmo
//!   «manter vivo» do transporte. Nao ha cifra nem formato novo: quem recebe
//!   autentica pela sessao, e a migracao e o roaming de endereco que o
//!   `receber_dados` ja fazia (o do WireGuard, secao 2.1 do artigo).
//! * **WireGuard**: nao perfura; so segue o ultimo endereco autenticado. Aqui
//!   isso continua valendo, com uma trava a mais: pacote que chega pelo
//!   repasse NAO desfaz um caminho direto ouvido ha pouco (`DIRETO_VALE`) --
//!   senao, na transicao, o caminho pularia de um lado para o outro a cada
//!   pacote atrasado.
//! * O endereco perfurado nao vai para o arquivo da rede nem para a lista de
//!   pares: e um mapeamento do NAT que so vale para ESTE par e so por agora.
//!   Gravado, faria o modo `auto` gastar ~10 s tentando o direto velho a cada
//!   religar -- pior que hoje.

use super::{No, Via};
use phxsql_core::hash::{hmac_sha256, iguais_em_tempo_constante};
use phxsql_core::x25519;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

pub const TIPO_APRESENTAR: u8 = 10;
pub const TIPO_APRESENTACAO: u8 = 11;
pub const APRESENTAR_LEN: usize = 4 + 32 + 12 + 32;
pub const APRESENTACAO_LEN: usize = 4 + 32 + 1 + 16 + 2 + 12 + 32;

/// Pedido de A por B vale por tanto tempo esperando o de B por A. Os dois
/// pedem a cada tique (1 s) enquanto esperam, entao 15 s cobre a demora de um
/// lado confirmar a sessao e ainda da folga para perda de pacote.
pub const JANELA_PEDIDO: Duration = Duration::from_secs(15);
/// Teto da tabela de pedidos do repasse (cada entrada ja passou pelo HMAC).
pub const TETO_PEDIDOS: usize = 100_000;

/// Sem apresentacao neste prazo, desiste da rodada (o par pode estar com a
/// perfuracao desligada, ou ainda sem sessao do lado dele).
const PRAZO_PEDIDO: Duration = Duration::from_secs(10);
/// Sondas por rodada: uma por tique. O furo abre na primeira que sai dos
/// dois lados; as outras cobrem perda e a defasagem entre as apresentacoes.
pub const SONDAS: u32 = 10;
/// Rodada que falhou (NAT simetrico, sondas bloqueadas) so volta depois
/// disto: o trafego segue pelo repasse, e tentar sem parar so gastaria sonda.
pub const RETENTAR: Duration = Duration::from_secs(60);
/// Caminho direto ouvido ha menos que isto nao cede a pacote do repasse. Um
/// par ocioso manda «manter vivo» a cada 25 s, entao um direto saudavel se
/// ouve dentro deste prazo; passou, o direto morreu e volta-se ao repasse.
pub const DIRETO_VALE: Duration = Duration::from_secs(40);

/// O segredo que prova os pacotes entre um no e o repasse.
pub fn segredo(privada_no: &[u8; 32], publica_repasse: &[u8; 32]) -> Option<[u8; 32]> {
    x25519::segredo(privada_no, publica_repasse).ok()
}

fn mac_pedido(segredo: &[u8; 32], par: &[u8; 32], carimbo: &[u8; 12]) -> [u8; 32] {
    let mut m = b"phxvpn-repasse-apresentar".to_vec();
    m.extend_from_slice(par);
    m.extend_from_slice(carimbo);
    hmac_sha256(segredo, &m)
}

fn mac_apresentacao(segredo: &[u8; 32], corpo: &[u8]) -> [u8; 32] {
    let mut m = b"phxvpn-repasse-apresentacao".to_vec();
    m.extend_from_slice(corpo);
    hmac_sha256(segredo, &m)
}

/// APRESENTAR: o no pede ao repasse o endereco publico do par.
pub fn apresentar(segredo: &[u8; 32], par: &[u8; 32], carimbo: [u8; 12]) -> Vec<u8> {
    let mut p = vec![TIPO_APRESENTAR, 0, 0, 0];
    p.extend_from_slice(par);
    p.extend_from_slice(&carimbo);
    p.extend_from_slice(&mac_pedido(segredo, par, &carimbo));
    p
}

/// A chave do par pedida num APRESENTAR (sem conferir nada ainda).
pub fn par_do_pedido(p: &[u8]) -> Option<[u8; 32]> {
    (p.len() == APRESENTAR_LEN && p[..4] == [TIPO_APRESENTAR, 0, 0, 0])
        .then(|| p[4..36].try_into().expect("32"))
}

fn endereco_em_bytes(a: SocketAddr) -> [u8; 19] {
    let mut b = [0u8; 19];
    match a.ip() {
        IpAddr::V4(v) => {
            b[0] = 4;
            b[1..5].copy_from_slice(&v.octets());
        }
        IpAddr::V6(v) => {
            b[0] = 6;
            b[1..17].copy_from_slice(&v.octets());
        }
    }
    b[17..].copy_from_slice(&a.port().to_be_bytes());
    b
}

fn endereco_de_bytes(b: &[u8]) -> Option<SocketAddr> {
    let porta = u16::from_be_bytes([b[17], b[18]]);
    let ip = match b[0] {
        4 => IpAddr::V4(Ipv4Addr::new(b[1], b[2], b[3], b[4])),
        6 => IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(&b[1..17]).ok()?)),
        _ => return None,
    };
    (porta != 0).then_some(SocketAddr::new(ip, porta))
}

/// APRESENTACAO: o repasse conta ao no onde o par esta.
pub fn apresentacao(
    segredo: &[u8; 32],
    par: &[u8; 32],
    endereco: SocketAddr,
    carimbo: [u8; 12],
) -> Vec<u8> {
    let mut p = vec![TIPO_APRESENTACAO, 0, 0, 0];
    p.extend_from_slice(par);
    p.extend_from_slice(&endereco_em_bytes(endereco));
    p.extend_from_slice(&carimbo);
    let m = mac_apresentacao(segredo, &p[4..]);
    p.extend_from_slice(&m);
    p
}

/// Confere e desmonta uma APRESENTACAO: (par, endereco dele, carimbo).
pub fn abrir_apresentacao(
    segredo: &[u8; 32],
    p: &[u8],
) -> Option<([u8; 32], SocketAddr, [u8; 12])> {
    if p.len() != APRESENTACAO_LEN || p[..4] != [TIPO_APRESENTACAO, 0, 0, 0] {
        return None;
    }
    let corte = APRESENTACAO_LEN - 32;
    if !iguais_em_tempo_constante(&mac_apresentacao(segredo, &p[4..corte]), &p[corte..]) {
        return None;
    }
    Some((
        p[4..36].try_into().expect("32"),
        endereco_de_bytes(&p[36..55])?,
        p[55..67].try_into().expect("12"),
    ))
}

// ------------------------------------------------ lado do repasse ----

/// A mesa de apresentacoes do repasse: quem pediu quem, e quando.
#[derive(Default)]
pub struct Mesa {
    pedidos: HashMap<([u8; 32], [u8; 32]), Instant>,
    /// Ultimo carimbo aceito por no: pedido gravado e reenviado depois que o
    /// no ja pediu de novo nao reabre a mesa.
    carimbos: HashMap<[u8; 32], [u8; 12]>,
    /// Apresentacoes feitas (para a prova ler e o teste conferir).
    pub feitas: u64,
}

impl Mesa {
    /// Trata um APRESENTAR que o repasse ja atribuiu a `origem` (pelo
    /// endereco registrado). `segredo` e o do registro dela; `alvo`, o
    /// endereco registrado do par pedido. Devolve a APRESENTACAO para mandar
    /// A ORIGEM -- e so a ela, no endereco registrado.
    pub fn pedir(
        &mut self,
        dado: &[u8],
        origem: [u8; 32],
        segredo: &[u8; 32],
        alvo: SocketAddr,
    ) -> Option<Vec<u8>> {
        let par = par_do_pedido(dado)?;
        if par == origem {
            return None;
        }
        let carimbo: [u8; 12] = dado[36..48].try_into().expect("12");
        // HMAC primeiro: e o unico custo, e quem nao o tem para aqui.
        if !iguais_em_tempo_constante(&mac_pedido(segredo, &par, &carimbo), &dado[48..80]) {
            return None;
        }
        // Igual passa: o no repete o MESMO pedido a cada tique ate ser
        // atendido, e a resposta so vai ao endereco registrado dele.
        if self.carimbos.get(&origem).is_some_and(|c| carimbo < *c) {
            return None;
        }
        if self.pedidos.len() >= TETO_PEDIDOS {
            self.pedidos.retain(|_, t| t.elapsed() < JANELA_PEDIDO);
            if self.pedidos.len() >= TETO_PEDIDOS {
                return None;
            }
        }
        if self.carimbos.len() >= TETO_PEDIDOS {
            self.carimbos.clear();
        }
        self.carimbos.insert(origem, carimbo);
        self.pedidos.insert((origem, par), Instant::now());
        let mutuo = self
            .pedidos
            .get(&(par, origem))
            .is_some_and(|t| t.elapsed() < JANELA_PEDIDO);
        if !mutuo {
            return None;
        }
        self.feitas += 1;
        Some(apresentacao(segredo, &par, alvo, carimbo))
    }
}

// ----------------------------------------------------- lado do no ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fase {
    Ocioso,
    Pedindo { carimbo: [u8; 12], desde: Instant },
    Sondando { alvo: SocketAddr, enviadas: u32 },
    Direto,
    Esperando { ate: Instant },
}

/// O estado da perfuracao com UM par.
#[derive(Clone, Copy, Debug)]
pub struct Furo {
    fase: Fase,
    direto_visto: Option<Instant>,
    /// Acabou de furar: o outro lado pode ainda nao ter ouvido nada direto
    /// (a sonda dele entrou, a nossa nao saiu depois). O proximo tique manda
    /// um «manter vivo» pelo direto -- medido no teste: sem isso, quem furou
    /// primeiro calava e o outro desistia com 10 sondas sem resposta.
    confirmar: bool,
}

impl Default for Furo {
    fn default() -> Self {
        Furo {
            fase: Fase::Ocioso,
            direto_visto: None,
            confirmar: false,
        }
    }
}

impl Furo {
    /// O caminho direto deste par saiu de uma perfuracao (em curso ou feita):
    /// o endereco dele nao se grava nem se espalha.
    pub fn perfurado(&self) -> bool {
        matches!(self.fase, Fase::Sondando { .. } | Fase::Direto)
    }

    /// Perfurado e ouvido ha pouco: vale a pena refazer o aperto por ele.
    pub fn direto_vivo(&self) -> bool {
        self.fase == Fase::Direto && self.direto_visto.is_some_and(|t| t.elapsed() < DIRETO_VALE)
    }

    /// Chegou um pacote AUTENTICADO do par por `via`. Devolve se o caminho do
    /// par deve passar a ser `via`. `aperto`: INICIO/RESPOSTA, que sempre
    /// mandam -- quem refez o aperto por um caminho escolheu esse caminho.
    pub fn chegou(&mut self, via: Via, aperto: bool) -> bool {
        match via {
            Via::Direta(_) => {
                // Esperando tambem: a sonda do outro lado pode chegar depois
                // de este lado desistir da rodada.
                if matches!(
                    self.fase,
                    Fase::Pedindo { .. } | Fase::Sondando { .. } | Fase::Esperando { .. }
                ) {
                    self.fase = Fase::Direto;
                    self.confirmar = true;
                }
                self.direto_visto = Some(Instant::now());
                true
            }
            Via::Repasse => {
                if !aperto && self.direto_vivo() {
                    return false;
                }
                if self.fase == Fase::Direto {
                    self.fase = Fase::Ocioso;
                }
                true
            }
        }
    }
}

/// O que o no precisa para perfurar: o segredo com o repasse. Existe so no
/// modo `auto` -- `repasse` e escolha explicita de passar tudo pelo servidor,
/// e `direto` nao tem apresentador.
#[derive(Clone, Copy)]
pub struct Perfurador {
    segredo: [u8; 32],
}

impl Perfurador {
    pub fn novo(privada: &[u8; 32], publica_repasse: &[u8; 32]) -> Option<Perfurador> {
        segredo(privada, publica_repasse).map(|segredo| Perfurador { segredo })
    }
}

impl No {
    /// Um tique da perfuracao: pede apresentacao de quem esta no repasse,
    /// sonda quem foi apresentado, desiste de quem nao respondeu. Devolve os
    /// datagramas a mandar (destino, pacote).
    pub(super) fn perfurar(&self, e: &mut super::Estado) -> Vec<(SocketAddr, Vec<u8>)> {
        let (Some(pf), Some(r)) = (&self.perfurador, &self.repasse) else {
            return Vec::new();
        };
        let mut saida = Vec::new();
        for par in &mut e.pares {
            let pronta = par
                .atual
                .as_ref()
                .is_some_and(|s| s.confirmada && !s.expirada());
            let fase = par.furo.fase;
            match fase {
                Fase::Ocioso if pronta && par.via == Some(Via::Repasse) => {
                    let carimbo = super::transporte::carimbo_agora();
                    par.furo.fase = Fase::Pedindo {
                        carimbo,
                        desde: Instant::now(),
                    };
                    saida.push((r.endereco, apresentar(&pf.segredo, &par.publica, carimbo)));
                }
                Fase::Pedindo { carimbo, desde } => {
                    if desde.elapsed() >= PRAZO_PEDIDO {
                        par.furo.fase = Fase::Esperando {
                            ate: Instant::now() + RETENTAR,
                        };
                    } else {
                        saida.push((r.endereco, apresentar(&pf.segredo, &par.publica, carimbo)));
                    }
                }
                Fase::Sondando { alvo, enviadas } => {
                    if enviadas >= SONDAS {
                        eprintln!(
                            "phxvpn: {} -- perfuracao sem resposta de {alvo}; segue pelo repasse",
                            par.ip
                        );
                        par.furo.fase = Fase::Esperando {
                            ate: Instant::now() + RETENTAR,
                        };
                    } else if let Some(p) = par.atual.as_mut().and_then(|s| s.selar(&[]).ok()) {
                        par.furo.fase = Fase::Sondando {
                            alvo,
                            enviadas: enviadas + 1,
                        };
                        saida.push((alvo, p));
                    }
                }
                Fase::Direto if par.furo.confirmar => {
                    par.furo.confirmar = false;
                    if let (Some(Via::Direta(a)), Some(p)) =
                        (par.via, par.atual.as_mut().and_then(|s| s.selar(&[]).ok()))
                    {
                        saida.push((a, p));
                    }
                }
                Fase::Direto if !par.furo.direto_vivo() => {
                    // O direto calou: volta ao repasse, que nunca deixou de
                    // estar registrado, em vez de esperar o aperto falhar.
                    eprintln!(
                        "phxvpn: {} -- caminho direto calado; volta ao repasse",
                        par.ip
                    );
                    par.furo.fase = Fase::Ocioso;
                    par.via = Some(Via::Repasse);
                }
                Fase::Esperando { ate } if Instant::now() >= ate => {
                    par.furo.fase = Fase::Ocioso;
                }
                _ => {}
            }
        }
        saida
    }

    /// Chegou uma APRESENTACAO do endereco do repasse. So vale a que abre
    /// com o segredo e casa com o pedido em aberto; entao sai a primeira
    /// sonda, ja.
    pub(super) fn apresentado(&self, dado: &[u8]) {
        let Some(pf) = &self.perfurador else {
            return;
        };
        let Some((chave, alvo, carimbo)) = abrir_apresentacao(&pf.segredo, dado) else {
            return;
        };
        let mut e = self.estado.lock().expect("estado");
        let Some(par) = e.pares.iter_mut().find(|p| p.publica == chave) else {
            return;
        };
        if !matches!(par.furo.fase, Fase::Pedindo { carimbo: c, .. } if c == carimbo) {
            return;
        }
        eprintln!(
            "phxvpn: {} -- apresentado em {alvo}; sondando o caminho direto",
            par.ip
        );
        par.furo.fase = Fase::Sondando { alvo, enviadas: 1 };
        let sonda = par.atual.as_mut().and_then(|s| s.selar(&[]).ok());
        drop(e);
        if let Some(p) = sonda {
            self.enviar(alvo, &p);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn end(p: u16) -> SocketAddr {
        SocketAddr::from(([198, 51, 100, 7], p))
    }

    #[test]
    fn endereco_ida_e_volta_v4_e_v6() {
        for a in [end(51820), "[2001:db8::1]:9".parse().unwrap()] {
            assert_eq!(endereco_de_bytes(&endereco_em_bytes(a)), Some(a));
        }
    }

    /// So o pedido MUTUO e apresentado; o de um lado so fica esperando. E a
    /// resposta carrega o carimbo de quem pediu.
    #[test]
    fn so_apresenta_pedido_mutuo() {
        let (ka, kb) = ([1u8; 32], [2u8; 32]);
        let (sa, sb) = ([7u8; 32], [8u8; 32]);
        let mut m = Mesa::default();
        let c = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5];
        assert!(m.pedir(&apresentar(&sa, &kb, c), ka, &sa, end(2)).is_none());
        let r = m.pedir(&apresentar(&sb, &ka, c), kb, &sb, end(1)).unwrap();
        assert_eq!(abrir_apresentacao(&sb, &r), Some((ka, end(1), c)));
        assert!(
            abrir_apresentacao(&sa, &r).is_none(),
            "outro segredo nao abre"
        );
        // A repete o pedido (mesmo carimbo): agora e atendido.
        let r = m.pedir(&apresentar(&sa, &kb, c), ka, &sa, end(2)).unwrap();
        assert_eq!(abrir_apresentacao(&sa, &r), Some((kb, end(2), c)));
        assert_eq!(m.feitas, 2);
    }

    /// Mac errado, pedido de si mesmo e carimbo que volta no tempo: nada.
    #[test]
    fn pedido_torto_nao_passa() {
        let (ka, kb) = ([1u8; 32], [2u8; 32]);
        let (sa, sb) = ([7u8; 32], [8u8; 32]);
        let mut m = Mesa::default();
        let c1 = [0u8; 12];
        let mut c2 = [0u8; 12];
        c2[11] = 2;
        m.pedir(&apresentar(&sb, &ka, c1), kb, &sb, end(1));
        // A assina com o segredo errado.
        assert!(m
            .pedir(&apresentar(&sb, &kb, c1), ka, &sa, end(2))
            .is_none());
        assert!(m
            .pedir(&apresentar(&sa, &ka, c1), ka, &sa, end(1))
            .is_none());
        m.pedir(&apresentar(&sa, &kb, c2), ka, &sa, end(2)).unwrap();
        assert!(
            m.pedir(&apresentar(&sa, &kb, c1), ka, &sa, end(2))
                .is_none(),
            "carimbo velho reabriu a mesa"
        );
        let mut torto = apresentar(&sa, &kb, c2);
        torto[40] ^= 1;
        assert!(m.pedir(&torto, ka, &sa, end(2)).is_none());
    }

    /// Pacote do repasse nao desfaz um direto ouvido ha pouco; aperto pelo
    /// repasse desfaz; direto chegado durante o pedido vira direto.
    #[test]
    fn furo_segura_o_direto_contra_pacote_atrasado() {
        let mut f = Furo {
            fase: Fase::Sondando {
                alvo: end(1),
                enviadas: 1,
            },
            direto_visto: None,
            confirmar: false,
        };
        assert!(f.chegou(Via::Direta(end(1)), false));
        assert!(f.perfurado() && f.direto_vivo());
        assert!(
            !f.chegou(Via::Repasse, false),
            "dado atrasado desfez o direto"
        );
        assert!(f.chegou(Via::Repasse, true));
        assert!(!f.perfurado());
        let mut g = Furo {
            fase: Fase::Pedindo {
                carimbo: [0; 12],
                desde: Instant::now(),
            },
            direto_visto: None,
            confirmar: false,
        };
        g.chegou(Via::Direta(end(3)), false);
        assert!(g.direto_vivo());
        // Ocioso: direto comum (LAN, IP publico) nao vira «perfurado».
        let mut h = Furo::default();
        h.chegou(Via::Direta(end(4)), false);
        assert!(!h.perfurado());
    }

    use super::super::{psk_da_rede, ParConfig, RepasseCfg};
    use crate::repasse;
    use crate::transporte;
    use std::net::UdpSocket;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    fn pacote_ip(origem: [u8; 4], destino: [u8; 4], carga: &[u8]) -> Vec<u8> {
        let mut p = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0];
        p.extend_from_slice(&origem);
        p.extend_from_slice(&destino);
        p.extend_from_slice(carga);
        p
    }

    /// Drena o soquete do no ate calar; devolve os pacotes IP que sairiam
    /// na placa.
    fn drenar(no: &No) -> Vec<Vec<u8>> {
        let mut buf = vec![0u8; 2048];
        let mut placa = Vec::new();
        while let Ok((n, de)) = no.udp.recv_from(&mut buf) {
            placa.extend(no.da_rede(&buf[..n], de));
        }
        placa
    }

    /// Dois nos no modo `auto`, sem endereco um do outro (como atras de
    /// NAT), e um repasse que conta os bytes de PARA que carregou. Devolve
    /// (caminho de A, caminho de B, bytes de PARA durante 20 pacotes de dados
    /// depois de assentar, apresentacoes feitas).
    fn malha(perfurar: bool) -> (String, String, u64, u64) {
        let r_priv = x25519::gerar_privada();
        let ur = UdpSocket::bind("127.0.0.1:0").unwrap();
        ur.set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let er = ur.local_addr().unwrap();
        let para = Arc::new(AtomicU64::new(0));
        let feitas = Arc::new(AtomicU64::new(0));
        let parar = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fio = {
            let (para, feitas, parar) = (para.clone(), feitas.clone(), parar.clone());
            std::thread::spawn(move || {
                let mut rep = repasse::Repasse::novo(r_priv, None);
                let mut buf = vec![0u8; 4096];
                while !parar.load(Ordering::Relaxed) {
                    let Ok((n, de)) = ur.recv_from(&mut buf) else {
                        continue;
                    };
                    if buf[0] == repasse::TIPO_PARA {
                        para.fetch_add(n as u64, Ordering::Relaxed);
                    }
                    if let Some((alvo, p)) = rep.tratar(&buf[..n], de) {
                        let _ = ur.send_to(&p, alvo);
                    }
                    feitas.store(rep.mesa.feitas, Ordering::Relaxed);
                }
            })
        };
        let (ka, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
        let novo = |k: [u8; 32], meu: &str, dele: [u8; 32], ip_dele: &str| {
            let u = UdpSocket::bind("127.0.0.1:0").unwrap();
            u.set_read_timeout(Some(Duration::from_millis(5))).unwrap();
            let no = No::novo(
                k,
                psk_da_rede("R", "s", 1_000),
                meu.parse().unwrap(),
                u,
                vec![ParConfig {
                    publica: dele,
                    ip: ip_dele.parse().unwrap(),
                    endereco: None,
                }],
            )
            .com_repasse(
                super::super::Modo::Auto,
                Some(RepasseCfg {
                    endereco: er,
                    publica: x25519::chave_publica(&r_priv),
                    conta: None,
                }),
            )
            .unwrap();
            if perfurar {
                no
            } else {
                no.sem_perfuracao()
            }
        };
        let a = novo(ka, "10.78.0.1", x25519::chave_publica(&kb), "10.78.0.2");
        let b = novo(kb, "10.78.0.2", x25519::chave_publica(&ka), "10.78.0.1");
        let caminho = |n: &No| n.situacao()[0][1].clone();
        // 40 tiques de 50 ms: o tique real e de 1 s, e so a cadencia muda.
        for _ in 0..40 {
            a.tique();
            b.tique();
            for _ in 0..5 {
                drenar(&a);
                drenar(&b);
            }
            if caminho(&a).starts_with("direto") && caminho(&b).starts_with("direto") {
                break;
            }
        }
        let antes = para.load(Ordering::Relaxed);
        let ida = pacote_ip([10, 78, 0, 1], [10, 78, 0, 2], b"dado-da-prova");
        let mut chegaram = 0;
        for _ in 0..20 {
            a.da_placa(&ida);
            for _ in 0..5 {
                chegaram += drenar(&b).iter().filter(|p| **p == ida).count();
                drenar(&a);
            }
        }
        assert_eq!(chegaram, 20, "os dados tinham de chegar por algum caminho");
        let durante = para.load(Ordering::Relaxed) - antes;
        parar.store(true, Ordering::Relaxed);
        let _ = fio.join();
        (
            caminho(&a),
            caminho(&b),
            durante,
            feitas.load(Ordering::Relaxed),
        )
    }

    /// Com a perfuracao: os dois migram para o direto e os dados param de
    /// passar pelo repasse (zero byte de PARA durante 20 pacotes).
    #[test]
    fn perfurados_os_dados_nao_passam_pelo_repasse() {
        let (ca, cb, durante, feitas) = malha(true);
        assert!(ca.starts_with("direto"), "A ficou em {ca}");
        assert!(cb.starts_with("direto"), "B ficou em {cb}");
        assert_eq!(durante, 0, "o repasse carregou dados depois da migracao");
        // Uma basta: quem fura primeiro para de pedir e a sonda dele avisa o
        // outro lado.
        assert!(feitas >= 1, "sem apresentacao nao ha perfuracao");
    }

    /// O outro sentido da prova: sem perfuracao, o mesmo cenario fica no
    /// repasse e os dados passam todos por ele.
    #[test]
    fn sem_perfuracao_fica_no_repasse() {
        let (ca, cb, durante, feitas) = malha(false);
        assert_eq!((ca.as_str(), cb.as_str()), ("repasse", "repasse"));
        assert!(durante > 0);
        assert_eq!(feitas, 0);
    }

    /// Nao vira refletor: APRESENTACAO forjada (mac errado), fora de pedido
    /// ou com carimbo que nao e o do pedido nao manda sonda a lugar nenhum.
    /// A legitima manda uma -- e e ela que prova que o teste enxerga sonda.
    #[test]
    fn apresentacao_forjada_nao_manda_sonda() {
        let (ka, kb, kr) = (
            x25519::gerar_privada(),
            x25519::gerar_privada(),
            x25519::gerar_privada(),
        );
        let (pb, pr) = (x25519::chave_publica(&kb), x25519::chave_publica(&kr));
        let falso_repasse = UdpSocket::bind("127.0.0.1:0").unwrap();
        let vitima = UdpSocket::bind("127.0.0.1:0").unwrap();
        vitima
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let alvo = vitima.local_addr().unwrap();
        let psk = psk_da_rede("R", "s", 1_000);
        let a = No::novo(
            ka,
            psk,
            "10.78.0.1".parse().unwrap(),
            UdpSocket::bind("127.0.0.1:0").unwrap(),
            vec![ParConfig {
                publica: pb,
                ip: "10.78.0.2".parse().unwrap(),
                endereco: None,
            }],
        )
        .com_repasse(
            super::super::Modo::Auto,
            Some(RepasseCfg {
                endereco: falso_repasse.local_addr().unwrap(),
                publica: pr,
                conta: None,
            }),
        )
        .unwrap();
        // Uma sessao confirmada com B, como a que o repasse teria aberto.
        let (ini, m1) =
            crate::noise::Iniciador::comecar(crate::noise::PROLOGO, ka, &pb, psk, b"").unwrap();
        let (_, m2) = crate::noise::ler_chamada(crate::noise::PROLOGO, &kb, &m1)
            .unwrap()
            .responder(psk, b"")
            .unwrap();
        let (chaves, _) = ini.terminar(&m2).unwrap();
        let segredo = segredo(&kr, &x25519::chave_publica(&ka)).unwrap();
        let pedido = [9u8; 12];
        {
            let mut e = a.estado.lock().unwrap();
            e.pares[0].atual = Some(transporte::Sessao::nova(chaves, 1, 2, true));
            e.pares[0].via = Some(Via::Repasse);
            e.pares[0].furo.fase = Fase::Pedindo {
                carimbo: pedido,
                desde: Instant::now(),
            };
        }
        let de = falso_repasse.local_addr().unwrap();
        let mut buf = [0u8; 256];
        // Mac com outro segredo (quem nao e o repasse).
        a.da_rede(&apresentacao(&[1; 32], &pb, alvo, pedido), de);
        // Carimbo que nao e o do pedido (resposta velha reenviada).
        a.da_rede(&apresentacao(&segredo, &pb, alvo, [8; 12]), de);
        // Par que ninguem pediu.
        a.da_rede(&apresentacao(&segredo, &[5; 32], alvo, pedido), de);
        assert!(vitima.recv_from(&mut buf).is_err(), "sonda saiu sem pedido");
        // A legitima: exatamente uma sonda, um DADOS cifrado.
        a.da_rede(&apresentacao(&segredo, &pb, alvo, pedido), de);
        let (n, _) = vitima
            .recv_from(&mut buf)
            .expect("a legitima tinha de sondar");
        assert_eq!(buf[0], transporte::TIPO_DADOS);
        assert_eq!(n, transporte::CAB_DADOS + phxsql_core::cifra::TAG_LEN);
        // Repetida, nao sonda de novo (a fase ja saiu de Pedindo).
        a.da_rede(&apresentacao(&segredo, &pb, alvo, pedido), de);
        assert!(vitima.recv_from(&mut buf).is_err());
    }

    /// Pelo repasse inteiro: so quem esta registrado, no endereco
    /// registrado, e so para um par tambem registrado; a resposta volta ao
    /// endereco de quem pediu, nunca a outro.
    #[test]
    fn repasse_so_apresenta_registrados_e_responde_a_quem_pediu() {
        let rp = x25519::gerar_privada();
        let mut r = repasse::Repasse::novo(rp, None);
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let (pa, pb) = (x25519::chave_publica(&a), x25519::chave_publica(&b));
        let (sa, sb) = (
            segredo(&a, &r.publica()).unwrap(),
            segredo(&b, &r.publica()).unwrap(),
        );
        let c = transporte::carimbo_agora();
        r.tratar(
            &repasse::registro(&a, &r.publica(), c, None).unwrap(),
            end(1),
        );
        // B ainda nao registrou: o pedido de A por B nao anda.
        assert!(r.tratar(&apresentar(&sa, &pb, c), end(1)).is_none());
        r.tratar(
            &repasse::registro(&b, &r.publica(), c, None).unwrap(),
            end(2),
        );
        assert!(
            r.tratar(&apresentar(&sa, &pb, c), end(1)).is_none(),
            "so um lado pediu"
        );
        // Pedido com o mac certo de B, mas de um endereco que nao e o dele
        // (gravado e reenviado por um terceiro).
        assert!(r.tratar(&apresentar(&sb, &pa, c), end(66)).is_none());
        let (para, resposta) = r.tratar(&apresentar(&sb, &pa, c), end(2)).unwrap();
        assert_eq!(para, end(2), "a resposta vai a quem pediu");
        assert_eq!(abrir_apresentacao(&sb, &resposta), Some((pa, end(1), c)));
    }
}
