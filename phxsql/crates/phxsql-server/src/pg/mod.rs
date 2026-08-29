//! O protocolo de fio do PostgreSQL(R) escrito a mao, so com a `std`.
//!
//! # Por que a mao
//!
//! Pela regra do projeto: nenhuma crate externa. E, como no cliente MySQL(R)
//! ao lado, o protocolo e pequeno quando o escopo e pequeno -- conectar,
//! autenticar e mandar consulta de texto.
//!
//! Os tijolos da autenticacao ja estavam prontos: o SCRAM-SHA-256 que o
//! PostgreSQL(R) 10+ usa e feito de SHA-256, HMAC e PBKDF2, e os tres foram
//! escritos aqui para o hash de senha do `config.json`. Ver [`scram`].
//!
//! # A mensagem
//!
//! Tudo, fora a de abertura, viaja como `1 byte de tipo + int32 de tamanho +
//! carga`. O tamanho **inclui os proprios 4 bytes** e nao inclui o byte de
//! tipo -- e o erro classico de quem escreve este protocolo pela primeira vez.
//! Numeros sao *big endian*, ao contrario do MySQL(R).
//!
//! A mensagem de abertura (`StartupMessage`) nao tem byte de tipo: comeca
//! direto pelo tamanho, e o "tipo" dela e a versao do protocolo que vem em
//! seguida. E assim porque ela e mais velha que o byte de tipo.
//!
//! # A conversa de uma consulta
//!
//! ```text
//! cliente  -> Q  "select ..."
//! servidor -> T  RowDescription   (as colunas)
//! servidor -> D  DataRow          (uma por linha)
//! servidor -> C  CommandComplete  ("SELECT 12")
//! servidor -> Z  ReadyForQuery    (fim do ciclo)
//! ```
//!
//! O `Z` e o que fecha o ciclo. Parar de ler no `C` deixaria o `Z` na fila e a
//! consulta seguinte leria a resposta da anterior -- que e o defeito mais
//! dificil de achar neste protocolo, porque tudo "funciona" com um desencontro
//! constante de uma mensagem.
//!
//! # Limites
//!
//! - **Sem TLS.** A conversa e em texto claro; use rede interna ou tunel.
//!   (A senha, essa, nunca vai em claro -- ver abaixo.)
//! - **So consulta simples** (`Q`). Sem protocolo estendido, sem `Parse`/
//!   `Bind`, sem instrucao preparada, sem cursor.
//! - **So SCRAM-SHA-256.** `md5` exige MD5, que este projeto nao tem e nao vai
//!   ter -- MD5 esta quebrado. `password` (texto puro) e RECUSADO de proposito:
//!   a regra do projeto e que senha nao viaja em claro, e um cliente que
//!   aceitasse isso calado tiraria a decisao de quem configurou o servidor.
//!   Nos dois casos a mensagem diz o que mudar no `pg_hba.conf`.
//! - **Tudo em texto.** O formato binario de resultado nao e pedido, entao
//!   todo valor chega como texto, que e o que a tela mostra de qualquer jeito.

pub mod scram;

use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use phxsql_core::error::{PhxError, Result};

/// Versao 3.0 do protocolo, a mesma desde o PostgreSQL(R) 7.4.
const PROTOCOLO_3: i32 = 196_608;

fn erro(msg: String) -> PhxError {
    PhxError::Esquema(msg)
}

/// Uma coluna do resultado.
#[derive(Debug, Clone)]
pub struct Coluna {
    pub nome: String,
    /// OID do tipo no catalogo do PostgreSQL(R).
    pub tipo_oid: u32,
    /// Nome do tipo, quando conhecido. `oid:1234` quando nao.
    pub tipo: String,
    /// Tamanho declarado; negativo para tipo de tamanho variavel.
    pub tamanho: i16,
    /// Numero, para a tela alinhar a direita e o pivot oferecer como medida.
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
    /// `server_version` anunciado no `ParameterStatus`, para o teste mostrar.
    pub versao: String,
    /// PID do processo do servidor que atende esta conexao.
    pub conexao_id: u32,
}

