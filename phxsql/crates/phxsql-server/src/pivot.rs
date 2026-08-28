//! Tabela dinamica: a tabulacao cruzada de uma tabela, com junção opcional.
//!
//! # Por que no servidor, e nao no navegador
//!
//! Um pivot resume: cem mil linhas viram uma grade de vinte por doze. Mandar as
//! cem mil pela rede para o navegador somar seria pagar o transporte do que vai
//! ser jogado fora. A agregacao acontece aqui e o que atravessa e o resumo.
//!
//! # A junção e por tabela de consulta, nao por linha
//!
//! Cruzar "vendas por cidade do cliente" exige a cidade, que mora na outra
//! tabela. A forma ingenua -- uma busca no indice por linha de venda -- custaria
//! uma descida na arvore por linha. Aqui a tabela de consulta e lida UMA vez
//! para um mapa em memoria, e o cruzamento vira acesso direto. E o *hash join*,
//! e para a forma de dado que um pivot cruza (muitos fatos, poucas dimensoes)
//! ele e a escolha certa.
//!
//! # Decimal soma exato
//!
//! Somar dinheiro em `f64` perde centavo, e a regra do projeto e nao perder. Um
//! `Decimal` e somado no proprio dominio inteiro escalado e so vira texto na
//! saida. `avg` divide no fim, uma vez -- nao a cada parcela.

use std::collections::HashMap;

use phxsql_core::datahora::{civil_de_dias, data_iso, hora_iso, instante_iso};
use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::Schema;
use phxsql_core::value::Value;

use crate::valores::decimal_para_texto;

/// Como as celulas resumem os valores que caem nelas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agregador {
    Soma,
    Media,
    Contagem,
    Minimo,
    Maximo,
    /// Quantos valores DIFERENTES caem na celula.
    ContagemDistinta,
}

impl Agregador {
    pub fn de_texto(t: &str) -> Result<Agregador> {
        Ok(match t.trim().to_ascii_lowercase().as_str() {
            "" | "soma" | "sum" => Agregador::Soma,
            "media" | "média" | "avg" => Agregador::Media,
            "contagem" | "count" => Agregador::Contagem,
            "minimo" | "mínimo" | "min" => Agregador::Minimo,
            "maximo" | "máximo" | "max" => Agregador::Maximo,
            "distintos" | "count_distinct" | "distinct" => Agregador::ContagemDistinta,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "agregador desconhecido: {outro:?} \
                     (use soma, media, contagem, minimo, maximo ou distintos)"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Agregador::Soma => "soma",
            Agregador::Media => "media",
            Agregador::Contagem => "contagem",
            Agregador::Minimo => "minimo",
            Agregador::Maximo => "maximo",
            Agregador::ContagemDistinta => "distintos",
        }
    }

    /// Contar nao precisa de coluna de valor; o resto precisa.
    pub fn precisa_de_valor(self) -> bool {
        !matches!(self, Agregador::Contagem)
    }
}

/// O que se acumula numa celula enquanto a varredura corre.
///
/// Guarda os dois dominios porque a coluna decide qual vale: `Decimal` soma
/// inteiro escalado (exato), o resto soma em `f64`. Manter os dois custa 40
/// bytes por celula e evita um enum que teria de ser conferido a cada linha.
#[derive(Debug, Default, Clone)]
struct Acumulador {
    n: u64,
    soma_i: i128,
    soma_f: f64,
    min_f: Option<f64>,
    max_f: Option<f64>,
    min_i: Option<i128>,
    max_i: Option<i128>,
    /// So enche quando o agregador e `distintos` -- senao seria memoria jogada
    /// fora numa varredura de milhoes de linhas.
    vistos: Option<std::collections::HashSet<String>>,
}

impl Acumulador {
    fn novo(distinta: bool) -> Acumulador {
        Acumulador {
            vistos: distinta.then(std::collections::HashSet::new),
            ..Default::default()
        }
    }

