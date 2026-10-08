//! Um cliente SMTP do tamanho do problema: mandar aviso para um rele.
//!
//! # Por que escrito aqui
//!
//! Pela mesma regra do resto do projeto -- so `std`. SMTP e um protocolo de
//! linhas de texto sobre TCP, e `TcpStream` mais `BufReader` dao conta. Nao ha
//! crate a acrescentar.
//!
//! # TLS com o rele -- pedido, nao imposto (pedido 572, T6d)
//!
//! Ate a 0.19 este cliente so falava em TEXTO CLARO: a `std` nao traz TLS. O
//! TLS 1.3 escrito nesta casa mudou isso. Com `"tls": "exigir"` ou
//! `"verificar"` em `alertas.email`, a conversa passa a TLS pelo `STARTTLS`
//! (RFC 3207) -- ou desde o primeiro byte na porta 465 ou com
//! `"tls_implicito": true` (RFC 8314). O rele que nao anuncia `STARTTLS` faz
//! o envio parar antes do remetente: quem pediu TLS nao cai para o claro.
//!
//! Sem `tls`, tudo segue como era -- serve para o rele interno que voce
//! controla, e o `AUTH LOGIN` em base64 continua legivel no fio: com senha,
//! ligue o TLS.

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

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use phxsql_core::base64;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::fio::{Canal, Recebido, TETO_DO_APERTO};

use crate::config::Email;
use crate::prazo::{self, ComPrazo, Prazo, Rotulo};
use phxsql_core::tls::FioDeCliente;

/// Teto de linhas de CONTINUACAO (`250-...`) que uma resposta aceita --
/// pedido 463.
///
/// Nao e teto de TEMPO: e teto de QUANTAS. Um rele que nunca manda a linha
/// final (o espaco no lugar do hifen) prendia a thread para sempre em
/// silencio curto entre linhas, porque o `timeout_s` so mede o silencio. O
/// TEMPO da conversa inteira e a outra metade do mesmo pedido, e tem prazo
/// proprio -- ver [`enviar_com`]. A maior resposta legitima observada e o
/// `EHLO` de um MTA cheio de extensoes, e nem essa passa de poucas dezenas de
/// linhas; mil e folga suficiente para nunca recusar um rele de verdade.
const TETO_DE_LINHAS_DE_CONTINUACAO: usize = 1000;

/// Entrega uma mensagem pelo rele configurado.
///
/// Devolve a ultima resposta do servidor quando dá certo -- ela costuma trazer
/// o identificador da fila, que e o que se procura no log do rele depois.
pub fn enviar(cfg: &Email, assunto: &str, corpo: &str) -> Result<String> {
    enviar_com(cfg, assunto, corpo, Duration::from_secs(cfg.timeout_s))
}

/// Quantas idas e voltas a conversa de [`enviar`] tem, no MAXIMO -- pedido
/// 463.
///
/// A saudacao, o `EHLO` e o `HELO` que o substitui quando recusado, as tres
/// do `AUTH LOGIN` quando ha login, o `MAIL`, um `RCPT` por destino, o
/// `DATA`, o corpo com o ponto, e o `QUIT`. Sai da forma da conversa, e nao
/// de um numero escolhido: e o que faz o prazo total nunca cortar um rele que
/// passaria no prazo de silencio de antes.
fn passos_da_conversa(cfg: &Email) -> u32 {
    let login = if cfg.usuario.is_empty() { 0 } else { 3 };
    // Com TLS: o aperto, o `STARTTLS` e o segundo `EHLO` (pedido 572, T6d).
    let tls = if matches!(cfg.tls.as_str(), "" | "desligado") && cfg.pino_tls.is_empty() {
        0
    } else {
        3
    };
    let passos = 1 + 2 + login + 1 + cfg.para.len() + 1 + 1 + 1 + tls;
    u32::try_from(passos).unwrap_or(u32::MAX)
}

