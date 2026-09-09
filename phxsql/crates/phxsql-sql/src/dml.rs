//! `INSERT`, `UPDATE` e `DELETE` por chave -- o passo 2 do roteiro de
//! `docs/SQL.md`, o que fecha o CRUD pela camada SQL.
//!
//! # A gramatica, e ela cabe aqui
//!
//! ```text
//! INSERT INTO [database.] [schema.] tabela (coluna {, coluna}) VALUES (literal {, literal})
//!   [ON CONFLICT [(coluna)] DO NOTHING | DO UPDATE SET coluna = literal {, coluna = literal}]
//!   [ON DUPLICATE KEY UPDATE coluna = literal {, coluna = literal}]
//! UPDATE      [database.] [schema.] tabela SET coluna = literal {, coluna = literal}
//!             WHERE coluna = literal
//! DELETE FROM [database.] [schema.] tabela WHERE coluna = literal
//! ```
//!
//! Uma linha por `INSERT`, a lista de colunas obrigatoria, o `WHERE` de
//! igualdade sobre uma coluna com indice UNICO -- e o que falta recusa dizendo
//! o que falta, com o nome da clausula, como o `SELECT` ja faz.
//!
//! `ON CONFLICT`/`ON DUPLICATE KEY UPDATE` (item 7) viram `inserir` com
//! `se_existir: "ignorar"|"atualizar"`. So o SET aceita LITERAL -- nem
//! `excluded.coluna` (a linha que tentou entrar) nem expressao tem
//! substrato, e os dois recusam nomeando o motivo.
//!
//! # O que este modulo NAO e: uma traducao direta
//!
//! A tabela da secao 1 de `docs/SQL.md` mapeia `UPDATE` para `atualizar`, e a
//! primeira leitura sugere que basta trocar o verbo pelo nome da operacao. Nao
//! basta, e o motivo esta em `valores.rs`: o `atualizar` recebe a linha
//! INTEIRA por `json_para_linha`, e **coluna ausente entra como NULL**. Um
//! `UPDATE t SET nome = 'x' WHERE id = 5` traduzido direto mandaria so `nome`
//! -- e zeraria `cidade`, `telefone` e tudo o mais, sem erro nenhum.
//!
//! Por isso `UPDATE` e `DELETE` por chave sao TRES passos, e o plano diz isso:
//!
//! 1. `buscar` no indice unico acha o `rowid` da chave;
//! 2. `ler` (com `com_versao`) traz a linha inteira e a `versao` dela;
//! 3. `atualizar` grava a linha MESCLADA (a lida, com o `SET` por cima), ou
//!    `excluir` marca a linha -- os dois levando a `versao` lida.
//!
//! A `versao` vai junto por decisao, nao por acaso: os tres passos abrem uma
//! janela entre ler e gravar, e a janela de conflito de escrita existe
//! justamente para isso. *Guarda nova entra pedida, nao imposta* -- e a camada
//! SQL e cliente NOVO, entao ela pede. Quem gravar a linha entre o passo 2 e o
//! 3 faz o motor recusar em vez de ser sobrescrito em silencio.
//!
//! # Quem executa os passos e o servidor, nao este crate
//!
//! Este modulo traduz texto em pedidos e nao abre arquivo: o `PlanoDml` carrega
//! o pedido de `buscar` pronto e o que fazer com a resposta dele, e a op `sql`
//! do servidor executa cada passo pelo MESMO `executar_derivado` que o `SELECT`
//! usa. O portao continua sendo um: cada pedido traduzido carrega o campo
//! `tabela` que ele ja sabe olhar.
//!
//! # Por que uma linha por INSERT
//!
//! `VALUES (…), (…)` traduziria para `inserir_lote` -- e `inserir_lote` NAO e
//! empilhavel numa transacao aberta (ver `OPS_EMPILHAVEIS` no servidor). Um
//! `BEGIN; INSERT … VALUES (a), (b); ROLLBACK` gravaria as duas linhas por
//! fora da transacao, com cara de ter desfeito. A recusa nomeia o caminho
//! certo para carga: `BULKINSERT` e a operacao `inserir_lote`, fora de
//! transacao.

use crate::lexico::{self, Comparador, Token};
use crate::sintaxe::{Alvo, Analisador, Condicao, Literal};
use crate::traduzir::{
    base_do_pedido, igual_sem_caso, literal_para_json, pedido_com_op, IndiceInfo,
};
use phxsql_core::json::Json;
use phxsql_core::{PhxError, Result};

/// `INSERT INTO t (a, b) VALUES (1, 'x')`, ja separado.
#[derive(Debug, Clone, PartialEq)]
pub struct Insercao {
    pub em: Alvo,
    pub colunas: Vec<String>,
    pub valores: Vec<Literal>,
    /// `ON CONFLICT (...) DO NOTHING|UPDATE` ou `ON DUPLICATE KEY UPDATE` --
    /// `None` e o `INSERT` de sempre.
    pub se_existir: Option<SeExistir>,
}

/// O que fazer quando a linha ja existe -- o item 7 do roteiro.
#[derive(Debug, Clone, PartialEq)]
pub enum SeExistir {
    /// `ON CONFLICT (coluna) DO NOTHING`. `coluna_conflito` e `None` so no
    /// `ON CONFLICT DO NOTHING` sem alvo (Postgres aceita; o servidor
    /// escolhe o indice, como no `inserir` de sempre).
    Ignorar { coluna_conflito: Option<String> },
    /// `ON CONFLICT (coluna) DO UPDATE SET ...` ou `ON DUPLICATE KEY UPDATE
    /// ...` -- a segunda forma NUNCA tem `coluna_conflito` (o MySQL nao
    /// nomeia indice, e o servidor escolhe o primario).
    Atualizar {
        coluna_conflito: Option<String>,
        atribuicoes: Vec<(String, Literal)>,
    },
}

/// `UPDATE t SET a = 1, b = 'x' WHERE id = 5`.
#[derive(Debug, Clone, PartialEq)]
pub struct Atualizacao {
    pub em: Alvo,
    pub atribuicoes: Vec<(String, Literal)>,
    pub onde: Condicao,
}

