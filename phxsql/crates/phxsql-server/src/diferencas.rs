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
    ///
    /// **Coluna de SISTEMA nunca aparece aqui**, e o motivo esta no cabecalho
    /// do modulo: a comparacao e pela CHAVE justamente porque as duas tabelas
    /// podem ter as mesmas linhas com rowids diferentes. `rownum`, `rowstamp`
    /// e `rowtime` sao dessa familia -- dizem onde e quando a linha entrou
    /// NAQUELA tabela, nunca o que ela guarda.
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

/// Um lado da comparacao, ja lido.
///
/// # Por que os tipos viajam junto com as linhas
///
/// Porque as duas tabelas podem declarar a MESMA coluna com tipos diferentes
/// -- `valor Decimal(12,2)` de um lado e `Decimal(12,4)` do outro --, e o
/// `Value::Decimal` guarda o inteiro ESCALADO. Sem o tipo de cada lado, a
/// comparacao le o inteiro cru e erra nos dois sentidos, calado: medido em
/// 23/09/2026, 7,25 (escala 2) e 0,0725 (escala 4) guardam os dois o inteiro
/// 725 e sairam como `iguais`, e 10,50 (escala 2) e 10,5000 (escala 4), que
/// sao o MESMO dinheiro, sairam como `so_em_a` e `so_em_b`.
pub struct LadoDaComparacao<'a> {
    /// `(chave, linha)` de cada linha, na ordem em que foram lidas.
    pub linhas: Vec<(Vec<Value>, Vec<Value>)>,
    pub esquema: &'a Schema,
    /// O tipo de cada coluna da CHAVE, na ordem do indice.
    pub tipos_da_chave: Vec<phxsql_core::types::ColumnType>,
}

/// A chave de uma linha, em texto canonico, para casar os dois lados.
///
/// Usa a noção de «mesmo valor» de `juncao::pedaco_de_chave`, que e a UNICA
/// desta casa -- a mesma da junção e a do `UNION` distinto. Ela pede o TIPO
/// porque o valor guardado do `Decimal` nao se le sozinho.
///
/// **Ela usava `phxsql_store::memoria::chave`**, que casa com o mapa de
/// igualdade do `SelectMemory` -- e ali esta certo, porque o filtro compara
/// dentro de UMA tabela, onde a escala e uma so. Aqui os dois lados sao
/// tabelas diferentes, e foi exatamente essa diferenca que o defeito de
/// escala mostrou.
fn texto_da_chave(chave: &[Value], tipos: &[phxsql_core::types::ColumnType]) -> String {
    let mut k = String::with_capacity(chave.len() * 12);
    for (i, v) in chave.iter().enumerate() {
        // O separador e o mesmo do `juncao::chave_de`: um byte que nao aparece
        // em texto de dado, para que ("ab","c") nao colida com ("a","bc").
        k.push('\u{1}');
        match tipos.get(i) {
            Some(t) => k.push_str(&crate::juncao::pedaco_de_chave(v, t)),
            // Chave mais longa que a lista de tipos nao acontece por caminho
            // nenhum de hoje; se acontecer, o valor entra sem tipo em vez de
            // sumir -- chave curta casaria linhas que nao sao a mesma.
            None => k.push_str(&format!("{v:?}")),
        }
    }
    k
}

/// Duas celulas da MESMA coluna, uma de cada lado, sao o mesmo valor?
///
/// `None` e a linha curta -- gravada antes de uma coluna nova --, e curta
/// contra longa e diferenca, que e o que ela e.
fn mesma_celula(
    a: Option<&Value>,
    b: Option<&Value>,
    ta: &phxsql_core::types::ColumnType,
    tb: &phxsql_core::types::ColumnType,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            crate::juncao::pedaco_de_chave(x, ta) == crate::juncao::pedaco_de_chave(y, tb)
        }
        _ => false,
    }
}