/// O `enviar`, com o prazo de silencio na mao -- a prova do 463 precisa de
/// um prazo abaixo do segundo que o `timeout_s` nao sabe dizer.
///
/// # O prazo TOTAL da conversa (pedido 463)
///
/// O `timeout_s` mede o SILENCIO: vale para cada leitura e cada escrita do
/// soquete, e recomeca a cada byte. Um rele que pingue uma linha de
/// continuacao a um passo do prazo segurava a thread de aviso por ate mil
/// prazos (o teto de linhas), e uma linha pingada byte a byte, por ate 64 KiB
/// deles (o teto de tamanho). Agora a conversa inteira tem prazo:
/// `timeout_s` vezes os [`passos_da_conversa`]. O multiplo e o numero de idas
/// e voltas porque o rele mais lento que o prazo de silencio deixava passar
/// -- um que responde cada passo de uma vez, perto do limite -- continua
/// cabendo inteiro; so o que PINGA e cortado. Sem campo novo no
/// `config.json`: um segundo numero ao lado do `timeout_s` so seria mais um
/// para ninguem ajustar.
fn enviar_com(cfg: &Email, assunto: &str, corpo: &str, silencio: Duration) -> Result<String> {
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
    let fluxo = conectar(&alvo, silencio)?;
    // O relogio da conversa comeca depois do `connect`, que ja tem prazo
    // proprio por endereco.
    let total = silencio.saturating_mul(passos_da_conversa(cfg));
    let (leitura, escrita) =
        ComPrazo::armar(fluxo, Prazo::com_total(silencio, total, &ROTULO_SMTP))
            .map_err(|e| PhxError::Esquema(format!("smtp: nao consegui armar: {e}")))?;
    let mut sessao = Sessao {
        leitor: BufReader::new(FioDeCliente::Claro(leitura)),
        escrita: FioDeCliente::Claro(escrita),
    };
    let tls = cfg.tls_de_saida()?;

    // RFC 8314 §3.3: a 465 e TLS desde o primeiro byte (ou quem o disser).
    let implicito = tls.ligado() && cfg.tls_implicito();
    if implicito {
        sessao.passar_a_tls(&tls, &cfg.servidor)?;
    }
    sessao.esperar(&[220])?;
    // EHLO primeiro: e o que anuncia AUTH. Rele antigo so entende HELO, e
    // insistir no EHLO faria o envio falhar em servidor que funciona.
    sessao.cru(&format!("EHLO {}\r\n", nome_da_maquina()))?;
    let ehlo = ler_resposta_inteira(&mut sessao.leitor, &[250]);
    if tls.ligado() && !implicito {
        // STARTTLS (RFC 3207): so o rele que o ANUNCIA no EHLO. Quem pediu
        // TLS e nao o achou para aqui -- nem HELO, nem credencial, nem dado.
        let anuncia = ehlo.as_ref().is_ok_and(|linhas| {
            linhas.iter().any(|l| {
                l.get(4..)
                    .is_some_and(|x| x.trim().eq_ignore_ascii_case("STARTTLS"))
            })
        });
        if !anuncia {
            return Err(PhxError::Autorizacao(format!(
                "smtp: o rele {alvo} nao anuncia STARTTLS e alertas.email pede tls \
                 -- nao se cai para o claro calado"
            )));
        }
        sessao.comando("STARTTLS", &[220])?;
        sessao.passar_a_tls(&tls, &cfg.servidor)?;
        // §4.2: depois do TLS o cliente esquece o que soube e repete o EHLO.
        sessao.comando(&format!("EHLO {}", nome_da_maquina()), &[250])?;
    } else if ehlo.is_err() {
        sessao.comando(&format!("HELO {}", nome_da_maquina()), &[250])?;
    }

    if !cfg.usuario.is_empty() {
        // Os tres passos do AUTH respondem pela via SIGILOSA (pedido 550): e
        // nelas que o rele acabou de receber a credencial, e ha rele que a
        // ecoa na recusa (`535 ... <base64>`). O erro aqui nunca traz o que
        // foi enviado -- e agora tambem nao o texto livre do que VOLTOU.
        sessao.comando_sigiloso("AUTH LOGIN", &[334])?;
        sessao.comando_sigiloso(&base64::codificar(cfg.usuario.as_bytes()), &[334])?;
        sessao.comando_sigiloso(&base64::codificar(senha.as_bytes()), &[235])?;
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

/// Como o prazo total do SMTP se apresenta quando acaba.
static ROTULO_SMTP: Rotulo = Rotulo {
    quem: "smtp: a conversa com o rele",
    regra: "o timeout_s vezes as idas e voltas da conversa (pedido 463)",
};

/// O erro de E/S do SMTP: o do prazo total vira `LimiteExcedido` (pelo motor
/// comum, `crate::prazo`); o resto continua como era.
fn erro_de_io(o_que: &str, e: io::Error) -> PhxError {
    prazo::classificar(e, |e| PhxError::Esquema(format!("smtp: {o_que}: {e}")))
}

struct Sessao {
    /// Claro ou TLS pelo motor do core (pedido 572, T6d).
    leitor: BufReader<FioDeCliente>,
    escrita: FioDeCliente,
}

impl Sessao {
    /// Passa a conversa para TLS. O buffer de leitura tem de estar vazio --
    /// rele que manda linha grudada no `220` do STARTTLS a faria passar por
    /// protegida (a injecao de comando do CVE-2011-0411, do lado do
    /// cliente); o `passar_a_tls_com` do core recusa o buffer com sobra.
    fn passar_a_tls(&mut self, tls: &crate::tls_saida::TlsDeSaida, host: &str) -> Result<()> {
        tls.passar(&mut self.leitor, &mut self.escrita, host)
            .map_err(|e| PhxError::Autorizacao(format!("smtp: o TLS com o rele nao fechou: {e}")))
    }

    fn cru(&mut self, texto: &str) -> Result<()> {
        self.escrita
            .write_all(texto.as_bytes())
            .and_then(|_| self.escrita.flush())
            .map_err(|e| erro_de_io("escrita falhou", e))
    }

    fn comando(&mut self, linha: &str, esperados: &[u16]) -> Result<String> {
        self.cru(&format!("{linha}\r\n"))?;
        self.esperar(esperados)
    }

    fn esperar(&mut self, esperados: &[u16]) -> Result<String> {
        ler_resposta(&mut self.leitor, esperados)
    }

    /// O `comando` cuja resposta pode ecoar a credencial -- ver
    /// [`redigir_resposta`].
    fn comando_sigiloso(&mut self, linha: &str, esperados: &[u16]) -> Result<String> {
        self.cru(&format!("{linha}\r\n"))?;
        ler_resposta(&mut self.leitor, esperados).map_err(redigir_erro)
    }
}

/// **Pedido 550:** a resposta do rele a um passo do `AUTH`, redigida
/// ANALISANDO, nunca recortando -- a lei que o Profiler deixou (CLAUDE.md).
/// Fica o que se analisa: o codigo de tres digitos e o status estendido
/// `x.y.z` da RFC 3463, que e o que diz o motivo a quem opera. O texto livre
/// nao se analisa -- o rele pode ter posto ali o base64 da senha, e base64
/// nao e cifra --, entao vira o tamanho em bytes.
fn redigir_resposta(linha: &str) -> String {
    let mut partes = linha.trim().splitn(3, ' ');
    let codigo = partes.next().unwrap_or("");
    let codigo_valido = codigo.len() == 3 && codigo.bytes().all(|b| b.is_ascii_digit());
    if !codigo_valido {
        return format!("<resposta sem codigo, {} bytes, omitida>", linha.len());
    }
    let resto = linha.trim()[codigo.len()..].trim_start();
    let (estendido, livre) = match resto.split_once(' ') {
        Some((e, l)) if status_estendido(e) => (Some(e), l),
        _ if status_estendido(resto) => (Some(resto), ""),
        _ => (None, resto),
    };
    let mut saida = codigo.to_string();
    if let Some(e) = estendido {
        saida.push(' ');
        saida.push_str(e);
    }
    if !livre.is_empty() {
        saida.push_str(&format!(
            " <texto do rele, {} bytes, omitido: resposta ao envio da credencial>",
            livre.len()
        ));
    }
    saida
}

/// `classe.assunto.detalhe` da RFC 3463: digito, ponto, 1-3 digitos, ponto,
/// 1-3 digitos. So numero -- nao cabe senha nenhuma aqui.
fn status_estendido(t: &str) -> bool {
    let p: Vec<&str> = t.split('.').collect();
    p.len() == 3
        && p.iter()
            .all(|x| !x.is_empty() && x.len() <= 3 && x.bytes().all(|b| b.is_ascii_digit()))
}

/// O erro de `ler_resposta` com a resposta do rele redigida. So os dois que
/// carregam o texto do rele mudam; o de E/S e o de teto nao trazem nada dele.
fn redigir_erro(e: PhxError) -> PhxError {
    match e {
        PhxError::Esquema(m) => {
            if let Some(ultima) = m.strip_prefix("smtp recusou: ") {
                PhxError::Esquema(format!("smtp recusou: {}", redigir_resposta(ultima)))
            } else if m.starts_with("smtp: resposta sem codigo: ") {
                PhxError::Esquema(format!(
                    "smtp: resposta sem codigo ({} bytes, omitida: resposta ao envio da \
                     credencial)",
                    m.len() - "smtp: resposta sem codigo: ".len()
                ))
            } else {
                PhxError::Esquema(m)
            }
        }
        outro => outro,
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
/// fecha isso contando QUANTAS, e nao QUANTO TEMPO: mil linhas a um passo do
/// silencio ainda eram mil prazos. O QUANTO TEMPO e o prazo total que o
/// leitor de [`enviar_com`] traz por baixo -- aqui ele chega como erro de
/// leitura, e sai como `LimiteExcedido`.
fn ler_resposta<L: BufRead>(leitor: &mut L, esperados: &[u16]) -> Result<String> {
    ler_resposta_inteira(leitor, esperados).map(|mut l| l.pop().unwrap_or_default())
}

/// A resposta com TODAS as linhas -- a do `EHLO` traz as extensoes nas
/// linhas de continuacao, e e la que o `STARTTLS` se anuncia.
fn ler_resposta_inteira<L: BufRead>(leitor: &mut L, esperados: &[u16]) -> Result<Vec<String>> {
    let mut fio = Canal::Claro;
    let mut continuacoes = 0usize;
    let mut linhas = Vec::new();
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
            Err(PhxError::Io(e)) => return Err(erro_de_io("leitura falhou", e)),
            Err(outro) => return Err(outro),
        };
        let limpa = linha.trim_end().to_string();
        if limpa.as_bytes().get(3) != Some(&b'-') {
            break limpa;
        }
        linhas.push(limpa);
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
        linhas.push(ultima);
        Ok(linhas)
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
    use std::time::Instant;

    fn cfg() -> Email {
        let j = Json::analisar(
            r#"{"alertas":{"ligado":true,"email":{"ligado":true,
                "servidor":"127.0.0.1","de":"phx@exemplo.com",
                "para":["a@exemplo.com","b@exemplo.com"]}}}"#,
        )
        .unwrap();
        crate::config::Config::de_json(&j).unwrap().alertas.email
    }

    /// **Pedido 550:** o rele que ecoa a credencial na recusa do `AUTH`
    /// (`535 5.7.8 ... <base64 da senha>`). O erro que sobe -- e que vai ao
    /// log e a tela -- nao pode trazer a senha, nem em claro nem em base64.
    ///
    /// # Prova real
    ///
    /// Com o `comando` comum no lugar do `comando_sigiloso`, o base64 da
    /// senha sai no erro -- o vermelho medido.
    #[test]
    fn o_eco_da_credencial_na_recusa_do_auth_nao_sai_no_erro() {
        let senha = "segredo-do-rele-9x";
        let b64 = base64::codificar(senha.as_bytes());
        let porta = crate::apoio_teste::rele_falso_que_ecoa_o_auth();
        let j = Json::analisar(&format!(
            r#"{{"alertas":{{"ligado":true,"email":{{"ligado":true,
                "servidor":"127.0.0.1","porta":{porta},"de":"phx@exemplo.com",
                "para":["a@exemplo.com"],"usuario":"phx","senha":"{senha}"}}}}}}"#
        ))
        .unwrap();
        let c = crate::config::Config::de_json(&j).unwrap().alertas.email;
        let e = enviar_com(&c, "a", "b", Duration::from_secs(5))
            .expect_err("o rele recusou o AUTH")
            .to_string();
        assert!(!e.contains(&b64), "o base64 da senha saiu no erro: {e}");
        assert!(!e.contains(senha), "a senha saiu no erro: {e}");
        assert!(
            e.contains("535 5.7.8"),
            "o motivo que se analisa sumiu: {e}"
        );
        assert!(e.contains("bytes, omitido"), "{e}");
    }

    #[test]
    fn a_resposta_redigida_guarda_o_codigo_e_o_status() {
        assert_eq!(redigir_resposta("535 5.7.8"), "535 5.7.8");
        assert_eq!(
            redigir_resposta("535 5.7.8 falhou c2VuaGE="),
            "535 5.7.8 <texto do rele, 15 bytes, omitido: resposta ao envio da credencial>"
        );
        assert!(redigir_resposta("535 c2VuaGE=").starts_with("535 <texto do rele"));
        assert!(redigir_resposta("lixo c2VuaGE=").starts_with("<resposta sem codigo"));
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

    // ------------------------------- pedido 463, a metade do TEMPO

    /// Uma linha do cliente, sem o fim de linha. O rele falso tambem le pelo
    /// motor: a catraca do 439 conta todo `read_line` fora do `Canal`, e a de
    /// um teste nao e excecao.
    fn linha_do_cliente<L: BufRead>(leitor: &mut L) -> Option<String> {
        match Canal::Claro.ler_ate(leitor, TETO_DO_APERTO) {
            Ok(Recebido::Linha(l)) => Some(l.trim_end().to_string()),
            _ => None,
        }
    }

    /// Um rele que da a saudacao, le o `EHLO` e responde com `250-x` a cada
    /// `intervalo` -- bem abaixo do prazo de silencio --, `linhas` vezes, e
    /// fecha. Nada que o teto de tamanho (439) ou o de quantas (463, primeira
    /// metade) alcance: so TEMPO.
    fn rele_que_pinga(intervalo: Duration, linhas: usize) -> u16 {
        use std::io::Write as _;
        let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = ouvinte.accept() else {
                return;
            };
            let mut leitor = BufReader::new(s.try_clone().unwrap());
            if s.write_all(b"220 rele\r\n").is_err() || linha_do_cliente(&mut leitor).is_none() {
                return;
            }
            for _ in 0..linhas {
                std::thread::sleep(intervalo);
                if s.write_all(b"250-x\r\n").is_err() {
                    return;
                }
            }
        });
        porta
    }

    /// Um rele que conversa direito, com `atraso` antes de cada resposta.
    fn rele_bem_educado(atraso: Duration) -> u16 {
        use std::io::Write as _;
        let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = ouvinte.accept() else {
                return;
            };
            let mut leitor = BufReader::new(s.try_clone().unwrap());
            let mut responder = |texto: &str| {
                std::thread::sleep(atraso);
                s.write_all(texto.as_bytes()).is_ok()
            };
            if !responder("220 rele\r\n") {
                return;
            }
            let mut no_corpo = false;
            loop {
                let Some(linha) = linha_do_cliente(&mut leitor) else {
                    return;
                };
                let resposta = if no_corpo {
                    if linha != "." {
                        continue;
                    }
                    no_corpo = false;
                    "250 2.0.0 na fila como X463\r\n"
                } else if linha.starts_with("EHLO") {
                    "250-rele\r\n250-SIZE 1000000\r\n250 OK\r\n"
                } else if linha.starts_with("DATA") {
                    no_corpo = true;
                    "354 manda\r\n"
                } else if linha.starts_with("QUIT") {
                    let _ = responder("221 tchau\r\n");
                    return;
                } else {
                    "250 ok\r\n"
                };
                if !responder(resposta) {
                    return;
                }
            }
        });
        porta
    }

    /// **463: o rele que pinga abaixo do prazo de silencio e cortado no
    /// prazo TOTAL da conversa.**
    ///
    /// Silencio de 500 ms e dois destinos: 9 idas e voltas, prazo total de
    /// 4,5 s. O rele pinga a cada 50 ms por 6 s. Com o defeito (so o
    /// silencio), a conversa durava o que o rele quisesse -- aqui, ate ele
    /// fechar, e o erro era o do fecho.
    #[test]
    fn o_rele_que_pinga_e_cortado_no_prazo_total() {
        let mut c = cfg();
        c.porta = rele_que_pinga(Duration::from_millis(50), 120);
        let silencio = Duration::from_millis(500);
        let prazo = silencio * passos_da_conversa(&c);
        assert_eq!(prazo, Duration::from_millis(4_500), "premissa: 9 passos");
        let comeco = Instant::now();
        let r = enviar_com(&c, "x", "y", silencio);
        let durou = comeco.elapsed();
        eprintln!("a conversa com o rele que pinga durou {durou:?} (prazo {prazo:?})");
        match r {
            Err(PhxError::LimiteExcedido(m)) => assert!(m.contains("prazo total"), "{m}"),
            outro => panic!("em {durou:?} a conversa nao parou no prazo total: {outro:?}"),
        }
        assert!(
            durou >= prazo - Duration::from_millis(100) && durou < Duration::from_secs(6),
            "a conversa parou em {durou:?}, e o prazo total era {prazo:?}"
        );
    }

    /// O rele de verdade continua: a conversa inteira, resposta de varias
    /// linhas no `EHLO`, e o recibo do `.` volta.
    #[test]
    fn a_conversa_normal_continua_inteira() {
        let mut c = cfg();
        c.porta = rele_bem_educado(Duration::ZERO);
        let recibo = enviar_com(&c, "assunto", "corpo", Duration::from_millis(500)).unwrap();
        assert_eq!(recibo, "250 2.0.0 na fila como X463");
    }

    /// **O multiplo nao corta o rele LENTO de verdade**: o que responde cada
    /// passo de uma vez, perto do prazo de silencio, cabe inteiro -- e por
    /// isso o total e o silencio vezes as idas e voltas, e nao um numero
    /// escolhido. 200 ms por resposta contra 300 ms de silencio: 1,8 s de
    /// conversa num prazo de 2,7 s.
    #[test]
    fn o_rele_lento_que_responde_cada_passo_inteiro_cabe() {
        let mut c = cfg();
        c.porta = rele_bem_educado(Duration::from_millis(200));
        let recibo = enviar_com(&c, "assunto", "corpo", Duration::from_millis(300)).unwrap();
        assert_eq!(recibo, "250 2.0.0 na fila como X463");
    }
}

