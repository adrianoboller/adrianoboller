//! A cifra do fio: aperto de mao estilo Noise, e a camada de registro.
//!
//! O desenho inteiro, com o argumento de cada escolha, esta em
//! `docs/CIFRA-DO-FIO.md`. Aqui fica o resumo do que este arquivo faz.
//!
//! # O padrao
//!
//! `Noise_NX_25519_ChaChaPoly_SHA256`:
//!
//! ```text
//! -> e
//! <- e, ee, s, es
//! ```
//!
//! Duas mensagens, um ida-e-volta. O servidor tem uma chave estatica; ela
//! viaja CIFRADA na segunda mensagem, e a etiqueta final so fecha se quem
//! respondeu tiver mesmo a privada dela -- e isso que autentica o servidor. O
//! cliente e anonimo: quem responde "quem e voce" continua sendo o
//! desafio-resposta (`desafio.rs`), agora por dentro do tunel.
//!
//! Cliente com PINO da estatica compara e recusa se nao bater (o efeito do
//! `NK`); cliente sem pino aprende na primeira vez (TOFU, como o `known_hosts`
//! do SSH) -- e o TOFU nao protege da primeira conexao, o que esta escrito no
//! documento em vez de escondido.
//!
//! # O que este arquivo NAO promete
//!
//! Interoperabilidade com outras implementacoes de Noise. Os tijolos sao de
//! norma e conferidos contra vetor oficial -- X25519 (RFC 7748), HKDF
//! (RFC 5869), ChaCha20-Poly1305 (RFC 8439), SHA-256 (FIPS 180-4). A
//! COMPOSICAO segue o padrao NX da especificacao Noise, mas nao foi rodada
//! contra os vetores de interoperabilidade do Noise. O que esta provado e que
//! os dois lados daqui fecham, e que um cliente escrito de novo em Python (o
//! da bancada) fecha com este servidor.

use std::io::{BufRead, Write};

use crate::base64;
use crate::cifra::{self, CHAVE_LEN, NONCE_LEN, TAG_LEN};
use crate::error::{PhxError, Result};
use crate::hash::sha256;
use crate::hkdf;
use crate::x25519;

/// O nome do protocolo, que e a semente do hash da transcricao.
///
/// Tem exatamente 32 bytes -- do tamanho do hash --, entao entra como esta, sem
/// passar pelo SHA-256. E a regra do Noise para nome curto.
pub const NOME: &[u8; 32] = b"Noise_NX_25519_ChaChaPoly_SHA256";

/// O prologo: o que amarra este aperto a ESTE protocolo e a esta versao.
///
/// Entra no hash da transcricao antes de tudo. Dois lados com prologos
/// diferentes nao fecham o aperto -- que e exatamente o que se quer no dia em
/// que a versao 2 existir e alguem apontar um cliente novo para um servidor
/// velho.
pub const PROLOGO: &[u8] = b"phxsql-fio-v1";

/// Tamanho da primeira mensagem: so a efemera do cliente.
pub const M1_LEN: usize = 32;
/// Tamanho da segunda: efemera (32) + estatica cifrada (32+16) + etiqueta (16).
pub const M2_LEN: usize = 96;

// ---------------------------------------------------------------------------
// O estado simetrico: a cadeia de chaves e o hash da transcricao
// ---------------------------------------------------------------------------

/// O `SymmetricState` do Noise: a cadeia `ck`, o hash `h` e a chave corrente.
///
/// O `h` acumula TUDO o que passou pelo aperto, na ordem, e e ele que entra
/// como dado associado de cada selagem. E dai que sai a garantia do
/// truncamento: a etiqueta da ultima mensagem so fecha se as duas mensagens
/// inteiras chegaram byte a byte como sairam.
struct Simetrico {
    ck: [u8; 32],
    h: [u8; 32],
    k: [u8; CHAVE_LEN],
    tem_chave: bool,
    n: u64,
}

impl Simetrico {
    fn novo() -> Simetrico {
        let mut s = Simetrico {
            ck: *NOME,
            h: *NOME,
            k: [0u8; CHAVE_LEN],
            tem_chave: false,
            n: 0,
        };
        s.misturar_hash(PROLOGO);
        s
    }

    fn misturar_hash(&mut self, dado: &[u8]) {
        let mut junto = Vec::with_capacity(32 + dado.len());
        junto.extend_from_slice(&self.h);
        junto.extend_from_slice(dado);
        self.h = sha256(&junto);
    }

    /// `MixKey`: cada Diffie-Hellman avanca a cadeia e troca a chave corrente.
    ///
    /// O contador volta a zero porque a CHAVE mudou -- e o par (chave, nonce)
    /// que precisa ser unico, nao o nonce sozinho.
    fn misturar_chave(&mut self, material: &[u8]) {
        let (ck, k) = hkdf::duas(&self.ck, material);
        self.ck = ck;
        self.k = k;
        self.tem_chave = true;
        self.n = 0;
    }

