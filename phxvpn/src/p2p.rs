//! O modo P2P: cada no liga a placa virtual a um soquete UDP e fala direto
//! com os pares, sem servidor no meio.
//!
//! # Roteamento pela chave (e a guarda que vem com ele)
//!
//! Cada par tem UM endereco virtual. Pacote que sai pela placa com destino a
//! esse endereco vai cifrado para aquele par; pacote que chega dele so entra
//! na placa se a ORIGEM de dentro for o endereco dele. Sem essa conferencia,
//! um membro poderia forjar pacote em nome de outro -- a etiqueta prova quem
//! mandou o pacote UDP, nao o que esta escrito dentro dele.
//!
//! # Admissao
//!
//! So fecha aperto quem esta na lista de pares (a estatica dele) E sabe a
//! senha da rede (a PSK). Um INICIO de chave desconhecida morre sem resposta:
//! nao se confirma a um estranho que ha alguem escutando.
//!
//! # O que ainda NAO esta aqui
//!
//! O farol (membro alcancavel que faz rele). A perfuracao de NAT mediada
//! pelo repasse mora em `perfuracao.rs`; o rol assinado em `rol.rs` e a
//! descoberta na LAN em `descoberta.rs` -- aqui ficam so os ganchos
//! (`receber_rol`, `aplicar_rol`, `sincronizar_rol`, `receber_anuncio`,
//! `anunciar`).

#[path = "p2p_rol.rs"]
mod ganchos_do_rol;
#[path = "perfuracao.rs"]
pub mod perfuracao;
use ganchos_do_rol::rol_do_disco_se_mais_novo;

use crate::noise;
use crate::rede_p2p::{self, Rede};
use crate::repasse;
use crate::transporte::{self, Sessao};
use crate::{descoberta, rol};
use phxsql_core::hash::pbkdf2_sha256;
use phxsql_core::senha::bytes_aleatorios;
use phxsql_core::x25519;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
#[cfg(any(target_os = "linux", windows))]
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// MTU da placa: 1500 da rede menos IPv4 (20) + UDP (8) + cabecalho (16) +
/// etiqueta (16), com folga para IPv6 por fora -- o numero do WireGuard.
pub const MTU: u32 = 1420;
const MANTER_VIVO: Duration = Duration::from_secs(25);
const REPETIR_APERTO: Duration = Duration::from_secs(5);
const FILA_MAX: usize = 64;
/// Mandou dado e nao ouviu nada do par neste prazo: refaz o aperto. E o
/// `KEEPALIVE_TIMEOUT + REKEY_TIMEOUT` do WireGuard (10 s + 5 s). Sem isso, um
/// par que reiniciou (e perdeu as chaves) ficava surdo ate a sessao velha
/// vencer, 120 s depois -- defeito achado na prova entre dois espacos de rede.
const SURDO_APOS: Duration = Duration::from_secs(15);

/// Iteracoes do PBKDF2 da senha da rede. Cada no paga uma vez ao ligar.
pub const ITERACOES_PSK: u32 = 310_000;

/// A PSK da rede, derivada do nome (como sal) e da senha.
pub fn psk_da_rede(nome: &str, senha: &str, iteracoes: u32) -> [u8; 32] {
    let mut psk = [0u8; 32];
    let sal = format!("phxvpn-p2p-psk:{nome}");
    pbkdf2_sha256(senha.as_bytes(), sal.as_bytes(), iteracoes, &mut psk);
    psk
}

pub struct ParConfig {
    pub publica: [u8; 32],
    pub ip: Ipv4Addr,
    pub endereco: Option<SocketAddr>,
}

/// `PUBHEX@IP_VIRTUAL` ou `PUBHEX@IP_VIRTUAL@HOST:PORTA`.
pub fn ler_par(texto: &str) -> Result<ParConfig, String> {
    let partes: Vec<&str> = texto.split('@').collect();
    if !(2..=3).contains(&partes.len()) {
        return Err(format!("par invalido (use CHAVE@IP[@HOST:PORTA]): {texto}"));
    }
    let publica = phxsql_core::hash::de_hex(partes[0])
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .ok_or_else(|| format!("chave publica invalida: {}", partes[0]))?;
    let ip = partes[1]
        .parse()
        .map_err(|_| format!("IP virtual invalido: {}", partes[1]))?;
    let endereco = match partes.get(2) {
        Some(e) => Some(
            std::net::ToSocketAddrs::to_socket_addrs(e)
                .map_err(|x| format!("endereco {e}: {x}"))?
                .next()
                .ok_or_else(|| format!("endereco {e} nao resolveu"))?,
        ),
        None => None,
    };
    Ok(ParConfig {
        publica,
        ip,
        endereco,
    })
}

/// Por onde o pacote chega ao par.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Direta(SocketAddr),
    Repasse,
}

/// Como o no procura os pares: so direto, so pelo servidor intermediario, ou
/// direto primeiro e o intermediario quando o direto nao responde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modo {
    Direto,
    Repasse,
    Auto,
}

impl Modo {
    pub fn de_texto(t: &str) -> Result<Modo, String> {
        match t {
            "direto" => Ok(Modo::Direto),
            "repasse" => Ok(Modo::Repasse),
            "auto" => Ok(Modo::Auto),
            _ => Err(format!("modo desconhecido: {t} (direto, repasse ou auto)")),
        }
    }
}

/// Mensagem de controle dentro do tunel: comeca com 0x00, que nenhum pacote
/// IP tem no primeiro byte (a versao ocupa os 4 bits de cima). `P` = a lista
/// de pares que quem manda conhece.
const CONTROLE: u8 = 0x00;
const CONTROLE_PARES: u8 = b'P';
/// Eco (o «ping» do phxvpn): pedido e resposta, com um numero de 8 bytes.
/// Mede o tempo de ida e volta PELO TUNEL, sem socket bruto de ICMP.
const CONTROLE_ECO: u8 = b'E';
const CONTROLE_ECO_VOLTA: u8 = b'e';
/// Chat: texto UTF-8 entre membros, dentro do tunel cifrado.
const CONTROLE_CHAT: u8 = b'C';
/// O rol assinado (`rol.rs`), em binario: vai junto da lista de pares.
const CONTROLE_ROL: u8 = b'R';
/// De quanto em quanto o no rele o rol do disco: `p2p remover` roda em outro
/// processo e grava ali o rol novo, como `p2p convidar` grava a ficha.
const RELER_ROL: Duration = Duration::from_secs(2);
/// Teto de uma mensagem de chat e da caixa de entrada (membro falante nao
/// enche a memoria).
pub const TETO_CHAT: usize = 1000;
const TETO_CAIXA: usize = 200;

/// Uma mensagem de chat, enviada ou recebida.
#[derive(Clone, Debug)]
pub struct Mensagem {
    pub numero: u64,
    /// IP virtual do outro lado.
    pub ip: Ipv4Addr,
    pub minha: bool,
    pub texto: String,
    pub quando: u64,
}
/// A lista de pares se reenvia a cada tanto (e logo que a sessao abre).
const REENVIAR_PARES: Duration = Duration::from_secs(30);
/// Teto de pares aprendidos pela malha (lista de membro malicioso nao enche
/// a memoria).
const TETO_PARES: usize = 1024;

/// Acima de tantos INICIOs com mac1 valido num segundo, o no entra «sob
/// carga» (por 1 s) e passa a exigir o cookie (mac2). Um aperto custa
/// 1,19 ms medido: 20 por segundo e ~2% de um nucleo, e uma rede de dezenas
/// de pares refaz cada sessao a cada 120 s -- longe disso.
const CARGA_INICIOS_POR_S: u32 = 20;
/// Sob carga, e com o cookie provado, cada origem abre no maximo isto por
/// segundo.
const INICIOS_POR_ORIGEM_POR_S: u32 = 5;
/// O segredo dos cookies gira; cookie velho para de valer (WireGuard: 120 s).
const VIDA_COOKIE: Duration = Duration::from_secs(120);

/// Defesa contra inundacao de INICIO (ver `transporte.rs`, mac1/mac2).
struct Anti {
    segredo: [u8; 32],
    desde: Instant,
    janela: Instant,
    na_janela: u32,
    sob_carga_ate: Option<Instant>,
    por_origem: HashMap<Vec<u8>, (Instant, u32)>,
}

/// Tentativas diretas sem resposta antes de o modo `auto` ir ao repasse.
const TENTATIVAS_DIRETAS: u8 = 2;

pub struct RepasseCfg {
    pub endereco: SocketAddr,
    pub publica: [u8; 32],
    /// Conta no repasse (usuario + credencial), quando ele exige.
    pub conta: Option<repasse::Conta>,
}

