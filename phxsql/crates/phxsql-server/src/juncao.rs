//! Junções e união: as sete formas do diagrama, e o `UNION`.
//!
//! ```text
//!   ( A ∩ B )        interna          A ∪ B          completa
//!   ( A       )      esquerda         ( A − B )      so_esquerda
//!   (       B )      direita          ( B − A )      so_direita
//!                                     (A ∪ B) − (A ∩ B)   so_dos_lados
//! ```
//!
//! # Cinco modos, e não sete
//!
//! `direita` é `esquerda` com os lados trocados, e `so_direita` é
//! `so_esquerda` com os lados trocados. Quem troca é quem chama; aqui dentro
//! existem cinco casos, e o resultado sai com as colunas na ordem que o pedido
//! pediu. Escrever os sete daria dois caminhos a mais para o mesmo defeito
//! aparecer.
//!
//! A troca não é só economia de código: ela decide **qual tabela cabe na
//! memória**. O lado que a junção precisa inteiro (o `A` do `LEFT`) é o que
//! *streama*; o outro vira mapa. Num `RIGHT JOIN` de uma tabela enorme contra
//! um cadastro pequeno, trocar é o que faz o cadastro ser o mapa.
//!
//! # NULO nunca casa com NULO
//!
//! Em SQL, `A.chave = B.chave` com um dos lados nulo não dá falso: dá
//! *desconhecido*, e a linha não casa. Uma linha de A com chave nula se
//! comporta como linha **sem par** -- aparece no `LEFT`, some no `INNER`, e
//! aparece no `so_esquerda`. Não é detalhe: tratar nulo como um valor faria
//! todas as linhas sem chave de A casarem com todas as sem chave de B, e o
//! resultado explodiria em produto cartesiano com cara de junção.
//!
//! O resultado conta quantas linhas de cada lado tinham chave nula, para que
//! um `INNER` que devolveu menos do que se esperava tenha explicação em vez de
//! mistério.
//!
//! # Chave repetida multiplica
//!
//! Junção não é consulta: se a chave `7` aparece três vezes em B, cada linha
//! de A com chave `7` produz três linhas. É o comportamento certo, e é também
//! como uma junção descuidada vira milhões de linhas -- por isso há teto, e
//! ele avisa em vez de encher a memória calado.

use std::collections::HashMap;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::Schema;
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;

use crate::pivot::Iterador;

/// Qual das sete figuras do diagrama.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// `INNER JOIN` -- só o que casa dos dois lados.
    Interna,
    /// `LEFT JOIN` -- tudo de A, com B quando houver.
    Esquerda,
    /// `RIGHT JOIN` -- tudo de B, com A quando houver.
    Direita,
    /// `FULL OUTER JOIN` -- tudo dos dois.
    Completa,
    /// `LEFT JOIN … WHERE B.chave IS NULL` -- o que só existe em A.
    SoEsquerda,
    /// `RIGHT JOIN … WHERE A.chave IS NULL` -- o que só existe em B.
    SoDireita,
    /// `FULL OUTER … WHERE A.chave IS NULL OR B.chave IS NULL` -- o que existe
    /// num lado só, dos dois lados.
    SoDosLados,
}

impl Tipo {
    pub fn de_texto(t: &str) -> Result<Tipo> {
        Ok(
            match t
                .trim()
                .to_ascii_lowercase()
                .replace([' ', '-'], "_")
                .as_str()
            {
                "" | "interna" | "inner" | "inner_join" => Tipo::Interna,
                "esquerda" | "left" | "left_join" | "esquerda_externa" => Tipo::Esquerda,
                "direita" | "right" | "right_join" | "direita_externa" => Tipo::Direita,
                "completa" | "full" | "full_outer" | "full_outer_join" | "externa" => {
                    Tipo::Completa
                }
                "so_esquerda" | "left_only" | "so_a" | "anti_esquerda" => Tipo::SoEsquerda,
                "so_direita" | "right_only" | "so_b" | "anti_direita" => Tipo::SoDireita,
                "so_dos_lados" | "full_only" | "diferenca_simetrica" | "so_os_dois" => {
                    Tipo::SoDosLados
                }
                outro => {
                    return Err(PhxError::Esquema(format!(
                        "junção desconhecida: {outro:?} (use interna, esquerda, direita, \
                     completa, so_esquerda, so_direita ou so_dos_lados)"
                    )))
                }
            },
        )
    }

