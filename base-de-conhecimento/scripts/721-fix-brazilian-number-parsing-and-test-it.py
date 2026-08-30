# Fix Brazilian number parsing and test it
# 28/08 19:25

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        ColumnType::Real4 | ColumnType::Real8 => {
            // Virgula decimal tambem entra: e o que sai de uma planilha em
            // portugues, e recusar obrigaria a editar o arquivo antes de colar.
            let limpo = t.replace(',', ".");
            Value::Real(limpo.parse::<f64>().map_err(|_| erro("numero"))?)
        }
        ColumnType::Decimal { escala, .. } => {
            Value::Decimal(texto_para_decimal(&t.replace(',', "."), *escala)?)
        }'''
novo='''        ColumnType::Real4 | ColumnType::Real8 => {
            Value::Real(numero_pt(t).parse::<f64>().map_err(|_| erro("numero"))?)
        }
        ColumnType::Decimal { escala, .. } => {
            Value::Decimal(texto_para_decimal(&numero_pt(t), *escala)?)
        }'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''/// Uma linha inteira vinda de formato de texto. Ver [`json_para_valor_de_texto`].'''
novo2='''/// Normaliza numero escrito à brasileira para a forma que o analisador come.
///
/// # A regra, e o defeito que ela conserta
///
/// Trocar toda virgula por ponto parece obvio e esta errado: `2.000,00` viraria
/// `2.000.00`, que nao e numero nenhum. Aqui o **ultimo separador manda** --
/// ele e o decimal, e o outro e milhar e sai fora.
///
/// | entra | sai |
/// |---|---|
/// | `1500,50` | `1500.50` |
/// | `1.500,50` | `1500.50` |
/// | `1,500.50` | `1500.50` |
/// | `1500.50` | `1500.50` |
/// | `1.500` | `1.500` — ambiguo, e fica como esta |
///
/// A ultima linha e a decisao dificil: `1.500` pode ser mil e quinhentos ou um
/// e meio, e nao ha como saber. Fica como veio, e o tipo da coluna decide --
/// num `Decimal(15,2)` vira 1,50, que e o que o analisador ja fazia antes desta
/// funcao existir. Adivinhar mudaria o valor de quem digitou certo.
fn numero_pt(t: &str) -> String {
    let ponto = t.rfind('.');
    let virgula = t.rfind(',');
    match (ponto, virgula) {
        // Os dois presentes: o ultimo e o decimal, o outro e milhar.
        (Some(p), Some(v)) if v > p => t.replace('.', "").replace(',', "."),
        (Some(p), Some(v)) if p > v => t.replace(',', ""),
        // So virgula: ela e o decimal.
        (None, Some(_)) => t.replace(',', "."),
        // So ponto, ou nenhum: fica como esta.
        _ => t.to_string(),
    }
}

/// Uma linha inteira vinda de formato de texto. Ver [`json_para_valor_de_texto`].'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
