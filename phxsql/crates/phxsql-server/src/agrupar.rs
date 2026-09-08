//! `GROUP BY` generico: agrupa por colunas e resume cada grupo.
//!
//! # Por que ele nasce do `pivot.rs`, e nao ao lado dele
//!
//! O `pivotar` ja resumia -- `Soma, Media, Contagem, Minimo, Maximo,
//! ContagemDistinta`, com acumulador exato para `Decimal` e a media dividindo
//! UMA vez no fim. O que ele tem a mais e o formato linha x coluna; o que ele
//! nao tem e agregar VARIAS colunas de uma vez com apelidos, e filtrar o
//! resultado agregado.
//!
//! Entao o `agrupar` nao traz acumulador proprio: usa o
//! [`crate::pivot::Acumulador`]. Uma segunda soma de `Decimal` divergiria no
//! primeiro arredondamento, e a divergencia apareceria como «o pivot fecha em
//! 1.234,56 e o group by em 1.234,55» -- o pior defeito possivel numa conta de
//! dinheiro, porque os dois numeros parecem certos.
//!
//! # A ordem dos passos, e por que ela e essa
//!
//! `onde`/`expressao` (na linha CRUA) -> agrupa -> `tendo` (na linha
//! AGREGADA) -> `ordem` -> `max`. E a ordem do SQL, e ela nao e arbitraria: o
//! `tendo` fala de `n` e de `total`, que so existem depois da agregacao, e o
//! `onde` fala de colunas da tabela, que deixam de existir individualmente
//! depois dela.
//!
//! # O teto e de GRUPOS, e nao de linhas lidas
//!
//! Uma tabela de dez milhoes de linhas agrupada por `cidade` cabe em duzentos
//! grupos; a mesma tabela agrupada por `id` cabe em dez milhoes, todos em
//! memoria ao mesmo tempo. O que estoura a maquina e o segundo caso, entao e
//! ele que o teto conta -- e a recusa diz o numero e o campo que o levanta,
//! em vez de engasgar a maquina calada.

use std::collections::HashMap;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::Schema;
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;

use crate::pivot::{fechar_valor, Acumulador, Agregador, Iterador};

/// Uma coluna resumida, ja resolvida contra o esquema.
pub struct Agregado {
    pub funcao: Agregador,
    /// `None` so vale para `contagem`, que conta linhas e nao valores.
    pub coluna: Option<usize>,
    pub apelido: String,
}

impl Agregado {
    /// O apelido que o contrato promete quando ninguem escreve um:
    /// `contagem`, `soma_preco`.
    pub fn apelido_padrao(funcao: Agregador, coluna: Option<&str>) -> String {
        match coluna {
            Some(c) => format!("{}_{c}", funcao.nome()),
            None => funcao.nome().to_string(),
        }
    }
}

/// Um grupo fechado: a chave e os valores resumidos, ja tipados.
#[derive(Debug)]
pub struct Grupo {
    /// Os valores das colunas de `por`, na ordem em que foram pedidas.
    pub chave: Vec<Value>,
    /// Um por agregado, na ordem em que foram pedidos.
    pub valores: Vec<(Value, ColumnType)>,
}

#[derive(Debug)]
pub struct Resultado {
    pub grupos: Vec<Grupo>,
    /// Quantas linhas o motor leu para responder -- o irmao do `examinadas`
    /// do `varrer`, e pelo mesmo motivo: sem ele a tela nao sabe se a
    /// contagem que ela mostra e da tabela ou da pagina.
    pub lidas: u64,
}

