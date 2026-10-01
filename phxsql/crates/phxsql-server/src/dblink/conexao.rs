//! Uma conexao de DbLink, seja qual for o motor do outro lado.
//!
//! # Por que este tipo demorou a existir
//!
//! O comentario que estava em `Definicao::conectar` explicava a demora e
//! estava certo: um tipo comum aos dois motores faria o codigo **compilar**
//! para o PostgreSQL(R) e falhar na primeira consulta, porque o SQL era de
//! MySQL(R). Compilar e falhar em producao e pior do que nao compilar.
//!
//! O que mudou foi o `dialeto`: agora as perguntas do DbLink existem nas duas
//! linguas. Com elas escritas, o tipo comum deixa de esconder um buraco e passa
//! a esconder uma diferenca que ja foi resolvida -- que e o que uma abstracao
//! deve fazer.
//!
//! # O que ele NAO uniformiza
//!
//! O formato do resultado de cada pergunta. `SHOW FULL COLUMNS` e a consulta
//! ao `pg_attribute` devolvem as mesmas seis colunas na mesma ordem porque o
//! `dialeto` as montou assim, e nao porque este tipo as tenha reordenado. A
//! traducao mora no SQL, onde da para ler as duas versoes lado a lado.

use crate::prazo::Prazo;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::fio::TETO_DO_REGISTRO;
use phxsql_core::json::Json;

use super::{mysql, phx, Definicao, Motor};
use crate::pg;

/// Uma coluna do resultado, no formato que a grade da tela espera.
///
/// E o menor denominador HONESTO dos dois: o que o MySQL(R) traz e o
/// PostgreSQL(R) nao (tabela de origem, casas decimais) sai vazio ou zero em
/// vez de inventado.
#[derive(Debug, Clone, Default)]
pub struct Coluna {
    pub nome: String,
    pub tabela: String,
    pub tipo: String,
    pub tamanho: u32,
    pub decimais: u8,
    pub nulavel: bool,
    pub primaria: bool,
    pub numerico: bool,
    /// Bytes, e nao texto -- pedido 590. Quem decide e o leitor do fio de
    /// cada motor (o conjunto `binary` no MySQL(R), o OID do `bytea` no
    /// PostgreSQL(R)), e a celula ja chega aqui em hexadecimal minusculo, na
    /// mesma forma do BLOB daqui. A marca viaja para quem le o JSON saber que
    /// `cafe` ali sao dois bytes, e nao a palavra.
    pub binario: bool,
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

impl Resultado {
    /// O valor de uma celula, ou `None` quando ela e NULL ou nao existe.
    pub fn celula(&self, linha: usize, coluna: usize) -> Option<String> {
        self.linhas.get(linha)?.get(coluna)?.clone()
    }

