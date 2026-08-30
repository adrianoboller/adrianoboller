# Add Table version guard and build
# 28/08 23:52

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
alvo = '''    /// Regrava a linha inteira mantendo o mesmo rowid e a mesma posicao
    /// fisica no `.reg`.
    pub fn atualizar(&mut self, rowid: RowId, valores: &[Value]) -> Result<()> {'''
novo = '''    /// A versao do registro: 1 quando nasce, +1 a cada regravacao.
    ///
    /// `None` quer dizer slot inativo -- nunca usado, ou excluido de vez.
    pub fn versao(&mut self, rowid: RowId) -> Result<Option<u64>> {
        self.reg.versao(rowid)
    }

    /// Confere se o registro ainda esta na versao que quem vai gravar leu.
    ///
    /// # A janela
    ///
    /// Entre ler uma linha na tela e clicar em «salvar» passam segundos ou
    /// minutos. Se outra sessao gravar nesse intervalo, o segundo `atualizar`
    /// simplesmente escreve por cima: o trabalho do primeiro some, sem erro,
    /// sem registro, sem ninguem perceber. Isto e o que o HFSQL(R) chama de
    /// conflito de escrita, e a resposta dele nao e travar a linha na leitura
    /// -- e AVISAR na gravacao, mostrando os tres valores para quem decide.
    ///
    /// A peca ja estava no formato desde a v1 do `.reg`: cada slot guarda uma
    /// versao que sobe a cada regravacao. Conferir custa 24 bytes de leitura.
    ///
    /// # Por que nao e trava
    ///
    /// Travar na leitura resolveria o mesmo problema e criaria dois piores: a
    /// linha fica presa quando o cliente cai com a ficha aberta, e duas
    /// sessoes que travam em ordem trocada se abracam. O contador nao trava
    /// nada -- so recusa a segunda gravacao quando ela chegou depois de
    /// alguem ter mudado a linha.
    ///
    /// Excluida de vez conta como conflito, e nao como "nao encontrado":
    /// quem leu a linha ha um minuto quer saber que ela foi apagada, e nao
    /// que o rowid nunca existiu.
    pub fn conferir_versao(&mut self, rowid: RowId, esperada: u64) -> Result<()> {
        match self.reg.versao(rowid)? {
            Some(atual) if atual == esperada => Ok(()),
            Some(atual) => Err(PhxError::Conflito(format!(
                "o registro {rowid} de {} esta na versao {atual} e voce leu a \\
                 {esperada}: outra sessao gravou nesse meio-tempo",
                self.nome
            ))),
            None => Err(PhxError::Conflito(format!(
                "o registro {rowid} de {} foi excluido de vez depois que voce \\
                 o leu na versao {esperada}",
                self.nome
            ))),
        }
    }

    /// `atualizar` que so grava se ninguem tiver mexido desde a leitura.
    ///
    /// A conferencia e a gravacao acontecem sem soltar o `&mut self`, entao
    /// nao ha janela entre uma e outra dentro do processo.
    pub fn atualizar_se(&mut self, rowid: RowId, valores: &[Value], esperada: u64) -> Result<()> {
        self.conferir_versao(rowid, esperada)?;
        self.atualizar(rowid, valores)
    }

    /// Regrava a linha inteira mantendo o mesmo rowid e a mesma posicao
    /// fisica no `.reg`.
    pub fn atualizar(&mut self, rowid: RowId, valores: &[Value]) -> Result<()> {'''
assert alvo in s
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
