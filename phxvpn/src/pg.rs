//! Cliente do protocolo de fio do PostgreSQL (versao 3.0), so com `std`.
//!
//! # Por que escrito aqui
//!
//! A petrea da casa e zero dependencia externa, e o que o painel precisa do
//! PostgreSQL e pouco: autenticar, mandar comando com PARAMETROS e ler linhas
//! em texto. Isso cabe no protocolo estendido (Parse/Bind/Describe/Execute/
//! Sync) sem nada de cursor, COPY ou tipo binario.
//!
//! # Por que so o protocolo estendido para dado de usuario
//!
//! O protocolo simples manda o SQL inteiro como texto, e ai todo valor vindo da
//! tela teria de ser escapado a mao -- e o escape que alguem esquecer vira
//! injecao. No estendido o valor viaja num campo proprio do `Bind` e nunca e
//! lido como SQL. O simples fica so para o esquema, que e texto nosso.
//!
//! # Autenticacao
//!
//! SCRAM-SHA-256 (RFC 5802 / RFC 7677), o padrao do PostgreSQL desde a 14, com
//! o SHA-256, o HMAC e o PBKDF2 do `phxsql-core` -- ja conferidos contra os
//! vetores oficiais la. A assinatura do servidor e CONFERIDA: sem isso um
//! servidor falso aceitaria qualquer prova e o cliente nem saberia. Senha em
//! texto claro (codigo 3) e aceita porque o `pg_hba` pode pedi-la; MD5 nao,
//! porque o proprio PostgreSQL o declara obsoleto.

use phxsql_core::base64;
use phxsql_core::hash::{hmac_sha256, iguais_em_tempo_constante, pbkdf2_sha256, sha256};
use phxsql_core::senha::bytes_aleatorios;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Teto de uma mensagem do servidor. Linha de cadastro nao chega perto; o teto
/// existe para que um servidor torto nao faca o cliente alocar gigabytes.
const TETO_MENSAGEM: usize = 64 * 1024 * 1024;

pub type R<T> = Result<T, String>;

/// Onde e como conectar. Sai de uma cadeia `chave=valor` no formato da libpq.
#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub porta: u16,
    pub usuario: String,
    pub senha: String,
    pub banco: String,
}

impl Config {
    /// Le `host=... port=... user=... password=... dbname=...`. Valor com espaco
    /// nao e suportado -- o cadastro do painel nao precisa, e dizer que nao
    /// suporta e melhor que aceitar pela metade.
    pub fn de_texto(texto: &str) -> R<Config> {
        let mut cfg = Config {
            host: "127.0.0.1".into(),
            porta: 5432,
            usuario: "postgres".into(),
            senha: String::new(),
            banco: String::new(),
        };
        for par in texto.split_whitespace() {
            let (k, v) = par
                .split_once('=')
                .ok_or_else(|| format!("parametro sem '=' na conexao: {par}"))?;
            match k {
                "host" => cfg.host = v.into(),
                "port" => cfg.porta = v.parse().map_err(|_| format!("porta invalida: {v}"))?,
                "user" => cfg.usuario = v.into(),
                "password" => cfg.senha = v.into(),
                "dbname" => cfg.banco = v.into(),
                outro => return Err(format!("parametro de conexao desconhecido: {outro}")),
            }
        }
        if cfg.banco.is_empty() {
            cfg.banco = cfg.usuario.clone();
        }
        Ok(cfg)
    }
}

/// O que um comando devolveu: colunas, linhas em texto (`None` e o NULL) e o
/// numero de linhas afetadas lido do `CommandComplete`.
#[derive(Debug, Default)]
pub struct Resposta {
    pub colunas: Vec<String>,
    pub linhas: Vec<Vec<Option<String>>>,
    pub afetadas: u64,
}

impl Resposta {
    /// Valor da coluna `nome` na linha `i`, ou vazio.
    pub fn valor(&self, i: usize, nome: &str) -> Option<&str> {
        let c = self.colunas.iter().position(|x| x == nome)?;
        self.linhas.get(i)?.get(c)?.as_deref()
    }
}

pub struct Pg {
    fio: TcpStream,
}

impl Pg {
    pub fn conectar(cfg: &Config) -> R<Pg> {
        let fio = TcpStream::connect((cfg.host.as_str(), cfg.porta)).map_err(|e| {
            format!(
                "PostgreSQL em {}:{} nao respondeu: {e}",
                cfg.host, cfg.porta
            )
        })?;
        fio.set_read_timeout(Some(Duration::from_secs(30))).ok();
        fio.set_nodelay(true).ok();
        let mut pg = Pg { fio };
        pg.iniciar(cfg)?;
        Ok(pg)
    }

