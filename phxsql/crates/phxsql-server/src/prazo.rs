//! O prazo TOTAL de uma conversa de soquete -- um motor so (pedido 578).
//!
//! # Por que existe
//!
//! O prazo de soquete da `std` (`set_read_timeout`) mede o SILENCIO: vale
//! para cada leitura e recomeca a cada byte. Um par que manda um byte a um
//! passo do prazo nunca estoura, e a thread que conversa com ele fica presa
//! pelo tempo que ELE quiser. O `email.rs` pagou isso primeiro (pedido 463) e
//! escreveu o `ComPrazo` para si; o DbLink tinha o mesmo buraco nos tres
//! clientes (mysql, pg e phx), e a thread presa ali e a de um job ou a de uma
//! conexao do servidor.
//!
//! Morar aqui, e nao copiado ao lado de cada cliente, e a petrea «funcao e
//! comando vem do mesmo motor»: a pergunta «quanto esta leitura ainda pode
//! esperar» e UMA so, e a copia que alguem esquecesse de corrigir seria o
//! cliente que volta a prender a thread.
//!
//! # O desenho
//!
//! Cada `read` e cada `write` perguntam ao [`Prazo`] quanto ainda cabe -- o
//! silencio ou o que sobra do total, o que for menor -- e armam o soquete com
//! isso ANTES de ir ao nucleo. E por syscall, e nao por linha nem por quadro:
//! um quadro gotejado byte a byte tambem para no total.
//!
//! Sem total ([`Prazo::so_silencio`]) o caminho nao toca o soquete a cada
//! leitura: o silencio foi armado uma vez em [`ComPrazo::armar`], e quem nao
//! pediu total (a replica) nao paga `setsockopt` por leitura.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use phxsql_core::error::PhxError;

/// Como o erro do total se apresenta: QUEM passou do prazo e QUAL regra o
/// calculou. Estatico porque cada cliente tem um so, e o erro precisa dizer a
/// regra para quem opera saber o que ajustar.
#[derive(Debug)]
pub struct Rotulo {
    pub quem: &'static str,
    pub regra: &'static str,
}

/// Os dois prazos de uma conversa: o de SILENCIO, por leitura e por escrita,
/// e o TOTAL, opcional.
#[derive(Clone, Copy, Debug)]
pub struct Prazo {
    silencio: Duration,
    total: Option<(Duration, &'static Rotulo)>,
    /// `None` sem total, ou quando o instante nao se representa (total
    /// absurdo): ai o silencio e o unico prazo, como antes.
    ate: Option<Instant>,
}

impl Prazo {
    /// So o silencio, o comportamento de antes do total.
    pub fn so_silencio(silencio: Duration) -> Prazo {
        Prazo {
            silencio,
            total: None,
            ate: None,
        }
    }

    /// Silencio e total; o relogio do total comeca AGORA.
    pub fn com_total(silencio: Duration, total: Duration, rotulo: &'static Rotulo) -> Prazo {
        Prazo {
            silencio,
            total: Some((total, rotulo)),
            ate: Instant::now().checked_add(total),
        }
    }

    pub fn silencio(&self) -> Duration {
        self.silencio
    }

    /// O mesmo prazo com o relogio do total recomecado agora -- e o que marca
    /// o comeco de uma operacao nova numa conexao que dura varias.
    pub fn rearmado(&self) -> Prazo {
        Prazo {
            ate: self.total.and_then(|(t, _)| Instant::now().checked_add(t)),
            ..*self
        }
    }

    /// Quanto ESTA leitura ou escrita pode esperar. `None` e «so o silencio,
    /// que ja esta armado» -- o caminho sem total nao toca o soquete.
    fn espera(&self) -> io::Result<Option<Duration>> {
        let Some(ate) = self.ate else {
            return Ok(None);
        };
        let resta = ate.saturating_duration_since(Instant::now());
        if resta.is_zero() {
            return Err(self.esgotado());
        }
        Ok(Some(resta.min(self.silencio)))
    }

    /// O erro do soquete, trocado pelo do total quando foi ELE que acabou --
    /// e nao o silencio de sempre, que continua dizendo o que diz.
    fn explicar(&self, e: io::Error) -> io::Error {
        let esgotou = self.ate.is_some_and(|ate| Instant::now() >= ate);
        if esgotou
            && matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            )
        {
            self.esgotado()
        } else {
            e
        }
    }

    fn esgotado(&self) -> io::Error {
        let (total, rotulo) = self.total.expect("so ha instante limite quando ha total");
        io::Error::new(io::ErrorKind::TimedOut, PrazoEsgotado { total, rotulo })
    }
}

/// O prazo total acabou. Tipo proprio, e nao texto, para quem recebe o erro o
/// reconhecer sem comparar frase.
#[derive(Debug)]
pub struct PrazoEsgotado {
    total: Duration,
    rotulo: &'static Rotulo,
}

impl std::fmt::Display for PrazoEsgotado {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} passou do prazo total de {:.1} s -- {}",
            self.rotulo.quem,
            self.total.as_secs_f64(),
            self.rotulo.regra
        )
    }
}

impl std::error::Error for PrazoEsgotado {}

