//! O repasse servindo UDP e TCP ao mesmo tempo, com UMA tabela de nos.
//!
//! Os dois fios passam pelo MESMO `Repasse::tratar_de`: a conta (HMAC antes
//! do DH), o mac do REGISTRO, o carimbo crescente, a lista de permitidas e o
//! limitador de tentativas sao os mesmos. Um no por UDP fala com um no por
//! TCP sem saber -- o repasse so troca o envelope.
//!
//! # Tetos (nada de memoria grande antes de o par provar quem e)
//!
//! * Conexao anonima: le no maximo um quadro de REGISTRO (`TETO_QUADRO_ANONIMO`
//!   bytes), tem `PRAZO_ANONIMO` para registrar (prazo TOTAL, nao por
//!   leitura: um byte a cada 4 s nao a segura), e nao ganha fila de saida
//!   nem thread de escrita. O primeiro quadro que nao registra fecha a conexao.
//! * No maximo `TETO_ANONIMAS` conexoes anonimas e `TETO_CONEXOES` no total;
//!   acima disso, a conexao e fechada ao aceitar.
//! * Conexao registrada: fila de saida de `TETO_FILA` bytes. Par lento que
//!   nao le perde pacote (como perderia no UDP) -- nao segura o repasse e
//!   nao faz a memoria crescer.

use crate::fio::{Enquadrador, TETO_QUADRO};
use crate::repasse::{Ponta, Repasse, TIPO_REGISTRO, VALIDADE_REGISTRO};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// O maior REGISTRO: 80 bytes + usuario (1 + ate 32) + mac da conta (32).
pub const TETO_QUADRO_ANONIMO: usize = 80 + 1 + 32 + 32;
pub const PRAZO_ANONIMO: Duration = Duration::from_secs(5);
pub const TETO_ANONIMAS: usize = 64;
pub const TETO_CONEXOES: usize = 1024;
pub const TETO_FILA: usize = 256 * 1024;
/// Pilha das threads de conexao: o trabalho delas e ler e escrever quadros.
const PILHA: usize = 128 * 1024;

/// A fila de saida de uma conexao registrada.
struct Saida {
    fila: Mutex<(VecDeque<Vec<u8>>, usize, bool)>,
    sinal: Condvar,
}

impl Saida {
    fn nova() -> Saida {
        Saida {
            fila: Mutex::new((VecDeque::new(), 0, false)),
            sinal: Condvar::new(),
        }
    }

    /// Enfileira um quadro; acima do teto, descarta.
    fn por(&self, quadro: Vec<u8>) -> bool {
        let mut f = self.fila.lock().expect("fila");
        if f.2 || f.1 + quadro.len() > TETO_FILA {
            return false;
        }
        f.1 += quadro.len();
        f.0.push_back(quadro);
        self.sinal.notify_one();
        true
    }

    fn fechar(&self) {
        self.fila.lock().expect("fila").2 = true;
        self.sinal.notify_all();
    }

    /// Escreve ate a conexao cair ou a fila fechar. Junta os quadros que
    /// esperam numa escrita so: sob carga, uma chamada ao sistema por lote.
    fn escrever(&self, mut s: TcpStream) {
        loop {
            let lote: Vec<u8> = {
                let mut f = self.fila.lock().expect("fila");
                while f.0.is_empty() && !f.2 {
                    f = self.sinal.wait(f).expect("fila");
                }
                if f.2 {
                    break;
                }
                f.1 = 0;
                f.0.drain(..).flatten().collect()
            };
            if s.write_all(&lote).is_err() {
                break;
            }
        }
        self.fechar();
        let _ = s.shutdown(Shutdown::Both);
    }
}

/// O repasse inteiro: a tabela, o soquete UDP e as conexoes TCP registradas.
pub struct Central {
    repasse: Mutex<Repasse>,
    udp: UdpSocket,
    conexoes: Mutex<HashMap<SocketAddr, Arc<Saida>>>,
    anonimas: AtomicUsize,
    total: AtomicUsize,
}

impl Central {
    pub fn nova(repasse: Repasse, udp: UdpSocket) -> Arc<Central> {
        Arc::new(Central {
            repasse: Mutex::new(repasse),
            udp,
            conexoes: Mutex::new(HashMap::new()),
            anonimas: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
        })
    }

