//! O que os tres diarios (`.log`, `.trash`, `.reason`) tem em comum.
//!
//! Hoje e uma coisa so: **onde o volume corta**.
//!
//! # O problema que este modulo resolve
//!
//! Os tres cortam volume por bytes, e ate aqui usavam o mesmo
//! `bytes_por_arquivo` do esquema que o `.bin` e o `.memo` usam -- 1 GiB por
//! padrao. Para um anexo isso e um numero razoavel. Para um diario de eventos
//! de 44 bytes, 1 GiB e **24 milhoes de eventos**: na pratica o `.log` de uma
//! tabela de um milhao de linhas nunca fecha o primeiro volume.
//!
//! Isso foi medido, e nao suposto, em `--example quanto-ocupa`: e a razao pela
//! qual compactar volume fechado foi recusado na rodada passada. Nao porque
//! compactar nao funcione, mas porque nao havia volume fechado nenhum.
//!
//! # Por que um global, e nao um campo do esquema
//!
//! Porque o esquema esta gravado dentro de cada `.reg` que ja existe. Um campo
//! novo ali seria uma versao nova do bloco `PSCH` e uma migracao -- para
//! decidir uma coisa que nao e do DADO, e sim de como este servidor prefere
//! rolar os arquivos dele. E a mesma natureza do teto do cache de paginas
//! (`ndx::definir_cache_paginas`), e por isso mora no mesmo lugar: uma decisao
//! do processo, tomada uma vez no arranque.
//!
//! Zero -- o padrao -- quer dizer **nao mexe**: vale o `bytes_por_arquivo` do
//! esquema, byte por byte como antes.

use std::sync::atomic::{AtomicU64, Ordering};

use phxsql_core::paginacao::Paginacao;

/// Piso do corte. Abaixo disto o volume nao caberia nem um punhado de eventos,
/// e a paginacao viraria um arquivo por registro.
pub const CORTE_MINIMO: u64 = 64 * 1024;

/// Zero = herdar o `bytes_por_arquivo` do esquema, que e o comportamento velho.
static BYTES_POR_VOLUME: AtomicU64 = AtomicU64::new(0);

/// Ajusta onde o volume dos tres diarios corta, em bytes.
///
/// Vale para os arquivos abertos DAQUI PARA A FRENTE. Como isto e chamado no
/// arranque, antes de a primeira tabela abrir, na pratica vale para tudo.
///
/// Zero volta ao comportamento velho: manda o esquema. Um valor abaixo do piso
/// e subido ao piso em vez de recusado -- quem digitou `1024` queria volume
/// pequeno, e um arquivo por evento nao e o que ele queria.
pub fn definir_bytes_por_volume(bytes: u64) {
    let valor = if bytes == 0 {
        0
    } else {
        bytes.max(CORTE_MINIMO)
    };
    BYTES_POR_VOLUME.store(valor, Ordering::Relaxed);
}

/// O corte vigente, em bytes. Zero = manda o esquema.
pub fn bytes_por_volume() -> u64 {
    BYTES_POR_VOLUME.load(Ordering::Relaxed)
}

/// A paginacao que os tres diarios usam, ja com o corte deles.
///
/// Chamada por `LogFile`, `LixeiraFile` e `MotivoFile` na criacao e na
/// abertura. O `.bin` e o `.memo` **nao** passam por aqui: o corte de um anexo
/// e outro assunto, e junta-los faria mexer no diario mexer nas fotos.
pub fn paginacao(esquema: Paginacao) -> Paginacao {
    let corte = bytes_por_volume();
    if corte == 0 || !esquema.ligada() {
        return esquema;
    }
    Paginacao {
        bytes_por_arquivo: corte,
        ..esquema
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O unico teste que cabe AQUI: ele nao muda o global.
    ///
    /// Todo teste que LIGA o corte vive em `tests/corte-do-diario.rs`, e a
    /// razao e a mesma do cofre: `cargo test` roda os testes do mesmo binario
    /// em paralelo, e um teste que corta o diario em 1 MiB aqui dentro faria o
    /// `.log` de outro teste virar de volume no meio da corrida.
    #[test]
    fn o_padrao_e_nao_mexer() {
        assert_eq!(bytes_por_volume(), 0, "o corte do diario nasceu ligado");
        let p = Paginacao::nova(1_000, 99).unwrap();
        assert_eq!(
            paginacao(p).bytes_por_arquivo,
            p.bytes_por_arquivo,
            "sem configuracao o corte do diario mudou"
        );
        assert_eq!(
            paginacao(Paginacao::DESLIGADA).bytes_por_arquivo,
            Paginacao::DESLIGADA.bytes_por_arquivo
        );
    }
}
