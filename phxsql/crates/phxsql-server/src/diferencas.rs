//! `diferencas`: o que mudou entre duas tabelas, pela chave.
//!
//! # Por que ela nao e um `checksum` melhor
//!
//! O `checksum` responde «estas duas tabelas sao a mesma?» com um numero, e e
//! a resposta certa quando se quer conferir uma replica sem transportar nada.
//! O que ele nao responde e a pergunta seguinte, que e a que interessa quando
//! a resposta e «nao»: **onde**. Uma soma que difere nao diz se faltou uma
//! linha, se sobrou uma, ou se uma coluna de uma linha mudou -- e sem isso a
//! conferencia vira uma varredura manual.
//!
//! # A chave e o eixo, e ela vem de um indice UNICO
//!
//! Comparar linha a linha pela ORDEM nao serve: as duas tabelas podem ter as
//! mesmas linhas com rowids diferentes, e ai tudo apareceria como diferente. E
//! comparar por um indice repetivel nao decide nada -- duas linhas com a mesma
//! chave de um lado nao tem par unico do outro. Entao o indice tem de existir
//! nos DOIS lados, com o mesmo nome, e ser unico -- e a recusa diz qual dos
//! tres requisitos faltou.
//!
//! # As duas tabelas cabem na memoria, ou a comparacao nao acontece
//!
//! Os dois lados entram num mapa. Recusar com o numero na mensagem e melhor do
//! que engasgar a maquina -- e a mesma decisao do `materializar` do `juntar`.

use std::collections::HashMap;

use phxsql_core::schema::Schema;
use phxsql_core::value::Value;

/// Uma linha que existe dos dois lados e nao e igual.
#[derive(Debug)]
pub struct Diferenca {
    pub chave: Vec<Value>,
    /// So as colunas que mudaram, pelo nome -- e nao a linha inteira. Quem
    /// compara duas tabelas de trinta colunas quer saber QUAL mudou.
    pub colunas: Vec<String>,
    pub a: Vec<Value>,
    pub b: Vec<Value>,
}

#[derive(Debug, Default)]
pub struct Resultado {
    pub so_em_a: Vec<Vec<Value>>,
    pub so_em_b: Vec<Vec<Value>>,
    pub diferentes: Vec<Diferenca>,
    pub iguais: u64,
    /// Alguma das tres listas foi cortada por `max`.
    pub truncado: bool,
}

/// A chave de uma linha, em texto canonico, para casar os dois lados.
///
/// Usa a MESMA nocao de «mesmo valor» do mapa de igualdade do `SelectMemory`
/// (`phxsql_store::memoria::chave`): uma segunda faria a comparacao dizer que
/// duas linhas sao diferentes quando o filtro diria que sao a mesma.
fn texto_da_chave(chave: &[Value]) -> Vec<u8> {
    let mut k = Vec::with_capacity(chave.len() * 9);
    for v in chave {
        k.extend_from_slice(&phxsql_store::memoria::chave(v));
        k.push(0xff);
    }
    k
}

