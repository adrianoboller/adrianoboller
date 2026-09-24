//! O FAROL: um membro alcancavel da propria rede faz o papel do repasse
//! para os outros -- registro, apresentacao para a perfuracao e, quando ela
//! nao passa (NAT simetrico), o rele dos dados. Com um farol, a rede anda sem
//! nenhum `phxvpn repasse` separado.
//!
//! # Um motor so
//!
//! O farol NAO e um segundo repasse. O lado de quem serve e o mesmo
//! `repasse::Repasse` (a mesma conferencia do REGISTRO, o mesmo `PARA`/`DE`,
//! a mesma `Mesa` de apresentacoes), so que no soquete do no em vez de num
//! processo proprio. O lado de quem usa e o mesmo `fio::FioRepasse` e o mesmo
//! `perfuracao.rs`. O que mora aqui e so o que o repasse separado nao tinha:
//! quem pode ser farol, quem pode usa-lo, os tetos, e a escolha entre varios.
//!
//! # Quem e farol: o rol, e nao o proprio membro
//!
//! O endereco do farol esta no rol ASSINADO pelo dono (`rol.rs`, v2). Se o
//! membro pudesse se declarar farol, qualquer um atrairia o trafego dos
//! outros -- cifrado, mas desviado: o desviador ve quem fala com quem, quando
//! e quanto, e pode simplesmente descartar. E o membro tambem precisa
//! CONSENTIR (`--farol` ou `"farol": true` no arquivo dele): servir custa
//! banda dele. A autoridade e do dono; o consentimento, de quem serve.
//!
//! # Quem usa o farol: so o rol
//!
//! A lista de permitidas do `Repasse` e o rol aceito, relida a cada versao
//! nova. Um no fora do rol nao registra (a conferencia vem ANTES do
//! Diffie-Hellman), nao manda `PARA`, nao pede apresentacao -- nada de
//! refletor aberto. E quem sai do rol sai da tabela na hora
//! (`Repasse::trocar_permitidas`), nao quando o registro vence.
//!
//! # O farol nao le o trafego
//!
//! Ele so ve `PARA chave | pacote Noise`: o pacote e o mesmo que iria pelo
//! repasse, cifrado de ponta a ponta com a sessao dos dois pares. O farol
//! tem a PSK da rede (e membro), mas nao as chaves de sessao dos outros:
//! abrir um DADOS de B para C exigiria a privada de B ou de C.
//!
//! # A chave do farol e a identidade do membro
//!
//! O `DH(no, farol)` que prova o REGISTRO sai da MESMA X25519 do aperto
//! Noise. Nao e reuso perigoso: o Noise usa esse DH como ENTRADA do HKDF
//! (`ss`), aqui ele e CHAVE de um HMAC com rotulo proprio; nenhum dos dois o
//! revela. E e o que poupa um segredo novo em disco e um campo a mais no rol:
//! quem conhece o membro ja conhece a chave do farol. O Tailscale faz o mesmo
//! com a chave do no para autenticar no DERP.
//!
//! # Varios farois (e o repasse externo junto)
//!
//! O no se registra em TODOS os farois do rol (um REGISTRO de 80 bytes a
//! cada 20 s cada). Dado vai pelo farol por onde o par foi ouvido por
//! ultimo, se ele ainda confirma; aperto (INICIO/RESPOSTA/cookie) vai por
//! todos os que confirmam -- e o que faz o par cair para outro farol quando
//! um some: o dado para, o par fica surdo (15 s), o aperto novo sai por
//! todos, e o primeiro que responder vira o caminho. Nenhum confirmando,
//! vai por todos os do rol (o primeiro REGISTRO pode ainda estar no ar). O
//! repasse externo (`--repasse`) entra na conta como mais um.
//!
//! # Tetos
//!
//! Banda total (`--farol-mbit`, padrao 100 Mbit/s) e por origem (metade),
//! medidas em balde de fichas; REGISTRO no maximo 50/s e 5/s por IP, ANTES
//! do Diffie-Hellman. Pacote acima do teto se perde, como no UDP -- o farol
//! nao enfileira nada, entao nao acumula memoria por causa do papel.

use super::{Modo, No, Via};
use crate::fio::{CfgFio, FioRepasse};
use crate::repasse::{self, Ponta, Repasse};
use crate::transporte;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

/// Teto padrao de banda repassada pelo farol, em Mbit/s.
pub const MBIT_PADRAO: u32 = 100;
/// REGISTROs aceitos para conferencia (DH) por segundo, no total.
pub const REGISTROS_POR_S: u32 = 50;
/// ... e por IP de origem.
pub const REGISTROS_POR_IP_POR_S: u32 = 5;
/// De quanto em quanto tempo o farol diz o que repassou (so se mudou).
const RELATAR: Duration = Duration::from_secs(5);

