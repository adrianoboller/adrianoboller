//! O fio da porta de DADOS: claro ou TLS (pedido 572, T6).
//!
//! A porta 5000 le por um `BufReader` e escreve por um clone do soquete. Com
//! TLS o fluxo e UM so -- o registro protegido tem estado (a sequencia de
//! cada sentido) e nao se clona. Os dois lados passam a compartilhar o mesmo
//! `FluxoTls` na thread da conexao, e isso e seguro porque o protocolo da
//! porta e pedido-resposta na MESMA thread: le uma linha, responde, le a
//! proxima. Nunca ha leitura e escrita ao mesmo tempo.
//!
//! O resto do laco nao sabe qual dos dois esta por baixo -- a mesma razao do
//! `FioWeb` das portas HTTP e do `Canal` do `fio.rs`.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::rc::Rc;

use phxsql_core::tls::FluxoTls;

type Tls = Rc<RefCell<FluxoTls<TcpStream>>>;

pub enum Leitura {
    Claro(TcpStream),
    Tls(Tls),
}

pub enum Escrita {
    Claro(TcpStream),
    Tls(Tls),
}

/// Os dois lados de um fio em claro: o soquete e um clone dele.
pub fn claro(fluxo: TcpStream) -> std::io::Result<(Leitura, Escrita)> {
    let escrita = fluxo.try_clone()?;
    Ok((Leitura::Claro(fluxo), Escrita::Claro(escrita)))
}

/// Os dois lados de um fio TLS, sobre o mesmo fluxo protegido.
pub fn tls(fluxo: FluxoTls<TcpStream>) -> (Leitura, Escrita) {
    let comum = Rc::new(RefCell::new(fluxo));
    (Leitura::Tls(Rc::clone(&comum)), Escrita::Tls(comum))
}

impl Escrita {
    pub fn cifrado(&self) -> bool {
        matches!(self, Escrita::Tls(_))
    }
}

impl Read for Leitura {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Leitura::Claro(t) => t.read(buf),
            Leitura::Tls(t) => t.borrow_mut().read(buf),
        }
    }
}

impl Write for Escrita {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Escrita::Claro(t) => t.write(buf),
            Escrita::Tls(t) => t.borrow_mut().write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Escrita::Claro(t) => t.flush(),
            Escrita::Tls(t) => t.borrow_mut().flush(),
        }
    }
}

/// O `close_notify` sai no `Drop`, pelo mesmo motivo do `FioWeb`: o laco tem
/// dezenas de `return`, e o que alguem esquecesse fecharia sem ele.
impl Drop for Escrita {
    fn drop(&mut self) {
        if let Escrita::Tls(t) = self {
            if let Ok(mut t) = t.try_borrow_mut() {
                let _ = t.despedir();
            }
        }
    }
}

/// A frase da transicao Noise -> TLS (decisao do dono, 01/10/2026, plano
/// `plano-tls13-572` T6b-2). Uma so, para o teste e o operador procurarem o
/// mesmo texto.
pub const FRASE_DO_NOISE: &str = "o Noise será recusado na 0.20; use TLS";

/// Quanto tempo o aviso de UM par fica calado depois de dito.
///
/// Uma hora: o par que reconecta a cada segundo (a replica e o pulso do
/// cluster fazem isso) viraria uma linha por segundo e enterraria o resto do
/// log; uma hora ainda lembra o operador que o no antigo segue la.
pub const SILENCIO_DO_NOISE_MS: i64 = 3_600_000;

/// Teto dos pares lembrados: a memoria do aviso nao cresce com o numero de
/// enderecos que um dia bateram na porta. Cheio e sem nada vencido, os pares
/// novos ficam calados -- melhor que esquecer os velhos e voltar a gritar.
const TETO_DE_PARES_DO_NOISE: usize = 1024;

/// A memoria do aviso de Noise: silencio POR PAR, como o carteiro faz por
/// tipo. Vive no `Servidor`, e e consultada num ponto so
/// (`Servidor::responder_aperto`).
#[derive(Default)]
pub struct AvisoDoNoise {
    visto: std::sync::Mutex<std::collections::HashMap<String, i64>>,
}

impl AvisoDoNoise {
    /// Este par deve ser avisado agora? Sim na primeira vez e depois de
    /// [`SILENCIO_DO_NOISE_MS`]; e ja anota que avisou.
    pub fn avisar(&self, par: &str, agora_ms: i64) -> bool {
        let mut v = self.visto.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(ultimo) = v.get_mut(par) {
            if agora_ms.saturating_sub(*ultimo) < SILENCIO_DO_NOISE_MS {
                return false;
            }
            *ultimo = agora_ms;
            return true;
        }
        if v.len() >= TETO_DE_PARES_DO_NOISE {
            v.retain(|_, ultimo| agora_ms.saturating_sub(*ultimo) < SILENCIO_DO_NOISE_MS);
            if v.len() >= TETO_DE_PARES_DO_NOISE {
                return false;
            }
        }
        v.insert(par.to_string(), agora_ms);
        true
    }
}

/// A linha de log. Sem segredo nem caminho: o endereco do par e quem o
/// iniciou (um no da lista do cluster, ou um cliente qualquer).
pub fn linha_do_noise(par: &str, iniciador: &str) -> String {
    format!("aviso: conexao por Noise do par {par} (iniciador: {iniciador}): {FRASE_DO_NOISE}")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_aviso_e_um_por_par_e_volta_depois_do_silencio() {
        let a = AvisoDoNoise::default();
        assert!(a.avisar("10.0.0.2", 1_000));
        assert!(!a.avisar("10.0.0.2", 2_000), "o mesmo par reconectando");
        assert!(a.avisar("10.0.0.3", 2_000), "outro par e outra linha");
        assert!(a.avisar("10.0.0.2", 1_000 + SILENCIO_DO_NOISE_MS));
    }

    #[test]
    fn a_memoria_dos_pares_tem_teto() {
        let a = AvisoDoNoise::default();
        for i in 0..TETO_DE_PARES_DO_NOISE {
            assert!(a.avisar(&format!("p{i}"), 10));
        }
        assert!(!a.avisar("novo", 11), "cheia e sem nada vencido: calada");
        assert!(a.avisar("novo", 10 + SILENCIO_DO_NOISE_MS), "venceu: cabe");
    }

    #[test]
    fn a_linha_traz_a_frase_do_dono_e_nenhum_caminho() {
        let l = linha_do_noise("10.0.0.2", "no2");
        assert!(l.contains(FRASE_DO_NOISE), "{l}");
        assert!(!l.contains('/'), "{l}");
    }
}
