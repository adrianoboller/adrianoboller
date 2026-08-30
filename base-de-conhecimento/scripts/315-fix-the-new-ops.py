# Fix the new ops
# 28/08 11:24

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()

# a permissao no destino, escrita com o que ja existe
v = '''        // A permissao de criar vale no destino. Sem esta linha, quem pode ler
        // um database e criar noutro conseguiria escrever onde nao devia --
        // ou, pior, o contrario.
        self.exigir(sessao, destino_db, "criar_tabela")?;
'''
n = '''        // O portao geral confere a permissao contra o database do campo
        // `database`, que aqui e a ORIGEM. O destino precisa da sua propria
        // conferencia: sem esta linha, quem pode ler um database e nao pode
        // criar no outro conseguiria escrever onde nao devia.
        if let Some(u) = &sessao.usuario {
            if !u.pode(destino_db, Atividade::Criar) {
                return Err(PhxError::Autorizacao(format!(
                    "sem permissao de criar em {destino_db}"
                )));
            }
        }
'''
assert s.count(v) == 1
s = s.replace(v, n)

s = s.replace('for nome in db.tabelas_qualificadas()? {', 'for nome in db.todas_as_tabelas()? {')
s = s.replace('("bytes_por_linha", Json::de_u64(t.slot_size() as u64)),',
              '("bytes_por_linha", Json::de_u64(e.payload_len() as u64)),')
s = s.replace('''            let Ok(t) = db.abrir_qualificada(&nome) else {
                continue;
            };''', '''            let Ok(t) = db.abrir_qualificada(&nome) else {
                continue;
            };''')
p.write_text(s)
print('ok')
