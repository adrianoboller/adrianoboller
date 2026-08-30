# Expose mirroring on Table
# 27/08 21:44

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()
s=s.replace('''    pub fn esquema(&self) -> &Schema {''','''    /// Liga o espelho `.bkp`. Chamado logo depois de abrir ou criar, e antes
    /// de qualquer escrita -- ligar no meio deixaria o espelho comecando pela
    /// metade, que e pior do que nao ter espelho nenhum.
    pub fn espelhar(&mut self) -> Result<()> {
        let volumes = std::mem::replace(
            &mut self.volumes,
            Volumes::novo(".", "", phxsql_core::EXT_REG, self.esquema.paginacao()),
        );
        self.volumes = volumes.com_espelho(phxsql_core::EXT_BKP);
        // O espelho pode nao existir ainda (tabela criada sem ele, ou primeira
        // vez). Semear com o conteudo atual e o que o torna util desde ja.
        for volume in self.volumes.existentes() {
            let tamanho = self.volumes.tamanho(volume)?;
            let mut buf = vec![0u8; tamanho as usize];
            self.volumes.ler(volume, 0, &mut buf)?;
            self.volumes.escrever_no_espelho(volume, 0, &buf)?;
        }
        self.volumes.sincronizar()?;
        Ok(())
    }

    pub fn tem_espelho(&self) -> bool {
        self.volumes.tem_espelho()
    }

    pub fn esquema(&self) -> &Schema {''')
open(p,'w').write(s)

p='crates/phxsql-store/src/table.rs'
s=open(p).read()
s=s.replace('''    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<Table> {''',
'''    /// Abre com o espelho `.bkp` ligado -- a segunda chance do `.reg`.
    pub fn abrir_espelhada(diretorio: impl AsRef<Path>, nome: &str) -> Result<Table> {
        let mut t = Table::abrir(diretorio, nome)?;
        t.reg.espelhar()?;
        Ok(t)
    }

    /// Cria com o espelho ligado desde o primeiro registro.
    pub fn criar_espelhada(diretorio: impl AsRef<Path>, esquema: Schema) -> Result<Table> {
        let mut t = Table::criar(diretorio, esquema)?;
        t.reg.espelhar()?;
        Ok(t)
    }

    /// Leituras que o espelho salvou nesta sessao. Zero e o esperado.
    pub fn recuperados(&self) -> u64 {
        self.reg.recuperados()
    }

    pub fn tem_espelho(&self) -> bool {
        self.reg.tem_espelho()
    }

    /// Confere os dois lados e conserta o que der. Ver `RegFile::reparar`.
    pub fn reparar(&mut self) -> Result<(u64, u64, u64)> {
        self.reg.reparar()
    }

    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<Table> {''')
open(p,'w').write(s)