    fn iniciar(&mut self, cfg: &Config) -> R<()> {
        let mut corpo = Vec::new();
        corpo.extend_from_slice(&196_608i32.to_be_bytes()); // versao 3.0
        for (k, v) in [
            ("user", cfg.usuario.as_str()),
            ("database", cfg.banco.as_str()),
            ("client_encoding", "UTF8"),
            ("application_name", "phxvpn"),
        ] {
            corpo.extend_from_slice(k.as_bytes());
            corpo.push(0);
            corpo.extend_from_slice(v.as_bytes());
            corpo.push(0);
        }
        corpo.push(0);
        let mut msg = ((corpo.len() + 4) as i32).to_be_bytes().to_vec();
        msg.extend_from_slice(&corpo);
        self.fio.write_all(&msg).map_err(|e| e.to_string())?;

        let mut scram: Option<Scram> = None;
        loop {
            let (tipo, dados) = self.ler()?;
            match tipo {
                b'R' => {
                    let codigo = i32::from_be_bytes(pedaco4(&dados, 0)?);
                    match codigo {
                        0 => {}
                        3 => {
                            let mut s = cfg.senha.as_bytes().to_vec();
                            s.push(0);
                            self.enviar(b'p', &s)?;
                        }
                        10 => {
                            let mecanismos = String::from_utf8_lossy(&dados[4..]);
                            if !mecanismos.split('\0').any(|m| m == "SCRAM-SHA-256") {
                                return Err(format!(
                                    "servidor so oferece {mecanismos:?}; este cliente fala SCRAM-SHA-256"
                                ));
                            }
                            let s = Scram::novo();
                            let primeira = s.primeira_mensagem();
                            let mut corpo = b"SCRAM-SHA-256\0".to_vec();
                            corpo.extend_from_slice(&(primeira.len() as i32).to_be_bytes());
                            corpo.extend_from_slice(primeira.as_bytes());
                            self.enviar(b'p', &corpo)?;
                            scram = Some(s);
                        }
                        11 => {
                            let s = scram.as_mut().ok_or("SASLContinue sem SASL iniciado")?;
                            let resposta = s.segunda_mensagem(&cfg.senha, &dados[4..])?;
                            self.enviar(b'p', resposta.as_bytes())?;
                        }
                        12 => {
                            let s = scram.as_ref().ok_or("SASLFinal sem SASL iniciado")?;
                            s.conferir_servidor(&dados[4..])?;
                        }
                        5 => {
                            return Err(
                                "servidor pediu MD5; configure scram-sha-256 no pg_hba".into()
                            )
                        }
                        outro => {
                            return Err(format!("autenticacao PostgreSQL nao suportada: {outro}"))
                        }
                    }
                }
                b'E' => return Err(erro_pg(&dados)),
                b'Z' => return Ok(()),
                _ => {} // ParameterStatus, BackendKeyData, NoticeResponse
            }
        }
    }

    /// Varios comandos SEM parametro, pelo protocolo simples. So para SQL nosso
    /// (o esquema) -- dado de usuario vai por [`Pg::executar`].
    pub fn lote(&mut self, sql: &str) -> R<()> {
        let mut corpo = sql.as_bytes().to_vec();
        corpo.push(0);
        self.enviar(b'Q', &corpo)?;
        let mut falha = None;
        loop {
            let (tipo, dados) = self.ler()?;
            match tipo {
                b'E' => falha = Some(erro_pg(&dados)),
                b'Z' => return falha.map_or(Ok(()), Err),
                _ => {}
            }
        }
    }

    /// Um comando com parametros `$1..$n`, todos em texto. `None` e o NULL.
    pub fn executar(&mut self, sql: &str, params: &[Option<&str>]) -> R<Resposta> {
        let mut buf = Vec::new();
        // Parse: comando sem nome, sem tipo declarado (o servidor infere).
        let mut p = vec![0u8];
        p.extend_from_slice(sql.as_bytes());
        p.push(0);
        p.extend_from_slice(&0i16.to_be_bytes());
        mensagem(&mut buf, b'P', &p);
        // Bind: portal e comando sem nome, tudo em texto.
        let mut b = vec![0u8, 0u8];
        b.extend_from_slice(&0i16.to_be_bytes());
        b.extend_from_slice(&(params.len() as i16).to_be_bytes());
        for v in params {
            match v {
                None => b.extend_from_slice(&(-1i32).to_be_bytes()),
                Some(t) => {
                    b.extend_from_slice(&(t.len() as i32).to_be_bytes());
                    b.extend_from_slice(t.as_bytes());
                }
            }
        }
        b.extend_from_slice(&0i16.to_be_bytes());
        mensagem(&mut buf, b'B', &b);
        mensagem(&mut buf, b'D', b"P\0");
        mensagem(&mut buf, b'E', &[0, 0, 0, 0, 0]);
        mensagem(&mut buf, b'S', &[]);
        self.fio.write_all(&buf).map_err(|e| e.to_string())?;

        let mut r = Resposta::default();
        let mut falha = None;
        loop {
            let (tipo, dados) = self.ler()?;
            match tipo {
                b'T' => r.colunas = colunas(&dados)?,
                b'D' => r.linhas.push(linha(&dados)?),
                b'C' => {
                    let tag = String::from_utf8_lossy(&dados);
                    r.afetadas = tag
                        .trim_end_matches('\0')
                        .rsplit(' ')
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(0);
                }
                b'E' => falha = Some(erro_pg(&dados)),
                b'Z' => return falha.map_or(Ok(r), Err),
                _ => {}
            }
        }
    }

