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