    fn somar(&mut self, v: &Value) {
        self.n += 1;
        if let Some(s) = &mut self.vistos {
            s.insert(rotulo_cru(v));
        }
        match v {
            Value::Decimal(d) => {
                self.soma_i += *d;
                self.min_i = Some(self.min_i.map_or(*d, |m| m.min(*d)));
                self.max_i = Some(self.max_i.map_or(*d, |m| m.max(*d)));
            }
            outro => {
                if let Some(f) = como_f64(outro) {
                    self.soma_f += f;
                    self.min_f = Some(self.min_f.map_or(f, |m| m.min(f)));
                    self.max_f = Some(self.max_f.map_or(f, |m| m.max(f)));
                }
            }
        }
    }
}

/// O valor de um campo como numero, quando ele tem um.
fn como_f64(v: &Value) -> Option<f64> {
    Some(match v {
        Value::Int(i) => *i as f64,
        Value::UInt(u) => *u as f64,
        Value::Real(r) => *r,
        Value::Bool(b) => *b as u8 as f64,
        Value::Date(d) => *d as f64,
        Value::Time(t) => *t as f64,
        Value::DateTime(m) => *m as f64,
        _ => return None,
    })
}

/// O rotulo de um valor: o que vira cabecalho de linha ou de coluna.
///
/// Data vira `AAAA-MM-DD` e nao um numero de dias, porque cabecalho e para
/// ler. Nulo vira uma marca visivel em vez de sumir -- uma linha sem valor
/// ainda e uma linha, e escondê-la faria os totais nao fecharem.
pub fn rotulo(v: &Value, escala: u8) -> String {
    match v {
        Value::Null => "(vazio)".to_string(),
        Value::Decimal(d) => decimal_para_texto(*d, escala),
        Value::Date(d) => data_iso(*d),
        Value::Time(t) => hora_iso(*t),
        Value::DateTime(m) => instante_iso(*m),
        outro => rotulo_cru(outro),
    }
}

fn rotulo_cru(v: &Value) -> String {
    match v {
        Value::Null => "(vazio)".to_string(),
        Value::Bool(b) => (if *b { "sim" } else { "nao" }).to_string(),
        Value::Int(i) => i.to_string(),
        Value::UInt(u) => u.to_string(),
        Value::Real(r) => r.to_string(),
        Value::Decimal(d) => d.to_string(),
        Value::Date(d) => d.to_string(),
        Value::Time(t) => t.to_string(),
        Value::DateTime(m) => m.to_string(),
        Value::Str(s) | Value::Memo(s) => s.clone(),
        Value::Uuid(u) => u.to_string(),
        Value::Uuid256(u) => u.to_string(),
        Value::Bin(b) => format!("<{} bytes>", b.len()),
    }
}

/// Um campo agrupado por periodo em vez de valor exato.
///
/// Cruzar vendas por DIA da uma coluna por dia do ano -- inutil. O que se quer
/// e por mes, trimestre ou ano, e isso e escolha de quem monta o pivot, nao
/// propriedade do dado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularidade {
    Exato,
    Dia,
    Mes,
    Trimestre,
    Ano,
}

