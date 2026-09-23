//! O que e do NO (do processo) e nao de uma tabela.
//!
//! Duas coisas moram aqui, e as duas sao do processo por necessidade e nao
//! por conveniencia:
//!
//! * o **contador de criacao** ([`COLUNA_ROWSTAMP`]), que tem de ser unico
//!   entre TABELAS -- pai e filha estao em tabelas diferentes por definicao,
//!   porque a chave estrangeira e declarada na filha. Um contador dentro do
//!   `Table` nao cumpriria a ordem: dois `Table` abertos emitiriam carimbos
//!   independentes, e o «pai antes do filho» morreria no caso mais comum;
//! * o **inicio da faixa da `Sequence`**, que e a identidade deste servidor.
//!   Ele nao pode morar no esquema: uma tabela nascida por replicacao e criada
//!   do MESMO bloco de esquema do source, byte a byte, entao o que viesse
//!   gravado ali chegaria igual nos dois nos -- e o que e igual nos dois nos
//!   nao pode ser o que os distingue.
//!
//! # Por que atomico, e nao um campo com a trava global de fora
//!
//! O servidor serializa escrita na trava global de dados, e sob ela dois
//! commits nunca se cruzam. Mas o `phxsql-store` tambem e usado direto pela
//! CLI, pela FFI, pelos `examples` e pelos testes, **sem** aquela trava.
//! Premissa de trava alheia e a que se perde numa divisao de arquivo -- e aqui
//! a perda seria dois carimbos iguais, que e exatamente o que a coluna existe
//! para impedir.
//!
//! [`COLUNA_ROWSTAMP`]: phxsql_core::schema::COLUNA_ROWSTAMP

use std::sync::atomic::{AtomicU64, Ordering};

/// Ultimo carimbo de criacao emitido por este processo. 0 = nenhum ainda.
static ULTIMO_CARIMBO: AtomicU64 = AtomicU64::new(0);

/// Inicio da faixa da `Sequence` deste no. Ver [`definir_inicio_da_sequencia`].
static INICIO_DA_SEQUENCIA: AtomicU64 = AtomicU64::new(0);

/// O proximo carimbo de criacao. Estritamente maior que todos os anteriores.
///
/// Nunca devolve 0: zero e reservado para «esta linha nasceu antes de a coluna
/// existir», e uma linha nova nunca pode ser confundida com uma dessas.
pub fn proximo_carimbo() -> u64 {
    ULTIMO_CARIMBO.fetch_add(1, Ordering::SeqCst) + 1
}

/// Empurra o contador para pelo menos `ate`. Nunca o faz recuar.
///
/// Chamada em dois lugares, e os dois sao obrigatorios:
///
/// * ao ABRIR uma tabela, com a marca d'agua que veio do `.reg` -- senao um no
///   que reinicie emitiria carimbo menor que o de linha que ja esta gravada, e
///   a garantia morreria calada;
/// * ao aplicar um evento de REPLICACAO, com o carimbo que veio na imagem --
///   senao a proxima escrita local sairia atras de uma linha que ja esta la.
pub fn empurrar_carimbo(ate: u64) {
    ULTIMO_CARIMBO.fetch_max(ate, Ordering::SeqCst);
}

/// Ultimo carimbo emitido por este processo. So para medir e para teste.
pub fn ultimo_carimbo() -> u64 {
    ULTIMO_CARIMBO.load(Ordering::SeqCst)
}

/// Declara em que faixa este no numera a `Sequence`.
///
/// O par e `(inicio, passo)`: o `passo` vem do `PSCH` de cada tabela e o
/// `inicio` vem daqui. Com `passo = 3` e tres nos, um diz `inicio = 0`, outro
/// `1` e o terceiro `2`, e nenhum numero nasce duas vezes.
///
/// Zero e o padrao e quer dizer «primeira faixa», que com `passo = 1` -- o
/// padrao das tabelas -- e exatamente o comportamento de sempre: 1, 2, 3...
///
/// # Por que um numero declarado, e nao derivado do nome do servidor
///
/// Porque derivar de hash colide: com `passo = 2` e dois nos,
/// `hash(nome) mod 2` da o mesmo valor em metade dos pares -- e colidir aqui e
/// literalmente o defeito que a faixa existe para consertar, agora produzido
/// pelo conserto e em silencio. Aniversario nao particiona espaco de chave.
pub fn definir_inicio_da_sequencia(inicio: u64) {
    INICIO_DA_SEQUENCIA.store(inicio, Ordering::SeqCst);
}

/// Em que faixa este no numera. Ver [`definir_inicio_da_sequencia`].
pub fn inicio_da_sequencia() -> u64 {
    INICIO_DA_SEQUENCIA.load(Ordering::SeqCst)
}

/// O primeiro numero `>= piso` que cai na faixa `(inicio, passo)`.
///
/// Funcao pura, e por isso testavel sem tocar disco nem estado de processo.
/// Com `passo = 1` devolve o proprio `piso`: a tabela sem faixa continua
/// numerando 1, 2, 3... exatamente como sempre numerou.
pub fn na_faixa(piso: u64, inicio: u64, passo: u64) -> u64 {
    if passo <= 1 {
        return piso;
    }
    let alvo = inicio % passo;
    let resto = piso % passo;
    if resto == alvo {
        piso
    } else {
        piso + (alvo + passo - resto) % passo
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sem_faixa_o_piso_e_o_proprio_numero() {
        // A guarda do comportamento VELHO, que e a que mais importa: toda
        // tabela gravada antes da v10 tem passo 1.
        for piso in [1u64, 2, 7, 1_000_000] {
            assert_eq!(na_faixa(piso, 0, 1), piso);
        }
    }

    #[test]
    fn a_faixa_arredonda_para_cima_dentro_da_propria_classe() {
        // passo 3, no 1: 1, 4, 7, 10...
        assert_eq!(na_faixa(1, 1, 3), 1);
        assert_eq!(na_faixa(2, 1, 3), 4);
        assert_eq!(na_faixa(3, 1, 3), 4);
        assert_eq!(na_faixa(4, 1, 3), 4);
        assert_eq!(na_faixa(5, 1, 3), 7);
        // passo 3, no 0: 3, 6, 9... (o 0 nao se entrega, o piso e 1)
        assert_eq!(na_faixa(1, 0, 3), 3);
        assert_eq!(na_faixa(3, 0, 3), 3);
        assert_eq!(na_faixa(4, 0, 3), 6);
    }

    #[test]
    fn duas_faixas_nunca_entregam_o_mesmo_numero() {
        // A prova do que a faixa existe para dar: percorre os mil primeiros
        // numeros de cada no e conta interseccao.
        let mut a = 1u64;
        let mut b = 1u64;
        let (mut sa, mut sb) = (Vec::new(), Vec::new());
        for _ in 0..1000 {
            a = na_faixa(a, 0, 2);
            sa.push(a);
            a += 1;
            b = na_faixa(b, 1, 2);
            sb.push(b);
            b += 1;
        }
        assert!(
            !sa.iter().any(|n| sb.contains(n)),
            "as duas faixas entregaram o mesmo numero"
        );
    }

    #[test]
    fn o_carimbo_nunca_empata_nem_recua() {
        let anterior = proximo_carimbo();
        let depois = proximo_carimbo();
        assert!(depois > anterior, "{depois} nao e maior que {anterior}");
        empurrar_carimbo(anterior);
        assert!(
            proximo_carimbo() > depois,
            "empurrar para tras fez o contador recuar"
        );
    }
}
