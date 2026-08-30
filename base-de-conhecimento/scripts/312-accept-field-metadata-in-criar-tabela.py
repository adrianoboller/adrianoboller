# Accept field metadata in criar_tabela
# 28/08 11:23

import pathlib
p = pathlib.Path('crates/phxsql-server/src/valores.rs')
s = p.read_text()
v = '''        let ty = tipo_de_texto(c.texto_ou("tipo", "Str(60)"))?;
        let col = Column::new(cn, ty);
        colunas.push(if c.booleano_ou("obrigatoria", false) {
            col.obrigatoria()
        } else {
            col
        });'''
n = '''        let ty = tipo_de_texto(c.texto_ou("tipo", "Str(60)"))?;
        let mut col = Column::new(cn, ty)
            .com_caption(c.texto_ou("caption", ""))
            .com_descricao(c.texto_ou("descricao", ""))
            .com_mascara(c.texto_ou("mascara", ""));
        // O `id` normalmente nasce aqui, sorteado. Aceitar um de fora existe
        // para UM caso: recriar uma tabela mantendo a identidade das colunas,
        // para que telas e relatorios que apontam para elas continuem valendo.
        let id = c.texto_ou("id", "").trim().to_string();
        if !id.is_empty() {
            col = col.com_id(
                Uuid::de_texto(&id)
                    .map_err(|e| PhxError::Esquema(format!("id da coluna {i}: {e}")))?,
            );
        }
        if c.booleano_ou("obrigatoria", false) {
            col = col.obrigatoria();
        }
        colunas.push(col);'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''            let d = IndexDef::new(inome, partes);
            indices.push(if idx.booleano_ou("unico", false) {
                d.unico()
            } else {
                d
            });'''
n = '''            let mut d = IndexDef::new(inome, partes);
            if idx.booleano_ou("unico", false) {
                d = d.unico();
            }
            // Primaria implica unica; o `primaria()` cuida disso.
            if idx.booleano_ou("primario", false) || idx.booleano_ou("primaria", false) {
                d = d.primaria();
            }
            indices.push(d);'''
assert s.count(v) == 1
s = s.replace(v, n)
s = s.replace('use phxsql_core::uuid::{Uuid, Uuid256};', 'use phxsql_core::uuid::{Uuid, Uuid256};')
p.write_text(s)
print('ok')