    fn nonce(&self) -> [u8; NONCE_LEN] {
        nonce_do_contador(self.n)
    }

    fn cifrar_e_hash(&mut self, claro: &[u8]) -> Vec<u8> {
        if !self.tem_chave {
            // Sem chave ainda, "cifrar" e passar adiante -- e o que o Noise
            // manda fazer antes do primeiro DH.
            self.misturar_hash(claro);
            return claro.to_vec();
        }
        let (mut cifrado, tag) = cifra::selar(&self.k, &self.nonce(), &self.h, claro);
        self.n += 1;
        cifrado.extend_from_slice(&tag);
        self.misturar_hash(&cifrado);
        cifrado
    }

    fn decifrar_e_hash(&mut self, cifrado: &[u8]) -> Result<Vec<u8>> {
        if !self.tem_chave {
            self.misturar_hash(cifrado);
            return Ok(cifrado.to_vec());
        }
        if cifrado.len() < TAG_LEN {
            return Err(PhxError::Corrompido(
                "pedaco do aperto menor que a etiqueta de autenticacao".into(),
            ));
        }
        let corte = cifrado.len() - TAG_LEN;
        let mut tag = [0u8; TAG_LEN];
        tag.copy_from_slice(&cifrado[corte..]);
        let claro = cifra::abrir(&self.k, &self.nonce(), &self.h, &cifrado[..corte], &tag)?;
        self.n += 1;
        // O hash come o CIFRADO, e nao o claro: e o que o outro lado viu.
        self.misturar_hash(cifrado);
        Ok(claro)
    }

    /// `Split()`: as duas chaves de transporte, uma por direcao.
    fn dividir(&self) -> ([u8; CHAVE_LEN], [u8; CHAVE_LEN]) {
        hkdf::duas(&self.ck, &[])
    }
}

/// O nonce de 96 bits a partir do contador de 64, como o Noise define.
fn nonce_do_contador(n: u64) -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    nonce[4..].copy_from_slice(&n.to_le_bytes());
    nonce
}

// ---------------------------------------------------------------------------
// O aperto
// ---------------------------------------------------------------------------

/// O lado que comeca: o cliente.
pub struct Iniciador {
    simetrico: Simetrico,
    efemera: [u8; 32],
    pino: Option<[u8; 32]>,
}

impl Iniciador {
    /// Comeca o aperto. Devolve a mensagem 1, que sao 32 bytes.
    ///
    /// `pino` e a estatica que se ESPERA do servidor. Com ela, um servidor que
    /// apresente outra chave derruba o aperto -- e e assim que o cliente se
    /// protege de quem esta no meio. Sem ela, o cliente aceita a chave que
    /// vier (TOFU): protege da escuta passiva a partir da segunda conexao, e
    /// nao protege da primeira.
    pub fn comecar(pino: Option<[u8; 32]>) -> (Iniciador, [u8; M1_LEN]) {
        Iniciador::comecar_com(x25519::gerar_privada(), pino)
    }

    fn comecar_com(efemera: [u8; 32], pino: Option<[u8; 32]>) -> (Iniciador, [u8; M1_LEN]) {
        let mut simetrico = Simetrico::novo();
        let publica = x25519::chave_publica(&efemera);
        simetrico.misturar_hash(&publica);
        // A carga da mensagem 1 e vazia, e mesmo vazia ela entra no hash: o
        // Noise sempre trata a carga, e pular quando ela e vazia seria uma
        // excecao a mais para os dois lados concordarem.
        simetrico.cifrar_e_hash(&[]);
        (
            Iniciador {
                simetrico,
                efemera,
                pino,
            },
            publica,
        )
    }

    /// Fecha o aperto com a mensagem 2. Devolve o transporte e a estatica que
    /// o servidor apresentou.
    pub fn terminar(mut self, m2: &[u8]) -> Result<(Transporte, [u8; 32])> {
        if m2.len() != M2_LEN {
            return Err(PhxError::Corrompido(format!(
                "a mensagem 2 do aperto tem {} bytes, e devia ter {M2_LEN}",
                m2.len()
            )));
        }
        let mut efemera_dele = [0u8; 32];
        efemera_dele.copy_from_slice(&m2[..32]);
        self.simetrico.misturar_hash(&efemera_dele);
        self.simetrico
            .misturar_chave(&x25519::segredo(&self.efemera, &efemera_dele)?);

        let estatica = self.simetrico.decifrar_e_hash(&m2[32..80])?;
        let mut dele = [0u8; 32];
        dele.copy_from_slice(&estatica);

        // O pino e conferido ANTES do ultimo Diffie-Hellman, e nao depois: sem
        // isso, um servidor errado que fechasse a conta continuaria fechando o
        // aperto, e a recusa viraria uma comparacao esquecivel la na frente.
        if let Some(esperada) = self.pino {
            if !crate::hash::iguais_em_tempo_constante(&dele, &esperada) {
                return Err(PhxError::Autorizacao(format!(
                    "a chave do servidor nao e a esperada: pino {}, apresentada {}",
                    crate::hash::para_hex(&esperada),
                    crate::hash::para_hex(&dele)
                )));
            }
        }

        self.simetrico
            .misturar_chave(&x25519::segredo(&self.efemera, &dele)?);
        // A etiqueta desta carga vazia e o que prova que o outro lado tem a
        // privada da estatica que ele apresentou.
        self.simetrico.decifrar_e_hash(&m2[80..])?;

        let (envio, recepcao) = self.simetrico.dividir();
        Ok((Transporte::novo(envio, recepcao, self.simetrico.h), dele))
    }
}