    pub fn nome(self) -> &'static str {
        match self {
            Tipo::Interna => "interna",
            Tipo::Esquerda => "esquerda",
            Tipo::Direita => "direita",
            Tipo::Completa => "completa",
            Tipo::SoEsquerda => "so_esquerda",
            Tipo::SoDireita => "so_direita",
            Tipo::SoDosLados => "so_dos_lados",
        }
    }

    /// O SQL equivalente, para a tela mostrar ao lado do desenho.
    pub fn sql(self) -> &'static str {
        match self {
            Tipo::Interna => "INNER JOIN",
            Tipo::Esquerda => "LEFT JOIN",
            Tipo::Direita => "RIGHT JOIN",
            Tipo::Completa => "FULL OUTER JOIN",
            Tipo::SoEsquerda => "LEFT JOIN … WHERE B.chave IS NULL",
            Tipo::SoDireita => "RIGHT JOIN … WHERE A.chave IS NULL",
            Tipo::SoDosLados => "FULL OUTER JOIN … WHERE A.chave IS NULL OR B.chave IS NULL",
        }
    }

    /// Este tipo é o espelho de outro com os lados trocados?
    ///
    /// Devolve o tipo a executar depois da troca. É onde os sete viram cinco.
    pub fn trocando_os_lados(self) -> Option<Tipo> {
        match self {
            Tipo::Direita => Some(Tipo::Esquerda),
            Tipo::SoDireita => Some(Tipo::SoEsquerda),
            _ => None,
        }
    }

    /// Precisa das linhas do lado do mapa que ninguém casou?
    fn quer_sobras_do_mapa(self) -> bool {
        matches!(self, Tipo::Completa | Tipo::SoDosLados)
    }

    /// Emite as linhas que casaram?
    fn quer_casadas(self) -> bool {
        matches!(self, Tipo::Interna | Tipo::Esquerda | Tipo::Completa)
    }

    /// Emite as linhas do fluxo que não casaram?
    fn quer_sobras_do_fluxo(self) -> bool {
        !matches!(self, Tipo::Interna)
    }
}

/// Um lado da junção, já resolvido contra o esquema.
pub struct Lado {
    /// Prefixo dos nomes na saída: `cliente.nome`.
    pub prefixo: String,
    pub esquema: Schema,
    /// Colunas da chave, na ordem. Mais de uma = chave composta.
    pub chave: Vec<usize>,
}

/// Uma coluna do resultado.
#[derive(Debug)]
pub struct ColunaSaida {
    /// `cliente.nome` -- já desambiguado.
    pub nome: String,
    pub ty: ColumnType,
    /// De qual lado ela veio, para a tela pintar a origem.
    pub lado: &'static str,
    /// Faz parte da chave da junção?
    pub chave: bool,
}

#[derive(Debug)]
pub struct Resultado {
    pub colunas: Vec<ColunaSaida>,
    pub linhas: Vec<Vec<Value>>,
    pub lidas_esquerda: u64,
    pub lidas_direita: u64,
    /// Linhas cuja chave tinha nulo, e que por isso nunca casam.
    pub chave_nula_esquerda: u64,
    pub chave_nula_direita: u64,
    /// O teto cortou o resultado?
    pub truncado: bool,
}

/// As famílias de tipo que podem ser comparadas entre si.
///
/// Juntar um `Int` com um `Str` não daria erro nenhum -- daria **zero linhas**,
/// que é o pior resultado possível: parece resposta. Recusar na entrada é o que
/// transforma um mistério de meia hora numa mensagem.
fn familia(t: &ColumnType) -> &'static str {
    match t {
        ColumnType::Bool => "booleano",
        ColumnType::Int1
        | ColumnType::Int2
        | ColumnType::Int4
        | ColumnType::Int8
        | ColumnType::UInt1
        | ColumnType::UInt2
        | ColumnType::UInt4
        | ColumnType::UInt8
        | ColumnType::Real4
        | ColumnType::Real8
        | ColumnType::Decimal { .. }
        | ColumnType::Sequence => "numero",
        ColumnType::Date => "data",
        ColumnType::Time => "hora",
        ColumnType::DateTime => "instante",
        ColumnType::Str(_) | ColumnType::Memo => "texto",
        ColumnType::Uuid => "uuid",
        ColumnType::Uuid256 => "uuid256",
        ColumnType::Bin => "binario",
    }
}

/// Confere que as duas chaves casam em quantidade e em família de tipo.
pub fn conferir_chaves(a: &Lado, b: &Lado) -> Result<()> {
    if a.chave.is_empty() {
        return Err(PhxError::Esquema(
            "a junção precisa de ao menos uma coluna de chave".into(),
        ));
    }
    if a.chave.len() != b.chave.len() {
        return Err(PhxError::Esquema(format!(
            "a chave tem {} coluna(s) de um lado e {} do outro: uma junção compara \
             par a par, na ordem",
            a.chave.len(),
            b.chave.len()
        )));
    }
    for (i, (ca, cb)) in a.chave.iter().zip(b.chave.iter()).enumerate() {
        let ta = &a.esquema.colunas()[*ca].ty;
        let tb = &b.esquema.colunas()[*cb].ty;
        if familia(ta) != familia(tb) {
            return Err(PhxError::Esquema(format!(
                "o par {} da chave compara {}.{} ({}) com {}.{} ({}), que são famílias \
                 diferentes: a junção não acharia par nenhum e devolveria zero linhas \
                 parecendo resposta",
                i + 1,
                a.prefixo,
                a.esquema.colunas()[*ca].nome,
                familia(ta),
                b.prefixo,
                b.esquema.colunas()[*cb].nome,
                familia(tb),
            )));
        }
        if matches!(ta, ColumnType::Bin) || matches!(tb, ColumnType::Bin) {
            return Err(PhxError::Esquema(
                "coluna binária não serve de chave de junção: ela mora no `.bin` e \
                 comparar dois blocos inteiros por linha custaria uma leitura a mais \
                 por comparação"
                    .into(),
            ));
        }
    }
    Ok(())
}

