# Fix clippy warnings
# 28/08 20:18

import pathlib, re
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """    /// Desmonta a imagem. Inversa exata de [`Table::imagem_da_linha`].
    pub fn abrir_imagem(imagem: &[u8]) -> Result<(Vec<u8>, Vec<(u16, Vec<u8>)>)> {"""
novo = """    /// Desmonta a imagem. Inversa exata de [`Table::imagem_da_linha`].
    pub fn abrir_imagem(imagem: &[u8]) -> Result<ImagemAberta> {"""
assert antigo in s
s = s.replace(antigo, novo)
antigo = """/// Como a pagina por posicao chegou ao inicio dela."""
novo = """/// O que sai de [`Table::abrir_imagem`]: o payload cru e, para cada coluna
/// externa, o conteudo dela -- e nao o ponteiro, que so vale na maquina de
/// origem.
pub type ImagemAberta = (Vec<u8>, Vec<(u16, Vec<u8>)>);

/// Como a pagina por posicao chegou ao inicio dela."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

p = pathlib.Path("crates/phxsql-store/tests/replicacao.rs")
s = p.read_text()
s = s.replace("s.inserir(&vec![", "s.inserir(&[")
s = re.sub(r"    \]\)\n    \.unwrap\(\);", "    ])\n    .unwrap();", s)
p.write_text(s)
