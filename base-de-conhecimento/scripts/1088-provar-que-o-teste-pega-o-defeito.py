# Provar que o teste pega o defeito
# 29/08 06:06

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
# volta o defeito: ler_pagina joga o despejo fora
s=s.replace('''        self.guardar_no_cache(n, &p, false)?;
        Ok(p)''','''        let _ = self.cache.por(n, &p, false); // DEFEITO, so para provar o teste
        Ok(p)''')
io.open(p,'w',encoding='utf-8').write(s)
