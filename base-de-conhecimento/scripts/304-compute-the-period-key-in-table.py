# Compute the period key in Table
# 28/08 11:19

import pathlib
p = pathlib.Path('crates/phxsql-store/src/table.rs')
s = p.read_text()
v = '''        let rowid = self.reg.inserir(&payload)?;'''
n = '''        let rowid = self.reg.inserir_no_periodo(&payload, self.chave_do_periodo(valores)?)?;'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    pub fn inserir(&mut self, valores: &[Value]) -> Result<RowId> {'''
n = '''    /// Em que periodo esta linha cai, quando a tabela e particionada por data.
    ///
    /// `None` na particao por quantidade -- ali o volume sai de divisao e a
    /// data nao tem nada a ver com o assunto.
    fn chave_do_periodo(&self, valores: &[Value]) -> Result<Option<i64>> {
        let modo = self.esquema.paginacao().modo;
        let (Some(periodo), Some(i)) = (modo.periodo(), modo.coluna()) else {
            return Ok(None);
        };
        let dias = match valores.get(i) {
            Some(Value::Date(d)) => *d,
            // DateTime e milissegundos; vira dia por divisao inteira, com
            // `div_euclid` para que datas antes de 1970 nao arredondem para o
            // lado errado.
            Some(Value::DateTime(ms)) => (ms.div_euclid(86_400_000)) as i32,
            outro => {
                return Err(PhxError::Tipo(format!(
                    "a coluna de particao {} precisa de uma data; recebi {outro:?}",
                    self.esquema.colunas()[i].nome
                )))
            }
        };
        let (ano, mes, _) = civil_de_dias(dias);
        Ok(Some(periodo.chave(ano, mes)))
    }

    pub fn inserir(&mut self, valores: &[Value]) -> Result<RowId> {'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
