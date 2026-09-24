//! Apoio aos testes UNITARIOS do `phxsql-server`.
//!
//! Copia deliberada do `phxsql-store::apoio_teste`: sao crates diferentes, e
//! exportar um helper `#[cfg(test)]` de um para o outro exigiria uma dependencia
//! de dev so para isso. O texto e curto e as duas provas viajam junto.
//!
//! So para testes: `#[cfg(test)]` no `lib.rs` tira isso do binario de producao.
//! Pedido 150 -- a bateria nao limpava o que criava. O padrao velho era um
//! `rm` que nunca rodava (helper devolvia so o `PathBuf`, sem guarda), entao
//! um teste que falhava no meio deixava o diretorio para tras -- e falhar no
//! meio e o normal de um teste de asserção. `DirTemp` apaga no `Drop`, que o
//! Rust roda mesmo durante um panic (unwind), e nao so no fim feliz do corpo.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Diretorio temporario exclusivo, removido no `Drop`.
pub struct DirTemp(pub PathBuf);

impl DirTemp {
    /// `rotulo` identifica o teste/uso no nome do diretorio (depuracao); a
    /// unicidade real vem do PID do processo de teste e de um contador.
    pub fn novo(rotulo: &str) -> DirTemp {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("phxsrv-ut-{}-{rotulo}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        DirTemp(p)
    }
}

impl Drop for DirTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Deref para `Path`: converte um helper local `fn dir(...) -> PathBuf` para
// `-> DirTemp` sem tocar cada chamada no corpo dos testes -- `&d`, `d.join`,
// `d.display()` continuam compilando pelo mesmo caminho de sempre.
impl std::ops::Deref for DirTemp {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

// `AsRef<Path>`: o Deref acima nao basta para `fn criar(d: impl AsRef<Path>)`
// -- coercao de Deref so vale quando o alvo e' um `&Path` explicito na
// assinatura, nao para satisfazer um bound generico como `impl AsRef<Path>`.
impl AsRef<Path> for DirTemp {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// Um SMTP falso do tamanho do que o `email.rs` fala: 220/250/354/250/221.
///
/// Existe porque teste unitario NAO prova entrega de e-mail -- soquete
/// prova, e e a mesma licao do `BULKINSERT`. Devolve a porta efemera e o
/// canal por onde cada mensagem recebida chega inteira (o `DATA`, com os
/// cabecalhos). Morava no modulo de testes da politica; veio para ca quando a
/// saude do disco (pedido 249) precisou do mesmo rele.
pub fn rele_falso() -> (u16, std::sync::mpsc::Receiver<String>) {
    use std::io::{BufRead, BufReader, Write};
    let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let (envia, recebe) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for fluxo in ouvinte.incoming() {
            let Ok(fluxo) = fluxo else { return };
            let envia = envia.clone();
            std::thread::spawn(move || {
                let Ok(mut escrita) = fluxo.try_clone() else {
                    return;
                };
                let mut leitor = BufReader::new(fluxo);
                let _ = escrita.write_all(b"220 rele-falso\r\n");
                let mut linha = String::new();
                while leitor.read_line(&mut linha).unwrap_or(0) > 0 {
                    let comando = linha.trim_end().to_uppercase();
                    linha.clear();
                    if comando == "DATA" {
                        let _ = escrita.write_all(b"354 manda\r\n");
                        let mut corpo = String::new();
                        let mut l = String::new();
                        while leitor.read_line(&mut l).unwrap_or(0) > 0 {
                            if l.trim_end() == "." {
                                break;
                            }
                            corpo.push_str(&l);
                            l.clear();
                        }
                        let _ = envia.send(corpo);
                        let _ = escrita.write_all(b"250 OK fila-1\r\n");
                    } else if comando == "QUIT" {
                        let _ = escrita.write_all(b"221 tchau\r\n");
                        return;
                    } else {
                        let _ = escrita.write_all(b"250 OK\r\n");
                    }
                }
            });
        }
    });
    (porta, recebe)
}

/// O cliente de teste da porta de dados: UMA linha de pedido, UMA de resposta.
///
/// Existe porque os testes do `servidor.rs` tinham um cliente cada um --
/// leituras de linha escritas a mao no mesmo arquivo, cada uma com o seu
/// prazo e o seu jeito de dizer «a conexao caiu» (pedido 451, a lei «funcao e
/// comando nao se duplicam»). O `conferidor_canal` conta essas leituras, e a
/// do cliente passa a ser uma so, aqui.
///
/// «A conexao caiu sem responder» e `None`, e nao panico: e exatamente o dano
/// que metade destes testes mede -- o panico dentro do `atender`, a recusa
/// acima do teto, a queda do processo filho.
pub struct Ligacao {
    escrita: std::net::TcpStream,
    leitor: std::io::BufReader<std::net::TcpStream>,
}

impl Ligacao {
    /// O prazo de leitura de quem nao pede outro: folgado, porque quem espera
    /// a resposta e a propria prova, e um prazo curto numa maquina carregada
    /// viraria teste que floca.
    pub const PRAZO: std::time::Duration = std::time::Duration::from_secs(20);