struct Par {
    publica: [u8; 32],
    ip: Ipv4Addr,
    /// Endereco direto conhecido (configurado ou aprendido).
    endereco: Option<SocketAddr>,
    /// Caminho que funcionou por ultimo -- e por ele que se responde.
    via: Option<Via>,
    tentativas_diretas: u8,
    atual: Option<Sessao>,
    anterior: Option<Sessao>,
    pendente: Option<(u32, noise::Iniciador, Instant)>,
    fila: Vec<Vec<u8>>,
    ultimo_carimbo: [u8; 12],
    ultimo_envio: Instant,
    /// Desde quando mandamos dado sem ouvir nada autenticado do par.
    sem_resposta_desde: Option<Instant>,
    /// Quando a lista de pares foi mandada a este par por ultimo.
    ultimo_rol: Option<Instant>,
    /// Cookie que este par nos deu (vai no mac2 dos proximos INICIOs).
    cookie: Option<([u8; transporte::MAC_LEN], Instant)>,
    /// mac1 do ultimo INICIO mandado: e o aad da resposta de cookie.
    ultimo_mac1: [u8; transporte::MAC_LEN],
    /// Perfuracao de NAT com este par (ver `perfuracao.rs`).
    furo: perfuracao::Furo,
}

struct Estado {
    pares: Vec<Par>,
    /// indice local de sessao -> par
    indices: HashMap<u32, usize>,
}

pub struct No {
    privada: [u8; 32],
    psk: [u8; 32],
    ip: Ipv4Addr,
    estado: Mutex<Estado>,
    udp: UdpSocket,
    modo: Modo,
    repasse: Option<RepasseCfg>,
    /// So no modo `auto`: o segredo com o repasse para pedir apresentacao.
    perfurador: Option<perfuracao::Perfurador>,
    ultimo_registro: Mutex<Option<Instant>>,
    /// O arquivo da rede (convites abertos, pares) e onde grava-lo. Sem ele
    /// (pares so pela linha de comando), nao ha admissao nem persistencia.
    rede: Mutex<Option<(Rede, String)>>,
    /// Ficha do convite que este no apresenta ate ser admitido.
    ficha_de_entrada: Mutex<Option<[u8; 16]>>,
    /// Fichas que este no ja consumiu. Toda releitura do disco as filtra:
    /// sem isso, o `persistir` relia o convite ainda no disco e RESSUSCITAVA
    /// a ficha usada -- achado na prova com quatro nos, em que o segundo uso
    /// so foi barrado por acaso (o IP ja estava ocupado).
    fichas_usadas: Mutex<Vec<[u8; 16]>>,
    /// Desligar a rede: as tres threads do `rodar` olham isto e saem; a placa
    /// fecha junto e o sistema a apaga.
    parar: std::sync::atomic::AtomicBool,
    /// Ecos em voo: numero -> (quando saiu, tempo de volta quando voltar).
    ecos: Mutex<HashMap<u64, (Instant, Option<Duration>)>>,
    caixa: Mutex<std::collections::VecDeque<Mensagem>>,
    numero_mensagem: std::sync::atomic::AtomicU64,
    /// Chave do mac1 dos INICIOs que chegam (sai da MINHA publica).
    chave_mac1: [u8; 32],
    anti: Mutex<Anti>,
    /// Quantos INICIOs chegaram ao X25519. Os testes o leem para provar que
    /// o lixo morre ANTES.
    apertos_tentados: std::sync::atomic::AtomicU64,
    minha_publica: [u8; 32],
    /// Versao do rol que ja esta refletida em `estado.pares` (0 = nenhuma).
    rol_aplicado: std::sync::atomic::AtomicU64,
    rol_lido: Mutex<Option<Instant>>,
    /// `None` = descoberta desligada (e o anuncio morre no primeiro byte).
    descoberta: Mutex<Option<descoberta::Descoberta>>,
}

fn indice_novo(indices: &HashMap<u32, usize>) -> u32 {
    loop {
        let b = bytes_aleatorios(4);
        let i = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if i != 0 && !indices.contains_key(&i) {
            return i;
        }
    }
}

fn destino_ipv4(pacote: &[u8]) -> Option<Ipv4Addr> {
    (pacote.len() >= 20 && pacote[0] >> 4 == 4)
        .then(|| Ipv4Addr::new(pacote[16], pacote[17], pacote[18], pacote[19]))
}

fn origem_ipv4(pacote: &[u8]) -> Option<Ipv4Addr> {
    (pacote.len() >= 20 && pacote[0] >> 4 == 4)
        .then(|| Ipv4Addr::new(pacote[12], pacote[13], pacote[14], pacote[15]))
}

impl No {
    pub fn novo(
        privada: [u8; 32],
        psk: [u8; 32],
        ip: Ipv4Addr,
        udp: UdpSocket,
        pares: Vec<ParConfig>,
    ) -> No {
        let agora = Instant::now();
        let pares = pares
            .into_iter()
            .map(|p| Par {
                publica: p.publica,
                ip: p.ip,
                endereco: p.endereco,
                via: p.endereco.map(Via::Direta),
                tentativas_diretas: 0,
                atual: None,
                anterior: None,
                pendente: None,
                fila: Vec::new(),
                ultimo_carimbo: [0; 12],
                ultimo_envio: agora,
                sem_resposta_desde: None,
                ultimo_rol: None,
                cookie: None,
                ultimo_mac1: [0; transporte::MAC_LEN],
                furo: perfuracao::Furo::default(),
            })
            .collect();
        No {
            privada,
            psk,
            ip,
            estado: Mutex::new(Estado {
                pares,
                indices: HashMap::new(),
            }),
            udp,
            modo: Modo::Direto,
            repasse: None,
            perfurador: None,
            ultimo_registro: Mutex::new(None),
            rede: Mutex::new(None),
            ficha_de_entrada: Mutex::new(None),
            fichas_usadas: Mutex::new(Vec::new()),
            parar: std::sync::atomic::AtomicBool::new(false),
            ecos: Mutex::new(HashMap::new()),
            caixa: Mutex::new(std::collections::VecDeque::new()),
            numero_mensagem: std::sync::atomic::AtomicU64::new(1),
            chave_mac1: transporte::chave_mac1(&x25519::chave_publica(&privada)),
            anti: Mutex::new(Anti {
                segredo: bytes_aleatorios(32).try_into().expect("32"),
                desde: agora,
                janela: agora,
                na_janela: 0,
                sob_carga_ate: None,
                por_origem: HashMap::new(),
            }),
            apertos_tentados: std::sync::atomic::AtomicU64::new(0),
            minha_publica: x25519::chave_publica(&privada),
            rol_aplicado: std::sync::atomic::AtomicU64::new(0),
            rol_lido: Mutex::new(None),
            descoberta: Mutex::new(None),
        }
    }

    /// Liga (ou nao) o anuncio na LAN. O soquete precisa de `SO_BROADCAST`
    /// para mandar ao endereco de broadcast.
    pub fn com_descoberta(self, ligada: bool) -> No {
        if ligada {
            let _ = self.udp.set_broadcast(true);
            *self.descoberta.lock().expect("descoberta") =
                Some(descoberta::Descoberta::nova(&self.psk, true));
        }
        self
    }

    /// Liga o no ao arquivo da rede: admite quem traz ficha de convite,
    /// apresenta a propria ficha (se veio de um convite) e grava o que
    /// aprender.
    pub fn com_rede(self, rede: Rede, caminho: String) -> No {
        *self.ficha_de_entrada.lock().expect("ficha") = rede.ficha_de_entrada;
        *self.rede.lock().expect("rede") = Some((rede, caminho));
        self
    }

    /// Regrava o arquivo da rede com os pares que o no conhece agora.
    ///
    /// Os CONVITES sao do disco, nao da memoria: `p2p convidar` roda em outro
    /// processo com a rede ligada, e regravar a copia de memoria apagava o
    /// convite recem-feito -- defeito achado na prova com quatro nos. Entao
    /// rele o arquivo e troca so os pares e a ficha de entrada.
    /// Os convites do disco, sem as fichas que este no ja consumiu.
    fn convites_do_disco(&self, caminho: &str) -> Option<Vec<rede_p2p::ConviteAberto>> {
        let usadas = self.fichas_usadas.lock().expect("fichas");
        Rede::ler(caminho).ok().map(|d| {
            d.convites
                .into_iter()
                .filter(|c| !usadas.contains(&c.ficha))
                .collect()
        })
    }

