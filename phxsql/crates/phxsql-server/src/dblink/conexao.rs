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

use phxsql_core::error::Result;
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
            Motor::MySql => Conexao::MySql(Box::new(mysql::Conexao::abrir(
                &self.host,
                self.porta,
                &self.usuario,
                self.senha()?,
                &self.database,
                prazo,
            )?)),
            Motor::Postgres => Conexao::Postgres(Box::new(pg::Conexao::abrir(
                &self.host,
                self.porta,
                &self.usuario,
                self.senha()?,
                &self.database,
                prazo,
            )?)),
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
