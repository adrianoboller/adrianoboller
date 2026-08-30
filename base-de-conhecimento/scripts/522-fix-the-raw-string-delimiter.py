# Fix the raw string delimiter
# 28/08 16:56

p='crates/phxsql-server/src/exportar.rs'
s=open(p).read()
# O conteudo do XLSX tem `"#` (em formatCode="#,##0.00"), que fecha o r#"..."#
# cedo. Sobe para r##"..."##.
a='const ESTILOS_XLSX: &str = r#"<?xml'
b='const ESTILOS_XLSX: &str = r##"<?xml'
assert a in s; s=s.replace(a,b,1)
a='''</cellStyles>
</styleSheet>"#;'''
b='''</cellStyles>
</styleSheet>"##;'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
