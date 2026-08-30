# Add the rownum system column
# 28/08 18:26

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    /// Empurra o contador para depois de um valor gravado a mao.'''
novo='''    /// Toma o proximo `rownum` e avanca o contador.
    ///
    /// Diferente da sequencia em duas coisas que importam: nao se escreve a
    /// mao, e nao se ajusta. E o numero de ORDEM de chegada da linha, e o
    /// unico jeito de ele estar certo e o motor ser o unico a mexer nele.
    ///
    /// Nunca reaproveita, nem depois de exclusao -- pela mesma razao que o
    /// slot nao reaproveita: se reaproveitasse, um cursor parado numa pagina
    /// veria linha nova aparecer ATRAS de onde ele esta, e a paginacao passaria
    /// a pular registro sem avisar.
    pub fn proximo_do_rownum(&mut self) -> u64 {
        let v = self.proximo_rownum.max(1);
        self.proximo_rownum = v + 1;
        v
    }

    /// Proximo `rownum` que a tabela vai entregar.
    pub fn rownum_atual(&self) -> u64 {
        self.proximo_rownum.max(1)
    }

    /// Empurra o contador para depois de um valor gravado a mao.'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