// ----------------------------------------------- TLS com o rele (572, T6d) ----
//
// O rele de verdade aqui e um falso escrito em PYTHON, com o modulo `ssl` da
// biblioteca padrao dele -- que e o OpenSSL do sistema: outra mao, outra pilha
// TLS. Ele fala SMTP o bastante para um envio inteiro, com `STARTTLS` (RFC
// 3207) ou TLS desde o primeiro byte (RFC 8314), e diz o que viu.
#[cfg(test)]
mod testes_tls {
    use super::*;
    use crate::apoio_teste::DirTemp;
    use phxsql_core::json::Json;
    use std::io::Read;
    use std::process::{Command, Stdio};

    const RELE_PY: &str = r#"
import socket, ssl, sys, json
modo, cert, chave = sys.argv[1], sys.argv[2], sys.argv[3]
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.minimum_version = ssl.TLSVersion.TLSv1_3
ctx.load_cert_chain(cert, chave)
s = socket.socket(); s.bind(("127.0.0.1", 0)); s.listen(1)
print(s.getsockname()[1], flush=True)
c, _ = s.accept(); c.settimeout(10)
visto = {"claro": [], "tls": [], "versao": None}
if modo == "implicito":
    c = ctx.wrap_socket(c, server_side=True); visto["versao"] = c.version()
f = c.makefile("rb")
def linha():
    l = f.readline().decode().rstrip("\r\n")
    visto["tls" if visto["versao"] else "claro"].append(l)
    return l
def manda(t):
    c.sendall((t + "\r\n").encode())
manda("220 rele de teste")
while True:
    l = linha()
    if not l: break
    v = l.upper()
    if v.startswith("EHLO"):
        if modo == "sem_starttls" or visto["versao"]:
            manda("250-rele\r\n250 OK")
        else:
            manda("250-rele\r\n250-STARTTLS\r\n250 OK")
    elif v == "STARTTLS":
        manda("220 vai")
        f.close()
        c = ctx.wrap_socket(c, server_side=True); visto["versao"] = c.version()
        f = c.makefile("rb")
    elif v.startswith("MAIL") or v.startswith("RCPT"):
        manda("250 ok")
    elif v == "DATA":
        manda("354 manda")
        while linha() != ".": pass
        manda("250 aceita")
    elif v == "QUIT":
        manda("221 tchau"); break
    else:
        manda("502 nao")
print(json.dumps(visto), flush=True)
"#;

