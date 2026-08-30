# Make server reload blacklist when changed externally
# 27/08 19:29

p='crates/phxsql-server/src/blacklist.rs'
s=open(p).read()
s=s.replace('''use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};''','''use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::SystemTime;''')
s=s.replace('''pub struct Blacklist {
    caminho: PathBuf,
    bloqueios: Vec<Bloqueio>,
    /// Carimbos das tentativas leves recentes, por IP. So em memoria.
    tentativas: HashMap<String, Vec<i64>>,
}''','''pub struct Blacklist {
    caminho: PathBuf,
    bloqueios: Vec<Bloqueio>,
    /// Carimbos das tentativas leves recentes, por IP. So em memoria.
    tentativas: HashMap<String, Vec<i64>>,
    /// Quando o arquivo foi gravado da ultima vez que o lemos.
    ///
    /// O `phxsqld --desbloquear` roda em OUTRO processo e mexe no mesmo
    /// arquivo. Sem isto, o servidor continuaria barrando um IP que ja saiu da
    /// lista -- foi exatamente o que aconteceu no primeiro teste ao vivo.
    lido_em: Option<SystemTime>,
}''')
s=s.replace('''        Ok(Blacklist {
            caminho,
            bloqueios,
            tentativas: HashMap::new(),
        })
    }''','''        let lido_em = mtime(&caminho);
        Ok(Blacklist {
            caminho,
            bloqueios,
            tentativas: HashMap::new(),
            lido_em,
        })
    }

    /// Rele o arquivo se alguem de fora mexeu nele. Devolve `true` se releu.
    ///
    /// Custa um `stat` por chamada, e e o que faz o `--desbloquear` de outro
    /// processo valer sem reiniciar o servidor.
    pub fn recarregar_se_mudou(&mut self) -> Result<bool> {
        let agora = mtime(&self.caminho);
        if agora == self.lido_em {
            return Ok(false);
        }
        let recarregada = Blacklist::abrir(&self.caminho)?;
        self.bloqueios = recarregada.bloqueios;
        self.lido_em = recarregada.lido_em;
        // As tentativas em memoria seguem: elas nao moram no arquivo.
        Ok(true)
    }''')
s=s.replace('''        std::fs::write(&self.caminho, doc.escrever_identado())?;
        Ok(())
    }''','''        std::fs::write(&self.caminho, doc.escrever_identado())?;
        Ok(())
    }

    /// Grava e anota o carimbo, para nao reler a propria escrita.
    fn gravar_e_marcar(&mut self) -> Result<()> {
        self.gravar()?;
        self.lido_em = mtime(&self.caminho);
        Ok(())
    }''')
s=s.replace('''        self.tentativas.remove(ip);
        if let Err(e) = self.gravar() {
            aviso = Some(format!("nao consegui gravar a blacklist: {e}"));
        }''','''        self.tentativas.remove(ip);
        if let Err(e) = self.gravar_e_marcar() {
            aviso = Some(format!("nao consegui gravar a blacklist: {e}"));
        }''')
s=s.replace('''        self.bloqueios.retain(|b| b.ip != ip);
        self.tentativas.remove(ip);
        self.gravar()?;
        if let Some(fw) = &politica.firewall {''','''        self.bloqueios.retain(|b| b.ip != ip);
        self.tentativas.remove(ip);
        self.gravar_e_marcar()?;
        if let Some(fw) = &politica.firewall {''')
s=s.replace('''        self.gravar()?;
        Ok(antes - self.bloqueios.len())''','''        self.gravar_e_marcar()?;
        Ok(antes - self.bloqueios.len())''')
s=s.replace('''#[cfg(test)]
mod tests {''','''/// Carimbo de alteracao do arquivo, ou `None` se ele nao existe.
fn mtime(caminho: &Path) -> Option<SystemTime> {
    std::fs::metadata(caminho).ok()?.modified().ok()
}

#[cfg(test)]
mod tests {''')
s=s.replace('''    #[test]
    fn desbloquear_tira_da_lista() {''','''    #[test]
    fn rele_o_arquivo_quando_outro_processo_mexe() {
        let d = dir_temp("recarrega");
        let caminho = d.join("blacklist.json");
        let p = politica();

        let mut servidor = Blacklist::abrir(&caminho).unwrap();
        servidor.violacao_grave("10.0.0.7", "excluir", "comando proibido", &p, T0);
        assert!(servidor.bloqueado("10.0.0.7", T0).is_some());

        // Outro processo -- o `phxsqld --desbloquear` -- tira o IP da lista.
        {
            let mut cli = Blacklist::abrir(&caminho).unwrap();
            assert!(cli.desbloquear("10.0.0.7", &p).unwrap());
        }
        // Sem reler, o servidor continuaria barrando.
        assert!(servidor.recarregar_se_mudou().unwrap(), "deveria ter relido");
        assert!(
            servidor.bloqueado("10.0.0.7", T0).is_none(),
            "o servidor tem de enxergar o desbloqueio feito de fora"
        );
        // Sem mudanca no arquivo, nao rele a toa.
        assert!(!servidor.recarregar_se_mudou().unwrap());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn nao_rele_a_propria_escrita() {
        let d = dir_temp("propria");
        let mut bl = Blacklist::abrir(d.join("blacklist.json")).unwrap();
        let p = politica();
        bl.violacao_grave("10.0.0.8", "excluir", "x", &p, T0);
        assert!(!bl.recarregar_se_mudou().unwrap(), "gravou ele mesmo");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn desbloquear_tira_da_lista() {''')
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('''            let _ = lista.limpar_vencidos(agora, &self.config.politica);''','''            // Outro processo pode ter mexido no arquivo (phxsqld --desbloquear).
            let _ = lista.recarregar_se_mudou();
            let _ = lista.limpar_vencidos(agora, &self.config.politica);''')
open(p,'w').write(s)
