//! A composicao: o `consultar` que recebe o resultado de outras operacoes.
//!
//! # O que ele nao e: uma porta dos fundos
//!
//! Toda operacao que le tabela aqui dentro sai pelo `executar_derivado` do
//! servidor -- o MESMO portao de permissao de um pedido que chega pela rede.
//! Este modulo nao abre arquivo, nao conhece `Table` e nao sabe o que e uma
//! tabela: ele recebe LINHAS JA LIDAS, em JSON, e faz sobre elas o que sobra
//! do SQL (o `IN`, a expressao, a janela, a ordem, o recorte e a projecao).
//!
//! A separacao e deliberada e e a garantia: quem quisesse ler uma tabela por
//! aqui teria de acrescentar um caminho de leitura a um modulo que nao tem
//! nenhum, e isso aparece na revisao. Espalhar o portao por dentro da
//! composicao e que seria invisivel.
//!
//! # O tipo depois do JSON, e a perda que ele custa
//!
//! A linha chega como o sub-pedido a devolveu: numero JSON, texto JSON,
//! booleano, nulo. O esquema ficou do outro lado. Entao o resolvedor da
//! expressao decide pelo FORMATO: numero sem parte fracionaria vira inteiro,
//! com parte fracionaria vira real, texto vira texto.
//!
//! O que se perde e a ESCALA de um `Decimal`, que no protocolo inteiro viaja
//! como TEXTO (`"10.01"`) justamente para nao passar por um `f64`. Aqui ele
//! continua texto, e por isso compara com texto -- `preco = '10.01'` funciona,
//! `preco > 10` nao, e a recusa diz isso com todas as letras em vez de
//! responder errado calado. Quem quer comparar `Decimal` com numero poe o
//! filtro na `expressao` do PROPRIO sub-pedido, que le o tipo do esquema e
//! compara no dominio inteiro escalado, sem perder centavo.
//!
//! Nao ha como fazer melhor sem o esquema, e adivinhar seria pior: converter
//! todo texto que PARECE numero mudaria o significado de uma coluna de texto
//! -- `codigo = '10'` deixaria de casar com o codigo `10` gravado como texto.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::expressao::Expressao;
use phxsql_core::json::Json;
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;

/// Uma linha em transito: os pares como o sub-pedido os devolveu.
pub type Linha = Vec<(String, Json)>;

/// O valor de um campo, pelo nome. Sem distinguir caixa, como em todo o resto.
pub fn campo<'a>(linha: &'a Linha, nome: &str) -> Option<&'a Json> {
    linha
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(nome))
        .map(|(_, v)| v)
}

/// Um valor JSON no par (valor, tipo) que o avaliador de expressoes pede.
///
/// A decisao esta no cabecalho do modulo: o formato do JSON e a unica pista
/// que sobrou depois de o esquema ficar do outro lado.
pub fn valor_de_json(j: &Json) -> (Value, ColumnType) {
    match j {
        Json::Nulo => (Value::Null, ColumnType::Int8),
        Json::Bool(b) => (Value::Bool(*b), ColumnType::Bool),
        Json::Numero(n) => {
            // `1.0` e o inteiro 1, e nao o real 1,0: quem escreveu `id = 1` na
            // expressao espera comparar inteiro com inteiro, e o JSON nao
            // distingue os dois.
            if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
                (Value::Int(*n as i64), ColumnType::Int8)
            } else {
                (Value::Real(*n), ColumnType::Real8)
            }
        }
        Json::Texto(t) => (Value::Str(t.clone()), ColumnType::Str(0)),
        // Lista e objeto nao sao valor de coluna: viram NULO, e ai `IS NULL`
        // responde sobre eles em vez de a expressao inteira estourar.
        _ => (Value::Null, ColumnType::Int8),
    }
}

/// Avalia uma expressao sobre uma linha em JSON.
///
/// Materializa os valores convertidos ANTES de avaliar porque o resolvedor
/// devolve REFERENCIAS -- e uma conversao feita dentro do fecho morreria antes
/// de a expressao usa-la. Sao duas alocacoes por linha, e elas so acontecem
/// para quem manda `expressao`: quem nao manda nao chega aqui.
pub fn avaliar_sobre(linha: &Linha, e: &Expressao) -> Result<Option<bool>> {
    let convertidos: Vec<(&str, (Value, ColumnType))> = linha
        .iter()
        .map(|(n, v)| (n.as_str(), valor_de_json(v)))
        .collect();
    e.avaliar_bool(&|nome| {
        convertidos
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
            .map(|(_, (v, ty))| (v, ty))
    })
    .map_err(|erro| enriquecer(erro, e))
}

