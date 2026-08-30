//! O gestor de travas das transacoes: intencao na tabela, exclusivo na linha.
//!
//! # O que mudou, e o que pagou pela mudanca
//!
//! O desenho escrito em `docs/TRANSACOES.md` §4.2 escolhia **reserva de tabela
//! SEM espera**: quem esbarrava recebia a recusa na hora. Aquilo tornava o
//! abraco mortal impossivel por construcao -- sem espera nao ha grafo de
//! espera, e sem grafo nao ha ciclo --, e o preco era um conflito ARTIFICIAL:
//! quinhentos caixas vendendo, um mexendo no pedido 9001 e outro no 18223, sem
//! disputa nenhuma de verdade, e mesmo assim um esperava o outro.
//!
//! Travar por LINHA desfaz o conflito artificial e traz a espera de volta --
//! logo, traz de volta a possibilidade de ciclo. **O que paga por isso e a
//! declaracao previa do escopo**: com as tabelas conhecidas na abertura, as
//! travas de tabela sao adquiridas SEMPRE na mesma ordem canonica, e o ciclo
//! classico (A pega `pedidos` e quer `estoque` enquanto B pega `estoque` e
//! quer `pedidos`) deixa de existir.
//!
//! **E a garantia nao e total, e isso tem de ficar dito.** A ordenacao mata o
//! ciclo entre TABELAS. Ciclo entre LINHAS da MESMA tabela continua possivel:
//! A trava a linha 5 e depois quer a 9, B trava a 9 e depois quer a 5. Para
//! esse caso a resposta nao e prevenir, e limitar: o `LOCK TIMEOUT` transforma
//! a espera num erro nomeado com numero, e nunca numa thread pendurada. Este
//! motor **nao promete «sem deadlock»** -- promete ordem canonica entre
//! tabelas e espera limitada dentro delas.
//!
//! # Os quatro modos
//!
//! | modo | na tabela | na linha |
//! |---|---|---|
//! | `AUTO` (padrao) | intencao (IX) | exclusiva |
//! | `ROW` | intencao (IX) | exclusiva |
//! | `TABLE` | exclusiva (X) | nao precisa |
//! | `EXCLUSIVE` | exclusiva (X) | nao precisa |
//!
//! `AUTO` e `ROW` sao o mesmo comportamento hoje, e a diferenca esta na
//! promessa: `ROW` diz «trave por linha», e `AUTO` diz «escolha por mim», o
//! que deixa a porta aberta para o dia em que a escolha depender do numero de
//! linhas tocadas. Enquanto a escolha for uma so, dizer que sao dois modos
//! diferentes seria mentir sobre o mecanismo.
//!
//! `TABLE` e `EXCLUSIVE` tambem coincidem hoje, e pelo mesmo motivo: nao ha
//! trava de tabela COMPARTILHADA para leitura, porque leitura nao trava nada
//! neste desenho (nada nao confirmado existe em disco). O dia em que existir,
//! `TABLE` sera a exclusiva de escrita e `EXCLUSIVE` a que barra ate leitor.

use std::collections::HashMap;

/// Como uma transacao trava o que toca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Modo {
    #[default]
    Auto,
    Linha,
    Tabela,
    Exclusivo,
}

impl Modo {
    pub fn nome(self) -> &'static str {
        match self {
            Modo::Auto => "AUTO",
            Modo::Linha => "ROW",
            Modo::Tabela => "TABLE",
            Modo::Exclusivo => "EXCLUSIVE",
        }
    }

    pub fn de_texto(t: &str) -> Option<Modo> {
        Some(match t.trim().to_ascii_uppercase().as_str() {
            "" | "AUTO" => Modo::Auto,
            "ROW" | "LINHA" => Modo::Linha,
            "TABLE" | "TABELA" => Modo::Tabela,
            "EXCLUSIVE" | "EXCLUSIVO" => Modo::Exclusivo,
            _ => return None,
        })
    }

    /// Que trava de tabela este modo pede.
    pub fn na_tabela(self) -> Trava {
        match self {
            Modo::Auto | Modo::Linha => Trava::Intencao,
            Modo::Tabela | Modo::Exclusivo => Trava::Exclusiva,
        }
    }

    /// Este modo trava linha a linha?
    pub fn trava_linha(self) -> bool {
        matches!(self, Modo::Auto | Modo::Linha)
    }
}

/// O que o escopo declarado faz com uma tabela que nao esta nele.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EscopoModo {
    /// Expande o escopo e AVISA. E o padrao, e e o padrao por regra da casa:
    /// `STRICT` como padrao recusaria toda escrita de todo cliente que nunca
    /// declarou escopo -- guarda nova entra pedida, nao imposta.
    #[default]
    Dinamico,
    /// Recusa a tabela nao declarada. E o que um ERP ou um financeiro quer, e
    /// por isso existe -- mas se pede.
    Estrito,
}

