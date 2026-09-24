//! Um cliente SMTP do tamanho do problema: mandar aviso para um rele.
//!
//! # Por que escrito aqui
//!
//! Pela mesma regra do resto do projeto -- so `std`. SMTP e um protocolo de
//! linhas de texto sobre TCP, e `TcpStream` mais `BufReader` dao conta. Nao ha
//! crate a acrescentar.
//!
//! # O limite honesto: nao ha TLS
//!
//! A `std` nao traz TLS, e sem crate nao ha como falar `STARTTLS` nem a porta
//! 465. Entao este cliente conversa em TEXTO CLARO, e isso decide para quem
//! ele serve:
//!
//! - **Serve** para um rele que voce controla -- `postfix`, `exim` ou o
//!   servidor de e-mail da empresa -- na porta 25 da rede interna. Ele recebe
//!   em texto claro e cuida do TLS para fora.
//! - **Nao serve** para entregar direto num provedor publico, que exige TLS.
//!
//! Se `usuario` e `senha` estiverem preenchidos, o `AUTH LOGIN` manda os dois
//! em base64 -- que e codificacao, nao cifra, e qualquer um no caminho le. Por
//! isso o conselho no `config.json` e liberar o IP no rele em vez de mandar
//! senha.
//!
//! # Acento no cabecalho, e o corpo em base64
//!
//! Cabecalho de e-mail e ASCII por definicao (RFC 5322). Um assunto com `ç`
//! passou cru na primeira versao -- um rele moderno costuma aceitar, um rele
//! rigoroso embaralha ou recusa. Agora o assunto sai em palavra codificada da
//! RFC 2047 quando tem acento, e passa direto quando nao tem (assunto legivel
//! no log do rele vale mais do que uniformidade).
//!
//! O corpo vai em base64, com `Content-Transfer-Encoding: base64`. UTF-8 cru
//! seria 8 bits declarado como 7, e um rele sem `8BITMIME` teria licenca para
//! cortar o oitavo bit -- o acento chegaria trocado. Base64 e feio de ler no
//! log e chega igual em qualquer rele.
//!
//! # Injecao de cabecalho
//!
//! Cabecalho de e-mail termina em CRLF, e um assunto com quebra de linha
//! deixaria quem escreve o `config.json` inventar cabecalho -- um `Bcc:` a
//! mais, por exemplo. Toda linha que entra na mensagem passa por
//! [`uma_linha_so`].

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use phxsql_core::base64;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::fio::{Canal, Recebido, TETO_DO_APERTO};

use crate::config::Email;

/// Teto de linhas de CONTINUACAO (`250-...`) que uma resposta aceita --
/// pedido 463.
///
/// Nao e teto de TEMPO: e teto de QUANTAS. Um rele que nunca manda a linha
/// final (o espaco no lugar do hifen) prende a thread para sempre em silencio
/// curto entre linhas, e o `timeout_s` so mede o silencio -- nunca a
/// conversa inteira. A maior resposta legitima observada e o `EHLO` de um MTA
/// cheio de extensoes, e nem essa passa de poucas dezenas de linhas; mil e
/// folga suficiente para nunca recusar um rele de verdade.
const TETO_DE_LINHAS_DE_CONTINUACAO: usize = 1000;

