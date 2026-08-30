# Fix test and run
# 28/08 10:56

import pathlib
p = pathlib.Path('crates/phxsql-store/src/catalogo.rs')
s = p.read_text()
v = '''    fn qualificado_se_parte_em_schema_e_nome() {
        let parte = |q: &str| {
            let (e, n) = separar_qualificado(q);
            (e, n)
        };
        assert_eq!(parte("clientes"), (None, "clientes".to_string()));
        assert_eq!(
            parte("vendas.pedidos"),
            (Some("vendas"), "pedidos")
        );
    }'''
n = '''    fn qualificado_se_parte_em_schema_e_nome() {
        let parte = |q: &str| {
            let (e, n) = separar_qualificado(q);
            (e, n)
        };
        assert_eq!(parte("clientes"), (None, "clientes".into()));
        assert_eq!(parte("vendas.pedidos"), (Some("vendas".into()), "pedidos".into()));
        // Ponto solto nao vira schema vazio: o nome inteiro fica sendo a
        // tabela, e o `validar_nome` recusa depois. E o que impede um
        // ".reg" de virar caminho.
        assert_eq!(parte(".pedidos"), (None, ".pedidos".into()));
        assert_eq!(parte("vendas."), (None, "vendas.".into()));
    }'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
