# Route all syncs through the window
# 28/08 13:49

import pathlib, re
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '            dados: Mutex::new(instancia),'
assert s.count(v) == 1
s = s.replace(v, '            janela: Janela::nova(&config.recursos),\n' + v)

# ------------------------------------------------- o sincronizar passa pela janela
# Todas as gravacoes chamam `t.sincronizar()?;` -- vira uma chamada que decide.
antes = s.count('        t.sincronizar()?;')
s = s.replace('        t.sincronizar()?;', '        self.gravar_de_verdade(&mut t)?;')
print(f'{antes} chamadas de sincronizar viraram gravar_de_verdade')

AJUDANTE = '''    /// Fecha a gravacao no disco, se a janela de durabilidade mandar.
    ///
    /// Chamado depois de toda escrita. Em `por_operacao` sincroniza sempre --
    /// e o que o servidor fazia. Em `por_lote` sincroniza quando a janela
    /// fecha, e o `fsync` de uma vale por todas as da janela. Em `sistema`
    /// nunca sincroniza aqui: o `write` ja aconteceu, e o resto e com o
    /// sistema operacional.
    fn gravar_de_verdade(&self, t: &mut Table) -> Result<()> {
        if self.janela.hora_de_gravar() {
            t.sincronizar()?;
        }
        Ok(())
    }

'''
marca = '''    /// Cria um schema -- uma pasta dentro do database.'''
assert s.count(marca) == 1
s = s.replace(marca, AJUDANTE + marca, 1)

s = s.replace('use std::sync::atomic::{AtomicUsize, Ordering};',
              'use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};')
s = s.replace('use crate::config::Config;', 'use crate::config::{Config, Durabilidade};')
p.write_text(s)