/// A chave de uma linha, ou `None` quando algum pedaço é nulo.
///
/// `None` quer dizer «nunca casa», que é o que o SQL faz. O prefixo de tipo
/// separa o número 1 do texto "1": sem ele, uma chave de texto casaria com uma
/// numérica por acidente de escrita.
fn chave_de(linha: &[Value], colunas: &[usize], esquema: &Schema) -> Option<String> {
    let mut k = String::with_capacity(colunas.len() * 12);
    for c in colunas {
        let v = linha.get(*c)?;
        if v.e_null() {
            return None;
        }
        // O separador é um byte que não aparece em texto de dado, para que
        // ("ab","c") não colida com ("a","bc") numa chave composta.
        k.push('\u{1}');
        k.push_str(&pedaco_de_chave(v, &esquema.colunas()[*c].ty));
    }
    Some(k)
}

/// Um valor na forma canônica de comparação.
fn pedaco_de_chave(v: &Value, ty: &ColumnType) -> String {
    match v {
        // Decimal precisa de forma canônica: 12.34 com escala 2 e 12.3400 com
        // escala 4 são o MESMO número e têm i128 diferente. Sem normalizar, as
        // duas tabelas não casariam por um zero à direita.
        Value::Decimal(d) => {
            let escala = match ty {
                ColumnType::Decimal { escala, .. } => *escala,
                _ => 0,
            };
            format!(
                "n{}",
                sem_zeros_a_direita(&crate::valores::decimal_para_texto(*d, escala))
            )
        }
        // Inteiro e decimal entram na mesma família, então precisam da mesma
        // forma: o inteiro 12 tem de casar com o decimal 12,00.
        Value::Int(i) => format!("n{i}"),
        Value::UInt(u) => format!("n{u}"),
        Value::Real(r) => format!("n{}", sem_zeros_a_direita(&format!("{r:.10}"))),
        Value::Bool(b) => format!("b{}", u8::from(*b)),
        Value::Date(d) => format!("d{d}"),
        Value::Time(t) => format!("h{t}"),
        Value::DateTime(m) => format!("i{m}"),
        Value::Str(s) | Value::Memo(s) => format!("t{s}"),
        Value::Uuid(u) => format!("u{u}"),
        Value::Uuid256(u) => format!("U{u}"),
        Value::Bin(b) => format!("x{}", b.len()),
        Value::Null => String::new(),
    }
}

fn sem_zeros_a_direita(t: &str) -> String {
    if !t.contains('.') {
        return t.to_string();
    }
    let t = t.trim_end_matches('0');
    t.trim_end_matches('.').to_string()
}

/// Monta as colunas da saída, com o prefixo de cada lado.
///
/// O prefixo não é enfeite: `clientes` e `pedidos` costumam ter os dois uma
/// coluna `id`, e sem prefixo a segunda apagaria a primeira em qualquer mapa
/// por nome -- que é exatamente o que a grade da tela usa.
fn colunas_de(a: &Lado, b: &Lado) -> Vec<ColunaSaida> {
    let mut v = Vec::with_capacity(a.esquema.colunas().len() + b.esquema.colunas().len());
    for (lado, rotulo) in [(a, "esquerda"), (b, "direita")] {
        for (i, c) in lado.esquema.colunas().iter().enumerate() {
            v.push(ColunaSaida {
                nome: format!("{}.{}", lado.prefixo, c.nome),
                ty: c.ty,
                lado: rotulo,
                chave: lado.chave.contains(&i),
            });
        }
    }
    v
}