/// Por qual intermediario um par foi ouvido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rele {
    Externo,
    Farol([u8; 32]),
}

/// Balde de fichas: `por_s` por segundo, ate `cap` guardadas.
struct Balde {
    fichas: f64,
    cap: f64,
    por_s: f64,
    visto: Instant,
}

impl Balde {
    fn novo(por_s: f64, cap: f64) -> Balde {
        Balde {
            fichas: cap,
            cap,
            por_s,
            visto: Instant::now(),
        }
    }

    fn tirar(&mut self, n: f64) -> bool {
        let agora = Instant::now();
        let dt = agora.duration_since(self.visto).as_secs_f64();
        self.visto = agora;
        self.fichas = (self.fichas + dt * self.por_s).min(self.cap);
        if self.fichas >= n {
            self.fichas -= n;
            true
        } else {
            false
        }
    }
}

/// O que o farol fez, para o registro e para a prova.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contas {
    pub repassados: u64,
    pub bytes: u64,
    /// REGISTRO/PARA/APRESENTAR que o repasse recusou (fora do rol, mac
    /// errado, endereco nao registrado).
    pub recusados: u64,
    /// Cortados pelos tetos de banda ou de registro.
    pub cortados: u64,
    pub registrados: u64,
}

/// O lado de quem SERVE.
struct Servidor {
    repasse: Repasse,
    banda_total: Balde,
    banda_por_origem: HashMap<SocketAddr, Balde>,
    bytes_por_s: f64,
    registros: Balde,
    registros_por_ip: HashMap<IpAddr, Balde>,
    contas: Contas,
    relatado: (Instant, Contas),
}

impl Servidor {
    fn novo(privada: [u8; 32], permitidas: HashSet<[u8; 32]>, mbit: u32) -> Servidor {
        let bytes_por_s = mbit.max(1) as f64 * 1_000_000.0 / 8.0;
        Servidor {
            repasse: Repasse::novo(privada, Some(permitidas)),
            // Rajada de 1/4 s: absorve o arranque de uma janela TCP sem
            // deixar um par sozinho segurar o farol.
            banda_total: Balde::novo(bytes_por_s, (bytes_por_s / 4.0).max(65_536.0)),
            banda_por_origem: HashMap::new(),
            bytes_por_s,
            registros: Balde::novo(REGISTROS_POR_S as f64, REGISTROS_POR_S as f64),
            registros_por_ip: HashMap::new(),
            contas: Contas::default(),
            relatado: (Instant::now(), Contas::default()),
        }
    }

    /// Um pacote de repasse que chegou ao soquete do farol.
    fn tratar(&mut self, dado: &[u8], de: SocketAddr) -> Option<(SocketAddr, Vec<u8>)> {
        let tipo = *dado.first()?;
        if tipo == repasse::TIPO_REGISTRO {
            // Os tetos vem ANTES do Diffie-Hellman que o REGISTRO custa.
            if self.registros_por_ip.len() > 4096 {
                self.registros_por_ip.clear();
            }
            let por_s = REGISTROS_POR_IP_POR_S as f64;
            let ip_ok = self
                .registros_por_ip
                .entry(de.ip())
                .or_insert_with(|| Balde::novo(por_s, por_s))
                .tirar(1.0);
            if !ip_ok || !self.registros.tirar(1.0) {
                self.contas.cortados += 1;
                return None;
            }
            let r = self.repasse.tratar(dado, de);
            match r {
                Some(_) => self.contas.registrados += 1,
                None => self.contas.recusados += 1,
            }
            return r;
        }
        // PARA e APRESENTAR: so de endereco registrado -- e so entao ha
        // balde por origem, para endereco forjado nao criar entrada.
        if !self.repasse.registrado(Ponta::Udp(de)) {
            self.contas.recusados += 1;
            return None;
        }
        if tipo == repasse::TIPO_PARA {
            let n = dado.len() as f64;
            let meia = self.bytes_por_s / 2.0;
            if self.banda_por_origem.len() > repasse::TETO_NOS {
                self.banda_por_origem.clear();
            }
            let origem_ok = self
                .banda_por_origem
                .entry(de)
                .or_insert_with(|| Balde::novo(meia, (meia / 4.0).max(65_536.0)))
                .tirar(n);
            if !origem_ok || !self.banda_total.tirar(n) {
                self.contas.cortados += 1;
                return None;
            }
        }
        let r = self.repasse.tratar(dado, de);
        match (&r, tipo) {
            (Some(_), repasse::TIPO_PARA) => {
                self.contas.repassados += 1;
                self.contas.bytes += dado.len() as u64;
            }
            (None, repasse::TIPO_PARA) => self.contas.recusados += 1,
            _ => {}
        }
        r
    }
}

