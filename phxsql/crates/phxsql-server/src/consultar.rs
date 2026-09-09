//! A composicao: o `consultar` que recebe o resultado de outras operacoes.
//!
//! # O que ele nao e: uma porta dos fundos
//!
//! Toda operacao que le tabela aqui dentro sai pelo `executar_derivado` do
//! servidor -- o MESMO portao de permissao de um pedido que chega pela rede.
//! Este modulo nao abre arquivo, nao conhece `Table` e nao sabe o que e uma
//! tabela: ele recebe LINHAS JA LIDAS, em JSON, e faz sobre elas o que sobra
//! do SQL (as juncoes, o `IN`, o `EXISTS`, a expressao, a janela, a ordem, o
//! recorte e a projecao).
//!
//! A separacao e deliberada e e a garantia: quem quisesse ler uma tabela por
//! aqui teria de acrescentar um caminho de leitura a um modulo que nao tem
//! nenhum, e isso aparece na revisao. Espalhar o portao por dentro da
//! composicao e que seria invisivel.
//!
//! # O modelo ao lado da linha, e o que ele comprou
//!
//! Cada lado chega com DUAS coisas: as linhas, em JSON, e o MODELO -- o nome
//! e o tipo de cada coluna, na ordem da linha (`Modelo`). O modelo sai do
//! esquema da tabela (`varrer`/`buscar`), do cabecalho `colunas` que o
//! `agrupar` e o proprio `consultar` passaram a responder, e viaja prefixado
//! junto das linhas a cada junção.
//!
//! Ele fecha os dois defeitos do pedido 237 com uma peca so. Uma direita
//! VAZIA nao tem linha de onde tirar os nomes das colunas, mas tem modelo --
//! entao a linha da esquerda que nao casou sai com TODAS as colunas da direita
//! nulas, e a forma da linha e a mesma em toda linha da resposta. E a celula
//! e convertida PELO TIPO do modelo antes de a expressao ve-la: um `Decimal`,
//! que viaja como texto (`"9.50"`) para nao passar por `f64`, compara como
//! numero. Uma coluna `Str` que contem `"10"` CONTINUA texto -- rotulo se
//! estiliza, dado nunca, e adivinhar numero pelo conteudo seria mentira sobre
//! o dado.
//!
//! O que o modelo nao muda: a chave de junção e de `IN` continua sendo o
//! texto canonico do valor (`chave_de_juncao`), e `"10.00"` continua nao
//! casando com `10` -- sao colunas de tipos diferentes.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::expressao::Expressao;
use phxsql_core::json::Json;
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;

/// Uma linha em transito: os pares como o sub-pedido os devolveu.
pub type Linha = Vec<(String, Json)>;

/// O modelo de um lado: o nome e o tipo de cada coluna, na ordem da linha.
///
/// E o que a linha JSON perdeu ao atravessar o protocolo, e o que a direita
/// vazia precisa para ter nome de coluna. Quem o monta e
/// `linhas_do_sub_pedido`, no servidor; aqui ele so se le e se prefixa.
pub type Modelo = Vec<(String, ColumnType)>;

/// O valor de um campo, pelo nome. Sem distinguir caixa, como em todo o resto.
pub fn campo<'a>(linha: &'a Linha, nome: &str) -> Option<&'a Json> {
    linha
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(nome))
        .map(|(_, v)| v)
}

/// O tipo de uma coluna no modelo, pelo nome REAL dela.
pub fn tipo_no_modelo(modelo: &Modelo, nome: &str) -> Option<ColumnType> {
    modelo
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(nome))
        .map(|(_, t)| *t)
}

/// O modelo como a resposta o escreve: `[{"nome", "tipo"}]`.
///
/// O `tipo` usa a MESMA grafia que a op `esquema` imprime (`Int4`,
/// `Str(20)`, `Decimal { precisao: 10, escala: 2 }`), e e a grafia que
/// `valores::tipo_de_texto` le de volta -- e por essa ida e volta que um
/// `consultar` aninhado recebe o modelo do de dentro sem um segundo formato.
pub fn modelo_para_json(modelo: &Modelo) -> Json {
    Json::Lista(
        modelo
            .iter()
            .map(|(n, t)| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(n)),
                    ("tipo", Json::texto_de(format!("{t:?}"))),
                ])
            })
            .collect(),
    )
}