impl EscopoModo {
    pub fn nome(self) -> &'static str {
        match self {
            EscopoModo::Dinamico => "DYNAMIC",
            EscopoModo::Estrito => "STRICT",
        }
    }

    pub fn de_texto(t: &str) -> Option<EscopoModo> {
        Some(match t.trim().to_ascii_uppercase().as_str() {
            "" | "DYNAMIC" | "DINAMICO" => EscopoModo::Dinamico,
            "STRICT" | "ESTRITO" => EscopoModo::Estrito,
            _ => return None,
        })
    }
}

/// A trava que uma transacao tem numa tabela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trava {
    /// Intencao de escrever em ALGUMAS linhas. Duas intencoes convivem.
    Intencao,
    /// A tabela inteira. Nao convive com nada.
    Exclusiva,
}

impl Trava {
    pub fn nome(self) -> &'static str {
        match self {
            Trava::Intencao => "IX",
            Trava::Exclusiva => "X",
        }
    }
}

#[derive(Debug, Default)]
struct EstadoDaTabela {
    /// Transacoes com intencao (IX). Convivem entre si.
    intencao: Vec<u64>,
    /// A transacao com a tabela inteira (X), se houver.
    exclusiva: Option<u64>,
    /// rowid -> transacao que o segura.
    linhas: HashMap<u64, u64>,
}

impl EstadoDaTabela {
    fn vazio(&self) -> bool {
        self.intencao.is_empty() && self.exclusiva.is_none() && self.linhas.is_empty()
    }
}

/// Quem segura o que. Uma trava propria, nunca a de dados.
#[derive(Debug, Default)]
pub struct Travas {
    /// Chave `database/tabela` em caixa baixa -- a mesma da reserva de carga.
    tabelas: HashMap<String, EstadoDaTabela>,
}

/// Quem barrou, e por que. E o que a mensagem de recusa precisa dizer: sem
/// nomear quem segura, «tabela em transacao» manda a pessoa procurar sozinha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Barrada {
    pub transacao: u64,
    pub tabela: String,
    /// `None` quando o conflito e na tabela inteira.
    pub rowid: Option<u64>,
    pub trava: Trava,
}

/// O slot que um `INSERT` disputa: o FIM da tabela.
///
/// # Por que anexar tambem trava
///
/// Porque o rowid E o endereco, e o proximo e `slots() + 1`. Duas transacoes
/// que anexam na mesma tabela ao mesmo tempo preveem o MESMO slot -- e a
/// segunda descobriria isso so na passada de commit, com metade do trabalho
/// gravado. Travar o fim transforma essa colisao numa espera limitada e num
/// erro nomeado, que e o que ela e de verdade: duas transacoes disputando o
/// mesmo lugar.
///
/// Zero serve de sentinela porque rowid zero nao existe -- o protocolo exige
/// `rowid` maior que zero desde sempre.
pub const FIM_DA_TABELA: u64 = 0;

impl Travas {
    /// Quem barra esta pretensao de tabela, sem tomar nada.
    ///
    /// Existe separado do `pegar_` porque QUEM PERGUNTA nem sempre e uma
    /// transacao: uma escrita comum -- sem `BEGIN` nenhum -- tambem precisa
    /// respeitar a trava de quem tem, e ela nao tem id de transacao para
    /// registrar. Passa `tx = 0` e so pergunta.
    pub fn conflito_de_tabela(&self, chave: &str, tx: u64, quero: Trava) -> Option<Barrada> {
        let e = self.tabelas.get(chave)?;
        if let Some(dono) = e.exclusiva {
            if dono != tx {
                return Some(Barrada {
                    transacao: dono,
                    tabela: chave.to_string(),
                    rowid: None,
                    trava: Trava::Exclusiva,
                });
            }
        }
        if quero == Trava::Exclusiva {
            // A exclusiva nao convive com a intencao de OUTRO. A propria
            // intencao desta transacao pode subir para exclusiva -- e
            // legitimo, e acontece quando alguem pede `LOCK MODE EXCLUSIVE`
            // depois de ja ter tocado a tabela.
            if let Some(outro) = e.intencao.iter().find(|o| **o != tx) {
                return Some(Barrada {
                    transacao: *outro,
                    tabela: chave.to_string(),
                    rowid: None,
                    trava: Trava::Intencao,
                });
            }
            // Linha de outra transacao tambem barra a exclusiva: ela nao pode
            // ficar por cima de escrita alheia ja anunciada.
            if let Some((rowid, outro)) = e.linhas.iter().find(|(_, o)| **o != tx) {
                return Some(Barrada {
                    transacao: *outro,
                    tabela: chave.to_string(),
                    rowid: Some(*rowid),
                    trava: Trava::Intencao,
                });
            }
        }
        None
    }