    /// Como a grade da tela le o resultado.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            (
                "colunas",
                Json::Lista(
                    self.colunas
                        .iter()
                        .map(|c| {
                            Json::objeto(vec![
                                ("nome", Json::texto_de(&c.nome)),
                                ("tabela", Json::texto_de(&c.tabela)),
                                ("tipo", Json::texto_de(&c.tipo)),
                                ("tamanho", Json::de_u64(c.tamanho as u64)),
                                ("decimais", Json::de_u64(c.decimais as u64)),
                                ("nulavel", Json::Bool(c.nulavel)),
                                ("primaria", Json::Bool(c.primaria)),
                                ("numerico", Json::Bool(c.numerico)),
                                ("binario", Json::Bool(c.binario)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "linhas",
                Json::Lista(
                    self.linhas
                        .iter()
                        .map(|l| {
                            Json::Lista(
                                l.iter()
                                    .map(|v| match v {
                                        Some(t) => Json::texto_de(t),
                                        None => Json::Nulo,
                                    })
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            ),
            ("quantas", Json::de_u64(self.linhas.len() as u64)),
            ("afetadas", Json::de_u64(self.afetadas)),
            ("truncado", Json::Bool(self.truncado)),
        ])
    }
}

/// Uma linha do resultado, como os tres clientes a montam.
pub type Linha = Vec<Option<String>>;

const MIB: u64 = 1024 * 1024;

/// O teto de BYTES do resultado que nasce em toda ligacao -- pedido 546.
///
/// # O defeito
///
/// O resultado era cortado so por LINHAS, e quanto pesa cada linha quem diz
/// e o outro lado: ate 128 MiB por quadro no MySQL(R) (o 443 limitou o
/// quadro, nao a soma), 64 MiB por mensagem no PostgreSQL(R). `max_linhas`
/// vai a 100.000, entao um par malicioso -- ou quem esta no meio do fio em
/// claro -- fazia este processo guardar terabytes antes do primeiro corte.
///
/// # A decisao do valor (papel J, regua do CLAUDE.md)
///
/// Hipoteses escritas antes: (a) 64 MiB, o `max_allowed_packet` do
/// MySQL(R) 8; (b) 1 GiB, o `MaxAllocSize` do PostgreSQL(R) e o
/// `SQLITE_MAX_LENGTH`; (c) o [`TETO_DO_REGISTRO`] desta casa, 128 MiB.
///
/// Os quatro CONVERGEM em nao ter teto sobre a SOMA do resultado no cliente:
/// o `PQgetResult` da libpq e o `mysql_store_result` (MySQL e MariaDB) juntam
/// tudo; o `postgres_fdw` fatia em `fetch_size` = 100 LINHAS, sem bytes; o
/// SQLite entrega linha a linha. Os numeros de (a) e (b) sao tetos de UM
/// pacote ou de UM valor -- outra pergunta. E sobre ESSES a regua empata:
/// 64 MiB do MySQL (2) mais 16 MiB do MariaDB, padrao desde a 10.2.4 (3), dao
/// 5 contra 1 GB do PG (4) mais 1e9 do SQLite (1), 5. Empate de pergunta
/// alheia nao decide a nossa.
///
/// Diverge-se do «sem teto» pela restricao do 578: la quem opera derruba a
/// sessao; aqui a memoria e a do servidor inteiro, e quem escolhe quanto ele
/// reserva seria o par. Vence (c), e e o unico dos tres que NAO e um numero
/// novo: o resultado do `dblink_consultar` sai daqui como UM registro do
/// fio, e todo cliente desta casa (replica, console, ODBC) le no maximo
/// [`TETO_DO_REGISTRO`] por registro. Um resultado acima disso nao chegaria
/// a ninguem de qualquer jeito -- o teto nao recusa nada que antes
/// funcionasse ponta a ponta. A sincronia, que guarda em tabela em vez de
/// responder, e quem pode precisar de mais: sobe `max_mib` na ligacao.
/// Escolhido, nao medido.
pub const TETO_DE_BYTES_DO_RESULTADO: u64 = TETO_DO_REGISTRO;

/// O `max_mib` de fabrica, na unidade que a ligacao escreve.
pub const MIB_DO_RESULTADO_DE_FABRICA: u64 = TETO_DE_BYTES_DO_RESULTADO / MIB;

/// O maior `max_mib` que uma ligacao aceita: 1 GiB, o `MaxAllocSize` do
/// PostgreSQL(R) -- acima disso nem o maior dos quatro guarda um valor so.
/// Sem teto aqui, `"max_mib": 9e18` devolveria o defeito por configuracao.
pub const MIB_DO_RESULTADO_MAXIMO: u64 = 1024;

/// O que os tres clientes guardam de um resultado -- o motor UNICO do corte
/// por linhas e do teto de bytes (pedido 546).
///
/// Existe para que «guardo esta linha?» tenha UMA resposta. O teto de linhas
/// morava copiado em cada cliente (`linhas.len() >= teto` no MySQL(R), o
/// contrario no PostgreSQL(R), um `take` no PhxSql); pendurar o de bytes ao
/// lado de cada copia seria a terceira, a quarta e a quinta, e a que alguem
/// esquecesse seria o cliente que volta a guardar o que o par mandar.
///
/// # Contado ENQUANTO le
///
/// A linha e pesada antes de entrar, e a que passaria do teto e recusada sem
/// ser guardada: o pico do que se guarda e o teto, e nao o teto mais o resto
/// do resultado. A linha cortada pelo teto de LINHAS nao conta -- ela e lida
/// e jogada fora, e um `SELECT *` de tabela grande com `max_linhas` continua
/// voltando cortado, e nao recusado, como sempre voltou.
///
/// # O peso e o da MEMORIA, e nao o do fio
///
/// Cada celula custa o texto mais a moldura do `Option<String>` (24 bytes),
/// e cada linha a do `Vec`. Contar so os bytes do fio deixaria um MySQL(R)
/// mandar linhas de 4.096 NULOS (um byte cada no fio) e guardar 24 vezes o
/// que o teto diz.
#[derive(Debug)]
pub struct Acumulador {
    linhas: Vec<Linha>,
    teto_de_linhas: u64,
    teto_de_bytes: u64,
    bytes: u64,
    truncado: bool,
}

impl Acumulador {
    pub fn novo(teto_de_linhas: u64, teto_de_bytes: u64) -> Acumulador {
        Acumulador {
            linhas: Vec::new(),
            teto_de_linhas,
            teto_de_bytes,
            bytes: 0,
            truncado: false,
        }
    }

    /// Ainda se guarda linha? O `false` ja marca o corte -- e quem pergunta
    /// antes de decodificar poupa o trabalho da linha que ninguem guarda.
    pub fn quer_mais(&mut self) -> bool {
        let quer = (self.linhas.len() as u64) < self.teto_de_linhas;
        if !quer {
            self.truncado = true;
        }
        quer
    }

    /// Guarda a linha, corta-a pelo teto de linhas, ou recusa o resultado
    /// pelo de bytes.
    ///
    /// A recusa e erro, e nao corte calado como o de linhas, de proposito:
    /// cortar por bytes devolveria um resultado com MENOS linhas do que o
    /// `max_linhas` promete, e quem o lesse nao saberia se a tabela acaba ali.
    /// Depois dela a conexao fica no meio do resultado e nao se reaproveita --
    /// o mesmo contrato do teto de colunas (443/544). Ler o resto so para
    /// descartar daria ao par que nao para de mandar o prazo total inteiro.
    pub fn receber(&mut self, linha: Linha) -> Result<()> {
        if !self.quer_mais() {
            return Ok(());
        }
        let depois = self.bytes.saturating_add(peso_da_linha(&linha));
        if depois > self.teto_de_bytes {
            return Err(PhxError::LimiteExcedido(format!(
                "dblink: o resultado passaria do teto de {} MiB ({} bytes) na \
                 linha {}, com {} bytes ja guardados -- quem decide o tamanho \
                 de cada linha e o outro banco, e este lado nao guarda isso na \
                 memoria. Peca menos linhas ou colunas; se o resultado e \
                 legitimo, suba \"max_mib\" da ligacao (ate {} MiB)",
                self.teto_de_bytes / MIB,
                self.teto_de_bytes,
                self.linhas.len() + 1,
                self.bytes,
                MIB_DO_RESULTADO_MAXIMO
            )));
        }
        self.bytes = depois;
        self.linhas.push(linha);
        Ok(())
    }

    /// Quanto ainda cabe, em bytes -- o teto de LEITURA de quem recebe o
    /// resultado inteiro numa mensagem so (pedido 610).
    pub fn cabe(&self) -> u64 {
        self.teto_de_bytes.saturating_sub(self.bytes)
    }

    /// Pesa a mensagem CRUA, antes de ela ser analisada -- pedido 610.
    ///
    /// O motor `phxsql` recebe o resultado inteiro numa linha do fio, e a
    /// copia em `Linha` so nasce depois de `Json::analisar` montar a arvore
    /// -- de 16 a 32 vezes a linha. Pesar so a copia deixava o `max_mib` sem
    /// efeito sobre o pico: com `max_mib` 1, uma linha de 128 MiB ainda virava
    /// gigabytes antes da primeira recusa. A crua conta no MESMO contador que
    /// a copia, e continua contando enquanto a copia se faz: as duas estao
    /// vivas ao mesmo tempo, e a arvore nunca e menor que a linha.
    ///
    /// `lidos` acima de [`Acumulador::cabe`] e recusa com a frase do teto; quem
    /// le com `cabe()` de teto nunca chega aqui com mais, e usa
    /// [`Acumulador::recusa_da_crua`] quando o fio estoura.
    pub fn pesar_crua(&mut self, lidos: u64) -> Result<()> {
        if lidos > self.cabe() {
            return Err(self.recusa_da_crua());
        }
        self.bytes = self.bytes.saturating_add(lidos);
        Ok(())
    }

    /// A recusa da mensagem que passou do que cabe ANTES de ser analisada:
    /// a mesma classe e a mesma saida escrita da recusa por linha, porque o
    /// interruptor que a resolve e o mesmo (`max_mib`).
    pub fn recusa_da_crua(&self) -> PhxError {
        PhxError::LimiteExcedido(format!(
            "dblink: a resposta passaria do teto de {} MiB ({} bytes) antes de \
             ser analisada, com {} bytes ja guardados -- quem decide o tamanho \
             da resposta e o outro banco, e este lado nao a le inteira para \
             depois recusar. Peca menos linhas ou colunas; se o resultado e \
             legitimo, suba \"max_mib\" da ligacao (ate {} MiB)",
            self.teto_de_bytes / MIB,
            self.teto_de_bytes,
            self.bytes,
            MIB_DO_RESULTADO_MAXIMO
        ))
    }

    /// Quanto ja se guardou -- e o contador que a prova le, em vez da RAM.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// As linhas guardadas, e se houve corte.
    pub fn fim(self) -> (Vec<Linha>, bool) {
        (self.linhas, self.truncado)
    }
}

/// O peso de uma linha na memoria: as molduras e a CAPACIDADE de cada texto,
/// que e o que o alocador entregou -- e nao o tamanho, que e o que se usa.
fn peso_da_linha(linha: &Linha) -> u64 {
    let moldura =
        std::mem::size_of::<Linha>() + linha.capacity() * std::mem::size_of::<Option<String>>();
    linha.iter().flatten().fold(moldura as u64, |soma, s| {
        soma.saturating_add(s.capacity() as u64)
    })
}

impl From<mysql::Resultado> for Resultado {
    fn from(r: mysql::Resultado) -> Resultado {
        Resultado {
            colunas: r
                .colunas
                .into_iter()
                .map(|c| Coluna {
                    nome: c.nome,
                    tabela: c.tabela,
                    tipo: c.tipo,
                    tamanho: c.tamanho,
                    decimais: c.decimais,
                    nulavel: c.nulavel,
                    primaria: c.primaria,
                    numerico: c.numerico,
                    binario: c.binario,
                })
                .collect(),
            linhas: r.linhas,
            afetadas: r.afetadas,
            truncado: r.truncado,
        }
    }
}

impl From<pg::Resultado> for Resultado {
    fn from(r: pg::Resultado) -> Resultado {
        Resultado {
            colunas: r
                .colunas
                .into_iter()
                .map(|c| Coluna {
                    nome: c.nome,
                    // O `RowDescription` traz o OID da tabela, e nao o NOME
                    // dela; resolver o nome exigiria uma segunda consulta ao
                    // `pg_class` por coluna. Fica vazio, que e a verdade, em
                    // vez de um numero que a tela mostraria como nome.
                    tabela: String::new(),
                    tipo: c.tipo,
                    tamanho: if c.tamanho > 0 { c.tamanho as u32 } else { 0 },
                    decimais: 0,
                    // O protocolo simples nao diz se a coluna aceita NULO nem
                    // se e chave; quem quer isso pergunta a `dblink_estrutura`,
                    // que consulta o catalogo.
                    nulavel: true,
                    primaria: false,
                    numerico: c.numerico,
                    binario: c.binario,
                })
                .collect(),
            linhas: r.linhas,
            afetadas: r.afetadas,
            truncado: r.truncado,
        }
    }
}

/// A conexao aberta com o outro banco.
///
/// O terceiro caso NAO fala SQL para o catalogo: `Conexao::Phx` responde as
/// perguntas do DbLink pelo protocolo proprio, e por isso as operacoes o
/// desviam ANTES de montarem instrucao nenhuma (ver `operacoes`). Um metodo
/// aqui que fingisse aceitar SQL de catalogo compilaria e falharia na primeira
/// consulta -- a mesma armadilha que este arquivo ja documenta no cabecalho.
pub enum Conexao {
    MySql(Box<mysql::Conexao>),
    Postgres(Box<pg::Conexao>),
    Phx(Box<phx::Conexao>),
}

impl Conexao {
    /// Uma instrucao SQL contra o outro banco.
    ///
    /// O caso `Phx` precisa de um database, porque a op `sql` do PhxSql o pede
    /// -- e o unico chamador que passa por aqui e o `dblink_consultar`, que
    /// tem o pedido na mao. Ver `Conexao::consultar_em`.
    pub fn consultar(&mut self, sql: &str, teto: u64) -> Result<Resultado> {
        self.consultar_em("", sql, teto)
    }

    /// A mesma consulta, dizendo em que database ela roda.
    ///
    /// O `database` so vale para o motor `phxsql`: nos outros dois a base ja
    /// foi escolhida no aperto de mao, e mandar de novo nao mudaria nada.
    pub fn consultar_em(&mut self, database: &str, sql: &str, teto: u64) -> Result<Resultado> {
        match self {
            Conexao::MySql(c) => Ok(c.consultar(sql, teto)?.into()),
            Conexao::Postgres(c) => Ok(c.consultar(sql, teto)?.into()),
            Conexao::Phx(c) => c.consultar(database, sql, teto),
        }
    }

    pub fn ping(&mut self) -> Result<()> {
        match self {
            Conexao::MySql(c) => c.ping(),
            Conexao::Postgres(c) => c.ping(),
            Conexao::Phx(c) => c.ping().map(|_| ()),
        }
    }

    pub fn encerrar(&mut self) {
        match self {
            Conexao::MySql(c) => c.encerrar(),
            Conexao::Postgres(c) => c.encerrar(),
            // O cliente do protocolo proprio fecha o soquete ao ser largado.
            Conexao::Phx(_) => {}
        }
    }

    /// A versao anunciada pelo outro servidor.
    pub fn versao(&self) -> String {
        match self {
            Conexao::MySql(c) => c.versao.clone(),
            Conexao::Postgres(c) => c.versao.clone(),
            Conexao::Phx(c) => c.versao.clone(),
        }
    }

    /// O identificador da conexao do lado de la: `connection_id()` no
    /// MySQL(R), o PID do processo no PostgreSQL(R).
    ///
    /// O PhxSql nao numera a conexao -- ele conta quantas ha --, entao ali
    /// vale zero. Inventar um numero seria pior que nao ter nenhum.
    pub fn conexao_id(&self) -> u32 {
        match self {
            Conexao::MySql(c) => c.conexao_id,
            Conexao::Postgres(c) => c.conexao_id,
            Conexao::Phx(_) => 0,
        }
    }
}

impl Definicao {
    /// Abre a ligacao pelo cliente do motor que ela declara.
    ///
    /// Substitui o par `conectar`/`conectar_pg`, que continuam existindo para
    /// quem precisa do tipo concreto -- o teste de protocolo, por exemplo.
    pub fn abrir(&self) -> Result<Conexao> {
        self.abrir_com(self.prazo())
    }

    /// O mesmo, com o prazo na mao -- a prova do pedido 578 precisa de um
    /// silencio abaixo do segundo. Os TRES motores passam por aqui, e e isso
    /// que a prova percorre: o cliente que ficasse de fora do prazo comum
    /// reprovaria nela.
    pub(crate) fn abrir_com(&self, prazo: Prazo) -> Result<Conexao> {
        Ok(match self.motor {
            Motor::MySql => Conexao::MySql(Box::new(self.conectar_com(prazo)?)),
            Motor::Postgres => Conexao::Postgres(Box::new(self.conectar_pg_com(prazo)?)),
            // Este recebe a DEFINICAO inteira, e nao primitivas, porque o fio
            // dele tem cifra e pino: campo do fio que atravessa a fronteira a
            // mao e campo que alguem esquece de passar um dia, e o esquecimento
            // compila e abre a conexao em claro.
            Motor::Phx => Conexao::Phx(Box::new(phx::Conexao::abrir(self, prazo)?)),
        })
    }
}

#[cfg(test)]
mod testes_do_prazo_total {
    //! Pedido 578: o par que goteja abaixo do prazo de silencio.

    use super::*;
    use crate::prazo::Rotulo;
    use phxsql_core::error::PhxError;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    static ROTULO: Rotulo = Rotulo {
        quem: "dblink de prova",
        regra: "prova do 578",
    };

    /// Um byte a cada `PASSO`, bem abaixo do `SILENCIO`: nenhuma leitura
    /// estoura o prazo por leitura, e a resposta nunca termina. Os tres
    /// protocolos esperam uma mensagem longa -- o quadro do MySQL(R) de
    /// 65.535 bytes, a mensagem `R` do PostgreSQL(R) de 64 KiB, a linha do
    /// aperto do PhxSql sem quebra -- e o gotejo a enche devagar.
    const PASSO: Duration = Duration::from_millis(100);
    const SILENCIO: Duration = Duration::from_millis(400);
    const TOTAL: Duration = Duration::from_millis(1500);
    /// Quanto o teste espera antes de declarar a thread presa. Sem o total
    /// ela ficaria presa para sempre; aqui basta o total com folga larga.
    const PACIENCIA: Duration = Duration::from_secs(6);

    fn par_que_goteja(prefixo: &'static [u8]) -> u16 {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = ouvinte.accept() else {
                return;
            };
            if s.write_all(prefixo).is_err() {
                return;
            }
            // Para quando o cliente fecha -- e o que o conserto faz ao cortar.
            loop {
                std::thread::sleep(PASSO);
                if s.write_all(b"x").is_err() {
                    return;
                }
            }
        });
        porta
    }

    fn ligacao(motor: &str, porta: u16) -> Definicao {
        Definicao::de_json(
            &Json::analisar(&format!(
                r#"{{"nome":"prova578","motor":"{motor}","host":"127.0.0.1","porta":{porta},
                    "usuario":"","token_remoto":"t","timeout_s":1}}"#
            ))
            .unwrap(),
        )
        .unwrap()
    }

    /// **578: os TRES clientes do DbLink param no prazo total.** Cada um abre
    /// contra um par que goteja, pelo mesmo `abrir_com` que a producao usa, e
    /// tem de voltar com `LimiteExcedido` perto do total -- nem antes (o
    /// silencio nao estourou), nem nunca.
    ///
    /// # Prova real
    ///
    /// Com o defeito reposto (so o prazo de silencio, o de antes), nenhum dos
    /// tres volta em `PACIENCIA`: o vermelho lista quais ficaram presos.
    #[test]
    fn o_par_que_goteja_para_no_prazo_total() {
        let casos: [(&str, &'static [u8]); 3] = [
            ("mysql", &[0xFF, 0xFF, 0x00, 0x00]),
            ("postgres", &[b'R', 0x00, 0x01, 0x00, 0x00]),
            ("phxsql", b""),
        ];
        let (avisar, avisos) = mpsc::channel();
        for (motor, prefixo) in casos {
            let d = ligacao(motor, par_que_goteja(prefixo));
            let avisar = avisar.clone();
            std::thread::spawn(move || {
                let relogio = Instant::now();
                let r = d
                    .abrir_com(Prazo::com_total(SILENCIO, TOTAL, &ROTULO))
                    .map(|_| ());
                let _ = avisar.send((motor, r, relogio.elapsed()));
            });
        }
        drop(avisar);
        let prazo_final = Instant::now() + PACIENCIA;
        let mut voltaram = Vec::new();
        while voltaram.len() < casos.len() {
            let resta = prazo_final.saturating_duration_since(Instant::now());
            match avisos.recv_timeout(resta) {
                Ok(v) => voltaram.push(v),
                Err(_) => break,
            }
        }
        let presos: Vec<&str> = casos
            .iter()
            .map(|(m, _)| *m)
            .filter(|m| !voltaram.iter().any(|(v, _, _)| v == m))
            .collect();
        assert!(
            presos.is_empty(),
            "o par que goteja prendeu a thread alem de {PACIENCIA:?} -- sem prazo total: {presos:?}"
        );
        for (motor, r, durou) in voltaram {
            match r {
                Err(PhxError::LimiteExcedido(m)) => {
                    assert!(m.contains("prazo total"), "{motor}: {m}")
                }
                outro => panic!("{motor}: esperava o prazo total, veio {outro:?}"),
            }
            assert!(
                durou >= TOTAL - PASSO,
                "{motor}: cortou em {durou:?}, antes do total {TOTAL:?}"
            );
        }
    }
}

#[cfg(test)]
mod testes_do_teto_de_bytes {
    //! Pedido 546: o par que anuncia linhas enormes, pelos TRES clientes.

    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    /// O teto da prova: 1 MiB, o menor que a ligacao aceita escrever.
    const MAX_MIB: u64 = 1;
    /// Cada linha do par: 64 KiB numa celula so.
    const CELULA: usize = 64 * 1024;
    /// Quantas linhas o par manda: 4 MiB no total, quatro vezes o teto. Com o
    /// defeito reposto o cliente guarda as 64; com o conserto para antes da
    /// decima sexta.
    const LINHAS: usize = 64;

    type Conversa = fn(TcpStream, usize, usize) -> std::io::Result<()>;

    fn ligacao(motor: &str, porta: u16, extra: &str) -> Definicao {
        // O par PostgreSQL(R) desta prova e `trust`, e `trust` com senha na
        // ligacao e recusa desde o pedido 612: sem senha, ele e o que o
        // cadastro escolheu.
        let senha = if motor == "postgres" { "" } else { "s" };
        Definicao::de_json(
            &Json::analisar(&format!(
                r#"{{"nome":"prova546","motor":"{motor}","host":"127.0.0.1","porta":{porta},
                    "usuario":"","senha":"{senha}","token_remoto":"t","cifra":false,
                    "timeout_s":5{extra}}}"#
            ))
            .unwrap(),
        )
        .unwrap()
    }

