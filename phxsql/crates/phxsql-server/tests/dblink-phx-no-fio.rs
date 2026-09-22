//! O DbLink falando com outro PhxSql **pelo soquete** -- e a ORDEM do aperto.
//!
//! # Por que pelo fio, e nao por teste unitario
//!
//! Porque o que se prova aqui e uma ORDEM, e ordem nao aparece no valor de
//! nenhum campo: `cifra()` devolver `true` nao diz se o tunel subiu antes ou
//! depois do token. Um teste unitario que chamasse `abrir` e conferisse o
//! resultado passaria dos dois jeitos -- e teste que passa por engano e pior
//! que teste que falta.
//!
//! O servidor falso aqui e minimo de proposito: ele **so le a primeira linha**
//! que o cliente escreve e devolve essa linha para o teste. Nao ha aperto de
//! mao X25519 do outro lado, e nem precisa haver: o que se mede e o que sai
//! deste lado do fio, primeiro.
//!
//! # A premissa, medida em vez de citada
//!
//! O pedido 378 diz que «no DbLink o token viaja no PRIMEIRO pedido, entao
//! cifrar depois protege so o que sobrou». O segundo teste deste arquivo e a
//! medida dessa frase: com a cifra desligada, o token APARECE na primeira
//! linha do fio. E por isso que a ordem importa mais aqui do que na replica.

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use phxsql_core::json::Json;
use phxsql_server::dblink::Definicao;

/// O token de servico do OUTRO servidor -- o portao 1 dele.
///
/// Quem o tem alcanca a porta de dados daquele servidor sem usuario nenhum, e
/// e por isso que ele nao pode viajar em claro.
const TOKEN: &str = "TOKEN-DE-SERVICO-DO-OUTRO-PHXSQL";

/// Sobe um servidor que le UMA linha e devolve o que leu.
///
/// Devolve a porta e o canal por onde a linha chega. A conexao e fechada logo
/// depois: o cliente vai falhar, e a falha nao e o que se mede.
fn escuta_uma_linha() -> (u16, mpsc::Receiver<String>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let (envia, recebe) = mpsc::channel();
    thread::spawn(move || {
        if let Ok((fluxo, _)) = ouvinte.accept() {
            fluxo.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let mut leitor = BufReader::new(fluxo);
            let mut linha = String::new();
            let _ = leitor.read_line(&mut linha);
            let _ = envia.send(linha);
        }
    });
    (porta, recebe)
}

/// A ligacao COM usuario, e o usuario esta aqui de proposito.
///
/// Sem ele nao ha `autenticar`, e a primeira linha do fio seria o `ping` nos
/// dois desenhos -- o certo e o errado. Foi o que aconteceu na primeira
/// escrita deste arquivo: o defeito reposto (mover o `cifrar` para depois do
/// `autenticar`) **passou**, porque sem login nao havia login para vir antes.
/// Teste que passa por engano e pior que teste que falta; o `usuario` e o que
/// poe o `{"op":"desafio"}` no meio, que e o pedido que a ordem protege.
fn ligacao(porta: u16, extra: &str) -> Definicao {
    Definicao::de_json(
        &Json::analisar(&format!(
            r#"{{"nome":"erp","motor":"phxsql","host":"127.0.0.1","porta":{porta},
                 "token_remoto":"{TOKEN}","usuario":"leitor","senha":"x",
                 "timeout_s":2{extra}}}"#
        ))
        .unwrap(),
    )
    .unwrap()
}

/// A primeira linha que sai de uma ligacao `phxsql` de fabrica e o `cifrar` --
/// e o token NAO esta nela.
///
/// Este e o teste da ORDEM. Com o `cifrar` depois do `autenticar`, a primeira
/// linha do fio seria o `{"op":"desafio"}` do login, com o token do outro
/// servidor em claro dentro dela, e o tunel subiria para proteger o que
/// sobrou.
#[test]
fn a_ligacao_de_fabrica_abre_o_tunel_antes_de_qualquer_pedido() {
    let (porta, recebe) = escuta_uma_linha();
    // A conexao vai falhar (o servidor falso fecha sem responder o aperto), e
    // isso nao e o que se mede -- o que se mede e o que ja saiu no fio.
    let _ = ligacao(porta, "").abrir();
    let linha = recebe.recv_timeout(Duration::from_secs(10)).unwrap();

    assert!(
        linha.contains("\"op\":\"cifrar\""),
        "a primeira linha do fio nao foi o aperto de mao: {linha:?}"
    );
    assert!(
        !linha.contains(TOKEN),
        "o token de servico do outro servidor foi para o fio EM CLARO: {linha:?}"
    );
}

/// `"cifra": false` continua abrindo em claro -- e a primeira linha mostra o
/// token.
///
/// As duas metades num teste so, de proposito: a de cima e o escape escrito
/// funcionando (quem precisa falar com um servidor anterior ao aperto continua
/// podendo), e a de baixo e a MEDIDA da premissa do pedido 378 -- o token
/// viaja no primeiro pedido, entao cifrar depois dele chega tarde.
#[test]
fn com_o_escape_escrito_o_fio_vai_em_claro_e_o_token_sai_no_primeiro_pedido() {
    let (porta, recebe) = escuta_uma_linha();
    let _ = ligacao(porta, r#","cifra":false"#).abrir();
    let linha = recebe.recv_timeout(Duration::from_secs(10)).unwrap();

    assert!(
        !linha.contains("\"op\":\"cifrar\""),
        "o escape escrito nao foi respeitado: {linha:?}"
    );
    assert!(
        linha.contains(TOKEN),
        "o token nao saiu no primeiro pedido -- a premissa da ordem mudou: {linha:?}"
    );
}

/// Com pino escrito, o aperto de mao leva a chave publica deste lado e o
/// `"cifra": false` ao lado nao desliga nada.
///
/// O pino vence o interruptor, como no ODBC: quem escreveu o pino quer o tunel
/// CONFERIDO, e desliga-lo por uma chave escrita ao lado seria rebaixar em
/// silencio o que a pessoa pediu.
#[test]
fn o_pino_vence_o_interruptor_tambem_no_fio() {
    let (porta, recebe) = escuta_uma_linha();
    let pino = "cc".repeat(32);
    let _ = ligacao(porta, &format!(r#","cifra":false,"chave_do_fio":"{pino}""#)).abrir();
    let linha = recebe.recv_timeout(Duration::from_secs(10)).unwrap();

    assert!(
        linha.contains("\"op\":\"cifrar\""),
        "o false desligou o tunel de quem pediu pino: {linha:?}"
    );
    assert!(!linha.contains(TOKEN), "{linha:?}");
}