/// Le o modelo de um cabecalho `colunas` como `modelo_para_json` o escreveu.
///
/// Entrada que nao se le vira modelo VAZIO, e nao erro: modelo vazio quer
/// dizer «nao sei a forma», e o resto do modulo trata isso como tratava antes
/// de o modelo existir -- pela linha.
pub fn modelo_de_json(cabecalho: Option<&Json>) -> Modelo {
    cabecalho
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .filter_map(|c| {
            let nome = c.texto_ou("nome", "").trim().to_string();
            if nome.is_empty() {
                return None;
            }
            let tipo = crate::valores::tipo_de_texto(c.texto_ou("tipo", "")).ok()?;
            Some((nome, tipo))
        })
        .collect()
}

/// Poe o prefixo do apelido em toda coluna da linha -- E do modelo.
///
/// Depois de UMA junção a linha tem colunas de duas tabelas, e as duas tem
/// `id`. Prefixar os DOIS lados -- e nao so o que chegou -- e o que faz o
/// nome qualificado significar sempre a mesma coisa: sem isso, `id` seria a
/// coluna da esquerda antes da junção e um nome ambiguo depois dela, e a
/// mesma consulta mudaria de sentido conforme alguem acrescentasse um `JOIN`.
///
/// Linhas e modelo se prefixam JUNTOS porque sao a mesma coisa vista de dois
/// lados: um prefixado e o outro nao seria um modelo que nomeia colunas que
/// a linha nao tem.
pub fn prefixar(linhas: &mut [Linha], modelo: &mut Modelo, apelido: &str) {
    for l in linhas.iter_mut() {
        for (n, _) in l.iter_mut() {
            *n = format!("{apelido}.{n}");
        }
    }
    for (n, _) in modelo.iter_mut() {
        *n = format!("{apelido}.{n}");
    }
}

/// O nome REAL de uma coluna, a partir do nome como ele foi PEDIDO.
///
/// Duas regras, nesta ordem: o nome bate inteiro (`p.id` acha `p.id`), ou --
/// so quando o pedido nao traz ponto -- ele bate com o sufixo de um nome
/// qualificado (`nome` acha `c.nome`). A segunda so vale quando ha UM
/// candidato: `id` depois de juntar duas tabelas e ambiguo, e escolher um dos
/// dois seria responder sobre a coluna errada calado.
///
/// A resolucao acontece contra o MODELO, uma vez por pedido e nao por linha:
/// as linhas de um mesmo resultado tem todas a mesma forma, porque quem as
/// monta e o mesmo laco. E por ser contra o modelo, ela funciona tambem
/// sobre resultado VAZIO -- que e onde a versao anterior, que lia a primeira
/// linha, nao tinha contra o que resolver.
pub fn resolver(modelo: &Modelo, nome: &str) -> Result<Option<String>> {
    let nome = nome.trim();
    if let Some((n, _)) = modelo.iter().find(|(n, _)| n.eq_ignore_ascii_case(nome)) {
        return Ok(Some(n.clone()));
    }
    if nome.contains('.') {
        return Ok(None);
    }
    let candidatos: Vec<&String> = modelo
        .iter()
        .map(|(n, _)| n)
        .filter(|n| {
            n.split_once('.')
                .is_some_and(|(_, sufixo)| sufixo.eq_ignore_ascii_case(nome))
        })
        .collect();
    match candidatos.len() {
        0 => Ok(None),
        1 => Ok(Some(candidatos[0].clone())),
        _ => Err(PhxError::Esquema(format!(
            "o nome {nome:?} e ambiguo depois da junção: pode ser {}. \
             Escreva o nome com o apelido do lado",
            candidatos
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(" ou ")
        ))),
    }
}

/// Um nome pedido, ja ligado ao nome que a linha tem -- e ao tipo dele.
pub struct Ligacao {
    /// Como quem escreveu a consulta o pediu (`total`, ou `p.total`).
    pub pedido: String,
    /// Como a linha o chama depois das junções (`p.total`).
    pub real: String,
    /// O tipo pelo qual a celula se converte antes de a expressao ve-la.
    pub tipo: ColumnType,
}

/// Liga uma lista de nomes pedidos aos nomes reais, ou recusa nomeando.
///
/// Nome que nao existe NAO e erro aqui: ha quem possa faltar de propósito --
/// a coluna de um lado esquerdo que voltou vazio. Quem precisa que ele exista
/// confere depois, e diz o que ha.
pub fn ligar(modelo: &Modelo, pedidos: &[String]) -> Result<Vec<Ligacao>> {
    let mut saida = Vec::with_capacity(pedidos.len());
    for pedido in pedidos {
        if let Some(real) = resolver(modelo, pedido)? {
            let tipo = tipo_no_modelo(modelo, &real).unwrap_or(ColumnType::Str(0));
            saida.push(Ligacao {
                pedido: pedido.clone(),
                real,
                tipo,
            });
        }
    }
    Ok(saida)
}

