//! O fio ate o servidor intermediario: UDP, TCP ou TCP por um proxy HTTP.
//!
//! # Por que existe
//!
//! Rede de hotel, de escritorio e de celular corporativo muitas vezes so
//! deixa sair TCP/443, as vezes so por um proxy. O Radmin tem o relay por
//! TCP; o OpenVPN tem `proto tcp` e `--http-proxy`. Aqui o P2P e o repasse
//! eram so UDP. Este modulo e a UNICA decisao sobre «por qual fio vai o
//! pacote ao repasse» -- o `p2p.rs` chama `enviar` e nao sabe se e UDP ou
//! TCP. Espalhar `if tcp` pelo no seria repetir a decisao em cada envio, e o
//! envio que alguem esquecesse mandaria por UDP numa rede que o engole.
//!
//! # Enquadramento (o do OpenVPN)
//!
//! TCP e fluxo, nao datagrama: cada pacote vai com um prefixo de 2 bytes
//! (tamanho, big-endian), como o OpenVPN faz em `proto tcp`. Um quadro vazio
//! ou maior que o teto derruba a conexao -- nao ha como ressincronizar.
//!
//! # Queda para TCP (`Escolha::Auto`)
//!
//! O repasse CONFIRMA cada REGISTRO valido (`repasse.rs`). Se um REGISTRO
//! fica `reserva` segundos sem confirmacao, o no passa a falar com o repasse
//! por TCP. Se o TCP tambem nao traz confirmacao, volta ao UDP e tenta de
//! novo mais tarde -- porta 443 aberta que nao e o nosso repasse (um servidor
//! web, por exemplo) nao pode prender o no num fio mudo.
//!
//! # Volta ao UDP (`Escolha::Auto`)
//!
//! A rede que engoliu o UDP as vezes o devolve (o hotel troca de rede, o
//! celular sai do Wi-Fi da empresa). No TCP, o no manda uma SONDA por UDP
//! (`repasse.rs`) -- a primeira `sonda` segundos depois de cair, e o recuo
//! dobra a cada sonda sem eco, ate `RECUO_MAXIMO`. So volta depois de
//! `ECOS_PARA_VOLTAR` ecos autenticos SEGUIDOS (um datagrama que passa no
//! meio de uma perda nao e «o UDP voltou»); e, se cair de novo dentro de
//! `JANELA_INSTAVEL` depois de voltar, a proxima espera DOBRA em vez de
//! recomecar -- rede que oscila nao faz o no oscilar junto. Na volta o
//! REGISTRO sai na hora pelo UDP, o `geracao` sobe (o no refaz o aperto
//! pendente) e a conexao TCP ainda e lida por `CARENCIA_TCP`, para o que o
//! repasse ja tinha posto nela antes de ver o REGISTRO nao se perder.
//!
//! # Proxy HTTP
//!
//! `CONNECT ip:porta` (RFC 9110, secao 9.3.6). Com usuario, `Basic`. A
//! credencial so existe no cabecalho mandado ao proxy: nenhuma mensagem de
//! erro nem de registro a carrega, e o `Debug` do `Proxy` a esconde.
//! `HTTPS_PROXY` do ambiente NAO e lido -- ver `docs/PHXVPN.md`.

use crate::repasse;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Maior quadro possivel (o tamanho cabe em 2 bytes).
pub const TETO_QUADRO: usize = u16::MAX as usize;
/// Sem confirmacao do repasse por tanto tempo, `Auto` tenta o TCP.
pub const RESERVA_PADRAO: Duration = Duration::from_secs(10);
/// Enquanto o REGISTRO nao e confirmado, ele se repete neste ritmo: um
/// datagrama perdido nao pode ser lido como «UDP bloqueado».
pub const REPETIR_SEM_CONFIRMA: Duration = Duration::from_secs(2);
const PRAZO_CONEXAO: Duration = Duration::from_secs(5);
const PRAZO_ESCRITA: Duration = Duration::from_secs(2);
/// Cabecalho de resposta do proxy: mais que isso e proxy torto (ou hostil).
const TETO_RESPOSTA_PROXY: usize = 8192;
const ESPERA_MAXIMA: u64 = 60;
/// Primeira sonda do UDP, depois de cair para o TCP.
pub const RECUO_INICIAL: Duration = Duration::from_secs(30);
/// O recuo dobra ate aqui: sondar mais raro que isso adia a volta demais.
pub const RECUO_MAXIMO: Duration = Duration::from_secs(300);
/// Ecos seguidos para voltar ao UDP.
pub const ECOS_PARA_VOLTAR: u32 = 3;
/// Caiu de novo antes disso, depois de voltar: a rede oscila.
pub const JANELA_INSTAVEL: Duration = Duration::from_secs(300);
/// Depois do primeiro eco, as sondas seguintes vao neste ritmo.
const ENTRE_SONDAS: Duration = Duration::from_secs(1);
/// Sonda sem eco nesse prazo conta como perdida.
const PRAZO_ECO: Duration = Duration::from_secs(2);
/// Na volta ao UDP, a conexao TCP ainda e lida por tanto tempo.
const CARENCIA_TCP: Duration = Duration::from_secs(2);

// ---------------------------------------------------------- quadros ----

/// Pacote -> quadro `[tamanho:u16 BE][pacote]`.
pub fn quadro(pacote: &[u8]) -> Option<Vec<u8>> {
    if pacote.is_empty() || pacote.len() > TETO_QUADRO {
        return None;
    }
    let mut q = Vec::with_capacity(2 + pacote.len());
    q.extend_from_slice(&(pacote.len() as u16).to_be_bytes());
    q.extend_from_slice(pacote);
    Some(q)
}

/// Remonta quadros de um fluxo TCP que chega em pedacos -- e que pode parar
/// no meio de um quadro quando o prazo de leitura vence, sem perder nada.
///
/// O `teto` e o maior quadro aceito; a memoria fica em `2 * (2 + teto)` no
/// maximo, porque so se le mais quando o que esta guardado nao fecha um
/// quadro. Antes de o par provar quem e, o teto e o de um REGISTRO: uma
/// conexao anonima custa algumas centenas de bytes, nao 64 KiB.
pub struct Enquadrador {
    guardado: Vec<u8>,
    teto: usize,
}

