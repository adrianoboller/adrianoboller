# Fix the empty-volume seeding
# 28/08 11:27

import pathlib
p = pathlib.Path('crates/phxsql-store/src/reg.rs')
s = p.read_text()

v = '''#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fronteira {
    pub primeiro_rowid: RowId,
    pub chave_periodo: i64,
}'''
n = '''#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fronteira {
    pub primeiro_rowid: RowId,
    pub chave_periodo: i64,
}

/// Volume aberto e ainda sem nenhuma linha: nao ha periodo para gravar.
///
/// Acontece na criacao da tabela -- o volume 1 nasce antes da primeira
/// insercao. O primeiro registro adota o volume em vez de cortar um novo, para
/// a tabela nao nascer com um arquivo vazio.
pub const SEM_PERIODO: i64 = i64::MIN;'''
assert s.count(v) == 1
s = s.replace(v, n)

# criar: semeia a fronteira do volume 1 quando a particao e por periodo
v = '''            recuperados: 0,
            fronteiras: Vec::new(),
        };
        r.volumes.criar(1)?;'''
n = '''            recuperados: 0,
            fronteiras: Vec::new(),
        };
        if r.esquema.paginacao().modo.periodo().is_some() {
            r.fronteiras.push(Fronteira {
                primeiro_rowid: 1,
                chave_periodo: SEM_PERIODO,
            });
        }
        r.volumes.criar(1)?;'''
assert s.count(v) == 1
s = s.replace(v, n)

# o corte: volume vazio adota o periodo em vez de cortar
v = '''        let corta = match self.fronteiras.last() {
            None => true,
            Some(f) => {
                let no_volume = rowid - f.primeiro_rowid;
                no_volume >= paginacao.registros_por_arquivo || f.chave_periodo != chave
            }
        };'''
n = '''        let corta = match self.fronteiras.last() {
            None => true,
            // Volume ainda vazio: ele ADOTA o periodo da primeira linha. Sem
            // isto a tabela nasceria com um volume 1 vazio e a primeira linha
            // iria para o volume 2.
            Some(f) if rowid == f.primeiro_rowid => {
                if f.chave_periodo != chave {
                    let ultimo = self.fronteiras.len() - 1;
                    self.fronteiras[ultimo].chave_periodo = chave;
                }
                false
            }
            Some(f) => {
                let no_volume = rowid - f.primeiro_rowid;
                no_volume >= paginacao.registros_por_arquivo || f.chave_periodo != chave
            }
        };'''
assert s.count(v) == 1
s = s.replace(v, n)

# a leitura tolera o volume semeado e ainda vazio
v = '''        // Um volume que existe mas nunca foi escrito na v3 vem com zero. Zero
        // nao e rowid: seria endereco 1 para tudo. Melhor recusar alto do que
        // devolver a linha errada em silencio.
        if let Some(i) = self.fronteiras.iter().position(|f| f.primeiro_rowid == 0) {'''
n = '''        // Um volume que existe mas nunca foi escrito na v3 vem com zero. Zero
        // nao e rowid: seria endereco 1 para tudo. Melhor recusar alto do que
        // devolver a linha errada em silencio.
        //
        // A tabela recem-criada e vazia nao cai aqui: o volume 1 dela ja nasce
        // com `primeiro_rowid = 1` e periodo indefinido.
        if let Some(i) = self.fronteiras.iter().position(|f| f.primeiro_rowid == 0) {'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