/// Um farol do rol, do lado de quem USA.
struct Candidato {
    chave: [u8; 32],
    endereco: SocketAddr,
    fio: FioRepasse,
    segredo: [u8; 32],
    ultimo_registro: Option<Instant>,
    confirmado: bool,
}

/// O estado do farol no no: o que serve e o que usa.
pub(super) struct Farois {
    /// Este no aceita servir (o consentimento; a autoridade e do rol).
    consentido: bool,
    mbit: u32,
    /// A perfuracao pelo farol (desligada com `--sem-perfuracao`).
    pub(super) perfurar: bool,
    servidor: Option<Servidor>,
    candidatos: Vec<Candidato>,
    versao_vista: u64,
    rotas: HashMap<[u8; 32], Rele>,
    avisado_sem_marca: bool,
}

impl Default for Farois {
    fn default() -> Self {
        Farois {
            consentido: false,
            mbit: MBIT_PADRAO,
            perfurar: true,
            servidor: None,
            candidatos: Vec::new(),
            versao_vista: 0,
            rotas: HashMap::new(),
            avisado_sem_marca: false,
        }
    }
}

impl Farois {
    fn confirmado(&self, rele: Rele) -> bool {
        match rele {
            Rele::Externo => true,
            Rele::Farol(k) => self
                .candidatos
                .iter()
                .any(|c| c.chave == k && c.fio.confirmado()),
        }
    }
}

impl No {
    /// Este no aceita servir de farol, com o teto de banda dado (Mbit/s).
    /// So serve se o rol do dono o marcar.
    pub fn com_farol(self, servir: bool, mbit: Option<u32>) -> No {
        {
            let mut f = self.farois.lock().expect("farol");
            f.consentido = servir;
            if let Some(m) = mbit {
                f.mbit = m.max(1);
            }
        }
        self
    }

    /// Modo `auto`/`repasse` sem `--repasse`: vale quando ha farol no rol.
    /// Quem chama confere isso -- o `com_repasse` continua recusando o modo
    /// sem intermediario nenhum.
    pub fn com_modo_so_farol(mut self, modo: Modo) -> No {
        self.modo = modo;
        self
    }

    /// O que o farol deste no repassou (zeros se nao serve).
    pub fn contas_do_farol(&self) -> Option<Contas> {
        let f = self.farois.lock().expect("farol");
        f.servidor.as_ref().map(|s| s.contas)
    }

    /// Um datagrama que pode ser do papel de farol: `None` = nao e (segue
    /// para o `despachar`); `Some(ip)` = tratado aqui.
    pub(super) fn farol_da_rede(&self, dado: &[u8], de: SocketAddr) -> Option<Option<Vec<u8>>> {
        let tipo = *dado.first()?;
        match tipo {
            repasse::TIPO_REGISTRO | repasse::TIPO_PARA | super::perfuracao::TIPO_APRESENTAR => {
                let saida = {
                    let mut f = self.farois.lock().expect("farol");
                    f.servidor.as_mut()?.tratar(dado, de)
                };
                if let Some((alvo, p)) = saida {
                    self.enviar(alvo, &p);
                }
                Some(None)
            }
            repasse::TIPO_DE | repasse::TIPO_CONFIRMA | super::perfuracao::TIPO_APRESENTACAO => {
                let mut f = self.farois.lock().expect("farol");
                let i = f.candidatos.iter().position(|c| c.endereco == de)?;
                let c = &mut f.candidatos[i];
                if tipo == repasse::TIPO_CONFIRMA {
                    c.fio.chegou(dado);
                    if !c.confirmado && c.fio.confirmado() {
                        c.confirmado = true;
                        eprintln!("phxvpn: farol {de} confirmou o registro");
                    }
                    return Some(None);
                }
                if tipo == super::perfuracao::TIPO_APRESENTACAO {
                    let (segredo, perfurar) = (c.segredo, f.perfurar);
                    drop(f);
                    if perfurar && self.modo == Modo::Auto {
                        self.apresentado(dado, &segredo);
                    }
                    return Some(None);
                }
                let chave = c.chave;
                let Some((origem, dentro)) = repasse::desembrulhar_de(dado) else {
                    return Some(None);
                };
                let nova = f.rotas.insert(origem, Rele::Farol(chave)) != Some(Rele::Farol(chave));
                drop(f);
                if nova {
                    let ip = self.rede.lock().expect("rede").as_ref().and_then(|(r, _)| {
                        r.rol.as_ref().and_then(|x| x.membro(&origem)).map(|m| m.ip)
                    });
                    if let Some(ip) = ip {
                        eprintln!("phxvpn: {ip} -- pelo farol {de}");
                    }
                }
                Some(self.despachar(dentro, Via::Repasse, Some(origem)))
            }
            _ => None,
        }
    }

