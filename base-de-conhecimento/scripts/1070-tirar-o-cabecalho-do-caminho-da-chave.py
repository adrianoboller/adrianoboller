# Tirar o cabecalho do caminho da chave
# 29/08 05:18

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
velho='''            cache: CachePaginas::nova(cache_paginas()),
            gravacoes: 0,
        })
    }'''
novo='''            cache: CachePaginas::nova(cache_paginas()),
            gravacoes: 0,
            estrutura_mudou: false,
        })
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# 3. alocar_pagina levanta o sinalizador -- as duas saidas
velho2='''            self.cache.esquecer(n);
            return Ok(n);
        }
        let n = self.qtd_paginas;
        self.qtd_paginas += 1;
        self.arquivo
            .set_len(self.qtd_paginas * self.page_size as u64)?;
        Ok(n)
    }'''
novo2='''            self.cache.esquecer(n);
            self.estrutura_mudou = true;
            return Ok(n);
        }
        let n = self.qtd_paginas;
        self.qtd_paginas += 1;
        self.arquivo
            .set_len(self.qtd_paginas * self.page_size as u64)?;
        self.estrutura_mudou = true;
        Ok(n)
    }'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# 4. o caminho quente: so grava quando a estrutura mudou
velho3='''            self.gravar_pagina(nova_raiz, &mut p)?;
            self.indices[idx].raiz = nova_raiz;
        }
        self.indices[idx].qtd_chaves += 1;
        self.gravar_cabecalho()
    }'''
novo3='''            self.gravar_pagina(nova_raiz, &mut p)?;
            self.indices[idx].raiz = nova_raiz;
            self.estrutura_mudou = true;
        }
        self.indices[idx].qtd_chaves += 1;
        // O contador nao justifica 4 KiB por chave: ele vai no `sincronizar`,
        // e `verificar` sabe recalcula-lo. A ESTRUTURA vai na hora.
        if self.estrutura_mudou {
            self.gravar_cabecalho()?;
        }
        Ok(())
    }'''
assert s.count(velho3)==1
s=s.replace(velho3,novo3)

# 5. gravar_cabecalho baixa o sinalizador, e sincronizar sempre grava
velho4='''        buf[CAB_LEN..CAB_LEN + dir.len()].copy_from_slice(&dir);
        escrever_em(&mut self.arquivo, 0, &buf)
    }'''
novo4='''        buf[CAB_LEN..CAB_LEN + dir.len()].copy_from_slice(&dir);
        escrever_em(&mut self.arquivo, 0, &buf)?;
        self.estrutura_mudou = false;
        Ok(())
    }'''
assert s.count(velho4)==1
s=s.replace(velho4,novo4)

velho5='''    pub fn sincronizar(&mut self) -> Result<()> {
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }'''
novo5='''    pub fn sincronizar(&mut self) -> Result<()> {
        // O cabecalho vai ANTES do `sync_all`, senao o contador que ele carrega
        // ficaria de fora justamente da gravacao que promete durabilidade.
        self.gravar_cabecalho()?;
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }'''
assert s.count(velho5)==1
s=s.replace(velho5,novo5)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
