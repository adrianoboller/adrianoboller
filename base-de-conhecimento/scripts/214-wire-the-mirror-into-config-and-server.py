# Wire the mirror into config and server
# 27/08 21:46

import os
p='crates/phxsql-server/src/config.rs'
s=open(p).read()
s=s.replace('''    /// Recusa qualquer operacao de escrita.
    pub somente_leitura: bool,''','''    /// Recusa qualquer operacao de escrita.
    pub somente_leitura: bool,
    /// Espelha todo `.reg` num `.bkp` irmao -- a segunda chance.
    ///
    /// Custa uma escrita a mais por gravacao e o dobro de espaco do `.reg`.
    /// Protege contra o dado ficar RUIM, nao contra o disco morrer: os dois
    /// arquivos moram no mesmo lugar.
    pub espelho: bool,''')
s=s.replace('''            somente_leitura: false,
            replicacao: Replicacao::default(),''','''            somente_leitura: false,
            espelho: false,
            replicacao: Replicacao::default(),''')
s=s.replace('''            somente_leitura: j.booleano_ou("somente_leitura", false),
            replicacao: rep,''','''            somente_leitura: j.booleano_ou("somente_leitura", false),
            espelho: j.booleano_ou("espelho", false),
            replicacao: rep,''')
s=s.replace('''            ("somente_leitura", Json::Bool(self.somente_leitura)),''',
            '''            ("somente_leitura", Json::Bool(self.somente_leitura)),
            ("espelho", Json::Bool(self.espelho)),''')
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('''        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = dados.abrir_database(database)?.abrir_qualificada(tabela)?;''',
'''        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = dados.abrir_database(database)?.abrir_qualificada(tabela)?;
        // O espelho e decisao do servidor, nao da tabela: ligar no config.json
        // vale para tudo que este servidor abrir daqui para a frente.
        if self.config.espelho && !t.tem_espelho() {
            t.espelhar()?;
        }''')
s=s.replace('''            "backup" => self.op_backup(p, sessao),''','''            "backup" => self.op_backup(p, sessao),
            "reparar" => self.op_reparar(p, sessao),''')
s=s.replace('''    fn op_conferir_backup(&self, p: &Json) -> Result<Json> {''','''    /// Confere `.reg` contra `.bkp` e conserta o que der.
    fn op_reparar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let (conferidos, reparados, perdidos) = t.reparar()?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("conferidos", Json::de_u64(conferidos)),
            ("reparados", Json::de_u64(reparados)),
            ("perdidos", Json::de_u64(perdidos)),
            ("integro", Json::Bool(perdidos == 0)),
        ]))
    }

    fn op_conferir_backup(&self, p: &Json) -> Result<Json> {''')
open(p,'w').write(s)