    /// O par foi ouvido pelo repasse externo.
    pub(super) fn ouvido_pelo_externo(&self, origem: [u8; 32]) {
        let mut f = self.farois.lock().expect("farol");
        if !f.candidatos.is_empty() {
            f.rotas.insert(origem, Rele::Externo);
        }
    }

    /// Manda um `PARA` (ja embrulhado para `chave_dele`) pelo intermediario:
    /// o farol por onde o par foi ouvido, ou todos -- ver o topo do arquivo.
    /// Sem farol no rol, e exatamente o `ao_repasse` de antes.
    pub(super) fn ao_rele(&self, chave_dele: &[u8; 32], embrulhado: &[u8]) {
        let aperto = matches!(
            embrulhado.get(36),
            Some(&transporte::TIPO_INICIO | &transporte::TIPO_RESPOSTA | &transporte::TIPO_COOKIE)
        );
        let f = self.farois.lock().expect("farol");
        if f.candidatos.is_empty() {
            drop(f);
            self.ao_repasse(embrulhado);
            return;
        }
        // O par E um farol: ele e alcancavel por definicao, entao o pacote
        // vai direto a ele. Embrulhado, morreria no proprio farol -- ele nao
        // esta registrado na tabela dele mesmo.
        if let Some(c) = f.candidatos.iter().find(|c| c.chave == *chave_dele) {
            if let Some(dentro) = embrulhado.get(36..) {
                let _ = self.udp.send_to(dentro, c.endereco);
            }
            return;
        }
        let rota = f
            .rotas
            .get(chave_dele)
            .copied()
            .filter(|r| *r != Rele::Externo || self.fio.is_some())
            .filter(|r| f.confirmado(*r));
        let alvos: Vec<Rele> = match rota {
            Some(r) if !aperto => vec![r],
            _ => {
                let mut v: Vec<Rele> = f
                    .candidatos
                    .iter()
                    .filter(|c| c.fio.confirmado())
                    .map(|c| Rele::Farol(c.chave))
                    .collect();
                if v.is_empty() {
                    v = f.candidatos.iter().map(|c| Rele::Farol(c.chave)).collect();
                }
                if self.fio.is_some() {
                    v.push(Rele::Externo);
                }
                v
            }
        };
        for a in &alvos {
            if let Rele::Farol(k) = a {
                if let Some(c) = f.candidatos.iter().find(|c| c.chave == *k) {
                    c.fio.enviar(&self.udp, embrulhado);
                }
            }
        }
        drop(f);
        if alvos.contains(&Rele::Externo) {
            self.ao_repasse(embrulhado);
        }
    }

    /// Para a perfuracao do par `chave`: o endereco e o segredo do
    /// intermediario por onde ele foi ouvido. `externo` e o do repasse
    /// separado, quando ha.
    pub(super) fn rele_para_perfurar(
        &self,
        chave: &[u8; 32],
        externo: Option<(SocketAddr, [u8; 32])>,
    ) -> Option<(SocketAddr, [u8; 32])> {
        let f = self.farois.lock().expect("farol");
        match f.rotas.get(chave) {
            Some(Rele::Farol(k)) if f.perfurar && self.modo == Modo::Auto => f
                .candidatos
                .iter()
                .find(|c| c.chave == *k)
                .map(|c| (c.endereco, c.segredo)),
            Some(Rele::Farol(_)) => None,
            _ => externo,
        }
    }

