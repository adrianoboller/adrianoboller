# Serialize the partition mode
# 28/08 11:16

import pathlib
p = pathlib.Path('crates/phxsql-core/src/paginacao.rs')
s = p.read_text()
v = '''    /// Muda o tamanho de cada volume dos arquivos externos.'''
n = '''    /// Troca a regra de corte do volume para o calendario.
    ///
    /// O `registros_por_arquivo` continua valendo como **teto**: um periodo de
    /// movimento intenso corta o volume ao encher, mesmo antes de o periodo
    /// virar. Sem isso um unico mes poderia estourar o arquivo, e a paginacao
    /// existe justamente para que isso nao aconteca.
    pub fn com_modo(mut self, modo: ModoParticao) -> Result<Paginacao> {
        self.modo = modo;
        self.validada()
    }

    /// Muda o tamanho de cada volume dos arquivos externos.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))

p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()
v = '''        let paginacao = Paginacao {
            registros_por_arquivo: leitor.u64()?,
            max_arquivos: leitor.u32()?,
            digitos: leitor.u8()?,
            bytes_por_arquivo: leitor.u64()?,
        };'''
n = '''        let mut paginacao = Paginacao {
            registros_por_arquivo: leitor.u64()?,
            max_arquivos: leitor.u32()?,
            digitos: leitor.u8()?,
            bytes_por_arquivo: leitor.u64()?,
            modo: ModoParticao::PorQuantidade,
        };
        if versao >= 3 {
            paginacao.modo = ModoParticao::de_tag(leitor.u8()?, leitor.u16()?)?;
        }'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''        out.push(p.digitos);
        out.extend_from_slice(&p.bytes_por_arquivo.to_le_bytes());
        out'''
n = '''        out.push(p.digitos);
        out.extend_from_slice(&p.bytes_por_arquivo.to_le_bytes());
        let (tag, coluna) = p.modo.tag();
        out.push(tag);
        out.extend_from_slice(&coluna.to_le_bytes());
        out'''
assert s.count(v) == 1
s = s.replace(v, n)
s = s.replace('use crate::paginacao::Paginacao;', 'use crate::paginacao::{ModoParticao, Paginacao};')
p.write_text(s)
print('ok')
