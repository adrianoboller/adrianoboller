//! Prova real do pedido 150 para o guarda do lado dos testes de INTEGRACAO
//! (`tests/comum::DirTemp` -- o irmao deste guarda do lado unitario mora em
//! `src/apoio_teste.rs`, e tem a mesma prova).
//!
//! O padrao velho era um `rm` que so rodava se o teste chegasse ao fim do
//! corpo -- e falhar no MEIO, nao no fim, e' o caso comum de um teste de
//! asserção. Por isso a prova real tem de ser nos dois sentidos: o teste
//! abaixo tem de FALHAR se o `impl Drop` for removido (ja confirmado a mao,
//! comentando o `Drop` em `apoio_teste.rs` e vendo este teste cair com a
//! mesma asserção) e passar com o guarda no lugar.

mod comum;

#[test]
fn falha_no_meio_do_teste_ainda_assim_limpa_o_diretorio() {
    let guarda = comum::DirTemp::novo("prova-panico-integracao");
    let caminho = guarda.0.clone();
    assert!(
        caminho.is_dir(),
        "o guarda tem de criar o diretorio na hora"
    );

    // O guarda e' movido para dentro do closure: quando o panic desenrola
    // este quadro, o `Drop` roda ali, antes de `catch_unwind` devolver o
    // erro para fora.
    let resultado = std::panic::catch_unwind(move || {
        let _preso_no_escopo_que_vai_falhar = guarda;
        panic!("falha proposital, so para provar que o Drop roda mesmo assim");
    });

    assert!(resultado.is_err(), "o panico tinha de propagar ate aqui");
    assert!(
        !caminho.exists(),
        "o Drop tinha de ter apagado o diretorio durante o desenrolamento do panic"
    );
}
