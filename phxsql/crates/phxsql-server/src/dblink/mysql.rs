//! O protocolo do MySQL(R) escrito a mao, so com a `std`.
//!
//! # Por que a mao
//!
//! Pela regra do projeto: nenhuma crate externa. E, ao contrario do que
//! parece, o protocolo do MySQL(R) e pequeno quando o escopo e pequeno --
//! conectar, autenticar e mandar consulta de texto. O que este modulo NAO faz
//! esta em [`Limites`](#limites).
//!
//! # O quadro
//!
//! Tudo viaja em quadros de `3 bytes de tamanho (little endian) + 1 byte de
//! sequencia + carga`. A sequencia zera a cada comando novo e sobe de um em um
//! dentro do comando; o servidor conta junto e reclama se pular.
//!
//! # A autenticacao
//!
//! Duas formas, e as duas ja existiam no projeto ou entraram por causa dela:
//!
//! - `mysql_native_password`: `SHA1(senha) XOR SHA1(sal || SHA1(SHA1(senha)))`.
//!   Funciona sempre.
//! - `caching_sha2_password` (o padrao do MySQL(R) 8): o mesmo desenho com
//!   SHA-256. So o CAMINHO RAPIDO, que vale quando o servidor ja tem a senha
//!   daquele usuario em cache. O caminho completo exige mandar a senha cifrada
//!   com a chave publica RSA do servidor, ou TLS -- e nenhum dos dois cabe na
//!   `std`. Quando o servidor pede o caminho completo, este modulo diz
//!   exatamente isso e o que fazer, em vez de falhar com "acesso negado".
//!
//! Em nenhum caso a senha viaja em texto: o que vai na rede e o embaralhado
//! com o sal, que muda a cada conexao.
//!
//! # Limites
//!
//! - Sem TLS. A conversa e em texto claro; use rede interna ou tunel.
//! - Sem protocolo binario nem instrucao preparada -- so `COM_QUERY`.
//! - Sem `CLIENT_MULTI_STATEMENTS`: um pedido e uma instrucao, e um `;` no
//!   meio da consulta nao consegue emendar uma segunda.
//! - Carga acima de 16 MB chega partida em varios quadros; a leitura junta,
//!   a escrita nao parte (nenhuma consulta que este modulo manda chega perto).

use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::hash::sha256;
use phxsql_core::sha1::sha1;

// Capacidades pedidas ao servidor. Cada uma esta aqui por um motivo:
const LONG_PASSWORD: u32 = 0x0000_0001;
const LONG_FLAG: u32 = 0x0000_0004;
const CONNECT_WITH_DB: u32 = 0x0000_0008;
const PROTOCOL_41: u32 = 0x0000_0200;
const TRANSACTIONS: u32 = 0x0000_2000;
const SECURE_CONNECTION: u32 = 0x0000_8000;
const PLUGIN_AUTH: u32 = 0x0008_0000;
const PLUGIN_AUTH_LENENC: u32 = 0x0020_0000;

const COM_QUERY: u8 = 0x03;
const COM_PING: u8 = 0x0e;
const COM_QUIT: u8 = 0x01;

/// Uma coluna do resultado, ja traduzida para nomes que a tela entende.
#[derive(Debug, Clone)]
pub struct Coluna {
    pub nome: String,
    pub tabela: String,
    /// Nome do tipo no MySQL(R), como `INT` ou `VARCHAR`.
    pub tipo: String,
    /// O codigo cru, para quem precisar decidir por ele.
    pub tipo_codigo: u8,
    pub tamanho: u32,
    /// Casas decimais que o servidor declara.
    ///
    /// Existe porque sem ela a tela arredonda: um `DECIMAL(12,2)` de 15000,50
    /// aparecia como 15.001, que e outro numero. Tipo de ponto flutuante vem
    /// com 31 (o "nao fixo" do protocolo) e a tela trata isso a parte.
    pub decimais: u8,
    pub nulavel: bool,
    pub primaria: bool,
    /// Numero, para a tela alinhar a direita e o assistente de pivot oferecer
    /// como medida.
    pub numerico: bool,
}

