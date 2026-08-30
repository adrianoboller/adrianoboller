# Add mirroring to the volume layer
# 27/08 21:43

p='crates/phxsql-store/src/volume.rs'
s=open(p).read()

s=s.replace('''pub struct Volumes {
    diretorio: PathBuf,
    nome: String,
    ext: &'static str,
    paginacao: Paginacao,
    abertos: HashMap<u32, File>,
    ordem: VecDeque<u32>,
    limite: usize,
}''','''pub struct Volumes {
    diretorio: PathBuf,
    nome: String,
    ext: &'static str,
    paginacao: Paginacao,
    abertos: HashMap<u32, File>,
    ordem: VecDeque<u32>,
    limite: usize,
    /// Espelho do conjunto, com outra extensao. Toda escrita vai para os dois.
    ///
    /// A leitura NAO consulta o espelho: quem le vai ao arquivo principal, e
    /// so quem descobre corrupcao pede a segunda chance de proposito. Ler
    /// sempre dos dois dobraria o custo de toda leitura para pagar por um caso
    /// que quase nunca acontece.
    espelho: Option<Box<Volumes>>,
}''')

s=s.replace('''            abertos: HashMap::new(),
            ordem: VecDeque::new(),
            limite: LIMITE_ABERTOS_PADRAO,
        }
    }''','''            abertos: HashMap::new(),
            ordem: VecDeque::new(),
            limite: LIMITE_ABERTOS_PADRAO,
            espelho: None,
        }
    }

    /// Liga o espelho: um conjunto irmao com outra extensao.
    ///
    /// # O que o espelho protege, e o que nao protege
    ///
    /// Protege contra o dado ficar RUIM: escrita pela metade, bit virado,
    /// defeito nosso que estraga um slot. Nesses casos o outro lado ainda tem
    /// a versao boa, e o `reparar` a devolve.
    ///
    /// NAO protege contra o disco morrer. Os dois arquivos estao no mesmo
    /// disco, no mesmo diretorio. Quem precisa sobreviver a perda do disco
    /// precisa de backup em outro lugar -- e por isso o backup existe
    /// separado, e nao e a mesma coisa.
    pub fn com_espelho(mut self, ext: &'static str) -> Volumes {
        let mut e = Volumes::novo(&self.diretorio, self.nome.clone(), ext, self.paginacao);
        e.limite = self.limite;
        self.espelho = Some(Box::new(e));
        self
    }

    pub fn tem_espelho(&self) -> bool {
        self.espelho.is_some()
    }

    /// Le do ESPELHO, quando o principal falhou. A segunda chance.
    pub fn ler_do_espelho(&mut self, volume: u32, offset: u64, buf: &mut [u8]) -> Result<()> {
        match &mut self.espelho {
            None => Err(PhxError::NaoEncontrado("esta tabela nao tem espelho".into())),
            Some(e) => e.ler(volume, offset, buf),
        }
    }

    /// Copia um trecho do principal para o espelho, para o reparo inverso.
    pub fn escrever_no_espelho(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        match &mut self.espelho {
            None => Err(PhxError::NaoEncontrado("esta tabela nao tem espelho".into())),
            Some(e) => {
                e.garantir(volume)?;
                e.escrever(volume, offset, buf)
            }
        }
    }''')

# escritas duplicam
s=s.replace('''    pub fn escrever(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(buf)?;
        Ok(())
    }''','''    pub fn escrever(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(buf)?;
        // O espelho recebe a mesma coisa, no mesmo lugar. Falhar aqui NAO
        // desfaz a escrita boa: o principal ja tem o dado, e um espelho
        // defasado e melhor do que uma gravacao recusada.
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            let g = e.arquivo(volume, true)?;
            g.seek(SeekFrom::Start(offset))?;
            g.write_all(buf)?;
        }
        Ok(())
    }''')
s=s.replace('''        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(cabecalho)?;
        f.write_all(conteudo)?;
        Ok(())
    }''','''        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(cabecalho)?;
        f.write_all(conteudo)?;
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            let g = e.arquivo(volume, true)?;
            g.seek(SeekFrom::Start(offset))?;
            g.write_all(cabecalho)?;
            g.write_all(conteudo)?;
        }
        Ok(())
    }''')
s=s.replace('''    pub fn definir_tamanho(&mut self, volume: u32, tamanho: u64) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.set_len(tamanho)?;
        Ok(())
    }''','''    pub fn definir_tamanho(&mut self, volume: u32, tamanho: u64) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.set_len(tamanho)?;
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            e.arquivo(volume, true)?.set_len(tamanho)?;
        }
        Ok(())
    }''')
s=s.replace('''    pub fn sincronizar(&mut self) -> Result<()> {
        for f in self.abertos.values_mut() {
            f.flush()?;
            f.sync_all()?;
        }
        Ok(())
    }''','''    pub fn sincronizar(&mut self) -> Result<()> {
        for f in self.abertos.values_mut() {
            f.flush()?;
            f.sync_all()?;
        }
        if let Some(e) = &mut self.espelho {
            e.sincronizar()?;
        }
        Ok(())
    }''')
s=s.replace('''    pub fn fechar_todos(&mut self) {
        self.abertos.clear();
        self.ordem.clear();
    }''','''    pub fn fechar_todos(&mut self) {
        self.abertos.clear();
        self.ordem.clear();
        if let Some(e) = &mut self.espelho {
            e.fechar_todos();
        }
    }''')
open(p,'w').write(s)