/// Entrega uma mensagem pelo rele configurado.
///
/// Devolve a ultima resposta do servidor quando dá certo -- ela costuma trazer
/// o identificador da fila, que e o que se procura no log do rele depois.
pub fn enviar(cfg: &Email, assunto: &str, corpo: &str) -> Result<String> {
    if !cfg.ligado {
        return Err(PhxError::Esquema("alertas.email.ligado esta falso".into()));
    }
    if cfg.para.is_empty() {
        return Err(PhxError::Esquema("alertas.email sem destinatario".into()));
    }
    // A senha ANTES de ir a rede, e so quando ha login: a `senha_env` que
    // falta e erro que se sabe aqui, nomeando a variavel -- ir ao rele para
    // ouvir «535 authentication failed» mandaria procurar a senha no rele
    // (pedido 372).
    let senha = if cfg.usuario.is_empty() {
        ""
    } else {
        cfg.senha()?
    };
    let alvo = format!("{}:{}", cfg.servidor, cfg.porta);
    let espera = Duration::from_secs(cfg.timeout_s);
    let fluxo = conectar(&alvo, espera)?;
    fluxo
        .set_read_timeout(Some(espera))
        .and_then(|_| fluxo.set_write_timeout(Some(espera)))
        .map_err(|e| PhxError::Esquema(format!("smtp: nao consegui armar o timeout: {e}")))?;

    let mut sessao = Sessao {
        leitor: BufReader::new(
            fluxo
                .try_clone()
                .map_err(|e| PhxError::Esquema(format!("smtp: nao consegui duplicar: {e}")))?,
        ),
        escrita: fluxo,
    };

    sessao.esperar(&[220])?;
    // EHLO primeiro: e o que anuncia AUTH. Rele antigo so entende HELO, e
    // insistir no EHLO faria o envio falhar em servidor que funciona.
    if sessao
        .comando(&format!("EHLO {}", nome_da_maquina()), &[250])
        .is_err()
    {
        sessao.comando(&format!("HELO {}", nome_da_maquina()), &[250])?;
    }

    if !cfg.usuario.is_empty() {
        sessao.comando("AUTH LOGIN", &[334])?;
        sessao.comando(&base64::codificar(cfg.usuario.as_bytes()), &[334])?;
        // A senha entra aqui e em lugar nenhum mais: o erro devolvido por
        // `esperar` traz a resposta do SERVIDOR, nunca o que foi enviado.
        sessao.comando(&base64::codificar(senha.as_bytes()), &[235])?;
    }

    sessao.comando(&format!("MAIL FROM:<{}>", uma_linha_so(&cfg.de)?), &[250])?;
    for destino in &cfg.para {
        sessao.comando(
            &format!("RCPT TO:<{}>", uma_linha_so(destino)?),
            &[250, 251],
        )?;
    }
    sessao.comando("DATA", &[354])?;
    sessao.cru(&mensagem(cfg, assunto, corpo)?)?;
    let recibo = sessao.comando(".", &[250])?;
    // O QUIT e cortesia: se falhar, a mensagem ja foi aceita.
    let _ = sessao.comando("QUIT", &[221]);
    Ok(recibo)
}

/// Conecta com timeout. `TcpStream::connect` sozinho pode ficar minutos
/// pendurado num host que nao responde, e o relogio de alerta chama isto de
/// dentro de uma thread que tem mais o que fazer.
fn conectar(alvo: &str, espera: Duration) -> Result<TcpStream> {
    use std::net::ToSocketAddrs;
    let mut ultimo = String::from("nenhum endereco resolvido");
    let enderecos = alvo
        .to_socket_addrs()
        .map_err(|e| PhxError::Esquema(format!("smtp: nao resolvi {alvo}: {e}")))?;
    for endereco in enderecos {
        match TcpStream::connect_timeout(&endereco, espera) {
            Ok(s) => return Ok(s),
            Err(e) => ultimo = e.to_string(),
        }
    }
    Err(PhxError::Esquema(format!(
        "smtp: nao conectei em {alvo}: {ultimo}"
    )))
}

struct Sessao {
    leitor: BufReader<TcpStream>,
    escrita: TcpStream,
}

impl Sessao {
    fn cru(&mut self, texto: &str) -> Result<()> {
        self.escrita
            .write_all(texto.as_bytes())
            .and_then(|_| self.escrita.flush())
            .map_err(|e| PhxError::Esquema(format!("smtp: escrita falhou: {e}")))
    }

    fn comando(&mut self, linha: &str, esperados: &[u16]) -> Result<String> {
        self.cru(&format!("{linha}\r\n"))?;
        self.esperar(esperados)
    }

    fn esperar(&mut self, esperados: &[u16]) -> Result<String> {
        ler_resposta(&mut self.leitor, esperados)
    }
}

