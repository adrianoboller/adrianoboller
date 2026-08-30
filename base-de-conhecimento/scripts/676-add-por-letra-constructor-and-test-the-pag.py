# Add por_letra constructor and test the .pag
# 28/08 18:50

import io
p='crates/phxsql-core/src/paginacao.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    /// Muda a largura do sufixo (por exemplo 4, para passar de 999 volumes).'''
novo='''    /// Particao ALFANUMERICA: 37 volumes fixos, um por letra inicial.
    ///
    /// `registros_por_arquivo` passa a ser o teto POR LETRA, e nao da tabela.
    /// Dimensionar isto e a decisao que a tabela pede: num cadastro brasileiro
    /// o `_S` costuma ter dez vezes o `_K`, e quem enche primeiro derruba a
    /// insercao daquela letra -- com as outras 36 ainda vazias.
    pub fn por_letra(registros_por_arquivo: u64, coluna: u16) -> Result<Paginacao> {
        Paginacao {
            registros_por_arquivo,
            max_arquivos: BALDES.len() as u32,
            // Dois digitos porque 37 cabe neles. O sufixo desta particao e a
            // LETRA, e nao o numero -- mas `digitos` continua sendo o que
            // `ligada()` olha, e zero desligaria a paginacao inteira.
            digitos: 2,
            bytes_por_arquivo: BYTES_POR_ARQUIVO_PADRAO,
            modo: ModoParticao::PorLetra { coluna },
        }
        .validada()
    }

    /// Muda a largura do sufixo (por exemplo 4, para passar de 999 volumes).'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
