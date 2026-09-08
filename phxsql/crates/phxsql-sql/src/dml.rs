//! `INSERT`, `UPDATE` e `DELETE` por chave -- o passo 2 do roteiro de
//! `docs/SQL.md`, o que fecha o CRUD pela camada SQL.
//!
//! # A gramatica, e ela cabe aqui
//!
//! ```text
//! INSERT INTO [database.] [schema.] tabela (coluna {, coluna}) VALUES (literal {, literal})
//! UPDATE      [database.] [schema.] tabela SET coluna = literal {, coluna = literal}
//!             WHERE coluna = literal
//! DELETE FROM [database.] [schema.] tabela WHERE coluna = literal
//! ```
//!
//! Uma linha por `INSERT`, a lista de colunas obrigatoria, o `WHERE` de
//! igualdade sobre uma coluna com indice UNICO -- e o que falta recusa dizendo
//! o que falta, com o nome da clausula, como o `SELECT` ja faz.
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
        Ok(Insercao {
            em,
            colunas,
            valores,
        })
    }

    /// Depois do `UPDATE` ja consumido.
    pub(crate) fn atualizacao(&mut self, _pos: usize) -> Result<Atualizacao> {
        let em = self.alvo()?;
        self.exigir_palavra("SET")?;
        let mut atribuicoes: Vec<(String, Literal)> = Vec::new();
        loop {
            let coluna = self.identificador("nome de coluna no SET")?;
            if atribuicoes.iter().any(|(c, _)| igual_sem_caso(c, &coluna)) {
                return Err(lexico::erro(
                    self.posicao_atual(),
                    &format!("a coluna {coluna:?} aparece duas vezes no SET"),
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
            let valor = self.literal_de_escrita()?;
            self.recusar_expressao("no SET")?;
            atribuicoes.push((coluna, valor));
            if self.aceitar(&Token::Virgula) {
                continue;
            }
            break;
        }
        let onde = self.condicao_de_chave("UPDATE", "mudaria a tabela INTEIRA")?;
        Ok(Atualizacao {
            em,
            atribuicoes,
            onde,
        })
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
pub fn traduzir_insercao(i: &Insercao, database_corrente: &str) -> Result<PlanoDml> {
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
    notas.push(
        "INSERT vira `inserir` -- uma linha, e ela empilha numa transacao aberta como \
         qualquer inserir"
            .into(),
    );
    Ok(PlanoDml::Inserir {
        pedido: pedido_com_op("inserir", pares),
        notas,
    })
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
        let p = traduzir_insercao(&i, "loja").unwrap();
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
        let p = traduzir_insercao(&i, "loja").unwrap();
        let PlanoDml::Inserir { pedido, .. } = p else {
            unreachable!()
        };
        assert_eq!(pedido.texto_ou("database", ""), "outro");
        assert_eq!(pedido.texto_ou("tabela", ""), "matriz.clientes");

        let i = insercao("INSERT INTO clientes (id) VALUES (1)");
        let e = traduzir_insercao(&i, "").unwrap_err().to_string();
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
}
