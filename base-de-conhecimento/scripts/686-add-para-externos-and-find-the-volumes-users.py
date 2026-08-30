# Add para_externos and find the Volumes users
# 28/08 18:58

import io
p='crates/phxsql-core/src/paginacao.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    /// Muda a largura do sufixo (por exemplo 4, para passar de 999 volumes).'''
novo='''    /// A mesma paginacao, mas para os arquivos que NAO sao particionados.
    ///
    /// So o `.reg` -- e o espelho `.bkp`, que e um clone dele -- se parte por
    /// letra. O `.bin`, o `.memo`, o `.log`, o `.trash` e o `.reason` rolam por
    /// TAMANHO, e continuam rolando: um `.log` que passa do volume 1 viraria
    /// `Clientes_B.log`, que se le como «o diario do balde B» e nao e -- e o
    /// diario e da tabela inteira.
    ///
    /// Entao eles voltam ao sufixo numerico, com tres digitos.
    pub fn para_externos(mut self) -> Paginacao {
        if self.modo.por_letra() {
            self.modo = ModoParticao::PorQuantidade;
            self.digitos = DIGITOS_PADRAO;
            self.max_arquivos = 10u32.pow(DIGITOS_PADRAO as u32) - 1;
        }
        self
    }

    /// Muda a largura do sufixo (por exemplo 4, para passar de 999 volumes).'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
