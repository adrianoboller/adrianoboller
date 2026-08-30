# Caminho unico para o cache e remedir
# 29/08 06:05

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()

# um caminho unico para por no cache, que NUNCA perde o despejo
velho='''        self.cache.por(n, &p, false);
        Ok(p)
    }'''
novo='''        self.guardar_no_cache(n, &p, false)?;
        Ok(p)
    }

    /// Poe a pagina no cache e grava a que for despejada SUJA.
    ///
    /// Existe como caminho unico porque o despejo nao pode ser perdido em
    /// nenhum dos dois lados. Foi assim que o primeiro write-back quebrou: o
    /// `ler_pagina` chamava o cache e jogava fora o retorno, entao uma pagina
    /// recem-alocada que so existia suja em RAM era despejada e sumia -- o
    /// arquivo ficava com os zeros do `set_len`, e a leitura seguinte batia num
    /// CRC invalido. A suite inteira passou; quem pegou foi a medicao.
    fn guardar_no_cache(&mut self, n: u64, p: &[u8], suja: bool) -> Result<()> {
        if let Some((velha, mut bytes)) = self.cache.por(n, p, suja) {
            self.escrever_pagina(velha, &mut bytes)?;
        }
        Ok(())
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2='''        if let Some((velha, mut bytes)) = self.cache.por(n, p, true) {
            self.escrever_pagina(velha, &mut bytes)?;
        }
        Ok(())
    }'''
novo2='''        self.guardar_no_cache(n, p, true)
    }'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
