//! Do `SELECT` lido para o pedido do protocolo.
//!
//! # O trabalho e de traducao, nao de motor
//!
//! Nada se inventa aqui. `docs/SQL.md` mediu que tudo que um `SELECT` precisa
//! ja e uma operacao com teste: `buscar` desce o indice, `varrer` pagina e
//! conta, e a ordem sai do `.ndx` sem ninguem ordenar nada.
//!
//! # E por isso a traducao recusa em vez de fingir
//!
//! O que nao tem operacao embaixo sai como recusa escrita, dizendo o que
//! faltou. Um `WHERE cidade = 'Blumenau'` sem indice em `cidade` nao vira uma
//! varredura filtrada em segredo.
//!
//! # O motivo mudou, e a recusa ficou
//!
//! Ate a 0.18 o motivo era «o `varrer` NAO filtra», e isso deixou de ser
//! verdade: o `varrer` ganhou `"onde"`. O que ele NAO ganhou foi varredura
//! ilimitada -- o `max` continua sendo quantas linhas ele EXAMINA, e a
//! resposta diz `examinadas` ao lado de `devolvidas` justamente para quem
//! chama saber sobre o que a conta foi feita.
//!
//! Um `SELECT` nao tem onde dizer isso. Traduzido para um `varrer` filtrado,
//! ele responderia sobre a PRIMEIRA PAGINA e teria a cara de ter respondido
//! sobre a tabela -- resposta errada com cara de certa, que e pior que erro.
//! Entao a recusa fica, sobre o motivo honesto: falta o indice que torna a
//! pergunta respondivel inteira.

use crate::sintaxe::{Alvo, Condicao, Ordenacao, Projecao, Selecao};
use phxsql_core::json::Json;
use phxsql_core::{PhxError, Result};

/// Uma coluna de um indice, como o esquema a declara.
#[derive(Debug, Clone, PartialEq)]
pub struct ColunaDoIndice {
    pub nome: String,
    pub desc: bool,
}

/// O que o tradutor precisa saber da tabela. Sai do `esquema`, campo por campo
/// -- e nao de uma leitura propria, porque quem abre tabela e o motor.
#[derive(Debug, Clone, PartialEq)]
pub struct IndiceInfo {
    pub nome: String,
    pub colunas: Vec<ColunaDoIndice>,
    pub unico: bool,
    pub primario: bool,
}

impl IndiceInfo {
    /// Um indice serve ao `WHERE col = ?` quando col e a PRIMEIRA coluna dele
    /// e ele nao tem mais nenhuma -- uma chave composta espera todas as
    /// partes, e mandar so a primeira nao e a mesma busca.
    fn atende_igualdade(&self, coluna: &str) -> bool {
        self.colunas.len() == 1 && igual_sem_caso(&self.colunas[0].nome, coluna)
    }

    /// Um indice serve ao `ORDER BY col [DESC]` quando a ordem dele e
    /// exatamente essa. A direcao esta gravada no `.ndx`: pedir ASC num indice
    /// DESC devolveria a lista ao contrario, e nao ha quem inverta depois.
    fn atende_ordem(&self, o: &Ordenacao) -> bool {
        self.colunas.len() == 1
            && igual_sem_caso(&self.colunas[0].nome, &o.coluna)
            && self.colunas[0].desc == o.desc
    }
}

fn igual_sem_caso(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.to_lowercase() == b.to_lowercase()
}

/// O que a resposta do protocolo deve virar antes de chegar a quem perguntou.
#[derive(Debug, Clone, PartialEq)]
pub enum Saida {
    /// Devolve a linha inteira, como o motor mandou.
    LinhaInteira,
    /// Fica so com estas colunas, nesta ordem, com estes rotulos.
    ///
    /// A projecao e do CLIENTE porque o protocolo sempre devolve a linha
    /// inteira -- e isso e uma escolha do motor, nao um esquecimento: o `.reg`
    /// e de slot fixo, e ler meia linha custa a mesma leitura.
    Colunas(Vec<(String, String)>),
    /// So o numero de registros, que sai do cabecalho da tabela.
    Contagem,
}