    fn enviar(&mut self, tipo: u8, corpo: &[u8]) -> R<()> {
        let mut buf = Vec::with_capacity(corpo.len() + 5);
        mensagem(&mut buf, tipo, corpo);
        self.fio.write_all(&buf).map_err(|e| e.to_string())
    }

    fn ler(&mut self) -> R<(u8, Vec<u8>)> {
        let mut cab = [0u8; 5];
        self.fio
            .read_exact(&mut cab)
            .map_err(|e| format!("conexao com o PostgreSQL caiu: {e}"))?;
        let tam = i32::from_be_bytes([cab[1], cab[2], cab[3], cab[4]]);
        if tam < 4 || tam as usize > TETO_MENSAGEM {
            return Err(format!("mensagem do PostgreSQL com tamanho absurdo: {tam}"));
        }
        let mut dados = vec![0u8; tam as usize - 4];
        self.fio.read_exact(&mut dados).map_err(|e| e.to_string())?;
        Ok((cab[0], dados))
    }
}

impl Drop for Pg {
    fn drop(&mut self) {
        let _ = self.fio.write_all(&[b'X', 0, 0, 0, 4]);
    }
}

fn mensagem(buf: &mut Vec<u8>, tipo: u8, corpo: &[u8]) {
    buf.push(tipo);
    buf.extend_from_slice(&((corpo.len() + 4) as i32).to_be_bytes());
    buf.extend_from_slice(corpo);
}

fn pedaco4(d: &[u8], i: usize) -> R<[u8; 4]> {
    d.get(i..i + 4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| "mensagem do PostgreSQL truncada".to_string())
}

fn pedaco2(d: &[u8], i: usize) -> R<[u8; 2]> {
    d.get(i..i + 2)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| "mensagem do PostgreSQL truncada".to_string())
}