#[derive(Debug, Default)]
pub struct Resultado {
    pub colunas: Vec<Coluna>,
    /// `None` e NULL de verdade, e nao cadeia vazia -- a diferenca importa.
    pub linhas: Vec<Vec<Option<String>>>,
    pub afetadas: u64,
    /// Verdadeiro quando o teto cortou o resultado.
    pub truncado: bool,
}

pub struct Conexao {
    fluxo: BufReader<TcpStream>,
    escrita: TcpStream,
    sequencia: u8,
    /// Versao anunciada no aperto de mao, para o teste de ligacao mostrar.
    pub versao: String,
    pub conexao_id: u32,
}

impl Conexao {
    /// Conecta, autentica e deixa a ligacao pronta para consulta.
    pub fn abrir(
        host: &str,
        porta: u16,
        usuario: &str,
        senha: &str,
        database: &str,
        espera: Duration,
    ) -> Result<Conexao> {
        let soquete = conectar(host, porta, espera)?;
        soquete
            .set_read_timeout(Some(espera))
            .and_then(|_| soquete.set_write_timeout(Some(espera)))
            .and_then(|_| soquete.set_nodelay(true))
            .map_err(|e| erro(format!("nao consegui armar a conexao: {e}")))?;
        let escrita = soquete
            .try_clone()
            .map_err(|e| erro(format!("nao consegui duplicar o soquete: {e}")))?;
        let mut c = Conexao {
            fluxo: BufReader::new(soquete),
            escrita,
            sequencia: 0,
            versao: String::new(),
            conexao_id: 0,
        };
        c.apertar_a_mao(usuario, senha, database)?;
        Ok(c)
    }

    fn apertar_a_mao(&mut self, usuario: &str, senha: &str, database: &str) -> Result<()> {
        let saudacao = self.ler_quadro()?;
        let (sal, plugin) = self.ler_saudacao(&saudacao)?;

        let resposta = embaralhar(&plugin, senha, &sal)?;
        let capacidades = LONG_PASSWORD
            | LONG_FLAG
            | PROTOCOL_41
            | TRANSACTIONS
            | SECURE_CONNECTION
            | PLUGIN_AUTH
            | PLUGIN_AUTH_LENENC
            | if database.is_empty() {
                0
            } else {
                CONNECT_WITH_DB
            };

        let mut p = Vec::with_capacity(64);
        p.extend_from_slice(&capacidades.to_le_bytes());
        // Teto de pacote: 16 MB, o maximo de um quadro so.
        p.extend_from_slice(&0x0100_0000u32.to_le_bytes());
        p.push(45); // utf8mb4_general_ci
        p.extend_from_slice(&[0u8; 23]);
        cadeia_nula(&mut p, usuario);
        p.push(resposta.len() as u8);
        p.extend_from_slice(&resposta);
        if !database.is_empty() {
            cadeia_nula(&mut p, database);
        }
        cadeia_nula(&mut p, &plugin);
        self.escrever_quadro(&p)?;

        loop {
            let r = self.ler_quadro()?;
            match r.first() {
                Some(0x00) => return Ok(()),
                Some(0xFF) => return Err(self.erro_do_servidor(&r)),
                // Troca de plugin: o servidor manda o nome novo e um sal novo.
                Some(0xFE) => {
                    let mut i = 1;
                    let novo = cadeia_ate_nulo(&r, &mut i);
                    let sal_novo: Vec<u8> = r[i..]
                        .iter()
                        .take_while(|b| **b != 0)
                        .copied()
                        .collect();
                    let resp = embaralhar(&novo, senha, &sal_novo)?;
                    self.escrever_quadro(&resp)?;
                }
                // AuthMoreData do caching_sha2_password.
                Some(0x01) => match r.get(1) {
                    // 3 = a senha ja estava no cache; o OK vem no proximo.
                    Some(0x03) => continue,
                    // 4 = caminho completo. Exige TLS ou a chave RSA, e nenhum
                    // dos dois cabe na std. Dizer isso, com a saida, e melhor
                    // do que um "acesso negado" que manda o operador procurar
                    // senha errada.
                    Some(0x04) => {
                        return Err(erro(format!(
                            "o servidor pediu autenticacao completa do caching_sha2_password para {usuario:?}, \
                             que exige TLS ou a chave RSA -- nenhum dos dois cabe sem dependencia externa. \
                             Saidas: criar o usuario com  ALTER USER '{usuario}'@'%' IDENTIFIED WITH mysql_native_password BY '...'  \
                             ou conectar uma vez com o cliente oficial, o que deixa a senha em cache e libera o caminho rapido \
                             ate o servidor reiniciar"
                        )))
                    }
                    outro => {
                        return Err(erro(format!(
                            "resposta de autenticacao que nao sei ler: 0x01 0x{:02x}",
                            outro.copied().unwrap_or(0)
                        )))
                    }
                },
                outro => {
                    return Err(erro(format!(
                        "resposta inesperada no aperto de mao: 0x{:02x}",
                        outro.copied().unwrap_or(0)
                    )))
                }
            }
        }
    }