    /// Um par que atende UMA conexao e roda `conversa` nela. Erro de escrita
    /// encerra a conversa -- e o que acontece quando o cliente recusa e larga
    /// o soquete no meio do resultado.
    fn par(conversa: Conversa, linhas: usize, celula: usize) -> u16 {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((s, _)) = ouvinte.accept() {
                let _ = conversa(s, linhas, celula);
            }
        });
        porta
    }

    // ------------------------------------------------------------ MySQL(R)

    fn quadro(s: &mut TcpStream, seq: u8, carga: &[u8]) -> std::io::Result<()> {
        let mut cab = (carga.len() as u32).to_le_bytes();
        cab[3] = seq;
        s.write_all(&cab)?;
        s.write_all(carga)
    }

    fn engolir_quadro(s: &mut TcpStream) -> std::io::Result<()> {
        let mut cab = [0u8; 4];
        s.read_exact(&mut cab)?;
        let n = u32::from_le_bytes([cab[0], cab[1], cab[2], 0]) as usize;
        s.read_exact(&mut vec![0u8; n])
    }

    fn mysql(mut s: TcpStream, linhas: usize, celula: usize) -> std::io::Result<()> {
        // Saudacao de um MySQL(R) 8 com `mysql_native_password`.
        let mut sauda = vec![10];
        sauda.extend_from_slice(b"8.0.36\0");
        sauda.extend_from_slice(&7u32.to_le_bytes());
        sauda.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 0]);
        sauda.extend_from_slice(&[0xff, 0xf7, 45, 2, 0, 0xff, 0x81, 21]);
        sauda.extend_from_slice(&[0; 10]);
        sauda.extend_from_slice(&[9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 0]);
        sauda.extend_from_slice(b"mysql_native_password\0");
        quadro(&mut s, 0, &sauda)?;
        engolir_quadro(&mut s)?;
        quadro(&mut s, 2, &[0, 0, 0, 2, 0, 0, 0])?;
        engolir_quadro(&mut s)?;
        // Uma coluna VARCHAR `a`, o fim das colunas, as linhas, o fim.
        quadro(&mut s, 1, &[1])?;
        let mut def = Vec::new();
        for campo in [&b"def"[..], b"", b"t", b"t", b"a", b"a"] {
            def.push(campo.len() as u8);
            def.extend_from_slice(campo);
        }
        def.push(0x0c);
        def.extend_from_slice(&[45, 0]);
        def.extend_from_slice(&60u32.to_le_bytes());
        def.push(0xfd);
        def.extend_from_slice(&[0, 0, 0, 0, 0]);
        quadro(&mut s, 2, &def)?;
        let eof = [0xFE, 0, 0, 2, 0];
        quadro(&mut s, 3, &eof)?;
        let mut linha = vec![0xFD];
        linha.extend_from_slice(&(celula as u32).to_le_bytes()[..3]);
        linha.extend(std::iter::repeat_n(b'x', celula));
        for k in 0..linhas {
            quadro(&mut s, 4u8.wrapping_add(k as u8), &linha)?;
        }
        quadro(&mut s, 0, &eof)
    }

    // ------------------------------------------------------- PostgreSQL(R)

    fn mensagem(s: &mut TcpStream, tipo: u8, corpo: &[u8]) -> std::io::Result<()> {
        s.write_all(&[tipo])?;
        s.write_all(&((corpo.len() + 4) as i32).to_be_bytes())?;
        s.write_all(corpo)
    }

    fn postgres(mut s: TcpStream, linhas: usize, celula: usize) -> std::io::Result<()> {
        // A abertura nao tem byte de tipo.
        let mut tam = [0u8; 4];
        s.read_exact(&mut tam)?;
        s.read_exact(&mut vec![0u8; i32::from_be_bytes(tam) as usize - 4])?;
        // `trust`: AuthenticationOk e pronto.
        mensagem(&mut s, b'R', &0i32.to_be_bytes())?;
        mensagem(&mut s, b'Z', b"I")?;
        let mut cab = [0u8; 5];
        s.read_exact(&mut cab)?;
        let n = i32::from_be_bytes([cab[1], cab[2], cab[3], cab[4]]) as usize;
        s.read_exact(&mut vec![0u8; n - 4])?;
        let mut desc = 1i16.to_be_bytes().to_vec();
        desc.extend_from_slice(b"a\0");
        desc.extend_from_slice(&0i32.to_be_bytes());
        desc.extend_from_slice(&0i16.to_be_bytes());
        desc.extend_from_slice(&25i32.to_be_bytes());
        desc.extend_from_slice(&(-1i16).to_be_bytes());
        desc.extend_from_slice(&(-1i32).to_be_bytes());
        desc.extend_from_slice(&0i16.to_be_bytes());
        mensagem(&mut s, b'T', &desc)?;
        let mut linha = 1i16.to_be_bytes().to_vec();
        linha.extend_from_slice(&(celula as i32).to_be_bytes());
        linha.extend(std::iter::repeat_n(b'x', celula));
        for _ in 0..linhas {
            mensagem(&mut s, b'D', &linha)?;
        }
        mensagem(&mut s, b'C', format!("SELECT {linhas}\0").as_bytes())?;
        mensagem(&mut s, b'Z', b"I")
    }

    // ------------------------------------------------------------- PhxSql

    /// Descarta um pedido do cliente, byte a byte ate o `\n`. Sem leitura de
    /// linha de soquete: o `conferidor_canal` conta toda uma que nao passe
    /// pelo `Canal`, e o par desta prova nao precisa do conteudo.
    fn engolir_pedido(s: &mut TcpStream) -> std::io::Result<()> {
        let mut b = [0u8; 1];
        loop {
            s.read_exact(&mut b)?;
            if b[0] == b'\n' {
                return Ok(());
            }
        }
    }

    fn phx(mut s: TcpStream, linhas: usize, celula: usize) -> std::io::Result<()> {
        engolir_pedido(&mut s)?;
        s.write_all(b"{\"ok\":true,\"resultado\":{\"phxsql\":\"prova\",\"papel\":\"isolado\"}}\n")?;
        engolir_pedido(&mut s)?;
        let valor = "x".repeat(celula);
        let corpo: Vec<String> = (0..linhas)
            .map(|_| format!("{{\"a\":\"{valor}\"}}"))
            .collect();
        let resposta = format!(
            "{{\"ok\":true,\"resultado\":{{\"colunas\":[\"a\"],\"linhas\":[{}]}}}}\n",
            corpo.join(",")
        );
        s.write_all(resposta.as_bytes())
    }

    const OS_TRES: [(&str, Conversa); 3] =
        [("mysql", mysql), ("postgres", postgres), ("phxsql", phx)];

    /// Os bytes guardados que a recusa diz -- o contador do teto, e nao a RAM.
    fn guardados(m: &str) -> u64 {
        let (antes, _) = m.split_once(" bytes ja guardados").expect(m);
        antes.rsplit(' ').next().unwrap().parse().expect(m)
    }

    /// **546: os TRES clientes recusam o resultado que passa do teto de
    /// bytes, e recusam ENQUANTO leem.** O par manda 64 linhas de 64 KiB --
    /// quatro vezes o `max_mib` de 1 -- dentro do `max_linhas`, que e onde o
    /// teto de linhas nao ajuda. A recusa tem de vir com `LimiteExcedido`,
    /// nomeando `max_mib`, e com o contador do que ja se guardou abaixo do
    /// teto: e ele que mede o pico, sem alocar nada perto de gigabytes.
    ///
    /// # Prova real
    ///
    /// Com o defeito reposto (o `Acumulador` sem a conta de bytes), os tres
    /// voltam `Ok` com as 64 linhas guardadas, e o vermelho diz qual.
    #[test]
    fn o_par_que_anuncia_linhas_enormes_e_recusado_no_teto_de_bytes() {
        for (motor, conversa) in OS_TRES {
            let porta = par(conversa, LINHAS, CELULA);
            let d = ligacao(
                motor,
                porta,
                &format!(r#","max_linhas":100000,"max_mib":{MAX_MIB}"#),
            );
            let mut c = d
                .abrir()
                .unwrap_or_else(|e| panic!("{motor}: nao abriu: {e}"));
            match c.consultar_em("", "SELECT a FROM t", d.max_linhas) {
                Err(PhxError::LimiteExcedido(m)) => {
                    assert!(m.contains("max_mib"), "{motor}: {m}");
                    let pico = guardados(&m);
                    assert!(
                        pico <= d.teto_de_bytes(),
                        "{motor}: guardou {pico} bytes, acima do teto {}",
                        d.teto_de_bytes()
                    );
                }
                Ok(r) => panic!(
                    "{motor}: guardou as {} linhas (~{} bytes) sem recusar -- sem \
                     teto de bytes no resultado",
                    r.linhas.len(),
                    r.linhas.len() * CELULA
                ),
                Err(outro) => panic!("{motor}: esperava o teto de bytes, veio {outro}"),
            }
        }
    }

    /// **O comportamento velho:** a consulta legitima pequena passa inteira
    /// com o teto de FABRICA, e o resultado grande cortado pelo teto de
    /// LINHAS continua voltando cortado -- e nao recusado --, porque a linha
    /// cortada nao entra na conta de bytes.
    #[test]
    fn a_consulta_legitima_e_o_corte_por_linhas_continuam() {
        for (motor, conversa) in OS_TRES {
            // 64 linhas de 1 KiB, sem `max_mib` escrito: o de fabrica.
            let porta = par(conversa, LINHAS, 1024);
            let d = ligacao(motor, porta, "");
            assert_eq!(d.max_mib, MIB_DO_RESULTADO_DE_FABRICA, "{motor}");
            assert_eq!(d.teto_de_bytes(), TETO_DE_BYTES_DO_RESULTADO, "{motor}");
            let r = d
                .abrir()
                .and_then(|mut c| c.consultar_em("", "SELECT a FROM t", d.max_linhas))
                .unwrap_or_else(|e| panic!("{motor}: a consulta legitima recusou: {e}"));
            assert_eq!((r.linhas.len(), r.truncado), (LINHAS, false), "{motor}");
            assert_eq!(
                r.linhas[0][0].as_deref().map(str::len),
                Some(1024),
                "{motor}"
            );

            // 4 MiB no fio e `max_mib` 1, mas so 4 linhas guardadas. No
            // `phxsql` o resultado e UMA mensagem e ela pesa crua antes de
            // ser analisada (pedido 610): 4 MiB numa linha so passariam do
            // teto antes de qualquer corte. Ali a prova usa celulas de 12 KiB
            // -- 768 KiB crus, que cabem, e que com as 64 copias somariam
            // 1,5 MiB: o corte por linhas continua sendo o que decide.
            let celula = if motor == "phxsql" { 12 * 1024 } else { CELULA };
            let porta = par(conversa, LINHAS, celula);
            let d = ligacao(motor, porta, &format!(r#","max_mib":{MAX_MIB}"#));
            let r = d
                .abrir()
                .and_then(|mut c| c.consultar_em("", "SELECT a FROM t", 4))
                .unwrap_or_else(|e| panic!("{motor}: o corte por linhas virou recusa: {e}"));
            assert_eq!((r.linhas.len(), r.truncado), (4, true), "{motor}");
        }
    }

    /// Um par `phxsql` que responde ao `sql` com UMA linha de `total` bytes e
    /// conta quantos o cliente aceitou antes de largar o soquete.
    ///
    /// A celula e um texto unico, e nao `0,0,0,...`: com o defeito reposto o
    /// cliente analisa a linha inteira, e a lista de zeros viraria gigabytes
    /// de arvore dentro da propria prova. O texto custa a linha e mais nada,
    /// e o que a prova mede -- quanto o cliente LEU -- e o mesmo.
    fn par_da_linha_enorme(total: usize) -> (u16, std::sync::mpsc::Receiver<usize>) {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let (avisar, aviso) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = ouvinte.accept() else {
                return;
            };
            let _ = s.set_write_timeout(Some(std::time::Duration::from_secs(20)));
            let mut aceitos = 0usize;
            let _ = (|| -> std::io::Result<()> {
                engolir_pedido(&mut s)?;
                s.write_all(
                    b"{\"ok\":true,\"resultado\":{\"phxsql\":\"prova\",\"papel\":\"isolado\"}}\n",
                )?;
                engolir_pedido(&mut s)?;
                let cabeca =
                    b"{\"ok\":true,\"resultado\":{\"colunas\":[\"a\"],\"linhas\":[{\"a\":\"";
                s.write_all(cabeca)?;
                aceitos += cabeca.len();
                let pedaco = vec![b'x'; 64 * 1024];
                // Teto de voltas: `total` / 64 KiB, e o laco acaba quando o
                // cliente larga o soquete (erro de escrita) ou quando tudo
                // foi aceito.
                while aceitos + pedaco.len() < total {
                    s.write_all(&pedaco)?;
                    aceitos += pedaco.len();
                }
                s.write_all(b"\"}]}}\n")?;
                aceitos = total;
                Ok(())
            })();
            let _ = avisar.send(aceitos);
        });
        (porta, aviso)
    }

    /// **610: no motor `phxsql` o teto de bytes vale ANTES da analise.** O
    /// par responde com uma linha de 64 MiB e a ligacao tem `max_mib` 1. A
    /// recusa tem de vir pelo `max_mib`, e o par tem de ter conseguido
    /// entregar MUITO menos que a linha: o cliente para de ler no teto e larga
    /// o soquete. O que se mede e QUANTO foi lido, e nao se recusou -- antes
    /// do conserto o cliente tambem recusava, so que depois de ler os 64 MiB e
    /// de montar a arvore deles.
    ///
    /// A folga de 32 MiB e a das memorias de soquete do nucleo (leitura do
    /// cliente e escrita do par, no laco local); o conserto deixa o par perto
    /// de 1 MiB mais elas, e o defeito, nos 64 MiB inteiros.
    ///
    /// # Prova real
    ///
    /// Com o defeito reposto (`pedir_pesado` lendo com o teto do `Canal` em
    /// vez de `cabe()`), o par entrega os 64 MiB e o vermelho diz quanto.
    #[test]
    fn o_phxsql_pesa_a_linha_antes_de_analisar() {
        const TOTAL: usize = 64 * 1024 * 1024;
        let (porta, aviso) = par_da_linha_enorme(TOTAL);
        let d = ligacao(
            "phxsql",
            porta,
            &format!(r#","max_linhas":100000,"max_mib":{MAX_MIB}"#),
        );
        let mut c = d.abrir().expect("abrir");
        let veredito = c.consultar_em("", "SELECT a FROM t", d.max_linhas);
        drop(c);
        let aceitos = aviso
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("o par nao terminou");
        match veredito {
            Err(PhxError::LimiteExcedido(m)) => {
                assert!(m.contains("max_mib"), "{m}");
                assert!(m.contains("antes de ser analisada"), "{m}");
            }
            Ok(_) => panic!("a linha de 64 MiB passou com max_mib 1"),
            Err(outro) => panic!("esperava o teto de bytes, veio {outro}"),
        }
        // O numero vai para a saida: e ele que a documentacao cita.
        eprintln!("610: o par entregou {aceitos} de {TOTAL} bytes antes da recusa");
        assert!(
            aceitos < 32 * 1024 * 1024,
            "o cliente leu {aceitos} bytes da linha de {TOTAL} antes de recusar: \
             o teto de max_mib {MAX_MIB} nao valeu antes da analise"
        );
    }

    /// O peso conta a MOLDURA de cada celula, e nao so o texto: uma linha de
    /// NULOS, que custa um byte por celula no fio, nao e de graca.
    #[test]
    fn a_celula_nula_tambem_pesa() {
        let nulos: Linha = vec![None; 4096];
        assert!(peso_da_linha(&nulos) >= 4096 * 24);
        let mut a = Acumulador::novo(u64::MAX, 4096 * 24);
        assert!(matches!(a.receber(nulos), Err(PhxError::LimiteExcedido(_))));
        assert_eq!(a.bytes(), 0, "a linha recusada nao pode ficar guardada");
    }

    /// Fora da faixa, o `max_mib` e grampeado -- e nunca vira «sem teto».
    #[test]
    fn o_max_mib_e_grampeado() {
        for (escrito, vale) in [(0i64, 1u64), (-5, 1), (9_000_000, MIB_DO_RESULTADO_MAXIMO)] {
            let d = ligacao("mysql", 3306, &format!(r#","max_mib":{escrito}"#));
            assert_eq!(d.max_mib, vale, "{escrito}");
        }
        // E viaja para a tela e para o disco -- este, so quando foi escolhido:
        // o de fabrica gravado mudaria o arquivo de quem nao pediu nada.
        let d = ligacao("mysql", 3306, r#","max_mib":300"#);
        assert_eq!(d.para_json().inteiro_ou("max_mib", 0), 300);
        assert_eq!(d.para_disco(None).unwrap().inteiro_ou("max_mib", 0), 300);
        let de_fabrica = ligacao("mysql", 3306, "");
        assert!(de_fabrica
            .para_disco(None)
            .unwrap()
            .campo("max_mib")
            .is_none());
    }
}
