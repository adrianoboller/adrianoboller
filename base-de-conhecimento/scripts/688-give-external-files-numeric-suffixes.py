# Give external files numeric suffixes
# 28/08 18:58

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
# criar
velho='''        let bin = BlobFile::criar(&diretorio, &nome, EXT_BIN, MAGIC_BIN, paginacao)?;
        let memo = BlobFile::criar(&diretorio, &nome, EXT_MEMO, MAGIC_MEMO, paginacao)?;
        let log = LogFile::criar(&diretorio, &nome, paginacao)?;
        let lixeira = LixeiraFile::criar(&diretorio, &nome, paginacao)?;
        let motivos = MotivoFile::criar(&diretorio, &nome, paginacao)?;'''
novo='''        // Os arquivos que NAO se partem por letra levam o sufixo numerico.
        // Ver `Paginacao::para_externos`: um `Clientes_B.log` se leria como o
        // diario do balde B, e o diario e da tabela inteira.
        let externos = paginacao.para_externos();
        let bin = BlobFile::criar(&diretorio, &nome, EXT_BIN, MAGIC_BIN, externos)?;
        let memo = BlobFile::criar(&diretorio, &nome, EXT_MEMO, MAGIC_MEMO, externos)?;
        let log = LogFile::criar(&diretorio, &nome, externos)?;
        let lixeira = LixeiraFile::criar(&diretorio, &nome, externos)?;
        let motivos = MotivoFile::criar(&diretorio, &nome, externos)?;'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''        let bin = BlobFile::abrir(&diretorio, nome, EXT_BIN, MAGIC_BIN, paginacao)?;
        let memo = BlobFile::abrir(&diretorio, nome, EXT_MEMO, MAGIC_MEMO, paginacao)?;
        let log = LogFile::abrir(&diretorio, nome, paginacao)?;'''
novo2='''        let externos = paginacao.para_externos();
        let bin = BlobFile::abrir(&diretorio, nome, EXT_BIN, MAGIC_BIN, externos)?;
        let memo = BlobFile::abrir(&diretorio, nome, EXT_MEMO, MAGIC_MEMO, externos)?;
        let log = LogFile::abrir(&diretorio, nome, externos)?;'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

velho3='''        let lixeira = LixeiraFile::abrir(&diretorio, nome, paginacao)?;
        let motivos = MotivoFile::abrir(&diretorio, nome, paginacao)?;'''
novo3='''        let lixeira = LixeiraFile::abrir(&diretorio, nome, externos)?;
        let motivos = MotivoFile::abrir(&diretorio, nome, externos)?;'''
assert velho3 in s
s=s.replace(velho3,novo3,1)
io.open(p,'w',encoding='utf-8').write(s)