    /// Le o pacote de saudacao e devolve `(sal, nome do plugin)`.
    fn ler_saudacao(&mut self, p: &[u8]) -> Result<(Vec<u8>, String)> {
        if p.first() == Some(&0xFF) {
            return Err(self.erro_do_servidor(p));
        }
        if p.first() != Some(&10) {
            return Err(erro(format!(
                "protocolo {} nao suportado (este cliente fala o 10, de MySQL 4.1 em diante)",
                p.first().copied().unwrap_or(0)
            )));
        }
        let mut i = 1;
        self.versao = cadeia_ate_nulo(p, &mut i);
        self.conexao_id = le_u32(p, i)?;
        i += 4;
        // O sal vem partido: 8 bytes aqui, o resto depois dos filtros.
        let mut sal = p
            .get(i..i + 8)
            .ok_or_else(|| erro("saudacao curta demais".into()))?
            .to_vec();
        i += 8 + 1; // + o byte de enchimento
        i += 2 + 1 + 2 + 2; // capacidades baixas, charset, estado, capacidades altas
        let tamanho_sal = *p.get(i).unwrap_or(&0) as usize;
        i += 1 + 10; // + os 10 reservados
        if tamanho_sal > 8 {
            let falta = (tamanho_sal - 8).max(13) - 1;
            if let Some(parte) = p.get(i..i + falta) {
                sal.extend_from_slice(parte);
            }
            i += falta + 1;
        }
        let plugin = cadeia_ate_nulo(p, &mut i);
        Ok((
            sal,
            if plugin.is_empty() {
                "mysql_native_password".into()
            } else {
                plugin
            },
        ))
    }

    /// Manda um `SELECT` (ou qualquer instrucao) e devolve o resultado.
    ///
    /// `teto` corta a leitura de linhas. O corte acontece AQUI e nao no `LIMIT`
    /// de proposito: nem toda instrucao aceita `LIMIT`, e o teto tem de valer
    /// para todas.
    pub fn consultar(&mut self, sql: &str, teto: u64) -> Result<Resultado> {
        self.sequencia = 0;
        let mut p = Vec::with_capacity(sql.len() + 1);
        p.push(COM_QUERY);
        p.extend_from_slice(sql.as_bytes());
        self.escrever_quadro(&p)?;

        let primeiro = self.ler_quadro()?;
        match primeiro.first() {
            Some(0xFF) => return Err(self.erro_do_servidor(&primeiro)),
            // OK: instrucao sem resultado (INSERT, UPDATE, SET...).
            Some(0x00) => {
                let mut i = 1;
                let afetadas = le_lenenc(&primeiro, &mut i).unwrap_or(0);
                return Ok(Resultado {
                    afetadas,
                    ..Default::default()
                });
            }
            // 0xFB seria LOCAL INFILE, que este cliente nao aceita: aceitar
            // seria deixar o servidor do outro lado pedir arquivo DESTA
            // maquina.
            Some(0xFB) => {
                return Err(erro(
                    "o servidor pediu LOCAL INFILE, que este cliente nao atende".into(),
                ))
            }
            None => return Err(erro("resposta vazia".into())),
            _ => {}
        }

        let mut i = 0;
        let quantas = le_lenenc(&primeiro, &mut i)
            .ok_or_else(|| erro("nao entendi quantas colunas o resultado tem".into()))?;
        let mut colunas = Vec::with_capacity(quantas as usize);
        for _ in 0..quantas {
            let q = self.ler_quadro()?;
            colunas.push(ler_coluna(&q)?);
        }
        // Fim das colunas. Sem DEPRECATE_EOF o servidor manda um EOF aqui.
        let marca = self.ler_quadro()?;
        if !eh_eof(&marca) {
            return Err(erro(
                "esperava o fim da definicao das colunas e veio outra coisa".into(),
            ));
        }

        let mut linhas = Vec::new();
        let mut truncado = false;
        loop {
            let q = self.ler_quadro()?;
            if eh_eof(&q) {
                break;
            }
            if q.first() == Some(&0xFF) {
                return Err(self.erro_do_servidor(&q));
            }
            if linhas.len() as u64 >= teto {
                // Ler ate o fim mesmo depois do teto: parar no meio deixaria
                // linhas na conexao e o proximo comando leria resposta alheia.
                truncado = true;
                continue;
            }
            linhas.push(ler_linha(&q, colunas.len()));
        }
        Ok(Resultado {
            colunas,
            linhas,
            afetadas: 0,
            truncado,
        })
    }