    /// A cada tique: segue o rol (quem e farol, quem pode usar), registra
    /// nos farois e relata o que este farol repassou.
    pub(super) fn farol_tique(&self) {
        let (versao, farois, membros) = {
            let r = self.rede.lock().expect("rede");
            match r.as_ref().and_then(|(r, _)| r.dono.and(r.rol.as_ref())) {
                Some(rol) => (
                    rol.versao,
                    rol.farois().collect::<Vec<_>>(),
                    rol.membros.iter().map(|m| m.chave).collect::<HashSet<_>>(),
                ),
                None => return,
            }
        };
        let mut f = self.farois.lock().expect("farol");
        if versao != f.versao_vista {
            f.versao_vista = versao;
            let marcado = farois.iter().any(|(k, _)| *k == self.minha_publica);
            match (marcado && f.consentido, f.servidor.as_mut()) {
                (true, Some(s)) => s.repasse.trocar_permitidas(membros),
                (true, None) => {
                    eprintln!(
                        "phxvpn: farol ligado -- repassa para os {} membros do rol, teto {} Mbit/s",
                        membros.len(),
                        f.mbit
                    );
                    f.servidor = Some(Servidor::novo(self.privada, membros, f.mbit));
                }
                (false, s) => {
                    if s.is_some() {
                        eprintln!("phxvpn: farol desligado (o rol nao marca mais este no)");
                    }
                    f.servidor = None;
                    if f.consentido && !marcado && !f.avisado_sem_marca {
                        f.avisado_sem_marca = true;
                        eprintln!(
                            "phxvpn: --farol pedido, mas o rol do dono nao marca este no: \
                             nao serve (o dono marca com p2p farol)"
                        );
                    }
                }
            }
            // Os candidatos: os farois do rol, menos este no. O fio de quem
            // ja estava fica (o registro confirmado nao recomeca).
            let mut velhos = std::mem::take(&mut f.candidatos);
            for (k, end) in farois {
                if k == self.minha_publica {
                    continue;
                }
                if let Some(p) = velhos
                    .iter()
                    .position(|c| c.chave == k && c.endereco == end)
                {
                    f.candidatos.push(velhos.swap_remove(p));
                    continue;
                }
                let (Ok(fio), Some(segredo)) = (
                    FioRepasse::novo(&self.privada, &k, end, CfgFio::default()),
                    super::perfuracao::segredo(&self.privada, &k),
                ) else {
                    continue;
                };
                f.candidatos.push(Candidato {
                    chave: k,
                    endereco: end,
                    fio,
                    segredo,
                    ultimo_registro: None,
                    confirmado: false,
                });
            }
            let cands: HashSet<[u8; 32]> = f.candidatos.iter().map(|c| c.chave).collect();
            f.rotas.retain(|_, r| {
                matches!(r, Rele::Externo) || matches!(r, Rele::Farol(k) if cands.contains(k))
            });
        }
        if self.modo != Modo::Direto {
            for c in &mut f.candidatos {
                if !c.fio.registro_devido(c.ultimo_registro) {
                    continue;
                }
                if let Ok(p) =
                    repasse::registro(&self.privada, &c.chave, transporte::carimbo_agora(), None)
                {
                    c.fio.enviar(&self.udp, &p);
                    c.fio.registro_enviado();
                    c.ultimo_registro = Some(Instant::now());
                }
                if c.confirmado && !c.fio.confirmado() {
                    c.confirmado = false;
                    eprintln!("phxvpn: farol {} parou de confirmar", c.endereco);
                }
            }
        }
        if let Some(s) = f.servidor.as_mut() {
            if s.relatado.0.elapsed() >= RELATAR && s.relatado.1 != s.contas {
                let c = s.contas;
                eprintln!(
                    "phxvpn: farol -- {} repassados ({} bytes), {} registros, {} recusados, {} cortados pelo teto",
                    c.repassados, c.bytes, c.registrados, c.recusados, c.cortados
                );
                s.relatado = (Instant::now(), c);
            }
        }
    }

