//! Nucleo de distancia vetorial -- zero dependencia, so `std`.
//!
//! Tres metricas sobre `&[f32]` (o formato cru de um embedding): produto
//! interno, cosseno e euclidiana. Sem SIMD, sem BLAS, sem crate de algebra --
//! laco escalar, como manda a petrea de zero dependencias externas. E o
//! mesmo metodo do SHA-256 e do PBKDF2 desta casa: a formula e conhecida, o
//! codigo e nosso, a prova e contra valor de referencia (aqui, calculado a
//! mao/independente, nao citado).
//!
//! # So V1 -- nucleo, nao motor
//!
//! Este modulo e SO a funcao pura. Nao ha `ColumnType::Vetor`, nao ha
//! arquivo `.vec`, nada mudou em `FORMATO.md`. A bancada que mede se a
//! forca-bruta com este nucleo basta para K-NN, ou se precisa de indice ANN,
//! e o `--example custo-do-vizinho` em `phxsql-store` (frente V2). Ver
//! `docs/BACKLOG-TIPOS.md` (Frente V) e `docs/propostas/vetorial.md`.
//!
//! # Por que acumular em `f64`
//!
//! A entrada e a saida sao `f32` (o tamanho de um embedding real: 1536
//! dimensoes em f32 sao 6 KiB, o dobro em f64). Mas somar `1536` produtos
//! direto em `f32` acumula erro de arredondamento a cada soma; a mesma conta
//! em `f64` e depois convertida de volta para `f32` no fim custa uma
//! conversao por elemento e devolve um resultado que bate com o calculado a
//! mao ate a ultima casa que o `f32` consegue representar. Nao muda nada em
//! disco -- e so a largura do acumulador dentro do laco.
//!
//! # Por que panica em vez de devolver `Result`
//!
//! Comparar dois vetores de dimensao diferente nao e uma condicao do DADO,
//! e' um erro de quem chama: um `VECTOR<F32,N>` (frente V3, ainda nao
//! construida) tem a dimensao fixa na propria coluna, e o motor nunca havera
//! de passar aqui dois vetores de tamanho diferente vindos da mesma coluna.
//! `assert_eq!` documenta essa pre-condicao e falha alto, do jeito que
//! `<[T]>::copy_from_slice` do padrao ja faz. A alternativa -- truncar pelo
//! menor, como um `zip` faria em silencio -- devolveria um numero ERRADO sem
//! avisar ninguem, e essa e exatamente a categoria de defeito que este
//! modulo existe para nao ter.

/// Produto interno (`a . b`), somado em `f64` e devolvido em `f32`.
///
/// # Panics
///
/// Se `a.len() != b.len()`.
pub fn produto_interno(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "produto_interno: dimensoes diferentes ({} x {})",
        a.len(),
        b.len()
    );
    let mut soma = 0f64;
    for i in 0..a.len() {
        soma += a[i] as f64 * b[i] as f64;
    }
    soma as f32
}

/// Norma euclidiana (`||a||`) de um vetor.
///
/// Vetor vazio ou nulo (todos os componentes zero) tem norma `0.0`.
pub fn norma(a: &[f32]) -> f32 {
    let mut soma = 0f64;
    for &x in a {
        soma += x as f64 * x as f64;
    }
    soma.sqrt() as f32
}

/// Similaridade por cosseno: `(a . b) / (||a|| * ||b||)`, em `[-1.0, 1.0]`.
///
/// # Vetor nulo
///
/// Um vetor de norma zero nao tem direcao, entao o cosseno com ele e
/// indefinido (divisao por zero). A convencao desta casa: devolve `0.0` --
/// "sem similaridade" -- em vez de propagar `NaN`. Isso e o que o BUG
/// seguinte esquece de tratar (ver os testes): calcular a similaridade
/// exigiria dividir pelas duas normas, e um bug que esquece de dividir por
/// uma delas (ou pelas duas) devolve o produto interno cru, que so bate por
/// coincidencia quando os dois vetores ja sao unitarios.
///
/// # Panics
///
/// Se `a.len() != b.len()`.
pub fn similaridade_cosseno(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "similaridade_cosseno: dimensoes diferentes ({} x {})",
        a.len(),
        b.len()
    );
    let na = norma(a);
    let nb = norma(b);
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    produto_interno(a, b) / (na * nb)
}

/// Distancia por cosseno: `1.0 - similaridade_cosseno(a, b)`.
///
/// Fica em `[0.0, 2.0]`: `0.0` para vetores na mesma direcao, `1.0` para
/// ortogonais, `2.0` para direcoes opostas -- a convencao usual de "menor e
/// mais parecido" que um K-NN por distancia espera (o oposto da
/// similaridade, onde maior e mais parecido).
///
/// # Panics
///
/// Se `a.len() != b.len()`.
pub fn distancia_cosseno(a: &[f32], b: &[f32]) -> f32 {
    1.0 - similaridade_cosseno(a, b)
}

