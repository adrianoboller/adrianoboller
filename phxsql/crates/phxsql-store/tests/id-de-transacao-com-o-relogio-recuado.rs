//! **Pedido 684 (a): o id de transacao nao recua quando o relogio recua
//! entre dois arranques.**
//!
//! # O defeito
//!
//! O id sai do relogio (`ms << 16`) e do `ULTIMO_TX` em memoria. Um processo
//! novo comeca com o `ULTIMO_TX` zerado, entao, com o relogio da maquina
//! atras do da vida anterior (NTP que corrige para tras, relogio de BIOS),
//! o primeiro id da vida nova sai MENOR que o ultimo gravado. O diario
//! daquela tabela fica fora de ordem, e o `replica::Juntador`, que supoe cada
//! diario em ordem de id, entrega a venda daquele trecho partida -- sem
//! contar em `transacoes_em_pedacos`.
//!
//! # A prova, sem dormir
//!
//! O «segundo arranque» e o mesmo binario com o `ULTIMO_TX` esquecido e o
//! relogio do id recuado por injecao (`desviar_relogio_do_tx_para_teste`).
//! Duas vias, porque o piso tem duas fontes: o cabecalho sincronizado e a
//! cauda que a cura conta (o evento gravado depois do ultimo `sincronizar`).
//!
//! Um `#[test]` so neste arquivo, e de proposito: o `ULTIMO_TX` e o desvio
//! do relogio sao do PROCESSO, e um vizinho em paralelo os veria mexer.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::paginacao::Paginacao;
use phxsql_store::log::{self, LogFile, Operacao};

/// Uma hora para tras: muito alem do que o contador anda numa prova.
const RECUO_MS: i64 = -3_600_000;

fn um_evento(l: &mut LogFile, rowid: u64) -> u64 {
    l.registrar(Operacao::Inclusao, rowid, 1).unwrap().tx
}

#[test]
fn o_id_de_transacao_nao_recua_quando_o_relogio_recua_entre_dois_arranques() {
    let d = DirTemp::novo("tx-relogio-recuado");

    // Vida 1, relogio certo: um evento no cabecalho sincronizado (`a`) e um
    // so na cauda, depois do ultimo `sincronizar` (`b`).
    log::desviar_relogio_do_tx_para_teste(0);
    let mut a = LogFile::criar(&d.0, "a", Paginacao::DESLIGADA).unwrap();
    let tx_a = um_evento(&mut a, 1);
    a.sincronizar().unwrap();
    drop(a);
    let mut b = LogFile::criar(&d.0, "b", Paginacao::DESLIGADA).unwrap();
    b.sincronizar().unwrap();
    let tx_b = um_evento(&mut b, 1);
    drop(b); // sem `sincronizar`: o cabecalho no disco nao conta o evento
    let recuos_antes = log::recuos_do_relogio();

    // Vida 2, relogio uma hora atras.
    log::esquecer_ultimo_tx_para_teste();
    log::desviar_relogio_do_tx_para_teste(RECUO_MS);

    let mut a = LogFile::abrir(&d.0, "a", Paginacao::DESLIGADA).unwrap();
    let novo_a = um_evento(&mut a, 2);
    assert!(
        novo_a > tx_a,
        "o relogio recuou entre dois arranques e o diario da tabela a recebeu \
         o id {novo_a}, MENOR que o {tx_a} da vida anterior -- o cabecalho \
         sincronizado nao semeou o piso"
    );

    // A tabela b abre na mesma vida, ja com o piso de `a` -- entao o piso que
    // importa e o do proprio diario: so a cura viu o evento de `b`.
    log::esquecer_ultimo_tx_para_teste();
    let mut b = LogFile::abrir(&d.0, "b", Paginacao::DESLIGADA).unwrap();
    let novo_b = um_evento(&mut b, 2);
    assert!(
        novo_b > tx_b,
        "o evento de b estava so na cauda (depois do ultimo sincronizar), e o \
         id novo {novo_b} saiu MENOR que o {tx_b} dele -- a cura nao semeou o \
         piso"
    );

    // E o recuo vira numero, em vez de silencio.
    assert!(
        log::recuos_do_relogio() > recuos_antes,
        "o diario trouxe id a frente do relogio e o recuo nao foi contado"
    );

    // A ordem continua valendo dentro de cada diario, lida do disco.
    let ids: Vec<u64> = a.ler(0, 0).unwrap().iter().map(|e| e.tx).collect();
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "diario a fora de ordem: {ids:?}"
    );
    let ids: Vec<u64> = b.ler(0, 0).unwrap().iter().map(|e| e.tx).collect();
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "diario b fora de ordem: {ids:?}"
    );

    log::desviar_relogio_do_tx_para_teste(0);
}