/// Le a resposta e confere o codigo.
///
/// Resposta de SMTP pode vir em varias linhas: as intermediarias tem um
/// hifen depois do numero (`250-AUTH LOGIN`) e a ultima um espaco
/// (`250 OK`). Parar na primeira deixaria o resto no soquete e jogaria
/// todo o dialogo seguinte fora de sincronia.
///
/// # A linha vem do MOTOR, com teto -- pedido 439
///
/// Este era o sexto `read_line` de soquete fora do [`Canal`], e o unico em
/// codigo de producao depois do 434: teto de TEMPO (o `timeout_s`) e nenhum
/// de TAMANHO. Medido com um rele falso que nao quebra a linha, o cliente
/// tirava do soquete a oferta inteira -- 8.388.610 bytes numa linha so -- e
/// so parava quando o rele parava. Um rele comprometido, ou quem esta no meio
/// de um SMTP em claro, escolhia quanta memoria este lado reservava, que e a
/// definicao do defeito do 434.
///
/// O conserto nao e um teto pendurado ao lado deste `read_line`: e ler pelo
/// mesmo `ler_ate` que protege a porta de dados e a web. SMTP nao fala o
/// protocolo do `Canal`, e o HTTP tambem nao -- a pergunta que o motor
/// responde nao e «que protocolo», e «quanto eu reservo numa linha que vem do
/// soquete». [`Canal::Claro`] e exatamente o que esta conexao e.
///
/// E o teto e o [`TETO_DO_APERTO`] que ja existia, e nao uma constante nova:
/// ele responde pela linha lida de quem nao provou quem e, e o rele nunca
/// prova -- nao ha TLS aqui (ver o topo do arquivo). A maior resposta legitima
/// e a da RFC 5321, secao 4.5.3.1.5: 512 octetos por linha, codigo e CRLF
/// inclusive. Sessenta e quatro KiB sao 128 vezes isso, e a folga e para o
/// rele que nao segue a norma a letra.
///
/// # O laco das linhas de continuacao tinha teto de TAMANHO e nao de
/// QUANTAS -- pedido 463
///
/// Cada linha e solta antes da seguinte, entao o laco nao guardava memoria:
/// gastava TEMPO, e o `timeout_s` so responde por metade dessa pergunta,
/// porque ele mede o SILENCIO entre bytes. Um rele que manda uma linha de
/// continuacao (`250-...`) por vez, devagar mas sem nunca ficar quieto o
/// bastante para estourar o silencio, prende esta thread de aviso para
/// sempre sem nunca alocar mais que uma linha. `TETO_DE_LINHAS_DE_CONTINUACAO`
/// fecha isso contando QUANTAS, e nao QUANTO TEMPO.
fn ler_resposta<L: BufRead>(leitor: &mut L, esperados: &[u16]) -> Result<String> {
    let mut fio = Canal::Claro;
    let mut continuacoes = 0usize;
    let ultima = loop {
        let linha = match fio.ler_ate(leitor, TETO_DO_APERTO) {
            Ok(Recebido::Linha(l)) => l,
            Ok(Recebido::Fim) => {
                return Err(PhxError::Esquema(
                    "smtp: o servidor fechou a conexao no meio da resposta".into(),
                ))
            }
            // O teto vira erro do SMTP, com o nome do lado que o estourou --
            // e continua `LimiteExcedido`, que e o que ele e.
            Err(PhxError::LimiteExcedido(m)) => {
                return Err(PhxError::LimiteExcedido(format!("smtp: {m}")))
            }
            Err(PhxError::Io(e)) => {
                return Err(PhxError::Esquema(format!("smtp: leitura falhou: {e}")))
            }
            Err(outro) => return Err(outro),
        };
        let limpa = linha.trim_end().to_string();
        if limpa.as_bytes().get(3) != Some(&b'-') {
            break limpa;
        }
        continuacoes += 1;
        if continuacoes > TETO_DE_LINHAS_DE_CONTINUACAO {
            return Err(PhxError::LimiteExcedido(format!(
                "smtp: a resposta passou de {TETO_DE_LINHAS_DE_CONTINUACAO} linhas de \
                 continuacao sem fechar"
            )));
        }
    };
    let codigo: u16 = ultima
        .get(..3)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| PhxError::Esquema(format!("smtp: resposta sem codigo: {ultima:?}")))?;
    if esperados.contains(&codigo) {
        Ok(ultima)
    } else {
        Err(PhxError::Esquema(format!("smtp recusou: {ultima}")))
    }
}

