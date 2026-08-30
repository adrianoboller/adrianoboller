# Add PapelDeChave and the derivation
# 28/08 11:14

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()
v = '''    pub fn indices(&self) -> &[IndexDef] {'''
n = '''    /// O indice marcado como chave primaria, se houver.
    pub fn chave_primaria(&self) -> Option<&IndexDef> {
        self.indices.iter().find(|i| i.primario)
    }

    /// O papel de uma coluna nas chaves da tabela.
    ///
    /// Nao e campo gravado: sai dos indices e das chaves estrangeiras, que sao
    /// a verdade. Guardar "e primaria" na coluna criaria uma segunda verdade
    /// que pode discordar da primeira -- e um dia discordaria.
    pub fn papel_da_coluna(&self, i: usize) -> PapelDeChave {
        let na_pk = self.chave_primaria().filter(|k| pertence(k, i));
        let fks: Vec<&ForeignKey> = self
            .chaves_estrangeiras
            .iter()
            .filter(|fk| fk.colunas.contains(&i))
            .collect();
        PapelDeChave {
            primaria: na_pk.is_some(),
            // Composta se a chave de que ela participa tem mais de uma coluna.
            primaria_composta: na_pk.map(IndexDef::composta).unwrap_or(false),
            estrangeira: !fks.is_empty(),
            estrangeira_composta: fks.iter().any(|fk| fk.colunas.len() > 1),
            chaves_estrangeiras: fks.iter().map(|fk| fk.nome.clone()).collect(),
            indices: self
                .indices
                .iter()
                .filter(|idx| pertence(idx, i))
                .map(|idx| idx.nome.clone())
                .collect(),
        }
    }

    pub fn indices(&self) -> &[IndexDef] {'''
assert s.count(v) == 1
s = s.replace(v, n)

# a struct e o ajudante
v = '''/// Uma coluna: o que ela guarda, e o que a tela precisa saber para exibi-la.'''
n = '''/// O que uma coluna e dentro das chaves da tabela.
///
/// Tudo aqui e DERIVADO dos indices e das chaves estrangeiras -- nada disso e
/// gravado na coluna. Marcar "primaria" no proprio campo criaria uma segunda
/// verdade ao lado do indice, e as duas divergiriam no primeiro `ALTER`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PapelDeChave {
    pub primaria: bool,
    /// A chave primaria de que participa tem mais de uma coluna.
    pub primaria_composta: bool,
    pub estrangeira: bool,
    /// Alguma chave estrangeira de que participa tem mais de uma coluna.
    pub estrangeira_composta: bool,
    pub chaves_estrangeiras: Vec<String>,
    /// Todos os indices em que a coluna aparece, primario incluido.
    pub indices: Vec<String>,
}

fn pertence(idx: &IndexDef, coluna: usize) -> bool {
    idx.colunas.iter().any(|ic| ic.coluna == coluna)
}

/// Uma coluna: o que ela guarda, e o que a tela precisa saber para exibi-la.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
