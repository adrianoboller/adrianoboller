# Add metadata fields to Column
# 28/08 11:13

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()

v = '''#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub nome: String,
    pub ty: ColumnType,
    pub nullable: bool,
}

impl Column {
    pub fn new(nome: impl Into<String>, ty: ColumnType) -> Self {
        Column {
            nome: nome.into(),
            ty,
            nullable: true,
        }
    }

    /// Marca a coluna como obrigatoria (NOT NULL).
    pub fn obrigatoria(mut self) -> Self {
        self.nullable = false;
        self
    }
}'''

n = '''/// Uma coluna: o que ela guarda, e o que a tela precisa saber para exibi-la.
///
/// Os quatro campos de apresentacao -- `id`, `caption`, `descricao` e
/// `mascara` -- moram no `.reg` junto com o resto do esquema, e nao num
/// dicionario a parte. E a mesma razao de o esquema morar ali: a tabela tem de
/// se descrever sozinha. Um dicionario externo se perde, se desatualiza, e
/// obriga quem copia os cinco arquivos a copiar um sexto.
///
/// O `id` e um UUID v7 sorteado na criacao e **nunca reaproveitado**: e por
/// ele que uma tela, um relatorio ou um mapeamento se referem a coluna, para
/// que renomear a coluna nao quebre nada. Renomear troca o `nome`; o `id`
/// segue o mesmo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Identidade estavel da coluna. Sobrevive a renomear.
    pub id: Uuid,
    pub nome: String,
    /// Rotulo de tela. Vazio significa "use o nome".
    pub caption: String,
    /// Para que serve a coluna, em uma linha.
    pub descricao: String,
    /// Mascara de edicao e exibicao, no formato PICTURE do Clarion(R):
    /// `@N-11.2`, `@D6`, `@P###-####P`. Vazia = sem mascara.
    pub mascara: String,
    pub ty: ColumnType,
    pub nullable: bool,
}

impl Column {
    /// Coluna nova, com um `id` v7 recem-sorteado.
    pub fn new(nome: impl Into<String>, ty: ColumnType) -> Self {
        Column {
            id: Uuid::v7(),
            nome: nome.into(),
            caption: String::new(),
            descricao: String::new(),
            mascara: String::new(),
            ty,
            nullable: true,
        }
    }

    /// Marca a coluna como obrigatoria (NOT NULL).
    pub fn obrigatoria(mut self) -> Self {
        self.nullable = false;
        self
    }

    /// Fixa o `id` -- para reabrir uma coluna que ja existe, nao para criar.
    pub fn com_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    pub fn com_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn com_descricao(mut self, descricao: impl Into<String>) -> Self {
        self.descricao = descricao.into();
        self
    }

    pub fn com_mascara(mut self, mascara: impl Into<String>) -> Self {
        self.mascara = mascara.into();
        self
    }

    /// O rotulo que a tela deve mostrar: o caption, ou o nome se nao houver.
    pub fn rotulo(&self) -> &str {
        if self.caption.is_empty() {
            &self.nome
        } else {
            &self.caption
        }
    }
}'''
assert s.count(v) == 1
s = s.replace(v, n)
s = s.replace('use crate::types::ColumnType;', 'use crate::types::ColumnType;\nuse crate::uuid::Uuid;')
p.write_text(s)
print('ok')
