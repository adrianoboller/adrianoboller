# Campo sujo, byte 52 e a ordem do sincronizar
# 29/08 06:01

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()

# campo `sujo` na struct
velho='''    estrutura_mudou: bool,
}'''
novo='''    estrutura_mudou: bool,
    /// Ha pagina suja em RAM, e o cabecalho no disco ja diz isso.
    ///
    /// # A rede de seguranca do write-back
    ///
    /// Com paginas sujas, uma queda deixa o `.ndx` atrasado em relacao ao
    /// `.reg`: a arvore pode ter chave faltando. Isso, sozinho, seria o pior
    /// defeito possivel -- busca respondendo errado sem ninguem notar.
    ///
    /// A marca desfaz isso: ela vai ao cabecalho ANTES da primeira pagina suja
    /// e so sai depois de todas irem ao disco. Quem abre um `.ndx` com a marca
    /// levantada sabe que ele nao presta e recusa responder, mandando
    /// reconstruir -- que desde a 0.17.0 custa 0,31 s por milhao de chaves.
    ///
    /// E o mesmo desenho do Aria, que compra a garantia de volta com tres
    /// bytes de "nao fechei direito" (`ma_locking.c:460`) mais reparo na
    /// abertura, em vez do redo log do InnoDB.
    sujo: bool,
    /// O arquivo foi aberto com a marca de sujo: a arvore nao e confiavel.
    precisa_reconstruir: bool,
}'''
assert s.count(velho)==1
s=s.replace(velho,novo)

for a,b in [('''            gravacoes: 0,
            estrutura_mudou: false,
        };''','''            gravacoes: 0,
            estrutura_mudou: false,
            sujo: false,
            precisa_reconstruir: false,
        };'''),
            ('''            gravacoes: 0,
            estrutura_mudou: false,
        })''','''            gravacoes: 0,
            estrutura_mudou: false,
            sujo,
            precisa_reconstruir: sujo,
        })''')]:
    assert s.count(a)==1, a[:40]
    s=s.replace(a,b)

# o byte 52 do cabecalho carrega a marca
velho2='''        por_i64(&mut buf, 44, agora());'''
novo2='''        por_i64(&mut buf, 44, agora());
        // Byte 52: a marca de sujo. Ficava zerado ate a 0.17.0, e zero quer
        // dizer "limpo" -- entao um `.ndx` escrito antes desta versao continua
        // sendo lido com o significado certo, sem migracao.
        buf[52] = u8::from(self.sujo);'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# sincronizar: sujas -> fsync -> cabecalho limpo -> fsync
velho3='''    pub fn sincronizar(&mut self) -> Result<()> {
        // O cabecalho vai ANTES do `sync_all`, senao o contador que ele carrega
        // ficaria de fora justamente da gravacao que promete durabilidade.
        self.gravar_cabecalho()?;
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }'''
novo3='''    /// Leva tudo ao disco, e a ORDEM aqui e a garantia.
    ///
    /// Primeiro as paginas sujas e um `fsync` delas; so entao o cabecalho sem a
    /// marca de sujo, e outro `fsync`. Escrever o cabecalho limpo antes de as
    /// paginas estarem no disco abriria a janela exata que a marca existe para
    /// fechar: uma queda da maquina no meio deixaria "esta tudo bem" gravado
    /// por cima de uma arvore incompleta.
    ///
    /// Sao dois `fsync` por `sincronizar` -- que acontece uma vez por carga, e
    /// nao por linha.
    pub fn sincronizar(&mut self) -> Result<()> {
        self.descarregar()?;
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;

        self.sujo = false;
        self.gravar_cabecalho()?;
        self.arquivo.flush()?;
        self.arquivo.sync_all()?;
        Ok(())
    }

    /// O arquivo foi aberto com a marca de sujo levantada.
    ///
    /// A arvore pode ter chave faltando; reconstrua com `reindexar` antes de
    /// confiar em qualquer resposta dela.
    pub fn precisa_reconstruir(&self) -> bool {
        self.precisa_reconstruir
    }

    /// Recusa operar sobre um indice que ficou para tras numa queda.
    fn conferir_confiavel(&self) -> Result<()> {
        if self.precisa_reconstruir {
            return Err(PhxError::Corrompido(format!(
                "o indice de {} ficou para tras numa queda e nao e confiavel: \\
                 reconstrua com `reparar indice` antes de usar",
                self.caminho.display()
            )));
        }
        Ok(())
    }'''
assert s.count(velho3)==1
s=s.replace(velho3,novo3)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