/// O pedido pronto, mais o que quem chamou ainda tem de fazer com a resposta.
#[derive(Debug, Clone, PartialEq)]
pub struct Plano {
    pub op: String,
    /// O objeto que vai pela porta de dados, sem o `token`.
    pub pedido: Json,
    pub saida: Saida,
    /// O que o tradutor decidiu, em portugues. Nao e enfeite: sem isso, quem
    /// escreveu `ORDER BY nome` e recebeu a ordem de digitacao nao descobre
    /// por que, e culpa o motor.
    pub notas: Vec<String>,
}

impl Plano {
    /// O pedido com o `database` preenchido e o `token` na frente, pronto para
    /// a linha do protocolo.
    pub fn linha(&self, token: &str) -> String {
        let mut pares = vec![("token".to_string(), Json::texto_de(token))];
        if let Json::Objeto(p) = &self.pedido {
            pares.extend(p.clone());
        }
        Json::Objeto(pares).escrever()
    }
}

/// Traduz. `database_corrente` entra quando o `FROM` nao disse o banco.
pub fn traduzir(s: &Selecao, indices: &[IndiceInfo], database_corrente: &str) -> Result<Plano> {
    let mut notas = Vec::new();
    let database = if s.de.database.is_empty() {
        database_corrente.to_string()
    } else {
        s.de.database.clone()
    };
    if database.is_empty() {
        return Err(PhxError::Esquema(
            "nao sei em qual database: escreva FROM banco.tabela ou escolha o banco antes".into(),
        ));
    }
    if let Some(a) = &s.de.apelido {
        notas.push(format!(
            "o apelido {a:?} da tabela foi lido e descartado: so ha uma tabela"
        ));
    }

    let saida = saida_de(&s.projecao);

    match &s.onde {
        // ------------------------------------------------- com filtro: buscar
        Some(c) => plano_buscar(s, c, indices, &database, saida, notas),
        // ------------------------------------------------- sem filtro: varrer
        None => plano_varrer(s, indices, &database, saida, notas),
    }
}

fn saida_de(p: &Projecao) -> Saida {
    match p {
        Projecao::Tudo => Saida::LinhaInteira,
        Projecao::Contagem => Saida::Contagem,
        Projecao::Colunas(cs) => Saida::Colunas(
            cs.iter()
                .map(|c| (c.nome.clone(), c.rotulo().to_string()))
                .collect(),
        ),
    }
}

fn base_do_pedido(de: &Alvo, database: &str) -> Vec<(String, Json)> {
    vec![
        ("database".to_string(), Json::texto_de(database)),
        ("tabela".to_string(), Json::texto_de(de.nome_no_protocolo())),
    ]
}

fn plano_buscar(
    s: &Selecao,
    c: &Condicao,
    indices: &[IndiceInfo],
    database: &str,
    saida: Saida,
    mut notas: Vec<String>,
) -> Result<Plano> {
    use crate::lexico::Comparador;
    if c.op != Comparador::Igual {
        return Err(PhxError::Esquema(format!(
            "WHERE {} {} ... nao tem substrato: o indice desce ate uma chave IGUAL, e a \
             faixa ainda nao esta exposta no protocolo. So `=` passa por aqui",
            c.coluna,
            c.op.simbolo()
        )));
    }
    let Some(ix) = indices.iter().find(|i| i.atende_igualdade(&c.coluna)) else {
        let candidatos: Vec<&str> = indices
            .iter()
            .filter(|i| i.colunas.len() == 1)
            .map(|i| i.colunas[0].nome.as_str())
            .collect();
        return Err(PhxError::Esquema(format!(
            "WHERE {} = ... exige um indice de uma coluna sobre {}. Nao existe. O \
             `varrer` filtra, mas dentro da pagina que ele EXAMINA -- e um SELECT \
             que respondesse sobre a primeira pagina teria a cara de ter \
             respondido sobre a tabela. Ha indice de coluna unica sobre: {}",
            c.coluna,
            c.coluna,
            if candidatos.is_empty() {
                "nenhuma coluna".to_string()
            } else {
                candidatos.join(", ")
            }
        )));
    };
    if let Some(o) = &s.ordem {
        // A ordem de `buscar` e a das linhas daquela chave, e nao a de outra
        // coluna. Aceitar calado devolveria a lista em ordem nenhuma.
        if !igual_sem_caso(&o.coluna, &c.coluna) {
            return Err(PhxError::Esquema(format!(
                "WHERE {} = ... com ORDER BY {} nao tem substrato: `buscar` devolve na \
                 ordem do indice do filtro, e nao ha quem ordene depois",
                c.coluna, o.coluna
            )));
        }
        notas.push(format!(
            "ORDER BY {} ja e a ordem do indice {} usado no filtro",
            o.coluna, ix.nome
        ));
    }
    if s.salto > 0 {
        return Err(PhxError::Esquema(
            "OFFSET com WHERE nao tem substrato: `buscar` nao pula, e pular no cliente \
             seria trazer tudo para jogar fora"
                .into(),
        ));
    }
    if matches!(s.projecao, Projecao::Contagem) {
        notas.push(
            "COUNT(*) com WHERE conta o que a busca devolveu -- o campo `encontrados` da \
             resposta, e nao o `registros` da tabela"
                .into(),
        );
    }

    let mut pares = base_do_pedido(&s.de, database);
    pares.push(("indice".to_string(), Json::texto_de(&ix.nome)));
    pares.push((
        "chave".to_string(),
        Json::Lista(vec![literal_para_json(&c.valor)]),
    ));
    if let Some(l) = s.limite {
        pares.push(("max".to_string(), Json::de_u64(l)));
    }
    notas.push(format!(
        "indice {} escolhido pelo WHERE -- e nao ha planejador: se houvesse dois \
         candidatos, o primeiro declarado venceria",
        ix.nome
    ));
    Ok(Plano {
        op: "buscar".into(),
        pedido: pedido_com_op("buscar", pares),
        saida,
        notas,
    })
}

