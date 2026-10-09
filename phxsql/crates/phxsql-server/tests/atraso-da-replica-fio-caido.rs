//! Pedido 496, F7, revisao do papel C (4a): com o fio caido, o atraso nao
//! se mede, e o `replicacao_estado` diz isso -- `atraso_ms` e `alarme` nulos,
//! e nao o numero congelado da ultima rodada. Os tres maduros convergem
//! (`Seconds_Behind_Source` NULL com a thread de E/S parada, `replay_lag`
//! some sem reporte da standby).
//!
//! Processo proprio pelo motivo escrito em `atraso_da_replica/mod.rs`.

mod atraso_da_replica;
mod comum;

use std::time::{Duration, Instant};

use atraso_da_replica::*;
use comum::DirTemp;
use phxsql_core::json::Json;

/// **Prova real.** A replica puxa por uma ponte que o teste corta. Antes do
/// corte, a tabela parada com o mestre gravando tem `atraso_ms` e alarme; o
/// fio cai (`falhas_de_rede_seguidas > 0`) e os dois saem NULOS, com
/// `sem_relogio = "fio_caido"`, enquanto o `atraso` e a `amostra` ficam,
/// rotulados `na_ultima_amostra`.
///
/// **Vermelho** (o `para_json` antigo, sem o rotulo): o `atraso_ms` continua
/// o numero da ultima amostra com o fio caido, e o teste cai na asercao do
/// nulo.
#[test]
fn com_o_fio_caido_o_atraso_em_ms_e_o_alarme_saem_nulos() {
    let base_s = DirTemp::novo("atraso-fio-source");
    let base_r = DirTemp::novo("atraso-fio-replica");
    let (_source, porta_s) = subir_source(&base_s);
    preparar_source(porta_s);
    for id in 1..=3 {
        inserir(porta_s, id);
    }
    let ponte = Ponte::ate(porta_s);
    let (_replica, porta_r) = subir_replica(&base_r, ponte.porta);
    esperar_eventos(porta_r, 3);

    // A tabela para (escrita local) e o mestre segue gravando, ate o alarme.
    inserir(porta_r, 99);
    let escritor = Escritor::comecar(porta_s, 4, Duration::from_millis(250));
    let ate = Instant::now() + Duration::from_secs(40);
    let antes = loop {
        if let Some(a) =
            atraso(porta_r).filter(|a| a.campo("alarme").and_then(Json::texto).is_some())
        {
            break a;
        }
        assert!(Instant::now() < ate, "a tabela parada nao alarmou em 40 s");
        std::thread::sleep(Duration::from_millis(100));
    };
    drop(escritor);
    assert!(
        antes.inteiro_ou("atraso_ms", -1) >= 0,
        "{}",
        antes.escrever()
    );
    assert!(!antes.booleano_ou("na_ultima_amostra", true));

    ponte.cortar();
    let ate = Instant::now() + Duration::from_secs(30);
    while estado_da_fonte(porta_r).inteiro_ou("falhas_de_rede_seguidas", 0) == 0 {
        assert!(
            Instant::now() < ate,
            "o fio cortado nao virou falha de rede em 30 s"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let a = atraso(porta_r).expect("o atraso sumiu com o fio caido");
    assert!(
        matches!(a.campo("atraso_ms"), Some(Json::Nulo)),
        "o atraso em ms congelou com o fio caido: {}",
        a.escrever()
    );
    assert!(
        matches!(a.campo("alarme"), Some(Json::Nulo)),
        "o alarme congelou com o fio caido: {}",
        a.escrever()
    );
    assert_eq!(
        a.texto_ou("sem_relogio", ""),
        "fio_caido",
        "{}",
        a.escrever()
    );
    assert!(
        a.booleano_ou("na_ultima_amostra", false),
        "{}",
        a.escrever()
    );
    assert!(
        a.inteiro_ou("atraso", 0) > 0,
        "o atraso da ultima amostra sumiu: {}",
        a.escrever()
    );
}