/// O lado que responde: o servidor. Um passo so -- le a mensagem 1 e devolve
/// a 2 junto com o transporte ja pronto.
pub fn responder(estatica: &[u8; 32], m1: &[u8]) -> Result<(Transporte, Vec<u8>)> {
    if m1.len() != M1_LEN {
        return Err(PhxError::Corrompido(format!(
            "a mensagem 1 do aperto tem {} bytes, e devia ter {M1_LEN}",
            m1.len()
        )));
    }
    let mut simetrico = Simetrico::novo();
    let mut efemera_dele = [0u8; 32];
    efemera_dele.copy_from_slice(m1);
    simetrico.misturar_hash(&efemera_dele);
    simetrico.decifrar_e_hash(&[])?;

    let efemera = x25519::gerar_privada();
    let efemera_pub = x25519::chave_publica(&efemera);
    simetrico.misturar_hash(&efemera_pub);
    simetrico.misturar_chave(&x25519::segredo(&efemera, &efemera_dele)?);

    let mut m2 = Vec::with_capacity(M2_LEN);
    m2.extend_from_slice(&efemera_pub);
    m2.extend_from_slice(&simetrico.cifrar_e_hash(&x25519::chave_publica(estatica)));
    simetrico.misturar_chave(&x25519::segredo(estatica, &efemera_dele)?);
    m2.extend_from_slice(&simetrico.cifrar_e_hash(&[]));

    // O servidor RESPONDE, entao as duas chaves vem trocadas em relacao ao
    // iniciador: a primeira da divisao e sempre a de quem comecou.
    let (do_cliente, do_servidor) = simetrico.dividir();
    Ok((Transporte::novo(do_servidor, do_cliente, simetrico.h), m2))
}

// ---------------------------------------------------------------------------
// A camada de registro
// ---------------------------------------------------------------------------

/// O que um registro carrega.
///
/// O tipo vai DENTRO do texto claro: fica autenticado (mexer nele quebra a
/// etiqueta) e invisivel de fora (quem escuta nao distingue um pedido de uma
/// despedida).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tipo {
    /// Uma linha do protocolo JSON.
    Pedido,
    /// Fim de conversa. E o que separa "acabou" de "cortaram".
    Fim,
}

impl Tipo {
    fn byte(self) -> u8 {
        match self {
            Tipo::Pedido => 1,
            Tipo::Fim => 2,
        }
    }

    /// Tipo desconhecido e ERRO, e nao "ignore e siga": um registro que este
    /// lado nao entende ja passou pela etiqueta, entao quem o mandou fala o
    /// protocolo e discorda da versao. Seguir seria adivinhar.
    fn de_byte(b: u8) -> Result<Tipo> {
        match b {
            1 => Ok(Tipo::Pedido),
            2 => Ok(Tipo::Fim),
            outro => Err(PhxError::Corrompido(format!(
                "registro do fio com tipo {outro}, que esta versao nao conhece"
            ))),
        }
    }
}

/// Uma direcao do tunel: a chave e o contador dela.
struct Direcao {
    k: [u8; CHAVE_LEN],
    n: u64,
}

impl Direcao {
    /// O nonce do proximo registro -- ou o erro do teto.
    ///
    /// O Noise reserva `2^64 - 1` e manda nao usa-lo. Aqui o teto FECHA a
    /// conexao em vez de rechavear, e o argumento esta na secao 3 do
    /// `docs/CIFRA-DO-FIO.md`: 2^64 registros a um por microssegundo sao 584
    /// mil anos, entao chegar la e defeito, nao carga -- e um rechaveamento
    /// que so roda em condicao inatingivel e codigo que se degrada calado.
    fn proximo(&mut self) -> Result<[u8; NONCE_LEN]> {
        if self.n == u64::MAX {
            return Err(PhxError::LimiteExcedido(
                "contador de registros do fio no teto: a conexao fecha em vez \
                 de repetir um nonce"
                    .into(),
            ));
        }
        let nonce = nonce_do_contador(self.n);
        self.n += 1;
        Ok(nonce)
    }
}

/// O tunel pronto: uma chave por direcao, e o hash da transcricao do aperto.
pub struct Transporte {
    envio: Direcao,
    recepcao: Direcao,
    transcricao: [u8; 32],
    fim_recebido: bool,
    fim_enviado: bool,
}

