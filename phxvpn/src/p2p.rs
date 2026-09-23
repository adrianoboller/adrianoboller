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
//! Rol assinado, descoberta (convite, LAN, farol) e o repasse para CGNAT sao
//! as proximas pecas; hoje a lista de pares vem da linha de comando.

use crate::noise;
use crate::repasse;
use crate::transporte::{self, Sessao};
use phxsql_core::hash::pbkdf2_sha256;
use phxsql_core::senha::bytes_aleatorios;
use phxsql_core::x25519;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
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

/// Tentativas diretas sem resposta antes de o modo `auto` ir ao repasse.
const TENTATIVAS_DIRETAS: u8 = 2;

pub struct RepasseCfg {
    pub endereco: SocketAddr,
    pub publica: [u8; 32],
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
    ultimo_registro: Mutex<Option<Instant>>,
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
            ultimo_registro: Mutex::new(None),
        }
    }

    /// Liga o servidor intermediario e escolhe o modo.
    pub fn com_repasse(mut self, modo: Modo, repasse: Option<RepasseCfg>) -> Result<No, String> {
        if modo != Modo::Direto && repasse.is_none() {
            return Err("os modos repasse e auto precisam de --repasse CHAVE@HOST:PORTA".into());
        }
        self.modo = modo;
        self.repasse = repasse;
        Ok(self)
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
    fn aprender(par: &mut Par, via: Via) {
        par.via = Some(via);
        if let Via::Direta(a) = via {
            par.endereco = Some(a);
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
        let par = &mut e.pares[i];
        let via = match (self.modo, par.endereco) {
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
        let Ok((ini, m1)) = noise::Iniciador::comecar(
            noise::PROLOGO,
            self.privada,
            &publica,
            self.psk,
            &transporte::carimbo_agora(),
        ) else {
            return;
        };
        if let Some((velho, _, _)) = e.pares[i].pendente.take() {
            e.indices.remove(&velho);
        }
        e.indices.insert(indice, i);
        e.pares[i].pendente = Some((indice, ini, Instant::now()));
        self.mandar(via, &publica, &transporte::embrulhar_inicio(indice, &m1));
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
            _ => None,
        }
    }

    fn receber_inicio(&self, dado: &[u8], via: Via, declarada: Option<[u8; 32]>) {
        let Some((remetente, m1)) = transporte::desembrulhar_inicio(dado) else {
            return;
        };
        let Ok(chamada) = noise::ler_chamada(noise::PROLOGO, &self.privada, m1) else {
            return;
        };
        let mut e = self.estado.lock().expect("estado");
        // Admissao: so quem esta na lista.
        let Some(i) = e
            .pares
            .iter()
            .position(|p| p.publica == chamada.estatica_dele)
        else {
            return;
        };
        if declarada.is_some_and(|k| k != chamada.estatica_dele) {
            return;
        }
        // Repeticao: um INICIO gravado e reenviado tem carimbo velho.
        let Ok(carimbo) = <[u8; 12]>::try_from(chamada.carga.as_slice()) else {
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
        No::aprender(&mut e.pares[i], via);
        let chave = e.pares[i].publica;
        drop(e);
        self.mandar(
            via,
            &chave,
            &transporte::embrulhar_resposta(indice, remetente, &m2),
        );
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
        No::aprender(&mut e.pares[i], via);
        e.pares[i].sem_resposta_desde = None;
        // Esvazia a fila; sem nada na fila, um «manter vivo» confirma a
        // sessao do outro lado, que so fala depois de ouvir.
        let fila = std::mem::take(&mut e.pares[i].fila);
        let s = e.pares[i].atual.as_mut().expect("acabou de entrar");
        let mut saidas: Vec<Vec<u8>> = fila.iter().filter_map(|p| s.selar(p).ok()).collect();
        if saidas.is_empty() {
            saidas.extend(s.selar(&[]).ok());
        }
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
        No::aprender(par, via);
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
        let mut e = self.estado.lock().expect("estado");
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
        if let Ok(p) = repasse::registro(&self.privada, &r.publica, transporte::carimbo_agora()) {
            self.enviar(r.endereco, &p);
            *ultimo = Some(Instant::now());
        }
    }

    pub fn ip(&self) -> Ipv4Addr {
        self.ip
    }
}

/// Liga o no a uma placa TUN e roda para sempre (tres threads: placa, rede e
/// relogio).
#[cfg(target_os = "linux")]
pub fn rodar(no: Arc<No>, tun: crate::tun::Tun) -> Result<(), String> {
    let tun = Arc::new(tun);
    {
        let (no, tun) = (Arc::clone(&no), Arc::clone(&tun));
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 65_535];
            while let Ok(n) = tun.ler(&mut buf) {
                no.da_placa(&buf[..n]);
            }
        });
    }
    {
        let no = Arc::clone(&no);
        std::thread::spawn(move || loop {
            no.tique();
            std::thread::sleep(Duration::from_secs(1));
        });
    }
    let udp = no.udp.try_clone().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 65_535];
    loop {
        let (n, de) = udp.recv_from(&mut buf).map_err(|e| e.to_string())?;
        if let Some(ip) = no.da_rede(&buf[..n], de) {
            let _ = tun.escrever(&ip);
        }
    }
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
        assert_eq!(bombear(&a).unwrap(), volta);
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
