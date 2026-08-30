# Add inserir_lote to the Table
# 28/08 19:18

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    /// Le uma linha completa, carregando `.bin` e `.memo`.'''
novo='''    /// Insere varias linhas de uma vez.
    ///
    /// # De onde vem o ganho
    ///
    /// **Nao e do disco.** Cada linha custa o mesmo aqui dentro: montar o
    /// payload, conferir a unicidade, gravar o slot, inserir a chave em cada
    /// indice. Nao ha atalho -- e a insercao ja e o caminho mais caro do
    /// motor, com 65% do tempo na manutencao do `.ndx`.
    ///
    /// O ganho e de tudo que ACONTECIA POR LINHA e passa a acontecer uma vez:
    /// abrir a tabela (sete arquivos), tomar a trava, e o `fsync`. Pela rede
    /// isso dominava -- vinte mil insercoes eram vinte mil aberturas.
    ///
    /// # Nao ha transacao, e isso muda o que se pode prometer
    ///
    /// Se a linha 700 de mil falhar, as 699 anteriores **ficam gravadas**. Nao
    /// ha como desfazer: o `.reg` nao reaproveita slot, entao "desfazer" seria
    /// deixar 699 buracos. Por isso o padrao e `parar_no_erro`: entre uma
    /// carga que para na linha 700 e uma que grava 999 linhas com uma faltando
    /// no meio, a primeira e a que da para consertar.
    ///
    /// Quem esta importando dado sujo de proposito passa `false` e recebe a
    /// lista do que ficou de fora, com o numero da linha.
    pub fn inserir_lote(&mut self, linhas: &[Linha], parar_no_erro: bool) -> Result<Lote> {
        let mut lote = Lote {
            rowids: Vec::with_capacity(linhas.len()),
            recusadas: Vec::new(),
        };
        for (i, linha) in linhas.iter().enumerate() {
            match self.inserir(linha) {
                Ok(r) => lote.rowids.push(r),
                Err(e) => {
                    lote.recusadas.push((i, e.to_string()));
                    if parar_no_erro {
                        return Ok(lote);
                    }
                }
            }
        }
        Ok(lote)
    }

    /// Le uma linha completa, carregando `.bin` e `.memo`.'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''/// O que uma varredura enxerga.'''
novo2='''/// O que saiu de uma carga em lote.
#[derive(Debug, Clone, Default)]
pub struct Lote {
    /// Os rowids gravados, na ordem em que as linhas chegaram.
    pub rowids: Vec<RowId>,
    /// As que ficaram de fora: `(posicao na lista, motivo)`.
    ///
    /// A POSICAO, e nao o rowid: a linha recusada nao tem rowid, e quem mandou
    /// a carga precisa achar a linha no arquivo dele para consertar.
    pub recusadas: Vec<(usize, String)>,
}

/// O que uma varredura enxerga.'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
