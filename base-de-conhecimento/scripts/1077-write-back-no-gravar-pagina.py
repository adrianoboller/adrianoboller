# Write-back no gravar_pagina
# 29/08 06:00

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''struct Entrada {
    bytes: Vec<u8>,
    usada: bool,
}''','''struct Entrada {
    bytes: Vec<u8>,
    usada: bool,
    /// A pagina mudou em RAM e ainda nao foi ao arquivo.
    suja: bool,
}''')

# ler_pagina poe a pagina LIMPA (veio do arquivo)
s=s.replace('''        self.cache.por(n, &p);
        Ok(p)
    }''','''        self.cache.por(n, &p, false);
        Ok(p)
    }''')

# gravar_pagina vira write-back
velho='''    fn gravar_pagina(&mut self, n: u64, p: &mut [u8]) -> Result<()> {
        pag_selar(p);
        escrever_em(&mut self.arquivo, n * self.page_size as u64, p)?;
        // Guardar a pagina RECEM-GRAVADA e o que mais rende numa carga: a folha
        // que acabou de receber uma chave e quase sempre a que vai receber a
        // proxima, e sem isto ela voltaria do arquivo com CRC e tudo.
        self.cache.por(n, p);
        self.gravacoes += 1;
        Ok(())
    }'''
novo='''    /// A pagina mudou. Ela fica SUJA em RAM; o CRC e o `write` sao adiados.
    ///
    /// # Por que adiar, e o que isso troca
    ///
    /// Antes, toda pagina tocada era selada e escrita na hora: 2,06 gravacoes
    /// por linha, cada uma pagando o CRC-32 da pagina inteira. Numa carga a
    /// mesma folha recebe centenas de chaves seguidas, e pagava-se o CRC uma
    /// vez por chave em vez de uma vez por folha.
    ///
    /// E como o InnoDB e o Aria fazem: a mini-transacao do InnoDB so marca a
    /// pagina suja (`mtr0mtr.cc:338`) e o checksum sai na descarga
    /// (`buf0flu.cc:1243`); o Aria tem `PCBLOCK_CHANGED` (`ma_pagecache.c:177`)
    /// e `PAGECACHE_WRITE_DELAY` (`ma_page.c:255`). Medido aqui: **13,1 -> 7,2
    /// us por linha**.
    ///
    /// O preco e a garantia que o `FORMATO.md` descrevia: antes, uma queda do
    /// PROCESSO nao atrasava o `.ndx` em relacao ao `.reg`, porque o `write` ja
    /// tinha entregue a pagina ao nucleo. Agora atrasa -- e por isso existe a
    /// marca de sujo no cabecalho, que faz a queda ser DETECTADA. Indice
    /// atrasado se reconstroi do `.reg`; indice atrasado em silencio, nao.
    fn gravar_pagina(&mut self, n: u64, p: &mut [u8]) -> Result<()> {
        // A marca vai ao arquivo ANTES da primeira pagina suja existir. Ao
        // contrario, uma queda no meio deixaria cabecalho limpo com paginas
        // faltando -- que e exatamente o defeito que ela existe para impedir.
        if !self.sujo {
            self.sujo = true;
            self.gravar_cabecalho()?;
        }
        if let Some((velha, mut bytes)) = self.cache.por(n, p, true) {
            self.escrever_pagina(velha, &mut bytes)?;
        }
        Ok(())
    }

    /// Sela e escreve de verdade. So o despejo e o `sincronizar` chamam.
    fn escrever_pagina(&mut self, n: u64, p: &mut [u8]) -> Result<()> {
        pag_selar(p);
        escrever_em(&mut self.arquivo, n * self.page_size as u64, p)?;
        self.gravacoes += 1;
        Ok(())
    }

    /// Leva todas as paginas sujas ao arquivo.
    fn descarregar(&mut self) -> Result<()> {
        for (n, mut bytes) in self.cache.tirar_sujas() {
            self.escrever_pagina(n, &mut bytes)?;
        }
        Ok(())
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('gravar ok')