/// Um valor JSON no par (valor, tipo), decidido pelo FORMATO do JSON.
///
/// E o que sobra quando o modelo nao conhece a coluna: numero sem parte
/// fracionaria vira inteiro, com parte fracionaria vira real, texto vira
/// texto. Continua existindo por dois motivos: e o tipo de uma coluna que
/// nenhum modelo nomeou, e e o recuo de `valor_tipado` quando a ida e volta
/// pelo tipo nao fecha.
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

/// Um valor JSON no par (valor, tipo), decidido pelo TIPO do modelo.
///
/// Passa pelo `valores::json_para_valor` -- o MESMO conversor do `inserir`,
/// e nao um segundo: e ele quem sabe que um `Decimal` chega como texto e
/// vira inteiro escalado sem perder centavo, e que uma coluna `Str` que
/// contem `"10"` e texto e continua texto.
///
/// # O recuo, e por que ele e o certo
///
/// `Time` e `DateTime` saem para o JSON como texto ISO (`valor_para_json`) e
/// o conversor de entrada os espera como inteiro: a ida e volta nao fecha para
/// esses dois, e fechar aqui seria um segundo conversor. Quando a conversao
/// pelo tipo falha, o valor segue pelo FORMATO -- exatamente o que este modulo
/// fazia antes de o modelo existir. Para hora e instante isso e o texto ISO,
/// que e como o avaliador de expressoes ja os compara (`Valor::de_value`).
/// Recusar aqui trocaria uma comparacao que funciona por um erro novo.
pub fn valor_tipado(j: &Json, tipo: &ColumnType) -> (Value, ColumnType) {
    if j.e_nulo() {
        return (Value::Null, *tipo);
    }
    match crate::valores::json_para_valor(j, tipo) {
        Ok(v) => (v, *tipo),
        Err(_) => valor_de_json(j),
    }
}

/// Avalia uma expressao sobre uma linha em JSON.
///
/// Materializa os valores convertidos ANTES de avaliar porque o resolvedor
/// devolve REFERENCIAS -- e uma conversao feita dentro do fecho morreria antes
/// de a expressao usa-la. Sao duas alocacoes por linha, e elas so acontecem
/// para quem manda `expressao`: quem nao manda nao chega aqui.
pub fn avaliar_sobre(linha: &Linha, e: &Expressao, ligacoes: &[Ligacao]) -> Result<Option<bool>> {
    // So as colunas que a expressao usa sao convertidas -- e nao a linha
    // inteira. Numa linha de quarenta colunas com um filtro sobre duas, isso e
    // a diferenca entre duas conversoes e quarenta, por linha.
    let convertidos: Vec<(Value, ColumnType)> = ligacoes
        .iter()
        .map(|l| campo(linha, &l.real).map(|v| valor_tipado(v, &l.tipo)))
        // Coluna que a linha nao tem e NULA, e nao um estouro: e o caso do
        // lado esquerdo que nao casou.
        .map(|x| x.unwrap_or((Value::Null, ColumnType::Int8)))
        .collect();
    e.avaliar_bool(&|nome| {
        ligacoes
            .iter()
            .position(|l| l.pedido.eq_ignore_ascii_case(nome))
            .map(|i| (&convertidos[i].0, &convertidos[i].1))
    })
    .map_err(|erro| enriquecer(erro, e))
}