    /// Um `ping`: barato, e serve para o botao "testar ligacao".
    pub fn ping(&mut self) -> Result<()> {
        self.sequencia = 0;
        self.escrever_quadro(&[COM_PING])?;
        let r = self.ler_quadro()?;
        match r.first() {
            Some(0x00) => Ok(()),
            Some(0xFF) => Err(self.erro_do_servidor(&r)),
            _ => Err(erro("resposta estranha ao ping".into())),
        }
    }

    pub fn encerrar(&mut self) {
        self.sequencia = 0;
        let _ = self.escrever_quadro(&[COM_QUIT]);
    }

    // ------------------------------------------------------------- quadros

    fn escrever_quadro(&mut self, carga: &[u8]) -> Result<()> {
        let n = carga.len();
        if n >= 0x00FF_FFFF {
            return Err(erro("carga grande demais para um quadro".into()));
        }
        let mut cabeca = [0u8; 4];
        cabeca[..3].copy_from_slice(&(n as u32).to_le_bytes()[..3]);
        cabeca[3] = self.sequencia;
        self.sequencia = self.sequencia.wrapping_add(1);
        self.escrita
            .write_all(&cabeca)
            .and_then(|_| self.escrita.write_all(carga))
            .and_then(|_| self.escrita.flush())
            .map_err(|e| erro(format!("escrita falhou: {e}")))
    }

    /// Le um quadro, juntando as continuacoes de 16 MB.
    fn ler_quadro(&mut self) -> Result<Vec<u8>> {
        let mut carga = Vec::new();
        loop {
            let mut cabeca = [0u8; 4];
            self.fluxo
                .read_exact(&mut cabeca)
                .map_err(|e| erro(format!("leitura falhou: {e}")))?;
            let n = u32::from_le_bytes([cabeca[0], cabeca[1], cabeca[2], 0]) as usize;
            self.sequencia = cabeca[3].wrapping_add(1);
            let inicio = carga.len();
            carga.resize(inicio + n, 0);
            self.fluxo
                .read_exact(&mut carga[inicio..])
                .map_err(|e| erro(format!("leitura falhou no meio do quadro: {e}")))?;
            // Um quadro cheio quer dizer "tem mais": o proximo continua a mesma
            // carga. Parar aqui cortaria resultado grande pela metade.
            if n < 0x00FF_FFFF {
                return Ok(carga);
            }
        }
    }

    /// Traduz o pacote de erro do servidor.
    fn erro_do_servidor(&self, p: &[u8]) -> PhxError {
        let codigo = le_u16(p, 1).unwrap_or(0);
        // Com PROTOCOL_41 vem "#" + 5 letras de estado antes da mensagem.
        let inicio = if p.get(3) == Some(&b'#') { 9 } else { 3 };
        let msg = String::from_utf8_lossy(p.get(inicio..).unwrap_or(&[])).to_string();
        erro(format!("MySQL {codigo}: {msg}"))
    }
}

