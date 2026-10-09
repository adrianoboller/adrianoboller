//! Pedido 496, fatia F8 (C10/C11 do `docs/propostas/ia-495-496-desenho.md`):
//! o tamanho do plano de escrita, medido ANTES da primeira escrita.
//!
//! O `UPDATE`/`DELETE` por faixa ja colhe a lista inteira de rowids antes de
//! gravar (e o que fecha o Halloween), e a cascata do `ao_alterar` ja e
//! achatada numa lista antes da marca. O numero existe nos dois caminhos e
//! ninguem olhava para ele: um `WHERE id > 0` esquecido reescreve a tabela
//! inteira, e o primeiro sinal era o dano.
//!
//! # So observa
//!
//! Nada aqui recusa nem muda a resposta. A ocorrencia sai pelo produtor
//! unico (`telemetria::sinal`, A3), e quem decide o que fazer com ela e
//! gente. Recusar seria guarda nova IMPOSTA -- todo job de manutencao que
//! reescreve uma tabela pequena pararia de um dia para o outro.
//!
//! # O portao vem antes do trabalho
//!
//! A primeira comparacao e de inteiro contra o piso: plano menor que
//! [`PISO_DE_LINHAS`] -- o caso de quase todo pedido -- sai sem montar texto
//! nem contar nada.

use phxsql_core::json::Json;
use phxsql_store::table::EscritaDaCascata;

use crate::aquario::Alarme;

/// Abaixo disto, nenhum plano e largo, qualquer que seja a fracao: dez linhas
/// de uma tabela de doze sao «quase tudo» e nao assustam ninguem.
pub const PISO_DE_LINHAS: u64 = 1_000;

/// A fracao das vivas, em por cento, a partir da qual o plano e largo.
pub const FRACAO_EM_PORCENTO: u64 = 50;

/// O plano de `linhas` sobre uma tabela de `vivas` e largo?
///
/// `>=` nos dois limiares, como o desenho escreve («≥ 50% e ≥ 1.000»).
/// Em inteiros, para a fronteira nao depender de arredondamento.
pub fn largo(linhas: u64, vivas: u64) -> bool {
    linhas >= PISO_DE_LINHAS
        && linhas.saturating_mul(100) >= vivas.saturating_mul(FRACAO_EM_PORCENTO)
}

/// O plano de um `UPDATE`/`DELETE` por faixa, com a lista de rowids ja
/// fechada e nada gravado. `vivas` e o que o `coletar_rowids` examinou na
/// visao das ativas -- o mesmo passo, nenhuma leitura a mais.
pub fn observar_a_faixa(op: &str, database: &str, tabela: &str, linhas: u64, vivas: u64) {
    if !largo(linhas, vivas) {
        return;
    }
    avisar(op, database, tabela, linhas, vivas);
}

/// O plano de uma cascata do `ao_alterar`, achatado e ainda nao aplicado.
/// Conta por filha: mil linhas espalhadas por tres tabelas grandes nao sao
/// o mesmo susto que mil linhas de uma tabela de mil.
pub fn observar_a_cascata(database: &str, plano: &[EscritaDaCascata]) {
    // O total limita cada parcela: abaixo do piso no total, nenhuma filha
    // passa dele, e o laco nem comeca.
    if (plano.len() as u64) < PISO_DE_LINHAS {
        return;
    }
    let mut por_filha: Vec<(&str, u64, u64)> = Vec::new();
    for e in plano {
        match por_filha.iter_mut().find(|(t, _, _)| *t == e.tabela) {
            Some((_, n, _)) => *n += 1,
            None => por_filha.push((&e.tabela, 1, e.vivas_na_filha)),
        }
    }
    for (tabela, linhas, vivas) in por_filha {
        if largo(linhas, vivas) {
            avisar("cascata", database, tabela, linhas, vivas);
        }
    }
}

/// O corpo unico dos dois. Os `dados` vao como pedido JSON, porque a camada
/// redige pela forma (`profiler::forma_do_pedido`): `op`, `database` e
/// `tabela` ficam, e a tabela entra em `tabelas` da ocorrencia. Os numeros
/// a redacao troca por `?` -- e regra da camada, e nao daqui.
fn avisar(op: &str, database: &str, tabela: &str, linhas: u64, vivas: u64) {
    let dados = Json::objeto(vec![
        ("op", Json::texto_de(op)),
        ("database", Json::texto_de(database)),
        ("tabela", Json::texto_de(tabela)),
        ("linhas", Json::de_u64(linhas)),
        ("vivas", Json::de_u64(vivas)),
    ]);
    crate::telemetria::sinal(Alarme::PlanoLargo, &dados.escrever());
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A fronteira dos dois limiares, nos dois sentidos. Vermelho: trocar
    /// qualquer `>=` por `>` derruba uma das quatro linhas.
    #[test]
    fn os_dois_limiares_sao_inclusivos() {
        assert!(largo(1_000, 2_000), "50% e 1.000 exatos");
        assert!(!largo(999, 999), "100%, mas abaixo do piso");
        assert!(!largo(1_000, 2_001), "1.000, mas abaixo de 50%");
        assert!(largo(2_000, 2_000));
        assert!(!largo(10, 2_000));
    }
}