    /// Quem barra esta pretensao de linha, sem tomar nada.
    pub fn conflito_de_linha(&self, chave: &str, tx: u64, rowid: u64) -> Option<Barrada> {
        let e = self.tabelas.get(chave)?;
        if let Some(dono) = e.exclusiva {
            if dono != tx {
                return Some(Barrada {
                    transacao: dono,
                    tabela: chave.to_string(),
                    rowid: None,
                    trava: Trava::Exclusiva,
                });
            }
        }
        match e.linhas.get(&rowid) {
            Some(dono) if *dono != tx => Some(Barrada {
                transacao: *dono,
                tabela: chave.to_string(),
                rowid: Some(rowid),
                trava: Trava::Exclusiva,
            }),
            _ => None,
        }
    }

    /// Toma a trava de tabela para esta transacao. `Err` = quem barrou.
    ///
    /// Idempotente: pedir de novo o que ja e seu nao falha e nao duplica --
    /// uma transacao toca a mesma tabela dezenas de vezes.
    pub fn pegar_tabela(&mut self, chave: &str, tx: u64, quero: Trava) -> Result<(), Barrada> {
        if let Some(b) = self.conflito_de_tabela(chave, tx, quero) {
            return Err(b);
        }
        let e = self.tabelas.entry(chave.to_string()).or_default();
        match quero {
            Trava::Intencao => {
                if !e.intencao.contains(&tx) {
                    e.intencao.push(tx);
                }
            }
            Trava::Exclusiva => {
                e.intencao.retain(|o| *o != tx);
                e.exclusiva = Some(tx);
            }
        }
        Ok(())
    }

    /// Toma a trava exclusiva de UMA linha (ou do [`FIM_DA_TABELA`]).
    pub fn pegar_linha(&mut self, chave: &str, tx: u64, rowid: u64) -> Result<(), Barrada> {
        if let Some(b) = self.conflito_de_linha(chave, tx, rowid) {
            return Err(b);
        }
        self.tabelas
            .entry(chave.to_string())
            .or_default()
            .linhas
            .insert(rowid, tx);
        Ok(())
    }

    /// Solta tudo que esta transacao segura. Roda no `COMMIT`, no `ROLLBACK`,
    /// no estouro do prazo e na queda da conexao -- quatro portas, uma saida.
    pub fn soltar_tudo(&mut self, tx: u64) {
        self.tabelas.retain(|_, e| {
            e.intencao.retain(|o| *o != tx);
            if e.exclusiva == Some(tx) {
                e.exclusiva = None;
            }
            e.linhas.retain(|_, o| *o != tx);
            !e.vazio()
        });
    }