/// Uma mensagem lida do servidor: o byte de tipo e a carga, sem o tamanho.
struct Mensagem {
    tipo: u8,
    corpo: Vec<u8>,
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
            versao: String::new(),
            conexao_id: 0,
        };
        c.apertar_a_mao(usuario, senha, database)?;
        Ok(c)
    }

    fn apertar_a_mao(&mut self, usuario: &str, senha: &str, database: &str) -> Result<()> {
        let mut p = Vec::with_capacity(128);
        p.extend_from_slice(&PROTOCOLO_3.to_be_bytes());
        parametro(&mut p, "user", usuario);
        if !database.is_empty() {
            parametro(&mut p, "database", database);
        }
        parametro(&mut p, "client_encoding", "UTF8");
        // O `application_name` aparece no `pg_stat_activity` do outro lado:
        // quem administra o PostgreSQL(R) precisa saber quem esta consultando.
        parametro(&mut p, "application_name", "PhxSql DbLink");
        p.push(0);
        self.escrever_sem_tipo(&p)?;

        loop {
            let m = self.ler_mensagem()?;
            match m.tipo {
                b'R' => {
                    if self.autenticar(&m.corpo, usuario, senha)? {
                        continue;
                    }
                }
                b'S' => self.anotar_parametro(&m.corpo),
                b'K' => {
                    if m.corpo.len() >= 4 {
                        self.conexao_id =
                            u32::from_be_bytes([m.corpo[0], m.corpo[1], m.corpo[2], m.corpo[3]]);
                    }
                }
                b'Z' => return Ok(()),
                b'E' => return Err(erro_do_servidor(&m.corpo)),
                // Aviso do servidor nao interrompe o aperto de mao.
                b'N' => {}
                outro => {
                    return Err(erro(format!(
                        "mensagem inesperada no aperto de mao: {:?}",
                        outro as char
                    )))
                }
            }
        }
    }

    /// Trata uma mensagem `R`. Devolve `true` quando a conversa continua.
    fn autenticar(&mut self, corpo: &[u8], usuario: &str, senha: &str) -> Result<bool> {
        let codigo = i32::from_be_bytes(
            corpo
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or_else(|| erro("mensagem de autenticacao truncada".into()))?,
        );
        match codigo {
            // AuthenticationOk. O `Z` ainda vem depois.
            0 => Ok(true),
            3 => Err(PhxError::Autorizacao(
                "o servidor pediu a senha em TEXTO PURO (metodo `password`), e este \
                 cliente recusa: sem TLS ela iria legivel no fio. Troque a linha \
                 deste usuario no `pg_hba.conf` para `scram-sha-256`."
                    .into(),
            )),
            5 => Err(PhxError::Autorizacao(
                "o servidor pediu autenticacao `md5`, que este cliente nao faz: o \
                 MD5 esta quebrado e nao entra neste projeto. Troque a linha deste \
                 usuario no `pg_hba.conf` para `scram-sha-256` e regrave a senha \
                 com `\\password`."
                    .into(),
            )),
            // SASL: a lista de mecanismos, cada um terminado em NUL.
            10 => {
                let mecanismos = cadeias_nulas(&corpo[4..]);
                if !mecanismos.iter().any(|m| m == "SCRAM-SHA-256") {
                    return Err(PhxError::Autorizacao(format!(
                        "o servidor so oferece {mecanismos:?}, e este cliente faz \
                         SCRAM-SHA-256. (`SCRAM-SHA-256-PLUS` exige TLS.)"
                    )));
                }
                self.negociar_scram(usuario, senha)?;
                Ok(true)
            }
            outro => Err(PhxError::Autorizacao(format!(
                "metodo de autenticacao {outro} nao suportado por este cliente"
            ))),
        }
    }

    /// A troca de tres mensagens do SCRAM, ja dentro do envelope do PostgreSQL.
    fn negociar_scram(&mut self, _usuario: &str, senha: &str) -> Result<()> {
        let (mut s, primeira) = scram::Scram::comecar(&scram::nonce());

        // SASLInitialResponse: nome do mecanismo, tamanho da resposta, resposta.
        let mut p = Vec::with_capacity(64 + primeira.len());
        cadeia_nula(&mut p, "SCRAM-SHA-256");
        p.extend_from_slice(&(primeira.len() as i32).to_be_bytes());
        p.extend_from_slice(primeira.as_bytes());
        self.escrever(b'p', &p)?;

        // SASLContinue.
        let m = self.esperar_autenticacao(11)?;
        let servidor_primeira = String::from_utf8_lossy(&m).into_owned();
        let final_do_cliente = s.responder(senha, &servidor_primeira)?;
        self.escrever(b'p', final_do_cliente.as_bytes())?;

        // SASLFinal -- e a conferencia que fecha a autenticacao MUTUA.
        let m = self.esperar_autenticacao(12)?;
        s.conferir_servidor(&String::from_utf8_lossy(&m))
    }

    /// Le a proxima `R` e exige que seja do codigo esperado. Devolve a carga.
    fn esperar_autenticacao(&mut self, codigo: i32) -> Result<Vec<u8>> {
        loop {
            let m = self.ler_mensagem()?;
            match m.tipo {
                b'E' => return Err(erro_do_servidor(&m.corpo)),
                b'N' | b'S' => continue,
                b'R' => {
                    let vindo = i32::from_be_bytes(
                        m.corpo
                            .get(..4)
                            .and_then(|b| b.try_into().ok())
                            .ok_or_else(|| erro("mensagem SASL truncada".into()))?,
                    );
                    if vindo != codigo {
                        return Err(PhxError::Autorizacao(format!(
                            "esperava a etapa SASL {codigo} e veio {vindo}"
                        )));
                    }
                    return Ok(m.corpo[4..].to_vec());
                }
                outro => {
                    return Err(erro(format!(
                        "mensagem inesperada durante o SASL: {:?}",
                        outro as char
                    )))
                }
            }
        }
    }

    fn anotar_parametro(&mut self, corpo: &[u8]) {
        let campos = cadeias_nulas(corpo);
        if campos.len() >= 2 && campos[0] == "server_version" {
            self.versao = campos[1].clone();
        }
    }

    /// Manda uma consulta de texto e le o ciclo inteiro, ate o `ReadyForQuery`.
    ///
    /// O `teto` corta as linhas, e nao a consulta: o servidor ja mandou tudo, e
    /// parar de LER deixaria a conexao fora de sincronia. Le-se ate o fim e
    /// guarda-se ate o teto -- e `truncado` diz que houve corte.
    pub fn consultar(&mut self, sql: &str, teto: u64) -> Result<Resultado> {
        let mut p = Vec::with_capacity(sql.len() + 1);
        cadeia_nula(&mut p, sql);
        self.escrever(b'Q', &p)?;

        let mut r = Resultado::default();
        let mut falha: Option<PhxError> = None;
        loop {
            let m = self.ler_mensagem()?;
            match m.tipo {
                b'T' => r.colunas = ler_descricao(&m.corpo)?,
                b'D' => {
                    let linha = ler_linha(&m.corpo)?;
                    if (r.linhas.len() as u64) < teto {
                        r.linhas.push(linha);
                    } else {
                        r.truncado = true;
                    }
                }
                b'C' => r.afetadas = afetadas_do_rotulo(&m.corpo),
                // Guarda o erro e CONTINUA lendo ate o `Z`: sair aqui deixaria
                // o `Z` na fila, e a proxima consulta leria a resposta desta.
                b'E' => falha = Some(erro_do_servidor(&m.corpo)),
                b'Z' => break,
                // Aviso, consulta vazia, sem dado, cursor: nada a fazer.
                b'N' | b'I' | b'n' | b's' => {}
                _ => {}
            }
        }
        match falha {
            Some(e) => Err(e),
            None => Ok(r),
        }
    }

    /// Uma ida e volta barata, para o teste de ligacao.
    pub fn ping(&mut self) -> Result<()> {
        self.consultar("SELECT 1", 1).map(|_| ())
    }

    pub fn encerrar(&mut self) {
        let _ = self.escrever(b'X', &[]);
    }

    // ------------------------------------------------------------ mensagens

    fn escrever(&mut self, tipo: u8, carga: &[u8]) -> Result<()> {
        let mut m = Vec::with_capacity(carga.len() + 5);
        m.push(tipo);
        m.extend_from_slice(&((carga.len() + 4) as i32).to_be_bytes());
        m.extend_from_slice(carga);
        self.escrita
            .write_all(&m)
            .and_then(|_| self.escrita.flush())
            .map_err(|e| erro(format!("nao consegui escrever: {e}")))
    }

    /// So a de abertura: sem byte de tipo, e o tamanho ja contando a si mesmo.
    fn escrever_sem_tipo(&mut self, carga: &[u8]) -> Result<()> {
        let mut m = Vec::with_capacity(carga.len() + 4);
        m.extend_from_slice(&((carga.len() + 4) as i32).to_be_bytes());
        m.extend_from_slice(carga);
        self.escrita
            .write_all(&m)
            .and_then(|_| self.escrita.flush())
            .map_err(|e| erro(format!("nao consegui escrever: {e}")))
    }

    fn ler_mensagem(&mut self) -> Result<Mensagem> {
        let mut cabecalho = [0u8; 5];
        self.fluxo
            .read_exact(&mut cabecalho)
            .map_err(|e| erro(format!("conexao caiu esperando resposta: {e}")))?;
        let tamanho = i32::from_be_bytes([cabecalho[1], cabecalho[2], cabecalho[3], cabecalho[4]]);
        // O tamanho inclui os 4 bytes dele mesmo. Menor que 4 e mensagem
        // corrompida -- e sem esta conferencia viraria um `usize` gigante.
        if !(4..=64 * 1024 * 1024).contains(&tamanho) {
            return Err(erro(format!(
                "tamanho de mensagem fora do razoavel: {tamanho}"
            )));
        }
        let mut corpo = vec![0u8; tamanho as usize - 4];
        self.fluxo
            .read_exact(&mut corpo)
            .map_err(|e| erro(format!("conexao caiu no meio da mensagem: {e}")))?;
        Ok(Mensagem {
            tipo: cabecalho[0],
            corpo,
        })
    }
}

