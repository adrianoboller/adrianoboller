//! Pedido 497, P1 do parecer SEC: o valor do pedido citado sem teto.
use super::*;

/// O instante do PITR e a duracao do prazo recusavam citando o texto
/// recebido pelo `{:?}`, sem teto -- um megabyte ia inteiro ao
/// `acessos.log`. O curto continua citado (e o diagnostico, pedido 453);
/// o longo vira o tamanho. O instante fica aqui, e nao no soquete, porque
/// o `restaurar_backup` so o le depois de achar um backup de verdade.
#[test]
fn o_instante_e_a_duracao_citam_pelo_teto() {
    let longo = format!("SEGREDO123{}", "x".repeat(1 << 20));
    let p = Json::objeto(vec![
        ("ate", Json::texto_de(&longo)),
        ("timeout", Json::texto_de(&longo)),
    ]);
    let e = Servidor::instante_pedido(&p, "ate", "ate_ms")
        .unwrap_err()
        .to_string();
    assert!(
        e.len() < 400 && !e.contains("SEGREDO123"),
        "instante: {e:.200}"
    );
    assert!(e.contains("nao e um instante"), "{e:.200}");
    let e = duracao_ms(&p, "timeout", 0).unwrap_err().to_string();
    assert!(
        e.len() < 400 && !e.contains("SEGREDO123"),
        "duracao: {e:.200}"
    );
    // O curto: continua dizendo o que veio.
    let p = Json::objeto(vec![("ate", Json::texto_de("ontem"))]);
    let e = Servidor::instante_pedido(&p, "ate", "ate_ms")
        .unwrap_err()
        .to_string();
    assert!(e.contains("\"ontem\""), "{e}");
}