/// Junta o fluxo com a tabela, na forma pedida.
///
/// `fluxo` é o lado que se lê linha a linha; `tabela` é o lado que já está na
/// memória. Quem chama decide qual é qual -- e para `direita` e `so_direita`
/// troca os dois e pede a forma espelhada, com `trocado`.
///
/// Com `trocado`, as colunas saem na ordem (tabela, fluxo), que é a ordem que
/// o pedido pediu: quem escreveu `A RIGHT JOIN B` quer ver A antes de B, mesmo
/// que B seja o lado que streama.
pub fn juntar(
    fluxo: &mut dyn Iterador,
    lado_fluxo: &Lado,
    tabela: &[Vec<Value>],
    lado_tabela: &Lado,
    tipo: Tipo,
    trocado: bool,
    max: u64,
) -> Result<Resultado> {
    let (a, b) = if trocado {
        (lado_tabela, lado_fluxo)
    } else {
        (lado_fluxo, lado_tabela)
    };
    conferir_chaves(a, b)?;

    // O mapa aponta para POSIÇÕES na tabela, e não para cópias das linhas: uma
    // chave com mil repetições guardaria mil cópias da linha, e não guarda
    // nenhuma.
    let mut indice: HashMap<String, Vec<usize>> = HashMap::new();
    let mut chave_nula_tabela = 0u64;
    for (i, linha) in tabela.iter().enumerate() {
        match chave_de(linha, &lado_tabela.chave, &lado_tabela.esquema) {
            Some(k) => indice.entry(k).or_default().push(i),
            None => chave_nula_tabela += 1,
        }
    }
    let mut casou_tabela = vec![false; tabela.len()];

    let vazio_fluxo = vec![Value::Null; lado_fluxo.esquema.colunas().len()];
    let vazio_tabela = vec![Value::Null; lado_tabela.esquema.colunas().len()];

    let mut linhas: Vec<Vec<Value>> = Vec::new();
    let mut lidas_fluxo = 0u64;
    let mut chave_nula_fluxo = 0u64;
    let mut truncado = false;

    // Monta uma linha de saída na ordem que o pedido pediu.
    let montar = |f: &[Value], t: &[Value]| -> Vec<Value> {
        let (primeiro, segundo) = if trocado { (t, f) } else { (f, t) };
        let mut l = Vec::with_capacity(primeiro.len() + segundo.len());
        l.extend_from_slice(primeiro);
        l.extend_from_slice(segundo);
        l
    };

    while let Some(linha) = fluxo.proxima()? {
        lidas_fluxo += 1;
        let k = chave_de(&linha, &lado_fluxo.chave, &lado_fluxo.esquema);
        if k.is_none() {
            chave_nula_fluxo += 1;
        }
        let pares = k.as_ref().and_then(|k| indice.get(k));
        match pares {
            Some(pos) if !pos.is_empty() => {
                for i in pos {
                    casou_tabela[*i] = true;
                }
                if tipo.quer_casadas() {
                    for i in pos {
                        if linhas.len() as u64 >= max {
                            truncado = true;
                            break;
                        }
                        linhas.push(montar(&linha, &tabela[*i]));
                    }
                }
            }
            _ => {
                if tipo.quer_sobras_do_fluxo() {
                    if linhas.len() as u64 >= max {
                        truncado = true;
                    } else {
                        linhas.push(montar(&linha, &vazio_tabela));
                    }
                }
            }
        }
        if truncado {
            break;
        }
    }

    // As sobras do lado do mapa só se sabem no fim: uma linha da tabela pode
    // casar com a última linha do fluxo.
    if !truncado && tipo.quer_sobras_do_mapa() {
        for (i, linha) in tabela.iter().enumerate() {
            if casou_tabela[i] {
                continue;
            }
            if linhas.len() as u64 >= max {
                truncado = true;
                break;
            }
            linhas.push(montar(&vazio_fluxo, linha));
        }
    }

    let (lidas_esquerda, lidas_direita) = if trocado {
        (tabela.len() as u64, lidas_fluxo)
    } else {
        (lidas_fluxo, tabela.len() as u64)
    };
    let (nula_esq, nula_dir) = if trocado {
        (chave_nula_tabela, chave_nula_fluxo)
    } else {
        (chave_nula_fluxo, chave_nula_tabela)
    };

    Ok(Resultado {
        colunas: colunas_de(a, b),
        linhas,
        lidas_esquerda,
        lidas_direita,
        chave_nula_esquerda: nula_esq,
        chave_nula_direita: nula_dir,
        truncado,
    })
}

// ------------------------------------------------------------------- união

/// `UNION` empilha; `UNION ALL` empilha sem tirar repetida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uniao {
    /// Tira as linhas repetidas. É o `UNION` do SQL.
    Distinta,
    /// Mantém tudo. É o `UNION ALL`.
    Tudo,
}

impl Uniao {
    pub fn de_texto(t: &str) -> Result<Uniao> {
        Ok(
            match t
                .trim()
                .to_ascii_lowercase()
                .replace([' ', '-'], "_")
                .as_str()
            {
                "" | "distinta" | "union" | "distinct" => Uniao::Distinta,
                "tudo" | "all" | "union_all" | "todas" => Uniao::Tudo,
                outro => {
                    return Err(PhxError::Esquema(format!(
                        "união desconhecida: {outro:?} (use distinta ou tudo)"
                    )))
                }
            },
        )
    }

    pub fn nome(self) -> &'static str {
        match self {
            Uniao::Distinta => "distinta",
            Uniao::Tudo => "tudo",
        }
    }

    pub fn sql(self) -> &'static str {
        match self {
            Uniao::Distinta => "UNION",
            Uniao::Tudo => "UNION ALL",
        }
    }
}

