# Route inserts to the right bucket
# 28/08 18:47

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let payload = self.montar_payload(valores)?;
        let ponteiros = self.ponteiros(&payload)?;
        let rowid = self
            .reg
            .inserir_no_periodo(&payload, self.chave_do_periodo(valores)?)?;'''
novo='''        let payload = self.montar_payload(valores)?;
        let ponteiros = self.ponteiros(&payload)?;
        let rowid = match self.balde_da_linha(valores)? {
            Some(balde) => self.reg.inserir_no_balde(&payload, balde)?,
            None => self
                .reg
                .inserir_no_periodo(&payload, self.chave_do_periodo(valores)?)?,
        };'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''    /// Em que periodo esta linha cai, quando a tabela e particionada por data.'''
novo2='''    /// Em que balde esta linha cai, quando a particao e alfanumerica.
    ///
    /// `None` nos outros modos. O valor da coluna de referencia vira texto
    /// pela mesma funcao que o `.reason` usa -- entao numero tambem particiona,
    /// e o `12345` cai no balde `_1`.
    fn balde_da_linha(&self, valores: &[Value]) -> Result<Option<u32>> {
        let modo = self.esquema.paginacao().modo;
        if !modo.por_letra() {
            return Ok(None);
        }
        let Some(i) = modo.coluna() else {
            return Err(PhxError::Esquema(
                "particao alfanumerica sem coluna de referencia".into(),
            ));
        };
        let texto = valores
            .get(i)
            .map(|v| v.para_texto())
            .unwrap_or_default();
        Ok(Some(phxsql_core::paginacao::balde_de(&texto)))
    }

    /// Quantas linhas cada balde tem. Vazio fora da particao alfanumerica.
    pub fn baldes(&self) -> &[u64] {
        self.reg.baldes()
    }

    /// Em que periodo esta linha cai, quando a tabela e particionada por data.'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