impl Enquadrador {
    pub fn novo(teto: usize) -> Enquadrador {
        Enquadrador {
            guardado: Vec::new(),
            teto: teto.min(TETO_QUADRO),
        }
    }

    pub fn trocar_teto(&mut self, teto: usize) {
        self.teto = teto.min(TETO_QUADRO);
    }

    pub fn guardado(&self) -> usize {
        self.guardado.len()
    }

    /// Tira um quadro inteiro do que ja chegou.
    pub fn proximo(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.guardado.len() < 2 {
            return Ok(None);
        }
        let n = u16::from_be_bytes([self.guardado[0], self.guardado[1]]) as usize;
        if n == 0 || n > self.teto {
            return Err(format!("quadro de {n} bytes (teto {})", self.teto));
        }
        if self.guardado.len() < 2 + n {
            return Ok(None);
        }
        let q = self.guardado[2..2 + n].to_vec();
        self.guardado.drain(..2 + n);
        Ok(Some(q))
    }

    /// Le ate fechar um quadro. `Ok(None)`: o prazo de leitura venceu (o que
    /// ja chegou fica guardado). `Err`: conexao caiu ou quadro torto.
    pub fn ler(&mut self, r: &mut impl Read) -> Result<Option<Vec<u8>>, String> {
        loop {
            if let Some(q) = self.proximo()? {
                return Ok(Some(q));
            }
            let mut pedaco = vec![0u8; (2 + self.teto).min(16 * 1024)];
            match r.read(&mut pedaco) {
                Ok(0) => return Err("conexao fechada".into()),
                Ok(n) => self.guardado.extend_from_slice(&pedaco[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(None)
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.to_string()),
            }
        }
    }
}

// ------------------------------------------------------------ proxy ----

/// Proxy HTTP com `CONNECT`. A senha nunca sai daqui a nao ser no cabecalho
/// `Proxy-Authorization` mandado ao proprio proxy.
#[derive(Clone)]
pub struct Proxy {
    /// `host:porta` do proxy.
    pub endereco: String,
    pub usuario: Option<String>,
    senha: Option<String>,
}

impl std::fmt::Debug for Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proxy")
            .field("endereco", &self.endereco)
            .field("usuario", &self.usuario)
            .field("senha", &self.senha.as_ref().map(|_| "***"))
            .finish()
    }
}

impl Proxy {
    /// `host:porta`, com credencial opcional (usuario e senha juntos).
    pub fn novo(endereco: &str, credencial: Option<(String, String)>) -> Result<Proxy, String> {
        let (h, p) = endereco
            .rsplit_once(':')
            .ok_or("proxy no formato host:porta")?;
        if h.is_empty() || p.parse::<u16>().ok().filter(|p| *p > 0).is_none() {
            return Err("proxy no formato host:porta".into());
        }
        // Espaco, CR ou LF no endereco virariam cabecalho injetado.
        if endereco.bytes().any(|b| b <= b' ' || b == 0x7f) {
            return Err("proxy: endereco com caractere invalido".into());
        }
        let (usuario, senha) = match credencial {
            Some((u, s)) => {
                if u.is_empty() || u.contains(':') || u.bytes().any(|b| b < b' ') {
                    return Err("proxy: usuario invalido (sem dois-pontos nem controle)".into());
                }
                (Some(u), Some(s))
            }
            None => (None, None),
        };
        Ok(Proxy {
            endereco: endereco.to_string(),
            usuario,
            senha,
        })
    }

    /// O pedido `CONNECT` inteiro (com a credencial, se houver).
    fn pedido(&self, alvo: SocketAddr) -> String {
        let mut p = format!("CONNECT {alvo} HTTP/1.1\r\nHost: {alvo}\r\n");
        if let (Some(u), Some(s)) = (&self.usuario, &self.senha) {
            let b = phxsql_core::base64::codificar(format!("{u}:{s}").as_bytes());
            p.push_str(&format!("Proxy-Authorization: Basic {b}\r\n"));
        }
        p.push_str("\r\n");
        p
    }

    /// Abre o tunel ate `alvo` pelo proxy. Os erros dizem o codigo HTTP e
    /// nunca o cabecalho mandado.
    pub fn conectar(&self, alvo: SocketAddr, prazo: Duration) -> Result<TcpStream, String> {
        let end = self
            .endereco
            .to_socket_addrs()
            .map_err(|e| format!("proxy {}: {e}", self.endereco))?
            .next()
            .ok_or_else(|| format!("proxy {} nao resolveu", self.endereco))?;
        let mut s = TcpStream::connect_timeout(&end, prazo)
            .map_err(|e| format!("proxy {}: {e}", self.endereco))?;
        s.set_read_timeout(Some(prazo)).map_err(|e| e.to_string())?;
        s.set_write_timeout(Some(prazo))
            .map_err(|e| e.to_string())?;
        s.write_all(self.pedido(alvo).as_bytes())
            .map_err(|e| format!("proxy {}: {e}", self.endereco))?;
        // Byte a byte ate o fim do cabecalho: o que vier depois ja e do
        // repasse e nao pode ficar preso num buffer daqui.
        let mut cab = Vec::new();
        let mut b = [0u8; 1];
        while !cab.ends_with(b"\r\n\r\n") {
            if cab.len() >= TETO_RESPOSTA_PROXY {
                return Err(format!("proxy {}: resposta sem fim", self.endereco));
            }
            match s.read(&mut b) {
                Ok(1) => cab.push(b[0]),
                Ok(_) => return Err(format!("proxy {}: fechou a conexao", self.endereco)),
                Err(e) => return Err(format!("proxy {}: {e}", self.endereco)),
            }
        }
        let linha = cab.split(|c| *c == b'\n').next().unwrap_or_default();
        let codigo = std::str::from_utf8(linha)
            .ok()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|c| c.parse::<u16>().ok());
        match codigo {
            Some(200..=299) => Ok(s),
            Some(407) => Err(format!(
                "proxy {}: pediu credencial (407) -- use --proxy-usuario e PHXVPN_SENHA_PROXY",
                self.endereco
            )),
            Some(c) => Err(format!("proxy {}: recusou o CONNECT ({c})", self.endereco)),
            None => Err(format!("proxy {}: resposta que nao e HTTP", self.endereco)),
        }
    }
}