#[derive(Debug)]
pub struct ResultadoUniao {
    pub colunas: Vec<ColunaSaida>,
    pub linhas: Vec<Vec<Value>>,
    /// Quantas linhas cada parte trouxe, na ordem em que foram pedidas.
    pub por_parte: Vec<u64>,
    /// Quantas o `UNION` descartou por serem repetidas.
    pub repetidas: u64,
    pub truncado: bool,
}

/// Confere que as partes de uma união empilham.
///
/// O SQL exige mesma quantidade de colunas e tipos compatíveis, posição a
/// posição -- **o nome não importa**, a posição sim. Empilhar por nome
/// pareceria mais amigável e seria uma armadilha: duas tabelas com as mesmas
/// colunas em ordem diferente empilhariam trocando os valores de coluna, calado.
pub fn conferir_uniao(esquemas: &[&Schema]) -> Result<()> {
    let Some(primeiro) = esquemas.first() else {
        return Err(PhxError::Esquema(
            "a união precisa de ao menos uma parte".into(),
        ));
    };
    let n = primeiro.colunas().len();
    for (i, e) in esquemas.iter().enumerate().skip(1) {
        if e.colunas().len() != n {
            return Err(PhxError::Esquema(format!(
                "a parte {} tem {} coluna(s) e a primeira tem {}: uma união empilha \
                 posição a posição, então a quantidade tem de bater",
                i + 1,
                e.colunas().len(),
                n
            )));
        }
        for (c, (ca, cb)) in primeiro
            .colunas()
            .iter()
            .zip(e.colunas().iter())
            .enumerate()
        {
            if familia(&ca.ty) != familia(&cb.ty) {
                return Err(PhxError::Esquema(format!(
                    "na coluna {} a parte 1 traz {} ({}) e a parte {} traz {} ({}): \
                     famílias diferentes não empilham",
                    c + 1,
                    ca.nome,
                    familia(&ca.ty),
                    i + 1,
                    cb.nome,
                    familia(&cb.ty),
                )));
            }
        }
    }
    Ok(())
}