fn plano_varrer(
    s: &Selecao,
    indices: &[IndiceInfo],
    database: &str,
    saida: Saida,
    mut notas: Vec<String>,
) -> Result<Plano> {
    let mut pares = base_do_pedido(&s.de, database);

    if let Some(o) = &s.ordem {
        let Some(ix) = indices.iter().find(|i| i.atende_ordem(o)) else {
            let mesma_coluna = indices
                .iter()
                .find(|i| i.colunas.len() == 1 && igual_sem_caso(&i.colunas[0].nome, &o.coluna));
            return Err(PhxError::Esquema(match mesma_coluna {
                Some(i) => format!(
                    "ORDER BY {} {} nao tem substrato: o indice {} guarda essa coluna em \
                     {}, e a direcao esta gravada no .ndx -- nao ha quem inverta a lista \
                     depois",
                    o.coluna,
                    if o.desc { "DESC" } else { "ASC" },
                    i.nome,
                    if i.colunas[0].desc { "DESC" } else { "ASC" }
                ),
                None => format!(
                    "ORDER BY {} exige um indice de uma coluna sobre {}. Nao existe, e \
                     nao ha ordenador: a ordem do PhxSql sai do .ndx, sem ordenar nada",
                    o.coluna, o.coluna
                ),
            }));
        };
        pares.push(("indice".to_string(), Json::texto_de(&ix.nome)));
        notas.push(format!(
            "ORDER BY atendido pelo indice {} -- a ordem sai do .ndx, sem ordenar nada",
            ix.nome
        ));
    } else {
        notas.push(
            "sem ORDER BY a ordem e a de DIGITACAO, que no PhxSql e estavel: o .reg nunca \
             reaproveita slot excluido"
                .into(),
        );
    }

    if s.salto > 0 {
        pares.push(("pular".to_string(), Json::de_u64(s.salto)));
    }
    match s.limite {
        Some(l) => pares.push(("max".to_string(), Json::de_u64(l))),
        None if matches!(s.projecao, Projecao::Contagem) => {
            // A contagem sai do cabecalho: `registros` vem na resposta mesmo
            // com uma linha pedida, e trazer a tabela inteira para conta-la
            // seria o erro que a bancada ja cometeu uma vez.
            pares.push(("max".to_string(), Json::de_u64(1)));
            notas.push(
                "COUNT(*) le o campo `registros` da resposta, que sai do cabecalho em O(1) \
                 -- nenhuma linha e varrida para contar"
                    .into(),
            );
        }
        None => notas.push(
            "sem LIMIT o servidor aplica o teto dele (`max_linhas` do config.json), e a \
             resposta diz se ha mais"
                .into(),
        ),
    }

    Ok(Plano {
        op: "varrer".into(),
        pedido: pedido_com_op("varrer", pares),
        saida,
        notas,
    })
}