/// Monta o corpo RFC 5322 ja pronto para o `DATA`.
pub fn mensagem(cfg: &Email, assunto: &str, corpo: &str) -> Result<String> {
    let mut m = String::new();
    m.push_str(&format!("From: {}\r\n", uma_linha_so(&cfg.de)?));
    m.push_str(&format!("To: {}\r\n", uma_linha_so(&cfg.para.join(", "))?));
    m.push_str(&format!(
        "Subject: {}\r\n",
        palavra_codificada(uma_linha_so(assunto)?)
    ));
    m.push_str(&format!("Date: {}\r\n", data_rfc5322(crate::agora_ms())));
    m.push_str("MIME-Version: 1.0\r\n");
    m.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    m.push_str("Content-Transfer-Encoding: base64\r\n");
    m.push_str("X-Mailer: PhxSql\r\n");
    m.push_str("\r\n");
    // Base64 nao produz ponto no comeco de linha, entao o "dot stuffing" que
    // o DATA exigiria nao tem o que escapar.
    for pedaco in quebrar(&base64::codificar(corpo.as_bytes()), 76) {
        m.push_str(&pedaco);
        m.push_str("\r\n");
    }
    Ok(m)
}

/// Um cabecalho com acento, na palavra codificada da RFC 2047.
///
/// So quando precisa: assunto em ASCII passa inteiro, e continua legivel no
/// log do rele e em qualquer cliente antigo.
fn palavra_codificada(texto: &str) -> String {
    if texto.is_ascii() {
        return texto.to_string();
    }
    format!("=?UTF-8?B?{}?=", base64::codificar(texto.as_bytes()))
}

/// Quebra em linhas de no maximo `n`. O RFC 2045 para em 76 colunas.
fn quebrar(texto: &str, n: usize) -> Vec<String> {
    if texto.is_empty() {
        return vec![String::new()];
    }
    texto
        .as_bytes()
        .chunks(n)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect()
}

/// Recusa texto que atravessaria linhas -- ver a nota de injecao no topo.
fn uma_linha_so(texto: &str) -> Result<&str> {
    if texto.contains(['\r', '\n']) {
        return Err(PhxError::Esquema(format!(
            "texto de cabecalho com quebra de linha: {texto:?}"
        )));
    }
    Ok(texto)
}

/// Data no formato que o cabecalho `Date:` exige.
///
/// Sempre em `+0000`: o relogio do projeto conta em UTC, e inventar fuso seria
/// carimbar hora errada com cara de certa.
pub fn data_rfc5322(ms: i64) -> String {
    const DIAS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MESES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let dias = ms.div_euclid(86_400_000) as i32;
    let resto = ms.rem_euclid(86_400_000);
    let (ano, mes, dia) = phxsql_core::datahora::civil_de_dias(dias);
    // 1970-01-01 foi quinta-feira, e por isso o vetor comeca em "Thu".
    let semana = DIAS[dias.rem_euclid(7) as usize];
    let (h, m, s) = (
        resto / 3_600_000,
        (resto / 60_000) % 60,
        (resto / 1_000) % 60,
    );
    format!(
        "{semana}, {dia:02} {} {ano} {h:02}:{m:02}:{s:02} +0000",
        MESES[(mes as usize).saturating_sub(1).min(11)]
    )
}