/// Empilha as partes.
///
/// Os nomes de coluna saem da PRIMEIRA parte, como no SQL.
pub fn unir(
    partes: &mut [(&mut dyn Iterador, &Schema)],
    modo: Uniao,
    max: u64,
) -> Result<ResultadoUniao> {
    let esquemas: Vec<&Schema> = partes.iter().map(|(_, e)| *e).collect();
    conferir_uniao(&esquemas)?;
    let primeiro = esquemas[0];

    // Os nomes saem da primeira parte, como no SQL. As outras contribuem
    // linhas, não cabeçalho.
    let colunas: Vec<ColunaSaida> = primeiro
        .colunas()
        .iter()
        .map(|c| ColunaSaida {
            nome: c.nome.clone(),
            ty: c.ty,
            lado: "uniao",
            chave: false,
        })
        .collect();

    let mut linhas: Vec<Vec<Value>> = Vec::new();
    let mut vistas: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut por_parte = Vec::with_capacity(partes.len());
    let mut repetidas = 0u64;
    let mut truncado = false;

    for (fonte, esquema) in partes.iter_mut() {
        let mut desta = 0u64;
        while let Some(linha) = fonte.proxima()? {
            desta += 1;
            if modo == Uniao::Distinta {
                // A linha inteira vira chave, com o mesmo canonizador da
                // junção: assim 12,00 e 12 contam como repetida, que é o que o
                // SQL faz ao comparar valores e não bytes.
                let k: String = linha
                    .iter()
                    .zip(esquema.colunas())
                    .map(|(v, c)| {
                        if v.e_null() {
                            "\u{1}∅".to_string()
                        } else {
                            format!("\u{1}{}", pedaco_de_chave(v, &c.ty))
                        }
                    })
                    .collect();
                if !vistas.insert(k) {
                    repetidas += 1;
                    continue;
                }
            }
            if linhas.len() as u64 >= max {
                truncado = true;
                break;
            }
            linhas.push(linha);
        }
        por_parte.push(desta);
        if truncado {
            break;
        }
    }

    Ok(ResultadoUniao {
        colunas,
        linhas,
        por_parte,
        repetidas,
        truncado,
    })
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
    fn lista(v: Vec<Vec<Value>>) -> Lista {
        Lista(v.into_iter())
    }

    fn esq(nome: &str, colunas: Vec<Column>) -> Schema {
        Schema::new(nome, colunas, vec![]).unwrap()
    }

    fn txt(s: &str) -> Value {
        Value::Str(s.to_string())
    }

    /// A = clientes (id, nome); B = pedidos (cliente_id, valor).
    ///
    /// Cliente 3 não tem pedido; o pedido do cliente 9 não tem cliente. É o
    /// desenho mínimo que separa as sete figuras umas das outras.
    fn cenario() -> (Lado, Vec<Vec<Value>>, Lado, Vec<Vec<Value>>) {
        let a = Lado {
            prefixo: "c".into(),
            esquema: esq(
                "clientes",
                vec![
                    Column::new("id", ColumnType::Int4),
                    Column::new("nome", ColumnType::Str(20)),
                ],
            ),
            chave: vec![0],
        };
        let la = vec![
            vec![Value::Int(1), txt("Adriano")],
            vec![Value::Int(2), txt("Maria")],
            vec![Value::Int(3), txt("João")],
        ];
        let b = Lado {
            prefixo: "p".into(),
            esquema: esq(
                "pedidos",
                vec![
                    Column::new("cliente_id", ColumnType::Int4),
                    Column::new("valor", ColumnType::Int4),
                ],
            ),
            chave: vec![0],
        };
        let lb = vec![
            vec![Value::Int(1), Value::Int(100)],
            vec![Value::Int(1), Value::Int(200)],
            vec![Value::Int(2), Value::Int(300)],
            vec![Value::Int(9), Value::Int(400)],
        ];
        (a, la, b, lb)
    }

    /// Roda a junção do jeito que o servidor roda: `direita` troca os lados.
    fn rodar(tipo: Tipo) -> Resultado {
        let (a, la, b, lb) = cenario();
        match tipo.trocando_os_lados() {
            // A troca põe B para streamar e A no mapa -- e as colunas ainda
            // saem na ordem A, B.
            Some(espelho) => juntar(&mut lista(lb), &b, &la, &a, espelho, true, 10_000).unwrap(),
            None => juntar(&mut lista(la), &a, &lb, &b, tipo, false, 10_000).unwrap(),
        }
    }

    /// Só os nomes de cliente, para a asserção ficar legível.
    fn nomes(r: &Resultado) -> Vec<String> {
        r.linhas
            .iter()
            .map(|l| match &l[1] {
                Value::Str(s) => s.clone(),
                Value::Null => "—".into(),
                outro => format!("{outro:?}"),
            })
            .collect()
    }

    fn valores(r: &Resultado) -> Vec<String> {
        r.linhas
            .iter()
            .map(|l| match &l[3] {
                Value::Int(i) => i.to_string(),
                Value::Null => "—".into(),
                outro => format!("{outro:?}"),
            })
            .collect()
    }

    #[test]
    fn as_sete_figuras_do_diagrama() {
        // INNER: só quem casa. Adriano aparece duas vezes -- tem dois pedidos.
        let r = rodar(Tipo::Interna);
        assert_eq!(nomes(&r), ["Adriano", "Adriano", "Maria"]);

        // LEFT: tudo de A. João entra com o lado B vazio.
        let r = rodar(Tipo::Esquerda);
        assert_eq!(nomes(&r), ["Adriano", "Adriano", "Maria", "João"]);
        assert_eq!(valores(&r), ["100", "200", "300", "—"]);

        // RIGHT: tudo de B. O pedido órfão entra com o lado A vazio.
        let r = rodar(Tipo::Direita);
        assert_eq!(nomes(&r), ["Adriano", "Adriano", "Maria", "—"]);
        assert_eq!(valores(&r), ["100", "200", "300", "400"]);

        // FULL: tudo dos dois.
        let mut n = nomes(&rodar(Tipo::Completa));
        n.sort();
        assert_eq!(n, ["Adriano", "Adriano", "João", "Maria", "—"]);

        // Só A: o cliente sem pedido.
        let r = rodar(Tipo::SoEsquerda);
        assert_eq!(nomes(&r), ["João"]);

        // Só B: o pedido sem cliente.
        let r = rodar(Tipo::SoDireita);
        assert_eq!(nomes(&r), ["—"]);
        assert_eq!(valores(&r), ["400"]);

        // Os dois lados de fora, e nenhum do meio.
        let mut n = nomes(&rodar(Tipo::SoDosLados));
        n.sort();
        assert_eq!(n, ["João", "—"]);
    }

    /// As colunas saem na ordem A, B mesmo quando o RIGHT trocou os lados para
    /// escolher quem cabe na memória.
    #[test]
    fn o_right_troca_os_lados_mas_nao_a_ordem_das_colunas() {
        let r = rodar(Tipo::Direita);
        let nomes: Vec<&str> = r.colunas.iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(nomes, ["c.id", "c.nome", "p.cliente_id", "p.valor"]);
        assert_eq!(r.colunas[0].lado, "esquerda");
        assert_eq!(r.colunas[3].lado, "direita");
        // E as contagens também não trocam de lugar.
        assert_eq!(r.lidas_esquerda, 3);
        assert_eq!(r.lidas_direita, 4);
    }

    /// A armadilha clássica: em SQL, nulo não é igual a nulo.
    ///
    /// Se nulo casasse com nulo, os dois clientes sem código casariam com os
    /// dois pedidos sem código e sairiam QUATRO linhas do nada.
    #[test]
    fn nulo_nunca_casa_com_nulo() {
        let (a, mut la, b, mut lb) = cenario();
        la.push(vec![Value::Null, txt("Sem código A")]);
        la.push(vec![Value::Null, txt("Outro sem código")]);
        lb.push(vec![Value::Null, Value::Int(500)]);
        lb.push(vec![Value::Null, Value::Int(600)]);

        let r = juntar(
            &mut lista(la.clone()),
            &a,
            &lb,
            &b,
            Tipo::Interna,
            false,
            10_000,
        )
        .unwrap();
        assert_eq!(nomes(&r), ["Adriano", "Adriano", "Maria"]);
        assert_eq!(r.chave_nula_esquerda, 2);
        assert_eq!(r.chave_nula_direita, 2);

        // No LEFT elas aparecem, como linhas sem par -- que é o que são.
        let r = juntar(&mut lista(la), &a, &lb, &b, Tipo::Esquerda, false, 10_000).unwrap();
        assert_eq!(
            nomes(&r),
            [
                "Adriano",
                "Adriano",
                "Maria",
                "João",
                "Sem código A",
                "Outro sem código"
            ]
        );
    }

    /// Chave repetida dos DOIS lados multiplica: 2 × 3 = 6.
    #[test]
    fn chave_repetida_dos_dois_lados_multiplica() {
        let a = Lado {
            prefixo: "a".into(),
            esquema: esq("a", vec![Column::new("k", ColumnType::Int4)]),
            chave: vec![0],
        };
        let b = Lado {
            prefixo: "b".into(),
            esquema: esq("b", vec![Column::new("k", ColumnType::Int4)]),
            chave: vec![0],
        };
        let la = vec![vec![Value::Int(7)], vec![Value::Int(7)]];
        let lb = vec![
            vec![Value::Int(7)],
            vec![Value::Int(7)],
            vec![Value::Int(7)],
        ];
        let r = juntar(&mut lista(la), &a, &lb, &b, Tipo::Interna, false, 10_000).unwrap();
        assert_eq!(r.linhas.len(), 6);
    }

    /// Chave composta compara par a par, e o separador impede que
    /// ("ab","c") case com ("a","bc").
    #[test]
    fn chave_composta_nao_confunde_a_emenda() {
        let cols = || {
            vec![
                Column::new("p1", ColumnType::Str(10)),
                Column::new("p2", ColumnType::Str(10)),
            ]
        };
        let a = Lado {
            prefixo: "a".into(),
            esquema: esq("a", cols()),
            chave: vec![0, 1],
        };
        let b = Lado {
            prefixo: "b".into(),
            esquema: esq("b", cols()),
            chave: vec![0, 1],
        };
        let la = vec![vec![txt("ab"), txt("c")]];
        let lb = vec![vec![txt("a"), txt("bc")]];
        let r = juntar(&mut lista(la), &a, &lb, &b, Tipo::Interna, false, 10_000).unwrap();
        assert!(
            r.linhas.is_empty(),
            "as chaves emendaram e casaram por acidente"
        );

        // E o par certo casa.
        let la = vec![vec![txt("a"), txt("bc")]];
        let lb = vec![vec![txt("a"), txt("bc")]];
        let r = juntar(&mut lista(la), &a, &lb, &b, Tipo::Interna, false, 10_000).unwrap();
        assert_eq!(r.linhas.len(), 1);
    }

    /// Um decimal 12,3400 e um 12,34 são o mesmo número com i128 diferente.
    #[test]
    fn decimal_casa_por_valor_e_nao_por_escala() {
        let a = Lado {
            prefixo: "a".into(),
            esquema: esq(
                "a",
                vec![Column::new(
                    "v",
                    ColumnType::Decimal {
                        precisao: 10,
                        escala: 2,
                    },
                )],
            ),
            chave: vec![0],
        };
        let b = Lado {
            prefixo: "b".into(),
            esquema: esq(
                "b",
                vec![Column::new(
                    "v",
                    ColumnType::Decimal {
                        precisao: 10,
                        escala: 4,
                    },
                )],
            ),
            chave: vec![0],
        };
        // 12,34 com escala 2 e com escala 4.
        let la = vec![vec![Value::Decimal(1_234)]];
        let lb = vec![vec![Value::Decimal(123_400)]];
        let r = juntar(&mut lista(la), &a, &lb, &b, Tipo::Interna, false, 10_000).unwrap();
        assert_eq!(r.linhas.len(), 1, "12,34 não casou com 12,3400");
    }

    /// Juntar número com texto daria ZERO linhas, que parece resposta.
    #[test]
    fn familia_errada_e_recusada_na_entrada() {
        let a = Lado {
            prefixo: "a".into(),
            esquema: esq("a", vec![Column::new("k", ColumnType::Int4)]),
            chave: vec![0],
        };
        let b = Lado {
            prefixo: "b".into(),
            esquema: esq("b", vec![Column::new("k", ColumnType::Str(10))]),
            chave: vec![0],
        };
        let e = juntar(&mut lista(vec![]), &a, &[], &b, Tipo::Interna, false, 10).unwrap_err();
        assert!(e.to_string().contains("famílias diferentes"), "{e}");
    }

    #[test]
    fn chave_de_tamanhos_diferentes_e_recusada() {
        let a = Lado {
            prefixo: "a".into(),
            esquema: esq(
                "a",
                vec![
                    Column::new("k1", ColumnType::Int4),
                    Column::new("k2", ColumnType::Int4),
                ],
            ),
            chave: vec![0, 1],
        };
        let b = Lado {
            prefixo: "b".into(),
            esquema: esq("b", vec![Column::new("k", ColumnType::Int4)]),
            chave: vec![0],
        };
        assert!(juntar(&mut lista(vec![]), &a, &[], &b, Tipo::Interna, false, 10).is_err());
    }

    /// O teto corta e AVISA. Cortar calado devolveria meia resposta com cara
    /// de resposta inteira.
    #[test]
    fn o_teto_corta_e_se_declara() {
        let (a, la, b, lb) = cenario();
        let r = juntar(&mut lista(la), &a, &lb, &b, Tipo::Interna, false, 2).unwrap();
        assert_eq!(r.linhas.len(), 2);
        assert!(r.truncado);
    }

    // ------------------------------------------------------------ união

    fn esquema_uniao(nome: &str) -> Schema {
        esq(
            nome,
            vec![
                Column::new("codigo", ColumnType::Int4),
                Column::new("nome", ColumnType::Str(20)),
            ],
        )
    }

    #[test]
    fn union_tira_repetida_e_union_all_nao() {
        let e1 = esquema_uniao("matriz");
        let e2 = esquema_uniao("filial");
        let p1 = || {
            vec![
                vec![Value::Int(1), txt("caneta")],
                vec![Value::Int(2), txt("papel")],
            ]
        };
        let p2 = || {
            vec![
                vec![Value::Int(2), txt("papel")],
                vec![Value::Int(3), txt("tinta")],
            ]
        };

        let (mut a, mut b) = (lista(p1()), lista(p2()));
        let mut partes: Vec<(&mut dyn Iterador, &Schema)> = vec![(&mut a, &e1), (&mut b, &e2)];
        let r = unir(&mut partes, Uniao::Distinta, 100).unwrap();
        assert_eq!(r.linhas.len(), 3);
        assert_eq!(r.repetidas, 1);
        assert_eq!(r.por_parte, vec![2, 2]);
        // Os nomes saem da primeira parte.
        assert_eq!(r.colunas[0].nome, "codigo");

        let (mut a, mut b) = (lista(p1()), lista(p2()));
        let mut partes: Vec<(&mut dyn Iterador, &Schema)> = vec![(&mut a, &e1), (&mut b, &e2)];
        let r = unir(&mut partes, Uniao::Tudo, 100).unwrap();
        assert_eq!(r.linhas.len(), 4);
        assert_eq!(r.repetidas, 0);
    }

    /// Duas linhas nulas na mesma posição são a MESMA linha para o `UNION` --
    /// diferente da junção, onde nulo nunca casa. As duas regras são do SQL, e
    /// são mesmo diferentes.
    #[test]
    fn no_union_duas_linhas_nulas_sao_repetidas() {
        let e = esquema_uniao("x");
        let mut a = lista(vec![vec![Value::Null, Value::Null]]);
        let mut b = lista(vec![vec![Value::Null, Value::Null]]);
        let mut partes: Vec<(&mut dyn Iterador, &Schema)> = vec![(&mut a, &e), (&mut b, &e)];
        let r = unir(&mut partes, Uniao::Distinta, 100).unwrap();
        assert_eq!(r.linhas.len(), 1);
        assert_eq!(r.repetidas, 1);
    }

    #[test]
    fn parte_com_outra_quantidade_de_colunas_nao_empilha() {
        let e1 = esquema_uniao("a");
        let e2 = esq("b", vec![Column::new("codigo", ColumnType::Int4)]);
        assert!(conferir_uniao(&[&e1, &e2]).is_err());
    }

    /// Empilhar é por POSIÇÃO, não por nome: a conferência olha o tipo da
    /// posição, e trocar a ordem das colunas é erro de quem monta.
    #[test]
    fn a_uniao_confere_a_familia_posicao_a_posicao() {
        let e1 = esquema_uniao("a");
        let trocado = esq(
            "b",
            vec![
                Column::new("nome", ColumnType::Str(20)),
                Column::new("codigo", ColumnType::Int4),
            ],
        );
        let erro = conferir_uniao(&[&e1, &trocado]).unwrap_err().to_string();
        assert!(erro.contains("famílias diferentes"), "{erro}");
    }
}