impl Granularidade {
    pub fn de_texto(t: &str) -> Result<Granularidade> {
        Ok(match t.trim().to_ascii_lowercase().as_str() {
            "" | "exato" => Granularidade::Exato,
            "dia" => Granularidade::Dia,
            "mes" | "mês" => Granularidade::Mes,
            "trimestre" => Granularidade::Trimestre,
            "ano" => Granularidade::Ano,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "granularidade desconhecida: {outro:?} \
                     (use exato, dia, mes, trimestre ou ano)"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Granularidade::Exato => "exato",
            Granularidade::Dia => "dia",
            Granularidade::Mes => "mes",
            Granularidade::Trimestre => "trimestre",
            Granularidade::Ano => "ano",
        }
    }

    /// O rotulo do periodo a que um valor pertence.
    ///
    /// Sai em ordem lexicografica crescente de proposito (`2026-Q1` antes de
    /// `2026-Q2`), para que ordenar texto ja ordene tempo.
    fn aplicar(self, v: &Value, escala: u8) -> String {
        if self == Granularidade::Exato {
            return rotulo(v, escala);
        }
        let dias = match v {
            Value::Date(d) => *d,
            Value::DateTime(ms) => ms.div_euclid(86_400_000) as i32,
            // Campo que nao e data ignora a granularidade em vez de recusar:
            // trocar a linha do pivot nao pode derrubar a tela.
            outro => return rotulo(outro, escala),
        };
        let (ano, mes, dia) = civil_de_dias(dias);
        match self {
            Granularidade::Dia => format!("{ano:04}-{mes:02}-{dia:02}"),
            Granularidade::Mes => format!("{ano:04}-{mes:02}"),
            Granularidade::Trimestre => format!("{ano:04}-T{}", (mes - 1) / 3 + 1),
            Granularidade::Ano => format!("{ano:04}"),
            Granularidade::Exato => unreachable!(),
        }
    }
}

/// Um campo escolhido para linha, coluna ou valor.
#[derive(Debug, Clone)]
pub struct Campo {
    /// Nome como veio do pedido: `cidade` ou `cliente.cidade`.
    pub qualificado: String,
    /// De qual junção ele vem. `None` = a tabela principal.
    pub juncao: Option<usize>,
    /// Posicao dentro do esquema da tabela de onde ele vem.
    pub coluna: usize,
    pub granularidade: Granularidade,
}

/// Uma tabela de consulta lida inteira para a memoria.
pub struct Juncao {
    pub prefixo: String,
    pub esquema: Schema,
    /// Coluna da tabela PRINCIPAL que aponta para ca.
    pub coluna_local: usize,
    /// Valor da chave -> a linha inteira da tabela de consulta.
    pub mapa: HashMap<String, Vec<Value>>,
    pub lidas: usize,
}

/// O resultado: uma grade de rotulos com as celulas e os totais.
pub struct Resultado {
    pub rotulos_linha: Vec<String>,
    pub rotulos_coluna: Vec<String>,
    /// `celulas[l][c]`. `None` quando nenhuma linha caiu ali.
    pub celulas: Vec<Vec<Option<String>>>,
    pub total_linha: Vec<Option<String>>,
    pub total_coluna: Vec<Option<String>>,
    pub total: Option<String>,
    pub lidas: u64,
    pub consideradas: u64,
}

/// Monta a tabulacao cruzada.
///
/// `linhas` e `colunas` podem ter mais de um campo: os rotulos entao se
/// concatenam com ` / `, que e o mesmo que um pivot faz ao empilhar niveis.
#[allow(clippy::too_many_arguments)]
pub fn cruzar(
    fatos: &mut dyn Iterador,
    esquema: &Schema,
    juncoes: &[Juncao],
    linhas: &[Campo],
    colunas: &[Campo],
    valor: Option<&Campo>,
    agregador: Agregador,
    max: u64,
) -> Result<Resultado> {
    let distinta = agregador == Agregador::ContagemDistinta;
    let mut celulas: HashMap<(String, String), Acumulador> = HashMap::new();
    let mut por_linha: HashMap<String, Acumulador> = HashMap::new();
    let mut por_coluna: HashMap<String, Acumulador> = HashMap::new();
    let mut geral = Acumulador::novo(distinta);
    let mut lidas = 0u64;
    let mut consideradas = 0u64;

    while let Some(linha) = fatos.proxima()? {
        lidas += 1;
        if lidas > max {
            break;
        }
        // A chave de linha e de coluna sai da linha JA enriquecida com o que as
        // junções trouxeram -- senao um campo da tabela de consulta nao poderia
        // ser eixo, que e o motivo de a junção existir.
        let resolver = |cs: &[Campo]| -> String {
            cs.iter()
                .map(|c| valor_do_campo(c, &linha, esquema, juncoes))
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let kl = resolver(linhas);
        let kc = if colunas.is_empty() {
            "total".to_string()
        } else {
            resolver(colunas)
        };

        let v = match valor {
            None => Value::Int(1),
            Some(c) => match valor_bruto(c, &linha, esquema, juncoes) {
                Some(v) => v,
                None => Value::Null,
            },
        };
        // Nulo nao entra na conta: somar "sem valor" como zero afundaria a
        // media e faria o minimo virar zero.
        if valor.is_some() && v == Value::Null && agregador != Agregador::Contagem {
            continue;
        }
        consideradas += 1;

        celulas
            .entry((kl.clone(), kc.clone()))
            .or_insert_with(|| Acumulador::novo(distinta))
            .somar(&v);
        por_linha
            .entry(kl)
            .or_insert_with(|| Acumulador::novo(distinta))
            .somar(&v);
        por_coluna
            .entry(kc)
            .or_insert_with(|| Acumulador::novo(distinta))
            .somar(&v);
        geral.somar(&v);
    }

    let mut rl: Vec<String> = por_linha.keys().cloned().collect();
    let mut rc: Vec<String> = por_coluna.keys().cloned().collect();
    rl.sort();
    rc.sort();

    let escala = valor
        .map(|c| escala_do_campo(c, esquema, juncoes))
        .unwrap_or(0);
    let decimal = valor
        .map(|c| e_decimal(c, esquema, juncoes))
        .unwrap_or(false);
    let fechar = |a: &Acumulador| fechar_acumulador(a, agregador, decimal, escala);

    let mut grade = Vec::with_capacity(rl.len());
    for l in &rl {
        let mut linha_out = Vec::with_capacity(rc.len());
        for c in &rc {
            linha_out.push(celulas.get(&(l.clone(), c.clone())).map(&fechar));
        }
        grade.push(linha_out);
    }

    Ok(Resultado {
        total_linha: rl.iter().map(|l| por_linha.get(l).map(&fechar)).collect(),
        total_coluna: rc.iter().map(|c| por_coluna.get(c).map(&fechar)).collect(),
        total: (geral.n > 0).then(|| fechar(&geral)),
        rotulos_linha: rl,
        rotulos_coluna: rc,
        celulas: grade,
        lidas,
        consideradas,
    })
}

/// Fecha um acumulador no numero que a celula mostra.
fn fechar_acumulador(a: &Acumulador, ag: Agregador, decimal: bool, escala: u8) -> String {
    match ag {
        Agregador::Contagem => a.n.to_string(),
        Agregador::ContagemDistinta => a.vistos.as_ref().map_or(0, |s| s.len()).to_string(),
        _ if decimal => {
            // O dominio inteiro escalado nao perde centavo. A media divide UMA
            // vez, no fim -- dividir a cada parcela acumularia arredondamento.
            let v = match ag {
                Agregador::Soma => a.soma_i,
                Agregador::Media if a.n > 0 => a.soma_i / a.n as i128,
                Agregador::Media => 0,
                Agregador::Minimo => a.min_i.unwrap_or(0),
                Agregador::Maximo => a.max_i.unwrap_or(0),
                _ => 0,
            };
            decimal_para_texto(v, escala)
        }
        _ => {
            let v = match ag {
                Agregador::Soma => a.soma_f,
                Agregador::Media if a.n > 0 => a.soma_f / a.n as f64,
                Agregador::Media => 0.0,
                Agregador::Minimo => a.min_f.unwrap_or(0.0),
                Agregador::Maximo => a.max_f.unwrap_or(0.0),
                _ => 0.0,
            };
            if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{}", v as i64)
            } else {
                format!("{v:.4}")
            }
        }
    }
}

/// De onde o valor de um campo vem: da linha, ou da tabela de consulta.
fn valor_bruto(c: &Campo, linha: &[Value], esquema: &Schema, juncoes: &[Juncao]) -> Option<Value> {
    match c.juncao {
        None => linha.get(c.coluna).cloned(),
        Some(j) => {
            let ju = juncoes.get(j)?;
            let chave = rotulo_cru(linha.get(ju.coluna_local)?);
            ju.mapa.get(&chave)?.get(c.coluna).cloned()
        }
    }
    .or(Some(Value::Null))
    .map(|v| {
        let _ = esquema;
        v
    })
}

fn valor_do_campo(c: &Campo, linha: &[Value], esquema: &Schema, juncoes: &[Juncao]) -> String {
    let v = valor_bruto(c, linha, esquema, juncoes).unwrap_or(Value::Null);
    c.granularidade
        .aplicar(&v, escala_do_campo(c, esquema, juncoes))
}

fn tipo_do_campo<'a>(
    c: &Campo,
    esquema: &'a Schema,
    juncoes: &'a [Juncao],
) -> Option<&'a phxsql_core::types::ColumnType> {
    let e = match c.juncao {
        None => esquema,
        Some(j) => &juncoes.get(j)?.esquema,
    };
    e.colunas().get(c.coluna).map(|x| &x.ty)
}