    fn persistir(&self, e: &Estado) {
        let mut r = self.rede.lock().expect("rede");
        if let Some((rede, caminho)) = r.as_mut() {
            if let Some(c) = self.convites_do_disco(caminho) {
                rede.convites = c;
            }
            // O rol do disco pode ser mais novo (`p2p remover` acabou de
            // grava-lo): regravar o da memoria o desfaria.
            if let Some(d) = rol_do_disco_se_mais_novo(rede, caminho) {
                rede.rol = Some(d);
            }
            rede.pares = e
                .pares
                .iter()
                .map(|p| rede_p2p::Par {
                    chave: p.publica,
                    ip: p.ip,
                    endereco: p.endereco.map(|a| a.to_string()),
                })
                .collect();
            rede.ficha_de_entrada = *self.ficha_de_entrada.lock().expect("ficha");
            if let Err(x) = rede.gravar(caminho) {
                eprintln!("phxvpn: {x}");
            }
        }
    }

    fn par_novo(publica: [u8; 32], ip: Ipv4Addr, endereco: Option<SocketAddr>) -> Par {
        Par {
            publica,
            ip,
            endereco,
            via: endereco.map(Via::Direta),
            tentativas_diretas: 0,
            atual: None,
            anterior: None,
            pendente: None,
            fila: Vec::new(),
            ultimo_carimbo: [0; 12],
            ultimo_envio: Instant::now(),
            sem_resposta_desde: None,
            ultimo_rol: None,
            cookie: None,
            ultimo_mac1: [0; transporte::MAC_LEN],
            furo: perfuracao::Furo::default(),
        }
    }

    /// A malha mudou (par novo admitido ou aprendido): a lista vai a todos no
    /// proximo tique, em vez de esperar o reenvio de 30 s. Medido: sem isso,
    /// o terceiro membro ficava ate 30 s sem ser reconhecido pelo segundo,
    /// que recusava o aperto dele como chave desconhecida.
    fn avisar_malha(e: &mut Estado) {
        for p in &mut e.pares {
            p.ultimo_rol = None;
        }
    }

    /// A lista de pares que este no conhece, para mandar a um par -- e, em
    /// rede de rol assinado, o rol logo atras (mensagem propria).
    fn rol_para(&self, e: &Estado, destino: usize) -> Vec<Vec<u8>> {
        // O rol vai ANTES da lista: em rede assinada a lista so ensina
        // endereco de quem ja esta no rol, e o rol novo e que o poe la. Na
        // ordem contraria, o endereco de um membro novo so chegava no reenvio
        // de 30 s -- medido na prova com tres netns (B->C em 30,2 s).
        let mut v: Vec<Vec<u8>> = self.mensagem_do_rol().into_iter().collect();
        v.push(self.lista_para(e, destino));
        v
    }