fn pedido_com_op(op: &str, pares: Vec<(String, Json)>) -> Json {
    let mut todos = vec![("op".to_string(), Json::texto_de(op))];
    todos.extend(pares);
    Json::Objeto(todos)
}

/// O valor como o protocolo o espera.
///
/// Numero vira TEXTO, e nao `Json::Numero`. E deliberado: o motor le decimal
/// de texto justamente para nao passar por `f64`, e transformar aqui desfaria
/// a garantia dentro do tradutor que existe para preserva-la.
fn literal_para_json(l: &crate::sintaxe::Literal) -> Json {
    use crate::sintaxe::Literal;
    match l {
        Literal::Numero(n) => Json::texto_de(n),
        Literal::Texto(t) => Json::texto_de(t),
        Literal::Bool(b) => Json::Bool(*b),
        Literal::Nulo => Json::Nulo,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::sintaxe::analisar;

    fn ix(nome: &str, coluna: &str, desc: bool, unico: bool) -> IndiceInfo {
        IndiceInfo {
            nome: nome.into(),
            colunas: vec![ColunaDoIndice {
                nome: coluna.into(),
                desc,
            }],
            unico,
            primario: unico,
        }
    }

    fn tabela() -> Vec<IndiceInfo> {
        vec![
            ix("porId", "id", false, true),
            ix("porNome", "nome", false, false),
            IndiceInfo {
                nome: "porCidadeLimite".into(),
                colunas: vec![
                    ColunaDoIndice {
                        nome: "cidade".into(),
                        desc: false,
                    },
                    ColunaDoIndice {
                        nome: "limite".into(),
                        desc: true,
                    },
                ],
                unico: false,
                primario: false,
            },
        ]
    }

    fn plano(sql: &str) -> Plano {
        traduzir(&analisar(sql).unwrap(), &tabela(), "Comercial").unwrap()
    }

    fn recusa(sql: &str) -> String {
        traduzir(&analisar(sql).unwrap(), &tabela(), "Comercial")
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn select_estrela_vira_varrer() {
        let p = plano("SELECT * FROM Clientes");
        assert_eq!(p.op, "varrer");
        assert_eq!(p.saida, Saida::LinhaInteira);
        let j = p.pedido;
        assert_eq!(j.texto_ou("op", ""), "varrer");
        assert_eq!(j.texto_ou("database", ""), "Comercial");
        assert_eq!(j.texto_ou("tabela", ""), "Clientes");
    }

    #[test]
    fn schema_chega_no_campo_tabela() {
        let p = plano("SELECT * FROM matriz.estoque");
        assert_eq!(p.pedido.texto_ou("tabela", ""), "matriz.estoque");
        assert_eq!(p.pedido.texto_ou("database", ""), "Comercial");
    }

    #[test]
    fn database_do_from_vence_o_corrente() {
        let p = plano("SELECT * FROM Outro.filial.estoque");
        assert_eq!(p.pedido.texto_ou("database", ""), "Outro");
        assert_eq!(p.pedido.texto_ou("tabela", ""), "filial.estoque");
    }

    #[test]
    fn where_de_igualdade_vira_buscar() {
        let p = plano("SELECT * FROM Clientes WHERE id = 7");
        assert_eq!(p.op, "buscar");
        assert_eq!(p.pedido.texto_ou("indice", ""), "porId");
        let Some(Json::Lista(chave)) = p.pedido.campo("chave") else {
            panic!("esperava lista na chave")
        };
        // Numero vai como TEXTO -- e a garantia do decimal.
        assert_eq!(chave[0], Json::texto_de("7"));
    }

    #[test]
    fn decimal_chega_inteiro_na_chave() {
        let p = traduzir(
            &analisar("SELECT * FROM t WHERE limite = 1500.00").unwrap(),
            &[ix("porLimite", "limite", false, false)],
            "C",
        )
        .unwrap();
        let Some(Json::Lista(chave)) = p.pedido.campo("chave") else {
            panic!()
        };
        assert_eq!(chave[0], Json::texto_de("1500.00"));
    }

    #[test]
    fn order_by_usa_o_indice() {
        let p = plano("SELECT * FROM Clientes ORDER BY nome");
        assert_eq!(p.op, "varrer");
        assert_eq!(p.pedido.texto_ou("indice", ""), "porNome");
    }

    #[test]
    fn limite_e_salto() {
        let p = plano("SELECT * FROM Clientes LIMIT 10 OFFSET 40");
        assert_eq!(p.pedido.inteiro_ou("max", 0), 10);
        assert_eq!(p.pedido.inteiro_ou("pular", 0), 40);
    }

    #[test]
    fn count_pede_uma_linha_so() {
        let p = plano("SELECT COUNT(*) FROM Clientes");
        assert_eq!(p.saida, Saida::Contagem);
        assert_eq!(
            p.pedido.inteiro_ou("max", 0),
            1,
            "contar nao pode varrer a tabela: o numero sai do cabecalho"
        );
    }

    #[test]
    fn projecao_e_do_cliente_e_o_plano_diz_isso() {
        let p = plano("SELECT nome, uf AS estado FROM Clientes");
        assert_eq!(
            p.saida,
            Saida::Colunas(vec![
                ("nome".into(), "nome".into()),
                ("uf".into(), "estado".into())
            ])
        );
    }

    /// O ponto onde a traducao tem de recusar em vez de inventar. Cada um
    /// destes, aceito calado, devolveria resposta errada sem erro nenhum.
    #[test]
    fn recusa_o_que_nao_tem_substrato() {
        // Confere o que a recusa PROMETE -- a coluna pedida e a lista de
        // saidas --, e nao a redacao dela. A versao anterior casava a frase
        // «NAO filtra», que virou mentira no dia em que o `varrer` ganhou
        // `"onde"`: o teste continuou verde defendendo um motivo falso.
        let e = recusa("SELECT * FROM Clientes WHERE cidade = 'Blumenau'");
        assert!(e.contains("cidade"), "{e}");
        assert!(e.contains("indice"), "{e}");
        assert!(e.contains("porNome") || e.contains("nome"), "{e}");

        let e = recusa("SELECT * FROM Clientes WHERE id > 7");
        assert!(e.contains("faixa"), "{e}");

        let e = recusa("SELECT * FROM Clientes ORDER BY limite");
        assert!(e.contains("nao ha ordenador"), "{e}");

        let e = recusa("SELECT * FROM Clientes WHERE id = 1 ORDER BY nome");
        assert!(e.contains("ordem do indice do filtro"), "{e}");

        let e = recusa("SELECT * FROM Clientes WHERE id = 1 LIMIT 5 OFFSET 5");
        assert!(e.contains("OFFSET"), "{e}");
    }

    /// Um indice composto nao atende `WHERE primeira_coluna = ?`: a chave
    /// espera todas as partes.
    #[test]
    fn indice_composto_nao_serve_a_meia_chave() {
        let e = recusa("SELECT * FROM Clientes WHERE cidade = 'Blumenau'");
        assert!(e.contains("indice de uma coluna"), "{e}");
    }

    #[test]
    fn direcao_do_indice_e_a_do_ndx() {
        let e = traduzir(
            &analisar("SELECT * FROM t ORDER BY nome DESC").unwrap(),
            &[ix("porNome", "nome", false, false)],
            "C",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("gravada no .ndx"), "{e}");
        // E no sentido certo ele passa.
        let p = traduzir(
            &analisar("SELECT * FROM t ORDER BY nome DESC").unwrap(),
            &[ix("porNomeDesc", "nome", true, false)],
            "C",
        )
        .unwrap();
        assert_eq!(p.pedido.texto_ou("indice", ""), "porNomeDesc");
    }

    #[test]
    fn sem_banco_nenhum_recusa() {
        let e = traduzir(&analisar("SELECT * FROM t").unwrap(), &tabela(), "")
            .unwrap_err()
            .to_string();
        assert!(e.contains("database"), "{e}");
    }

    #[test]
    fn a_linha_do_protocolo_leva_o_token_na_frente() {
        let l = plano("SELECT * FROM Clientes").linha("demo");
        assert!(l.starts_with("{\"token\":\"demo\""), "{l}");
        assert!(l.contains("\"op\":\"varrer\""), "{l}");
    }

    #[test]
    fn caso_da_coluna_nao_atrapalha() {
        // O motor aceita `Nome` e `nome` como a mesma coluna na tela; o
        // tradutor nao pode ser mais exigente que ele.
        let p = plano("SELECT * FROM Clientes WHERE ID = 3");
        assert_eq!(p.pedido.texto_ou("indice", ""), "porId");
    }
}