/// A recusa de tipo, dita no vocabulario de quem escreveu a expressao.
///
/// O avaliador diz «nao da para comparar texto com numero», que esta certo e
/// nao ajuda: quem escreveu `preco > 10` nao sabe que `preco` chegou como
/// texto. **Envolver nao e substituir** -- o erro de dentro continua inteiro
/// no fim da frase, porque ele e quem diz O QUE nao comparou.
fn enriquecer(erro: PhxError, e: &Expressao) -> PhxError {
    if !matches!(erro, PhxError::Tipo(_)) {
        return erro;
    }
    PhxError::Tipo(format!(
        "a expressao {:?} do consultar compara o valor como ele CHEGA no JSON do \
         sub-pedido, e ali uma coluna Decimal e TEXTO (\"10.01\") -- para \
         compara-la com numero, ponha o filtro na \"expressao\" do proprio \
         sub-pedido, que le o tipo do esquema. O erro foi: {erro}",
        e.texto()
    ))
}

/// A chave com que dois valores se comparam numa junção ou num `IN`.
///
/// # A regra, e o que ela custa
///
/// E o TEXTO CANONICO do valor: um numero JSON sem parte fracionaria vira o
/// inteiro (`1.0` -> `"1"`), texto e ele mesmo, booleano e `true`/`false`.
/// Assim `id = 7` casa com `"id": 7` venha ele de onde vier.
///
/// O preco, escrito para nao surpreender: um `Decimal` viaja como `"10.00"` e
/// NAO casa com o inteiro `10`, porque o texto canonico dos dois difere. Isso
/// esta certo -- `Decimal(12,2)` e `Int8` sao colunas diferentes --, e quem
/// precisa juntar uma na outra converte no sub-pedido, onde ha esquema.
///
/// `NULL` devolve `None` e NUNCA casa: e a regra do SQL, e ela vale nos dois
/// lados. Duas linhas sem valor nao sao a mesma linha.
pub fn chave_de_juncao(j: &Json) -> Option<String> {
    Some(match j {
        Json::Nulo => return None,
        Json::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
        Json::Numero(n) if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 => {
            format!("{}", *n as i64)
        }
        Json::Numero(n) => n.to_string(),
        Json::Texto(t) => t.clone(),
        _ => return None,
    })
}

/// Como uma ordem foi pedida: `"id"` ou `{"coluna": "id", "desc": true}`.
pub struct Criterio {
    pub coluna: String,
    pub desc: bool,
}

impl Criterio {
    pub fn da_lista(l: Option<&[Json]>) -> Vec<Criterio> {
        l.unwrap_or(&[])
            .iter()
            .map(|o| match o {
                Json::Texto(t) => Criterio {
                    coluna: t.clone(),
                    desc: false,
                },
                outro => Criterio {
                    coluna: outro.texto_ou("coluna", "").to_string(),
                    desc: outro.booleano_ou("desc", false),
                },
            })
            .filter(|c| !c.coluna.trim().is_empty())
            .collect()
    }
}

