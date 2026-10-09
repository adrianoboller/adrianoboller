//! A leitura de um pedido HTTP/1.1 -- o motor UNICO das portas web desta casa.
//!
//! # Por que mora no core
//!
//! Pedido 454 (fatia Z1 do `docs/propostas/plano-0.21.md`). O servidor do
//! PhxSql le HTTP desde a interface web, e o PhxZipWeb (`crates/phxzip-web`)
//! precisa ler o mesmo protocolo. Copiar o leitor para la seria a duplicacao
//! que a petrea «funcao e comando vem do mesmo motor» proibe -- e o custo dela
//! ja esta medido nesta casa: o pedido 434 nasceu de uma leitura de linha que
//! vivia fora do `Canal`. Dois leitores de HTTP teriam dois tetos, e o que
//! alguem esquecesse seria a porta dos fundos.
//!
//! # O que muda em relacao ao leitor do servidor
//!
//! * **O corpo e `Vec<u8>`.** O PhxZipWeb recebe pacote binario no corpo, e
//!   o leitor antigo passava tudo por `String::from_utf8_lossy` -- o byte 0xFF
//!   virava `EF BF BD` calado. Quem quer texto (o JSON do servidor) pede o
//!   texto; quem quer bytes recebe os bytes que vieram.
//! * **O teto e de quem chama** ([`Tetos`]). A porta do PhxSql aceita 4 MiB de
//!   JSON; a do PhxZip aceita um pacote. A DECISAO de conferir antes de ler e
//!   uma so; o numero e de cada porta.
//! * **O corpo cresce com o que chega, nao com o que se declara.** O leitor
//!   antigo reservava `vec![0u8; Content-Length]` de uma vez: com teto de 4 MiB
//!   isso era aceitavel, com o teto de um pacote seria dar a quem manda so o
//!   cabecalho o poder de fazer este lado reservar centenas de MiB.
//! * **[`drenar`]** vem junto, porque responder 413 sem drenar e responder
//!   para ninguem (ver a funcao).

use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::net::TcpStream;
use std::time::Duration;

use crate::error::PhxError;
use crate::fio::{Canal, Recebido};

/// O fio de uma porta HTTP: o que o atendimento le e escreve, com o prazo de
/// leitura que o [`drenar`] ajusta.
///
/// Mora aqui, e nao no servidor, porque o [`drenar`] precisa dele e o
/// `impl` para `TcpStream` so pode morar no crate do trait.
pub trait FioHttp: Read + std::io::Write {
    fn prazo(&self) -> Option<Duration>;
    fn por_prazo(&self, prazo: Option<Duration>) -> std::io::Result<()>;
}

impl FioHttp for TcpStream {
    fn prazo(&self) -> Option<Duration> {
        self.read_timeout().ok().flatten()
    }
    fn por_prazo(&self, prazo: Option<Duration>) -> std::io::Result<()> {
        self.set_read_timeout(prazo)
    }
}

/// Os tetos de uma porta, em bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tetos {
    /// Vale para cada linha E para o cabecalho acumulado.
    pub cabecalho: usize,
    /// O `Content-Length` maximo, conferido ANTES de ler um byte do corpo.
    pub corpo: usize,
}

/// Um pedido lido.
#[derive(Debug, Clone)]
pub struct Pedido {
    pub metodo: String,
    /// O caminho SEM a query, que e o que as rotas casam por igualdade.
    pub caminho: String,
    /// A query crua, sem o `?`.
    pub consulta: String,
    /// Nomes em minusculas: HTTP nao distingue caixa no nome do cabecalho.
    pub cabecalhos: HashMap<String, String>,
    /// Os bytes do corpo, como vieram.
    pub corpo: Vec<u8>,
}

impl Pedido {
    pub fn cabecalho(&self, nome: &str) -> Option<&str> {
        self.cabecalhos
            .get(&nome.to_lowercase())
            .map(String::as_str)
    }
}

