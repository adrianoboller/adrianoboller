//! O punho de uma linha lida: os valores, e a vista em C deles.
//!
//! # Por que uma linha e um punho, e nao um buffer do chamador
//!
//! Porque o chamador nao consegue dimensionar. Um `Memo` tem 10 bytes ou 10
//! MB, e nao ha como saber antes de ler. O esquema "pergunte o tamanho e
//! chame de novo" pagaria a leitura duas vezes em toda linha grande, que e
//! justamente onde doi.
//!
//! O contrato fica visivel na assinatura: `phx_linha_valores` nao copia nada,
//! devolve uma VISTA. Os ponteiros valem ate o `phx_linha_liberar`.

use phxsql_core::value::Value;

use crate::valor::{do_value, PhxValor, PHX_DECIMAL};

pub struct LinhaFFI {
    /// Os donos dos bytes. Nao se mexe nisto depois de montada a vista.
    _valores: Vec<Value>,
    /// Os 16 bytes de cada `Decimal`, num lugar estavel.
    ///
    /// O `i128` nao tem representacao propria na struct de C -- nem `numero`
    /// (64 bits) nem `real` (que perderia digito) servem. Entao ele viaja como
    /// bytes, e bytes precisam morar em algum lugar que sobreviva a chamada.
    /// Este vetor e montado ANTES da vista e nunca cresce depois, senao os
    /// ponteiros da vista apontariam para a alocacao velha.
    _decimais: Vec<[u8; 16]>,
    vista: Vec<PhxValor>,
}

impl LinhaFFI {
    pub fn nova(valores: Vec<Value>) -> LinhaFFI {
        let decimais: Vec<[u8; 16]> = valores
            .iter()
            .filter_map(|v| match v {
                Value::Decimal(d) => Some(d.to_le_bytes()),
                _ => None,
            })
            .collect();

        let mut vista = Vec::with_capacity(valores.len());
        let mut k = 0usize;
        for v in &valores {
            let mut p = do_value(v);
            if p.tipo == PHX_DECIMAL {
                p.dados = decimais[k].as_ptr();
                p.tam = 16;
                k += 1;
            }
            vista.push(p);
        }
        LinhaFFI {
            _valores: valores,
            _decimais: decimais,
            vista,
        }
    }

    pub fn vista(&self) -> &[PhxValor] {
        &self.vista
    }
}