    /// Para o console: por qual farol o par esta, se esta por um.
    pub(super) fn descrever_rele(&self, chave: &[u8; 32]) -> Option<String> {
        let f = self.farois.lock().expect("farol");
        match f.rotas.get(chave)? {
            Rele::Farol(k) => f
                .candidatos
                .iter()
                .find(|c| c.chave == *k)
                .map(|c| format!("farol {}", c.endereco)),
            Rele::Externo => None,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::super::{psk_da_rede, ParConfig, RepasseCfg};
    use super::*;
    use crate::rede_p2p::Rede;
    use crate::rol;
    use phxsql_core::x25519;
    use std::net::UdpSocket;

    fn pacote_ip(origem: [u8; 4], destino: [u8; 4], carga: &[u8]) -> Vec<u8> {
        let mut p = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0];
        p.extend_from_slice(&origem);
        p.extend_from_slice(&destino);
        p.extend_from_slice(carga);
        p
    }

    fn pasta(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "phxvpn-farol-{nome}-{}-{}",
            std::process::id(),
            phxsql_core::cifra::sortear_u64()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn soquete() -> (UdpSocket, SocketAddr) {
        let u = UdpSocket::bind("127.0.0.1:0").unwrap();
        u.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
        let a = u.local_addr().unwrap();
        (u, a)
    }

    fn membro(k: &[u8; 32], ip: &str, farol: Option<SocketAddr>) -> rol::Membro {
        rol::Membro {
            chave: x25519::chave_publica(k),
            ip: ip.parse().unwrap(),
            nome: None,
            farol,
        }
    }

    /// Um no de rede assinada `R`, com o rol dado e os pares que ja conhece.
    fn no(
        dir: &std::path::Path,
        k: [u8; 32],
        ip: &str,
        u: UdpSocket,
        conhece: Vec<ParConfig>,
        r: &rol::Rol,
        dono: [u8; 32],
    ) -> No {
        let mut rede = Rede::nova("R", ip.parse().unwrap(), 24, 0);
        rede.dono = Some(dono);
        rede.rol = Some(r.clone());
        let c = dir.join(format!("{ip}.p2p")).to_str().unwrap().to_string();
        rede.gravar(&c).unwrap();
        No::novo(
            k,
            psk_da_rede("R", "s", 1_000),
            ip.parse().unwrap(),
            u,
            conhece,
        )
        .com_rede(rede, c)
    }

    fn bombear(no: &No) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; 4096];
        let (n, de) = no.udp.recv_from(&mut buf).ok()?;
        no.da_rede(&buf[..n], de)
    }