/// Qual teto o pedido passou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Excesso {
    /// A linha do pedido (`GET /x HTTP/1.1`).
    Linha { teto: usize },
    /// Uma linha de cabecalho sozinha.
    LinhaDeCabecalho { teto: usize },
    /// O cabecalho somado.
    Cabecalho { teto: usize },
    /// O `Content-Length` declarado. NENHUM byte do corpo foi lido -- e por
    /// isso quem responde o 413 tem de [`drenar`] antes de fechar.
    Corpo { declarado: u64, teto: usize },
}

impl Excesso {
    /// O motivo, com o teto em bytes, para o log de acessos.
    ///
    /// A frase e a mesma que o servidor escrevia antes de o leitor vir para o
    /// core: quem compara o `acessos.log` de antes e de depois nao ve
    /// diferenca.
    pub fn motivo(&self) -> String {
        let (o_que, teto) = match self {
            Excesso::Linha { teto } => ("a linha do pedido", *teto),
            Excesso::LinhaDeCabecalho { teto } => ("uma linha de cabecalho", *teto),
            Excesso::Cabecalho { teto } => ("o cabecalho", *teto),
            Excesso::Corpo { teto, .. } => ("o Content-Length", *teto),
        };
        format!("pedido HTTP grande demais: {o_que} passou de {teto} bytes")
    }

    /// O codigo que a norma da para este excesso (RFC 9110 §15.5.14 e
    /// RFC 6585 §5).
    pub fn codigo_http(&self) -> u16 {
        match self {
            Excesso::Corpo { .. } => 413,
            _ => 431,
        }
    }
}

/// O que a leitura de um pedido deu.
#[derive(Debug)]
pub enum PedidoLido {
    Pedido(Pedido),
    /// Conexao fechada, prazo vencido, linha torta, `Content-Length` que nao
    /// e numero, corpo mais curto que o declarado.
    Nada,
    /// Passou de um teto. Separado do [`PedidoLido::Nada`] porque so este
    /// merece linha no log: a conexao vazia de um balanceador nao (pedido 445).
    GrandeDemais(Excesso),
}

/// Le um pedido HTTP/1.1 com os tetos da porta.
///
/// A linha vem do [`Canal::Claro`] -- o mesmo `take` que protege a porta de
/// dados (pedido 434) --, entao o teto da linha vale ANTES de a linha existir
/// na memoria.
pub fn ler_pedido<R: Read>(fluxo: R, tetos: Tetos) -> PedidoLido {
    let mut leitor = BufReader::new(fluxo);
    let mut canal = Canal::Claro;
    let teto_linha = tetos.cabecalho as u64;

    let linha = match canal.ler_ate(&mut leitor, teto_linha) {
        Ok(Recebido::Linha(l)) => l,
        Err(PhxError::LimiteExcedido(_)) => {
            return PedidoLido::GrandeDemais(Excesso::Linha {
                teto: tetos.cabecalho,
            })
        }
        _ => return PedidoLido::Nada,
    };
    let mut partes = linha.split_whitespace();
    let (Some(metodo), Some(caminho)) = (partes.next(), partes.next()) else {
        return PedidoLido::Nada;
    };
    let (metodo, caminho) = (metodo.to_string(), caminho.to_string());

    let mut cabecalhos = HashMap::new();
    let mut lidos = linha.len();
    loop {
        let l = match canal.ler_ate(&mut leitor, teto_linha) {
            Ok(Recebido::Linha(l)) => l,
            Err(PhxError::LimiteExcedido(_)) => {
                return PedidoLido::GrandeDemais(Excesso::LinhaDeCabecalho {
                    teto: tetos.cabecalho,
                })
            }
            _ => return PedidoLido::Nada,
        };
        lidos += l.len();
        if lidos > tetos.cabecalho {
            return PedidoLido::GrandeDemais(Excesso::Cabecalho {
                teto: tetos.cabecalho,
            });
        }
        let t = l.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some((chave, valor)) = t.split_once(':') {
            cabecalhos.insert(chave.trim().to_lowercase(), valor.trim().to_string());
        }
    }

    // `Content-Length` que nao e numero e pedido torto (RFC 9112 §6.3, item
    // 5), e nao corpo vazio: lido como zero, os bytes do corpo ficariam no
    // fio e o 400 viraria RST no cliente.
    let tamanho: u64 = match cabecalhos.get("content-length") {
        None => 0,
        Some(v) => match v.parse() {
            Ok(n) => n,
            Err(_) => return PedidoLido::Nada,
        },
    };
    if tamanho > tetos.corpo as u64 {
        return PedidoLido::GrandeDemais(Excesso::Corpo {
            declarado: tamanho,
            teto: tetos.corpo,
        });
    }
    // A reserva inicial e limitada: o que cresce o vetor e o que CHEGA. Quem
    // declara o teto inteiro e manda so o cabecalho nao faz este lado
    // reservar nada alem disto.
    let mut corpo = Vec::with_capacity((tamanho as usize).min(64 * 1024));
    if tamanho > 0 {
        match (&mut leitor).take(tamanho).read_to_end(&mut corpo) {
            Ok(n) if n as u64 == tamanho => {}
            _ => return PedidoLido::Nada,
        }
    }

    let (so_caminho, consulta) = match caminho.split_once('?') {
        Some((c, q)) => (c.to_string(), q.to_string()),
        None => (caminho, String::new()),
    };
    PedidoLido::Pedido(Pedido {
        metodo,
        caminho: so_caminho,
        consulta,
        cabecalhos,
        corpo,
    })
}