/// Varre e agrupa.
///
/// `por` vazio e UM grupo so, e nao nenhum: `SELECT COUNT(*) FROM t` tem de
/// responder a contagem da tabela inteira, e nao uma lista vazia. Sobre tabela
/// vazia, porem, o grupo unico nao nasce -- somar nada da resposta nenhuma.
pub fn agrupar(
    fatos: &mut dyn Iterador,
    esquema: &Schema,
    por: &[usize],
    agregados: &[Agregado],
    teto_grupos: u64,
) -> Result<Resultado> {
    let colunas = esquema.colunas();
    // A ordem de chegada e guardada a parte porque o mapa nao a tem, e sem ela
    // dois pedidos iguais devolveriam os grupos em ordens diferentes conforme
    // o `HashMap` resolvesse as colisoes -- uma consulta que muda de ordem
    // conforme a maquina e pior do que uma consulta lenta.
    let mut ordem_de_chegada: Vec<Vec<u8>> = Vec::new();
    let mut caixas: HashMap<Vec<u8>, (Vec<Value>, Vec<Acumulador>)> = HashMap::new();
    let mut lidas = 0u64;

    while let Some(linha) = fatos.proxima()? {
        lidas += 1;
        // `get` e nao indice: linha gravada antes de uma coluna nova nasce
        // curta, e coluna que a linha nao tem e NULA.
        let chave: Vec<Value> = por
            .iter()
            .map(|i| linha.get(*i).cloned().unwrap_or(Value::Null))
            .collect();
        // A mesma nocao de «mesmo valor» do mapa de igualdade do
        // `SelectMemory` (`phxsql_store::memoria::chave`). Uma segunda
        // divergiria, e a divergencia apareceria como um total que nao fecha
        // com a contagem.
        let mut k = Vec::with_capacity(chave.len() * 9);
        for v in &chave {
            k.extend_from_slice(&phxsql_store::memoria::chave(v));
            // Separador: sem ele, `("ab","c")` e `("a","bc")` cairiam no mesmo
            // grupo.
            k.push(0xff);
        }

        let caixa = match caixas.entry(k.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                if ordem_de_chegada.len() as u64 >= teto_grupos {
                    return Err(PhxError::LimiteExcedido(format!(
                        "o agrupamento passou de {teto_grupos} grupos, que e o \
                         teto de `recursos.max_linhas` deste servidor. Os grupos \
                         ficam TODOS em memoria ao mesmo tempo: agrupe por uma \
                         coluna com menos valores distintos, ou filtre antes com \
                         \"onde\"/\"expressao\""
                    )));
                }
                ordem_de_chegada.push(k);
                e.insert((
                    chave,
                    agregados
                        .iter()
                        .map(|a| Acumulador::novo(a.funcao == Agregador::ContagemDistinta))
                        .collect(),
                ))
            }
        };

        for (a, acc) in agregados.iter().zip(caixa.1.iter_mut()) {
            match a.coluna {
                // Contar linhas nao olha valor nenhum -- e por isso
                // `COUNT(*)` conta a linha nula tambem.
                None => acc.somar(&Value::Int(1)),
                Some(c) => {
                    let v = linha.get(c).cloned().unwrap_or(Value::Null);
                    // Nulo NAO entra na conta: somar «sem valor» como zero
                    // afundaria a media e faria o minimo virar zero. E a mesma
                    // decisao do pivot, e a mesma de todo SQL.
                    if !v.e_null() {
                        acc.somar(&v);
                    }
                }
            }
        }
    }

    let mut grupos = Vec::with_capacity(ordem_de_chegada.len());
    for k in ordem_de_chegada {
        let (chave, accs) = caixas.remove(&k).expect("a chave veio da propria lista");
        let valores = agregados
            .iter()
            .zip(accs.iter())
            .map(|(a, acc)| {
                let (decimal, escala) = match a.coluna.and_then(|c| colunas.get(c)) {
                    Some(c) => match c.ty {
                        ColumnType::Decimal { escala, .. } => (true, escala),
                        _ => (false, 0),
                    },
                    None => (false, 0),
                };
                fechar_valor(acc, a.funcao, decimal, escala)
            })
            .collect();
        grupos.push(Grupo { chave, valores });
    }
    Ok(Resultado { grupos, lidas })
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::schema::Column;

    struct Lista(std::vec::IntoIter<Vec<Value>>);
    impl Iterador for Lista {
        fn proxima(&mut self) -> Result<Option<Vec<Value>>> {
            Ok(self.0.next())
        }
    }

    fn esquema() -> Schema {
        Schema::new(
            "vendas",
            vec![
                Column::new("cidade", ColumnType::Str(20)),
                Column::new(
                    "total",
                    ColumnType::Decimal {
                        precisao: 15,
                        escala: 2,
                    },
                ),
            ],
            vec![],
        )
        .unwrap()
    }

    fn linhas() -> Lista {
        Lista(
            vec![
                vec![Value::Str("Blumenau".into()), Value::Decimal(1000)],
                vec![Value::Str("Itajai".into()), Value::Decimal(2050)],
                vec![Value::Str("Blumenau".into()), Value::Decimal(1)],
                vec![Value::Str("Blumenau".into()), Value::Null],
            ]
            .into_iter(),
        )
    }

    /// A soma de `Decimal` e EXATA: 10,00 + 0,01 da 10,01, e nao 10,009999.
    /// Somar dinheiro em `f64` perde centavo, e a regra do projeto e nao
    /// perder -- este teste e o que trava a regra neste caminho novo.
    #[test]
    fn a_soma_de_decimal_nao_perde_centavo() {
        let e = esquema();
        let ags = vec![
            Agregado {
                funcao: Agregador::Contagem,
                coluna: None,
                apelido: "n".into(),
            },
            Agregado {
                funcao: Agregador::Soma,
                coluna: Some(1),
                apelido: "total".into(),
            },
        ];
        let r = agrupar(&mut linhas(), &e, &[0], &ags, 1000).unwrap();
        assert_eq!(r.lidas, 4);
        assert_eq!(r.grupos.len(), 2);
        assert_eq!(r.grupos[0].chave[0], Value::Str("Blumenau".into()));
        // Tres linhas de Blumenau -- a de valor NULO conta como linha.
        assert_eq!(r.grupos[0].valores[0].0, Value::UInt(3));
        assert_eq!(r.grupos[0].valores[1].0, Value::Decimal(1001));
        assert_eq!(r.grupos[1].valores[1].0, Value::Decimal(2050));
    }

    /// `por` vazio e UM grupo, e nao nenhum: e o `SELECT COUNT(*) FROM t`.
    #[test]
    fn sem_por_e_um_grupo_so() {
        let ags = vec![Agregado {
            funcao: Agregador::Contagem,
            coluna: None,
            apelido: "n".into(),
        }];
        let r = agrupar(&mut linhas(), &esquema(), &[], &ags, 1000).unwrap();
        assert_eq!(r.grupos.len(), 1);
        assert_eq!(r.grupos[0].valores[0].0, Value::UInt(4));
        assert!(r.grupos[0].chave.is_empty());
    }

    /// Grupo sem valor nenhum sai NULO, e nao zero.
    ///
    /// «Somar nada» nao da zero reais -- da resposta nenhuma. Devolver zero
    /// faria uma cidade sem venda aparecer como cidade que vendeu zero, que e
    /// uma afirmacao que o dado nao sustenta.
    #[test]
    fn grupo_sem_valor_sai_nulo_e_nao_zero() {
        let mut so_nulo = Lista(vec![vec![Value::Str("Itajai".into()), Value::Null]].into_iter());
        let ags = vec![Agregado {
            funcao: Agregador::Soma,
            coluna: Some(1),
            apelido: "total".into(),
        }];
        let r = agrupar(&mut so_nulo, &esquema(), &[0], &ags, 1000).unwrap();
        assert_eq!(r.grupos[0].valores[0].0, Value::Null);
    }

    /// O teto conta GRUPOS, e a recusa diz o numero e o que fazer.
    #[test]
    fn passar_do_teto_de_grupos_recusa_nomeando() {
        let ags = vec![Agregado {
            funcao: Agregador::Contagem,
            coluna: None,
            apelido: "n".into(),
        }];
        let e = agrupar(&mut linhas(), &esquema(), &[0], &ags, 1)
            .expect_err("dois grupos com teto 1 tinham de recusar");
        assert!(e.to_string().contains('1'), "{e}");
        assert!(e.to_string().contains("max_linhas"), "{e}");
    }

    /// O apelido padrao e o do contrato: `contagem`, `soma_preco`.
    #[test]
    fn o_apelido_padrao_e_o_do_contrato() {
        assert_eq!(
            Agregado::apelido_padrao(Agregador::Contagem, None),
            "contagem"
        );
        assert_eq!(
            Agregado::apelido_padrao(Agregador::Soma, Some("preco")),
            "soma_preco"
        );
    }

    /// Chaves de tipos diferentes nao se misturam, e o NULO tem grupo proprio.
    ///
    /// Sem o separador entre as colunas da chave, `("ab","c")` e `("a","bc")`
    /// cairiam no mesmo grupo -- e o numero errado sairia calado.
    #[test]
    fn a_chave_composta_nao_junta_o_que_e_diferente() {
        let e = Schema::new(
            "t",
            vec![
                Column::new("a", ColumnType::Str(10)),
                Column::new("b", ColumnType::Str(10)),
            ],
            vec![],
        )
        .unwrap();
        let mut l = Lista(
            vec![
                vec![Value::Str("ab".into()), Value::Str("c".into())],
                vec![Value::Str("a".into()), Value::Str("bc".into())],
                vec![Value::Null, Value::Str("x".into())],
            ]
            .into_iter(),
        );
        let ags = vec![Agregado {
            funcao: Agregador::Contagem,
            coluna: None,
            apelido: "n".into(),
        }];
        let r = agrupar(&mut l, &e, &[0, 1], &ags, 1000).unwrap();
        assert_eq!(r.grupos.len(), 3, "duas chaves diferentes viraram uma");
    }
}