// ------------------------------------------------------------------- leitura

fn ler_descricao(corpo: &[u8]) -> Result<Vec<Coluna>> {
    let mut l = Leitor::novo(corpo);
    let n = l.i16()? as usize;
    let mut colunas = Vec::with_capacity(n);
    for _ in 0..n {
        let nome = l.cadeia()?;
        let _tabela_oid = l.i32()?;
        let _atributo = l.i16()?;
        let tipo_oid = l.i32()? as u32;
        let tamanho = l.i16()?;
        let _modificador = l.i32()?;
        let _formato = l.i16()?;
        colunas.push(Coluna {
            nome,
            tipo_oid,
            tipo: nome_do_tipo(tipo_oid),
            tamanho,
            numerico: e_numerico(tipo_oid),
        });
    }
    Ok(colunas)
}

fn ler_linha(corpo: &[u8]) -> Result<Vec<Option<String>>> {
    let mut l = Leitor::novo(corpo);
    let n = l.i16()? as usize;
    let mut linha = Vec::with_capacity(n);
    for _ in 0..n {
        let tam = l.i32()?;
        if tam < 0 {
            // -1 e NULL de verdade. Cadeia vazia tem tamanho 0, e nao e a
            // mesma coisa -- confundir as duas troca o dado.
            linha.push(None);
        } else {
            let b = l.bytes(tam as usize)?;
            linha.push(Some(String::from_utf8_lossy(b).into_owned()));
        }
    }
    Ok(linha)
}

