# Measure the jump on 400k rows
# 28/08 19:47

import pathlib
p = pathlib.Path("crates/phxsql-store/examples/custo-da-pagina.rs")
s = p.read_text()

antigo = """use phxsql_store::table::{Table, Visao};"""
novo = """use phxsql_store::table::{Salto, Table, Visao};"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    assert_eq!(
        primeiras.len(),
        p2.len(),
        "a pagina 1 deu tamanhos diferentes"
    );"""
novo = """    // 5. A MESMA pagina do meio, agora pela posicao -- que e o `OFFSET` do
    //    SQL. E a comparacao que interessa: mesmo pedido, mesmo resultado,
    //    caminhos com custo diferente.
    let c = Instant::now();
    let (p5, como) = t.pagina_por_posicao(meio, pagina, Visao::Ativas).unwrap();
    let t_salto = ms(c);
    assert_eq!(como, Salto::Bissecao, "a tabela intacta tinha de bissetar");

    // 6. E o mesmo pedido depois de UM buraco: a igualdade entre posicao e
    //    rownum cai, e o motor tem de voltar a andar -- com a MESMA resposta.
    t.excluir_de_vez(n as u64, "medicao").unwrap();
    let c = Instant::now();
    let (p6, como6) = t.pagina_por_posicao(meio, pagina, Visao::Ativas).unwrap();
    let t_passo = ms(c);
    assert_eq!(como6, Salto::Passo, "com buraco nao pode bissetar");
    assert_eq!(p5, p6, "os dois caminhos deram paginas diferentes");

    assert_eq!(
        primeiras.len(),
        p2.len(),
        "a pagina 1 deu tamanhos diferentes"
    );"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    assert_eq!(p3, p4, "a pagina do meio deu rowids diferentes");"""
novo = """    assert_eq!(p3, p4, "a pagina do meio deu rowids diferentes");
    assert_eq!(p3, p5, "o salto por posicao deu rowids diferentes do cursor");"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    println!("PAGINA DO MEIO (offset {meio})");
    println!("  varrer_com (hoje) . {t_hoje_meio} ms");
    println!("  depois_de (cursor)  {t_cursor} ms");

    println!(
        "RESULTADO {{\\"linhas\\":{n},\\"pagina\\":{pagina},\\
         \\"hoje_ms\\":{t_hoje},\\"pagina_ms\\":{t_pagina},\\
         \\"hoje_meio_ms\\":{t_hoje_meio},\\"cursor_ms\\":{t_cursor}}}"
    );"""
novo = """    println!("PAGINA DO MEIO (offset {meio})");
    println!("  varrer_com (hoje) . {t_hoje_meio} ms");
    println!("  depois_de (cursor)  {t_cursor} ms");
    println!("  posicao, bissecao . {t_salto} ms");
    println!("  posicao, andando .. {t_passo} ms  (a mesma pagina, com um buraco)");

    println!(
        "RESULTADO {{\\"linhas\\":{n},\\"pagina\\":{pagina},\\
         \\"hoje_ms\\":{t_hoje},\\"pagina_ms\\":{t_pagina},\\
         \\"hoje_meio_ms\\":{t_hoje_meio},\\"cursor_ms\\":{t_cursor},\\
         \\"salto_ms\\":{t_salto},\\"passo_ms\\":{t_passo}}}"
    );"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