/// `Debug` escrito a mao, pela mesma razao do `Cifra` do `config.rs`: o
/// derivado imprimiria as duas chaves de sessao, e um `{:?}` apressado num
/// diagnostico as jogaria no log. Segredo que aparece em `Debug` vaza no dia
/// em que alguem acrescentar um `dbg!`.
impl std::fmt::Debug for Transporte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transporte")
            .field("chaves", &"(ocultas)")
            .field("enviados", &self.envio.n)
            .field("recebidos", &self.recepcao.n)
            .field("fim_enviado", &self.fim_enviado)
            .field("fim_recebido", &self.fim_recebido)
            .finish()
    }
}

impl Transporte {
    fn novo(
        envio: [u8; CHAVE_LEN],
        recepcao: [u8; CHAVE_LEN],
        transcricao: [u8; 32],
    ) -> Transporte {
        Transporte {
            envio: Direcao { k: envio, n: 0 },
            recepcao: Direcao { k: recepcao, n: 0 },
            transcricao,
            fim_recebido: false,
            fim_enviado: false,
        }
    }

    /// O hash da transcricao do aperto.
    ///
    /// Os dois lados chegam ao mesmo valor, e ninguem no meio consegue faze-lo
    /// coincidir sem ter fechado o aperto. E o que uma amarracao da credencial
    /// ao canal usaria -- ainda ninguem usa, e a secao 10 do documento diz
    /// isso em vez de deixar parecer feito.
    pub fn transcricao(&self) -> [u8; 32] {
        self.transcricao
    }

    pub fn fim_recebido(&self) -> bool {
        self.fim_recebido
    }

    pub fn fim_enviado(&self) -> bool {
        self.fim_enviado
    }

    /// Sela um registro e devolve a linha Base64 que vai ao fio (sem o `\n`).
    pub fn selar(&mut self, tipo: Tipo, conteudo: &[u8]) -> Result<String> {
        // Pedido depois da despedida e defeito deste lado, e nao do fio: quem
        // ja disse "acabou" e continua falando faz o outro lado ver dado
        // DEPOIS do fim, que e a mesma ambiguidade que a despedida existe para
        // matar.
        if self.fim_enviado {
            return Err(PhxError::Corrompido(
                "registro depois da despedida: este lado ja fechou a conversa".into(),
            ));
        }
        let nonce = self.envio.proximo()?;
        let mut claro = Vec::with_capacity(1 + conteudo.len());
        claro.push(tipo.byte());
        claro.extend_from_slice(conteudo);
        let (mut cifrado, tag) = cifra::selar(&self.envio.k, &nonce, &[], &claro);
        cifrado.extend_from_slice(&tag);
        if tipo == Tipo::Fim {
            self.fim_enviado = true;
        }
        Ok(base64::codificar(&cifrado))
    }

    /// Abre uma linha Base64 recebida.
    ///
    /// O contador usado e o DESTE lado, e nao um que venha no fio: registro
    /// repetido, fora de ordem ou suprimido chega com o contador errado e nao
    /// autentica. Nao ha janela nem tolerancia -- o TCP ja entrega em ordem, e
    /// o que sobra e ataque.
    pub fn abrir(&mut self, linha: &str) -> Result<(Tipo, Vec<u8>)> {
        let bruto = base64::decodificar(linha.trim())?;
        if bruto.len() < TAG_LEN + 1 {
            return Err(PhxError::Corrompido(format!(
                "registro do fio com {} bytes: nao cabe nem o tipo e a etiqueta",
                bruto.len()
            )));
        }
        let nonce = self.recepcao.proximo()?;
        let corte = bruto.len() - TAG_LEN;
        let mut tag = [0u8; TAG_LEN];
        tag.copy_from_slice(&bruto[corte..]);
        let claro = cifra::abrir(&self.recepcao.k, &nonce, &[], &bruto[..corte], &tag)?;
        let tipo = Tipo::de_byte(claro[0])?;
        if tipo == Tipo::Fim {
            self.fim_recebido = true;
        }
        Ok((tipo, claro[1..].to_vec()))
    }

    /// So para o teste do esgotamento: poe os dois contadores onde se quer.
    ///
    /// Nao existe fora do teste de proposito -- uma API publica que deixa
    /// escolher o contador do nonce e exatamente o jeito de furar isto.
    #[cfg(test)]
    fn forcar_contadores(&mut self, n: u64) {
        self.envio.n = n;
        self.recepcao.n = n;
    }
}

// ---------------------------------------------------------------------------
// O canal: o mesmo laco de linhas, cifrado ou nao
// ---------------------------------------------------------------------------

/// O que uma leitura devolveu.
#[derive(Debug, PartialEq, Eq)]
pub enum Recebido {
    /// Uma linha do protocolo.
    Linha(String),
    /// Fim de conversa, limpo.
    Fim,
}