/// Le e joga fora o que o cliente ainda estava mandando, ate `teto` bytes ou
/// ate o fio ficar `prazo` sem mandar nada. Devolve quantos bytes descartou.
///
/// # Por que existe
///
/// Fechar um soquete com bytes por ler no buffer de recepcao faz o TCP mandar
/// RST em vez de FIN, e o RST descarta a resposta em voo. Quem recusa ANTES
/// de ler o corpo -- o 413 do `Content-Length`, o 403 da lista negra -- tem de
/// drenar depois de responder, senao a recusa nunca chega: a bancada do REST
/// viu `Connection reset by peer` no lugar do 403, e o Chromium ve `Failed to
/// fetch` no lugar do 413 (`docs/cognicao/cognicao_resposta-antes-do-corpo-
/// vira-erro-de-rede_20260924_0246.md`).
///
/// # Por que com teto e prazo
///
/// Drenar sem teto daria a quem foi recusado o direito de prender uma thread
/// lendo para sempre. O prazo vale por leitura, e nao para o todo: um cliente
/// que manda sem parar continua sendo drenado ate o teto, e um que parou
/// solta a thread em `prazo`. Acima do teto, fechar e o certo -- e o cliente
/// ve a conexao cair, que e verdade.
pub fn drenar<F: FioHttp + ?Sized>(fluxo: &mut F, teto: u64, prazo: Duration) -> u64 {
    let antes = fluxo.prazo();
    let _ = fluxo.por_prazo(Some(prazo));
    let mut resto = [0u8; 8192];
    let mut lidos = 0u64;
    while lidos < teto {
        let quanto = (teto - lidos).min(resto.len() as u64) as usize;
        match fluxo.read(&mut resto[..quanto]) {
            Ok(0) | Err(_) => break,
            Ok(n) => lidos += n as u64,
        }
    }
    let _ = fluxo.por_prazo(antes);
    lidos
}

