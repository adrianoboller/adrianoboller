//! Pedido 496, F7: o comportamento VELHO -- a replica em dia, com o mestre
//! gravando, nao gera ocorrencia nenhuma. Provada pelo soquete, entre um
//! source e uma replica de verdade.
//!
//! A prova irma (a replica parada vira ocorrencia) mora em
//! `atraso-da-replica-parada.rs`: outro processo, pelo motivo escrito em
//! `atraso_da_replica/mod.rs`.

mod atraso_da_replica;
mod comum;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use atraso_da_replica::*;
use comum::DirTemp;

/// O mestre grava sem parar por oito segundos -- oito rodadas ou mais da
/// replica, o bastante para a tendencia de tres amostras disparar se fosse
/// medida sobre o atraso CRU, que numa replica sadia sobe e desce com a
/// vazao de quem escreve. A replica acompanha, e ao fim: nenhuma linha no
/// `ocorrencias.log`, nenhuma pedra no sedimento, o alarme nulo e o atraso
/// exposto em zero.
#[test]
fn replica_em_dia_com_o_mestre_gravando_nao_gera_ocorrencia() {
    let base_s = DirTemp::novo("atraso-em-dia-source");
    let base_r = DirTemp::novo("atraso-em-dia-replica");
    let (_source, porta_s) = subir_source(&base_s);
    preparar_source(porta_s);
    for id in 1..=3 {
        inserir(porta_s, id);
    }
    let (_replica, porta_r) = subir_replica(&base_r, porta_s);
    esperar_eventos(porta_r, 3);
    let pedras_antes = pedras_do_atraso();

    let escritor = Escritor::comecar(porta_s, 4, Duration::from_millis(50));
    let ate = Instant::now() + Duration::from_secs(8);
    while Instant::now() < ate {
        if let Some(a) = atraso(porta_r) {
            assert!(
                a.campo("alarme").is_none_or(|m| m.texto().is_none()),
                "a replica em dia acusou atraso: {}",
                a.escrever()
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    // Parar ANTES de ler: o `inserir` em voo no momento da leitura entrava
    // depois dela, a replica chegava a um evento a mais e o `==` caia
    // (pedido 764). O `drop` junta a thread; so entao o contador e final.
    let contador = std::sync::Arc::clone(&escritor.escritos);
    drop(escritor);
    let escritos = contador.load(Ordering::SeqCst);
    assert!(
        escritos > 20,
        "o mestre mal gravou ({escritos}): a prova nao provou"
    );
    esperar_eventos(porta_r, 3 + escritos);

    // Uma rodada a mais, ja em dia, para a amostra final dizer zero.
    let ate = Instant::now() + Duration::from_secs(10);
    let a = loop {
        let a = atraso(porta_r).expect("o replicacao_estado nao expoe o atraso da tabela");
        if a.inteiro_ou("atraso", -1) == 0 || Instant::now() > ate {
            break a;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(a.inteiro_ou("atraso", -1), 0, "{}", a.escrever());
    assert_eq!(a.inteiro_ou("atraso_ms", -1), 0, "{}", a.escrever());
    assert!(a.campo("alarme").is_none_or(|m| m.texto().is_none()));
    assert!(
        ocorrencias_do_atraso(&base_r).is_empty(),
        "a replica em dia gerou ocorrencia: {:?}",
        ocorrencias_do_atraso(&base_r)
    );
    assert_eq!(
        pedras_do_atraso(),
        pedras_antes,
        "pedra no sedimento sem atraso"
    );
}

/// **Revisao do papel C (4b).** A tabela apagada na origem some de `atrasos`
/// na rodada seguinte: o `posicao` deixou de traze-la, e atraso de tabela que
/// nao existe mais e alarme que ninguem consegue resolver.
///
/// **Vermelho** (sem a poda depois do `posicao`): a entrada de
/// `loja/clientes` fica para sempre com a ultima amostra, e a espera de 20 s
/// cai.
///
/// Nenhum alarme nasce aqui -- a replica esta em dia o tempo todo --, entao
/// dividir o processo com a prova de cima nao a contamina.
#[test]
fn a_tabela_apagada_na_origem_some_do_atraso() {
    let base_s = DirTemp::novo("atraso-apagada-source");
    let base_r = DirTemp::novo("atraso-apagada-replica");
    let (_source, porta_s) = subir_source(&base_s);
    preparar_source(porta_s);
    inserir(porta_s, 1);
    let (_replica, porta_r) = subir_replica(&base_r, porta_s);
    esperar_eventos(porta_r, 1);
    let ate = Instant::now() + Duration::from_secs(20);
    while atraso(porta_r).is_none() {
        assert!(Instant::now() < ate, "a tabela nunca foi amostrada");
        std::thread::sleep(Duration::from_millis(100));
    }

    exigir(
        porta_s,
        r#""op":"excluir_tabela","database":"loja","tabela":"clientes","confirmar":"clientes""#,
    );
    let ate = Instant::now() + Duration::from_secs(20);
    while let Some(a) = atraso(porta_r) {
        assert!(
            Instant::now() < ate,
            "a tabela apagada na origem continua em atrasos: {}",
            a.escrever()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