/// `DELETE FROM t WHERE id = 5`.
#[derive(Debug, Clone, PartialEq)]
pub struct Exclusao {
    pub de: Alvo,
    pub onde: Condicao,
}

// ------------------------------------------------------------------ parser

impl Analisador {
    /// Depois do `INSERT` ja consumido.
    pub(crate) fn insercao(&mut self, pos: usize) -> Result<Insercao> {
        self.exigir_palavra("INTO")?;
        let em = self.alvo()?;
        if !self.aceitar(&Token::AbreParen) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "escreva as colunas: INSERT INTO t (a, b) VALUES (...) -- sem a lista, \
                 a ordem do esquema decidiria em silencio",
            ));
        }
        let colunas = self.lista_de_colunas()?;
        if self
            .espiar()
            .and_then(|s| s.token.palavra_chave())
            .as_deref()
            == Some("SELECT")
        {
            return Err(lexico::erro(
                self.posicao_atual(),
                "INSERT ... SELECT nao existe nesta camada: nao ha quem leia e grave no \
                 mesmo passo. Escreva VALUES (...)",
            ));
        }
        self.exigir_palavra("VALUES")?;
        if !self.aceitar(&Token::AbreParen) {
            return Err(lexico::erro(
                self.posicao_atual(),
                &format!("esperava ( depois de VALUES{}", self.mas_veio()),
            ));
        }
        let valores = self.lista_de_literais()?;
        if valores.len() != colunas.len() {
            return Err(lexico::erro(
                pos,
                &format!(
                    "{} colunas e {} valores -- as listas andam juntas",
                    colunas.len(),
                    valores.len()
                ),
            ));
        }
        if self.aceitar(&Token::Virgula) {
            return Err(lexico::erro(
                self.posicao_atual(),
                "uma linha por INSERT nesta camada. VALUES com varias linhas viraria \
                 inserir_lote, que NAO entra numa transacao aberta -- gravaria por fora \
                 dela com cara de ter desfeito. Para carga ha o BULKINSERT e a operacao \
                 inserir_lote, fora de transacao",
            ));
        }
        let se_existir = self.upsert_opcional()?;
        Ok(Insercao {
            em,
            colunas,
            valores,
            se_existir,
        })
    }

    /// `ON CONFLICT (coluna) DO NOTHING|UPDATE SET ...` (PostgreSQL/SQLite)
    /// ou `ON DUPLICATE KEY UPDATE ...` (MySQL) -- `None` quando nao ha `ON`
    /// nenhum, que e o `INSERT` de sempre.
    fn upsert_opcional(&mut self) -> Result<Option<SeExistir>> {
        if !self.aceitar_palavra("ON") {
            return Ok(None);
        }
        if self.aceitar_palavra("DUPLICATE") {
            self.exigir_palavra("KEY")?;
            self.exigir_palavra("UPDATE")?;
            let atribuicoes = self.lista_de_atribuicoes("no SET do ON DUPLICATE KEY UPDATE")?;
            return Ok(Some(SeExistir::Atualizar {
                // O MySQL nao nomeia indice -- ele resolve pela PRIMARIA ou
                // por qualquer chave unica que bateu, e o servidor ja faz
                // essa escolha quando `indice` vem ausente.
                coluna_conflito: None,
                atribuicoes,
            }));
        }
        self.exigir_palavra("CONFLICT")?;
        let coluna_conflito = if self.aceitar(&Token::AbreParen) {
            let c = self.identificador("coluna do ON CONFLICT")?;
            if self.aceitar(&Token::Virgula) {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "ON CONFLICT de mais de uma coluna nao tem substrato nesta rodada: so \
                     indice UNICO de uma coluna -- a mesma restricao do UPDATE/DELETE por \
                     chave",
                ));
            }
            if !self.aceitar(&Token::FechaParen) {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    &format!("esperava ) do ON CONFLICT ({c}{}", self.mas_veio()),
                ));
            }
            Some(c)
        } else {
            // `ON CONFLICT DO NOTHING` sem alvo -- legitimo no Postgres, e o
            // servidor decide o indice do mesmo jeito que decide quando o
            // pedido chega sem "indice" nenhum.
            None
        };
        self.exigir_palavra("DO")?;
        if self.aceitar_palavra("NOTHING") {
            return Ok(Some(SeExistir::Ignorar { coluna_conflito }));
        }
        self.exigir_palavra("UPDATE")?;
        self.exigir_palavra("SET")?;
        let atribuicoes = self.lista_de_atribuicoes("no SET do ON CONFLICT")?;
        Ok(Some(SeExistir::Atualizar {
            coluna_conflito,
            atribuicoes,
        }))
    }

    /// Depois do `UPDATE` ja consumido.
    pub(crate) fn atualizacao(&mut self, _pos: usize) -> Result<Atualizacao> {
        let em = self.alvo()?;
        self.exigir_palavra("SET")?;
        let atribuicoes = self.lista_de_atribuicoes("no SET")?;
        let onde = self.condicao_de_chave("UPDATE", "mudaria a tabela INTEIRA")?;
        Ok(Atualizacao {
            em,
            atribuicoes,
            onde,
        })
    }

    /// A lista `coluna = literal {, coluna = literal}` de um `SET` -- do
    /// `UPDATE` de sempre e do `DO UPDATE SET`/`ON DUPLICATE KEY UPDATE` do
    /// upsert, que sao a MESMA gramatica.
    ///
    /// `excluded.coluna` (a linha que tentou entrar, do Postgres) recusa
    /// NOMEANDO antes de cair na mensagem generica de "esperava um valor":
    /// a mensagem generica falaria em "comparar coluna com coluna", que nao
    /// e o que quem escreveu `excluded.preco` tentou fazer.
    fn lista_de_atribuicoes(&mut self, contexto: &str) -> Result<Vec<(String, Literal)>> {
        let mut atribuicoes: Vec<(String, Literal)> = Vec::new();
        loop {
            let coluna = self.identificador(&format!("nome de coluna {contexto}"))?;
            if atribuicoes.iter().any(|(c, _)| igual_sem_caso(c, &coluna)) {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    &format!("a coluna {coluna:?} aparece duas vezes {contexto}"),
                ));
            }
            if phxsql_core::schema::e_coluna_de_sistema(&coluna.to_lowercase()) {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    &format!(
                        "SET {coluna}: {coluna} e coluna de SISTEMA e nao se escreve pelo \
                         SQL -- a marca de excluido muda por excluir e restaurar, e o \
                         numero de ordem e do motor"
                    ),
                ));
            }
            match self.espiar().map(|s| &s.token) {
                Some(Token::Comparador(Comparador::Igual)) => self.i += 1,
                _ => {
                    return Err(lexico::erro(
                        self.posicao_atual(),
                        &format!("esperava = depois de {coluna}{}", self.mas_veio()),
                    ))
                }
            }
            if self
                .espiar()
                .and_then(|s| s.token.palavra_chave())
                .as_deref()
                == Some("EXCLUDED")
                && self.s.get(self.i + 1).map(|s| &s.token) == Some(&Token::Ponto)
            {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    "excluded.coluna nao tem substrato nesta camada: o SET so aceita \
                     literal, e \"a linha que tentou entrar\" nao vira literal nenhum -- \
                     escreva o valor direto",
                ));
            }
            let valor = self.literal_de_escrita()?;
            self.recusar_expressao(contexto)?;
            atribuicoes.push((coluna, valor));
            if self.aceitar(&Token::Virgula) {
                continue;
            }
            break;
        }
        Ok(atribuicoes)
    }

    /// Depois do `DELETE` ja consumido.
    pub(crate) fn exclusao(&mut self, _pos: usize) -> Result<Exclusao> {
        self.exigir_palavra("FROM")?;
        let de = self.alvo()?;
        let onde = self.condicao_de_chave("DELETE", "apagaria a tabela INTEIRA")?;
        Ok(Exclusao { de, onde })
    }

    fn lista_de_colunas(&mut self) -> Result<Vec<String>> {
        let mut colunas: Vec<String> = Vec::new();
        loop {
            let c = self.identificador("nome de coluna")?;
            if colunas.iter().any(|x| igual_sem_caso(x, &c)) {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    &format!("a coluna {c:?} aparece duas vezes na lista"),
                ));
            }
            colunas.push(c);
            if self.aceitar(&Token::Virgula) {
                continue;
            }
            if self.aceitar(&Token::FechaParen) {
                return Ok(colunas);
            }
            return Err(lexico::erro(
                self.posicao_atual(),
                &format!("esperava , ou ) nas colunas{}", self.mas_veio()),
            ));
        }
    }

    fn lista_de_literais(&mut self) -> Result<Vec<Literal>> {
        let mut valores = Vec::new();
        loop {
            valores.push(self.literal_de_escrita()?);
            self.recusar_expressao("nos valores")?;
            if self.aceitar(&Token::Virgula) {
                continue;
            }
            if self.aceitar(&Token::FechaParen) {
                return Ok(valores);
            }
            return Err(lexico::erro(
                self.posicao_atual(),
                &format!("esperava , ou ) nos valores{}", self.mas_veio()),
            ));
        }
    }

    /// Um literal para GRAVAR. `DEFAULT` e recusado pelo nome, porque a
    /// mensagem generica do `literal` fala em comparar coluna com coluna,
    /// que nao e o que quem escreveu DEFAULT tentou fazer.
    fn literal_de_escrita(&mut self) -> Result<Literal> {
        if self
            .espiar()
            .and_then(|s| s.token.palavra_chave())
            .as_deref()
            == Some("DEFAULT")
        {
            return Err(lexico::erro(
                self.posicao_atual(),
                "DEFAULT nao existe nesta camada: omita a coluna da lista, e ela entra \
                 como o motor decidir -- NULL, ou a sequencia, quando a coluna e uma",
            ));
        }
        self.literal()
    }

    /// `SET a = 1 + 2` e `VALUES (preco * 2)`: nao ha quem avalie, e o
    /// `literal` ja leu o primeiro numero -- sem esta guarda o `+` sobraria
    /// como "esperava WHERE", que manda procurar no lugar errado.
    fn recusar_expressao(&mut self, onde: &str) -> Result<()> {
        match self.espiar().map(|s| &s.token) {
            Some(Token::Mais | Token::Menos | Token::Barra | Token::Asterisco) => {
                Err(lexico::erro(
                    self.posicao_atual(),
                    &format!("expressao {onde} nao tem quem avalie: so literal"),
                ))
            }
            _ => Ok(()),
        }
    }

    /// O `WHERE coluna = literal` que localiza a linha. Obrigatorio, e so de
    /// igualdade: sem ele o comando alcancaria a tabela inteira, e nao ha
    /// varredura que grave.
    fn condicao_de_chave(&mut self, verbo: &str, estrago: &str) -> Result<Condicao> {
        if !self.aceitar_palavra("WHERE") {
            let fim = matches!(
                self.espiar().map(|s| &s.token),
                None | Some(Token::PontoEVirgula)
            );
            return Err(lexico::erro(
                self.posicao_atual(),
                &if fim {
                    format!(
                        "{verbo} sem WHERE {estrago}, e nao ha varredura que grave: diga a \
                         chave (WHERE coluna = valor)"
                    )
                } else {
                    format!("esperava WHERE{}", self.mas_veio())
                },
            ));
        }
        let pos = self.posicao_atual();
        let c = self.condicao()?;
        if c.op != Comparador::Igual {
            return Err(lexico::erro(
                pos,
                &format!(
                    "{verbo} por faixa ({} {} ...) nao tem substrato: o passo por chave desce \
                     o indice ate uma chave IGUAL. So `=` passa por aqui",
                    c.coluna,
                    c.op.simbolo()
                ),
            ));
        }
        Ok(c)
    }
}