    fn lista_para(&self, e: &Estado, destino: usize) -> Vec<u8> {
        let mut lista: Vec<rede_p2p::Par> = e
            .pares
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != destino)
            .map(|(_, p)| rede_p2p::Par {
                chave: p.publica,
                ip: p.ip,
                endereco: p.endereco.map(|a| a.to_string()),
            })
            .collect();
        // Este no tambem entra: quem so conhece o anfitriao pelo endereco do
        // convite aprende o IP e a chave certos dele por aqui.
        lista.push(rede_p2p::Par {
            chave: self.publica(),
            ip: self.ip,
            endereco: None,
        });
        let mut m = vec![CONTROLE, CONTROLE_PARES];
        m.extend_from_slice(rede_p2p::pares_json(&lista).escrever().as_bytes());
        m
    }

    /// Recebeu a lista de um par autenticado: acrescenta quem nao conhecia.
    /// Confianca transitiva: quem ja e membro apresenta outros -- e cada um
    /// deles ainda precisa da senha da rede (PSK) para fechar aperto.
    ///
    /// Em rede de rol assinado, a lista so ENSINA ENDERECO de quem ja esta no
    /// rol: membro nenhum apresenta membro -- so a assinatura do dono.
    fn aprender_rol(&self, e: &mut Estado, corpo: &[u8]) -> bool {
        let assinada = self.assinada();
        let Ok(j) = std::str::from_utf8(corpo)
            .map_err(|_| ())
            .and_then(|t| phxsql_core::json::Json::analisar(t).map_err(|_| ()))
        else {
            return false;
        };
        let mut mudou = false;
        for item in j.lista().unwrap_or_default() {
            let Ok(p) = rede_p2p::par_de_json(item) else {
                continue;
            };
            if p.chave == self.publica() || p.ip == self.ip {
                continue;
            }
            let endereco = p.endereco.as_deref().and_then(|a| a.parse().ok());
            match e.pares.iter_mut().find(|x| x.publica == p.chave) {
                Some(x) => {
                    if x.endereco.is_none() && endereco.is_some() {
                        x.endereco = endereco;
                        if x.via.is_none() {
                            x.via = endereco.map(Via::Direta);
                        }
                        mudou = true;
                    }
                }
                None if assinada => {}
                None => {
                    // IP ja usado por outra chave: recusa, nao sobrescreve.
                    if e.pares.len() < TETO_PARES && !e.pares.iter().any(|x| x.ip == p.ip) {
                        e.pares.push(No::par_novo(p.chave, p.ip, endereco));
                        mudou = true;
                    }
                }
            }
        }
        mudou
    }

    /// Liga o servidor intermediario e escolhe o modo.
    pub fn com_repasse(mut self, modo: Modo, repasse: Option<RepasseCfg>) -> Result<No, String> {
        if modo != Modo::Direto && repasse.is_none() {
            return Err("os modos repasse e auto precisam de --repasse CHAVE@HOST:PORTA".into());
        }
        self.modo = modo;
        self.perfurador = match (&repasse, modo) {
            (Some(r), Modo::Auto) => perfuracao::Perfurador::novo(&self.privada, &r.publica),
            _ => None,
        };
        self.repasse = repasse;
        Ok(self)
    }

    /// Desliga a perfuracao de NAT (o `auto` volta a ser «direto ou repasse»).
    pub fn sem_perfuracao(mut self) -> No {
        self.perfurador = None;
        self
    }

    pub fn publica(&self) -> [u8; 32] {
        x25519::chave_publica(&self.privada)
    }

    fn enviar(&self, destino: SocketAddr, pacote: &[u8]) {
        let _ = self.udp.send_to(pacote, destino);
    }

    /// Manda pelo caminho `via`; pelo repasse, embrulhado para a chave do par.
    fn mandar(&self, via: Via, chave_dele: &[u8; 32], pacote: &[u8]) {
        match via {
            Via::Direta(a) => self.enviar(a, pacote),
            Via::Repasse => {
                if let Some(r) = &self.repasse {
                    self.enviar(r.endereco, &repasse::embrulhar_para(chave_dele, pacote));
                }
            }
        }
    }

    /// Pacote autenticado chegou por `via`: e por ali que se responde.
    /// `aperto`: INICIO ou RESPOSTA (ver `Furo::chegou`).
    fn aprender(par: &mut Par, via: Via, aperto: bool) {
        if !par.furo.chegou(via, aperto) {
            return;
        }
        if par.via != Some(via) && par.furo.perfurado() {
            eprintln!("phxvpn: {} -- caminho direto (perfurado) {via:?}", par.ip);
        }
        par.via = Some(via);
        if let Via::Direta(a) = via {
            // Endereco perfurado e mapeamento do NAT so para este par: nao se
            // grava nem se espalha (ver `perfuracao.rs`).
            if !par.furo.perfurado() {
                par.endereco = Some(a);
            }
            par.tentativas_diretas = 0;
        }
    }

    /// Comeca (ou refaz) o aperto com o par `i`, se ha endereco para ele.
    fn iniciar_aperto(&self, e: &mut Estado, i: usize) {
        if let Some((_, _, quando)) = &e.pares[i].pendente {
            if quando.elapsed() < REPETIR_APERTO {
                return;
            }
        }
        let repetindo = e.pares[i].pendente.is_some();
        let par = &mut e.pares[i];
        let via = match (self.modo, par.endereco) {
            // Direto perfurado e vivo: o aperto novo vai por ele; se nao
            // responder, a repeticao cai no repasse (5 s, nao 10).
            (Modo::Auto, _) if !repetindo && par.furo.direto_vivo() => match par.via {
                Some(v @ Via::Direta(_)) => v,
                _ => Via::Repasse,
            },
            (Modo::Direto, Some(a)) => Via::Direta(a),
            (Modo::Direto, None) => return,
            (Modo::Repasse, _) => Via::Repasse,
            (Modo::Auto, Some(a)) if par.tentativas_diretas < TENTATIVAS_DIRETAS => {
                par.tentativas_diretas += 1;
                Via::Direta(a)
            }
            (Modo::Auto, _) => Via::Repasse,
        };
        let indice = indice_novo(&e.indices);
        let publica = e.pares[i].publica;
        let mut carga = transporte::carimbo_agora().to_vec();
        let ficha = *self.ficha_de_entrada.lock().expect("ficha");
        if let Some(f) = ficha {
            carga.extend_from_slice(&f);
            // O apelido vai com a ficha: e o dono que o escreve no rol.
            if let Some((r, _)) = self.rede.lock().expect("rede").as_ref() {
                let a = r.apelido.as_deref().unwrap_or("");
                if a.len() <= rol::TETO_NOME {
                    carga.extend_from_slice(a.as_bytes());
                }
            }
        }
        let Ok((ini, m1)) =
            noise::Iniciador::comecar(noise::PROLOGO, self.privada, &publica, self.psk, &carga)
        else {
            return;
        };
        if let Some((velho, _, _)) = e.pares[i].pendente.take() {
            e.indices.remove(&velho);
        }
        e.indices.insert(indice, i);
        e.pares[i].pendente = Some((indice, ini, Instant::now()));
        let cookie = e.pares[i]
            .cookie
            .filter(|(_, quando)| quando.elapsed() < VIDA_COOKIE)
            .map(|(c, _)| c);
        let pacote = transporte::embrulhar_inicio(indice, &m1, &publica, cookie.as_ref());
        let fim = pacote.len() - transporte::MAC_LEN;
        e.pares[i].ultimo_mac1 = pacote[fim - transporte::MAC_LEN..fim]
            .try_into()
            .expect("16");
        self.mandar(via, &publica, &pacote);
    }

    /// Um pacote que o sistema mandou para a placa: cifra para o par dono do
    /// destino, ou enfileira e comeca o aperto.
    pub fn da_placa(&self, pacote: &[u8]) {
        let Some(dst) = destino_ipv4(pacote) else {
            return;
        };
        let mut e = self.estado.lock().expect("estado");
        let Some(i) = e.pares.iter().position(|p| p.ip == dst) else {
            return;
        };
        let pronta = e.pares[i]
            .atual
            .as_ref()
            .is_some_and(|s| s.confirmada && !s.expirada() && !s.pede_novo_aperto());
        if pronta {
            let (via, chave) = (e.pares[i].via, e.pares[i].publica);
            let s = e.pares[i].atual.as_mut().expect("conferido");
            if let (Ok(p), Some(v)) = (s.selar(pacote), via) {
                let par = &mut e.pares[i];
                par.ultimo_envio = Instant::now();
                par.sem_resposta_desde.get_or_insert_with(Instant::now);
                drop(e);
                self.mandar(v, &chave, &p);
                return;
            }
        }
        if e.pares[i].fila.len() < FILA_MAX {
            e.pares[i].fila.push(pacote.to_vec());
        }
        self.iniciar_aperto(&mut e, i);
    }

    /// Um datagrama que chegou pela rede. Devolve o pacote IP a entregar na
    /// placa, se houver.
    pub fn da_rede(&self, dado: &[u8], de: SocketAddr) -> Option<Vec<u8>> {
        // Do repasse so se aceita `DE`, e so do endereco do repasse.
        if let Some(r) = &self.repasse {
            if de == r.endereco {
                if dado.first() == Some(&perfuracao::TIPO_APRESENTACAO) {
                    self.apresentado(dado);
                    return None;
                }
                let (origem, dentro) = repasse::desembrulhar_de(dado)?;
                return self.despachar(dentro, Via::Repasse, Some(origem));
            }
        }
        self.despachar(dado, Via::Direta(de), None)
    }

    /// `declarada`: a chave de origem que o repasse diz; tem de bater com a
    /// que o Noise autenticou, senao o pacote morre.
    fn despachar(&self, dado: &[u8], via: Via, declarada: Option<[u8; 32]>) -> Option<Vec<u8>> {
        match *dado.first()? {
            transporte::TIPO_INICIO => {
                self.receber_inicio(dado, via, declarada);
                None
            }
            transporte::TIPO_RESPOSTA => {
                self.receber_resposta(dado, via, declarada);
                None
            }
            transporte::TIPO_DADOS => self.receber_dados(dado, via, declarada),
            transporte::TIPO_COOKIE => {
                self.receber_cookie(dado);
                None
            }
            descoberta::TIPO_ANUNCIO => {
                if let Via::Direta(de) = via {
                    self.receber_anuncio(dado, de);
                }
                None
            }
            _ => None,
        }
    }

    fn receber_inicio(&self, dado: &[u8], via: Via, declarada: Option<[u8; 32]>) {
        let Some(ini) = transporte::desembrulhar_inicio(dado) else {
            return;
        };
        // Quem nao sabe a minha publica morre aqui, num HMAC -- sem X25519.
        if !ini.mac1_confere(&self.chave_mac1) {
            return;
        }
        let origem: Vec<u8> = match (via, declarada) {
            (Via::Direta(a), _) => a.to_string().into_bytes(),
            (Via::Repasse, Some(k)) => k.to_vec(),
            (Via::Repasse, None) => return,
        };
        if let Some(cookie) = self.cookie_se_sob_carga(&origem) {
            if !ini.mac2_confere(&cookie) {
                // Resposta barata: um HMAC e uma XChaCha. Quem forjou o IP
                // nao a recebe; quem a recebe prova o endereco.
                let chave = declarada.unwrap_or([0; 32]);
                let minha = x25519::chave_publica(&self.privada);
                self.mandar(
                    via,
                    &chave,
                    &transporte::embrulhar_cookie(ini.remetente, &minha, &ini.mac1, &cookie),
                );
                return;
            }
            if !self.origem_dentro_do_limite(&origem) {
                return;
            }
        }
        let (remetente, m1) = (ini.remetente, ini.m1);
        self.apertos_tentados
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Ok(chamada) = noise::ler_chamada(noise::PROLOGO, &self.privada, m1) else {
            return;
        };
        let mut e = self.estado.lock().expect("estado");
        // Admissao: quem esta na lista, ou quem traz a ficha de um convite
        // em aberto (a ficha so sai de dentro do convite, que so abre com a
        // senha da rede). A ficha morre ao ser usada.
        let i = match e
            .pares
            .iter()
            .position(|p| p.publica == chamada.estatica_dele)
        {
            Some(i) => i,
            None => {
                let Ok(ficha) = <[u8; 16]>::try_from(chamada.carga.get(12..28).unwrap_or_default())
                else {
                    return;
                };
                let ip = {
                    let mut r = self.rede.lock().expect("rede");
                    let Some((rede, caminho)) = r.as_mut() else {
                        return;
                    };
                    // O convite pode ter sido feito depois de o no ligar:
                    // a ficha se confere contra o DISCO.
                    if let Some(c) = self.convites_do_disco(caminho) {
                        rede.convites = c;
                    }
                    // Em rede de rol assinado so o dono admite: e ele quem
                    // assina o rol novo com o convidado dentro.
                    let dono = rede.dono;
                    if dono.is_some_and(|d| rol::publica_do_dono(&self.privada, &rede.nome) != d) {
                        return;
                    }
                    let Some(ip) = rede.usar_ficha(&ficha) else {
                        return;
                    };
                    self.fichas_usadas.lock().expect("fichas").push(ficha);
                    if dono.is_some() {
                        let nome = chamada
                            .carga
                            .get(28..)
                            .and_then(|b| std::str::from_utf8(b).ok())
                            .filter(|n| !n.is_empty() && n.len() <= rol::TETO_NOME)
                            .map(str::to_string);
                        let novo = rede.rol.as_ref().map(|r| {
                            r.com(
                                rol::Membro {
                                    chave: chamada.estatica_dele,
                                    ip,
                                    nome,
                                },
                                &self.privada,
                            )
                        });
                        match novo {
                            Some(Ok(r)) => rede.rol = Some(r),
                            _ => return,
                        }
                    }
                    ip
                };
                if e.pares.iter().any(|p| p.ip == ip) {
                    return;
                }
                let endereco = match via {
                    Via::Direta(a) => Some(a),
                    Via::Repasse => None,
                };
                e.pares
                    .push(No::par_novo(chamada.estatica_dele, ip, endereco));
                No::avisar_malha(&mut e);
                self.persistir(&e);
                self.aplicar_rol(&mut e);
                match e
                    .pares
                    .iter()
                    .position(|p| p.publica == chamada.estatica_dele)
                {
                    Some(i) => i,
                    None => return,
                }
            }
        };
        // Em rede de rol assinado, estar na lista nao basta: tem de estar no
        // rol mais novo que este no aceitou.
        if self.fora_do_rol(&chamada.estatica_dele) {
            return;
        }
        if declarada.is_some_and(|k| k != chamada.estatica_dele) {
            return;
        }
        // Repeticao: um INICIO gravado e reenviado tem carimbo velho.
        let Ok(carimbo) = <[u8; 12]>::try_from(chamada.carga.get(..12).unwrap_or_default()) else {
            return;
        };
        if carimbo <= e.pares[i].ultimo_carimbo {
            return;
        }
        let Ok((chaves, m2)) = chamada.responder(self.psk, &[]) else {
            return;
        };
        e.pares[i].ultimo_carimbo = carimbo;
        let indice = indice_novo(&e.indices);
        e.indices.insert(indice, i);
        let nova = Sessao::nova(chaves, indice, remetente, false);
        self.trocar_sessao(&mut e, i, nova);
        No::aprender(&mut e.pares[i], via, true);
        let chave = e.pares[i].publica;
        drop(e);
        self.mandar(
            via,
            &chave,
            &transporte::embrulhar_resposta(indice, remetente, &m2),
        );
    }

    /// Conta o INICIO (ja com mac1 valido) e, se o no esta sob carga,
    /// devolve o cookie que esta origem teria de provar.
    fn cookie_se_sob_carga(&self, origem: &[u8]) -> Option<[u8; transporte::MAC_LEN]> {
        let mut a = self.anti.lock().expect("anti");
        let agora = Instant::now();
        if a.desde.elapsed() >= VIDA_COOKIE {
            a.segredo = bytes_aleatorios(32).try_into().expect("32");
            a.desde = agora;
        }
        if a.janela.elapsed() >= Duration::from_secs(1) {
            a.janela = agora;
            a.na_janela = 0;
        }
        a.na_janela += 1;
        if a.na_janela > CARGA_INICIOS_POR_S {
            a.sob_carga_ate = Some(agora + Duration::from_secs(1));
        }
        let sob_carga = a.sob_carga_ate.is_some_and(|t| agora < t);
        sob_carga.then(|| transporte::mac(&a.segredo, origem))
    }

    fn origem_dentro_do_limite(&self, origem: &[u8]) -> bool {
        let mut a = self.anti.lock().expect("anti");
        if a.por_origem.len() > 4096 {
            a.por_origem
                .retain(|_, (t, _)| t.elapsed() < Duration::from_secs(1));
        }
        let (t, n) = a
            .por_origem
            .entry(origem.to_vec())
            .or_insert((Instant::now(), 0));
        if t.elapsed() >= Duration::from_secs(1) {
            *t = Instant::now();
            *n = 0;
        }
        *n += 1;
        *n <= INICIOS_POR_ORIGEM_POR_S
    }

    /// Resposta de cookie a um INICIO nosso: so vale se o indice e de um
    /// aperto pendente e se abre com o mac1 que mandamos.
    fn receber_cookie(&self, dado: &[u8]) {
        let Some(receptor) = transporte::receptor_do_cookie(dado) else {
            return;
        };
        let mut e = self.estado.lock().expect("estado");
        let Some(&i) = e.indices.get(&receptor) else {
            return;
        };
        if !e.pares[i]
            .pendente
            .as_ref()
            .is_some_and(|(ind, _, _)| *ind == receptor)
        {
            return;
        }
        let par = &mut e.pares[i];
        if let Some(c) = transporte::abrir_cookie(dado, &par.publica, &par.ultimo_mac1) {
            par.cookie = Some((c, Instant::now()));
        }
    }

    fn trocar_sessao(&self, e: &mut Estado, i: usize, nova: Sessao) {
        if let Some(velha) = e.pares[i].anterior.take() {
            e.indices.remove(&velha.meu_indice);
        }
        e.pares[i].anterior = e.pares[i].atual.take();
        e.pares[i].atual = Some(nova);
    }

    fn receber_resposta(&self, dado: &[u8], via: Via, declarada: Option<[u8; 32]>) {
        let Some((remetente, receptor, m2)) = transporte::desembrulhar_resposta(dado) else {
            return;
        };
        let mut e = self.estado.lock().expect("estado");
        let Some(&i) = e.indices.get(&receptor) else {
            return;
        };
        let bate = matches!(&e.pares[i].pendente, Some((idx, _, _)) if *idx == receptor);
        if !bate || declarada.is_some_and(|k| k != e.pares[i].publica) {
            return;
        }
        let (_, ini, _) = e.pares[i].pendente.take().expect("conferido");
        let Ok((chaves, _)) = ini.terminar(m2) else {
            // Aperto que nao fecha (senha da rede errada): o indice morre.
            e.indices.remove(&receptor);
            return;
        };
        let nova = Sessao::nova(chaves, receptor, remetente, true);
        self.trocar_sessao(&mut e, i, nova);
        No::aprender(&mut e.pares[i], via, true);
        e.pares[i].sem_resposta_desde = None;
        // Esvazia a fila; sem nada na fila, um «manter vivo» confirma a
        // sessao do outro lado, que so fala depois de ouvir.
        // Admitido: a ficha do convite ja cumpriu o papel.
        let ficha_usada = self
            .ficha_de_entrada
            .lock()
            .expect("ficha")
            .take()
            .is_some();
        if ficha_usada {
            self.persistir(&e);
        }
        let fila = std::mem::take(&mut e.pares[i].fila);
        let rol = self.rol_para(&e, i);
        e.pares[i].ultimo_rol = Some(Instant::now());
        let s = e.pares[i].atual.as_mut().expect("acabou de entrar");
        let mut saidas: Vec<Vec<u8>> = fila.iter().filter_map(|p| s.selar(p).ok()).collect();
        // A lista de pares vai logo: e ela que confirma a sessao do outro
        // lado (no lugar do «manter vivo») e o apresenta ao resto da malha.
        saidas.extend(rol.iter().filter_map(|m| s.selar(m).ok()));
        e.pares[i].ultimo_envio = Instant::now();
        let chave = e.pares[i].publica;
        drop(e);
        for p in saidas {
            self.mandar(via, &chave, &p);
        }
    }

    fn receber_dados(&self, dado: &[u8], via: Via, declarada: Option<[u8; 32]>) -> Option<Vec<u8>> {
        let receptor = transporte::receptor_de_dados(dado)?;
        let mut e = self.estado.lock().expect("estado");
        let i = *e.indices.get(&receptor)?;
        let par = &mut e.pares[i];
        if declarada.is_some_and(|k| k != par.publica) {
            return None;
        }
        let sessao = [par.atual.as_mut(), par.anterior.as_mut()]
            .into_iter()
            .flatten()
            .find(|s| s.meu_indice == receptor)?;
        let claro = sessao.abrir(dado).ok()?;
        // Pacote autenticado: o par pode ter mudado de endereco (NAT, rede
        // movel). Segue-se o ultimo endereco que PROVOU ter a chave.
        No::aprender(par, via, false);
        par.sem_resposta_desde = None;
        // Quem respondeu acaba de ser confirmado: solta a fila.
        let mut saidas = Vec::new();
        if let Some(s) = par.atual.as_mut().filter(|s| s.confirmada) {
            for p in std::mem::take(&mut par.fila) {
                if let Ok(c) = s.selar(&p) {
                    saidas.push(c);
                }
            }
        }
        let (ip_do_par, chave) = (par.ip, par.publica);
        // Sessao nova do lado de quem respondeu: manda a lista tambem.
        if e.pares[i].ultimo_rol.is_none() {
            let rol = self.rol_para(&e, i);
            if let Some(s) = e.pares[i].atual.as_mut().filter(|s| s.confirmada) {
                saidas.extend(rol.iter().filter_map(|m| s.selar(m).ok()));
                e.pares[i].ultimo_rol = Some(Instant::now());
            }
        }
        if claro.first() == Some(&CONTROLE) {
            match claro.get(1) {
                Some(&CONTROLE_PARES) => {
                    if self.aprender_rol(&mut e, &claro[2..]) {
                        No::avisar_malha(&mut e);
                        self.persistir(&e);
                    }
                }
                Some(&CONTROLE_ROL) => self.receber_rol(&mut e, &claro[2..], i),
                Some(&CONTROLE_ECO) if claro.len() == 10 => {
                    let mut volta = vec![CONTROLE, CONTROLE_ECO_VOLTA];
                    volta.extend_from_slice(&claro[2..10]);
                    if let Some(s) = e.pares[i].atual.as_mut() {
                        saidas.extend(s.selar(&volta).ok());
                    }
                }
                Some(&CONTROLE_ECO_VOLTA) if claro.len() == 10 => {
                    let n = u64::from_le_bytes(claro[2..10].try_into().expect("8"));
                    if let Some((saiu, t)) = self.ecos.lock().expect("ecos").get_mut(&n) {
                        *t = Some(saiu.elapsed());
                    }
                }
                Some(&CONTROLE_CHAT) if claro.len() - 2 <= TETO_CHAT => {
                    if let Ok(texto) = std::str::from_utf8(&claro[2..]) {
                        self.guardar_mensagem(ip_do_par, false, texto.to_string());
                    }
                }
                _ => {}
            }
            drop(e);
            for p in saidas {
                self.mandar(via, &chave, &p);
            }
            return None;
        }
        drop(e);
        for p in saidas {
            self.mandar(via, &chave, &p);
        }
        if claro.is_empty() {
            return None; // manter vivo
        }
        // Roteamento pela chave: a origem de dentro tem de ser o IP do par.
        (origem_ipv4(&claro) == Some(ip_do_par)).then_some(claro)
    }

    /// Chamado a cada segundo: manter vivo, refazer aperto vencido, repetir
    /// aperto sem resposta e comecar com quem tem endereco e nao tem sessao.
    pub fn tique(&self) {
        self.registrar_no_repasse();
        self.anunciar();
        let mut e = self.estado.lock().expect("estado");
        self.sincronizar_rol(&mut e);
        let furos = self.perfurar(&mut e);
        let mut vivos = Vec::new();
        for i in 0..e.pares.len() {
            let surdo = e.pares[i]
                .sem_resposta_desde
                .is_some_and(|t| t.elapsed() >= SURDO_APOS);
            let precisa_aperto = surdo
                || match &e.pares[i].atual {
                    None => true,
                    Some(s) => s.expirada() || s.pede_novo_aperto(),
                };
            if precisa_aperto {
                self.iniciar_aperto(&mut e, i);
                continue;
            }
            if e.pares[i]
                .ultimo_rol
                .map_or(true, |t| t.elapsed() >= REENVIAR_PARES)
            {
                let rol = self.rol_para(&e, i);
                let (via, chave) = (e.pares[i].via, e.pares[i].publica);
                let par = &mut e.pares[i];
                if let (Some(s), Some(v)) = (par.atual.as_mut(), via) {
                    let selados: Vec<Vec<u8>> =
                        rol.iter().filter_map(|m| s.selar(m).ok()).collect();
                    if !selados.is_empty() {
                        par.ultimo_rol = Some(Instant::now());
                        par.ultimo_envio = Instant::now();
                    }
                    vivos.extend(selados.into_iter().map(|p| (v, chave, p)));
                }
            }
            if e.pares[i].ultimo_envio.elapsed() >= MANTER_VIVO {
                let (via, chave) = (e.pares[i].via, e.pares[i].publica);
                if let (Some(s), Some(v)) = (e.pares[i].atual.as_mut(), via) {
                    if let Ok(p) = s.selar(&[]) {
                        vivos.push((v, chave, p));
                        e.pares[i].ultimo_envio = Instant::now();
                    }
                }
            }
        }
        drop(e);
        for (v, k, p) in vivos {
            self.mandar(v, &k, &p);
        }
        for (destino, p) in furos {
            self.enviar(destino, &p);
        }
    }

    /// Renova o registro no repasse (e, com isso, o furo no NAT ate ele).
    fn registrar_no_repasse(&self) {
        let Some(r) = &self.repasse else {
            return;
        };
        let mut ultimo = self.ultimo_registro.lock().expect("registro");
        if ultimo.is_some_and(|t| t.elapsed() < repasse::RENOVAR_REGISTRO) {
            return;
        }
        if let Ok(p) = repasse::registro(
            &self.privada,
            &r.publica,
            transporte::carimbo_agora(),
            r.conta.as_ref(),
        ) {
            self.enviar(r.endereco, &p);
            *ultimo = Some(Instant::now());
        }
    }

    pub fn ip(&self) -> Ipv4Addr {
        self.ip
    }

    /// Manda uma mensagem de controle ao par dono de `ip`, pela sessao que ja
    /// existe. Sem sessao, erro -- controle nao abre aperto sozinho.
    fn mandar_controle(&self, ip: Ipv4Addr, corpo: &[u8]) -> Result<(), String> {
        let mut e = self.estado.lock().expect("estado");
        let i = e
            .pares
            .iter()
            .position(|p| p.ip == ip)
            .ok_or_else(|| format!("{ip} nao e membro desta rede"))?;
        let (via, chave) = (e.pares[i].via, e.pares[i].publica);
        let s = e.pares[i]
            .atual
            .as_mut()
            .filter(|s| s.confirmada && !s.expirada())
            .ok_or_else(|| format!("sem conexao com {ip} agora"))?;
        let p = s.selar(corpo)?;
        e.pares[i].ultimo_envio = Instant::now();
        drop(e);
        self.mandar(via.ok_or("sem caminho para o par")?, &chave, &p);
        Ok(())
    }

    /// Comeca um eco ate `ip`; devolve o numero para `eco_resultado`.
    pub fn eco_comecar(&self, ip: Ipv4Addr) -> Result<u64, String> {
        let n = phxsql_core::cifra::sortear_u64();
        let mut m = vec![CONTROLE, CONTROLE_ECO];
        m.extend_from_slice(&n.to_le_bytes());
        self.ecos
            .lock()
            .expect("ecos")
            .insert(n, (Instant::now(), None));
        if let Err(x) = self.mandar_controle(ip, &m) {
            self.ecos.lock().expect("ecos").remove(&n);
            return Err(x);
        }
        Ok(n)
    }

    /// O tempo de volta do eco `n`, se ja voltou (e o esquece).
    pub fn eco_resultado(&self, n: u64) -> Option<Duration> {
        let mut ecos = self.ecos.lock().expect("ecos");
        let r = ecos.get(&n).and_then(|(_, t)| *t);
        if r.is_some() {
            ecos.remove(&n);
        }
        // Eco que nunca voltou nao fica para sempre.
        ecos.retain(|_, (saiu, _)| saiu.elapsed() < Duration::from_secs(30));
        r
    }

    /// Ping pelo tunel: espera a volta ate `prazo`.
    pub fn pingar(&self, ip: Ipv4Addr, prazo: Duration) -> Result<Duration, String> {
        let n = self.eco_comecar(ip)?;
        let fim = Instant::now() + prazo;
        while Instant::now() < fim {
            if let Some(t) = self.eco_resultado(n) {
                return Ok(t);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(format!("{ip} nao respondeu em {} s", prazo.as_secs()))
    }

    fn guardar_mensagem(&self, ip: Ipv4Addr, minha: bool, texto: String) {
        let mut c = self.caixa.lock().expect("caixa");
        if c.len() >= TETO_CAIXA {
            c.pop_front();
        }
        c.push_back(Mensagem {
            numero: self
                .numero_mensagem
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ip,
            minha,
            texto,
            quando: rede_p2p::agora(),
        });
    }

    /// Chat: manda `texto` ao membro `ip`.
    pub fn conversar(&self, ip: Ipv4Addr, texto: &str) -> Result<(), String> {
        let texto = texto.trim();
        if texto.is_empty() {
            return Err("mensagem vazia".into());
        }
        if texto.len() > TETO_CHAT {
            return Err(format!("mensagem acima de {TETO_CHAT} bytes"));
        }
        let mut m = vec![CONTROLE, CONTROLE_CHAT];
        m.extend_from_slice(texto.as_bytes());
        self.mandar_controle(ip, &m)?;
        self.guardar_mensagem(ip, true, texto.to_string());
        Ok(())
    }

    /// Mensagens depois do numero `desde` (0 = todas as guardadas).
    pub fn mensagens(&self, desde: u64) -> Vec<Mensagem> {
        self.caixa
            .lock()
            .expect("caixa")
            .iter()
            .filter(|m| m.numero > desde)
            .cloned()
            .collect()
    }

    /// Pede para o `rodar` sair (em ate ~1 s).
    pub fn desligar(&self) {
        self.parar.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn desligado(&self) -> bool {
        self.parar.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Uma linha por par, para o console: IP, caminho, idade da sessao e o
    /// comeco da chave publica.
    pub fn situacao(&self) -> Vec<[String; 4]> {
        let e = self.estado.lock().expect("estado");
        e.pares
            .iter()
            .map(|p| {
                let via = match p.via {
                    Some(Via::Direta(a)) => format!("direto {a}"),
                    Some(Via::Repasse) => "repasse".into(),
                    None => "-".into(),
                };
                let sessao = match &p.atual {
                    Some(s) if s.confirmada && !s.expirada() => {
                        format!("{} s", s.idade().as_secs())
                    }
                    Some(_) => "abrindo".into(),
                    None => "sem sessao".into(),
                };
                [
                    p.ip.to_string(),
                    via,
                    sessao,
                    phxsql_core::hash::para_hex(&p.publica[..6]),
                ]
            })
            .collect()
    }
}

/// Liga o no a uma placa TUN e roda para sempre (tres threads: placa, rede e
/// relogio).
#[cfg(any(target_os = "linux", windows))]
pub fn rodar(no: Arc<No>, tun: crate::tun::Tun) -> Result<(), String> {
    let tun = Arc::new(tun);
    let placa = {
        let (no, tun) = (Arc::clone(&no), Arc::clone(&tun));
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 65_535];
            while let Ok(Some(n)) = tun.ler_ou_parar(&mut buf, &no.parar) {
                no.da_placa(&buf[..n]);
            }
        })
    };
    let relogio = {
        let no = Arc::clone(&no);
        std::thread::spawn(move || {
            while !no.desligado() {
                no.tique();
                std::thread::sleep(Duration::from_secs(1));
            }
        })
    };
    let udp = no.udp.try_clone().map_err(|e| e.to_string())?;
    udp.set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 65_535];
    let resultado = loop {
        if no.desligado() {
            break Ok(());
        }
        match udp.recv_from(&mut buf) {
            Ok((n, de)) => {
                if let Some(ip) = no.da_rede(&buf[..n], de) {
                    let _ = tun.escrever(&ip);
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => {
                no.desligar();
                break Err(e.to_string());
            }
        }
    };
    // Espera as outras duas: so entao a ultima referencia da placa cai e o
    // sistema apaga a interface.
    let _ = placa.join();
    let _ = relogio.join();
    resultado
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um pacote IPv4 minimo de `origem` para `destino`.
    fn ip(origem: [u8; 4], destino: [u8; 4], carga: &[u8]) -> Vec<u8> {
        let mut p = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0];
        p.extend_from_slice(&origem);
        p.extend_from_slice(&destino);
        p.extend_from_slice(carga);
        p
    }

    fn dois_nos(senha_b: &str) -> (No, No, SocketAddr, SocketAddr) {
        let (ka, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
        let (ua, ub) = (
            UdpSocket::bind("127.0.0.1:0").unwrap(),
            UdpSocket::bind("127.0.0.1:0").unwrap(),
        );
        let (ea, eb) = (ua.local_addr().unwrap(), ub.local_addr().unwrap());
        for u in [&ua, &ub] {
            u.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        }
        let a = No::novo(
            ka,
            psk_da_rede("R", "senha-1", 1_000),
            "10.78.0.1".parse().unwrap(),
            ua,
            vec![ParConfig {
                publica: x25519::chave_publica(&kb),
                ip: "10.78.0.2".parse().unwrap(),
                endereco: Some(eb),
            }],
        );
        let b = No::novo(
            kb,
            psk_da_rede("R", senha_b, 1_000),
            "10.78.0.2".parse().unwrap(),
            ub,
            vec![ParConfig {
                publica: x25519::chave_publica(&ka),
                ip: "10.78.0.1".parse().unwrap(),
                endereco: None, // B nao sabe onde A esta: aprende pelo aperto
            }],
        );
        (a, b, ea, eb)
    }

    /// Entrega o proximo datagrama que chegou em `no` e devolve o que iria
    /// para a placa.
    fn bombear(no: &No) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; 2048];
        let (n, de) = no.udp.recv_from(&mut buf).ok()?;
        no.da_rede(&buf[..n], de)
    }

    /// Bombeia ate sair um pacote IP na placa (a lista de pares e o manter
    /// vivo passam pelo meio e nao saem na placa).
    fn bombear_ate_ip(no: &No) -> Option<Vec<u8>> {
        for _ in 0..8 {
            if let Some(p) = bombear(no) {
                return Some(p);
            }
        }
        None
    }

    #[test]
    fn pacote_atravessa_por_udp_real_e_volta() {
        let (a, b, _, _) = dois_nos("senha-1");
        let ida = ip([10, 78, 0, 1], [10, 78, 0, 2], b"ping");
        a.da_placa(&ida); // enfileira e manda INICIO
        assert!(bombear(&b).is_none()); // B le INICIO, manda RESPOSTA
        assert!(bombear(&a).is_none()); // A le RESPOSTA, solta a fila
        assert_eq!(
            bombear(&b).unwrap(),
            ida,
            "o pacote chegou inteiro na placa de B"
        );
        let volta = ip([10, 78, 0, 2], [10, 78, 0, 1], b"pong");
        b.da_placa(&volta);
        assert_eq!(bombear_ate_ip(&a).unwrap(), volta);
    }

    fn tentados(no: &No) -> u64 {
        no.apertos_tentados
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// INICIO de quem nao sabe a publica de B morre num HMAC: o contador
    /// que fica ANTES do X25519 nao anda. Com o mac1 certo (e m1 lixo), anda
    /// -- prova de que o contador esta no lugar.
    #[test]
    fn lixo_sem_mac1_morre_antes_do_x25519() {
        let (_a, b, _, _) = dois_nos("senha-1");
        let de: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let pub_b = x25519::chave_publica(&b.privada);
        let lixo = [7u8; 120];
        b.da_rede(&transporte::embrulhar_inicio(1, &lixo, &[3; 32], None), de);
        let mut cru = vec![1, 0, 0, 0, 1, 0, 0, 0];
        cru.extend_from_slice(&[7; 152]);
        b.da_rede(&cru, de);
        assert_eq!(tentados(&b), 0, "lixo chegou ao X25519");
        b.da_rede(&transporte::embrulhar_inicio(1, &lixo, &pub_b, None), de);
        assert_eq!(tentados(&b), 1);
    }

    /// Inundacao de INICIOs com mac1 valido (quem conhece a publica): depois
    /// de 20 no segundo, B so responde cookie, e o X25519 para. O par
    /// legitimo recebe o cookie, manda o mac2 e abre a sessao assim mesmo.
    #[test]
    fn sob_carga_exige_cookie_e_o_par_legitimo_passa() {
        let (a, b, _, eb) = dois_nos("senha-1");
        let pub_b = x25519::chave_publica(&b.privada);
        let atacante = UdpSocket::bind("127.0.0.1:0").unwrap();
        atacante
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        for n in 0..40u32 {
            atacante
                .send_to(
                    &transporte::embrulhar_inicio(n, &[9; 120], &pub_b, None),
                    eb,
                )
                .unwrap();
            bombear(&b);
        }
        assert_eq!(
            tentados(&b),
            CARGA_INICIOS_POR_S as u64,
            "sob carga o X25519 tinha de parar"
        );
        let mut buf = [0u8; 256];
        let (n, _) = atacante.recv_from(&mut buf).unwrap();
        assert_eq!(
            buf[0],
            transporte::TIPO_COOKIE,
            "o atacante devia receber cookie"
        );
        assert_eq!(n, 64);

        // A, legitimo, chega com B sob carga: recebe cookie, nao sessao.
        let ida = ip([10, 78, 0, 1], [10, 78, 0, 2], b"ping");
        a.da_placa(&ida);
        bombear(&b);
        assert_eq!(tentados(&b), CARGA_INICIOS_POR_S as u64);
        bombear(&a);
        assert!(
            a.estado.lock().unwrap().pares[0].cookie.is_some(),
            "A nao guardou o cookie"
        );
        // O relogio do reenvio (5 s) adiantado: A repete o INICIO, com mac2.
        if let Some(p) = a.estado.lock().unwrap().pares[0].pendente.as_mut() {
            p.2 -= REPETIR_APERTO;
        }
        a.da_placa(&ida);
        assert!(bombear(&b).is_none()); // INICIO com mac2: aperto feito
        assert_eq!(tentados(&b), CARGA_INICIOS_POR_S as u64 + 1);
        assert!(bombear(&a).is_none()); // RESPOSTA
        assert_eq!(
            bombear_ate_ip(&b).unwrap()[20..],
            ida[20..],
            "o dado de A tinha de passar"
        );
    }

    #[test]
    fn senha_da_rede_diferente_nao_passa_nada() {
        let (a, b, _, _) = dois_nos("outra-senha");
        a.da_placa(&ip([10, 78, 0, 1], [10, 78, 0, 2], b"x"));
        assert!(bombear(&b).is_none());
        assert!(bombear(&a).is_none()); // RESPOSTA nao fecha em A
        assert!(bombear(&b).is_none(), "nada de dados pode chegar em B");
    }

    #[test]
    fn par_nao_forja_origem_de_outro_endereco() {
        let (a, b, _, _) = dois_nos("senha-1");
        // A manda um pacote dizendo vir de 10.78.0.99 -- nao e o IP dele.
        a.da_placa(&ip([10, 78, 0, 99], [10, 78, 0, 2], b"forjado"));
        bombear(&b);
        bombear(&a);
        assert!(bombear(&b).is_none(), "origem forjada nao entra na placa");
    }

    /// O par reiniciou e perdeu as chaves: quem manda dado e nao ouve nada
    /// em `SURDO_APOS` refaz o aperto sozinho, sem esperar os 120 s.
    #[test]
    fn par_surdo_dispara_aperto_novo() {
        let (a, b, _, _) = dois_nos("senha-1");
        a.da_placa(&ip([10, 78, 0, 1], [10, 78, 0, 2], b"x"));
        bombear(&b);
        bombear(&a);
        bombear(&b);
        a.da_placa(&ip([10, 78, 0, 1], [10, 78, 0, 2], b"y"));
        {
            let mut e = a.estado.lock().unwrap();
            let t = e.pares[0].sem_resposta_desde.expect("mandou sem ouvir");
            e.pares[0].sem_resposta_desde = Some(t - SURDO_APOS);
        }
        a.tique();
        let mut buf = vec![0u8; 2048];
        // B recebe o dado "y" e, logo atras, um INICIO novo.
        let mut tipos = Vec::new();
        while let Ok((n, _)) = b.udp.recv_from(&mut buf) {
            tipos.push(buf[0]);
            if n > 0 && buf[0] == transporte::TIPO_INICIO {
                break;
            }
        }
        assert!(
            tipos.contains(&transporte::TIPO_INICIO),
            "tipos vistos: {tipos:?}"
        );
    }

    /// Os dois sem endereco um do outro (como atras de CGNAT): so o repasse
    /// liga. E o repasse so ve pacote cifrado.
    #[test]
    fn pelo_repasse_o_pacote_atravessa_cifrado() {
        let r_priv = x25519::gerar_privada();
        let mut rep = repasse::Repasse::novo(r_priv, None);
        let ur = UdpSocket::bind("127.0.0.1:0").unwrap();
        ur.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let er = ur.local_addr().unwrap();
        let (ka, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
        let novo = |k: [u8; 32], meu: &str, dele: [u8; 32], ip_dele: &str| {
            let u = UdpSocket::bind("127.0.0.1:0").unwrap();
            u.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            No::novo(
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
                Modo::Repasse,
                Some(RepasseCfg {
                    endereco: er,
                    publica: x25519::chave_publica(&r_priv),
                    conta: None,
                }),
            )
            .unwrap()
        };
        let a = novo(ka, "10.78.0.1", x25519::chave_publica(&kb), "10.78.0.2");
        let b = novo(kb, "10.78.0.2", x25519::chave_publica(&ka), "10.78.0.1");
        // O repasse gira num laco proprio; guarda o que viu passar.
        let visto = Arc::new(Mutex::new(Vec::<u8>::new()));
        let v2 = Arc::clone(&visto);
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            while let Ok((n, de)) = ur.recv_from(&mut buf) {
                v2.lock().unwrap().extend_from_slice(&buf[..n]);
                if let Some((alvo, p)) = rep.tratar(&buf[..n], de) {
                    let _ = ur.send_to(&p, alvo);
                }
            }
        });
        a.tique();
        b.tique();
        std::thread::sleep(Duration::from_millis(100));
        let ida = ip([10, 78, 0, 1], [10, 78, 0, 2], b"SEGREDO-DO-PING");
        a.da_placa(&ida);
        // Os dois ja iniciaram aperto no tique (cruzado): bombeia os dois
        // lados ate o pacote sair na placa de B.
        for u in [&a.udp, &b.udp] {
            u.set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
        }
        let mut chegou = false;
        for _ in 0..40 {
            bombear(&a);
            if bombear(&b).as_deref() == Some(&ida[..]) {
                chegou = true;
                break;
            }
        }
        assert!(chegou, "o pacote nao atravessou pelo repasse");
        let visto = visto.lock().unwrap();
        assert!(
            !visto.windows(15).any(|w| w == b"SEGREDO-DO-PING"),
            "o repasse viu texto claro"
        );
    }

    #[test]
    fn modo_repasse_sem_repasse_e_recusado() {
        let u = UdpSocket::bind("127.0.0.1:0").unwrap();
        let n = No::novo([1; 32], [0; 32], "10.78.0.1".parse().unwrap(), u, vec![]);
        assert!(n.com_repasse(Modo::Auto, None).is_err());
    }

    /// Convite feito com o no LIGADO: a ficha e aceita uma vez, e regravar o
    /// arquivo depois nao a ressuscita.
    #[test]
    fn ficha_de_convite_admite_uma_vez_e_nao_ressuscita() {
        let dir = std::env::temp_dir().join(format!("phxvpn-ficha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("R.p2p").to_str().unwrap().to_string();
        let _ = std::fs::remove_file(&caminho);
        let (ka, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
        let psk = psk_da_rede("R", "s", 1_000);
        let mut rede = Rede::nova("R", "10.78.0.1".parse().unwrap(), 24, 0);
        rede.gravar(&caminho).unwrap();
        let ua = UdpSocket::bind("127.0.0.1:0").unwrap();
        let ea = ua.local_addr().unwrap();
        ua.set_read_timeout(Some(Duration::from_millis(300)))
            .unwrap();
        let a = No::novo(ka, psk, "10.78.0.1".parse().unwrap(), ua, vec![])
            .com_rede(rede.clone(), caminho.clone());
        // O convite nasce DEPOIS de o no ligar, por outro processo (o disco).
        let codigo =
            rede_p2p::convidar(&mut rede, &psk, x25519::chave_publica(&ka), None, 60).unwrap();
        rede.gravar(&caminho).unwrap();
        let c = rede_p2p::abrir_convite(&codigo, &psk).unwrap();
        let ub = UdpSocket::bind("127.0.0.1:0").unwrap();
        ub.set_read_timeout(Some(Duration::from_millis(300)))
            .unwrap();
        let mut rb = rede_p2p::rede_do_convidado(&c, 0);
        rb.pares[0].endereco = Some(ea.to_string());
        let b = No::novo(
            kb,
            psk,
            c.ip_convidado,
            ub,
            vec![ParConfig {
                publica: x25519::chave_publica(&ka),
                ip: "10.78.0.1".parse().unwrap(),
                endereco: Some(ea),
            }],
        )
        .com_rede(rb, dir.join("B.p2p").to_str().unwrap().to_string());
        b.tique(); // INICIO com a ficha
        bombear(&a);
        assert_eq!(
            a.estado.lock().unwrap().pares.len(),
            1,
            "a ficha tinha de admitir"
        );
        assert!(
            Rede::ler(&caminho).unwrap().convites.is_empty(),
            "ficha usada ficou aberta"
        );
        // Qualquer regravacao depois (a malha muda o tempo todo) nao a traz de volta.
        a.persistir(&a.estado.lock().unwrap());
        assert!(
            Rede::ler(&caminho).unwrap().convites.is_empty(),
            "a ficha usada ressuscitou"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ping e chat pelo tunel, por UDP real: o eco volta com o tempo, o texto
    /// chega do lado de la marcado com o IP de quem mandou.
    #[test]
    fn ping_e_chat_pelo_tunel() {
        let (a, b, _, _) = dois_nos("senha-1");
        for u in [&a.udp, &b.udp] {
            u.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        }
        // Bombeia os dois lados ate `feito` valer (a lista de pares e os
        // mantem-vivo cruzam no meio, em ordem que o teste nao controla).
        let ate = |feito: &dyn Fn() -> bool| {
            for _ in 0..60 {
                if feito() {
                    return true;
                }
                bombear(&a);
                bombear(&b);
            }
            feito()
        };
        a.da_placa(&ip([10, 78, 0, 1], [10, 78, 0, 2], b"x"));
        let ipb: Ipv4Addr = "10.78.0.2".parse().unwrap();
        assert!(ate(&|| b.estado.lock().unwrap().pares[0]
            .atual
            .as_ref()
            .is_some_and(|s| s.confirmada)));
        let n = a.eco_comecar(ipb).unwrap();
        let volta = std::cell::Cell::new(None);
        assert!(
            ate(&|| {
                if volta.get().is_none() {
                    volta.set(a.eco_resultado(n));
                }
                volta.get().is_some()
            }),
            "o eco nao voltou"
        );
        a.conversar(ipb, "Bom dia, filial!").unwrap();
        assert!(ate(&|| !b.mensagens(0).is_empty()));
        let m = b.mensagens(0);
        assert_eq!(m[0].texto, "Bom dia, filial!");
        assert_eq!(m[0].ip, "10.78.0.1".parse::<Ipv4Addr>().unwrap());
        assert!(!m[0].minha && a.mensagens(0)[0].minha);
        assert!(a.conversar(ipb, &"x".repeat(TETO_CHAT + 1)).is_err());
        assert!(
            a.conversar("10.78.0.9".parse().unwrap(), "oi").is_err(),
            "nao-membro"
        );
    }

    #[test]
    fn inicio_repetido_nao_abre_sessao_nova() {
        let (a, b, _, eb) = dois_nos("senha-1");
        a.da_placa(&ip([10, 78, 0, 1], [10, 78, 0, 2], b"x"));
        let mut buf = vec![0u8; 2048];
        let (n, de) = b.udp.recv_from(&mut buf).unwrap();
        let gravado = buf[..n].to_vec();
        b.da_rede(&gravado, de);
        bombear(&a);
        let indices_antes = b.estado.lock().unwrap().indices.len();
        b.da_rede(&gravado, de); // o atacante reenvia o INICIO gravado
        assert_eq!(b.estado.lock().unwrap().indices.len(), indices_antes);
        let _ = eb;
    }
}