/// O fio, cifrado ou em claro.
///
/// Existe para que o laco de conexao seja UM so. Espalhar `if cifrado` por
/// cada `read_line` e cada `writeln!` do servidor seria repetir a decisao em
/// dezenas de lugares, e a que alguem esquecesse mandaria texto claro por um
/// fio que o cliente acha cifrado.
pub enum Canal {
    Claro,
    Cifrado(Box<Transporte>),
}

/// Teto de um registro do fio, em bytes.
///
/// Sem ele o `read_line` e ilimitado e quem decide quanta memoria este lado
/// reserva e o outro lado da conexao.
pub const TETO_DO_REGISTRO: u64 = 128 * 1024 * 1024;

impl Canal {
    pub fn cifrado(&self) -> bool {
        matches!(self, Canal::Cifrado(_))
    }

    /// A transcricao do aperto, quando ha aperto.
    pub fn transcricao(&self) -> Option<[u8; 32]> {
        match self {
            Canal::Claro => None,
            Canal::Cifrado(t) => Some(t.transcricao()),
        }
    }

    /// Le uma linha do fio.
    ///
    /// # As tres saidas, e nao duas
    ///
    /// Este e o ponto em que "nao deu erro" nao pode virar "deu certo":
    ///
    /// * registro `FIM` e depois EOF -> [`Recebido::Fim`], fim limpo;
    /// * EOF **sem** ter visto `FIM` -> **erro**, o fio foi cortado;
    /// * linha sem `\n` (EOF no meio de um registro) -> **erro**, truncado.
    ///
    /// Em claro nao ha como distinguir: EOF e fim, e ponto. Essa
    /// impossibilidade e um dos ganhos do tunel, e nao uma lacuna dele.
    pub fn ler<L: BufRead>(&mut self, leitor: &mut L) -> Result<Recebido> {
        self.ler_ate(leitor, TETO_DO_REGISTRO)
    }

    /// O mesmo, com o teto explicito -- so para quem tem motivo para outro.
    ///
    /// O teto mora AQUI, e nao em quem chama, por um motivo que a integracao
    /// de duas frentes tornou visivel: a frente da trava pos um limite no
    /// `read_line` da replica, a frente da cifra trocou aquele `read_line` por
    /// este canal, e juntar as duas sem cuidado devolveria o limite ilimitado
    /// -- com quem escolhe o tamanho sendo o outro lado do fio. No canal, o
    /// teto vale para todo mundo que le, cifrado ou claro.
    pub fn ler_ate<L: BufRead>(&mut self, leitor: &mut L, teto: u64) -> Result<Recebido> {
        let mut linha = String::new();
        // O `+1` e o que separa "coube" de "estourou" sem contar o que ainda
        // vem. Passou do teto, a conexao esta no meio de uma linha e nao serve
        // mais: por isso a recusa e erro, e a proxima rodada abre outra.
        let lidos = {
            let mut limitado = <&mut L as std::io::Read>::take(leitor, teto + 1);
            limitado.read_line(&mut linha)?
        };
        if lidos as u64 > teto {
            return Err(PhxError::LimiteExcedido(format!(
                "o outro lado mandou mais de {} MiB num registro so, e este \
                 lado nao guarda isso na memoria; baixe o tamanho do lote de \
                 quem serve ou parta a tabela",
                teto / (1024 * 1024)
            )));
        }
        match self {
            Canal::Claro => {
                if lidos == 0 {
                    return Ok(Recebido::Fim);
                }
                Ok(Recebido::Linha(linha))
            }
            Canal::Cifrado(t) => {
                if lidos == 0 {
                    return if t.fim_recebido() {
                        Ok(Recebido::Fim)
                    } else {
                        Err(PhxError::Corrompido(
                            "o fio cifrado foi cortado: a conexao acabou sem a \
                             despedida, entao pode faltar dado que ninguem viu \
                             faltar"
                                .into(),
                        ))
                    };
                }
                if !linha.ends_with('\n') {
                    return Err(PhxError::Corrompido(
                        "registro do fio truncado: a conexao acabou no meio de \
                         um registro"
                            .into(),
                    ));
                }
                let (tipo, conteudo) = t.abrir(&linha)?;
                match tipo {
                    Tipo::Fim => Ok(Recebido::Fim),
                    Tipo::Pedido => Ok(Recebido::Linha(String::from_utf8(conteudo).map_err(
                        |e| PhxError::Corrompido(format!("registro do fio nao e UTF-8: {e}")),
                    )?)),
                }
            }
        }
    }

    /// Manda uma linha do protocolo.
    pub fn escrever<E: Write>(&mut self, saida: &mut E, linha: &str) -> Result<()> {
        match self {
            Canal::Claro => writeln!(saida, "{linha}")?,
            Canal::Cifrado(t) => {
                let registro = t.selar(Tipo::Pedido, linha.as_bytes())?;
                writeln!(saida, "{registro}")?;
            }
        }
        saida.flush()?;
        Ok(())
    }

