# Make mudar_marcadas persist
# 28/08 19:41

import pathlib
p = pathlib.Path("crates/phxsql-store/src/reg.rs")
s = p.read_text()
antigo = """    /// Soma `delta` ao contador de marcadas. Negativo desmarca.
    ///
    /// Nao grava o cabecalho: quem chama esta no meio de uma operacao que ja
    /// vai grava-lo (o `excluir` e o `atualizar` do slot escrevem o cabecalho
    /// no fim). Escrever duas vezes seria pagar um `write` por nada.
    pub fn mudar_marcadas(&mut self, delta: i64) {
        if delta >= 0 {
            self.marcadas = self.marcadas.saturating_add(delta as u64);
        } else {
            self.marcadas = self.marcadas.saturating_sub(delta.unsigned_abs());
        }
    }

    /// Regrava o contador de marcadas e leva ao disco.
    pub fn definir_marcadas(&mut self, n: u64) -> Result<()> {
        if self.marcadas == n {
            return Ok(());
        }
        self.marcadas = n;
        self.gravar_cabecalho(1)
    }"""
novo = """    /// Soma `delta` ao contador de marcadas, e grava o cabecalho.
    ///
    /// Grava mesmo custando um `write` de 128 bytes a mais na operacao. Um
    /// contador que so vai ao disco no `sincronizar` volta atras numa queda, e
    /// este aqui nao e um numero de vitrine: e ele que decide se o salto por
    /// bisseccao pode confiar no `rownum`. Errado, ele manda a tela para a
    /// linha errada -- calada.
    pub fn mudar_marcadas(&mut self, delta: i64) -> Result<()> {
        if delta == 0 {
            return Ok(());
        }
        if delta > 0 {
            self.marcadas = self.marcadas.saturating_add(delta as u64);
        } else {
            self.marcadas = self.marcadas.saturating_sub(delta.unsigned_abs());
        }
        self.gravar_cabecalho(1)
    }

    /// Regrava o contador de marcadas e leva ao disco. E o caminho do reparo.
    pub fn definir_marcadas(&mut self, n: u64) -> Result<()> {
        if self.marcadas == n {
            return Ok(());
        }
        self.marcadas = n;
        self.gravar_cabecalho(1)
    }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
s = s.replace("            self.reg.mudar_marcadas(1);\n", "            self.reg.mudar_marcadas(1)?;\n")
s = s.replace("            self.reg.mudar_marcadas(delta);\n", "            self.reg.mudar_marcadas(delta)?;\n")
s = s.replace("        self.reg.mudar_marcadas(if valor { 1 } else { -1 });\n",
              "        self.reg.mudar_marcadas(if valor { 1 } else { -1 })?;\n")
s = s.replace("                self.reg.mudar_marcadas(-1);\n", "                self.reg.mudar_marcadas(-1)?;\n")
p.write_text(s)
print("ok")