/// O erro de E/S de quem conversa: o do total vira `LimiteExcedido`, que e o
/// que ele e; o resto segue pelo caminho que cada cliente ja tinha.
pub fn classificar(e: io::Error, senao: impl FnOnce(io::Error) -> PhxError) -> PhxError {
    match e.get_ref().and_then(|x| x.downcast_ref::<PrazoEsgotado>()) {
        Some(p) => PhxError::LimiteExcedido(p.to_string()),
        None => senao(e),
    }
}

/// O mesmo, para o erro que ja virou `PhxError` -- o `Canal` embrulha o de
/// E/S em `PhxError::Io`, e sem isto o total chegaria como erro de E/S cru.
pub fn reclassificar(e: PhxError) -> PhxError {
    match e {
        PhxError::Io(io) => classificar(io, PhxError::Io),
        outro => outro,
    }
}

/// O soquete com o [`Prazo`] por cima.
#[derive(Debug)]
pub struct ComPrazo {
    fluxo: TcpStream,
    prazo: Prazo,
}

impl ComPrazo {
    /// As duas metades da conversa -- `(leitura, escrita)` -- sobre o mesmo
    /// soquete e o mesmo prazo. O silencio fica armado aqui uma vez, que e o
    /// que deixa o caminho sem total igual ao de antes.
    pub fn armar(fluxo: TcpStream, prazo: Prazo) -> io::Result<(ComPrazo, ComPrazo)> {
        fluxo.set_read_timeout(Some(prazo.silencio))?;
        fluxo.set_write_timeout(Some(prazo.silencio))?;
        let outra = fluxo.try_clone()?;
        Ok((
            ComPrazo {
                fluxo: outra,
                prazo,
            },
            ComPrazo { fluxo, prazo },
        ))
    }

    pub fn prazo(&self) -> Prazo {
        self.prazo
    }
}

/// Recomeca o total nas DUAS metades com o mesmo instante: uma operacao nova
/// tem o total inteiro, e a metade esquecida cortaria a operacao pelo relogio
/// da anterior.
pub fn rearmar(leitura: &mut ComPrazo, escrita: &mut ComPrazo) {
    let p = escrita.prazo.rearmado();
    leitura.prazo = p;
    escrita.prazo = p;
}

impl Read for ComPrazo {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        if let Some(espera) = self.prazo.espera()? {
            self.fluxo.set_read_timeout(Some(espera))?;
        }
        self.fluxo.read(b).map_err(|e| self.prazo.explicar(e))
    }
}

impl Write for ComPrazo {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if let Some(espera) = self.prazo.espera()? {
            self.fluxo.set_write_timeout(Some(espera))?;
        }
        self.fluxo.write(b).map_err(|e| self.prazo.explicar(e))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.fluxo.flush()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::net::TcpListener;

    static ROTULO: Rotulo = Rotulo {
        quem: "teste",
        regra: "regra de teste",
    };

    /// Um par que manda `bytes` bytes, um a cada `passo`, e fecha.
    fn par_que_goteja(bytes: usize, passo: Duration) -> TcpStream {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = ouvinte.accept() else {
                return;
            };
            for _ in 0..bytes {
                std::thread::sleep(passo);
                if s.write_all(b"x").is_err() {
                    return;
                }
            }
        });
        TcpStream::connect(("127.0.0.1", porta)).unwrap()
    }

    fn ler(c: &mut ComPrazo, n: usize) -> io::Result<()> {
        let mut b = vec![0u8; n];
        c.read_exact(&mut b)
    }

    /// **O rearme e por OPERACAO, e e nas duas metades.** Duas leituras de
    /// ~0,6 s cada com total de 1 s: sem rearme a segunda passa do relogio
    /// da primeira e e cortada; com rearme, as duas cabem. A operacao que
    /// passa sozinha do total continua cortada, com o erro que diz qual regra.
    #[test]
    fn o_total_recomeca_a_cada_operacao_e_corta_a_que_passa() {
        let passo = Duration::from_millis(100);
        let prazo = Prazo::com_total(Duration::from_millis(500), Duration::from_secs(1), &ROTULO);
        let (mut leitura, mut escrita) =
            ComPrazo::armar(par_que_goteja(1000, passo), prazo).unwrap();
        ler(&mut leitura, 6).unwrap();
        rearmar(&mut leitura, &mut escrita);
        ler(&mut leitura, 6).unwrap();
        rearmar(&mut leitura, &mut escrita);
        let relogio = Instant::now();
        let e = ler(&mut leitura, 1000).unwrap_err();
        let durou = relogio.elapsed();
        match classificar(e, PhxError::Io) {
            PhxError::LimiteExcedido(m) => {
                assert!(m.contains("teste passou do prazo total"), "{m}");
                assert!(m.contains("regra de teste"), "{m}");
            }
            outro => panic!("esperava o prazo total, veio {outro:?}"),
        }
        assert!(durou < Duration::from_secs(3), "{durou:?}");
    }

    /// Sem total, o silencio continua sendo o unico prazo: o gotejo lento
    /// passa, como passava antes.
    #[test]
    fn so_silencio_nao_corta_o_gotejo() {
        let (mut leitura, _escrita) = ComPrazo::armar(
            par_que_goteja(8, Duration::from_millis(50)),
            Prazo::so_silencio(Duration::from_millis(500)),
        )
        .unwrap();
        ler(&mut leitura, 8).unwrap();
    }
}
