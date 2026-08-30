# Remove the duplicate accessor
# 28/08 13:59

import pathlib
p = pathlib.Path('crates/phxsql-store/src/table.rs')
s = p.read_text()
v = '''    /// Proximo numero que a sequencia desta tabela vai dar. Zero = nunca usada.
    pub fn sequencia_atual(&self) -> u64 {
        self.reg.sequencia_atual()
    }

    /// Qual coluna e a sequencia, se houver.
    pub fn coluna_sequencia(&self) -> Option<usize> {
        self.esquema.coluna_sequencia()
    }

'''
assert s.count(v) == 1
p.write_text(s.replace(v, ''))
