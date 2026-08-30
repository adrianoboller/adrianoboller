# Add RegFile::versao
# 28/08 23:51

import pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs")
s = p.read_text()
s = s.replace(
'''        assert!(!PhxError::Duplicado(String::new()).adianta_repetir());''',
'''        assert!(!PhxError::Duplicado(String::new()).adianta_repetir());
        // Repetir um conflito e escrever por cima do outro sem olhar. Quem
        // decide e gente, e nao um laco de nova tentativa.
        assert!(!PhxError::Conflito(String::new()).adianta_repetir());''')
p.write_text(s)

p = pathlib.Path("crates/phxsql-store/src/reg.rs")
s = p.read_text()
alvo = '''    /// Regrava o payload de um registro existente, no mesmo slot.'''
novo = '''    /// A versao do registro: quantas vezes ele foi regravado desde que nasceu.
    ///
    /// Devolve `None` quando o slot nao esta ativo -- registro nunca usado ou
    /// excluido de vez.
    ///
    /// Le so o cabecalho do slot, 24 bytes, e nao o payload: quem confere se
    /// pode gravar nao precisa do conteudo, e uma tabela com memo de
    /// megabytes cobraria o arquivo externo inteiro por uma pergunta de
    /// oito bytes.
    pub fn versao(&mut self, rowid: RowId) -> Result<Option<u64>> {
        self.conferir_faixa(rowid)?;
        let (volume, offset) = self.localizar(rowid);
        let mut cab = [0u8; SLOT_CAB];
        self.volumes.ler(volume, offset, &mut cab)?;
        if cab[0] != STATUS_ATIVO {
            return Ok(None);
        }
        Ok(Some(Campos(&cab).u64(8)))
    }

    /// Regrava o payload de um registro existente, no mesmo slot.'''
assert alvo in s
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