    /// Conecta, ou `None` se a porta recusar -- o servidor que ja caiu.
    pub fn tentar(porta: u16) -> Option<Ligacao> {
        Ligacao::tentar_com_prazo(porta, Ligacao::PRAZO)
    }

    /// O mesmo, com o prazo de leitura dito por quem chama.
    pub fn tentar_com_prazo(porta: u16, prazo: std::time::Duration) -> Option<Ligacao> {
        let fluxo = std::net::TcpStream::connect(("127.0.0.1", porta)).ok()?;
        fluxo.set_read_timeout(Some(prazo)).ok()?;
        Some(Ligacao {
            escrita: fluxo.try_clone().ok()?,
            leitor: std::io::BufReader::new(fluxo),
        })
    }

    /// Conecta, e o teste para se a porta recusar.
    pub fn nova(porta: u16) -> Ligacao {
        Ligacao::tentar(porta)
            .unwrap_or_else(|| panic!("a porta de dados {porta} recusou a conexao"))
    }

    /// Manda `linha` (sem a quebra) e devolve a resposta, sem a quebra. `None`
    /// quando a conexao caiu, recusou a escrita ou fechou sem responder.
    pub fn pedir(&mut self, linha: &str) -> Option<String> {
        use std::io::{BufRead, Write};
        writeln!(self.escrita, "{linha}").ok()?;
        let mut resposta = String::new();
        match self.leitor.read_line(&mut resposta) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(resposta.trim_end().to_string()),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Prova real do pedido 150, nos dois sentidos: um `rm` no fim do corpo
    /// NUNCA rodaria aqui, porque o teste nunca chega ao fim -- e falhar no
    /// meio, nao no fim, e o caso comum de um teste de asserção. Isso so
    /// prova o `Drop` se o teste FALHA de propósito e o diretorio some mesmo
    /// assim; sem o panico de dentro, o teste provaria so o caminho feliz,
    /// que o helper velho ja cobria.
    #[test]
    fn falha_no_meio_do_teste_ainda_assim_limpa_o_diretorio() {
        let guarda = DirTemp::novo("prova-panico");
        let caminho = guarda.0.clone();
        assert!(
            caminho.is_dir(),
            "o guarda tem de criar o diretorio na hora"
        );

        // O `DirTemp` e' movido para dentro do closure: quando o panic
        // desenrola este quadro, o `Drop` dele roda ali, antes de o
        // `catch_unwind` devolver o erro para fora.
        let resultado = std::panic::catch_unwind(move || {
            let _preso_no_escopo_que_vai_falhar = guarda;
            panic!("falha proposital, so para testar que o Drop roda mesmo assim");
        });

        assert!(resultado.is_err(), "o panico tinha de propagar ate aqui");
        assert!(
            !caminho.exists(),
            "o Drop tinha de ter apagado o diretorio durante o desenrolamento do panic -- \
             e nao apagou, o que e exatamente o defeito que o pedido 150 descreve"
        );
    }

    #[test]
    fn caminho_feliz_tambem_limpa() {
        let caminho = {
            let guarda = DirTemp::novo("prova-feliz");
            let c = guarda.0.clone();
            assert!(c.is_dir());
            c
            // `guarda` sai de escopo aqui, no fim do bloco -- Drop normal.
        };
        assert!(!caminho.exists());
    }
}
