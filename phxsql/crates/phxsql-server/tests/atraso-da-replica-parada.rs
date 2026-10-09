//! Pedido 496, F7: a replica PARADA com o mestre gravando vira ocorrencia
//! pela tendencia, antes dos 60 s do limiar -- provada pelo soquete, entre um
//! source e uma replica de verdade.
//!
//! A prova irma, a do comportamento velho (replica em dia, nenhuma
//! ocorrencia), mora em `atraso-da-replica-em-dia.rs`: outro processo, pelo
//! motivo escrito em `atraso_da_replica/mod.rs`.

mod atraso_da_replica;
mod comum;

use std::time::{Duration, Instant};

use atraso_da_replica::*;
use comum::DirTemp;

/// **Prova real.** A tabela para pelo caminho real -- uma escrita local
/// aceita na replica rompe a continuidade (pedido 300 (4)) -- e o mestre
/// segue gravando. O residuo cresce rodada a rodada, e em tres amostras
/// crescendo o atraso vira ocorrencia pelo produtor unico: o
/// `ocorrencias.log` da replica ganha a linha, o sedimento ganha a pedra, e o
/// `replicacao_estado` diz `"alarme":"crescendo"` com o atraso em eventos.
///
/// **Vermelho** (a tendencia tirada de `AtrasoDaReplica::amostrar`, so o
/// limiar de 60 s): a tabela esta parada ha menos de 60 s quando o prazo de
/// 40 s vence, nada avisa, e o teste cai na espera da ocorrencia.
#[test]
fn replica_parada_com_o_mestre_gravando_vira_ocorrencia_pela_tendencia() {
    let base_s = DirTemp::novo("atraso-parada-source");
    let base_r = DirTemp::novo("atraso-parada-replica");
    let (_source, porta_s) = subir_source(&base_s);
    preparar_source(porta_s);
    for id in 1..=3 {
        inserir(porta_s, id);
    }
    let (_replica, porta_r) = subir_replica(&base_r, porta_s);
    esperar_eventos(porta_r, 3);
    let pedras_antes = pedras_do_atraso();

    // A escrita local toma o lugar do evento 4 do source no diario daqui: a
    // proxima conferencia rompe a continuidade e a tabela PARA.
    inserir(porta_r, 99);
    let inicio = Instant::now();
    let escritor = Escritor::comecar(porta_s, 4, Duration::from_millis(250));

    let ate = inicio + Duration::from_secs(40);
    let linhas = loop {
        let l = ocorrencias_do_atraso(&base_r);
        if !l.is_empty() {
            break l;
        }
        assert!(
            Instant::now() < ate,
            "a replica parada nao virou ocorrencia em 40 s; atraso: {:?}",
            atraso(porta_r).map(|a| a.escrever())
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    let decorrido = inicio.elapsed();
    // O estado e lido com o mestre AINDA gravando: parado o escritor, o
    // residuo deixa de crescer e a tendencia se desfaz na rodada seguinte.
    let a = atraso(porta_r).expect("o replicacao_estado nao expoe o atraso da tabela");
    drop(escritor);

    // Antes dos 60 s: foi a TENDENCIA, nao o limiar.
    assert!(
        decorrido < Duration::from_secs(60),
        "avisou so depois de {decorrido:?}: isso e o limiar, nao a tendencia"
    );
    assert!(
        a.inteiro_ou("atraso", 0) > 0,
        "o atraso exposto e zero com a tabela parada: {}",
        a.escrever()
    );
    assert!(
        a.inteiro_ou("na_origem", 0) > a.inteiro_ou("consumida", i64::MAX),
        "{}",
        a.escrever()
    );
    // O alarme que vale e o da tendencia (o limiar so depois de 60 s).
    assert_eq!(
        a.campo("alarme").and_then(|m| m.texto()),
        Some("crescendo"),
        "{}",
        a.escrever()
    );
    // A ocorrencia saiu pelo produtor unico: a linha E a pedra. A linha
    // nomeia a tabela, porque e por tabela que se conserta.
    assert!(
        linhas[0].contains(r#"{"database":"loja","tabela":"clientes"}"#),
        "a ocorrencia nao nomeia a tabela: {linhas:?}"
    );
    assert!(
        pedras_do_atraso() > pedras_antes,
        "a ocorrencia saiu, mas o sedimento do aquario nao ganhou a pedra"
    );
}