// --------------------------------------------------------------- traducao

/// O plano de um comando de escrita: o pedido pronto, ou os passos ate ele.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanoDml {
    /// Um `inserir`, pronto.
    Inserir { pedido: Json, notas: Vec<String> },
    /// `buscar` pela chave; depois `ler` e `atualizar` com a linha mesclada.
    Atualizar {
        busca: Json,
        /// O `SET`, ja como o protocolo escreve cada valor.
        atribuicoes: Vec<(String, Json)>,
        notas: Vec<String>,
    },
    /// `buscar` pela chave; depois `ler` e `excluir` com a versao.
    Excluir { busca: Json, notas: Vec<String> },
}

impl PlanoDml {
    /// A operacao do protocolo que GRAVA -- a ultima do plano.
    pub fn op(&self) -> &'static str {
        match self {
            PlanoDml::Inserir { .. } => "inserir",
            PlanoDml::Atualizar { .. } => "atualizar",
            PlanoDml::Excluir { .. } => "excluir",
        }
    }

    pub fn notas(&self) -> &[String] {
        match self {
            PlanoDml::Inserir { notas, .. }
            | PlanoDml::Atualizar { notas, .. }
            | PlanoDml::Excluir { notas, .. } => notas,
        }
    }
}

/// `INSERT` vira um `inserir`. Nao precisa do esquema: quem confere coluna e
/// tipo e o motor, com a mensagem que ele ja tem.
///
/// `indices` so importa para `ON CONFLICT (coluna) ...`: e dali que sai o
/// NOME do indice unico que o campo `"indice"` do pedido espera -- o SQL
/// nomeia uma COLUNA, o protocolo espera um INDICE, e a traducao e quem faz
/// essa ponte (a mesma que `traduzir_atualizacao`/`traduzir_exclusao` ja
/// fazem para o `WHERE` por chave).
pub fn traduzir_insercao(
    i: &Insercao,
    indices: &[IndiceInfo],
    database_corrente: &str,
) -> Result<PlanoDml> {
    let database = database_de(&i.em, database_corrente, "INSERT INTO")?;
    let mut notas = nota_do_apelido(&i.em);
    let valores: Vec<(String, Json)> = i
        .colunas
        .iter()
        .zip(i.valores.iter())
        .map(|(c, v)| (c.clone(), literal_para_json(v)))
        .collect();
    let mut pares = base_do_pedido(&i.em, &database);
    pares.push(("valores".to_string(), Json::Objeto(valores)));

    match &i.se_existir {
        None => {
            notas.push(
                "INSERT vira `inserir` -- uma linha, e ela empilha numa transacao aberta \
                 como qualquer inserir"
                    .into(),
            );
        }
        Some(SeExistir::Ignorar { coluna_conflito }) => {
            pares.push(("se_existir".to_string(), Json::texto_de("ignorar")));
            if let Some(c) = coluna_conflito {
                pares.push((
                    "indice".to_string(),
                    Json::texto_de(indice_unico_da_coluna(indices, c)?),
                ));
            }
            notas.push(
                "ON CONFLICT ... DO NOTHING vira `inserir` com se_existir: \"ignorar\" -- a \
                 linha existente fica exatamente como estava"
                    .into(),
            );
        }
        Some(SeExistir::Atualizar {
            coluna_conflito,
            atribuicoes,
        }) => {
            pares.push(("se_existir".to_string(), Json::texto_de("atualizar")));
            if let Some(c) = coluna_conflito {
                pares.push((
                    "indice".to_string(),
                    Json::texto_de(indice_unico_da_coluna(indices, c)?),
                ));
            }
            let atualizar: Vec<(String, Json)> = atribuicoes
                .iter()
                .map(|(c, v)| (c.clone(), literal_para_json(v)))
                .collect();
            pares.push(("atualizar".to_string(), Json::Objeto(atualizar)));
            notas.push(
                "ON CONFLICT/ON DUPLICATE KEY ... UPDATE vira `inserir` com se_existir: \
                 \"atualizar\" -- o SET vai no campo \"atualizar\""
                    .into(),
            );
        }
    }

    Ok(PlanoDml::Inserir {
        pedido: pedido_com_op("inserir", pares),
        notas,
    })
}