/// A frase da linha de estado de cada codigo que as portas desta casa
/// respondem.
///
/// Mora aqui pela mesma razao do [`ler_pedido`]: o PhxZipWeb responde HTTP e
/// o servidor do PhxSql tambem, e duas tabelas de frases seriam duas tabelas
/// para alguem esquecer de completar. As frases sao as da RFC 9110 §15; o 499
/// nao e da norma, e o que os balanceadores usam para «o cliente desistiu», e
/// o REST do servidor o adotou.
pub fn frase_do_codigo(codigo: u16) -> &'static str {
    match codigo {
        200 => "OK",
        // O MCP sobre HTTP responde 202 sem corpo a uma notificacao (mensagem
        // sem `id`): responder qualquer outra coisa quebra o cliente logo no
        // `notifications/initialized`, a mesma armadilha do transporte stdio.
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        // A frase do 421 e a da norma: "o servidor nao consegue produzir uma
        // resposta para a autoridade pedida" -- e e exatamente o caso de
        // escrever numa replica.
        421 => "Misdirected Request",
        422 => "Unprocessable Content",
        431 => "Request Header Fields Too Large",
        499 => "Client Closed Request",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

/// A cabeca de uma resposta: linha de estado, `Content-Type`,
/// `Content-Length`, os `extras` de cada porta e o `Connection: close`.
///
/// # Por que so a cabeca, e o corpo fica com quem chama
///
/// O servidor do PhxSql responde texto e o PhxZipWeb responde bytes (a fonte,
/// o pacote). Devolver a cabeca e deixar o corpo com quem chama serve aos dois
/// sem copiar o corpo nem converter bytes em texto. A politica de seguranca
/// (CSP e irmaos) e de cada porta e entra pelos `extras`, cada linha ja
/// terminada em `\r\n`: o que e comum -- o `Content-Length` contado e a
/// conexao que fecha -- e o que mora aqui.
///
/// `Connection: close` sempre: uma resposta por conexao e o que as duas
/// portas fazem, e dizer isso poupa o cliente de esperar uma segunda.
pub fn montar_cabeca(codigo: u16, tipo: &str, tamanho: usize, extras: &str) -> String {
    format!(
        "HTTP/1.1 {codigo} {}\r\n\
         Content-Type: {tipo}\r\n\
         Content-Length: {tamanho}\r\n\
         {extras}\
         Connection: close\r\n\
         \r\n",
        frase_do_codigo(codigo)
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Write;
    use std::net::{Shutdown, TcpListener};

    const TETOS: Tetos = Tetos {
        cabecalho: 16 * 1024,
        corpo: 64 * 1024,
    };

    fn ler(bruto: &[u8]) -> PedidoLido {
        ler_pedido(bruto, TETOS)
    }

    fn pedido(bruto: &[u8]) -> Pedido {
        match ler(bruto) {
            PedidoLido::Pedido(p) => p,
            outro => panic!("esperava um pedido, veio {outro:?}"),
        }
    }

    /// RED: com o corpo passando por `String::from_utf8_lossy`, como o leitor
    /// do servidor fazia, o 0xFF vira `EF BF BD` e o tamanho muda.
    #[test]
    fn o_corpo_binario_chega_intacto() {
        let corpo: Vec<u8> = (0..=255u8)
            .chain([0xFF, 0x00, 0xFE, b'\r', b'\n'])
            .collect();
        let mut bruto = format!(
            "POST /api/compactar HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            corpo.len()
        )
        .into_bytes();
        bruto.extend_from_slice(&corpo);
        let p = pedido(&bruto);
        assert_eq!(p.corpo, corpo);
        assert_eq!(p.corpo[255], 0xFF);
        assert_eq!(p.metodo, "POST");
        assert_eq!(p.caminho, "/api/compactar");
    }

    #[test]
    fn caminho_consulta_e_cabecalho_sem_caixa() {
        let p = pedido(b"GET /idiomas?idioma=Ingles HTTP/1.1\r\nX-Sessao: abc\r\n\r\n");
        assert_eq!(p.caminho, "/idiomas");
        assert_eq!(p.consulta, "idioma=Ingles");
        assert_eq!(p.cabecalho("x-sessao"), Some("abc"));
        assert_eq!(p.cabecalho("X-SESSAO"), Some("abc"));
        assert!(p.corpo.is_empty());
    }

    #[test]
    fn content_length_acima_do_teto_recusa_sem_ler_o_corpo() {
        let bruto = format!(
            "POST /x HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            TETOS.corpo + 1
        );
        match ler(bruto.as_bytes()) {
            PedidoLido::GrandeDemais(e) => {
                assert_eq!(
                    e,
                    Excesso::Corpo {
                        declarado: TETOS.corpo as u64 + 1,
                        teto: TETOS.corpo
                    }
                );
                assert_eq!(e.codigo_http(), 413);
                assert!(e.motivo().contains("Content-Length"), "{}", e.motivo());
            }
            outro => panic!("esperava 413, veio {outro:?}"),
        }
    }

    /// O teto exato cabe: `>` e nao `>=`.
    #[test]
    fn o_corpo_no_teto_exato_cabe() {
        let mut bruto = format!(
            "POST /x HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            TETOS.corpo
        )
        .into_bytes();
        bruto.resize(bruto.len() + TETOS.corpo, 0xAB);
        assert_eq!(pedido(&bruto).corpo.len(), TETOS.corpo);
    }

    #[test]
    fn content_length_torto_ou_corpo_curto_e_nada() {
        assert!(matches!(
            ler(b"POST /x HTTP/1.1\r\nContent-Length: abc\r\n\r\n"),
            PedidoLido::Nada
        ));
        assert!(matches!(
            ler(b"POST /x HTTP/1.1\r\nContent-Length: -1\r\n\r\n"),
            PedidoLido::Nada
        ));
        assert!(matches!(
            ler(b"POST /x HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc"),
            PedidoLido::Nada
        ));
    }

    #[test]
    fn linha_e_cabecalho_acima_do_teto() {
        let mut longa = b"GET /".to_vec();
        longa.resize(TETOS.cabecalho + 10, b'a');
        assert!(matches!(
            ler(&longa),
            PedidoLido::GrandeDemais(Excesso::Linha { .. })
        ));
        let mut cab = b"GET / HTTP/1.1\r\nX: ".to_vec();
        cab.resize(cab.len() + TETOS.cabecalho, b'b');
        assert!(matches!(
            ler(&cab),
            PedidoLido::GrandeDemais(Excesso::LinhaDeCabecalho { .. })
        ));
        // Muitas linhas pequenas: cada uma cabe, a soma nao.
        let mut muitas = b"GET / HTTP/1.1\r\n".to_vec();
        for i in 0..2000 {
            muitas.extend_from_slice(format!("X-{i}: abcdefgh\r\n").as_bytes());
        }
        assert!(matches!(
            ler(&muitas),
            PedidoLido::GrandeDemais(Excesso::Cabecalho { .. })
        ));
    }

    /// O drenar para no teto: quem foi recusado nao faz este lado ler mais do
    /// que a porta decidiu.
    #[test]
    fn drenar_para_no_teto() {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let end = ouvinte.local_addr().unwrap();
        let cliente = std::thread::spawn(move || {
            let mut c = TcpStream::connect(end).unwrap();
            let _ = c.write_all(&vec![7u8; 100_000]);
            let _ = c.shutdown(Shutdown::Write);
            let mut fim = Vec::new();
            let _ = c.read_to_end(&mut fim);
        });
        let (mut s, _) = ouvinte.accept().unwrap();
        let lidos = drenar(&mut s, 10_000, Duration::from_millis(500));
        assert_eq!(lidos, 10_000);
        // E o resto continua drenavel: o teto parou a leitura, nao o fio.
        let resto = drenar(&mut s, u64::MAX, Duration::from_millis(500));
        assert_eq!(resto, 90_000);
        drop(s);
        cliente.join().unwrap();
    }
}