fn conectar(host: &str, porta: u16, espera: Duration) -> Result<TcpStream> {
    use std::net::ToSocketAddrs;
    let alvo = format!("{host}:{porta}");
    let mut ultimo = "nenhum endereco resolvido".to_string();
    for e in alvo
        .to_socket_addrs()
        .map_err(|e| erro(format!("nao resolvi {alvo}: {e}")))?
    {
        match TcpStream::connect_timeout(&e, espera) {
            Ok(s) => return Ok(s),
            Err(e) => ultimo = e.to_string(),
        }
    }
    Err(erro(format!("nao conectei em {alvo}: {ultimo}")))
}

/// A senha embaralhada com o sal, do jeito que o plugin manda.
fn embaralhar(plugin: &str, senha: &str, sal: &[u8]) -> Result<Vec<u8>> {
    if senha.is_empty() {
        // Senha vazia manda resposta vazia, e nao o embaralhado de "".
        return Ok(Vec::new());
    }
    let sal = &sal[..sal.len().min(20)];
    match plugin {
        "mysql_native_password" => {
            let um = sha1(senha.as_bytes());
            let dois = sha1(&um);
            let mut base = Vec::with_capacity(sal.len() + dois.len());
            base.extend_from_slice(sal);
            base.extend_from_slice(&dois);
            let tres = sha1(&base);
            Ok(um.iter().zip(tres.iter()).map(|(a, b)| a ^ b).collect())
        }
        "caching_sha2_password" => {
            let um = sha256(senha.as_bytes());
            let dois = sha256(&um);
            let mut base = Vec::with_capacity(dois.len() + sal.len());
            base.extend_from_slice(&dois);
            base.extend_from_slice(sal);
            let tres = sha256(&base);
            Ok(um.iter().zip(tres.iter()).map(|(a, b)| a ^ b).collect())
        }
        // `mysql_clear_password` mandaria a senha em texto. Sem TLS isso e
        // entregar a senha para quem estiver no caminho, entao nao.
        outro => Err(erro(format!(
            "plugin de autenticacao {outro:?} nao suportado \
             (este cliente fala mysql_native_password e o caminho rapido do caching_sha2_password)"
        ))),
    }
}

fn eh_eof(p: &[u8]) -> bool {
    // Um valor de linha tambem pode comecar com 0xFE -- quando e o marcador de
    // tamanho de 8 bytes. A diferenca e o tamanho do pacote: EOF nao passa de
    // 9 bytes, e uma linha com valor desse porte passa muito.
    p.first() == Some(&0xFE) && p.len() < 9
}

fn ler_coluna(p: &[u8]) -> Result<Coluna> {
    let mut i = 0;
    pular_lenenc(p, &mut i); // catalogo
    pular_lenenc(p, &mut i); // schema
    let tabela = texto_lenenc(p, &mut i);
    pular_lenenc(p, &mut i); // tabela de origem
    let nome = texto_lenenc(p, &mut i);
    pular_lenenc(p, &mut i); // nome de origem
    le_lenenc(p, &mut i); // comprimento do bloco fixo
    i += 2; // conjunto de caracteres
    let tamanho = le_u32(p, i).unwrap_or(0);
    i += 4;
    let codigo = *p.get(i).unwrap_or(&0);
    i += 1;
    let bandeiras = le_u16(p, i).unwrap_or(0);
    i += 2;
    let decimais = *p.get(i).unwrap_or(&0);
    Ok(Coluna {
        decimais,
        nome,
        tabela,
        tipo: nome_do_tipo(codigo, bandeiras).to_string(),
        tipo_codigo: codigo,
        tamanho,
        nulavel: bandeiras & 0x0001 == 0,
        primaria: bandeiras & 0x0002 != 0,
        numerico: eh_numerico(codigo),
    })
}

