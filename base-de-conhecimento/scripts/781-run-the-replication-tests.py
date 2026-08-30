# Run the replication tests
# 28/08 20:10

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """    /// Eventos de um registro especifico.
    pub fn historico(&mut self, rowid: RowId) -> Result<Vec<Evento>> {"""
novo = """    /// O mesmo, trazendo a imagem de cada evento. E o fluxo da replicacao.
    ///
    /// `pular` e a POSICAO que a replica guardou: o evento N e a posicao N, e
    /// por isso a replica precisa de um numero so por tabela -- nao ha GTID a
    /// inventar nem par arquivo+offset a negociar.
    pub fn diario_com_imagem(&mut self, pular: u64, limite: u64) -> Result<Vec<(Evento, Vec<u8>)>> {
        self.log.ler_com_imagem(pular, limite)
    }

    /// Eventos de um registro especifico.
    pub fn historico(&mut self, rowid: RowId) -> Result<Vec<Evento>> {"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