/// Compara duas linhas por uma lista de criterios.
///
/// Reusa `phxsql_store::memoria::comparar`, a MESMA ordem do `varrer` e do
/// `SelectMemory` -- uma segunda nocao de «vem antes» faria a mesma consulta
/// sair em ordens diferentes conforme quem a respondeu.
pub fn comparar_por(a: &Linha, b: &Linha, criterios: &[Criterio]) -> std::cmp::Ordering {
    for c in criterios {
        let va = campo(a, &c.coluna).map(valor_de_json);
        let vb = campo(b, &c.coluna).map(valor_de_json);
        let (va, vb) = (
            va.map(|(v, _)| v).unwrap_or(Value::Null),
            vb.map(|(v, _)| v).unwrap_or(Value::Null),
        );
        let ord = phxsql_store::memoria::comparar(&va, &vb);
        let ord = if c.desc { ord.reverse() } else { ord };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

/// `ROW_NUMBER() OVER (PARTITION BY ... ORDER BY ...)`.
///
/// # Por que ela NAO reordena a saida
///
/// A janela e uma COLUNA calculada, e nao uma ordem de resposta: `ORDER BY` e
/// um passo depois dela, e pode ser outro. Numerar reordenando faria
/// `row_number() OVER (ORDER BY id)` mudar a ordem das linhas mesmo quando o
/// pedido nao pediu ordem nenhuma -- e ai a janela deixaria de ser uma coluna
/// para virar um efeito colateral.
///
/// Entao ela numera sobre uma copia dos INDICES e escreve o numero de volta na
/// linha, no lugar dela.
pub fn numerar(
    linhas: &mut [Linha],
    particao: &[String],
    ordem: &[Criterio],
    apelido: &str,
) -> Result<()> {
    if linhas.iter().any(|l| campo(l, apelido).is_some()) {
        return Err(PhxError::Esquema(format!(
            "a janela usa o apelido {apelido:?}, que ja e uma coluna da linha; \
             de outro apelido para as duas nao se sobreporem"
        )));
    }
    let mut idx: Vec<usize> = (0..linhas.len()).collect();
    let chave = |l: &Linha| -> Vec<Option<String>> {
        particao
            .iter()
            .map(|p| campo(l, p).and_then(chave_de_juncao))
            .collect()
    };
    // `sort_by` da std e ESTAVEL: linhas empatadas na ordem da janela ficam na
    // ordem em que o sub-pedido as entregou, que na ordem de digitacao e a
    // ordem do arquivo. Numeracao ao acaso seria pior que numeracao nenhuma.
    idx.sort_by(|a, b| {
        let (la, lb) = (&linhas[*a], &linhas[*b]);
        let ord = chave(la).cmp(&chave(lb));
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        comparar_por(la, lb, ordem)
    });

    let mut anterior: Option<Vec<Option<String>>> = None;
    let mut n = 0u64;
    let mut numeros = vec![0u64; linhas.len()];
    for i in idx {
        let k = chave(&linhas[i]);
        if anterior.as_ref() != Some(&k) {
            n = 0;
            anterior = Some(k);
        }
        n += 1;
        numeros[i] = n;
    }
    for (l, n) in linhas.iter_mut().zip(numeros) {
        l.push((apelido.to_string(), Json::de_u64(n)));
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn linha(pares: &[(&str, Json)]) -> Linha {
        pares
            .iter()
            .map(|(n, v)| (n.to_string(), v.clone()))
            .collect()
    }

    /// O numero JSON sem parte fracionaria e INTEIRO: `id = 1` tem de casar.
    #[test]
    fn o_numero_redondo_vira_inteiro() {
        assert_eq!(valor_de_json(&Json::Numero(1.0)).0, Value::Int(1));
        assert_eq!(valor_de_json(&Json::Numero(1.5)).0, Value::Real(1.5));
        assert_eq!(
            valor_de_json(&Json::texto_de("10.01")).0,
            Value::Str("10.01".into())
        );
    }

    /// A recusa de tipo ENSINA: ela diz que o Decimal chega como texto e diz
    /// onde por o filtro -- e mantem o erro de dentro no fim da frase.
    #[test]
    fn comparar_decimal_com_numero_recusa_ensinando() {
        let l = linha(&[("preco", Json::texto_de("10.01"))]);
        let e = Expressao::analisar("preco > 10").unwrap();
        let erro = avaliar_sobre(&l, &e).expect_err("texto contra numero tinha de recusar");
        let t = erro.to_string();
        assert!(t.contains("Decimal"), "{t}");
        assert!(t.contains("sub-pedido"), "{t}");
        assert!(
            t.contains("nao da para comparar"),
            "o erro de dentro sumiu: {t}"
        );
        // E a comparacao com TEXTO funciona, que e a saida documentada.
        let e = Expressao::analisar("preco = '10.01'").unwrap();
        assert_eq!(avaliar_sobre(&l, &e).unwrap(), Some(true));
    }

    /// `NULL` nunca casa numa junção nem num `IN` -- os dois lados.
    #[test]
    fn o_nulo_nunca_casa() {
        assert_eq!(chave_de_juncao(&Json::Nulo), None);
        assert_eq!(chave_de_juncao(&Json::Numero(7.0)).as_deref(), Some("7"));
        assert_eq!(chave_de_juncao(&Json::texto_de("7")).as_deref(), Some("7"));
        // O preco documentado: `Decimal` nao casa com inteiro.
        assert_ne!(
            chave_de_juncao(&Json::texto_de("7.00")),
            chave_de_juncao(&Json::Numero(7.0))
        );
    }

    /// A janela numera POR PARTICAO e nao reordena a saida.
    #[test]
    fn a_janela_numera_por_particao_sem_reordenar() {
        let mut linhas = vec![
            linha(&[("c", Json::texto_de("B")), ("id", Json::Numero(3.0))]),
            linha(&[("c", Json::texto_de("A")), ("id", Json::Numero(2.0))]),
            linha(&[("c", Json::texto_de("B")), ("id", Json::Numero(1.0))]),
        ];
        numerar(
            &mut linhas,
            &["c".to_string()],
            &[Criterio {
                coluna: "id".into(),
                desc: false,
            }],
            "n",
        )
        .unwrap();
        // A ordem das linhas NAO mudou.
        assert_eq!(campo(&linhas[0], "id"), Some(&Json::Numero(3.0)));
        assert_eq!(campo(&linhas[1], "id"), Some(&Json::Numero(2.0)));
        // Em B, o id 1 e o primeiro e o id 3 e o segundo.
        assert_eq!(campo(&linhas[0], "n"), Some(&Json::Numero(2.0)));
        assert_eq!(campo(&linhas[1], "n"), Some(&Json::Numero(1.0)));
        assert_eq!(campo(&linhas[2], "n"), Some(&Json::Numero(1.0)));
    }

    /// Apelido de janela que ja e coluna recusa: a linha ficaria com dois
    /// campos de mesmo nome, e quem le veria um deles sem saber qual.
    #[test]
    fn apelido_de_janela_repetido_recusa() {
        let mut linhas = vec![linha(&[("n", Json::Numero(1.0))])];
        let e = numerar(&mut linhas, &[], &[], "n").expect_err("apelido repetido passou");
        assert!(e.to_string().contains("apelido"), "{e}");
    }
}