fn colunas(d: &[u8]) -> R<Vec<String>> {
    let n = i16::from_be_bytes(pedaco2(d, 0)?) as usize;
    let mut i = 2;
    let mut nomes = Vec::with_capacity(n);
    for _ in 0..n {
        let fim = d[i..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("RowDescription sem terminador")?;
        nomes.push(String::from_utf8_lossy(&d[i..i + fim]).into_owned());
        i += fim + 1 + 18; // tabela(4) coluna(2) tipo(4) tamanho(2) modificador(4) formato(2)
    }
    Ok(nomes)
}

fn linha(d: &[u8]) -> R<Vec<Option<String>>> {
    let n = i16::from_be_bytes(pedaco2(d, 0)?) as usize;
    let mut i = 2;
    let mut valores = Vec::with_capacity(n);
    for _ in 0..n {
        let tam = i32::from_be_bytes(pedaco4(d, i)?);
        i += 4;
        if tam < 0 {
            valores.push(None);
        } else {
            let fim = i + tam as usize;
            let v = d.get(i..fim).ok_or("DataRow truncada")?;
            valores.push(Some(String::from_utf8_lossy(v).into_owned()));
            i = fim;
        }
    }
    Ok(valores)
}

/// Monta o texto de um `ErrorResponse`: severidade, codigo SQLSTATE e mensagem.
fn erro_pg(d: &[u8]) -> String {
    let (mut sev, mut cod, mut msg) = (String::new(), String::new(), String::new());
    for campo in d.split(|&b| b == 0) {
        if let Some((&t, resto)) = campo.split_first() {
            let v = String::from_utf8_lossy(resto).into_owned();
            match t {
                b'S' => sev = v,
                b'C' => cod = v,
                b'M' => msg = v,
                _ => {}
            }
        }
    }
    format!("PostgreSQL {sev} {cod}: {msg}")
}

/// Estado do SCRAM-SHA-256 entre as tres mensagens.
struct Scram {
    nonce_cliente: String,
    primeira_nua: String,
    assinatura_servidor: Option<[u8; 32]>,
}

impl Scram {
    fn novo() -> Scram {
        let nonce_cliente = base64::codificar(&bytes_aleatorios(18));
        // O PostgreSQL ignora o nome do SCRAM (usa o da conexao), por isso `n=`.
        let primeira_nua = format!("n=,r={nonce_cliente}");
        Scram {
            nonce_cliente,
            primeira_nua,
            assinatura_servidor: None,
        }
    }

    fn primeira_mensagem(&self) -> String {
        format!("n,,{}", self.primeira_nua)
    }

    fn segunda_mensagem(&mut self, senha: &str, servidor: &[u8]) -> R<String> {
        let primeira_srv = String::from_utf8_lossy(servidor).into_owned();
        let mut nonce = "";
        let mut sal = "";
        let mut iter = 0u32;
        for parte in primeira_srv.split(',') {
            if let Some(v) = parte.strip_prefix("r=") {
                nonce = v;
            } else if let Some(v) = parte.strip_prefix("s=") {
                sal = v;
            } else if let Some(v) = parte.strip_prefix("i=") {
                iter = v.parse().map_err(|_| "iteracoes SCRAM invalidas")?;
            }
        }
        // O nonce do servidor TEM de comecar pelo nosso: e o que amarra esta
        // resposta a este pedido e impede repetir uma troca gravada.
        if !nonce.starts_with(&self.nonce_cliente) || iter == 0 {
            return Err("resposta SCRAM do servidor nao confere com o pedido".into());
        }
        let sal = base64::decodificar(sal).map_err(|e| format!("sal SCRAM: {e}"))?;
        let mut salgada = [0u8; 32];
        pbkdf2_sha256(senha.as_bytes(), &sal, iter, &mut salgada);
        let chave_cliente = hmac_sha256(&salgada, b"Client Key");
        let guardada = sha256(&chave_cliente);
        let sem_prova = format!("c=biws,r={nonce}");
        let auth = format!("{},{primeira_srv},{sem_prova}", self.primeira_nua);
        let assinatura = hmac_sha256(&guardada, auth.as_bytes());
        let prova: Vec<u8> = chave_cliente
            .iter()
            .zip(assinatura.iter())
            .map(|(a, b)| a ^ b)
            .collect();
        let chave_servidor = hmac_sha256(&salgada, b"Server Key");
        self.assinatura_servidor = Some(hmac_sha256(&chave_servidor, auth.as_bytes()));
        Ok(format!("{sem_prova},p={}", base64::codificar(&prova)))
    }

    fn conferir_servidor(&self, final_srv: &[u8]) -> R<()> {
        let texto = String::from_utf8_lossy(final_srv);
        let v = texto
            .split(',')
            .find_map(|p| p.strip_prefix("v="))
            .ok_or("SASLFinal sem assinatura do servidor")?;
        let recebida = base64::decodificar(v.trim_end_matches('\0'))
            .map_err(|e| format!("assinatura do servidor: {e}"))?;
        let esperada = self.assinatura_servidor.ok_or("SASLFinal fora de ordem")?;
        if !iguais_em_tempo_constante(&recebida, &esperada) {
            return Err("o servidor PostgreSQL nao provou conhecer a senha (SCRAM)".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Vetor da RFC 7677 secao 3 (usuario "user", senha "pencil"): a prova do
    /// cliente e a assinatura do servidor tem de sair identicas as da norma.
    #[test]
    fn scram_confere_contra_rfc7677() {
        let mut s = Scram {
            nonce_cliente: "rOprNGfwEbeRWgbNEkqO".into(),
            primeira_nua: "n=user,r=rOprNGfwEbeRWgbNEkqO".into(),
            assinatura_servidor: None,
        };
        let srv = b"r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096";
        let fim = s.segunda_mensagem("pencil", srv).unwrap();
        assert_eq!(
            fim,
            "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
        );
        s.conferir_servidor(b"v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=")
            .unwrap();
        // Assinatura torta: tem de recusar.
        assert!(s
            .conferir_servidor(b"v=7rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=")
            .is_err());
    }

    #[test]
    fn nonce_do_servidor_que_nao_estende_o_nosso_e_recusado() {
        let mut s = Scram::novo();
        assert!(s
            .segunda_mensagem("x", b"r=outro,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096")
            .is_err());
    }

    #[test]
    fn config_le_o_formato_da_libpq() {
        let c = Config::de_texto("host=db port=6543 user=vpn password=x dbname=phx").unwrap();
        assert_eq!(
            (c.host.as_str(), c.porta, c.banco.as_str()),
            ("db", 6543, "phx")
        );
        assert!(Config::de_texto("sslmode=require").is_err());
    }
}