/// O NOME do indice UNICO de uma coluna so -- o mesmo criterio de
/// `busca_pela_chave`, so que sem exigir chave nenhuma no pedido (o
/// `ON CONFLICT` so precisa saber QUAL indice, quem compara e o motor).
fn indice_unico_da_coluna<'a>(indices: &'a [IndiceInfo], coluna: &str) -> Result<&'a str> {
    let candidatos: Vec<&IndiceInfo> = indices
        .iter()
        .filter(|ix| ix.atende_igualdade(coluna) && (ix.unico || ix.primario))
        .collect();
    match candidatos.as_slice() {
        [ix] => Ok(&ix.nome),
        [] => Err(PhxError::Esquema(format!(
            "ON CONFLICT ({coluna}) exige um indice UNICO de uma coluna sobre {coluna}. Nao \
             existe"
        ))),
        varios => Err(PhxError::Esquema(format!(
            "ON CONFLICT ({coluna}) e ambiguo: {} indices unicos casam com essa coluna -- {}",
            varios.len(),
            varios
                .iter()
                .map(|ix| ix.nome.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// `UPDATE` por chave: o `buscar` pronto, e o `SET` para o servidor mesclar
/// na linha que ele vai ler.
pub fn traduzir_atualizacao(
    a: &Atualizacao,
    indices: &[IndiceInfo],
    database_corrente: &str,
) -> Result<PlanoDml> {
    let database = database_de(&a.em, database_corrente, "UPDATE")?;
    let mut notas = nota_do_apelido(&a.em);
    let (busca, ix) = busca_pela_chave(&a.em, &database, &a.onde, indices, "UPDATE")?;
    let atribuicoes = a
        .atribuicoes
        .iter()
        .map(|(c, v)| (c.clone(), literal_para_json(v)))
        .collect();
    notas.push(format!(
        "UPDATE por chave e TRES passos, nao um: `buscar` no indice {ix} acha o rowid, `ler` \
         traz a linha inteira e a versao, e `atualizar` grava a linha MESCLADA -- o \
         protocolo grava a linha inteira, e mandar so o SET zeraria as outras colunas"
    ));
    notas.push(
        "a versao lida vai no `atualizar`: quem gravar a linha entre os passos faz o motor \
         recusar, em vez de ser sobrescrito em silencio"
            .into(),
    );
    Ok(PlanoDml::Atualizar {
        busca,
        atribuicoes,
        notas,
    })
}

/// `DELETE` por chave: o `buscar` pronto; o servidor le a versao e exclui.
pub fn traduzir_exclusao(
    e: &Exclusao,
    indices: &[IndiceInfo],
    database_corrente: &str,
) -> Result<PlanoDml> {
    let database = database_de(&e.de, database_corrente, "DELETE FROM")?;
    let mut notas = nota_do_apelido(&e.de);
    let (busca, ix) = busca_pela_chave(&e.de, &database, &e.onde, indices, "DELETE")?;
    notas.push(format!(
        "DELETE por chave: `buscar` no indice {ix} acha o rowid, `ler` traz a versao, e \
         `excluir` marca a linha. E o excluir SUAVE, o padrao do motor: a linha some da \
         lista e fica no arquivo, reversivel por `restaurar`. Para apagar de vez ha a \
         operacao excluir com `fisico`"
    ));
    Ok(PlanoDml::Excluir { busca, notas })
}

fn database_de(alvo: &Alvo, corrente: &str, verbo: &str) -> Result<String> {
    let d = if alvo.database.is_empty() {
        corrente
    } else {
        &alvo.database
    };
    if d.is_empty() {
        return Err(PhxError::Esquema(format!(
            "nao sei em qual database: escreva {verbo} banco.tabela ou escolha o banco antes"
        )));
    }
    Ok(d.to_string())
}

fn nota_do_apelido(alvo: &Alvo) -> Vec<String> {
    match &alvo.apelido {
        Some(a) => vec![format!(
            "o apelido {a:?} da tabela foi lido e descartado: so ha uma tabela"
        )],
        None => Vec::new(),
    }
}

/// O `buscar` que localiza a linha da chave, e o nome do indice escolhido.
///
/// So indice UNICO de uma coluna serve. Um indice comum acharia varias linhas,
/// e um `UPDATE` que alcanca N linhas sem dizer quantas e a resposta errada
/// com cara de certa -- a mesma recusa que o `SELECT` faz com a varredura.
fn busca_pela_chave(
    alvo: &Alvo,
    database: &str,
    c: &Condicao,
    indices: &[IndiceInfo],
    verbo: &str,
) -> Result<(Json, String)> {
    let candidatos: Vec<&IndiceInfo> = indices
        .iter()
        .filter(|i| i.atende_igualdade(&c.coluna))
        .collect();
    let Some(ix) = candidatos.iter().find(|i| i.unico || i.primario) else {
        if let Some(comum) = candidatos.first() {
            return Err(PhxError::Esquema(format!(
                "WHERE {} = ... acha linhas pelo indice {}, mas ele NAO e unico: um {verbo} \
                 por ele alcancaria varias linhas e o SQL nao diz quantas. O passo por \
                 chave exige indice unico (ou primario) de uma coluna sobre {}",
                c.coluna, comum.nome, c.coluna
            )));
        }
        let unicos: Vec<&str> = indices
            .iter()
            .filter(|i| i.colunas.len() == 1 && (i.unico || i.primario))
            .map(|i| i.colunas[0].nome.as_str())
            .collect();
        return Err(PhxError::Esquema(format!(
            "{verbo} por chave exige um indice UNICO de uma coluna sobre {}. Nao existe. Ha \
             indice unico de coluna unica sobre: {}",
            c.coluna,
            if unicos.is_empty() {
                "nenhuma coluna".to_string()
            } else {
                unicos.join(", ")
            }
        )));
    };
    let mut pares = base_do_pedido(alvo, database);
    pares.push(("indice".to_string(), Json::texto_de(&ix.nome)));
    pares.push((
        "chave".to_string(),
        Json::Lista(vec![literal_para_json(&c.valor)]),
    ));
    // Um indice unico devolve no maximo uma linha; pedir duas e a guarda que
    // deixa o servidor acusar um estado que nao deveria existir, em vez de
    // gravar na primeira e calar sobre a segunda.
    pares.push(("max".to_string(), Json::de_u64(2)));
    Ok((pedido_com_op("buscar", pares), ix.nome.clone()))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::sintaxe::{analisar, analisar_comando, Comando};
    use crate::traduzir::ColunaDoIndice;

    fn ix(nome: &str, coluna: &str, unico: bool, primario: bool) -> IndiceInfo {
        IndiceInfo {
            nome: nome.into(),
            colunas: vec![ColunaDoIndice {
                nome: coluna.into(),
                desc: false,
            }],
            unico,
            primario,
        }
    }

    fn insercao(sql: &str) -> Insercao {
        match analisar_comando(sql).unwrap() {
            Comando::Insercao(i) => i,
            outro => panic!("{sql} nao deu INSERT: {outro:?}"),
        }
    }

    fn atualizacao(sql: &str) -> Atualizacao {
        match analisar_comando(sql).unwrap() {
            Comando::Atualizacao(a) => a,
            outro => panic!("{sql} nao deu UPDATE: {outro:?}"),
        }
    }

    fn exclusao(sql: &str) -> Exclusao {
        match analisar_comando(sql).unwrap() {
            Comando::Exclusao(e) => e,
            outro => panic!("{sql} nao deu DELETE: {outro:?}"),
        }
    }

    // ------------------------------------------------------------ parser

    #[test]
    fn insert_com_colunas_e_valores() {
        let i = insercao("INSERT INTO loja.clientes (id, nome, ativo) VALUES (7, 'Ana', TRUE)");
        // Duas partes e schema.tabela -- a regra do SELECT; tres partes e
        // database.schema.tabela. O banco vem do envelope, ou das tres.
        assert_eq!(i.em.database, "");
        assert_eq!(i.em.schema, "loja");
        assert_eq!(i.em.tabela, "clientes");
        assert_eq!(i.em.nome_no_protocolo(), "loja.clientes");
        assert_eq!(i.colunas, vec!["id", "nome", "ativo"]);
        assert_eq!(
            i.valores,
            vec![
                Literal::Numero("7".into()),
                Literal::Texto("Ana".into()),
                Literal::Bool(true)
            ]
        );
    }

    #[test]
    fn update_com_set_e_where_de_chave() {
        let a = atualizacao("UPDATE clientes SET nome = 'Bia', saldo = 10.50 WHERE id = 7");
        assert_eq!(a.em.tabela, "clientes");
        assert_eq!(a.atribuicoes.len(), 2);
        assert_eq!(a.atribuicoes[1].0, "saldo");
        // Decimal continua texto: nao passa por f64 em ponto nenhum.
        assert_eq!(a.atribuicoes[1].1, Literal::Numero("10.50".into()));
        assert_eq!(a.onde.coluna, "id");
        assert_eq!(a.onde.op, Comparador::Igual);
    }

    #[test]
    fn delete_com_where_de_chave() {
        let e = exclusao("DELETE FROM matriz.estoque WHERE codigo = 'AB-1';");
        assert_eq!(e.de.schema, "matriz");
        assert_eq!(e.de.tabela, "estoque");
        assert_eq!(e.onde.valor, Literal::Texto("AB-1".into()));
    }

    /// Cada falta recusa pelo NOME, e nao por "sintaxe invalida".
    #[test]
    fn o_que_falta_recusa_pelo_nome() {
        for (sql, pedaco) in [
            ("INSERT INTO t VALUES (1)", "escreva as colunas"),
            ("INSERT INTO t (a) SELECT a FROM u", "INSERT ... SELECT"),
            ("INSERT INTO t (a, b) VALUES (1)", "2 colunas e 1 valores"),
            ("INSERT INTO t (a) VALUES (1), (2)", "uma linha por INSERT"),
            ("INSERT INTO t (a) VALUES (DEFAULT)", "DEFAULT"),
            ("INSERT INTO t (a, a) VALUES (1, 2)", "duas vezes"),
            ("INSERT INTO t (a) VALUES (1 + 2)", "expressao nos valores"),
            ("UPDATE t SET a = 1", "UPDATE sem WHERE"),
            ("UPDATE t SET a = 1 WHERE id > 5", "por faixa"),
            (
                "UPDATE t SET a = 1 WHERE id = 5 AND b = 2",
                "UMA comparacao",
            ),
            ("UPDATE t SET a = b WHERE id = 5", "esperava um valor"),
            // Coluna como valor cai no `literal` antes de o `+` aparecer, e a
            // recusa e a dele; o `+` depois de um literal cai na guarda do SET.
            ("UPDATE t SET a = a + 1 WHERE id = 5", "nao tem quem avalie"),
            ("UPDATE t SET a = 1 + 1 WHERE id = 5", "expressao no SET"),
            (
                "UPDATE t SET softdeleted = FALSE WHERE id = 5",
                "coluna de SISTEMA",
            ),
            ("UPDATE t SET rownum = 1 WHERE id = 5", "coluna de SISTEMA"),
            (
                "UPDATE t SET a = 1, a = 2 WHERE id = 5",
                "duas vezes no SET",
            ),
            ("UPDATE t a = 1 WHERE id = 5", "esperava SET"),
            ("DELETE FROM t", "DELETE sem WHERE"),
            ("DELETE FROM t WHERE id <> 5", "por faixa"),
            ("DELETE t WHERE id = 5", "esperava FROM"),
        ] {
            let e = analisar_comando(sql).unwrap_err().to_string();
            assert!(e.contains(pedaco), "{sql} -> {e}");
        }
    }

    /// `analisar` continua sendo a porta do SELECT: escrita por ela recusa
    /// dizendo que e escrita -- e nao mais "ainda nao existe", que virou
    /// mentira no dia em que este modulo entrou.
    #[test]
    fn a_porta_do_select_nomeia_a_escrita() {
        for sql in [
            "INSERT INTO t (a) VALUES (1)",
            "UPDATE t SET a = 1 WHERE id = 1",
            "DELETE FROM t WHERE id = 1",
        ] {
            let e = analisar(sql).unwrap_err().to_string();
            assert!(e.contains("ESCRITA"), "{sql} -> {e}");
            assert!(!e.contains("ainda nao existe"), "{sql} -> {e}");
        }
    }

    #[test]
    fn set_e_values_sao_clausulas_e_nao_apelidos() {
        // Sem SET em CLAUSULAS, `UPDATE t SET` leria SET como apelido da
        // tabela e o comando quebraria em "esperava SET" dois simbolos depois.
        let a = atualizacao("UPDATE t SET a = 1 WHERE id = 1");
        assert_eq!(a.em.apelido, None);
        // Citada, a palavra volta a ser nome.
        let a = atualizacao("UPDATE t SET \"set\" = 1 WHERE id = 1");
        assert_eq!(a.atribuicoes[0].0, "set");
    }

    // ---------------------------------------------------------- traducao

    #[test]
    fn insert_vira_inserir_com_numero_em_texto() {
        let i = insercao("INSERT INTO clientes (id, preco) VALUES (7, 10.50)");
        let p = traduzir_insercao(&i, &[], "loja").unwrap();
        let PlanoDml::Inserir { pedido, .. } = &p else {
            panic!("{p:?}");
        };
        assert_eq!(p.op(), "inserir");
        assert_eq!(pedido.texto_ou("op", ""), "inserir");
        assert_eq!(pedido.texto_ou("database", ""), "loja");
        assert_eq!(pedido.texto_ou("tabela", ""), "clientes");
        let v = pedido.campo("valores").unwrap();
        // O numero viaja como TEXTO, pelo mesmo motivo do SELECT: o motor le
        // decimal de texto para nao passar por f64.
        assert_eq!(v.texto_ou("id", ""), "7");
        assert_eq!(v.texto_ou("preco", ""), "10.50");
    }

    #[test]
    fn o_banco_do_comando_vence_o_corrente_e_sem_nenhum_recusa() {
        let i = insercao("INSERT INTO outro.matriz.clientes (id) VALUES (1)");
        let p = traduzir_insercao(&i, &[], "loja").unwrap();
        let PlanoDml::Inserir { pedido, .. } = p else {
            unreachable!()
        };
        assert_eq!(pedido.texto_ou("database", ""), "outro");
        assert_eq!(pedido.texto_ou("tabela", ""), "matriz.clientes");

        let i = insercao("INSERT INTO clientes (id) VALUES (1)");
        let e = traduzir_insercao(&i, &[], "").unwrap_err().to_string();
        assert!(e.contains("nao sei em qual database"), "{e}");
    }

    #[test]
    fn update_escolhe_o_indice_unico_e_monta_o_buscar() {
        let a = atualizacao("UPDATE clientes SET nome = 'Bia' WHERE id = 7");
        let indices = [
            ix("porNome", "nome", false, false),
            ix("porId", "id", true, true),
        ];
        let p = traduzir_atualizacao(&a, &indices, "loja").unwrap();
        let PlanoDml::Atualizar {
            busca,
            atribuicoes,
            notas,
        } = &p
        else {
            panic!("{p:?}");
        };
        assert_eq!(p.op(), "atualizar");
        assert_eq!(busca.texto_ou("op", ""), "buscar");
        assert_eq!(busca.texto_ou("indice", ""), "porId");
        assert_eq!(busca.campo("chave").unwrap().escrever(), r#"["7"]"#);
        assert_eq!(busca.inteiro_ou("max", 0), 2);
        assert_eq!(
            atribuicoes,
            &vec![("nome".to_string(), Json::texto_de("Bia"))]
        );
        assert!(notas.iter().any(|n| n.contains("TRES passos")), "{notas:?}");
        assert!(notas.iter().any(|n| n.contains("versao")), "{notas:?}");
    }

    #[test]
    fn delete_monta_o_mesmo_buscar() {
        let e = exclusao("DELETE FROM clientes WHERE id = 7");
        let p = traduzir_exclusao(&e, &[ix("porId", "id", true, true)], "loja").unwrap();
        let PlanoDml::Excluir { busca, notas } = &p else {
            panic!("{p:?}");
        };
        assert_eq!(p.op(), "excluir");
        assert_eq!(busca.texto_ou("indice", ""), "porId");
        assert!(notas.iter().any(|n| n.contains("SUAVE")), "{notas:?}");
    }

    /// Indice comum acha a linha -- e por isso mesmo e recusado: alcancaria
    /// varias, e o SQL nao diz quantas.
    #[test]
    fn indice_que_nao_e_unico_recusa_pelo_nome() {
        let a = atualizacao("UPDATE clientes SET nome = 'Bia' WHERE cidade = 'X'");
        let e = traduzir_atualizacao(&a, &[ix("porCidade", "cidade", false, false)], "loja")
            .unwrap_err()
            .to_string();
        assert!(e.contains("NAO e unico"), "{e}");
        assert!(e.contains("porCidade"), "{e}");
    }

    #[test]
    fn sem_indice_nenhum_recusa_listando_os_unicos() {
        let e = exclusao("DELETE FROM clientes WHERE cidade = 'X'");
        let erro = traduzir_exclusao(
            &e,
            &[
                ix("porId", "id", true, true),
                ix("porCpf", "cpf", true, false),
            ],
            "loja",
        )
        .unwrap_err()
        .to_string();
        assert!(erro.contains("exige um indice UNICO"), "{erro}");
        assert!(erro.contains("id, cpf"), "{erro}");
    }

    /// Chave composta nao serve: mandar so a primeira parte nao e a mesma
    /// busca -- a mesma regra do SELECT.
    #[test]
    fn chave_composta_nao_atende() {
        let a = atualizacao("UPDATE t SET a = 1 WHERE id = 1");
        let composto = IndiceInfo {
            nome: "porIdEAno".into(),
            colunas: vec![
                ColunaDoIndice {
                    nome: "id".into(),
                    desc: false,
                },
                ColunaDoIndice {
                    nome: "ano".into(),
                    desc: false,
                },
            ],
            unico: true,
            primario: true,
        };
        let e = traduzir_atualizacao(&a, &[composto], "loja")
            .unwrap_err()
            .to_string();
        assert!(e.contains("Nao existe"), "{e}");
    }

    // ------------------------------------------------- item 7: upsert

    fn ix_unico(nome: &str, coluna: &str) -> IndiceInfo {
        IndiceInfo {
            nome: nome.into(),
            colunas: vec![ColunaDoIndice {
                nome: coluna.into(),
                desc: false,
            }],
            unico: true,
            primario: false,
        }
    }

    #[test]
    fn on_conflict_do_nothing_vira_ignorar() {
        let i = insercao(
            "INSERT INTO clientes (cpf, nome) VALUES ('1', 'A') ON CONFLICT (cpf) DO NOTHING",
        );
        assert_eq!(
            i.se_existir,
            Some(SeExistir::Ignorar {
                coluna_conflito: Some("cpf".into())
            })
        );
        let p = traduzir_insercao(&i, &[ix_unico("porCpf", "cpf")], "loja").unwrap();
        let PlanoDml::Inserir { pedido, notas } = &p else {
            panic!("{p:?}")
        };
        assert_eq!(pedido.texto_ou("se_existir", ""), "ignorar");
        assert_eq!(pedido.texto_ou("indice", ""), "porCpf");
        assert!(notas.iter().any(|n| n.contains("ignorar")), "{notas:?}");
    }

    #[test]
    fn on_conflict_do_update_vira_atualizar_com_o_set_no_campo_atualizar() {
        let i = insercao(
            "INSERT INTO clientes (cpf, nome, saldo) VALUES ('1', 'A', 10) \
             ON CONFLICT (cpf) DO UPDATE SET nome = 'B', saldo = 20",
        );
        let PlanoDml::Inserir { pedido, .. } =
            traduzir_insercao(&i, &[ix_unico("porCpf", "cpf")], "loja").unwrap()
        else {
            panic!()
        };
        assert_eq!(pedido.texto_ou("se_existir", ""), "atualizar");
        assert_eq!(pedido.texto_ou("indice", ""), "porCpf");
        let at = pedido.campo("atualizar").unwrap();
        assert_eq!(at.texto_ou("nome", ""), "B");
        assert_eq!(at.texto_ou("saldo", ""), "20");
    }

    /// **O literal negativo grava.** `SET a = -5` e `VALUES (40, -3)` caiam
    /// com «esperava um valor e veio "-"» enquanto `WHERE a = -5` passava
    /// pela expressao -- o mesmo numero aceito num lado do comando e recusado
    /// no outro. O lexico entrega `-5` como dois simbolos, e o `literal` e
    /// quem os junta. O sinal so vale colado num numero: `- 'x'` continua
    /// recusando pela frase de sempre.
    #[test]
    fn o_literal_negativo_vale_no_set_e_no_values() {
        let a = atualizacao("UPDATE t SET a = -5 WHERE id = 32");
        assert_eq!(a.atribuicoes[0].1, Literal::Numero("-5".into()));
        let i = insercao("INSERT INTO t (id, a, w) VALUES (40, -3, 1)");
        assert_eq!(i.valores[1], Literal::Numero("-3".into()));
        // O WHERE por chave tambem, que e o mesmo `literal`.
        let a = atualizacao("UPDATE t SET a = 1 WHERE id = -1");
        assert_eq!(a.onde.valor, Literal::Numero("-1".into()));
        // E a expressao continua recusada: `-5` e literal, `- 5 + 1` nao.
        let e = analisar_comando("UPDATE t SET a = -5 + 1 WHERE id = 1").unwrap_err();
        assert!(e.to_string().contains("nao tem quem avalie"), "{e}");
        let e = analisar_comando("UPDATE t SET a = - 'x' WHERE id = 1").unwrap_err();
        assert!(e.to_string().contains("esperava um valor"), "{e}");
    }

    #[test]
    fn on_duplicate_key_update_vira_atualizar_sem_indice() {
        let i = insercao(
            "INSERT INTO clientes (cpf, nome) VALUES ('1', 'A') ON DUPLICATE KEY UPDATE \
             nome = 'B'",
        );
        assert_eq!(
            i.se_existir,
            Some(SeExistir::Atualizar {
                coluna_conflito: None,
                atribuicoes: vec![("nome".to_string(), Literal::Texto("B".into()))],
            })
        );
        // Sem indices NENHUM -- e nao recusa, porque o MySQL nunca nomeia
        // indice: o servidor escolhe o primario.
        let PlanoDml::Inserir { pedido, .. } = traduzir_insercao(&i, &[], "loja").unwrap() else {
            panic!()
        };
        assert_eq!(pedido.texto_ou("se_existir", ""), "atualizar");
        assert!(pedido.campo("indice").is_none(), "MySQL nao nomeia indice");
        assert_eq!(pedido.campo("atualizar").unwrap().texto_ou("nome", ""), "B");
    }

    #[test]
    fn on_conflict_do_nothing_sem_alvo_nao_pede_indice() {
        let i = insercao("INSERT INTO clientes (cpf) VALUES ('1') ON CONFLICT DO NOTHING");
        assert_eq!(
            i.se_existir,
            Some(SeExistir::Ignorar {
                coluna_conflito: None
            })
        );
        let PlanoDml::Inserir { pedido, .. } = traduzir_insercao(&i, &[], "loja").unwrap() else {
            panic!()
        };
        assert!(pedido.campo("indice").is_none());
    }

    #[test]
    fn on_conflict_sem_indice_unico_recusa_nomeando() {
        let i = insercao("INSERT INTO clientes (cpf) VALUES ('1') ON CONFLICT (cpf) DO NOTHING");
        let e = traduzir_insercao(&i, &[], "loja").unwrap_err().to_string();
        assert!(e.contains("ON CONFLICT (cpf)"), "{e}");
        assert!(e.contains("Nao existe"), "{e}");
    }

    #[test]
    fn on_conflict_com_indice_ambiguo_recusa_nomeando_os_candidatos() {
        let i = insercao("INSERT INTO clientes (cpf) VALUES ('1') ON CONFLICT (cpf) DO NOTHING");
        let e = traduzir_insercao(
            &i,
            &[ix_unico("porCpfA", "cpf"), ix_unico("porCpfB", "cpf")],
            "loja",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("ambiguo"), "{e}");
        assert!(e.contains("porCpfA"), "{e}");
        assert!(e.contains("porCpfB"), "{e}");
    }

    /// `excluded.coluna` e expressao no SET recusam pelo NOME, nao pela
    /// mensagem generica de "esperava um valor".
    #[test]
    fn upsert_o_que_falta_recusa_pelo_nome() {
        for (sql, pedaco) in [
            (
                "INSERT INTO t (a) VALUES (1) ON CONFLICT (a) DO UPDATE SET a = excluded.a",
                "excluded.coluna",
            ),
            (
                "INSERT INTO t (a, b) VALUES (1, 2) ON CONFLICT (a) DO UPDATE SET b = 1 + 1",
                "no SET do ON CONFLICT",
            ),
            (
                "INSERT INTO t (a) VALUES (1) ON CONFLICT (a, b) DO NOTHING",
                "mais de uma coluna",
            ),
            (
                "INSERT INTO t (a) VALUES (1) ON DUPLICATE KEY UPDATE softdeleted = FALSE",
                "coluna de SISTEMA",
            ),
            (
                "INSERT INTO t (a) VALUES (1) ON CONFLICT (a) DO UPDATE SET a = 1, a = 2",
                "duas vezes",
            ),
        ] {
            let e = analisar_comando(sql).unwrap_err().to_string();
            assert!(e.contains(pedaco), "{sql} -> {e}");
        }
    }

    #[test]
    fn insert_sem_on_continua_sem_se_existir() {
        let i = insercao("INSERT INTO t (a) VALUES (1)");
        assert_eq!(i.se_existir, None);
    }
}