fn ler_linha(p: &[u8], quantas: usize) -> Vec<Option<String>> {
    let mut i = 0;
    let mut v = Vec::with_capacity(quantas);
    for _ in 0..quantas {
        match p.get(i) {
            // 0xFB e NULL. Guardar como None, e nao como "", porque um texto
            // vazio e um valor e NULL e a ausencia dele.
            Some(0xFB) => {
                i += 1;
                v.push(None);
            }
            Some(_) => v.push(Some(texto_lenenc(p, &mut i))),
            None => v.push(None),
        }
    }
    v
}

fn nome_do_tipo(codigo: u8, bandeiras: u16) -> &'static str {
    let binario = bandeiras & 0x0080 != 0;
    match codigo {
        0x00 | 0xf6 => "DECIMAL",
        0x01 => "TINYINT",
        0x02 => "SMALLINT",
        0x03 => "INT",
        0x04 => "FLOAT",
        0x05 => "DOUBLE",
        0x07 => "TIMESTAMP",
        0x08 => "BIGINT",
        0x09 => "MEDIUMINT",
        0x0a => "DATE",
        0x0b => "TIME",
        0x0c => "DATETIME",
        0x0d => "YEAR",
        0x0f | 0xfd => {
            if binario {
                "VARBINARY"
            } else {
                "VARCHAR"
            }
        }
        0x10 => "BIT",
        0xf5 => "JSON",
        0xf7 => "ENUM",
        0xf8 => "SET",
        0xf9..=0xfc => {
            if binario {
                "BLOB"
            } else {
                "TEXT"
            }
        }
        0xfe => {
            if binario {
                "BINARY"
            } else {
                "CHAR"
            }
        }
        0xff => "GEOMETRY",
        _ => "DESCONHECIDO",
    }
}

fn eh_numerico(codigo: u8) -> bool {
    matches!(
        codigo,
        0x00 | 0x01 | 0x02 | 0x03 | 0x04 | 0x05 | 0x08 | 0x09 | 0x0d | 0xf6
    )
}

// --------------------------------------------------------------- leitores

fn cadeia_nula(destino: &mut Vec<u8>, texto: &str) {
    destino.extend_from_slice(texto.as_bytes());
    destino.push(0);
}

fn cadeia_ate_nulo(p: &[u8], i: &mut usize) -> String {
    let inicio = *i;
    while *i < p.len() && p[*i] != 0 {
        *i += 1;
    }
    let s = String::from_utf8_lossy(&p[inicio..*i]).to_string();
    *i += 1;
    s
}

/// Inteiro de tamanho variavel: o primeiro byte diz quantos vem depois.
fn le_lenenc(p: &[u8], i: &mut usize) -> Option<u64> {
    let primeiro = *p.get(*i)?;
    *i += 1;
    Some(match primeiro {
        0..=0xFA => primeiro as u64,
        0xFB => return None,
        0xFC => {
            let v = le_u16(p, *i)? as u64;
            *i += 2;
            v
        }
        0xFD => {
            let v = u32::from_le_bytes([*p.get(*i)?, *p.get(*i + 1)?, *p.get(*i + 2)?, 0]) as u64;
            *i += 3;
            v
        }
        _ => {
            let mut b = [0u8; 8];
            b.copy_from_slice(p.get(*i..*i + 8)?);
            *i += 8;
            u64::from_le_bytes(b)
        }
    })
}

fn texto_lenenc(p: &[u8], i: &mut usize) -> String {
    let n = le_lenenc(p, i).unwrap_or(0) as usize;
    let fim = (*i + n).min(p.len());
    let s = String::from_utf8_lossy(&p[*i..fim]).to_string();
    *i = fim;
    s
}

fn pular_lenenc(p: &[u8], i: &mut usize) {
    let n = le_lenenc(p, i).unwrap_or(0) as usize;
    *i = (*i + n).min(p.len());
}

fn le_u16(p: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*p.get(i)?, *p.get(i + 1)?]))
}

fn le_u32(p: &[u8], i: usize) -> Result<u32> {
    Ok(u32::from_le_bytes([
        *p.get(i).ok_or_else(|| erro("pacote curto".into()))?,
        *p.get(i + 1).ok_or_else(|| erro("pacote curto".into()))?,
        *p.get(i + 2).ok_or_else(|| erro("pacote curto".into()))?,
        *p.get(i + 3).ok_or_else(|| erro("pacote curto".into()))?,
    ]))
}

