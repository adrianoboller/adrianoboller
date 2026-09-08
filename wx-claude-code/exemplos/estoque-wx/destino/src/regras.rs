//! Regras de negocio puras, convertidas do PDF de codigo do ESTOQUE (WINDEV 2025).
//! Cada funcao cita `estoque-codigo.pdf` pagina N (sha256 71175b76...), a origem da regra.
//! Dinheiro em centavos inteiros (i64): `currency` do WLanguage e ponto fixo, e f64 nao e.

/// Round() do WLanguage e "metade para longe do zero"; o round() do Rust tambem. Mantem-se.
fn arredonda_centavos(valor: f64) -> i64 {
    (valor * 100.0).round() as i64
}

fn centavos(valor: f64) -> i64 {
    arredonda_centavos(valor)
}

pub fn reais(centavos: i64) -> f64 {
    centavos as f64 / 100.0
}

/// BR-001 — CalculaDesconto, estoque-codigo.pdf p.1.
/// Cliente comum (C) ate 15 %, especial (E) ate 25 %. Acima do teto o legado devolve -1
/// (e mostra Error); aqui devolve Err com a MESMA mensagem, e quem chama traduz para -1 no golden.
pub fn calcula_desconto(subtotal: f64, percentual: f64, tipo_cliente: &str) -> Result<f64, String> {
    let teto = if tipo_cliente == "E" { 25.0 } else { 15.0 };
    if percentual > teto {
        return Err(format!("Desconto de {percentual}% ultrapassa o máximo de {teto}% para este cliente."));
    }
    // Round(nSubtotal * nPercentual / 100, 2)
    Ok(reais(arredonda_centavos(subtotal * percentual / 100.0)))
}

/// Dias desde 1970-01-01 (algoritmo civil), para DateDifference sem crate de data.
fn dias_civis(data: &str) -> Result<i64, String> {
    let p: Vec<&str> = data.split('-').collect();
    if p.len() != 3 {
        return Err(format!("data invalida: {data}"));
    }
    let (y, m, d): (i64, i64, i64) = (
        p[0].parse().map_err(|_| format!("data invalida: {data}"))?,
        p[1].parse().map_err(|_| format!("data invalida: {data}"))?,
        p[2].parse().map_err(|_| format!("data invalida: {data}"))?,
    );
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146097 + doe - 719468)
}

/// BR-003 — CalculaJurosAtraso, estoque-codigo.pdf p.2.
/// 2 % ao mes pro rata die a partir do dia seguinte ao vencimento; no vencimento, zero.
/// Round(nValor * 0.02 / 30 * nDias, 2), feito em inteiros: valor_cent * 2 * dias / 3000.
pub fn calcula_juros_atraso(valor: f64, vencimento: &str, pagamento: &str) -> Result<f64, String> {
    let dias = dias_civis(pagamento)? - dias_civis(vencimento)?;
    if dias <= 0 {
        return Ok(0.0);
    }
    let num = centavos(valor) * 2 * dias;
    let den = 3000;
    Ok(reais((2 * num + den) / (2 * den)))
}

/// BR-004 — GeraTitulos, estoque-codigo.pdf p.4 (so o calculo; a gravacao esta em repo.rs).
/// Parcelas iguais = Round(total / n, 2); a diferenca de arredondamento vai para a ultima.
pub fn valores_das_parcelas(total: f64, parcelas: u32) -> Result<Vec<f64>, String> {
    if parcelas == 0 {
        return Err("parcelas deve ser maior que zero".into());
    }
    let total_c = centavos(total);
    let parcela_c = ((total_c as f64) / parcelas as f64).round() as i64;
    let mut acumulado = 0;
    let mut saida = Vec::with_capacity(parcelas as usize);
    for i in 1..=parcelas {
        if i == parcelas {
            saida.push(reais(total_c - acumulado));
        } else {
            saida.push(reais(parcela_c));
            acumulado += parcela_c;
        }
    }
    Ok(saida)
}

/// BR-005 — ValidaCPF, estoque-codigo.pdf p.2. Dois digitos verificadores; sequencia repetida e invalida.
pub fn valida_cpf(cpf: &str) -> bool {
    let d: Vec<u32> = cpf.chars().filter(|c| c.is_ascii_digit()).map(|c| c.to_digit(10).unwrap()).collect();
    // o legado tira so '.' e '-' e espacos; qualquer outra coisa deixa Length <> 11
    let limpo: String = cpf.replace(['.', '-', ' '], "");
    if limpo.len() != 11 || d.len() != 11 {
        return false;
    }
    if d.iter().all(|&x| x == d[0]) {
        return false;
    }
    let dv = |n: usize, peso0: u32| -> u32 {
        let soma: u32 = d.iter().take(n).enumerate().map(|(i, &x)| x * (peso0 - i as u32)).sum();
        let r = (soma * 10) % 11;
        if r == 10 { 0 } else { r }
    };
    dv(9, 10) == d[9] && dv(10, 11) == d[10]
}

#[cfg(test)]
mod testes {
    use super::*;
    // Os casos sao os do golden master (inputs/dados-de-amostra/resultados-esperados.json),
    // capturados a mao do legado; o golden.py comparar e a prova oficial, este e o atalho do cargo test.
    #[test]
    fn br_001_teto_por_tipo() {
        assert_eq!(calcula_desconto(2493.80, 10.0, "C").unwrap(), 249.38);
        assert!(calcula_desconto(2493.80, 20.0, "C").is_err());
        assert_eq!(calcula_desconto(1000.0, 25.0, "E").unwrap(), 250.0);
    }
    #[test]
    fn br_003_pro_rata_die() {
        assert_eq!(calcula_juros_atraso(748.14, "2026-08-01", "2026-08-16").unwrap(), 7.48);
        assert_eq!(calcula_juros_atraso(748.14, "2026-08-01", "2026-08-01").unwrap(), 0.0);
    }
    #[test]
    fn br_004_diferenca_na_ultima() {
        assert_eq!(valores_das_parcelas(2244.42, 3).unwrap(), vec![748.14, 748.14, 748.14]);
        assert_eq!(valores_das_parcelas(100.0, 3).unwrap(), vec![33.33, 33.33, 33.34]);
    }
    #[test]
    fn br_005_cpf() {
        assert!(valida_cpf("529.982.247-25"));
        assert!(!valida_cpf("111.111.111-11"));
        assert!(!valida_cpf("529.982.247-2"));
    }
}
