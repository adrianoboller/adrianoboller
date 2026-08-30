# Fix the seeding bug and retest
# 27/08 21:46

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()
velho = '''        // O espelho pode nao existir ainda (tabela criada sem ele, ou primeira
        // vez). Semear com o conteudo atual e o que o torna util desde ja.
        for volume in self.volumes.existentes() {
            let tamanho = self.volumes.tamanho(volume)?;
            let mut buf = vec![0u8; tamanho as usize];
            self.volumes.ler(volume, 0, &mut buf)?;
            self.volumes.escrever_no_espelho(volume, 0, &buf)?;
        }
        self.volumes.sincronizar()?;
        Ok(())'''
novo = '''        // Semeia SO o que ainda nao existe do outro lado.
        //
        // Copiar por cima de um espelho que ja existe seria destruir a copia
        // boa com a principal, que e exatamente o contrario do que ele serve.
        // Um teste pegou isso: estragar o principal e religar o espelho
        // apagava a segunda chance. Espelho fora de sincronia se acerta com
        // `reparar`, que olha os dois lados antes de escrever em qualquer um.
        for volume in self.volumes.existentes() {
            let tamanho = self.volumes.tamanho(volume)?;
            if self.volumes.tamanho_do_espelho(volume)? == tamanho {
                continue; // ja existe e tem o tamanho certo: nao toca
            }
            let mut buf = vec![0u8; tamanho as usize];
            self.volumes.ler(volume, 0, &mut buf)?;
            self.volumes.escrever_no_espelho(volume, 0, &buf)?;
        }
        self.volumes.sincronizar()?;
        Ok(())'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