/// `CommandComplete` traz "INSERT 0 12", "UPDATE 3", "SELECT 7".
fn afetadas_do_rotulo(corpo: &[u8]) -> u64 {
    String::from_utf8_lossy(corpo)
        .trim_end_matches('\0')
        .split_whitespace()
        .next_back()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// `ErrorResponse`: campos marcados por uma letra, terminados por NUL, e um
/// NUL sozinho no fim.
fn erro_do_servidor(corpo: &[u8]) -> PhxError {
    let (mut severidade, mut mensagem, mut codigo, mut detalhe) =
        (String::new(), String::new(), String::new(), String::new());
    let mut i = 0;
    while i < corpo.len() && corpo[i] != 0 {
        let marca = corpo[i];
        let fim = corpo[i + 1..]
            .iter()
            .position(|b| *b == 0)
            .map(|p| i + 1 + p)
            .unwrap_or(corpo.len());
        let valor = String::from_utf8_lossy(&corpo[i + 1..fim]).into_owned();
        match marca {
            b'S' | b'V' => severidade = valor,
            b'C' => codigo = valor,
            b'M' => mensagem = valor,
            b'D' => detalhe = valor,
            _ => {}
        }
        i = fim + 1;
    }
    let mut texto = format!("PostgreSQL {severidade} {codigo}: {mensagem}");
    if !detalhe.is_empty() {
        texto.push_str(&format!(" ({detalhe})"));
    }
    // `28xxx` e a familia de autorizacao do SQLSTATE; separar aqui faz o erro
    // sair pelo codigo certo do PhxSql em vez de virar "esquema".
    if codigo.starts_with("28") {
        PhxError::Autorizacao(texto)
    } else {
        erro(texto)
    }
}

struct Leitor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Leitor<'a> {
    fn novo(buf: &'a [u8]) -> Leitor<'a> {
        Leitor { buf, pos: 0 }
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let fim = self
            .pos
            .checked_add(n)
            .filter(|f| *f <= self.buf.len())
            .ok_or_else(|| erro("mensagem do PostgreSQL truncada".into()))?;
        let s = &self.buf[self.pos..fim];
        self.pos = fim;
        Ok(s)
    }
    fn i16(&mut self) -> Result<i16> {
        let b = self.bytes(2)?;
        Ok(i16::from_be_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> Result<i32> {
        let b = self.bytes(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn cadeia(&mut self) -> Result<String> {
        let fim = self.buf[self.pos..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| erro("cadeia sem terminador".into()))?;
        let s = String::from_utf8_lossy(&self.buf[self.pos..self.pos + fim]).into_owned();
        self.pos += fim + 1;
        Ok(s)
    }
}

// -------------------------------------------------------------------- tipos

/// Os OIDs sao fixos no catalogo do PostgreSQL(R) e nao mudam entre versoes.
fn nome_do_tipo(oid: u32) -> String {
    let n = match oid {
        16 => "BOOL",
        17 => "BYTEA",
        18 => "CHAR",
        19 => "NAME",
        20 => "INT8",
        21 => "INT2",
        23 => "INT4",
        25 => "TEXT",
        26 => "OID",
        114 => "JSON",
        700 => "FLOAT4",
        701 => "FLOAT8",
        1042 => "BPCHAR",
        1043 => "VARCHAR",
        1082 => "DATE",
        1083 => "TIME",
        1114 => "TIMESTAMP",
        1184 => "TIMESTAMPTZ",
        1186 => "INTERVAL",
        1700 => "NUMERIC",
        2950 => "UUID",
        3802 => "JSONB",
        _ => return format!("oid:{oid}"),
    };
    n.to_string()
}

fn e_numerico(oid: u32) -> bool {
    matches!(oid, 20 | 21 | 23 | 26 | 700 | 701 | 1700)
}

// ------------------------------------------------------------------ auxilio

fn cadeia_nula(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

fn parametro(out: &mut Vec<u8>, chave: &str, valor: &str) {
    cadeia_nula(out, chave);
    cadeia_nula(out, valor);
}

/// Cadeias terminadas em NUL, uma atras da outra, ate acabar ou vir um NUL so.
fn cadeias_nulas(buf: &[u8]) -> Vec<String> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < buf.len() && buf[i] != 0 {
        let fim = buf[i..]
            .iter()
            .position(|b| *b == 0)
            .map(|p| i + p)
            .unwrap_or(buf.len());
        v.push(String::from_utf8_lossy(&buf[i..fim]).into_owned());
        i = fim + 1;
    }
    v
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
            Err(err) => ultimo = err.to_string(),
        }
    }
    Err(erro(format!("nao conectei em {alvo}: {ultimo}")))
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O tamanho inclui os proprios 4 bytes. Errar isso por um e o defeito
    /// classico deste protocolo, e ele nao aparece em nenhum teste que nao
    /// conte os bytes.
    #[test]
    fn o_tamanho_da_mensagem_conta_a_si_mesmo() {
        let carga = b"select 1\0";
        let esperado = (carga.len() + 4) as i32;
        let mut m = Vec::new();
        m.push(b'Q');
        m.extend_from_slice(&esperado.to_be_bytes());
        m.extend_from_slice(carga);
        assert_eq!(m.len(), carga.len() + 5);
        assert_eq!(i32::from_be_bytes([m[1], m[2], m[3], m[4]]), esperado);
    }

    #[test]
    fn a_abertura_leva_os_parametros_e_termina_em_zero() {
        let mut p = Vec::new();
        p.extend_from_slice(&PROTOCOLO_3.to_be_bytes());
        parametro(&mut p, "user", "adriano");
        parametro(&mut p, "database", "loja");
        p.push(0);
        assert_eq!(
            i32::from_be_bytes([p[0], p[1], p[2], p[3]]),
            196_608,
            "a versao do protocolo mudou"
        );
        assert_eq!(*p.last().unwrap(), 0, "faltou o NUL que fecha a lista");
        assert_eq!(
            cadeias_nulas(&p[4..]),
            vec!["user", "adriano", "database", "loja"]
        );
    }

    #[test]
    fn a_descricao_das_colunas_vira_coluna() {
        // Uma RowDescription com duas colunas, montada byte a byte.
        let mut c = Vec::new();
        c.extend_from_slice(&2i16.to_be_bytes());
        for (nome, oid, tam) in [("id", 20u32, 8i16), ("nome", 1043u32, -1i16)] {
            cadeia_nula(&mut c, nome);
            c.extend_from_slice(&0i32.to_be_bytes()); // tabela
            c.extend_from_slice(&0i16.to_be_bytes()); // atributo
            c.extend_from_slice(&(oid as i32).to_be_bytes());
            c.extend_from_slice(&tam.to_be_bytes());
            c.extend_from_slice(&(-1i32).to_be_bytes()); // modificador
            c.extend_from_slice(&0i16.to_be_bytes()); // formato: texto
        }
        let cols = ler_descricao(&c).unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].nome, "id");
        assert_eq!(cols[0].tipo, "INT8");
        assert!(cols[0].numerico);
        assert_eq!(cols[1].tipo, "VARCHAR");
        assert!(!cols[1].numerico, "VARCHAR nao e numero");
    }

    /// NULL e cadeia vazia sao coisas diferentes, e o protocolo as separa por
    /// -1 contra 0. Confundir as duas troca o dado sem ninguem perceber.
    #[test]
    fn nulo_e_vazio_nao_se_confundem() {
        let mut d = Vec::new();
        d.extend_from_slice(&3i16.to_be_bytes());
        d.extend_from_slice(&(-1i32).to_be_bytes()); // NULL
        d.extend_from_slice(&0i32.to_be_bytes()); // ""
        d.extend_from_slice(&2i32.to_be_bytes());
        d.extend_from_slice(b"oi");

        let linha = ler_linha(&d).unwrap();
        assert_eq!(linha[0], None);
        assert_eq!(linha[1], Some(String::new()));
        assert_eq!(linha[2], Some("oi".into()));
    }

    #[test]
    fn linha_truncada_nao_estoura() {
        let mut d = Vec::new();
        d.extend_from_slice(&2i16.to_be_bytes());
        d.extend_from_slice(&10i32.to_be_bytes());
        d.extend_from_slice(b"cur"); // menos bytes do que o declarado
        assert!(ler_linha(&d).is_err());
    }

    #[test]
    fn o_rotulo_do_comando_da_as_linhas_afetadas() {
        assert_eq!(afetadas_do_rotulo(b"INSERT 0 12\0"), 12);
        assert_eq!(afetadas_do_rotulo(b"UPDATE 3\0"), 3);
        assert_eq!(afetadas_do_rotulo(b"SELECT 7\0"), 7);
        assert_eq!(afetadas_do_rotulo(b"BEGIN\0"), 0);
    }

    /// Erro de credencial tem de sair como `Autorizacao`, e nao como
    /// "esquema": e o codigo que o cliente do PhxSql trata.
    #[test]
    fn erro_de_autorizacao_sai_pelo_codigo_certo() {
        let mut c = Vec::new();
        c.push(b'S');
        c.extend_from_slice(b"FATAL\0");
        c.push(b'C');
        c.extend_from_slice(b"28P01\0");
        c.push(b'M');
        c.extend_from_slice(b"password authentication failed for user \"x\"\0");
        c.push(0);

        let e = erro_do_servidor(&c);
        assert!(matches!(e, PhxError::Autorizacao(_)), "{e}");
        assert!(format!("{e}").contains("28P01"), "{e}");
    }

    #[test]
    fn erro_comum_nao_vira_autorizacao() {
        let mut c = Vec::new();
        c.push(b'C');
        c.extend_from_slice(b"42P01\0");
        c.push(b'M');
        c.extend_from_slice(b"relation \"nao_existe\" does not exist\0");
        c.push(0);
        assert!(!matches!(erro_do_servidor(&c), PhxError::Autorizacao(_)));
    }

    #[test]
    fn a_lista_de_mecanismos_sasl_se_le() {
        let mut c = Vec::new();
        c.extend_from_slice(&10i32.to_be_bytes());
        cadeia_nula(&mut c, "SCRAM-SHA-256-PLUS");
        cadeia_nula(&mut c, "SCRAM-SHA-256");
        c.push(0);
        let m = cadeias_nulas(&c[4..]);
        assert_eq!(m, vec!["SCRAM-SHA-256-PLUS", "SCRAM-SHA-256"]);
        assert!(m.iter().any(|x| x == "SCRAM-SHA-256"));
    }
}
