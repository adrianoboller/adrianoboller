//! O IRMAO do pedido 312: o aperto de mao da INTERFACE, em `Remoto::cifrar`.
//!
//! # Por que um arquivo so para o irmao
//!
//! O pedido nomeou DOIS pontos, e o primeiro (`replica::Cliente::cifrar`) tem
//! prova de unidade ao lado do codigo. Este e o segundo: mesma coreografia --
//! escreve `{"op":"cifrar"}`, le UMA linha antes de o tunel existir, analisa,
//! fecha o aperto --, so que quem chama e a interface web falando com outro
//! PhxSql. «Conserto entra no caminho que o motivou, e o caminho IRMAO fica»:
//! o irmao fica, e fica com prova propria, senao a proxima refatoracao
//! desfaz um dos dois em silencio.
//!
//! O que se mede: antes do conserto, um destino falso empurrou **192 MiB numa
//! linha so, em 294 ms**, e este lado guardou tudo -- 1,5x o
//! `TETO_DO_REGISTRO`, que e a prova de que aquele teto nao alcancava esta
//! leitura. Depois, a leitura para no teto do APERTO e devolve erro.

use std::io::Write;
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use phxsql_server::servidor::Remoto;

/// Passa do `TETO_DO_REGISTRO` de proposito.
const DESPEJO: u64 = 192 * 1024 * 1024;

/// Um destino que aceita a conexao e nunca termina de falar.
fn destino_que_nunca_cala() -> (u16, Arc<AtomicU64>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let empurrados = Arc::new(AtomicU64::new(0));
    let conta = Arc::clone(&empurrados);
    std::thread::spawn(move || {
        let Ok((mut fluxo, _)) = ouvinte.accept() else {
            return;
        };
        let _ = fluxo.set_write_timeout(Some(Duration::from_secs(3)));
        let lixo = vec![b'x'; 64 * 1024];
        while conta.load(Ordering::SeqCst) < DESPEJO {
            if fluxo.write_all(&lixo).is_err() {
                break;
            }
            conta.fetch_add(lixo.len() as u64, Ordering::SeqCst);
        }
    });
    (porta, empurrados)
}

/// **Prova real do irmao, nos dois sentidos.** Com o `read_line` cru reposto,
/// nao ha erro nenhum e o contador chega aos 192 MiB.
#[test]
fn o_aperto_da_interface_recusa_a_linha_sem_fim() {
    let (porta, empurrados) = destino_que_nunca_cala();
    let mut r = Remoto::abrir(&format!("127.0.0.1:{porta}"), 2).unwrap();
    let erro = r
        .cifrar(None)
        .expect_err("o aperto da interface tinha de RECUSAR a linha sem fim");
    assert_eq!(
        erro.nome(),
        "LIMITE_EXCEDIDO",
        "a recusa saiu como {erro} ({})",
        erro.nome()
    );
    let engolido = empurrados.load(Ordering::SeqCst);
    assert!(
        engolido < DESPEJO,
        "o destino empurrou os {DESPEJO} bytes inteiros: nao ha teto nenhum"
    );
}

/// O COMPORTAMENTO VELHO: uma resposta de aperto do tamanho de sempre
/// continua sendo lida e analisada -- o teto so morde quem passa dele.
#[test]
fn resposta_curta_do_destino_continua_passando() {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let Ok((mut fluxo, _)) = ouvinte.accept() else {
            return;
        };
        let _ = fluxo
            .write_all(br#"{"ok":false,"op":"cifrar","erro":"a cifra do fio esta desligada"}"#);
        let _ = fluxo.write_all(b"\n");
        let _ = fluxo.flush();
        std::thread::sleep(Duration::from_millis(200));
    });
    let mut r = Remoto::abrir(&format!("127.0.0.1:{porta}"), 2).unwrap();
    let erro = r
        .cifrar(None)
        .expect_err("o destino recusou: tinha de vir erro");
    assert_eq!(erro.nome(), "ACESSO_NEGADO", "veio {erro}");
    assert!(erro.to_string().contains("desligada"), "{erro}");
}
