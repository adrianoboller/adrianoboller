//! As regras do cadastro, puras: sem banco, sem HTTP. Cada uma cita a origem
//! no legado (inputs/legado-php/clientes.php) e tem teste unitario com os
//! mesmos valores do golden master.

/// BR-001 — origem: clientes.php#6-8 `normalizar_email`: minusculas, sem espacos nas pontas.
pub fn normalizar_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// BR-002 — origem: clientes.php#11-15 `normalizar_limite`: `round(x, 2)` do PHP
/// arredonda MEIO PARA CIMA (1500.005 -> 1500.01), nao "banker's". Negativo e erro.
/// Trabalha em centavos inteiros para nao herdar o ruido binario do f64.
pub fn normalizar_limite(valor: &str) -> Result<i64, String> {
    let v: f64 = valor.trim().parse().map_err(|_| "limite invalido".to_string())?;
    // PHP round() usa "pre-rounding" para tirar o ruido do binario: 1500.005 vira
    // 1500.01 la, e um round() ingenuo em f64 daria 1500.00 aqui. Replicar o
    // pre-rounding com 15 digitos significativos e o que faz os dois baterem.
    let pre = format!("{:.15e}", v).parse::<f64>().unwrap_or(v);
    let centavos = (pre * 100.0).abs().round() as i64;
    let centavos = if pre < 0.0 { -centavos } else { centavos };
    if centavos < 0 {
        return Err("limite de credito nao pode ser negativo".into());
    }
    Ok(centavos)
}

/// BR-003 — origem: clientes.php#18-22 `normalizar_nome`: colapsa espacos, 3..80 BYTES
/// (o legado usa strlen, que conta bytes; "Maria da Silva" tem 14).
pub fn normalizar_nome(nome: &str) -> Result<String, String> {
    let n = nome.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.len() < 3 || n.len() > 80 {
        return Err("nome deve ter entre 3 e 80 caracteres".into());
    }
    Ok(n)
}

/// Como o legado imprime o limite: `number_format(x, 2, '.', '')`.
pub fn limite_como_texto(centavos: i64) -> String {
    format!("{}.{:02}", centavos / 100, (centavos % 100).abs())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn br_001_email_minusculo_sem_espacos() {
        assert_eq!(normalizar_email("MARIA@Loja.com "), "maria@loja.com");
    }

    #[test]
    fn br_002_meio_para_cima_como_o_php() {
        // os dois valores do golden: 1500.005 -> 1500.01 e 999.994 -> 999.99
        assert_eq!(normalizar_limite("1500.005"), Ok(150001));
        assert_eq!(normalizar_limite("999.994"), Ok(99999));
        assert_eq!(limite_como_texto(150001), "1500.01");
    }

    #[test]
    fn br_002_negativo_e_erro() {
        assert_eq!(normalizar_limite("-5"), Err("limite de credito nao pode ser negativo".into()));
    }

    #[test]
    fn br_003_nome_colapsa_espacos_e_tem_tamanho() {
        assert_eq!(normalizar_nome("  Maria   da  Silva "), Ok("Maria da Silva".into()));
        assert_eq!(normalizar_nome("Jo"), Err("nome deve ter entre 3 e 80 caracteres".into()));
    }
}