/// Distancia euclidiana (`||a - b||`).
///
/// # Panics
///
/// Se `a.len() != b.len()`.
pub fn distancia_euclidiana(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "distancia_euclidiana: dimensoes diferentes ({} x {})",
        a.len(),
        b.len()
    );
    let mut soma = 0f64;
    for i in 0..a.len() {
        let d = a[i] as f64 - b[i] as f64;
        soma += d * d;
    }
    soma.sqrt() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Todo valor de referencia abaixo saiu de um script Python independente
    /// (soma em f64, arredondada para f32 no fim -- a mesma disciplina do
    /// codigo acima), nunca de conta de cabeca. Reproduzir:
    ///
    /// ```python
    /// import struct, math
    /// def f32(x): return struct.unpack('f', struct.pack('f', x))[0]
    /// ```
    fn quase_igual(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn produto_interno_valor_de_referencia() {
        // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32, exato (sem parte fracionaria).
        assert_eq!(produto_interno(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]), 32.0);
        assert_eq!(
            produto_interno(&[1.0, 2.0, 3.0, 4.0], &[5.0, 6.0, 7.0, 8.0]),
            70.0
        );
    }

    #[test]
    fn norma_valor_de_referencia() {
        // Triangulo 3-4-5: norma de (3,4) e exatamente 5.
        assert_eq!(norma(&[3.0, 4.0]), 5.0);
        // sqrt(1+4+9) = sqrt(14), calculado pelo Python acima.
        assert!(quase_igual(norma(&[1.0, 2.0, 3.0]), 3.741_657_5));
        assert_eq!(norma(&[0.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn euclidiana_valor_de_referencia() {
        // diferenca (3,4,0) -> sqrt(9+16) = 5, exato.
        assert_eq!(
            distancia_euclidiana(&[1.0, 2.0, 3.0], &[4.0, 6.0, 3.0]),
            5.0
        );
        assert_eq!(
            distancia_euclidiana(&[0.0, 0.0, 0.0], &[3.0, 4.0, 0.0]),
            5.0
        );
        // diferenca (4,4,4,4) -> sqrt(64) = 8, exato.
        assert_eq!(
            distancia_euclidiana(&[1.0, 2.0, 3.0, 4.0], &[5.0, 6.0, 7.0, 8.0]),
            8.0
        );
    }

    #[test]
    fn cosseno_valor_de_referencia() {
        // Ortogonais: similaridade 0, distancia 1.
        assert_eq!(similaridade_cosseno(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert_eq!(distancia_cosseno(&[1.0, 0.0], &[0.0, 1.0]), 1.0);
        // Mesma direcao: similaridade 1, distancia 0.
        assert_eq!(similaridade_cosseno(&[1.0, 0.0], &[1.0, 0.0]), 1.0);
        assert_eq!(distancia_cosseno(&[1.0, 0.0], &[1.0, 0.0]), 0.0);
        // Direcoes opostas: similaridade -1, distancia 2.
        assert_eq!(similaridade_cosseno(&[1.0, 0.0], &[-1.0, 0.0]), -1.0);
        assert_eq!(distancia_cosseno(&[1.0, 0.0], &[-1.0, 0.0]), 2.0);
        // (1,1) x (1,0): angulo de 45 graus, cos = 1/sqrt(2).
        assert!(quase_igual(
            similaridade_cosseno(&[1.0, 1.0], &[1.0, 0.0]),
            0.707_106_77
        ));
        assert!(quase_igual(
            distancia_cosseno(&[1.0, 1.0], &[1.0, 0.0]),
            0.292_893_23
        ));
        // Vetor de 4 dimensoes, nao unitario dos dois lados.
        assert!(quase_igual(
            similaridade_cosseno(&[1.0, 2.0, 3.0, 4.0], &[5.0, 6.0, 7.0, 8.0]),
            0.968_863_9
        ));
    }

    #[test]
    fn vetor_nulo_nao_produz_nan() {
        assert_eq!(
            similaridade_cosseno(&[0.0, 0.0, 0.0], &[1.0, 2.0, 3.0]),
            0.0
        );
        assert_eq!(distancia_cosseno(&[0.0, 0.0, 0.0], &[1.0, 2.0, 3.0]), 1.0);
        assert_eq!(similaridade_cosseno(&[0.0, 0.0], &[0.0, 0.0]), 0.0);
    }

    #[test]
    #[should_panic(expected = "dimensoes diferentes")]
    fn dimensao_diferente_no_produto_interno_panica() {
        produto_interno(&[1.0, 2.0], &[1.0, 2.0, 3.0]);
    }

    #[test]
    #[should_panic(expected = "dimensoes diferentes")]
    fn dimensao_diferente_na_euclidiana_panica() {
        distancia_euclidiana(&[1.0, 2.0], &[1.0, 2.0, 3.0]);
    }

    #[test]
    #[should_panic(expected = "dimensoes diferentes")]
    fn dimensao_diferente_no_cosseno_panica() {
        similaridade_cosseno(&[1.0, 2.0], &[1.0, 2.0, 3.0]);
    }
}
