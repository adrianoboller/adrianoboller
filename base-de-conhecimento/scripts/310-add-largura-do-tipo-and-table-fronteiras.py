# Add largura_do_tipo and Table::fronteiras
# 28/08 11:22

import pathlib
p = pathlib.Path('crates/phxsql-store/src/table.rs')
s = p.read_text()
v = '''    pub fn inserir(&mut self, valores: &[Value]) -> Result<RowId> {'''
n = '''    /// As fronteiras de volume do `.reg`. Vazio na particao por quantidade,
    /// onde o volume sai de divisao e nao ha tabela nenhuma.
    pub fn fronteiras(&self) -> &[crate::reg::Fronteira] {
        self.reg.fronteiras()
    }

    pub fn inserir(&mut self, valores: &[Value]) -> Result<RowId> {'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))

# largura_do_tipo no servidor
p = pathlib.Path('crates/phxsql-server/src/valores.rs')
s = p.read_text()
v = '''/// Le um tipo de coluna escrito em texto.'''
n = '''/// Quantos bytes o tipo ocupa no slot -- o "tamanho" que a tela mostra.
///
/// Para `Str` e o numero de caracteres declarado, que e o que quem escreveu o
/// esquema tem na cabeca. Para `Bin` e `Memo` e zero no slot: o que mora ali e
/// um ponteiro, e o conteudo vive no arquivo externo.
pub fn largura_do_tipo(t: &ColumnType) -> u64 {
    match t {
        ColumnType::Bool | ColumnType::Int1 | ColumnType::UInt1 => 1,
        ColumnType::Int2 | ColumnType::UInt2 => 2,
        ColumnType::Int4 | ColumnType::UInt4 | ColumnType::Real4 => 4,
        ColumnType::Date | ColumnType::Time => 4,
        ColumnType::Int8 | ColumnType::UInt8 | ColumnType::Real8 => 8,
        ColumnType::DateTime | ColumnType::Sequence => 8,
        ColumnType::Uuid => 16,
        ColumnType::Uuid256 => 32,
        ColumnType::Decimal { .. } => 16,
        ColumnType::Str(n) => *n as u64,
        ColumnType::Bin | ColumnType::Memo => 0,
    }
}

/// Le um tipo de coluna escrito em texto.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
