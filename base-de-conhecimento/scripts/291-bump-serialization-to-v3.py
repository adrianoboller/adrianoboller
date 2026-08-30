# Bump serialization to v3
# 28/08 11:14

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()

s = s.replace('const VERSAO_ESQUEMA: u16 = 2;',
'''/// Versao do bloco de esquema gravado no `.reg`.
///
/// A 3 acrescentou os metadados de coluna (`id`, `caption`, `descricao`,
/// `mascara`), o marcador de chave primaria no indice e o modo de particao.
/// A leitura ainda aceita a 2: tabela gravada antes abre, ganha um `id` v7
/// sorteado na hora e os textos vazios. Escrever, so na 3.
const VERSAO_ESQUEMA: u16 = 3;
const VERSAO_ESQUEMA_MINIMA: u16 = 2;''')

# ------------------------------------------------------------ serializar
v = '''        out.extend_from_slice(&(self.colunas.len() as u16).to_le_bytes());
        for c in &self.colunas {
            escrever_texto(&mut out, &c.nome);
            let (a, b) = c.ty.params();
            out.push(c.ty.tag());
            out.extend_from_slice(&a.to_le_bytes());
            out.push(b);
            out.push(c.nullable as u8);
        }'''
n = '''        out.extend_from_slice(&(self.colunas.len() as u16).to_le_bytes());
        for c in &self.colunas {
            escrever_texto(&mut out, &c.nome);
            let (a, b) = c.ty.params();
            out.push(c.ty.tag());
            out.extend_from_slice(&a.to_le_bytes());
            out.push(b);
            out.push(c.nullable as u8);
            // v3: os metadados de apresentacao, na mesma ordem em que a tela
            // pede por eles.
            out.extend_from_slice(c.id.bytes());
            escrever_texto(&mut out, &c.caption);
            escrever_texto(&mut out, &c.descricao);
            escrever_texto(&mut out, &c.mascara);
        }'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''            escrever_texto(&mut out, &idx.nome);
            out.push(idx.unico as u8);'''
n = '''            escrever_texto(&mut out, &idx.nome);
            // Dois sinalizadores num byte: unico no bit 0, primario no 1.
            out.push((idx.unico as u8) | ((idx.primario as u8) << 1));'''
assert s.count(v) == 1
s = s.replace(v, n)

# ---------------------------------------------------------- desserializar
v = '''        let versao = leitor.u16()?;
        if versao != VERSAO_ESQUEMA {
            return Err(PhxError::Esquema(format!(
                "versao de esquema {versao} nao suportada"
            )));
        }'''
n = '''        let versao = leitor.u16()?;
        if !(VERSAO_ESQUEMA_MINIMA..=VERSAO_ESQUEMA).contains(&versao) {
            return Err(PhxError::Esquema(format!(
                "versao de esquema {versao} nao suportada \\
                 (este motor le da {VERSAO_ESQUEMA_MINIMA} a {VERSAO_ESQUEMA})"
            )));
        }'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''            let nullable = leitor.u8()? != 0;
            colunas.push(Column {
                nome,
                ty: ColumnType::de_tag(tag, a, b)?,
                nullable,
            });'''
n = '''            let nullable = leitor.u8()? != 0;
            // Tabela gravada na v2 nao tem metadados: ganha um id novo e
            // textos vazios, e passa a ter os campos assim que for regravada.
            let (id, caption, descricao, mascara) = if versao >= 3 {
                (
                    Uuid::de_bytes(leitor.bytes(16)?),
                    leitor.texto()?,
                    leitor.texto()?,
                    leitor.texto()?,
                )
            } else {
                (Uuid::v7(), String::new(), String::new(), String::new())
            };
            colunas.push(Column {
                id,
                nome,
                caption,
                descricao,
                mascara,
                ty: ColumnType::de_tag(tag, a, b)?,
                nullable,
            });'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''            let nome = leitor.texto()?;
            let unico = leitor.u8()? != 0;
            let n = leitor.u16()? as usize;'''
n = '''            let nome = leitor.texto()?;
            let sinais = leitor.u8()?;
            let (unico, primario) = (sinais & 1 != 0, sinais & 2 != 0);
            let n = leitor.u16()? as usize;'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''            indices.push(IndexDef {
                nome,
                colunas: cols,
                unico,
            });'''
n = '''            indices.push(IndexDef {
                nome,
                colunas: cols,
                unico,
                primario,
            });'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
