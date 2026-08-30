# Add marcadas to Relatorio and build
# 28/08 19:44

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()

antigo = """    /// Registros conferidos no `.reason`.
    pub motivos: u64,"""
novo = """    /// Registros conferidos no `.reason`.
    pub motivos: u64,
    /// Linhas marcadas como excluidas, RECONTADAS -- e nao lidas do cabecalho.
    ///
    /// A conferencia existe justamente para nao acreditar em contador: se o
    /// numero do cabecalho tiver divergido, este e o caminho que descobre e
    /// conserta.
    pub marcadas: u64,"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """        let descartadas = self.lixeira.verificar()?;
        let motivos = self.motivos.verificar()?;
"""
novo = """        let descartadas = self.lixeira.verificar()?;
        let motivos = self.motivos.verificar()?;
        // Reconta e corrige de passagem: um contador de cache so serve
        // enquanto alguem se dispoe a conferi-lo.
        let marcadas = self.recontar_marcadas()?;
"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """            eventos,
            descartadas,
            motivos,
            volumes: ("""
novo = """            eventos,
            descartadas,
            motivos,
            marcadas,
            volumes: ("""
assert antigo in s
s = s.replace(antigo, novo)

p.write_text(s)
print("ok")