    /// O que esta transacao segura: `(tabela, trava, quantas linhas)`.
    ///
    /// E a ficha de diagnostico, e ela so mostra o que e MEDIDO aqui dentro --
    /// nao ha linha inventada porque a ficha do capitulo tinha uma.
    pub fn ficha(&self, tx: u64) -> Vec<(String, &'static str, u64)> {
        let mut v: Vec<(String, &'static str, u64)> = self
            .tabelas
            .iter()
            .filter_map(|(k, e)| {
                let linhas = e.linhas.values().filter(|o| **o == tx).count() as u64;
                // Linha travada sem a intencao registrada nao acontece hoje --
                // a tabela e tomada antes da linha, sempre --, mas a ficha nao
                // pode SUMIR com uma trava por causa disso: ficha que esconde
                // trava e pior que ficha nenhuma.
                let trava = if e.exclusiva == Some(tx) {
                    Trava::Exclusiva.nome()
                } else if e.intencao.contains(&tx) || linhas > 0 {
                    Trava::Intencao.nome()
                } else {
                    return None;
                };
                Some((k.clone(), trava, linhas))
            })
            .collect();
        v.sort();
        v
    }

    /// Quantas tabelas tem alguma trava viva. Para o painel.
    pub fn quantas(&self) -> usize {
        self.tabelas.len()
    }
}

/// A ordem canonica de aquisicao.
///
/// # Por que o NOME serve de id interno
///
/// O que a ordem canonica exige e uma ordem total ESTAVEL sobre as tabelas --
/// qualquer uma serve, desde que todas as transacoes usem a mesma. Um id
/// numerico interno teria de ser inventado, gravado em algum lugar e mantido
/// estavel entre restaurações de backup; o nome qualificado em caixa baixa ja
/// e exatamente isso, e ja e a chave que a reserva de carga usa. Inventar um
/// segundo identificador para a mesma tabela seria criar a segunda verdade que
/// este projeto passa o tempo todo evitando.
pub fn em_ordem_canonica(chaves: &mut [String]) {
    chaves.sort();
    // A ordenacao so serve se nao houver repetida esperando duas vezes.
    debug_assert!(chaves.windows(2).all(|p| p[0] != p[1]));
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn duas_intencoes_convivem_e_a_exclusiva_nao() {
        let mut t = Travas::default();
        assert!(t.pegar_tabela("loja/pedidos", 1, Trava::Intencao).is_ok());
        assert!(t.pegar_tabela("loja/pedidos", 2, Trava::Intencao).is_ok());
        // A exclusiva de um terceiro esbarra na intencao dos dois.
        let e = t
            .pegar_tabela("loja/pedidos", 3, Trava::Exclusiva)
            .unwrap_err();
        assert!(e.transacao == 1 || e.transacao == 2);
        assert_eq!(e.rowid, None);
    }

    /// O caso que matou o exclusivo-por-padrao: dois caixas em pedidos
    /// diferentes nao disputam nada.
    #[test]
    fn dois_caixas_em_linhas_diferentes_nao_se_esbarram() {
        let mut t = Travas::default();
        t.pegar_tabela("loja/pedidos", 1, Trava::Intencao).unwrap();
        t.pegar_tabela("loja/pedidos", 2, Trava::Intencao).unwrap();
        assert!(t.pegar_linha("loja/pedidos", 1, 9001).is_ok());
        assert!(t.pegar_linha("loja/pedidos", 2, 18223).is_ok());
        // E na MESMA linha eles se esbarram, que e o conflito de verdade.
        let e = t.pegar_linha("loja/pedidos", 2, 9001).unwrap_err();
        assert_eq!(e.transacao, 1);
        assert_eq!(e.rowid, Some(9001));
    }

    #[test]
    fn a_exclusiva_de_tabela_barra_ate_a_linha_do_outro() {
        let mut t = Travas::default();
        t.pegar_tabela("loja/estoque", 1, Trava::Exclusiva).unwrap();
        let e = t.pegar_linha("loja/estoque", 2, 7).unwrap_err();
        assert_eq!(e.transacao, 1);
        assert_eq!(e.trava, Trava::Exclusiva);
        // E o dono continua entrando.
        assert!(t.pegar_linha("loja/estoque", 1, 7).is_ok());
    }

    #[test]
    fn soltar_libera_tabela_e_linha_e_deixa_o_registro_limpo() {
        let mut t = Travas::default();
        t.pegar_tabela("loja/pedidos", 1, Trava::Intencao).unwrap();
        t.pegar_linha("loja/pedidos", 1, 5).unwrap();
        assert_eq!(t.quantas(), 1);
        t.soltar_tudo(1);
        assert_eq!(t.quantas(), 0, "a entrada vazia tem de sair do mapa");
        // E agora outro entra sem esbarrar.
        assert!(t.pegar_tabela("loja/pedidos", 2, Trava::Exclusiva).is_ok());
    }

    #[test]
    fn pedir_de_novo_o_que_ja_e_seu_nao_falha() {
        let mut t = Travas::default();
        t.pegar_tabela("loja/pedidos", 1, Trava::Intencao).unwrap();
        t.pegar_tabela("loja/pedidos", 1, Trava::Intencao).unwrap();
        t.pegar_linha("loja/pedidos", 1, 5).unwrap();
        t.pegar_linha("loja/pedidos", 1, 5).unwrap();
        let f = t.ficha(1);
        assert_eq!(f, vec![("loja/pedidos".to_string(), "IX", 1)]);
    }

    /// Subir de IX para X e legitimo quando so a propria transacao esta la.
    #[test]
    fn a_propria_intencao_vira_exclusiva() {
        let mut t = Travas::default();
        t.pegar_tabela("loja/pedidos", 1, Trava::Intencao).unwrap();
        t.pegar_tabela("loja/pedidos", 1, Trava::Exclusiva).unwrap();
        assert_eq!(t.ficha(1), vec![("loja/pedidos".to_string(), "X", 0)]);
    }

    #[test]
    fn a_ordem_canonica_e_a_mesma_venha_a_lista_como_vier() {
        let mut a: Vec<String> = ["loja/pedidos", "loja/estoque", "loja/clientes"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut b: Vec<String> = ["loja/clientes", "loja/pedidos", "loja/estoque"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        em_ordem_canonica(&mut a);
        em_ordem_canonica(&mut b);
        assert_eq!(a, b);
        assert_eq!(a[0], "loja/clientes");
    }
}
