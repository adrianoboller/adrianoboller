//! Divisao de trabalho entre nucleos, sem dependencia externa.
//!
//! Nao e um `rayon` -- e o pedaco de `rayon` que este projeto usa: pegar uma
//! faixa `0..n`, cortar em pedacos e rodar cada pedaco numa thread, com a
//! ordem do resultado preservada.
//!
//! # Onde thread ajuda, e onde nao ajuda
//!
//! Aqui a regra e uma so: **thread paraleliza trabalho independente**. Inserir
//! uma linha nao e trabalho independente -- e uma descida na B+tree em que cada
//! passo depende do anterior, e nenhuma thread acelera isso. Ja varrer um
//! milhao de linhas na memoria aplicando um filtro sao um milhao de perguntas
//! que nao se conhecem: essas dividem.
//!
//! A outra metade da regra e mais dura de aceitar: **thread multiplica o que
//! existe, nao conserta o que esta errado**. Antes de repartir um laco por
//! quatro nucleos, vale conferir se o laco nao esta fazendo trabalho a toa --
//! o CRC deste projeto ficou 4x mais rapido trocando o algoritmo, e quatro
//! nucleos dariam no maximo 4x com muito mais risco.
//!
//! # Determinismo
//!
//! `mapear_faixa` devolve **sempre** o mesmo resultado da versao sequencial,
//! na mesma ordem: cada pedaco junta o seu num vetor proprio e os vetores sao
//! concatenados na ordem dos pedacos. Uma consulta que muda de ordem conforme
//! o numero de nucleos da maquina seria pior do que uma consulta lenta.

use std::num::NonZeroUsize;

/// Quantas threads vale usar. Nunca zero, nunca mais que o tamanho do trabalho.
pub fn nucleos() -> usize {
    std::thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1)
}

/// Piso abaixo do qual paralelizar custa mais do que rende.
///
/// Criar thread e juntar resultado nao sao de graca; para poucas linhas o
/// laco simples ganha. O numero saiu de medicao, nao de gosto.
pub const MINIMO_PARA_DIVIDIR: usize = 50_000;

/// Aplica `f` a cada indice de `0..n` e concatena o que cada um devolveu,
/// **na ordem dos indices**.
///
/// Roda sequencial quando o trabalho e pequeno ou a maquina tem um nucleo so:
/// nesse caso o resultado e literalmente o do laco simples.
pub fn mapear_faixa<T, F>(n: usize, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize, &mut Vec<T>) + Sync,
{
    let threads = nucleos().min(n.div_ceil(MINIMO_PARA_DIVIDIR)).max(1);
    if threads == 1 || n < MINIMO_PARA_DIVIDIR {
        let mut saida = Vec::new();
        for i in 0..n {
            f(i, &mut saida);
        }
        return saida;
    }

    let por_pedaco = n.div_ceil(threads);
    let mut partes: Vec<Vec<T>> = Vec::new();

    // `scope` deixa as threads pegarem emprestado o que esta na pilha sem
    // Arc nem clone: elas terminam antes do fim do bloco, e o compilador
    // garante isso.
    std::thread::scope(|escopo| {
        let mut maos = Vec::with_capacity(threads);
        for k in 0..threads {
            let inicio = k * por_pedaco;
            let fim = ((k + 1) * por_pedaco).min(n);
            let f = &f;
            maos.push(escopo.spawn(move || {
                let mut meu = Vec::new();
                for i in inicio..fim {
                    f(i, &mut meu);
                }
                meu
            }));
        }
        for mao in maos {
            // Uma thread que entrou em panico derruba a consulta inteira, e
            // deve mesmo: resultado parcial de uma varredura e resposta
            // errada, nao resposta incompleta.
            partes.push(mao.join().expect("thread da varredura entrou em panico"));
        }
    });

    let total = partes.iter().map(Vec::len).sum();
    let mut saida = Vec::with_capacity(total);
    for parte in partes {
        saida.extend(parte);
    }
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nucleos_nunca_e_zero() {
        assert!(nucleos() >= 1);
    }

    #[test]
    fn ordem_e_a_mesma_do_laco_simples() {
        // O ponto que faz a funcao servir: a resposta nao pode depender de
        // quantos nucleos a maquina tem.
        for n in [0usize, 1, 7, 999, MINIMO_PARA_DIVIDIR, 250_000] {
            let esperado: Vec<usize> = (0..n).filter(|i| i % 3 == 0).collect();
            let obtido = mapear_faixa(n, |i, saida| {
                if i % 3 == 0 {
                    saida.push(i);
                }
            });
            assert_eq!(obtido, esperado, "divergiu com n = {n}");
        }
    }

    #[test]
    fn trabalho_pequeno_nao_cria_thread() {
        // Nao da para observar a ausencia de thread de fora, mas da para
        // exigir que o resultado esteja certo no caminho sequencial.
        let r = mapear_faixa(10, |i, s| s.push(i * 2));
        assert_eq!(r, vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
    }

    #[test]
    fn cada_indice_e_visitado_uma_vez_so() {
        let n = 300_000;
        let r = mapear_faixa(n, |i, s| s.push(i));
        assert_eq!(r.len(), n);
        assert!(r.windows(2).all(|p| p[0] < p[1]), "saiu fora de ordem");
    }

    #[test]
    fn vazio_devolve_vazio() {
        let r: Vec<usize> = mapear_faixa(0, |i, s| s.push(i));
        assert!(r.is_empty());
    }
}