/// Compara os dois lados, ja lidos e com a chave extraida.
///
/// `max` corta cada lista SEPARADAMENTE, e nao o total: uma tabela com mil
/// linhas so de um lado esconderia todas as diferentes se o corte fosse
/// comum, e a lista que sumisse seria justamente a que se estava procurando.
/// Zero e sem teto.
pub fn comparar(
    a: Vec<(Vec<Value>, Vec<Value>)>,
    b: Vec<(Vec<Value>, Vec<Value>)>,
    esquema: &Schema,
    max: usize,
) -> Resultado {
    let mut r = Resultado::default();
    let mut de_b: HashMap<Vec<u8>, (Vec<Value>, Vec<Value>)> = HashMap::with_capacity(b.len());
    for (chave, linha) in b {
        de_b.insert(texto_da_chave(&chave), (chave, linha));
    }
    let cabe = |n: usize| max == 0 || n < max;

    for (chave, linha) in a {
        match de_b.remove(&texto_da_chave(&chave)) {
            None => {
                if cabe(r.so_em_a.len()) {
                    r.so_em_a.push(chave);
                } else {
                    r.truncado = true;
                }
            }
            Some((_, outra)) => {
                let colunas: Vec<String> = esquema
                    .colunas()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        // `get` e nao indice: linha gravada antes de uma
                        // coluna nova nasce curta, e curta contra longa e
                        // diferenca -- que e o que ela e.
                        linha.get(*i) != outra.get(*i)
                    })
                    .map(|(_, c)| c.nome.clone())
                    .collect();
                if colunas.is_empty() {
                    r.iguais += 1;
                } else if cabe(r.diferentes.len()) {
                    r.diferentes.push(Diferenca {
                        chave,
                        colunas,
                        a: linha,
                        b: outra,
                    });
                } else {
                    r.truncado = true;
                }
            }
        }
    }
    // O que sobrou no mapa de B nao tem par em A.
    for (_, (chave, _)) in de_b {
        if cabe(r.so_em_b.len()) {
            r.so_em_b.push(chave);
        } else {
            r.truncado = true;
        }
    }
    // O `HashMap` nao tem ordem, e uma resposta que muda de ordem a cada
    // corrida e uma resposta que ninguem consegue comparar com a de ontem.
    r.so_em_b.sort_by_key(|x| texto_da_chave(x));
    r
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    fn esquema() -> Schema {
        Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(20)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria()],
        )
        .unwrap()
    }

    fn lado(linhas: &[(i64, &str)]) -> Vec<(Vec<Value>, Vec<Value>)> {
        linhas
            .iter()
            .map(|(id, nome)| {
                (
                    vec![Value::Int(*id)],
                    vec![Value::Int(*id), Value::Str((*nome).into())],
                )
            })
            .collect()
    }

    /// As tres listas saem separadas, e `colunas` diz QUAL mudou.
    #[test]
    fn as_tres_listas_e_a_coluna_que_mudou() {
        let r = comparar(
            lado(&[(1, "ana"), (2, "bia"), (3, "caio")]),
            lado(&[(1, "ana"), (2, "BIA"), (4, "duda")]),
            &esquema(),
            0,
        );
        assert_eq!(r.iguais, 1);
        assert_eq!(r.so_em_a, vec![vec![Value::Int(3)]]);
        assert_eq!(r.so_em_b, vec![vec![Value::Int(4)]]);
        assert_eq!(r.diferentes.len(), 1);
        assert_eq!(r.diferentes[0].chave, vec![Value::Int(2)]);
        assert_eq!(r.diferentes[0].colunas, vec!["nome".to_string()]);
        assert!(!r.truncado);
    }

    /// **`max` corta cada lista SEPARADAMENTE.**
    ///
    /// Com um corte comum, uma tabela com mil linhas so de um lado esconderia
    /// todas as diferentes -- e a lista que sumisse seria justamente a que se
    /// estava procurando.
    #[test]
    fn o_max_corta_cada_lista_e_a_resposta_diz_que_cortou() {
        let r = comparar(
            lado(&[(1, "x"), (2, "y"), (3, "z")]),
            lado(&[(4, "a"), (5, "b")]),
            &esquema(),
            1,
        );
        assert_eq!(r.so_em_a.len(), 1);
        assert_eq!(r.so_em_b.len(), 1);
        assert!(r.truncado, "cortou e nao disse");
    }

    /// Duas tabelas iguais nao tem diferenca nenhuma -- o controle.
    #[test]
    fn iguais_nao_tem_diferenca() {
        let l = &[(1, "ana"), (2, "bia")];
        let r = comparar(lado(l), lado(l), &esquema(), 0);
        assert_eq!(r.iguais, 2);
        assert!(r.so_em_a.is_empty() && r.so_em_b.is_empty() && r.diferentes.is_empty());
        assert!(!r.truncado);
    }

    /// A ordem de `so_em_b` e estavel: ela sai de um `HashMap`, e resposta que
    /// muda de ordem a cada corrida e resposta que ninguem compara com a de
    /// ontem.
    #[test]
    fn a_ordem_do_so_em_b_e_estavel() {
        let a = lado(&[]);
        let b = lado(&[(9, "i"), (2, "ii"), (5, "iii")]);
        let r = comparar(a, b, &esquema(), 0);
        let ids: Vec<Value> = r.so_em_b.iter().map(|c| c[0].clone()).collect();
        assert_eq!(
            ids,
            vec![Value::Int(2), Value::Int(5), Value::Int(9)],
            "a ordem saiu do acaso do mapa"
        );
    }
}
