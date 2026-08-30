# Measure at 800k rows in microseconds
# 28/08 19:48

import pathlib
p = pathlib.Path("crates/phxsql-store/examples/custo-da-pagina.rs")
s = p.read_text()
s = s.replace("""fn ms(t: Instant) -> u128 {
    t.elapsed().as_millis()
}""", """fn ms(t: Instant) -> u128 {
    t.elapsed().as_millis()
}

/// A bisseccao nao aparece em milissegundo: vinte leituras num arquivo quente
/// somam microssegundos. Medir em ms devolveria «0» e nao provaria nada.
fn us(t: Instant) -> u128 {
    t.elapsed().as_micros()
}""")
s = s.replace("""    let (p5, como) = t.pagina_por_posicao(meio, pagina, Visao::Ativas).unwrap();
    let t_salto = ms(c);""", """    let (p5, como) = t.pagina_por_posicao(meio, pagina, Visao::Ativas).unwrap();
    let t_salto = us(c);""")
s = s.replace("""    let (p6, como6) = t.pagina_por_posicao(meio, pagina, Visao::Ativas).unwrap();
    let t_passo = ms(c);""", """    let (p6, como6) = t.pagina_por_posicao(meio, pagina, Visao::Ativas).unwrap();
    let t_passo = us(c);""")
s = s.replace("""    println!("  posicao, bissecao . {t_salto} ms");
    println!("  posicao, andando .. {t_passo} ms  (a mesma pagina, com um buraco)");""",
"""    println!("  posicao, bissecao . {t_salto} us");
    println!("  posicao, andando .. {t_passo} us  (a mesma pagina, com um buraco)");""")
s = s.replace("""\\"salto_ms\\":{t_salto},\\"passo_ms\\":{t_passo}}}\"""",
              """\\"salto_us\\":{t_salto},\\"passo_us\\":{t_passo}}}\"""")
p.write_text(s)
print("ok")