    /// Gira todos (um tique a cada 20 voltas) ate `alvo` entregar um pacote
    /// na placa. Devolve o pacote.
    fn ate_entregar(nos: &[&No], alvo: &No) -> Option<Vec<u8>> {
        for volta in 0..120 {
            if volta % 20 == 0 {
                for n in nos {
                    n.tique();
                }
            }
            for n in nos {
                if let Some(p) = bombear(n) {
                    if std::ptr::eq(*n, alvo) {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    struct Rede3 {
        a: No,
        b: No,
        c: No,
        dir: std::path::PathBuf,
    }

    /// A (dono, farol se `marcar`, serve se `servir`), B e C. B e C so
    /// conhecem A; um ao outro, so pelo rol -- sem endereco. B e C no modo
    /// `repasse`, que so tem o farol como intermediario.
    fn tres(nome: &str, marcar: bool, servir: bool) -> Rede3 {
        let dir = pasta(nome);
        let (ka, kb, kc) = (
            x25519::gerar_privada(),
            x25519::gerar_privada(),
            x25519::gerar_privada(),
        );
        let ((ua, ea), (ub, _), (uc, _)) = (soquete(), soquete(), soquete());
        let farol = marcar.then_some(ea);
        let dono = rol::publica_do_dono(&ka, "R");
        let r = rol::Rol::primeiro("R", membro(&ka, "10.78.0.1", farol), &ka)
            .unwrap()
            .com(membro(&kb, "10.78.0.2", None), &ka)
            .unwrap()
            .com(membro(&kc, "10.78.0.3", None), &ka)
            .unwrap();
        let pa = x25519::chave_publica(&ka);
        let conhece_a = || {
            vec![ParConfig {
                publica: pa,
                ip: "10.78.0.1".parse().unwrap(),
                endereco: Some(ea),
            }]
        };
        let a = no(&dir, ka, "10.78.0.1", ua, Vec::new(), &r, dono).com_farol(servir, None);
        let b =
            no(&dir, kb, "10.78.0.2", ub, conhece_a(), &r, dono).com_modo_so_farol(Modo::Repasse);
        let c =
            no(&dir, kc, "10.78.0.3", uc, conhece_a(), &r, dono).com_modo_so_farol(Modo::Repasse);
        Rede3 { a, b, c, dir }
    }

    /// B fala com C so pelo farol A, sem repasse nenhum.
    #[test]
    fn farol_repassa_entre_membros_sem_repasse() {
        let t = tres("repassa", true, true);
        for n in [&t.a, &t.b, &t.c] {
            n.tique();
        }
        t.b.da_placa(&pacote_ip([10, 78, 0, 2], [10, 78, 0, 3], b"segredo-de-b"));
        let chegou = ate_entregar(&[&t.a, &t.b, &t.c], &t.c).expect("C nao recebeu");
        assert!(chegou.ends_with(b"segredo-de-b"));
        let contas = t.a.contas_do_farol().expect("A nao serve");
        assert!(contas.repassados >= 3, "{contas:?}");
        assert_eq!(contas.registrados, 2, "{contas:?}");
        assert!(t.b.situacao().iter().any(|l| l[1].starts_with("farol")));
        let _ = std::fs::remove_dir_all(&t.dir);
    }

    /// O outro sentido: sem a marca do dono (ou sem o consentimento de A),
    /// A nao serve e B nao chega a C.
    #[test]
    fn sem_marca_ou_sem_consentimento_nao_serve() {
        for (marcar, servir) in [(false, true), (true, false)] {
            let t = tres("sem-marca", marcar, servir);
            for n in [&t.a, &t.b, &t.c] {
                n.tique();
            }
            t.b.da_placa(&pacote_ip([10, 78, 0, 2], [10, 78, 0, 3], b"x"));
            assert!(
                ate_entregar(&[&t.a, &t.b, &t.c], &t.c).is_none(),
                "marcar={marcar} servir={servir}: passou"
            );
            assert!(t.a.contas_do_farol().is_none());
            let _ = std::fs::remove_dir_all(&t.dir);
        }
    }

    /// Quem nao esta no rol nao usa o farol, mesmo sabendo a senha da rede e
    /// apontando para ele como se fosse um repasse: nao registra, nao
    /// repassa -- e o farol conta a recusa.
    #[test]
    fn fora_do_rol_nao_usa_o_farol() {
        let t = tres("fora", true, true);
        for n in [&t.a, &t.b, &t.c] {
            n.tique();
        }
        let kx = x25519::gerar_privada();
        let (ux, _) = soquete();
        let ea = t.a.udp.local_addr().unwrap();
        let x = No::novo(
            kx,
            psk_da_rede("R", "s", 1_000),
            "10.78.0.9".parse().unwrap(),
            ux,
            vec![ParConfig {
                publica: t.c.publica(),
                ip: "10.78.0.3".parse().unwrap(),
                endereco: None,
            }],
        )
        .com_repasse(
            Modo::Repasse,
            Some(RepasseCfg {
                endereco: ea,
                publica: t.a.publica(),
                conta: None,
            }),
        )
        .unwrap();
        x.tique();
        x.da_placa(&pacote_ip([10, 78, 0, 9], [10, 78, 0, 3], b"x"));
        assert!(ate_entregar(&[&t.a, &t.b, &t.c, &x], &t.c).is_none());
        // B e C se registram (e se falam pelo farol, por isso `repassados`
        // nao e zero); X nao: nem o REGISTRO nem o PARA dele passam.
        let contas = t.a.contas_do_farol().unwrap();
        assert_eq!(contas.registrados, 2, "{contas:?}");
        assert!(contas.recusados >= 2, "{contas:?}");
        assert!(
            !x.fio().unwrap().confirmado(),
            "o farol confirmou um estranho"
        );
        let _ = std::fs::remove_dir_all(&t.dir);
    }

    /// Dois farois: o aperto sai pelos dois (e assim que o par cai para o
    /// outro quando um some), o dado so pelo farol por onde o par foi ouvido.
    #[test]
    fn dois_farois_aperto_pelos_dois_dado_por_um() {
        let dir = pasta("dois");
        let ks: Vec<[u8; 32]> = (0..4).map(|_| x25519::gerar_privada()).collect();
        let us: Vec<(UdpSocket, SocketAddr)> = (0..4).map(|_| soquete()).collect();
        let (e1, e2) = (us[0].1, us[1].1);
        let dono = rol::publica_do_dono(&ks[0], "R");
        let r = rol::Rol::primeiro("R", membro(&ks[0], "10.78.0.1", Some(e1)), &ks[0])
            .unwrap()
            .com(membro(&ks[1], "10.78.0.4", Some(e2)), &ks[0])
            .unwrap()
            .com(membro(&ks[2], "10.78.0.2", None), &ks[0])
            .unwrap()
            .com(membro(&ks[3], "10.78.0.3", None), &ks[0])
            .unwrap();
        let mut us = us.into_iter();
        let mut prox = || us.next().unwrap().0;
        let f1 = no(&dir, ks[0], "10.78.0.1", prox(), Vec::new(), &r, dono).com_farol(true, None);
        let f2 = no(&dir, ks[1], "10.78.0.4", prox(), Vec::new(), &r, dono).com_farol(true, None);
        let b = no(&dir, ks[2], "10.78.0.2", prox(), Vec::new(), &r, dono)
            .com_modo_so_farol(Modo::Repasse);
        let c = no(&dir, ks[3], "10.78.0.3", prox(), Vec::new(), &r, dono)
            .com_modo_so_farol(Modo::Repasse);
        let todos = [&f1, &f2, &b, &c];
        // Dois tiques: os REGISTROs saem e as CONFIRMAs voltam.
        for _ in 0..2 {
            for n in todos {
                n.tique();
            }
            for _ in 0..10 {
                for n in todos {
                    bombear(n);
                }
            }
        }
        b.da_placa(&pacote_ip([10, 78, 0, 2], [10, 78, 0, 3], b"um"));
        assert!(ate_entregar(&todos, &c).is_some());
        let (c1, c2) = (
            f1.contas_do_farol().unwrap().repassados,
            f2.contas_do_farol().unwrap().repassados,
        );
        assert!(
            c1 >= 1 && c2 >= 1,
            "o aperto nao saiu pelos dois: {c1} {c2}"
        );
        // Agora so dados: um farol so carrega.
        for i in 0..10u8 {
            b.da_placa(&pacote_ip([10, 78, 0, 2], [10, 78, 0, 3], &[i]));
            assert!(ate_entregar(&todos, &c).is_some());
        }
        let (d1, d2) = (
            f1.contas_do_farol().unwrap().repassados - c1,
            f2.contas_do_farol().unwrap().repassados - c2,
        );
        assert!(
            (d1 >= 10 && d2 == 0) || (d2 >= 10 && d1 == 0),
            "dado dividido entre os farois: {d1} {d2}"
        );
        // O farol que carregava some (ninguem mais o bombeia). O aperto
        // novo de B -- o que o `SURDO_APOS` dispararia 15 s depois -- tem de
        // achar C pelo outro, e o dado seguinte tambem.
        let vivo = if d1 > 0 { &f2 } else { &f1 };
        let restantes = [vivo, &b, &c];
        {
            let mut e = b.estado.lock().unwrap();
            let i = e
                .pares
                .iter()
                .position(|p| p.publica == c.publica())
                .unwrap();
            // O aperto de fundo que o tique abriu no comeco ainda esta
            // pendente (< 5 s neste teste); 15 s depois ja teria vencido.
            if let Some((velho, _, _)) = e.pares[i].pendente.take() {
                e.indices.remove(&velho);
            }
            b.iniciar_aperto(&mut e, i);
        }
        for _ in 0..20 {
            for n in restantes {
                bombear(n);
            }
        }
        b.da_placa(&pacote_ip([10, 78, 0, 2], [10, 78, 0, 3], b"depois"));
        let p = ate_entregar(&restantes, &c).expect("sem o farol que caiu, C nao recebeu");
        assert!(p.ends_with(b"depois"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Os tetos: 1 Mbit/s nao deixa passar 1 MB de uma vez; e REGISTRO acima
    /// de 5/s do mesmo IP nem chega ao Diffie-Hellman.
    #[test]
    fn tetos_de_banda_e_de_registro() {
        let (ka, kb, kc) = (
            x25519::gerar_privada(),
            x25519::gerar_privada(),
            x25519::gerar_privada(),
        );
        let (pb, pc) = (x25519::chave_publica(&kb), x25519::chave_publica(&kc));
        let pa = x25519::chave_publica(&ka);
        let mut s = Servidor::novo(ka, [pb, pc].into_iter().collect(), 1);
        let eb: SocketAddr = "198.51.100.2:1".parse().unwrap();
        let ec: SocketAddr = "198.51.100.3:1".parse().unwrap();
        let reg = |k: &[u8; 32], n: u8| {
            let mut c = [0u8; 12];
            c[11] = n;
            repasse::registro(k, &pa, c, None).unwrap()
        };
        assert!(s.tratar(&reg(&kb, 1), eb).is_some());
        assert!(s.tratar(&reg(&kc, 1), ec).is_some());
        let carga = vec![0u8; 1000];
        let mut passou = 0usize;
        for _ in 0..1000 {
            if s.tratar(&repasse::embrulhar_para(&pc, &carga), eb)
                .is_some()
            {
                passou += 1;
            }
        }
        // 1 Mbit/s = 125.000 B/s; a rajada e de 64 KiB por origem.
        assert!(passou < 200, "passaram {passou} de 1000 pacotes de 1 KB");
        assert!(s.contas.cortados >= 800, "{:?}", s.contas);
        // Registro: do IP de B, 20 seguidos (carimbos novos) -- no maximo 5
        // chegam a conferir (e o primeiro ja gastou um).
        let antes = s.contas.registrados + s.contas.recusados;
        for n in 2..22u8 {
            s.tratar(&reg(&kb, n), eb);
        }
        let conferidos = s.contas.registrados + s.contas.recusados - antes;
        assert!(conferidos <= 5, "{conferidos} REGISTROs conferidos");
    }
}
