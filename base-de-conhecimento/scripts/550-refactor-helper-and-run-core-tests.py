# Refactor helper and run core tests
# 28/08 17:24

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    fn esquema_clientes() -> Schema {
        Schema::new(
            "cadastroClientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(60)).obrigatoria(),
                Column::new("cnpj", ColumnType::Str(14)),
                Column::new(
                    "limite",
                    ColumnType::Decimal {
                        precisao: 15,
                        escala: 2,
                    },
                ),
                Column::new("foto", ColumnType::Bin),
                Column::new("observacao", ColumnType::Memo),
            ],'''
novo='''    fn colunas_clientes() -> Vec<Column> {
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cnpj", ColumnType::Str(14)),
            Column::new(
                "limite",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
            Column::new("foto", ColumnType::Bin),
            Column::new("observacao", ColumnType::Memo),
        ]
    }

    fn esquema_clientes() -> Schema {
        Schema::new(
            "cadastroClientes",
            colunas_clientes(),'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