/// Nome desta maquina para o EHLO. Cai num literal quando nao da para saber:
/// rele nenhum recusa por causa do EHLO, e travar o alerta por isso seria
/// perder o aviso justamente quando ele importa.
pub(crate) fn nome_da_maquina() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.contains(['\r', '\n', ' ']))
        .unwrap_or_else(|| "phxsql".to_string())
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::json::Json;

    fn cfg() -> Email {
        let j = Json::analisar(
            r#"{"alertas":{"ligado":true,"email":{"ligado":true,
                "servidor":"127.0.0.1","de":"phx@exemplo.com",
                "para":["a@exemplo.com","b@exemplo.com"]}}}"#,
        )
        .unwrap();
        crate::config::Config::de_json(&j).unwrap().alertas.email
    }

    #[test]
    fn o_cabecalho_sai_completo() {
        let m = mensagem(&cfg(), "disco apertado", "linha 1\nlinha 2").unwrap();
        assert!(m.contains("From: phx@exemplo.com\r\n"));
        assert!(m.contains("To: a@exemplo.com, b@exemplo.com\r\n"));
        // Assunto sem acento passa inteiro: legivel no log do rele.
        assert!(m.contains("Subject: disco apertado\r\n"));
        let corpo = m.split("\r\n\r\n").nth(1).unwrap();
        assert_eq!(
            phxsql_core::base64::decodificar_texto(corpo.trim()).unwrap(),
            "linha 1\nlinha 2"
        );
    }

    /// Cabecalho e ASCII por definicao. Um assunto com acento passou cru na
    /// primeira versao, e um rele rigoroso o embaralharia.
    #[test]
    fn assunto_com_acento_vira_palavra_codificada() {
        let m = mensagem(&cfg(), "espaço em disco", "corpo").unwrap();
        let linha = m
            .lines()
            .find(|l| l.starts_with("Subject:"))
            .unwrap()
            .to_string();
        assert!(linha.is_ascii(), "cabecalho com byte alto: {linha:?}");
        assert!(linha.starts_with("Subject: =?UTF-8?B?"), "{linha}");
        let dentro = linha
            .trim_start_matches("Subject: =?UTF-8?B?")
            .trim_end_matches("?=");
        assert_eq!(
            phxsql_core::base64::decodificar_texto(dentro).unwrap(),
            "espaço em disco"
        );
    }

    /// UTF-8 cru seria 8 bits declarado como 7: um rele sem 8BITMIME teria
    /// licenca para cortar o oitavo bit, e o acento chegaria trocado.
    #[test]
    fn o_corpo_atravessa_rele_de_sete_bits() {
        let m = mensagem(&cfg(), "x", "acentuação e ç no corpo").unwrap();
        assert!(m.contains("Content-Transfer-Encoding: base64\r\n"));
        assert!(m.is_ascii(), "a mensagem inteira tem de ser ASCII");
        let corpo = m.split("\r\n\r\n").nth(1).unwrap().replace("\r\n", "");
        assert_eq!(
            phxsql_core::base64::decodificar_texto(&corpo).unwrap(),
            "acentuação e ç no corpo"
        );
    }

    /// O RFC 2045 para em 76 colunas.
    #[test]
    fn a_linha_de_base64_nao_passa_de_setenta_e_seis() {
        let m = mensagem(&cfg(), "x", &"a".repeat(5_000)).unwrap();
        for l in m.lines() {
            assert!(l.len() <= 78, "linha de {} bytes: {l:?}", l.len());
        }
    }

    #[test]
    fn assunto_com_quebra_de_linha_nao_vira_cabecalho() {
        // O ataque: quem escreve o assunto acrescenta um destinatario oculto.
        let e = mensagem(&cfg(), "oi\r\nBcc: ladrao@fora.com", "corpo");
        assert!(e.is_err(), "assunto com CRLF passou");
    }

    /// O corpo em base64 nunca produz linha comecando com ponto, entao a
    /// linha que encerraria a mensagem cedo nao existe.
    #[test]
    fn nenhuma_linha_do_corpo_comeca_com_ponto() {
        let m = mensagem(&cfg(), "x", ".\n..\n.fim").unwrap();
        let corpo = m.split("\r\n\r\n").nth(1).unwrap();
        for l in corpo.lines() {
            assert!(!l.starts_with('.'), "linha com ponto no inicio: {l:?}");
        }
        assert_eq!(
            phxsql_core::base64::decodificar_texto(&corpo.replace("\r\n", "")).unwrap(),
            ".\n..\n.fim"
        );
    }

    #[test]
    fn a_data_bate_com_o_calendario() {
        // 2024-02-29T12:24:56Z -- ano bissexto, para pegar erro de calendario.
        assert_eq!(
            data_rfc5322(1_709_209_496_000),
            "Thu, 29 Feb 2024 12:24:56 +0000"
        );
        assert_eq!(data_rfc5322(0), "Thu, 01 Jan 1970 00:00:00 +0000");
    }

    #[test]
    fn desligado_nao_tenta_conectar() {
        let mut c = cfg();
        c.ligado = false;
        assert!(enviar(&c, "x", "y").is_err());
    }

    // -------------------------------------------------- pedido 439, o teto

    /// Quanto a linha que o rele manda pode ter antes de o cliente desistir.
    ///
    /// 8 MiB: 128 vezes o teto do motor, e 16.384 vezes a maior linha que a
    /// RFC 5321 (secao 4.5.3.1.5) permite a uma resposta, 512 octetos. A
    /// folga separa «parou no teto» de «leu tudo» sem gastar mais memoria do
    /// que a bateria precisa.
    const OFERTA: usize = 8 * 1024 * 1024;

    /// Um leitor que conta o que tirou do SOQUETE -- e o numero do dano.
    ///
    /// O veredito sozinho nao prova o conserto: um cliente que lesse a linha
    /// inteira e so depois recusasse daria o mesmo `Err`, com a memoria ja
    /// reservada. O que se mede e quanto saiu do soquete para dentro deste
    /// processo.
    struct Contador<R> {
        dentro: R,
        lidos: u64,
    }

    impl<R: std::io::Read> std::io::Read for Contador<R> {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            let n = self.dentro.read(b)?;
            self.lidos += n as u64;
            Ok(n)
        }
    }

    /// Um rele que manda `OFERTA` bytes de digito SEM quebra de linha, depois
    /// a quebra, e segura o soquete ate o outro lado fechar -- para o fim da
    /// linha nao chegar por EOF, que mediria outra coisa.
    ///
    /// O digito e de proposito: `222...` tem codigo de tres digitos, e um
    /// cliente sem teto chega ao fim, acha o codigo e recusa com a linha
    /// INTEIRA dentro da mensagem de erro.
    fn rele_que_nao_quebra_a_linha() -> u16 {
        use std::io::{Read, Write};
        let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = ouvinte.accept() else {
                return;
            };
            let bloco = vec![b'2'; 64 * 1024];
            let mut mandados = 0;
            while mandados < OFERTA {
                if s.write_all(&bloco).is_err() {
                    return;
                }
                mandados += bloco.len();
            }
            let _ = s.write_all(b"\r\n");
            let mut resto = [0u8; 64];
            while matches!(s.read(&mut resto), Ok(n) if n > 0) {}
        });
        porta
    }

    /// **A linha sem fim do rele para no teto do MOTOR -- pedido 439.**
    ///
    /// Medido antes do conserto, por este teste: o cliente tirava do soquete
    /// a oferta inteira (8.388.610 bytes) numa linha so, e a teria tirado ate
    /// a memoria acabar se o rele nao parasse. Com o `ler_ate` do motor ele
    /// tira o teto mais um, mais o que o `BufReader` ja tinha no buffer.
    #[test]
    fn a_linha_sem_fim_do_rele_para_no_teto_do_motor() {
        use phxsql_core::fio::TETO_DO_APERTO;
        let porta = rele_que_nao_quebra_a_linha();
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let mut leitor = BufReader::new(Contador {
            dentro: fluxo,
            lidos: 0,
        });
        let r = ler_resposta(&mut leitor, &[220]);
        let lidos = leitor.get_ref().lidos;
        eprintln!("o cliente SMTP tirou {lidos} bytes do soquete numa linha de {OFERTA}");
        // O teto, o byte que prova o estouro e um buffer do `BufReader`.
        let limite = TETO_DO_APERTO + 1 + 8 * 1024;
        assert!(
            lidos <= limite,
            "o cliente SMTP tirou {lidos} bytes do soquete numa linha so, contra \
             {limite} de teto: quem escolhe a memoria deste lado e o rele"
        );
        match r {
            Err(PhxError::LimiteExcedido(m)) => {
                assert!(m.starts_with("smtp:"), "{m}");
                assert!(m.contains(&TETO_DO_APERTO.to_string()), "{m}");
            }
            outro => panic!("a linha acima do teto nao foi recusada pelo teto: {outro:?}"),
        }
    }

    /// **E o envio inteiro desiste pelo mesmo teto, sem carregar a linha.**
    ///
    /// E o caminho de verdade (`enviar`), contra o mesmo rele: o erro que ele
    /// devolve vai ao `eprintln!` do alerta e a resposta do `email_testar`, e
    /// com a linha inteira dentro ele era um segundo lugar onde os 8 MiB
    /// moravam.
    #[test]
    fn o_envio_contra_rele_sem_quebra_de_linha_desiste_no_teto() {
        let mut c = cfg();
        c.porta = rele_que_nao_quebra_a_linha();
        c.timeout_s = 20;
        let e = enviar(&c, "x", "y").unwrap_err();
        let texto = e.to_string();
        eprintln!("o erro do envio tem {} bytes: {texto}", texto.len());
        assert!(
            texto.len() < 1024,
            "o erro do envio carrega {} bytes -- a linha do rele inteira",
            texto.len()
        );
        assert!(matches!(e, PhxError::LimiteExcedido(_)), "{texto}");
    }

    /// **O comportamento VELHO: resposta de varias linhas continua lida
    /// inteira** -- a do `EHLO`, que anuncia o `AUTH`. Um teto que cortasse a
    /// continuacao deixaria o resto no soquete e o dialogo fora de sincronia.
    #[test]
    fn a_resposta_de_varias_linhas_continua_inteira() {
        let bruto =
            b"250-rele.exemplo\r\n250-SIZE 10240000\r\n250-AUTH LOGIN\r\n250 OK\r\n220 x\r\n";
        let mut leitor = BufReader::new(&bruto[..]);
        assert_eq!(ler_resposta(&mut leitor, &[250]).unwrap(), "250 OK");
        assert_eq!(ler_resposta(&mut leitor, &[220]).unwrap(), "220 x");
    }

    // -------------------------------------------------- pedido 463, o teto

    /// Um rele que nunca fecha a resposta: so manda `250-x` para sempre, cada
    /// linha curta e bem formada -- nada que o teto de TAMANHO (pedido 439)
    /// alcance. O que depende do sistema operacional se prova contra o
    /// sistema operacional, e por isso e um soquete de verdade e nao um
    /// `&[u8]` estatico: o ataque e sobre TEMPO de thread, nao sobre bytes
    /// parados num buffer.
    fn rele_que_nunca_fecha_a_resposta() -> u16 {
        use std::io::Write as _;
        let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = ouvinte.accept() else {
                return;
            };
            // Bem mais que o teto: se o cliente nao parar sozinho, este laco
            // segura a thread do teste, e nao so a do cliente.
            for _ in 0..(TETO_DE_LINHAS_DE_CONTINUACAO * 2) {
                if s.write_all(b"250-x\r\n").is_err() {
                    return;
                }
            }
        });
        porta
    }

    /// **A resposta sem fim para no teto de QUANTAS -- pedido 463.**
    ///
    /// Antes do teto este teste travava: o laco de `ler_resposta` nunca
    /// achava a linha final e a thread do teste ficava presa atras do
    /// `read_timeout` de 20 s vezes `TETO_DE_LINHAS_DE_CONTINUACAO * 2`
    /// linhas -- na pratica, sem fim nenhum para este teste. Com o teto, a
    /// recusa chega bem antes de o rele falso acabar de mandar linha.
    #[test]
    fn a_resposta_sem_fim_para_no_teto_de_quantas() {
        let porta = rele_que_nunca_fecha_a_resposta();
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let mut leitor = BufReader::new(fluxo);
        match ler_resposta(&mut leitor, &[220]) {
            Err(PhxError::LimiteExcedido(m)) => {
                assert!(m.starts_with("smtp:"), "{m}");
                assert!(
                    m.contains(&TETO_DE_LINHAS_DE_CONTINUACAO.to_string()),
                    "{m}"
                );
            }
            outro => panic!("a resposta sem fim nao foi recusada pelo teto: {outro:?}"),
        }
    }
}