fn erro(m: String) -> PhxError {
    PhxError::Esquema(format!("dblink mysql: {m}"))
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Vetor conhecido do `mysql_native_password`, com sal e senha fixos.
    ///
    /// Conferido refazendo a conta a mao: o resultado tem de bater com
    /// `SHA1(senha) XOR SHA1(sal || SHA1(SHA1(senha)))`.
    #[test]
    fn o_embaralhado_nativo_segue_a_formula() {
        let sal: Vec<u8> = (1u8..=20).collect();
        let r = embaralhar("mysql_native_password", "senha-do-link", &sal).unwrap();
        let um = sha1(b"senha-do-link");
        let mut base = sal.clone();
        base.extend_from_slice(&sha1(&um));
        let tres = sha1(&base);
        let esperado: Vec<u8> = um.iter().zip(tres.iter()).map(|(a, b)| a ^ b).collect();
        assert_eq!(r, esperado);
        assert_eq!(r.len(), 20);
    }

    #[test]
    fn o_embaralhado_sha2_segue_a_formula() {
        let sal: Vec<u8> = (1u8..=20).collect();
        let r = embaralhar("caching_sha2_password", "senha-do-link", &sal).unwrap();
        let um = sha256(b"senha-do-link");
        let mut base = sha256(&um).to_vec();
        base.extend_from_slice(&sal);
        let tres = sha256(&base);
        let esperado: Vec<u8> = um.iter().zip(tres.iter()).map(|(a, b)| a ^ b).collect();
        assert_eq!(r, esperado);
        assert_eq!(r.len(), 32);
    }

    /// Senha vazia manda resposta vazia. Embaralhar "" daria 20 bytes que o
    /// servidor recusaria.
    #[test]
    fn senha_vazia_manda_resposta_vazia() {
        assert!(embaralhar("mysql_native_password", "", &[1, 2, 3])
            .unwrap()
            .is_empty());
    }

    /// Mandar a senha em texto seria entrega-la a quem estiver no caminho.
    #[test]
    fn plugin_de_senha_em_texto_e_recusado() {
        let e = embaralhar("mysql_clear_password", "x", &[1, 2, 3]).unwrap_err();
        assert!(e.to_string().contains("nao suportado"), "{e}");
    }

    /// O byte 0xFE e ambiguo: fim de resultado ou marcador de tamanho longo. O
    /// tamanho do pacote e o que desempata, e errar aqui cortaria o resultado.
    #[test]
    fn eof_se_distingue_de_valor_longo() {
        assert!(eh_eof(&[0xFE, 0, 0, 0, 0]));
        let mut linha = vec![0xFE];
        linha.extend_from_slice(&1000u64.to_le_bytes());
        linha.extend_from_slice(&[b'x'; 1000]);
        assert!(!eh_eof(&linha));
    }

    #[test]
    fn o_inteiro_de_tamanho_variavel_le_as_quatro_formas() {
        let mut i = 0;
        assert_eq!(le_lenenc(&[42], &mut i), Some(42));
        i = 0;
        assert_eq!(le_lenenc(&[0xFC, 0x10, 0x27], &mut i), Some(10_000));
        assert_eq!(i, 3);
        i = 0;
        assert_eq!(
            le_lenenc(&[0xFD, 0x40, 0x42, 0x0F], &mut i),
            Some(1_000_000)
        );
        assert_eq!(i, 4);
        i = 0;
        assert_eq!(le_lenenc(&[0xFB], &mut i), None);
    }

    /// NULL e cadeia vazia sao coisas diferentes, e a linha tem de guardar a
    /// diferenca.
    #[test]
    fn nulo_nao_vira_texto_vazio() {
        // Uma linha com tres campos: "ab", NULL, "".
        let linha = vec![2, b'a', b'b', 0xFB, 0];
        let v = ler_linha(&linha, 3);
        assert_eq!(v[0], Some("ab".to_string()));
        assert_eq!(v[1], None);
        assert_eq!(v[2], Some(String::new()));
    }
}