fn escala_do_campo(c: &Campo, esquema: &Schema, juncoes: &[Juncao]) -> u8 {
    match tipo_do_campo(c, esquema, juncoes) {
        Some(phxsql_core::types::ColumnType::Decimal { escala, .. }) => *escala,
        _ => 0,
    }
}

fn e_decimal(c: &Campo, esquema: &Schema, juncoes: &[Juncao]) -> bool {
    matches!(
        tipo_do_campo(c, esquema, juncoes),
        Some(phxsql_core::types::ColumnType::Decimal { .. })
    )
}

/// De onde as linhas de fato vem.
///
/// E um traço e nao um `Vec` para que a varredura nao precise materializar a
/// tabela inteira antes de comecar a somar -- num pivot de milhoes de linhas
/// isso seria o dobro da memoria, para nada.
pub trait Iterador {
    fn proxima(&mut self) -> Result<Option<Vec<Value>>>;
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::schema::Column;
    use phxsql_core::types::ColumnType;

    struct Lista(std::vec::IntoIter<Vec<Value>>);
    impl Iterador for Lista {
        fn proxima(&mut self) -> Result<Option<Vec<Value>>> {
            Ok(self.0.next())
        }
    }

    fn esquema_vendas() -> Schema {
        Schema::new(
            "vendas",
            vec![
                Column::new("cidade", ColumnType::Str(20)),
                Column::new("quando", ColumnType::Date),
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

    fn dia(a: i32, m: u32, d: u32) -> Value {
        Value::Date(phxsql_core::datahora::dias_de_civil(a, m, d))
    }

    fn campo(i: usize, g: Granularidade) -> Campo {
        Campo {
            qualificado: format!("c{i}"),
            juncao: None,
            coluna: i,
            granularidade: g,
        }
    }

    fn vendas() -> Vec<Vec<Value>> {
        vec![
            vec![
                Value::Str("Blumenau".into()),
                dia(2026, 1, 10),
                Value::Decimal(150_000),
            ],
            vec![
                Value::Str("Blumenau".into()),
                dia(2026, 2, 3),
                Value::Decimal(89_000),
            ],
            vec![
                Value::Str("Itajai".into()),
                dia(2026, 1, 20),
                Value::Decimal(4_500),
            ],
            vec![
                Value::Str("Itajai".into()),
                dia(2026, 4, 1),
                Value::Decimal(32_000),
            ],
            vec![
                Value::Str("Blumenau".into()),
                dia(2026, 4, 15),
                Value::Decimal(270_000),
            ],
        ]
    }

    fn cruza(ag: Agregador, gran: Granularidade) -> Resultado {
        let e = esquema_vendas();
        let mut it = Lista(vendas().into_iter());
        cruzar(
            &mut it,
            &e,
            &[],
            &[campo(0, Granularidade::Exato)],
            &[campo(1, gran)],
            Some(&campo(2, Granularidade::Exato)),
            ag,
            1_000,
        )
        .unwrap()
    }

    /// A prova que importa: dinheiro nao perde centavo no caminho.
    #[test]
    fn soma_de_decimal_e_exata() {
        let r = cruza(Agregador::Soma, Granularidade::Ano);
        assert_eq!(r.rotulos_linha, vec!["Blumenau", "Itajai"]);
        assert_eq!(r.rotulos_coluna, vec!["2026"]);
        assert_eq!(r.celulas[0][0].as_deref(), Some("5090.00"));
        assert_eq!(r.celulas[1][0].as_deref(), Some("365.00"));
        assert_eq!(r.total.as_deref(), Some("5455.00"));
        assert_eq!(r.total_linha[0].as_deref(), Some("5090.00"));
        assert_eq!(r.lidas, 5);
    }

    #[test]
    fn os_totais_fecham_nas_duas_direcoes() {
        let r = cruza(Agregador::Soma, Granularidade::Trimestre);
        // Soma das celulas de cada linha bate com o total daquela linha, e o
        // total geral bate com a soma dos totais de coluna. Se um pivot nao
        // fecha, ele nao serve para nada.
        let n = |s: &Option<String>| s.as_deref().unwrap_or("0").parse::<f64>().unwrap();
        for (i, linha) in r.celulas.iter().enumerate() {
            let soma: f64 = linha.iter().map(n).sum();
            assert!(
                (soma - n(&r.total_linha[i])).abs() < 0.005,
                "linha {i}: {soma} != {:?}",
                r.total_linha[i]
            );
        }
        let soma_col: f64 = r.total_coluna.iter().map(n).sum();
        assert!((soma_col - n(&r.total)).abs() < 0.005);
    }

    #[test]
    fn trimestre_agrupa_os_meses_certos() {
        let r = cruza(Agregador::Soma, Granularidade::Trimestre);
        // jan e fev sao o T1; abril e o T2.
        assert_eq!(r.rotulos_coluna, vec!["2026-T1", "2026-T2"]);
        assert_eq!(
            r.celulas[0][0].as_deref(),
            Some("2390.00"),
            "Blumenau no T1"
        );
        assert_eq!(
            r.celulas[0][1].as_deref(),
            Some("2700.00"),
            "Blumenau no T2"
        );
    }

    #[test]
    fn mes_sai_em_ordem_de_calendario() {
        let r = cruza(Agregador::Soma, Granularidade::Mes);
        // O rotulo `2026-01` ordena antes de `2026-02` como TEXTO, e e por isso
        // que ele tem zero a esquerda.
        assert_eq!(r.rotulos_coluna, vec!["2026-01", "2026-02", "2026-04"]);
    }

    #[test]
    fn celula_sem_linha_nenhuma_fica_vazia() {
        let r = cruza(Agregador::Soma, Granularidade::Mes);
        // Itajai nao vendeu em fevereiro: a celula e vazia, nao zero. Zero
        // seria "vendeu nada"; vazio e "nao houve venda".
        let i = r.rotulos_linha.iter().position(|x| x == "Itajai").unwrap();
        let f = r
            .rotulos_coluna
            .iter()
            .position(|x| x == "2026-02")
            .unwrap();
        assert_eq!(r.celulas[i][f], None);
    }

    #[test]
    fn contagem_nao_precisa_de_coluna_de_valor() {
        let e = esquema_vendas();
        let mut it = Lista(vendas().into_iter());
        let r = cruzar(
            &mut it,
            &e,
            &[],
            &[campo(0, Granularidade::Exato)],
            &[],
            None,
            Agregador::Contagem,
            1_000,
        )
        .unwrap();
        assert_eq!(r.rotulos_coluna, vec!["total"]);
        assert_eq!(r.celulas[0][0].as_deref(), Some("3"), "Blumenau tem 3");
        assert_eq!(r.celulas[1][0].as_deref(), Some("2"), "Itajai tem 2");
        assert_eq!(r.total.as_deref(), Some("5"));
    }

    #[test]
    fn media_divide_uma_vez_no_fim() {
        let r = cruza(Agregador::Media, Granularidade::Ano);
        // Blumenau: (1500 + 890 + 2700) / 3 = 1696.666… -> 1696.66 no dominio
        // escalado, que e o truncamento de centavo, nao um float aproximado.
        assert_eq!(r.celulas[0][0].as_deref(), Some("1696.66"));
    }

    #[test]
    fn minimo_e_maximo_saem_do_dominio_certo() {
        let r = cruza(Agregador::Minimo, Granularidade::Ano);
        assert_eq!(r.celulas[0][0].as_deref(), Some("890.00"));
        let r = cruza(Agregador::Maximo, Granularidade::Ano);
        assert_eq!(r.celulas[0][0].as_deref(), Some("2700.00"));
    }

    #[test]
    fn nulo_nao_entra_na_conta() {
        let e = esquema_vendas();
        let mut linhas = vendas();
        linhas.push(vec![
            Value::Str("Blumenau".into()),
            dia(2026, 1, 5),
            Value::Null,
        ]);
        let mut it = Lista(linhas.into_iter());
        let r = cruzar(
            &mut it,
            &e,
            &[],
            &[campo(0, Granularidade::Exato)],
            &[],
            Some(&campo(2, Granularidade::Exato)),
            Agregador::Soma,
            1_000,
        )
        .unwrap();
        assert_eq!(r.lidas, 6, "leu as seis");
        assert_eq!(r.consideradas, 5, "mas so cinco tinham valor");
        // Somar o nulo como zero nao mudaria a soma, mas mudaria a media.
        assert_eq!(r.total.as_deref(), Some("5455.00"));
    }

    #[test]
    fn distintos_conta_valores_diferentes() {
        let e = esquema_vendas();
        let mut it = Lista(vendas().into_iter());
        let r = cruzar(
            &mut it,
            &e,
            &[],
            &[campo(0, Granularidade::Exato)],
            &[],
            Some(&campo(1, Granularidade::Exato)),
            Agregador::ContagemDistinta,
            1_000,
        )
        .unwrap();
        assert_eq!(r.celulas[0][0].as_deref(), Some("3"), "3 datas em Blumenau");
    }

    #[test]
    fn o_teto_de_leitura_para_a_varredura() {
        let e = esquema_vendas();
        let mut it = Lista(vendas().into_iter());
        let r = cruzar(
            &mut it,
            &e,
            &[],
            &[campo(0, Granularidade::Exato)],
            &[],
            None,
            Agregador::Contagem,
            2,
        )
        .unwrap();
        assert_eq!(r.consideradas, 2, "parou no teto");
    }

    #[test]
    fn agregador_e_granularidade_recusam_o_que_nao_conhecem() {
        assert!(Agregador::de_texto("mediana").is_err());
        assert!(Granularidade::de_texto("quinzena").is_err());
        assert_eq!(Agregador::de_texto("SUM").unwrap(), Agregador::Soma);
        assert_eq!(Granularidade::de_texto("Mês").unwrap(), Granularidade::Mes);
    }
}
