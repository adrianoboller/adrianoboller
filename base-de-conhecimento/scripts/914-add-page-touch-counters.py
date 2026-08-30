# Add page-touch counters
# 29/08 00:15

import pathlib
p = pathlib.Path("crates/phxsql-store/src/ndx.rs")
s = p.read_text()
s = s.replace('''    indices: Vec<DescritorIndice>,
    cache: CachePaginas,
}''','''    indices: Vec<DescritorIndice>,
    cache: CachePaginas,
    gravacoes: u64,
}''',1)
s = s.replace('''            indices: Vec::new(),
            cache: CachePaginas::nova(PAGINAS_EM_CACHE),
        };''','''            indices: Vec::new(),
            cache: CachePaginas::nova(PAGINAS_EM_CACHE),
            gravacoes: 0,
        };''',1)
s = s.replace('''            indices,
            cache: CachePaginas::nova(PAGINAS_EM_CACHE),
        })''','''            indices,
            cache: CachePaginas::nova(PAGINAS_EM_CACHE),
            gravacoes: 0,
        })''',1)
s = s.replace('''        self.cache.por(n, p);
        Ok(())
    }''','''        self.cache.por(n, p);
        self.gravacoes += 1;
        Ok(())
    }

    /// Quantas paginas o cache serviu, quantas vieram do arquivo, e quantas
    /// foram gravadas.
    ///
    /// Existe para o medidor nao ter de CITAR um `strace` de outro dia: o
    /// numero de toques de pagina por linha inserida e medido aqui dentro, e
    /// envelhece junto com o codigo em vez de envelhecer calado.
    pub fn estatisticas_paginas(&self) -> (u64, u64, u64) {
        (self.cache.acertos, self.cache.faltas, self.gravacoes)
    }''',1)
p.write_text(s)

p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
alvo = '''    pub fn paginas_indice(&self) -> u64 {'''
novo = '''    /// Do `.ndx`: paginas servidas pelo cache, lidas do arquivo, e gravadas.
    pub fn estatisticas_paginas(&self) -> (u64, u64, u64) {
        self.ndx.estatisticas_paginas()
    }

    pub fn paginas_indice(&self) -> u64 {'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