    /// Manda a despedida. Em claro nao ha o que mandar -- e por isso que em
    /// claro nao ha como distinguir fim de corte.
    pub fn despedir<E: Write>(&mut self, saida: &mut E) -> Result<()> {
        if let Canal::Cifrado(t) = self {
            if t.fim_enviado() {
                return Ok(());
            }
            let registro = t.selar(Tipo::Fim, &[])?;
            writeln!(saida, "{registro}")?;
            saida.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Cursor;

    /// Um aperto inteiro, e os dois lados com o mesmo material.
    fn aperto() -> (Transporte, Transporte, [u8; 32]) {
        let estatica = x25519::gerar_privada();
        let (iniciador, m1) = Iniciador::comecar(None);
        let (do_servidor, m2) = responder(&estatica, &m1).unwrap();
        let (do_cliente, apresentada) = iniciador.terminar(&m2).unwrap();
        assert_eq!(apresentada, x25519::chave_publica(&estatica));
        (do_cliente, do_servidor, estatica)
    }

    #[test]
    fn o_nome_do_protocolo_tem_o_tamanho_do_hash() {
        // Se um dia o nome mudar de tamanho, ele passa a precisar de SHA-256
        // antes de virar `h` -- e o aperto quebraria calado entre versoes.
        assert_eq!(NOME.len(), 32);
    }

    #[test]
    fn aperto_fecha_e_os_dois_lados_derivam_o_mesmo() {
        let (mut cliente, mut servidor, _) = aperto();
        assert_eq!(cliente.transcricao(), servidor.transcricao());

        let linha = servidor
            .selar(Tipo::Pedido, br#"{"ok":true}"#)
            .expect("selar");
        let (tipo, conteudo) = cliente.abrir(&linha).expect("abrir");
        assert_eq!(tipo, Tipo::Pedido);
        assert_eq!(conteudo, br#"{"ok":true}"#);

        let volta = cliente.selar(Tipo::Pedido, b"ping").unwrap();
        assert_eq!(servidor.abrir(&volta).unwrap().1, b"ping");
    }

    /// Dois apertos seguidos nao derivam a mesma chave -- as efemeras mudam.
    #[test]
    fn dois_apertos_nao_dao_a_mesma_transcricao() {
        let (a, _, _) = aperto();
        let (b, _, _) = aperto();
        assert_ne!(a.transcricao(), b.transcricao());
    }

    #[test]
    fn pino_certo_passa_e_pino_errado_derruba() {
        let estatica = x25519::gerar_privada();
        let publica = x25519::chave_publica(&estatica);

        let (iniciador, m1) = Iniciador::comecar(Some(publica));
        let (_, m2) = responder(&estatica, &m1).unwrap();
        assert!(iniciador.terminar(&m2).is_ok());

        let outra = x25519::chave_publica(&x25519::gerar_privada());
        let (iniciador, m1) = Iniciador::comecar(Some(outra));
        let (_, m2) = responder(&estatica, &m1).unwrap();
        let erro = iniciador
            .terminar(&m2)
            .expect_err("pino errado tem de cair");
        assert!(
            matches!(erro, PhxError::Autorizacao(_)),
            "o pino errado devia ser recusa de autorizacao, e veio {erro:?}"
        );
    }

    /// Um servidor que apresenta a estatica de outro nao fecha o aperto: a
    /// etiqueta final depende da PRIVADA, e ele nao a tem.
    #[test]
    fn quem_apresenta_estatica_alheia_nao_fecha() {
        let verdadeira = x25519::gerar_privada();
        let impostor = x25519::gerar_privada();
        let (iniciador, m1) = Iniciador::comecar(Some(x25519::chave_publica(&verdadeira)));
        let (_, mut m2) = responder(&impostor, &m1).unwrap();
        // O impostor troca a estatica cifrada pela do servidor de verdade.
        // Nao adianta: a etiqueta e o `es` sao dele.
        m2[32..80].copy_from_slice(&[0u8; 48]);
        assert!(iniciador.terminar(&m2).is_err());
    }

    /// Qualquer bit mexido na mensagem 2 derruba o aperto -- a transcricao
    /// cobre o aperto inteiro.
    #[test]
    fn mensagem_2_mexida_nao_autentica() {
        let estatica = x25519::gerar_privada();
        for posicao in [0usize, 31, 32, 79, 80, 95] {
            let (iniciador, m1) = Iniciador::comecar(None);
            let (_, mut m2) = responder(&estatica, &m1).unwrap();
            m2[posicao] ^= 1;
            assert!(
                iniciador.terminar(&m2).is_err(),
                "o byte {posicao} da mensagem 2 passou mexido"
            );
        }
    }

    /// A mensagem 1 mexida tambem cai -- so que do outro lado, na etiqueta que
    /// o cliente confere.
    #[test]
    fn mensagem_1_mexida_nao_autentica() {
        let estatica = x25519::gerar_privada();
        let (iniciador, mut m1) = Iniciador::comecar(None);
        m1[0] ^= 1;
        let (_, m2) = responder(&estatica, &m1).unwrap();
        assert!(iniciador.terminar(&m2).is_err());
    }

    #[test]
    fn mensagem_do_tamanho_errado_e_recusada() {
        let estatica = x25519::gerar_privada();
        assert!(responder(&estatica, &[0u8; 31]).is_err());
        assert!(responder(&estatica, &[0u8; 33]).is_err());
        let (iniciador, m1) = Iniciador::comecar(None);
        let (_, m2) = responder(&estatica, &m1).unwrap();
        assert!(iniciador.terminar(&m2[..95]).is_err());
    }

    /// Efemera de ordem pequena na mensagem 1: o servidor recusa em vez de
    /// derivar um segredo todo-zeros.
    #[test]
    fn efemera_de_ordem_pequena_derruba_o_aperto() {
        let estatica = x25519::gerar_privada();
        assert!(responder(&estatica, &[0u8; 32]).is_err());
    }

    #[test]
    fn registro_mexido_nao_abre() {
        let (mut cliente, mut servidor, _) = aperto();
        let linha = servidor.selar(Tipo::Pedido, b"segredo").unwrap();
        let mut bytes = base64::decodificar(&linha).unwrap();
        bytes[0] ^= 1;
        assert!(cliente.abrir(&base64::codificar(&bytes)).is_err());
    }

    #[test]
    fn registro_repetido_nao_abre() {
        let (mut cliente, mut servidor, _) = aperto();
        let primeiro = servidor.selar(Tipo::Pedido, b"um").unwrap();
        assert!(cliente.abrir(&primeiro).is_ok());
        assert!(
            cliente.abrir(&primeiro).is_err(),
            "o mesmo registro abriu duas vezes: o contador nao esta valendo"
        );
    }

    #[test]
    fn registro_fora_de_ordem_nao_abre() {
        let (mut cliente, mut servidor, _) = aperto();
        let primeiro = servidor.selar(Tipo::Pedido, b"um").unwrap();
        let segundo = servidor.selar(Tipo::Pedido, b"dois").unwrap();
        // Pular o primeiro tem de derrubar o segundo.
        assert!(cliente.abrir(&segundo).is_err());
        let _ = primeiro;
    }

    /// As duas direcoes tem chaves diferentes: um registro do servidor nao
    /// abre no proprio servidor.
    #[test]
    fn registro_nao_volta_pela_mesma_direcao() {
        let (_, mut servidor, _) = aperto();
        let linha = servidor.selar(Tipo::Pedido, b"eco").unwrap();
        assert!(servidor.abrir(&linha).is_err());
    }

    /// O teto do contador FECHA, e nao repete nem rechaveia.
    #[test]
    fn contador_no_teto_recusa_em_vez_de_repetir() {
        let (mut cliente, mut servidor, _) = aperto();
        servidor.forcar_contadores(u64::MAX - 1);
        cliente.forcar_contadores(u64::MAX - 1);

        // O ultimo registro utilizavel ainda passa.
        let linha = servidor.selar(Tipo::Pedido, b"o ultimo").unwrap();
        assert_eq!(cliente.abrir(&linha).unwrap().1, b"o ultimo");

        let erro = servidor
            .selar(Tipo::Pedido, b"o que nao pode")
            .expect_err("o teto tinha de recusar");
        assert!(matches!(erro, PhxError::LimiteExcedido(_)), "{erro:?}");
        assert!(matches!(
            cliente
                .abrir(&linha)
                .expect_err("o teto tinha de recusar na leitura"),
            PhxError::LimiteExcedido(_)
        ));
    }

    #[test]
    fn tipo_desconhecido_e_erro() {
        assert!(Tipo::de_byte(0).is_err());
        assert!(Tipo::de_byte(3).is_err());
        assert_eq!(Tipo::de_byte(1).unwrap(), Tipo::Pedido);
        assert_eq!(Tipo::de_byte(2).unwrap(), Tipo::Fim);
    }

    /// O CANAL: fim limpo e fio cortado sao vereditos diferentes.
    #[test]
    fn fim_e_corte_sao_vereditos_diferentes() {
        let (cliente, mut servidor, _) = aperto();
        let mut canal = Canal::Cifrado(Box::new(cliente));

        // (a) o servidor se despede: fim limpo.
        let despedida = servidor.selar(Tipo::Fim, &[]).unwrap();
        let mut fio = Cursor::new(format!("{despedida}\n"));
        assert_eq!(canal.ler(&mut fio).unwrap(), Recebido::Fim);
        let mut vazio = Cursor::new(String::new());
        assert_eq!(canal.ler(&mut vazio).unwrap(), Recebido::Fim);

        // (b) sem despedida, o EOF e erro.
        let (cliente, _, _) = aperto();
        let mut canal = Canal::Cifrado(Box::new(cliente));
        let mut vazio = Cursor::new(String::new());
        assert!(
            canal.ler(&mut vazio).is_err(),
            "EOF sem despedida passou por fim limpo: o fio cortado virou sucesso"
        );

        // (c) registro pela metade -- sem o `\n` final -- e erro.
        let (cliente, mut servidor, _) = aperto();
        let mut canal = Canal::Cifrado(Box::new(cliente));
        let inteiro = servidor.selar(Tipo::Pedido, b"metade").unwrap();
        let mut cortado = Cursor::new(inteiro[..inteiro.len() / 2].to_string());
        assert!(canal.ler(&mut cortado).is_err());
    }

    /// Em claro, o EOF e fim -- e continua sendo, porque mudar isso mudaria o
    /// comportamento de todo cliente que existe.
    #[test]
    fn em_claro_o_eof_continua_sendo_fim() {
        let mut canal = Canal::Claro;
        let mut vazio = Cursor::new(String::new());
        assert_eq!(canal.ler(&mut vazio).unwrap(), Recebido::Fim);
        let mut uma = Cursor::new("{\"op\":\"ping\"}\n".to_string());
        assert_eq!(
            canal.ler(&mut uma).unwrap(),
            Recebido::Linha("{\"op\":\"ping\"}\n".to_string())
        );
    }

    /// O canal inteiro, ida e volta, pelo par de transportes.
    #[test]
    fn canal_leva_e_traz() {
        let (cliente, servidor, _) = aperto();
        let mut do_cliente = Canal::Cifrado(Box::new(cliente));
        let mut do_servidor = Canal::Cifrado(Box::new(servidor));

        let mut fio: Vec<u8> = Vec::new();
        do_cliente
            .escrever(&mut fio, r#"{"op":"ping"}"#)
            .expect("escrever");
        do_cliente.despedir(&mut fio).expect("despedir");

        let mut leitor = Cursor::new(fio);
        assert_eq!(
            do_servidor.ler(&mut leitor).unwrap(),
            Recebido::Linha(r#"{"op":"ping"}"#.to_string())
        );
        assert_eq!(do_servidor.ler(&mut leitor).unwrap(), Recebido::Fim);
    }

    /// Nada do que passa pelo tunel aparece no fio.
    /// O teto de um registro vale nos DOIS canais, e essa e a razao de ele
    /// morar aqui.
    ///
    /// A protecao nasceu na replica, num `read_line` com `take`. A cifra
    /// depois trocou aquele `read_line` por este canal -- e juntar as duas
    /// frentes sem olhar teria devolvido a leitura ilimitada, com quem escolhe
    /// quanta memoria este lado reserva sendo o outro lado do fio.
    ///
    /// A assercao e sobre **quanto foi lido**, e nao sobre o veredito, e a
    /// primeira versao deste teste ensinou por que: conferir so o erro passava
    /// com o defeito reposto, porque a conferencia `lidos > teto` vem DEPOIS
    /// da leitura e acusa mesmo sem o `take` -- so que ai a memoria ja foi
    /// gasta, que e justamente o dano. Teste que passa por engano e pior que
    /// teste que falta.
    #[test]
    fn o_teto_do_registro_para_a_leitura_e_nao_so_recusa_depois() {
        let teto = 64u64;
        let gorda = format!("{}\n", "x".repeat(10_000));
        let bytes = gorda.as_bytes();

        let mut claro = Canal::Claro;
        let mut fonte: &[u8] = bytes;
        let erro = claro
            .ler_ate(&mut fonte, teto)
            .expect_err("linha acima do teto tem de ser recusada");
        assert!(matches!(erro, PhxError::LimiteExcedido(_)), "{erro:?}");

        // O que importa: sobrou quase tudo por ler. Sem o `take`, `fonte`
        // estaria vazia -- os 10 KiB teriam entrado na memoria antes da recusa.
        let consumido = bytes.len() - fonte.len();
        assert!(
            consumido as u64 <= teto + 1,
            "leu {consumido} bytes com teto de {teto}: a recusa veio depois de \
             gastar a memoria, que e o defeito que este teto existe para impedir"
        );

        // E o que CABE continua passando -- teto que recusa tudo nao e teto,
        // e parede.
        let magra = "cabe\n";
        assert_eq!(
            claro.ler_ate(&mut magra.as_bytes(), teto).unwrap(),
            Recebido::Linha(magra.to_string())
        );
    }

    #[test]
    fn o_texto_claro_nao_aparece_no_fio() {
        let (cliente, _, _) = aperto();
        let mut canal = Canal::Cifrado(Box::new(cliente));
        let mut fio: Vec<u8> = Vec::new();
        canal
            .escrever(&mut fio, r#"{"op":"login","token":"segredo-do-servico"}"#)
            .unwrap();
        let texto = String::from_utf8_lossy(&fio);
        assert!(!texto.contains("segredo-do-servico"));
        assert!(!texto.contains("login"));
    }
}