/// A recusa de tipo, dita no vocabulario de quem escreveu a expressao.
///
/// O avaliador diz «nao da para comparar texto com numero», que esta certo e
/// nao ajuda sozinho: quem escreveu `codigo > 9` nao sabe que `codigo` e uma
/// coluna de TEXTO que por acaso guarda digitos. **Envolver nao e
/// substituir** -- o erro de dentro continua inteiro no fim da frase, porque
/// ele e quem diz O QUE nao comparou.
fn enriquecer(erro: PhxError, e: &Expressao) -> PhxError {
    if !matches!(erro, PhxError::Tipo(_)) {
        return erro;
    }
    PhxError::Tipo(format!(
        "a expressao {:?} do consultar converte cada coluna pelo TIPO que o \
         sub-pedido declarou, e uma coluna de texto continua texto mesmo quando \
         guarda digitos -- compare-a com texto ('10'). O erro foi: {erro}",
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

/// A chave COMPOSTA de uma linha, pelos pares de uma junção.
///
/// `lado` escolhe qual metade de cada par se le nesta linha. `None` quando
/// qualquer coluna e nula, porque nulo nunca casa.
fn chave_composta(
    l: &Linha,
    pares: &[(String, String)],
    lado: fn(&(String, String)) -> &String,
) -> Option<String> {
    let mut k = String::new();
    for par in pares {
        let v = campo(l, lado(par)).and_then(chave_de_juncao)?;
        k.push_str(&v);
        // Separador, pelo mesmo motivo do `agrupar`: sem ele `("ab","c")` e
        // `("a","bc")` casariam.
        k.push('\u{1}');
    }
    Some(k)
}

/// Como uma ordem foi pedida: `"id"` ou `{"coluna": "id", "desc": true}`.
pub struct Criterio {
    pub coluna: String,
    pub desc: bool,
    /// O tipo da coluna no modelo, quando ele a conhece: e por ele que
    /// `"9.50"` vem antes de `"10.00"` num `Decimal`, em vez de depois.
    pub tipo: Option<ColumnType>,
}

impl Criterio {
    pub fn da_lista(l: Option<&[Json]>) -> Vec<Criterio> {
        l.unwrap_or(&[])
            .iter()
            .map(|o| match o {
                Json::Texto(t) => Criterio {
                    coluna: t.clone(),
                    desc: false,
                    tipo: None,
                },
                outro => Criterio {
                    coluna: outro.texto_ou("coluna", "").to_string(),
                    desc: outro.booleano_ou("desc", false),
                    tipo: None,
                },
            })
            .filter(|c| !c.coluna.trim().is_empty())
            .collect()
    }
}

/// Da a cada criterio o tipo que o modelo conhece para a coluna REAL dele.
pub fn tipar(criterios: &mut [Criterio], modelo: &Modelo) {
    for c in criterios.iter_mut() {
        c.tipo = tipo_no_modelo(modelo, &c.coluna);
    }
}

/// Compara duas linhas por uma lista de criterios.
///
/// Reusa `phxsql_store::memoria::comparar`, a MESMA ordem do `varrer` e do
/// `SelectMemory` -- uma segunda nocao de «vem antes» faria a mesma consulta
/// sair em ordens diferentes conforme quem a respondeu.
pub fn comparar_por(a: &Linha, b: &Linha, criterios: &[Criterio]) -> std::cmp::Ordering {
    for c in criterios {
        let converter = |j: &Json| match &c.tipo {
            Some(t) => valor_tipado(j, t).0,
            None => valor_de_json(j).0,
        };
        let va = campo(a, &c.coluna).map(converter).unwrap_or(Value::Null);
        let vb = campo(b, &c.coluna).map(converter).unwrap_or(Value::Null);
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
/// linha, no lugar dela -- e no modelo, como `UInt8`.
pub fn numerar(
    linhas: &mut [Linha],
    modelo: &mut Modelo,
    particao: &[String],
    ordem: &[Criterio],
    apelido: &str,
) -> Result<()> {
    let ja_existe = modelo.iter().any(|(n, _)| n.eq_ignore_ascii_case(apelido))
        || linhas.iter().any(|l| campo(l, apelido).is_some());
    if ja_existe {
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
    modelo.push((apelido.to_string(), ColumnType::UInt8));
    Ok(())
}

/// Como a junção liga os dois lados: pares de igualdade.
pub struct Par {
    pub esquerda: String,
    pub direita: String,
}

/// O que a junção faz com a linha que nao casou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoJuncao {
    /// So quem casa.
    Interno,
    /// A linha da esquerda fica, com as colunas da direita nulas.
    Esquerdo,
    /// A linha da direita fica, com as colunas da esquerda nulas.
    Direito,
    /// As duas: `esquerdo` mais as linhas da direita que nao casaram.
    Completo,
    /// O produto: toda linha da esquerda com toda linha da direita, sem par.
    Cruzado,
}

impl TipoJuncao {
    pub fn de_texto(t: &str) -> Result<TipoJuncao> {
        Ok(match t.trim().to_ascii_lowercase().as_str() {
            "" | "interno" | "inner" => TipoJuncao::Interno,
            "esquerdo" | "left" => TipoJuncao::Esquerdo,
            "direito" | "right" => TipoJuncao::Direito,
            "completo" | "full" => TipoJuncao::Completo,
            "cruzado" | "cross" => TipoJuncao::Cruzado,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "junção desconhecida: {outro:?} (use interno, esquerdo, \
                     direito, completo ou cruzado)"
                )))
            }
        })
    }

    /// O `cruzado` e a unica junção SEM par: ela e o produto. As outras
    /// quatro casam por igualdade, e sem par seriam um produto disfarcado.
    pub fn casa_por_pares(self) -> bool {
        self != TipoJuncao::Cruzado
    }
}

/// Junta dois conjuntos de linhas, em memoria.
///
/// # Por que espalhamento, e nao um laco de dois
///
/// O laco de dois custa esquerda x direita comparacoes; o mapa custa uma
/// passada em cada lado. Para a forma de dado que se junta aqui -- muitos
/// fatos, poucas dimensoes -- e a mesma escolha que o `pivotar` ja fez, e pelo
/// mesmo motivo. So o `cruzado` e o laco de dois, porque o produto E o
/// resultado pedido -- e quem chama confere o teto ANTES de chegar aqui.
///
/// # A forma da linha, e de onde vem o nome da coluna que falta
///
/// A linha sem casamento sai com as colunas do outro lado NULAS, e os NOMES
/// dessas colunas vem do MODELO do outro lado -- nao da primeira linha dele.
/// E o que faz a direita VAZIA continuar tendo colunas: com o modelo, a forma
/// da linha e a mesma em toda linha da resposta, casada ou nao, e quem le por
/// posicao nao quebra na primeira orfa.
///
/// A ordem das colunas e sempre esquerda e depois direita, nos dois sentidos:
/// no `direito`, a linha da direita que nao casou tambem sai com as da
/// esquerda (nulas) NA FRENTE.
pub fn juntar(
    esquerda: Vec<Linha>,
    modelo_esq: &Modelo,
    direita: &[Linha],
    modelo_dir: &Modelo,
    pares: &[(String, String)],
    tipo: TipoJuncao,
) -> Vec<Linha> {
    if tipo == TipoJuncao::Cruzado {
        let mut saida = Vec::with_capacity(esquerda.len().saturating_mul(direita.len()));
        for e in &esquerda {
            for d in direita {
                let mut nova = e.clone();
                nova.extend(d.iter().cloned());
                saida.push(nova);
            }
        }
        return saida;
    }

    let mut mapa: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, d) in direita.iter().enumerate() {
        if let Some(k) = chave_composta(d, pares, |p| &p.1) {
            mapa.entry(k).or_default().push(i);
        }
    }
    let nulos = |modelo: &Modelo| -> Vec<(String, Json)> {
        modelo
            .iter()
            .map(|(n, _)| (n.clone(), Json::Nulo))
            .collect()
    };
    let mut casou_direita = vec![false; direita.len()];
    let mut saida = Vec::with_capacity(esquerda.len());
    for e in esquerda {
        let casadas = chave_composta(&e, pares, |p| &p.0).and_then(|k| mapa.get(&k));
        match casadas {
            Some(is) if !is.is_empty() => {
                for &i in is {
                    casou_direita[i] = true;
                    let mut nova = e.clone();
                    nova.extend(direita[i].iter().cloned());
                    saida.push(nova);
                }
            }
            _ => {
                if matches!(tipo, TipoJuncao::Esquerdo | TipoJuncao::Completo) {
                    let mut nova = e;
                    nova.extend(nulos(modelo_dir));
                    saida.push(nova);
                }
            }
        }
    }
    if matches!(tipo, TipoJuncao::Direito | TipoJuncao::Completo) {
        for (i, d) in direita.iter().enumerate() {
            if !casou_direita[i] {
                let mut nova = nulos(modelo_esq);
                nova.extend(d.iter().cloned());
                saida.push(nova);
            }
        }
    }
    saida
}