    /// Raiz e folha (`localhost`) pelo openssl, na pasta dada.
    fn certificados(d: &std::path::Path) {
        let ok = |args: &[&str]| {
            let s = Command::new("openssl")
                .current_dir(d)
                .args(args)
                .output()
                .unwrap();
            assert!(s.status.success(), "{}", String::from_utf8_lossy(&s.stderr));
        };
        ok(&[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            "raiz.key",
            "-out",
            "raiz.pem",
            "-days",
            "2",
            "-subj",
            "/CN=Raiz do rele",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign",
        ]);
        std::fs::write(
            d.join("ext"),
            "[f]\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n",
        )
        .unwrap();
        ok(&[
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-out",
            "rele.key",
        ]);
        ok(&[
            "req",
            "-new",
            "-key",
            "rele.key",
            "-subj",
            "/CN=localhost",
            "-out",
            "rele.csr",
        ]);
        ok(&[
            "x509",
            "-req",
            "-in",
            "rele.csr",
            "-CA",
            "raiz.pem",
            "-CAkey",
            "raiz.key",
            "-CAcreateserial",
            "-days",
            "1",
            "-extfile",
            "ext",
            "-extensions",
            "f",
            "-out",
            "rele.pem",
        ]);
    }

    fn cfg(porta: u16, extra: &str) -> Email {
        let j = Json::analisar(&format!(
            r#"{{"alertas":{{"ligado":true,"email":{{"ligado":true,
                "servidor":"localhost","porta":{porta},"de":"phx@exemplo.com",
                "para":["a@exemplo.com"]{extra}}}}}}}"#
        ))
        .unwrap();
        crate::config::Config::de_json(&j).unwrap().alertas.email
    }

    /// Sobe o rele em Python e devolve (processo, porta).
    fn rele(d: &std::path::Path, modo: &str) -> (std::process::Child, u16) {
        let mut filho = Command::new("python3")
            .args(["-I", "-c", RELE_PY, modo])
            .arg(d.join("rele.pem"))
            .arg(d.join("rele.key"))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut saida = filho.stdout.take().unwrap();
        let mut b = Vec::new();
        let mut um = [0u8; 1];
        while saida.read(&mut um).unwrap() == 1 && um[0] != b'\n' {
            b.push(um[0]);
        }
        filho.stdout = Some(saida);
        (filho, String::from_utf8(b).unwrap().trim().parse().unwrap())
    }

    fn o_que_viu(mut filho: std::process::Child) -> Json {
        let mut s = String::new();
        filho.stdout.take().unwrap().read_to_string(&mut s).unwrap();
        let _ = filho.wait();
        Json::analisar(s.trim()).unwrap_or(Json::Nulo)
    }

    fn textos(j: &Json, campo: &str) -> Vec<String> {
        j.campo(campo)
            .and_then(Json::lista)
            // A linha vazia e o fim da conexao que o `readline` do Python
            // devolve, e nao algo que o cliente mandou.
            .map(|l| {
                l.iter()
                    .filter_map(|x| x.texto().filter(|t| !t.is_empty()).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn starttls_e_implicito_contra_o_ssl_do_python_conferindo_a_cadeia() {
        let d = DirTemp::novo("smtp-tls");
        certificados(&d.0);
        let ca = format!(
            r#","tls":"verificar","tls_ca":"{}""#,
            d.0.join("raiz.pem").display()
        );
        for modo in ["starttls", "implicito"] {
            let (filho, porta) = rele(&d.0, modo);
            let extra = if modo == "implicito" {
                format!(r#"{ca},"tls_implicito":true"#)
            } else {
                ca.clone()
            };
            let recibo = enviar(&cfg(porta, &extra), "teste do TLS", "corpo secreto")
                .unwrap_or_else(|e| panic!("{modo}: {e}"));
            assert!(recibo.contains("250"), "{recibo}");
            let v = o_que_viu(filho);
            assert_eq!(v.texto_ou("versao", ""), "TLSv1.3", "{modo}: {v:?}");
            let claro = textos(&v, "claro");
            let tls = textos(&v, "tls");
            // Em claro so o EHLO e o STARTTLS; o remetente e o corpo, nunca.
            let esperado_claro = if modo == "implicito" { 0 } else { 2 };
            assert_eq!(claro.len(), esperado_claro, "{modo}: {claro:?}");
            assert!(claro.iter().all(|l| !l.contains("MAIL")), "{claro:?}");
            assert!(tls.iter().any(|l| l.starts_with("MAIL FROM")), "{tls:?}");
        }
    }

    #[test]
    fn rele_sem_starttls_para_antes_do_remetente() {
        let d = DirTemp::novo("smtp-sem-starttls");
        certificados(&d.0);
        let (filho, porta) = rele(&d.0, "sem_starttls");
        let e = enviar(&cfg(porta, r#","tls":"exigir""#), "x", "corpo")
            .unwrap_err()
            .to_string();
        assert!(e.contains("nao anuncia STARTTLS"), "{e}");
        let v = o_que_viu(filho);
        let claro = textos(&v, "claro");
        assert_eq!(claro.len(), 1, "so o EHLO, e nada depois: {claro:?}");
    }

    #[test]
    fn raiz_alheia_recusa_o_rele_antes_do_remetente() {
        let d = DirTemp::novo("smtp-raiz-alheia");
        certificados(&d.0);
        let outra = DirTemp::novo("smtp-raiz-alheia-2");
        certificados(&outra.0);
        let (filho, porta) = rele(&d.0, "starttls");
        let extra = format!(
            r#","tls":"verificar","tls_ca":"{}""#,
            outra.0.join("raiz.pem").display()
        );
        let e = enviar(&cfg(porta, &extra), "x", "corpo")
            .unwrap_err()
            .to_string();
        assert!(e.contains("cadeia X.509"), "{e}");
        let v = o_que_viu(filho);
        assert!(textos(&v, "tls").is_empty(), "{v:?}");
    }

    /// Linha grudada no `220` do STARTTLS: o rele (ou quem esta no meio)
    /// manda um comando em claro que o cliente leria como se viesse pelo TLS
    /// (o CVE-2011-0411, do lado do cliente). O buffer com sobra recusa.
    #[test]
    fn linha_grudada_no_220_do_starttls_recusa() {
        let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = ouvinte.accept().unwrap();
            let mut l = BufReader::new(s.try_clone().unwrap());
            // Pelo `Canal` do motor, como toda leitura de linha de soquete.
            let mut canal = Canal::Claro;
            s.write_all(b"220 oi\r\n").unwrap();
            canal.ler_ate(&mut l, TETO_DO_APERTO).unwrap();
            s.write_all(b"250-rele\r\n250 STARTTLS\r\n").unwrap();
            canal.ler_ate(&mut l, TETO_DO_APERTO).unwrap();
            s.write_all(b"220 vai\r\n250 injetada\r\n").unwrap();
            let mut resto = Vec::new();
            let _ = l.read_to_end(&mut resto);
        });
        let e = enviar(&cfg(porta, r#","tls":"exigir""#), "x", "corpo")
            .unwrap_err()
            .to_string();
        assert!(e.contains("nada foi lido"), "{e}");
    }
}