// --------------------------------------------------- fio do repasse ----

/// Por qual fio o no fala com o repasse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escolha {
    /// So UDP (o de antes).
    Udp,
    /// So TCP (`--tcp`, ou porque ha proxy).
    Tcp,
    /// UDP; sem confirmacao em `reserva`, TCP.
    Auto,
}

impl Escolha {
    pub fn de_texto(t: &str) -> Result<Escolha, String> {
        match t {
            "udp" => Ok(Escolha::Udp),
            "tcp" => Ok(Escolha::Tcp),
            "auto" => Ok(Escolha::Auto),
            _ => Err(format!(
                "fio do repasse desconhecido: {t} (udp, tcp ou auto)"
            )),
        }
    }

    pub fn texto(self) -> &'static str {
        match self {
            Escolha::Udp => "udp",
            Escolha::Tcp => "tcp",
            Escolha::Auto => "auto",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CfgFio {
    pub escolha: Escolha,
    /// Porta TCP do repasse (o host e o mesmo do UDP).
    pub porta_tcp: u16,
    pub proxy: Option<Proxy>,
    pub reserva: Duration,
    /// Primeira espera, no TCP do `Auto`, antes de sondar o UDP.
    pub sonda: Duration,
}

impl Default for CfgFio {
    /// O que o no fazia antes deste modulo: so UDP.
    fn default() -> Self {
        CfgFio {
            escolha: Escolha::Udp,
            porta_tcp: 443,
            proxy: None,
            reserva: RESERVA_PADRAO,
            sonda: RECUO_INICIAL,
        }
    }
}

/// A decisao de voltar do TCP ao UDP, sem relogio proprio (o `agora` vem de
/// fora) para o teste andar no tempo sem dormir.
#[derive(Debug)]
struct Volta {
    inicial: Duration,
    recuo: Duration,
    proxima: Instant,
    /// Carimbo da sonda no ar e quando saiu.
    pendente: Option<([u8; 12], Instant)>,
    ecos: u32,
    voltou_em: Option<Instant>,
}

impl Volta {
    fn nova(inicial: Duration, agora: Instant) -> Volta {
        Volta {
            inicial,
            recuo: inicial,
            proxima: agora + inicial,
            pendente: None,
            ecos: 0,
            voltou_em: None,
        }
    }

    fn dobrar(&mut self) {
        self.recuo = (self.recuo * 2).min(RECUO_MAXIMO.max(self.inicial));
    }

    /// O `Auto` caiu para o TCP. Logo depois de ter voltado, o recuo dobra;
    /// depois de um UDP estavel, recomeca.
    fn caiu(&mut self, agora: Instant) {
        let instavel = self
            .voltou_em
            .is_some_and(|t| agora.saturating_duration_since(t) < JANELA_INSTAVEL);
        if instavel {
            self.dobrar();
        } else {
            self.recuo = self.inicial;
        }
        self.proxima = agora + self.recuo;
        self.pendente = None;
        self.ecos = 0;
    }

    /// Hora de mandar uma sonda? A que ficou sem eco no prazo zera a
    /// contagem e dobra o recuo.
    fn devida(&mut self, agora: Instant) -> bool {
        if let Some((_, t)) = self.pendente {
            if agora.saturating_duration_since(t) < PRAZO_ECO {
                return false;
            }
            self.pendente = None;
            self.ecos = 0;
            self.dobrar();
            self.proxima = agora + self.recuo;
        }
        agora >= self.proxima
    }

    fn saiu(&mut self, carimbo: [u8; 12], agora: Instant) {
        self.pendente = Some((carimbo, agora));
    }

    /// Eco autentico da sonda `carimbo`. `true`: foram `ECOS_PARA_VOLTAR`
    /// seguidos -- voltar agora. Eco de outra sonda (velha, repetida) nao
    /// conta.
    fn eco(&mut self, carimbo: [u8; 12], agora: Instant) -> bool {
        if self.pendente.map(|(c, _)| c) != Some(carimbo) {
            return false;
        }
        self.pendente = None;
        self.ecos += 1;
        if self.ecos >= ECOS_PARA_VOLTAR {
            self.ecos = 0;
            self.voltou_em = Some(agora);
            return true;
        }
        self.proxima = agora + ENTRE_SONDAS;
        false
    }
}

struct Estado {
    /// O fio em uso agora.
    tcp: bool,
    /// Desde quando ha REGISTRO sem confirmacao (None = confirmado).
    esperando: Option<Instant>,
    confirmado: Option<Instant>,
    maior_carimbo: [u8; 12],
    /// Fio novo: o proximo tique registra na hora, sem esperar os 20 s.
    pedir_registro: bool,
    /// Antes disso nao se tenta (re)conectar o TCP.
    tcp_depois_de: Instant,
    falhas_tcp: u32,
    volta: Volta,
    /// O UDP respondeu as sondas: o proximo `vigiar` volta.
    voltar: bool,
    /// Ja no UDP, a conexao TCP velha e lida ate aqui e entao fechada.
    largar_tcp_em: Option<Instant>,
}

/// O fio do no ate o repasse. Uma instancia por no.
pub struct FioRepasse {
    cfg: CfgFio,
    alvo_udp: SocketAddr,
    segredo: [u8; 32],
    chave_no: [u8; 32],
    estado: Mutex<Estado>,
    /// Metade de escrita da conexao TCP (quem cifra manda por aqui), e se o
    /// REGISTRO ja saiu por ela.
    escrita: Mutex<Option<(TcpStream, bool)>>,
    /// Metade de leitura: so a thread do `receber` a usa.
    leitura: Mutex<Option<(TcpStream, Enquadrador)>>,
    /// Sobe a cada fio novo que passa a valer (TCP conectado, volta ao UDP).
    /// O no a olha no tique para refazer na hora o aperto que ficou pendente
    /// no fio velho -- sem isso, esperava o reenvio de 5 s (medido: primeiro
    /// ping 15,0 s no `auto` e 5,3 s pelo proxy, antes deste contador).
    geracao: std::sync::atomic::AtomicU64,
}

impl FioRepasse {
    pub fn novo(
        privada_no: &[u8; 32],
        publica_repasse: &[u8; 32],
        alvo_udp: SocketAddr,
        cfg: CfgFio,
    ) -> Result<FioRepasse, String> {
        let segredo =
            phxsql_core::x25519::segredo(privada_no, publica_repasse).map_err(|e| e.to_string())?;
        let agora = Instant::now();
        Ok(FioRepasse {
            estado: Mutex::new(Estado {
                tcp: cfg.escolha == Escolha::Tcp,
                esperando: None,
                confirmado: None,
                maior_carimbo: [0; 12],
                pedir_registro: false,
                tcp_depois_de: agora,
                falhas_tcp: 0,
                volta: Volta::nova(cfg.sonda, agora),
                voltar: false,
                largar_tcp_em: None,
            }),
            cfg,
            alvo_udp,
            segredo,
            chave_no: phxsql_core::x25519::chave_publica(privada_no),
            escrita: Mutex::new(None),
            leitura: Mutex::new(None),
            geracao: std::sync::atomic::AtomicU64::new(0),
        })
    }

    pub fn geracao(&self) -> u64 {
        self.geracao.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn fio_novo(&self) {
        self.geracao
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn escolha(&self) -> Escolha {
        self.cfg.escolha
    }

    /// Onde o repasse escuta UDP: a familia dele decide o soquete de saida.
    pub fn alvo_udp(&self) -> SocketAddr {
        self.alvo_udp
    }

    pub fn alvo_tcp(&self) -> SocketAddr {
        SocketAddr::new(self.alvo_udp.ip(), self.cfg.porta_tcp)
    }

    /// O fio em uso agora e TCP?
    pub fn no_tcp(&self) -> bool {
        self.estado.lock().expect("fio").tcp
    }

    /// Confirmado pelo repasse nos ultimos 45 s (duas renovacoes e folga).
    pub fn confirmado(&self) -> bool {
        self.estado
            .lock()
            .expect("fio")
            .confirmado
            .is_some_and(|t| t.elapsed() < repasse::RENOVAR_REGISTRO * 2 + Duration::from_secs(5))
    }

    /// Para o console e a janela: `udp`, `tcp 1.2.3.4:443` ou `tcp ... via proxy`.
    pub fn descricao(&self) -> String {
        if !self.no_tcp() {
            return "udp".into();
        }
        match &self.cfg.proxy {
            Some(p) => format!("tcp {} via proxy {}", self.alvo_tcp(), p.endereco),
            None => format!("tcp {}", self.alvo_tcp()),
        }
    }

    /// Manda um pacote ao repasse pelo fio em uso. Com TCP ainda sem
    /// conexao, o pacote se perde -- exatamente o que o UDP faria.
    pub fn enviar(&self, udp: &UdpSocket, pacote: &[u8]) {
        if !self.no_tcp() {
            let _ = udp.send_to(pacote, self.alvo_udp);
            return;
        }
        let Some(q) = quadro(pacote) else {
            return;
        };
        let mut e = self.escrita.lock().expect("escrita");
        if let Some((s, registrada)) = e.as_mut() {
            // O repasse so aceita conexao cujo PRIMEIRO quadro e um REGISTRO
            // (`repasse_tcp.rs`) e derruba a que comeca com dado. Com trafego
            // no ar, o dado da placa chegava antes do REGISTRO do tique, e o
            // no reconectava em laco (medido na volta ao UDP, com ping a
            // cada 0,2 s). Ate o REGISTRO sair, o dado se perde -- como no
            // TCP ainda sem conexao.
            let e_registro = pacote[0] == repasse::TIPO_REGISTRO;
            if !*registrada && !e_registro {
                return;
            }
            *registrada |= e_registro;
            if s.write_all(&q).is_err() {
                // A leitura ve o fim e reconecta; aqui so se larga a conexao.
                let _ = s.shutdown(Shutdown::Both);
                *e = None;
            }
        }
    }

    /// Um pacote do repasse. `true` se era uma CONFIRMA (e ja foi tratada).
    pub fn chegou(&self, pacote: &[u8]) -> bool {
        if pacote.first() == Some(&repasse::TIPO_ECO) {
            if let Some(c) = repasse::conferir_eco(&self.segredo, &self.chave_no, pacote) {
                let mut e = self.estado.lock().expect("fio");
                if e.tcp && self.cfg.escolha == Escolha::Auto && !e.voltar {
                    let ecos = e.volta.ecos;
                    if e.volta.eco(c, Instant::now()) {
                        e.voltar = true;
                    } else if e.volta.ecos > ecos {
                        eprintln!(
                            "phxvpn: o UDP ao repasse respondeu ({}/{ECOS_PARA_VOLTAR})",
                            e.volta.ecos
                        );
                    }
                }
            }
            return true;
        }
        if pacote.first() != Some(&repasse::TIPO_CONFIRMA) {
            return false;
        }
        if let Some(c) = repasse::conferir_confirmacao(&self.segredo, &self.chave_no, pacote) {
            let mut e = self.estado.lock().expect("fio");
            // Confirmacao gravada e reenviada nao conta de novo.
            if c > e.maior_carimbo {
                e.maior_carimbo = c;
                e.confirmado = Some(Instant::now());
                e.esperando = None;
                e.falhas_tcp = 0;
            }
        }
        true
    }

    /// Esta na hora de mandar o REGISTRO? (`ultimo` = quando saiu o ultimo.)
    pub fn registro_devido(&self, ultimo: Option<Instant>) -> bool {
        let mut e = self.estado.lock().expect("fio");
        if std::mem::take(&mut e.pedir_registro) {
            return true;
        }
        let confirmado = e.esperando.is_none() && e.confirmado.is_some();
        let ritmo = if confirmado {
            repasse::RENOVAR_REGISTRO
        } else {
            REPETIR_SEM_CONFIRMA
        };
        ultimo.map_or(true, |t| t.elapsed() >= ritmo)
    }

    /// O REGISTRO saiu: comeca a contar o prazo da confirmacao.
    pub fn registro_enviado(&self) {
        self.estado
            .lock()
            .expect("fio")
            .esperando
            .get_or_insert_with(Instant::now);
    }

    /// Chamado a cada segundo: decide a troca de fio no `Auto` (nos dois
    /// sentidos), manda a sonda do UDP pelo `udp` e larga a conexao TCP que
    /// nao confirma.
    pub fn vigiar(&self, udp: &UdpSocket) {
        let agora = Instant::now();
        let mut e = self.estado.lock().expect("fio");
        if !e.tcp && e.largar_tcp_em.is_some_and(|t| agora >= t) {
            e.largar_tcp_em = None;
            drop(e);
            self.largar();
            e = self.estado.lock().expect("fio");
        }
        let mudo = |e: &Estado, prazo: Duration| e.esperando.is_some_and(|t| t.elapsed() >= prazo);
        match (self.cfg.escolha, e.tcp) {
            (Escolha::Udp, _) => {}
            (Escolha::Auto, false) => {
                if mudo(&e, self.cfg.reserva) && Instant::now() >= e.tcp_depois_de {
                    eprintln!(
                        "phxvpn: o repasse nao confirmou por UDP em {} s; tentando TCP {}",
                        self.cfg.reserva.as_secs(),
                        self.destino_tcp()
                    );
                    e.tcp = true;
                    e.esperando = Some(Instant::now());
                    e.pedir_registro = true;
                    e.largar_tcp_em = None;
                    e.volta.caiu(agora);
                    eprintln!(
                        "phxvpn: proxima sonda do UDP em {} s",
                        e.volta.recuo.as_secs()
                    );
                }
            }
            (Escolha::Auto, true) if std::mem::take(&mut e.voltar) => {
                eprintln!(
                    "phxvpn: o UDP ao repasse voltou ({ECOS_PARA_VOLTAR} ecos seguidos); de volta ao UDP"
                );
                e.tcp = false;
                e.esperando = None;
                e.pedir_registro = true;
                e.falhas_tcp = 0;
                e.tcp_depois_de = agora;
                e.largar_tcp_em = Some(agora + CARENCIA_TCP);
                drop(e);
                self.fio_novo();
            }
            (Escolha::Auto, true) => {
                if e.volta.devida(agora) {
                    let c = crate::transporte::carimbo_agora();
                    e.volta.saiu(c, agora);
                    let p = repasse::sonda(&self.segredo, &self.chave_no, &c);
                    let _ = udp.send_to(&p, self.alvo_udp);
                }
                // Conectar + confirmar tem o dobro do prazo; sem isso, volta.
                if mudo(&e, self.cfg.reserva * 2) {
                    eprintln!("phxvpn: o repasse nao confirmou por TCP; de volta ao UDP");
                    e.tcp = false;
                    e.falhas_tcp += 1;
                    e.tcp_depois_de = Instant::now() + espera(e.falhas_tcp);
                    e.esperando = Some(Instant::now());
                    e.pedir_registro = true;
                    drop(e);
                    self.largar();
                    self.fio_novo();
                }
            }
            (Escolha::Tcp, _) => {
                if mudo(&e, self.cfg.reserva * 2) && self.escrita.lock().expect("escrita").is_some()
                {
                    eprintln!("phxvpn: o repasse nao confirmou pela conexao TCP; reconectando");
                    e.esperando = Some(Instant::now());
                    drop(e);
                    self.largar();
                }
            }
        }
    }

    fn destino_tcp(&self) -> String {
        match &self.cfg.proxy {
            Some(p) => format!("{} via proxy {}", self.alvo_tcp(), p.endereco),
            None => self.alvo_tcp().to_string(),
        }
    }

    /// Fecha a conexao TCP (a leitura ve o fim e para de ler dela).
    fn largar(&self) {
        if let Some((s, _)) = self.escrita.lock().expect("escrita").take() {
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    fn conectar(&self) -> Result<TcpStream, String> {
        let alvo = self.alvo_tcp();
        let s = match &self.cfg.proxy {
            Some(p) => p.conectar(alvo, PRAZO_CONEXAO)?,
            None => TcpStream::connect_timeout(&alvo, PRAZO_CONEXAO)
                .map_err(|e| format!("repasse TCP {alvo}: {e}"))?,
        };
        let _ = s.set_nodelay(true);
        s.set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| e.to_string())?;
        s.set_write_timeout(Some(PRAZO_ESCRITA))
            .map_err(|e| e.to_string())?;
        Ok(s)
    }

    /// Um passo da thread de leitura do TCP: conecta quando o fio e TCP e
    /// nao ha conexao, e le no maximo um quadro (ate 0,5 s). Sem nada,
    /// `None`. Quem chama entrega o quadro ao `No::da_repasse`.
    pub fn receber(&self) -> Option<Vec<u8>> {
        let (tcp, carencia) = {
            let e = self.estado.lock().expect("fio");
            (e.tcp, e.largar_tcp_em.is_some())
        };
        let mut l = self.leitura.lock().expect("leitura");
        let viva = self.escrita.lock().expect("escrita").is_some();
        if !tcp {
            // De volta ao UDP: a conexao velha so e lida durante a carencia
            // (o que o repasse pos nela antes do REGISTRO por UDP); depois
            // sai daqui, em vez de ficar aberta ate a proxima queda.
            if !(carencia && viva && l.is_some()) {
                if let Some((s, _)) = l.take() {
                    let _ = s.shutdown(Shutdown::Both);
                }
                drop(l);
                std::thread::sleep(Duration::from_millis(200));
                return None;
            }
        } else if l.is_none() || !viva {
            if let Some((s, _)) = l.take() {
                let _ = s.shutdown(Shutdown::Both);
            }
            if Instant::now() < self.estado.lock().expect("fio").tcp_depois_de {
                drop(l);
                std::thread::sleep(Duration::from_millis(200));
                return None;
            }
            match self
                .conectar()
                .and_then(|s| s.try_clone().map(|c| (s, c)).map_err(|e| e.to_string()))
            {
                Ok((s, c)) => {
                    eprintln!("phxvpn: repasse por TCP -- {}", self.destino_tcp());
                    *self.escrita.lock().expect("escrita") = Some((c, false));
                    *l = Some((s, Enquadrador::novo(TETO_QUADRO)));
                    let mut e = self.estado.lock().expect("fio");
                    e.pedir_registro = true;
                    e.esperando = Some(Instant::now());
                    drop(e);
                    self.fio_novo();
                }
                Err(x) => {
                    eprintln!("phxvpn: {x}");
                    let mut e = self.estado.lock().expect("fio");
                    e.falhas_tcp += 1;
                    e.tcp_depois_de = Instant::now() + espera(e.falhas_tcp);
                    drop(e);
                    drop(l);
                    std::thread::sleep(Duration::from_millis(200));
                    return None;
                }
            }
        }
        let (s, enq) = l.as_mut().expect("acabou de conectar");
        match enq.ler(s) {
            Ok(q) => q,
            Err(x) => {
                // Fim da carencia da volta ao UDP: fomos nos que fechamos.
                if tcp {
                    eprintln!("phxvpn: repasse TCP caiu -- {x}");
                }
                let _ = s.shutdown(Shutdown::Both);
                *l = None;
                drop(l);
                self.largar();
                None
            }
        }
    }
}

/// Espera antes de reconectar: 1, 2, 4... ate 60 s.
fn espera(falhas: u32) -> Duration {
    Duration::from_secs((1u64 << falhas.min(6)).min(ESPERA_MAXIMA))
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn quadro_ida_e_volta_em_pedacos_e_com_prazo_vencido() {
        let mut fluxo = Vec::new();
        for p in [&b"um"[..], &[7u8; 1500][..], b"tres"] {
            fluxo.extend(quadro(p).unwrap());
        }
        // Leitor que entrega 3 bytes por vez e, entre eles, «prazo vencido».
        struct Pingado(Vec<u8>, usize, bool);
        impl Read for Pingado {
            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                self.2 = !self.2;
                if self.2 {
                    return Err(std::io::ErrorKind::WouldBlock.into());
                }
                let n = 3.min(b.len()).min(self.0.len() - self.1);
                b[..n].copy_from_slice(&self.0[self.1..self.1 + n]);
                self.1 += n;
                Ok(n)
            }
        }
        let mut r = Pingado(fluxo, 0, false);
        let mut e = Enquadrador::novo(TETO_QUADRO);
        let mut saiu = Vec::new();
        for _ in 0..10_000 {
            match e.ler(&mut r) {
                Ok(Some(q)) => saiu.push(q),
                Ok(None) => {}
                Err(_) => break,
            }
        }
        assert_eq!(saiu.len(), 3);
        assert_eq!(saiu[0], b"um");
        assert_eq!(saiu[1], vec![7u8; 1500]);
        assert_eq!(saiu[2], b"tres");
    }

    #[test]
    fn quadro_vazio_ou_acima_do_teto_derruba() {
        let mut e = Enquadrador::novo(145);
        let mut c = std::io::Cursor::new(vec![0u8, 0, 1]);
        assert!(e.ler(&mut c).is_err(), "quadro vazio aceito");
        let mut e = Enquadrador::novo(145);
        let mut c = std::io::Cursor::new(quadro(&[1u8; 146]).unwrap());
        assert!(e.ler(&mut c).is_err(), "quadro acima do teto aceito");
        // E o teto limita o que fica guardado: nada alem de um quadro.
        assert!(e.guardado() <= 2 * (2 + 145));
        assert!(quadro(&[]).is_none());
        assert!(quadro(&vec![0u8; TETO_QUADRO + 1]).is_none());
    }

    /// Proxy minimo: le o CONNECT, guarda o cabecalho, responde `resposta`
    /// e, se 200, ecoa o que vier.
    fn proxy_de_teste(resposta: &'static str) -> (SocketAddr, std::sync::mpsc::Receiver<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let end = l.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut cab = Vec::new();
            let mut b = [0u8; 1];
            while !cab.ends_with(b"\r\n\r\n") && s.read(&mut b).unwrap_or(0) == 1 {
                cab.push(b[0]);
            }
            tx.send(String::from_utf8_lossy(&cab).into_owned()).unwrap();
            s.write_all(resposta.as_bytes()).unwrap();
            let mut buf = [0u8; 64];
            while let Ok(n) = s.read(&mut buf) {
                if n == 0 || s.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        });
        (end, rx)
    }

    #[test]
    fn proxy_connect_com_credencial_e_erro_sem_segredo() {
        let alvo: SocketAddr = "198.51.100.7:443".parse().unwrap();
        let (end, rx) = proxy_de_teste("HTTP/1.1 200 Connection established\r\n\r\n");
        let p = Proxy::novo(
            &end.to_string(),
            Some(("ana".into(), "segredo-do-proxy".into())),
        )
        .unwrap();
        let mut s = p.conectar(alvo, Duration::from_secs(2)).unwrap();
        let cab = rx.recv().unwrap();
        assert!(cab.starts_with("CONNECT 198.51.100.7:443 HTTP/1.1\r\n"));
        // base64("ana:segredo-do-proxy")
        assert!(cab.contains("Proxy-Authorization: Basic YW5hOnNlZ3JlZG8tZG8tcHJveHk=\r\n"));
        s.write_all(b"eco").unwrap();
        let mut b = [0u8; 3];
        s.read_exact(&mut b).unwrap();
        assert_eq!(&b, b"eco");
        // O Debug nao mostra a senha.
        assert!(!format!("{p:?}").contains("segredo-do-proxy"));

        // 407: o erro diz o que fazer e nao carrega a credencial.
        let (end, _rx) = proxy_de_teste("HTTP/1.1 407 Proxy Authentication Required\r\n\r\n");
        let p = Proxy::novo(
            &end.to_string(),
            Some(("ana".into(), "segredo-do-proxy".into())),
        )
        .unwrap();
        let erro = p.conectar(alvo, Duration::from_secs(2)).unwrap_err();
        assert!(erro.contains("407"), "{erro}");
        assert!(
            !erro.contains("segredo") && !erro.contains("YW5h"),
            "{erro}"
        );
    }

    #[test]
    fn proxy_recusa_endereco_que_injeta_cabecalho() {
        assert!(Proxy::novo("proxy:3128", None).is_ok());
        assert!(Proxy::novo("proxy:3128\r\nX: y", None).is_err());
        assert!(Proxy::novo("proxy", None).is_err());
        assert!(Proxy::novo("proxy:0", None).is_err());
        assert!(Proxy::novo("proxy:3128", Some(("a:b".into(), "s".into()))).is_err());
    }

    #[test]
    fn confirmacao_so_vale_do_repasse_e_nao_repete() {
        let (no, rep) = (
            phxsql_core::x25519::gerar_privada(),
            phxsql_core::x25519::gerar_privada(),
        );
        let pr = phxsql_core::x25519::chave_publica(&rep);
        let alvo: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let f = FioRepasse::novo(&no, &pr, alvo, CfgFio::default()).unwrap();
        let mut r = repasse::Repasse::novo(rep, None);
        let reg = repasse::registro(&no, &pr, crate::transporte::carimbo_agora(), None).unwrap();
        let (_, conf) = r.tratar(&reg, alvo).expect("o repasse confirma o registro");
        f.registro_enviado();
        assert!(!f.confirmado());
        let mut torta = conf.clone();
        torta[20] ^= 1;
        assert!(f.chegou(&torta));
        assert!(!f.confirmado(), "confirmacao adulterada valeu");
        assert!(f.chegou(&conf));
        assert!(f.confirmado());
        // Outro no nao confirma por este.
        let outro = phxsql_core::x25519::gerar_privada();
        let g = FioRepasse::novo(&outro, &pr, alvo, CfgFio::default()).unwrap();
        g.chegou(&conf);
        assert!(!g.confirmado(), "confirmacao de outro no valeu");
    }

    /// Auto: sem confirmacao na reserva, passa ao TCP; com confirmacao, fica.
    #[test]
    fn auto_cai_para_tcp_so_sem_confirmacao() {
        let (no, rep) = (
            phxsql_core::x25519::gerar_privada(),
            phxsql_core::x25519::gerar_privada(),
        );
        let pr = phxsql_core::x25519::chave_publica(&rep);
        let alvo: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let cfg = CfgFio {
            escolha: Escolha::Auto,
            reserva: Duration::from_millis(50),
            ..CfgFio::default()
        };
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let f = FioRepasse::novo(&no, &pr, alvo, cfg.clone()).unwrap();
        f.registro_enviado();
        std::thread::sleep(Duration::from_millis(80));
        f.vigiar(&udp);
        assert!(f.no_tcp(), "sem confirmacao nao caiu para TCP");

        let g = FioRepasse::novo(&no, &pr, alvo, cfg).unwrap();
        let mut r = repasse::Repasse::novo(rep, None);
        let reg = repasse::registro(&no, &pr, crate::transporte::carimbo_agora(), None).unwrap();
        g.registro_enviado();
        g.chegou(&r.tratar(&reg, alvo).unwrap().1);
        std::thread::sleep(Duration::from_millis(80));
        g.vigiar(&udp);
        assert!(!g.no_tcp(), "confirmado e mesmo assim trocou de fio");

        // So UDP (o padrao): nunca troca.
        let u = FioRepasse::novo(&no, &pr, alvo, CfgFio::default()).unwrap();
        u.registro_enviado();
        std::thread::sleep(Duration::from_millis(80));
        u.vigiar(&udp);
        assert!(!u.no_tcp());
    }

    fn c(n: u8) -> [u8; 12] {
        let mut c = [0u8; 12];
        c[11] = n;
        c
    }

    /// Anda a maquina: manda a sonda se devida e, com `responde`, o eco
    /// dela chega. Devolve se voltou.
    fn passo(v: &mut Volta, t: Instant, n: u8, responde: bool) -> Option<bool> {
        if !v.devida(t) {
            return None;
        }
        v.saiu(c(n), t);
        Some(responde && v.eco(c(n), t))
    }

    #[test]
    fn volta_so_depois_de_k_ecos_seguidos() {
        let t0 = Instant::now();
        let s = |x: u64| t0 + Duration::from_secs(x);
        let mut v = Volta::nova(RECUO_INICIAL, t0);
        v.caiu(t0);
        assert!(!v.devida(s(29)), "sondou antes do recuo");
        assert_eq!(passo(&mut v, s(30), 1, true), Some(false));
        assert!(!v.devida(s(30)), "sonda seguinte sem esperar o ritmo");
        assert_eq!(passo(&mut v, s(31), 2, true), Some(false));
        // Eco que nao e da sonda no ar (velho ou repetido) nao conta.
        v.saiu(c(3), s(32));
        assert!(!v.eco(c(2), s(32)));
        assert!(!v.eco(c(1), s(32)));
        assert!(v.eco(c(3), s(32)), "{ECOS_PARA_VOLTAR} ecos e nao voltou");
    }

    /// Eco no meio de perdas nao faz voltar: a sonda perdida zera a
    /// contagem e dobra o recuo, ate o teto.
    #[test]
    fn sonda_sem_eco_zera_a_contagem_e_dobra_o_recuo() {
        let t0 = Instant::now();
        let mut v = Volta::nova(RECUO_INICIAL, t0);
        v.caiu(t0);
        let mut t = t0 + RECUO_INICIAL;
        let mut n = 0u8;
        let mut voltou = false;
        // Rede que responde duas e perde uma, dez vezes.
        for _ in 0..10 {
            for responde in [true, true, false] {
                while !v.devida(t) {
                    t += Duration::from_millis(500);
                }
                n += 1;
                v.saiu(c(n), t);
                voltou |= responde && v.eco(c(n), t);
            }
        }
        assert!(!voltou, "voltou com ecos intercalados de perdas");
        assert_eq!(v.recuo, RECUO_MAXIMO, "o recuo nao chegou ao teto");
    }

    #[test]
    fn cair_logo_depois_de_voltar_dobra_o_recuo() {
        let t0 = Instant::now();
        let mut v = Volta::nova(RECUO_INICIAL, t0);
        v.caiu(t0);
        let mut t = t0 + RECUO_INICIAL;
        for n in 1..=ECOS_PARA_VOLTAR as u8 {
            while !v.devida(t) {
                t += Duration::from_millis(500);
            }
            v.saiu(c(n), t);
            v.eco(c(n), t);
        }
        // Caiu 40 s depois de voltar: a proxima espera e o dobro.
        v.caiu(t + Duration::from_secs(40));
        assert_eq!(v.recuo, RECUO_INICIAL * 2);
        assert!(!v.devida(t + Duration::from_secs(40) + RECUO_INICIAL));
        // Depois de um UDP estavel pela janela inteira, recomeca.
        let mut w = Volta::nova(RECUO_INICIAL, t0);
        w.recuo = RECUO_INICIAL * 4;
        w.voltou_em = Some(t0);
        w.caiu(t0 + JANELA_INSTAVEL);
        assert_eq!(w.recuo, RECUO_INICIAL);
    }

    /// De ponta a ponta com o repasse de verdade e soquetes: o `Auto` cai
    /// para TCP, sonda por UDP, e so volta depois de K ecos autenticos. Com
    /// o UDP mudo (sonda que ninguem responde), fica no TCP.
    #[test]
    fn auto_volta_ao_udp_quando_o_repasse_responde_as_sondas() {
        let (no, rep) = (
            phxsql_core::x25519::gerar_privada(),
            phxsql_core::x25519::gerar_privada(),
        );
        let pr = phxsql_core::x25519::chave_publica(&rep);
        let lado_rep = UdpSocket::bind("127.0.0.1:0").unwrap();
        lado_rep
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let cfg = CfgFio {
            escolha: Escolha::Auto,
            reserva: Duration::from_millis(30),
            sonda: Duration::from_millis(50),
            ..CfgFio::default()
        };
        let f = FioRepasse::novo(&no, &pr, lado_rep.local_addr().unwrap(), cfg).unwrap();
        let mut r = repasse::Repasse::novo(rep, None);
        // O no esta registrado (pelo TCP): e o que a sonda exige.
        let reg = repasse::registro(&no, &pr, crate::transporte::carimbo_agora(), None).unwrap();
        let (_, conf_tcp) = r
            .tratar_de(&reg, repasse::Ponta::Tcp("127.0.0.1:1".parse().unwrap()))
            .unwrap();
        f.registro_enviado();
        std::thread::sleep(Duration::from_millis(40));
        f.vigiar(&udp);
        assert!(f.no_tcp(), "nao caiu para TCP");
        // O TCP confirmou (senao o fio largaria o TCP por mudo).
        f.registro_enviado();
        assert!(f.chegou(&conf_tcp));
        let g0 = f.geracao();

        let mut buf = [0u8; 256];
        let mut sondas = 0;
        let fim = Instant::now() + Duration::from_secs(8);
        while f.no_tcp() && Instant::now() < fim {
            f.vigiar(&udp);
            if let Ok((n, de)) = lado_rep.recv_from(&mut buf) {
                assert_eq!(buf[0], repasse::TIPO_SONDA);
                sondas += 1;
                let (alvo, eco) = r.tratar(&buf[..n], de).expect("repasse sem eco");
                assert_eq!(alvo, udp.local_addr().unwrap());
                assert!(f.chegou(&eco));
                // Antes de K, o fio nao voltou.
                if sondas < ECOS_PARA_VOLTAR {
                    f.vigiar(&udp);
                    assert!(f.no_tcp(), "voltou com {sondas} eco(s)");
                }
            }
        }
        assert!(!f.no_tcp(), "{sondas} ecos e continuou no TCP");
        assert_eq!(sondas, ECOS_PARA_VOLTAR);
        assert!(f.geracao() > g0, "voltou sem avisar o no do fio novo");
        assert!(
            f.registro_devido(Some(Instant::now())),
            "voltou sem REGISTRO na hora"
        );

        // Sonda sem resposta: fica no TCP.
        let g = FioRepasse::novo(
            &no,
            &pr,
            lado_rep.local_addr().unwrap(),
            CfgFio {
                escolha: Escolha::Auto,
                reserva: Duration::from_millis(30),
                sonda: Duration::from_millis(50),
                ..CfgFio::default()
            },
        )
        .unwrap();
        g.registro_enviado();
        std::thread::sleep(Duration::from_millis(40));
        g.vigiar(&udp);
        assert!(g.no_tcp());
        g.registro_enviado();
        assert!(g.chegou(&conf_tcp));
        let fim = Instant::now() + Duration::from_millis(600);
        while Instant::now() < fim {
            g.vigiar(&udp);
            let _ = lado_rep.recv_from(&mut buf);
        }
        assert!(g.no_tcp(), "voltou ao UDP sem eco nenhum");
    }

    /// Conexao TCP nova: o primeiro quadro e o REGISTRO, mesmo com dado da
    /// placa pedindo passagem antes dele (o repasse derruba a conexao que
    /// comeca com dado).
    #[test]
    fn conexao_nova_comeca_pelo_registro_mesmo_com_trafego() {
        let (no, rep) = (
            phxsql_core::x25519::gerar_privada(),
            phxsql_core::x25519::gerar_privada(),
        );
        let pr = phxsql_core::x25519::chave_publica(&rep);
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let cfg = CfgFio {
            escolha: Escolha::Tcp,
            porta_tcp: l.local_addr().unwrap().port(),
            ..CfgFio::default()
        };
        let f = std::sync::Arc::new(
            FioRepasse::novo(&no, &pr, "127.0.0.1:9".parse().unwrap(), cfg).unwrap(),
        );
        let f2 = std::sync::Arc::clone(&f);
        let leitor = std::thread::spawn(move || {
            let _ = f2.receber();
        });
        let (mut s, _) = l.accept().unwrap();
        while f.escrita.lock().unwrap().is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let dado = repasse::embrulhar_para(&[7u8; 32], b"ping da placa");
        f.enviar(&udp, &dado);
        let reg = repasse::registro(&no, &pr, crate::transporte::carimbo_agora(), None).unwrap();
        f.enviar(&udp, &reg);
        f.enviar(&udp, &dado);
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut enq = Enquadrador::novo(TETO_QUADRO);
        let primeiro = enq.ler(&mut s).unwrap().expect("nada chegou");
        assert_eq!(
            primeiro[0],
            repasse::TIPO_REGISTRO,
            "a conexao comecou com dado"
        );
        let segundo = enq
            .ler(&mut s)
            .unwrap()
            .expect("o dado depois do REGISTRO sumiu");
        assert_eq!(segundo, dado);
        leitor.join().unwrap();
    }
}