    /// Conexoes TCP abertas agora (anonimas, total) -- para as provas.
    pub fn contagem(&self) -> (usize, usize) {
        (
            self.anonimas.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    fn tratar(&self, dado: &[u8], de: Ponta) -> Option<(Ponta, Vec<u8>)> {
        self.repasse.lock().expect("repasse").tratar_de(dado, de)
    }

    fn entregar(&self, alvo: Ponta, pacote: &[u8]) {
        match alvo {
            Ponta::Udp(a) => {
                let _ = self.udp.send_to(pacote, a);
            }
            Ponta::Tcp(a) => {
                let s = self.conexoes.lock().expect("conexoes").get(&a).cloned();
                if let (Some(s), Some(q)) = (s, crate::fio::quadro(pacote)) {
                    s.por(q);
                }
            }
        }
    }

    /// O laco do UDP (para sempre).
    pub fn servir_udp(self: &Arc<Self>) -> Result<(), String> {
        let mut buf = vec![0u8; 65_535];
        loop {
            let (n, de) = self.udp.recv_from(&mut buf).map_err(|e| e.to_string())?;
            if let Some((alvo, p)) = self.tratar(&buf[..n], Ponta::Udp(de)) {
                self.entregar(alvo, &p);
            }
        }
    }

    /// O laco do TCP (para sempre): uma thread por conexao aceita.
    pub fn servir_tcp(self: &Arc<Self>, ouvinte: TcpListener) {
        for s in ouvinte.incoming() {
            let Ok(s) = s else {
                continue;
            };
            if self.total.load(Ordering::Relaxed) >= TETO_CONEXOES
                || self.anonimas.load(Ordering::Relaxed) >= TETO_ANONIMAS
            {
                let _ = s.shutdown(Shutdown::Both);
                continue;
            }
            self.total.fetch_add(1, Ordering::Relaxed);
            self.anonimas.fetch_add(1, Ordering::Relaxed);
            let c = Arc::clone(self);
            let r = std::thread::Builder::new()
                .stack_size(PILHA)
                .spawn(move || c.conexao(s));
            if r.is_err() {
                self.total.fetch_sub(1, Ordering::Relaxed);
                self.anonimas.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }

    fn conexao(self: Arc<Self>, s: TcpStream) {
        let par = s.peer_addr().ok();
        let registrada = match par {
            Some(par) => self.atender(s, par),
            None => false,
        };
        if !registrada {
            self.anonimas.fetch_sub(1, Ordering::Relaxed);
        }
        self.total.fetch_sub(1, Ordering::Relaxed);
    }

    /// Devolve se a conexao chegou a registrar (e ja saiu das anonimas).
    fn atender(&self, mut s: TcpStream, par: SocketAddr) -> bool {
        let _ = s.set_nodelay(true);
        let inicio = Instant::now();
        let mut enq = Enquadrador::novo(TETO_QUADRO_ANONIMO);
        // Fase anonima: prazo TOTAL de registro, lido em fatias de 1 s.
        if s.set_read_timeout(Some(Duration::from_secs(1))).is_err() {
            return false;
        }
        let primeiro = loop {
            if inicio.elapsed() >= PRAZO_ANONIMO {
                let _ = s.shutdown(Shutdown::Both);
                return false;
            }
            match enq.ler(&mut s) {
                Ok(Some(q)) => break q,
                Ok(None) => {}
                Err(_) => return false,
            }
        };
        let confirma = match (primeiro.first(), self.tratar(&primeiro, Ponta::Tcp(par))) {
            (Some(&TIPO_REGISTRO), Some((Ponta::Tcp(a), c))) if a == par => c,
            _ => {
                let _ = s.shutdown(Shutdown::Both);
                return false;
            }
        };
        // Registrada: agora ganha fila e escritor, e o teto do quadro sobe.
        let Ok(escrita) = s.try_clone() else {
            return false;
        };
        let saida = Arc::new(Saida::nova());
        let s2 = Arc::clone(&saida);
        if std::thread::Builder::new()
            .stack_size(PILHA)
            .spawn(move || s2.escrever(escrita))
            .is_err()
        {
            return false;
        }
        self.anonimas.fetch_sub(1, Ordering::Relaxed);
        self.conexoes
            .lock()
            .expect("conexoes")
            .insert(par, Arc::clone(&saida));
        if let Some(q) = crate::fio::quadro(&confirma) {
            saida.por(q);
        }
        enq.trocar_teto(TETO_QUADRO);
        // O no renova a cada 20 s; mudo pela validade inteira, cai.
        let _ = s.set_read_timeout(Some(VALIDADE_REGISTRO));
        while let Ok(Some(q)) = enq.ler(&mut s) {
            if let Some((alvo, p)) = self.tratar(&q, Ponta::Tcp(par)) {
                self.entregar(alvo, &p);
            }
        }
        self.conexoes.lock().expect("conexoes").remove(&par);
        saida.fechar();
        let _ = s.shutdown(Shutdown::Both);
        true
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::fio::quadro;
    use crate::repasse::{self, embrulhar_para, registro};
    use phxsql_core::x25519;
    use std::io::Read;

    fn central() -> (Arc<Central>, [u8; 32], SocketAddr, SocketAddr) {
        let rp = x25519::gerar_privada();
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let eu = udp.local_addr().unwrap();
        let c = Central::nova(Repasse::novo(rp, None), udp);
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let et = l.local_addr().unwrap();
        let c2 = Arc::clone(&c);
        std::thread::spawn(move || c2.servir_tcp(l));
        let c3 = Arc::clone(&c);
        std::thread::spawn(move || c3.servir_udp());
        (c, x25519::chave_publica(&rp), eu, et)
    }

    fn ler_quadro(s: &mut TcpStream) -> Option<Vec<u8>> {
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        Enquadrador::novo(TETO_QUADRO).ler(s).ok().flatten()
    }

    /// Um no por TCP e outro por UDP: registram, sao confirmados, e o
    /// pacote atravessa nos dois sentidos -- o repasse so troca o envelope.
    #[test]
    fn tcp_e_udp_falam_pela_mesma_tabela() {
        let (_c, rp, eu, et) = central();
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let (pa, pb) = (x25519::chave_publica(&a), x25519::chave_publica(&b));
        let mut ta = TcpStream::connect(et).unwrap();
        ta.write_all(
            &quadro(&registro(&a, &rp, crate::transporte::carimbo_agora(), None).unwrap()).unwrap(),
        )
        .unwrap();
        let conf = ler_quadro(&mut ta).expect("sem confirmacao pelo TCP");
        assert_eq!(conf[0], repasse::TIPO_CONFIRMA);
        let ub = UdpSocket::bind("127.0.0.1:0").unwrap();
        ub.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        ub.send_to(
            &registro(&b, &rp, crate::transporte::carimbo_agora(), None).unwrap(),
            eu,
        )
        .unwrap();
        let mut buf = [0u8; 256];
        let (n, _) = ub.recv_from(&mut buf).unwrap();
        assert_eq!(buf[0], repasse::TIPO_CONFIRMA, "{n}");
        // A (TCP) -> B (UDP)
        ta.write_all(&quadro(&embrulhar_para(&pb, b"ida")).unwrap())
            .unwrap();
        let (n, _) = ub.recv_from(&mut buf).unwrap();
        assert_eq!(repasse::desembrulhar_de(&buf[..n]), Some((pa, &b"ida"[..])));
        // B (UDP) -> A (TCP)
        ub.send_to(&embrulhar_para(&pa, b"volta"), eu).unwrap();
        let q = ler_quadro(&mut ta).expect("nada voltou pelo TCP");
        assert_eq!(repasse::desembrulhar_de(&q), Some((pb, &b"volta"[..])));
    }

    /// Conexao anonima: quadro acima do teto de um REGISTRO, lixo no lugar
    /// do REGISTRO, ou silencio -- fecha, e o contador de anonimas volta.
    #[test]
    fn anonima_que_nao_registra_e_fechada() {
        let (c, rp, _, et) = central();
        // Quadro grande antes de registrar: o cabecalho anuncia 60.000 bytes
        // e so 10 chegam. Com o teto do REGISTRO, fecha JA no cabecalho; sem
        // ele, o repasse guardaria e esperaria o resto ate o prazo (5 s).
        let t = Instant::now();
        let mut s = TcpStream::connect(et).unwrap();
        s.write_all(&60_000u16.to_be_bytes()).unwrap();
        s.write_all(&[7u8; 10]).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
        let mut b = [0u8; 1];
        assert_eq!(s.read(&mut b).unwrap_or(0), 0);
        assert!(
            t.elapsed() < Duration::from_secs(1),
            "quadro acima do teto anonimo foi aceito e esperado: {:?}",
            t.elapsed()
        );
        // REGISTRO com mac torto.
        let mut s = TcpStream::connect(et).unwrap();
        let mut r = registro(
            &x25519::gerar_privada(),
            &rp,
            crate::transporte::carimbo_agora(),
            None,
        )
        .unwrap();
        r[50] ^= 1;
        s.write_all(&quadro(&r).unwrap()).unwrap();
        assert!(ler_quadro(&mut s).is_none());
        // PARA antes de registrar: nao e registro, fecha.
        let mut s = TcpStream::connect(et).unwrap();
        s.write_all(&quadro(&embrulhar_para(&[1; 32], b"x")).unwrap())
            .unwrap();
        let mut b = [0u8; 1];
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        assert_eq!(s.read(&mut b).unwrap_or(0), 0, "a conexao seguiu aberta");
        // Silencio: prazo total de 5 s.
        let t = Instant::now();
        let mut s = TcpStream::connect(et).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
        let _ = s.write_all(&[0]); // meio cabecalho, e para
        assert_eq!(s.read(&mut b).unwrap_or(0), 0);
        let dt = t.elapsed();
        assert!(
            dt >= PRAZO_ANONIMO && dt < PRAZO_ANONIMO + Duration::from_secs(2),
            "{dt:?}"
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(c.contagem(), (0, 0), "conexao anonima ficou contada");
    }

    #[test]
    fn teto_de_anonimas_fecha_ao_aceitar() {
        let (c, _, _, et) = central();
        let presas: Vec<TcpStream> = (0..TETO_ANONIMAS)
            .map(|_| TcpStream::connect(et).unwrap())
            .collect();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(c.contagem().0, TETO_ANONIMAS);
        let mut mais = TcpStream::connect(et).unwrap();
        mais.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut b = [0u8; 1];
        assert_eq!(
            mais.read(&mut b).unwrap_or(0),
            0,
            "a 65a anonima foi aceita"
        );
        drop(presas);
    }

    /// Dois nos P2P de verdade (`p2p::No`) cujo UDP ate o repasse cai num
    /// buraco -- a porta UDP que eles conhecem nao tem ninguem. Com o fio
    /// `auto`, caem para TCP e o pacote atravessa cifrado; com `udp`, nada
    /// passa. E a prova dos dois sentidos, em miniatura, da `provas/tcp/`.
    fn dois_nos_pelo_tcp(escolha: crate::fio::Escolha) -> bool {
        use crate::fio::CfgFio;
        use crate::p2p::{psk_da_rede, Modo, No, ParConfig, RepasseCfg};
        let (c, rp, _, et) = central();
        let _ = &c;
        let buraco = UdpSocket::bind("127.0.0.1:0").unwrap();
        let morto = buraco.local_addr().unwrap();
        drop(buraco);
        let (ka, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
        let novo = |k: [u8; 32], meu: &str, dele: [u8; 32], ip_dele: &str| {
            let u = UdpSocket::bind("127.0.0.1:0").unwrap();
            Arc::new(
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
                        endereco: morto,
                        publica: rp,
                        conta: None,
                    }),
                )
                .unwrap()
                .com_fio(CfgFio {
                    escolha,
                    porta_tcp: et.port(),
                    reserva: Duration::from_millis(300),
                    ..CfgFio::default()
                })
                .unwrap(),
            )
        };
        let a = novo(ka, "10.78.0.1", x25519::chave_publica(&kb), "10.78.0.2");
        let b = novo(kb, "10.78.0.2", x25519::chave_publica(&ka), "10.78.0.1");
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        for no in [&a, &b] {
            let (n1, n2, tx) = (Arc::clone(no), Arc::clone(no), tx.clone());
            std::thread::spawn(move || loop {
                if let Some(ip) = n1.fio().unwrap().receber().and_then(|q| n1.da_repasse(&q)) {
                    let _ = tx.send(ip);
                }
            });
            std::thread::spawn(move || loop {
                n2.tique();
                std::thread::sleep(Duration::from_millis(100));
            });
        }
        let mut ida = vec![
            0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0, 10, 78, 0, 1, 10, 78, 0, 2,
        ];
        ida.extend_from_slice(b"SEGREDO-PELO-TCP");
        let fim = Instant::now() + Duration::from_secs(6);
        while Instant::now() < fim {
            a.da_placa(&ida);
            if let Ok(p) = rx.recv_timeout(Duration::from_millis(200)) {
                if p == ida {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn auto_cai_para_tcp_e_o_pacote_atravessa() {
        assert!(
            dois_nos_pelo_tcp(crate::fio::Escolha::Auto),
            "com UDP no buraco, o auto nao levou o pacote pelo TCP"
        );
    }

    #[test]
    fn so_udp_com_udp_no_buraco_nao_passa_nada() {
        assert!(
            !dois_nos_pelo_tcp(crate::fio::Escolha::Udp),
            "passou sem TCP: a prova do auto nao prova nada"
        );
    }
}
