# Add the image flag to Table
# 28/08 20:08

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()

antigo = """    log: LogFile,
    lixeira: LixeiraFile,
    motivos: MotivoFile,
}"""
novo = """    log: LogFile,
    lixeira: LixeiraFile,
    motivos: MotivoFile,
    /// Gravar a imagem da linha no diario? Ver [`Table::com_imagem_no_diario`].
    imagem_no_diario: bool,
}"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
