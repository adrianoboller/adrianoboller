# Remove duplicate helper
# 28/08 10:55

import pathlib
p = pathlib.Path('crates/phxsql-store/src/catalogo.rs')
s = p.read_text()

# 1. comentario orfao que sobrou da insercao, e a contagem errada
v = '''    /// A tabela existe?
    /// As cinco extensoes de uma tabela, na ordem em que se fala delas.
    const EXTENSOES: [&'static str; 6] = ["reg", "ndx", "bin", "memo", "log", "bkp"];'''
n = '''    /// Os cinco arquivos de uma tabela, mais o espelho `.bkp`.
    const EXTENSOES: [&'static str; 6] = ["reg", "ndx", "bin", "memo", "log", "bkp"];'''
assert s.count(v) == 1
s = s.replace(v, n)

# 2. o ajudante duplicado: `separar_qualificado` ja fazia isso, e melhor --
#    ele recusa ".x" e "x." em vez de aceitar schema ou tabela vazios.
v = '''fn partir_qualificado(q: &str) -> (Option<&str>, &str) {
    match q.split_once('.') {
        Some((e, n)) => (Some(e), n),
        None => (None, q),
    }
}

'''
assert s.count(v) == 1
s = s.replace(v, "")

s = s.replace('''        let (schema, nome) = partir_qualificado(qualificado);
        validar_nome("tabela", nome)?;
        let dir = self.diretorio(schema)?;''',
'''        let (schema, nome) = separar_qualificado(qualificado);
        let (schema, nome) = (schema.as_deref(), nome.as_str());
        validar_nome("tabela", nome)?;
        let dir = self.diretorio(schema)?;''')

s = s.replace('''        let (schema_o, nome_o) = partir_qualificado(origem);
        let (schema_d, nome_d) = partir_qualificado(destino);
        validar_nome("tabela", nome_o)?;''',
'''        let (schema_o, nome_o) = separar_qualificado(origem);
        let (schema_d, nome_d) = separar_qualificado(destino);
        let (schema_o, nome_o) = (schema_o.as_deref(), nome_o.as_str());
        let (schema_d, nome_d) = (schema_d.as_deref(), nome_d.as_str());
        validar_nome("tabela", nome_o)?;''')

# 3. o teste do ajudante passa a exercer o que ficou
v = '''    #[test]
    fn qualificado_se_parte_em_schema_e_nome() {
        assert_eq!(partir_qualificado("clientes"), (None, "clientes"));
        assert_eq!(
            partir_qualificado("vendas.pedidos"),'''
n = '''    #[test]
    fn qualificado_se_parte_em_schema_e_nome() {
        let parte = |q: &str| {
            let (e, n) = separar_qualificado(q);
            (e, n)
        };
        assert_eq!(parte("clientes"), (None, "clientes".to_string()));
        assert_eq!(
            parte("vendas.pedidos"),'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