/// Compara os dois lados, ja lidos e com a chave extraida.
///
/// `max` corta cada lista SEPARADAMENTE, e nao o total: uma tabela com mil
/// linhas so de um lado esconderia todas as diferentes se o corte fosse
/// comum, e a lista que sumisse seria justamente a que se estava procurando.
/// Zero e sem teto.
pub fn comparar(a: LadoDaComparacao, b: LadoDaComparacao, max: usize) -> Resultado {
    let mut r = Resultado::default();
    let mut de_b: HashMap<String, (Vec<Value>, Vec<Value>)> =
        HashMap::with_capacity(b.linhas.len());
    for (chave, linha) in b.linhas {
        de_b.insert(texto_da_chave(&chave, &b.tipos_da_chave), (chave, linha));
    }
    let cabe = |n: usize| max == 0 || n < max;
    let esquema = a.esquema;
    // O tipo da MESMA posição nos dois esquemas. A operação so chega aqui
    // depois de conferir que as duas tabelas tem as mesmas colunas na mesma
    // ordem (`op_diferencas`), entao a posição basta -- o que ela NAO garante
    // e o tipo igual, que e justamente o que se compara.
    let tipo = |e: &Schema, i: usize| {
        e.colunas()
            .get(i)
            .map_or(phxsql_core::types::ColumnType::Int8, |c| c.ty)
    };

    for (chave, linha) in a.linhas {
        match de_b.remove(&texto_da_chave(&chave, &a.tipos_da_chave)) {
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
                    // A coluna de SISTEMA fica de fora da comparacao, e isto
                    // e o cabecalho do modulo aplicado ate o fim: «comparar
                    // linha a linha pela ORDEM nao serve -- as duas tabelas
                    // podem ter as mesmas linhas com rowids diferentes, e ai
                    // tudo apareceria como diferente».
                    //
                    // Ate o `PSCH` v9 a regra estava escrita e nao aplicada, e
                    // passava despercebida por coincidencia: `softdeleted` e
                    // false nos dois lados (o `varrer` so traz ativa) e o
                    // `rownum` empatava quando as duas tabelas tinham recebido
                    // os mesmos inserts na mesma ordem. O v10 acabou com a
                    // coincidencia -- o `rowstamp` e um contador do NO, entao
                    // `hoje` fica com 1,2,3 e `ontem` com 4,5,6 --, e a
                    // operacao passou a responder «iguais: 0» sobre duas
                    // tabelas com o mesmo dado. Resposta errada numa operacao
                    // que existe para ser acreditada.
                    .filter(|(_, c)| !phxsql_core::schema::e_coluna_de_sistema(&c.nome))
                    .filter(|(i, _)| {
                        // Cada lado se le pelo SEU tipo: as duas tabelas
                        // podem declarar a mesma coluna com escalas
                        // diferentes, e o `Value::Decimal` cru nao diz qual.
                        !mesma_celula(
                            linha.get(*i),
                            outra.get(*i),
                            &tipo(esquema, *i),
                            &tipo(b.esquema, *i),
                        )
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
    //
    // A ORDEM sai de `memoria::chave` e nao da chave canonica, e sao papeis
    // diferentes: a canonica decide IDENTIDADE (e por isso pede o tipo), esta
    // decide POSIÇAO. A canonica e texto, e em texto `"n10"` vem antes de
    // `"n9"` -- ordenar por ela publicaria 10 antes de 9. A do `memoria` e
    // big-endian, entao numero sai em ordem de numero; e como `so_em_b` so
    // tem chave de UM lado, a escala ali e uma so e o inteiro cru ordena
    // certo.
    r.so_em_b.sort_by_cached_key(|x| {
        x.iter()
            .flat_map(phxsql_store::memoria::chave)
            .collect::<Vec<u8>>()
    });
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

    fn linhas(l: &[(i64, &str)]) -> Vec<(Vec<Value>, Vec<Value>)> {
        l.iter()
            .map(|(id, nome)| {
                (
                    vec![Value::Int(*id)],
                    vec![Value::Int(*id), Value::Str((*nome).into())],
                )
            })
            .collect()
    }

    /// Um lado cuja chave e a `id` do esquema de cima.
    fn lado<'a>(e: &'a Schema, l: Vec<(Vec<Value>, Vec<Value>)>) -> LadoDaComparacao<'a> {
        LadoDaComparacao {
            linhas: l,
            esquema: e,
            tipos_da_chave: vec![e.colunas()[0].ty],
        }
    }

    /// As tres listas saem separadas, e `colunas` diz QUAL mudou.
    #[test]
    fn as_tres_listas_e_a_coluna_que_mudou() {
        let e = esquema();
        let r = comparar(
            lado(&e, linhas(&[(1, "ana"), (2, "bia"), (3, "caio")])),
            lado(&e, linhas(&[(1, "ana"), (2, "BIA"), (4, "duda")])),
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
        let e = esquema();
        let r = comparar(
            lado(&e, linhas(&[(1, "x"), (2, "y"), (3, "z")])),
            lado(&e, linhas(&[(4, "a"), (5, "b")])),
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
        let e = esquema();
        let r = comparar(lado(&e, linhas(l)), lado(&e, linhas(l)), 0);
        assert_eq!(r.iguais, 2);
        assert!(r.so_em_a.is_empty() && r.so_em_b.is_empty() && r.diferentes.is_empty());
        assert!(!r.truncado);
    }

    /// **A CAUDA DE SISTEMA NAO E DIFERENCA -- e o controle vem junto.**
    ///
    /// # O defeito que ela repoe
    ///
    /// Tire o `filter` do `e_coluna_de_sistema` do `comparar` e este teste
    /// volta a dizer `iguais: 0` com tres colunas em `diferentes[0].colunas`:
    /// `rownum`, `rowstamp` e `rowtime`. Foi o que o `PSCH` v10 fez a
    /// operacao inteira -- duas tabelas com o MESMO dado respondendo que nada
    /// batia --, e ate o v9 passava despercebido porque o `rownum` empatava
    /// quando as duas tabelas tinham recebido os mesmos inserts na mesma
    /// ordem. O `rowstamp` e um contador do NO: a segunda tabela carregada
    /// nunca repete os numeros da primeira.
    ///
    /// O controle e a segunda metade e nao e enfeite: um `comparar` que
    /// simplesmente parasse de reportar qualquer coluna passaria na primeira
    /// asserçao. A `nome` tem de continuar aparecendo.
    #[test]
    fn a_cauda_de_sistema_nao_e_diferenca_e_a_coluna_do_usuario_continua_sendo() {
        let e = esquema();
        // A linha completa, com a cauda de sistema que o esquema tiver: os
        // valores da cauda saem DIFERENTES dos dois lados de proposito.
        let linha = |nome: &str, semente: u64| -> (Vec<Value>, Vec<Value>) {
            let mut l = vec![Value::Int(1), Value::Str(nome.into())];
            for c in &e.colunas()[2..] {
                assert!(
                    phxsql_core::schema::e_coluna_de_sistema(&c.nome),
                    "a coluna {} nao e de sistema",
                    c.nome
                );
                l.push(match c.ty {
                    ColumnType::DateTime => Value::DateTime(semente as i64 * 1_000),
                    ColumnType::UInt8 => Value::UInt(semente),
                    _ => Value::Bool(false),
                });
            }
            (vec![Value::Int(1)], l)
        };

        let r = comparar(
            lado(&e, vec![linha("ana", 1)]),
            lado(&e, vec![linha("ana", 77)]),
            0,
        );
        assert_eq!(r.iguais, 1, "a cauda de sistema virou diferenca");
        assert!(
            r.diferentes.is_empty(),
            "diferentes: {:?}",
            r.diferentes.iter().map(|d| &d.colunas).collect::<Vec<_>>()
        );

        // O CONTROLE: a coluna do usuario continua sendo diferenca, e sozinha.
        let r = comparar(
            lado(&e, vec![linha("ana", 1)]),
            lado(&e, vec![linha("BIA", 77)]),
            0,
        );
        assert_eq!(r.iguais, 0);
        assert_eq!(r.diferentes.len(), 1);
        assert_eq!(r.diferentes[0].colunas, vec!["nome".to_string()]);
    }

    /// **Escalas diferentes: a comparacao e do DINHEIRO, nao do inteiro
    /// guardado.**
    ///
    /// As duas tabelas podem declarar a mesma coluna com escalas diferentes
    /// -- a operacao confere os NOMES das colunas, nunca os tipos --, e o
    /// `Value::Decimal` guarda o inteiro escalado. Os dois sentidos, medidos
    /// em 23/09/2026 pelo protocolo:
    ///
    /// - 10,50 (escala 2, `1050`) e 10,5000 (escala 4, `105000`) sao o MESMO
    ///   dinheiro e sairam como `diferentes`;
    /// - 7,25 (escala 2) e 0,0725 (escala 4) guardam o MESMO `i128` (725) e
    ///   sairam como **`iguais`**.
    ///
    /// **Prova real:** troque o `mesma_celula` por `linha.get(*i) !=
    /// outra.get(*i)` e as duas asserções invertem.
    #[test]
    fn escalas_diferentes_comparam_pelo_valor() {
        let dec = |escala: u8| ColumnType::Decimal {
            precisao: 12,
            escala,
        };
        let e = |escala: u8| {
            Schema::new(
                "contas",
                vec![
                    Column::new("id", ColumnType::Int8).obrigatoria(),
                    Column::new("valor", dec(escala)),
                ],
                vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
                    .unico()
                    .primaria()],
            )
            .unwrap()
        };
        let (e2, e4) = (e(2), e(4));
        let linha = |id: i64, bruto: i128| {
            (
                vec![Value::Int(id)],
                vec![Value::Int(id), Value::Decimal(bruto)],
            )
        };
        let dois = LadoDaComparacao {
            // 10,50 e 7,25 na escala 2.
            linhas: vec![linha(1, 1050), linha(2, 725)],
            esquema: &e2,
            tipos_da_chave: vec![ColumnType::Int8],
        };
        let quatro = LadoDaComparacao {
            // 10,5000 (o mesmo dinheiro) e 0,0725 (o mesmo inteiro).
            linhas: vec![linha(1, 105_000), linha(2, 725)],
            esquema: &e4,
            tipos_da_chave: vec![ColumnType::Int8],
        };
        let r = comparar(dois, quatro, 0);
        assert_eq!(r.iguais, 1, "o mesmo dinheiro nao contou como igual");
        assert_eq!(r.diferentes.len(), 1, "{:?}", r.diferentes);
        assert_eq!(
            r.diferentes[0].chave,
            vec![Value::Int(2)],
            "a linha diferente e a do 7,25 contra 0,0725"
        );
    }

    /// E a CHAVE tambem: uma chave `Decimal` de escalas diferentes casa pelo
    /// valor, senao as duas linhas apareceriam como `so_em_a` e `so_em_b`.
    #[test]
    fn a_chave_decimal_casa_por_valor() {
        let dec = |escala: u8| ColumnType::Decimal {
            precisao: 12,
            escala,
        };
        let e = |escala: u8| {
            Schema::new(
                "contas",
                vec![Column::new("valor", dec(escala)).obrigatoria()],
                vec![IndexDef::new("porValor", vec![IndexColumn::asc(0)])
                    .unico()
                    .primaria()],
            )
            .unwrap()
        };
        let (e2, e4) = (e(2), e(4));
        let r = comparar(
            LadoDaComparacao {
                linhas: vec![(vec![Value::Decimal(1050)], vec![Value::Decimal(1050)])],
                esquema: &e2,
                tipos_da_chave: vec![dec(2)],
            },
            LadoDaComparacao {
                linhas: vec![(vec![Value::Decimal(105_000)], vec![Value::Decimal(105_000)])],
                esquema: &e4,
                tipos_da_chave: vec![dec(4)],
            },
            0,
        );
        assert_eq!(
            r.iguais, 1,
            "so_em_a: {:?} so_em_b: {:?}",
            r.so_em_a, r.so_em_b
        );
    }

    /// A ordem de `so_em_b` e de NUMERO, e nao de texto.
    ///
    /// A chave canonica da identidade e texto (`"n10"`, `"n9"`), e ordenar por
    /// ela publicaria 10 antes de 9 -- foi o que quase entrou junto com o
    /// conserto da escala. A posição continua saindo de `memoria::chave`, que
    /// e big-endian.
    #[test]
    fn a_ordem_do_so_em_b_e_de_numero() {
        let e = esquema();
        let r = comparar(
            lado(&e, linhas(&[])),
            lado(&e, linhas(&[(10, "x"), (9, "y"), (2, "z")])),
            0,
        );
        let ids: Vec<Value> = r.so_em_b.iter().map(|c| c[0].clone()).collect();
        assert_eq!(
            ids,
            vec![Value::Int(2), Value::Int(9), Value::Int(10)],
            "10 saiu antes do 9: a ordem virou lexicografica"
        );
    }

    /// A ordem de `so_em_b` e estavel: ela sai de um `HashMap`, e resposta que
    /// muda de ordem a cada corrida e resposta que ninguem compara com a de
    /// ontem.
    #[test]
    fn a_ordem_do_so_em_b_e_estavel() {
        let e = esquema();
        let a = lado(&e, linhas(&[]));
        let b = lado(&e, linhas(&[(9, "i"), (2, "ii"), (5, "iii")]));
        let r = comparar(a, b, 0);
        let ids: Vec<Value> = r.so_em_b.iter().map(|c| c[0].clone()).collect();
        assert_eq!(
            ids,
            vec![Value::Int(2), Value::Int(5), Value::Int(9)],
            "a ordem saiu do acaso do mapa"
        );
    }
}
