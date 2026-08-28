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

use crate::config::Email;

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
        sessao.comando(&base64::codificar(cfg.senha().as_bytes()), &[235])?;
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

    /// Le a resposta e confere o codigo.
    ///
    /// Resposta de SMTP pode vir em varias linhas: as intermediarias tem um
    /// hifen depois do numero (`250-AUTH LOGIN`) e a ultima um espaco
    /// (`250 OK`). Parar na primeira deixaria o resto no soquete e jogaria
    /// todo o dialogo seguinte fora de sincronia.
    fn esperar(&mut self, esperados: &[u16]) -> Result<String> {
        let ultima = loop {
            let mut linha = String::new();
            let lidos = self
                .leitor
                .read_line(&mut linha)
                .map_err(|e| PhxError::Esquema(format!("smtp: leitura falhou: {e}")))?;
            if lidos == 0 {
                return Err(PhxError::Esquema(
                    "smtp: o servidor fechou a conexao no meio da resposta".into(),
                ));
            }
            let limpa = linha.trim_end().to_string();
            if limpa.as_bytes().get(3) != Some(&b'-') {
                break limpa;
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
}

/// Monta o corpo RFC 5322 ja pronto para o `DATA`.
pub fn mensagem(cfg: &Email, assunto: &str, corpo: &str) -> Result<String> {
    let mut m = String::new();
    m.push_str(&format!("From: {}\r\n", uma_linha_so(&cfg.de)?));
    m.push_str(&format!("To: {}\r\n", uma_linha_so(&cfg.para.join(", "))?));
    m.push_str(&format!("Subject: {}\r\n", uma_linha_so(assunto)?));
    m.push_str(&format!("Date: {}\r\n", data_rfc5322(crate::agora_ms())));
    m.push_str("MIME-Version: 1.0\r\n");
    m.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    m.push_str("X-Mailer: PhxSql\r\n");
    m.push_str("\r\n");
    for linha in corpo.split('\n') {
        let linha = linha.trim_end_matches('\r');
        // "Dot stuffing": uma linha que comeca com ponto encerraria o DATA. O
        // ponto dobrado e como o RFC manda escapar, e o outro lado desfaz.
        if linha.starts_with('.') {
            m.push('.');
        }
        m.push_str(linha);
        m.push_str("\r\n");
    }
    Ok(m)
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
fn nome_da_maquina() -> String {
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
        assert!(m.contains("Subject: disco apertado\r\n"));
        assert!(m.contains("\r\n\r\nlinha 1\r\nlinha 2\r\n"));
    }

    #[test]
    fn assunto_com_quebra_de_linha_nao_vira_cabecalho() {
        // O ataque: quem escreve o assunto acrescenta um destinatario oculto.
        let e = mensagem(&cfg(), "oi\r\nBcc: ladrao@fora.com", "corpo");
        assert!(e.is_err(), "assunto com CRLF passou");
    }

    #[test]
    fn linha_que_comeca_com_ponto_e_dobrada() {
        // Sem dobrar, esta linha encerraria o DATA e o resto do corpo viraria
        // comando SMTP.
        let m = mensagem(&cfg(), "x", ".\nfim").unwrap();
        assert!(m.contains("\r\n..\r\nfim\r\n"), "{m}");
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
}