/// A semijunção (e a antijunção) por espalhamento: o `EXISTS`.
///
/// Mantem as linhas da esquerda que TEM par na direita -- ou, com `nao`, as
/// que NAO tem. Nao acrescenta coluna nenhuma e nao multiplica linha: uma
/// linha da esquerda com tres pares na direita sai UMA vez, que e a
/// diferenca entre `EXISTS` e `JOIN`.
///
/// # Por que espalhamento, e nao a subconsulta por linha
///
/// `EXISTS` correlacionado e, no caso que todo mundo escreve, uma igualdade
/// entre uma coluna de fora e uma de dentro. Por espalhamento isso custa uma
/// passada em cada lado; rodar a subconsulta por linha custaria N passagens
/// pelo portao de permissao, e cada sub-pedido desta casa e materializado
/// inteiro por ele. E a divergencia desta casa: correlacao SO por igualdade,
/// e o resto recusa nomeando.
///
/// Nulo nunca casa, dos dois lados: a linha de fora com a chave nula NAO
/// existe na direita -- e por isso `nao` a mantem, como no SQL.
pub fn semijuntar(
    esquerda: Vec<Linha>,
    direita: &[Linha],
    pares: &[(String, String)],
    nao: bool,
) -> Vec<Linha> {
    let conjunto: std::collections::HashSet<String> = direita
        .iter()
        .filter_map(|d| chave_composta(d, pares, |p| &p.1))
        .collect();
    esquerda
        .into_iter()
        .filter(|e| {
            let tem = chave_composta(e, pares, |p| &p.0).is_some_and(|k| conjunto.contains(&k));
            tem != nao
        })
        .collect()
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

    fn modelo(pares: &[(&str, ColumnType)]) -> Modelo {
        pares.iter().map(|(n, t)| (n.to_string(), *t)).collect()
    }

    fn decimal2() -> ColumnType {
        ColumnType::Decimal {
            precisao: 10,
            escala: 2,
        }
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

    /// **O tipo do modelo decide, e nao o formato do JSON.** O mesmo texto
    /// `"10"` e `Decimal(1000)` numa coluna Decimal e texto numa coluna Str --
    /// rotulo se estiliza, dado nunca.
    #[test]
    fn a_celula_se_converte_pelo_tipo_do_modelo() {
        let dez = Json::texto_de("10");
        assert_eq!(valor_tipado(&dez, &decimal2()).0, Value::Decimal(1000));
        assert_eq!(
            valor_tipado(&dez, &ColumnType::Str(10)).0,
            Value::Str("10".into())
        );
        assert_eq!(valor_tipado(&dez, &ColumnType::Int4).0, Value::Int(10));
        // Nulo e nulo em qualquer tipo, e carrega o tipo da coluna.
        assert_eq!(
            valor_tipado(&Json::Nulo, &decimal2()),
            (Value::Null, decimal2())
        );
        // A ida e volta que NAO fecha (hora ISO num `Time`) recua para o
        // formato, em vez de recusar: e o texto que o avaliador ja comparava.
        assert_eq!(
            valor_tipado(&Json::texto_de("10:30:00"), &ColumnType::Time).0,
            Value::Str("10:30:00".into())
        );
    }

    /// `preco > media` com `9.50` e `10.00` e FALSO quando os dois sao
    /// Decimal no modelo -- e seria verdadeiro como texto. E o defeito 2 do
    /// pedido 237, na unidade: com `Str` no lugar de `Decimal` este teste cai.
    #[test]
    fn o_decimal_compara_como_numero_pelo_modelo() {
        let l = linha(&[
            ("preco", Json::texto_de("9.50")),
            ("media", Json::texto_de("10.00")),
        ]);
        let m = modelo(&[("preco", decimal2()), ("media", decimal2())]);
        let e = Expressao::analisar("preco > media").unwrap();
        let lig = ligar(&m, e.colunas()).unwrap();
        assert_eq!(avaliar_sobre(&l, &e, &lig).unwrap(), Some(false));
        // E contra literal numerico tambem.
        let e = Expressao::analisar("preco > 10").unwrap();
        let lig = ligar(&m, e.colunas()).unwrap();
        assert_eq!(avaliar_sobre(&l, &e, &lig).unwrap(), Some(false));

        // Modelo dizendo TEXTO: a comparacao textual da verdadeiro -- e o
        // que o defeito fazia, e o que uma coluna Str continua fazendo.
        let m = modelo(&[
            ("preco", ColumnType::Str(10)),
            ("media", ColumnType::Str(10)),
        ]);
        let e = Expressao::analisar("preco > media").unwrap();
        let lig = ligar(&m, e.colunas()).unwrap();
        assert_eq!(avaliar_sobre(&l, &e, &lig).unwrap(), Some(true));
    }

    /// A recusa de tipo ENSINA e mantem o erro de dentro no fim da frase.
    #[test]
    fn texto_contra_numero_recusa_ensinando() {
        let l = linha(&[("codigo", Json::texto_de("10"))]);
        let m = modelo(&[("codigo", ColumnType::Str(10))]);
        let e = Expressao::analisar("codigo > 9").unwrap();
        let lig = ligar(&m, e.colunas()).unwrap();
        let erro = avaliar_sobre(&l, &e, &lig).expect_err("texto contra numero tinha de recusar");
        let t = erro.to_string();
        assert!(t.contains("texto"), "{t}");
        assert!(
            t.contains("nao da para comparar"),
            "o erro de dentro sumiu: {t}"
        );
        let e = Expressao::analisar("codigo = '10'").unwrap();
        let lig = ligar(&m, e.colunas()).unwrap();
        assert_eq!(avaliar_sobre(&l, &e, &lig).unwrap(), Some(true));
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

    /// A ordem por `Decimal` e NUMERICA quando o modelo diz Decimal: `9.50`
    /// vem antes de `10.00`. Como texto, `"10.00"` viria antes -- e a mesma
    /// raiz do defeito 2, no `ordem`.
    #[test]
    fn a_ordem_por_decimal_e_numerica() {
        let a = linha(&[("preco", Json::texto_de("9.50"))]);
        let b = linha(&[("preco", Json::texto_de("10.00"))]);
        let mut c = Criterio::da_lista(Some(&[Json::texto_de("preco")]));
        // Sem modelo: texto, e "10.00" < "9.50".
        assert_eq!(comparar_por(&a, &b, &c), std::cmp::Ordering::Greater);
        tipar(&mut c, &modelo(&[("preco", decimal2())]));
        assert_eq!(comparar_por(&a, &b, &c), std::cmp::Ordering::Less);
    }

    /// A janela numera POR PARTICAO, nao reordena a saida, e entra no modelo.
    #[test]
    fn a_janela_numera_por_particao_sem_reordenar() {
        let mut linhas = vec![
            linha(&[("c", Json::texto_de("B")), ("id", Json::Numero(3.0))]),
            linha(&[("c", Json::texto_de("A")), ("id", Json::Numero(2.0))]),
            linha(&[("c", Json::texto_de("B")), ("id", Json::Numero(1.0))]),
        ];
        let mut m = modelo(&[("c", ColumnType::Str(1)), ("id", ColumnType::Int4)]);
        numerar(
            &mut linhas,
            &mut m,
            &["c".to_string()],
            &[Criterio {
                coluna: "id".into(),
                desc: false,
                tipo: Some(ColumnType::Int4),
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
        assert_eq!(tipo_no_modelo(&m, "n"), Some(ColumnType::UInt8));
    }

    /// Apelido de janela que ja e coluna recusa: a linha ficaria com dois
    /// campos de mesmo nome, e quem le veria um deles sem saber qual.
    #[test]
    fn apelido_de_janela_repetido_recusa() {
        let mut linhas = vec![linha(&[("n", Json::Numero(1.0))])];
        let mut m = modelo(&[("n", ColumnType::Int4)]);
        let e = numerar(&mut linhas, &mut m, &[], &[], "n").expect_err("apelido repetido passou");
        assert!(e.to_string().contains("apelido"), "{e}");
    }

    fn lados() -> (Vec<Linha>, Modelo, Vec<Linha>, Modelo) {
        let esq = vec![
            linha(&[("p.id", Json::Numero(1.0)), ("p.c", Json::Numero(1.0))]),
            linha(&[("p.id", Json::Numero(2.0)), ("p.c", Json::Numero(9.0))]),
            linha(&[("p.id", Json::Numero(3.0)), ("p.c", Json::Nulo)]),
        ];
        let me = modelo(&[("p.id", ColumnType::Int4), ("p.c", ColumnType::Int4)]);
        let dir = vec![
            linha(&[
                ("c.id", Json::Numero(1.0)),
                ("c.nome", Json::texto_de("ana")),
            ]),
            linha(&[
                ("c.id", Json::Numero(3.0)),
                ("c.nome", Json::texto_de("caio")),
            ]),
        ];
        let md = modelo(&[("c.id", ColumnType::Int4), ("c.nome", ColumnType::Str(20))]);
        (esq, me, dir, md)
    }

    fn pares() -> Vec<(String, String)> {
        vec![("p.c".to_string(), "c.id".to_string())]
    }

    /// Os cinco tipos, medidos em QUANTAS linhas e em que FORMA saem.
    ///
    /// Esquerda: 1 casa com ana, 2 aponta para 9 (nao existe), 3 tem chave
    /// NULA. Direita: ana casa, caio nao.
    #[test]
    fn os_cinco_tipos_de_juncao_produzem_a_forma_certa() {
        let (esq, me, dir, md) = lados();
        let forma = |l: &Linha| -> Vec<String> { l.iter().map(|(n, _)| n.clone()).collect() };
        let esperada = vec!["p.id", "p.c", "c.id", "c.nome"];

        let r = juntar(esq.clone(), &me, &dir, &md, &pares(), TipoJuncao::Interno);
        assert_eq!(r.len(), 1);
        let r = juntar(esq.clone(), &me, &dir, &md, &pares(), TipoJuncao::Esquerdo);
        assert_eq!(r.len(), 3);
        assert_eq!(campo(&r[1], "c.nome"), Some(&Json::Nulo));
        let r = juntar(esq.clone(), &me, &dir, &md, &pares(), TipoJuncao::Direito);
        assert_eq!(r.len(), 2, "ana casada + caio orfao");
        assert_eq!(campo(&r[1], "c.nome"), Some(&Json::texto_de("caio")));
        assert_eq!(campo(&r[1], "p.id"), Some(&Json::Nulo));
        assert_eq!(forma(&r[1]), esperada, "a orfa da direita mudou a forma");
        let r = juntar(esq.clone(), &me, &dir, &md, &pares(), TipoJuncao::Completo);
        assert_eq!(r.len(), 4, "1 casada + 2 orfas da esquerda + caio");
        for l in &r {
            assert_eq!(forma(l), esperada, "{l:?}");
        }
        let r = juntar(esq, &me, &dir, &md, &[], TipoJuncao::Cruzado);
        assert_eq!(r.len(), 6);
    }

    /// **A direita VAZIA continua tendo colunas** -- elas vem do modelo, e
    /// nao da primeira linha. Com o modelo ignorado (`direita.first()`), a
    /// linha sairia so com `p.*`, e este teste cai.
    #[test]
    fn a_direita_vazia_da_colunas_nulas_pelo_modelo() {
        let (esq, me, _, md) = lados();
        let r = juntar(esq, &me, &[], &md, &pares(), TipoJuncao::Esquerdo);
        assert_eq!(r.len(), 3);
        for l in &r {
            assert_eq!(campo(l, "c.nome"), Some(&Json::Nulo), "{l:?}");
            assert_eq!(l.len(), 4, "{l:?}");
        }
    }

    /// `existe` mantem quem tem par (uma vez so) e, com `nao`, quem nao tem;
    /// a chave nula NAO existe na direita.
    #[test]
    fn a_semijuncao_filtra_sem_multiplicar() {
        let (esq, _, mut dir, _) = lados();
        // Dois pares para o id 1 na direita: a esquerda sai UMA vez.
        dir.push(linha(&[
            ("c.id", Json::Numero(1.0)),
            ("c.nome", Json::texto_de("ana2")),
        ]));
        let r = semijuntar(esq.clone(), &dir, &pares(), false);
        assert_eq!(r.len(), 1);
        assert_eq!(campo(&r[0], "p.id"), Some(&Json::Numero(1.0)));
        let r = semijuntar(esq, &dir, &pares(), true);
        let ids: Vec<i64> = r
            .iter()
            .map(|l| {
                l.iter()
                    .find(|(n, _)| n == "p.id")
                    .unwrap()
                    .1
                    .inteiro()
                    .unwrap()
            })
            .collect();
        assert_eq!(ids, vec![2, 3], "a chave nula tinha de ficar no `nao`");
    }

    /// O modelo vai e volta pelo cabecalho `colunas` sem perder o tipo.
    #[test]
    fn o_modelo_faz_a_ida_e_volta_pelo_json() {
        let m = modelo(&[
            ("p.id", ColumnType::Int4),
            ("preco", decimal2()),
            ("nome", ColumnType::Str(20)),
        ]);
        let j = modelo_para_json(&m);
        assert_eq!(modelo_de_json(Some(&j)), m);
        assert!(modelo_de_json(None).is_empty());
    }

    /// Resolver contra o modelo funciona sobre resultado VAZIO e recusa o
    /// ambiguo nomeando os dois.
    #[test]
    fn resolver_pelo_modelo_nao_precisa_de_linha() {
        let m = modelo(&[
            ("p.id", ColumnType::Int4),
            ("c.id", ColumnType::Int4),
            ("c.nome", ColumnType::Str(9)),
        ]);
        assert_eq!(resolver(&m, "nome").unwrap().as_deref(), Some("c.nome"));
        assert_eq!(resolver(&m, "P.ID").unwrap().as_deref(), Some("p.id"));
        assert_eq!(resolver(&m, "x").unwrap(), None);
        let e = resolver(&m, "id").unwrap_err().to_string();
        assert!(e.contains("p.id") && e.contains("c.id"), "{e}");
    }
}
