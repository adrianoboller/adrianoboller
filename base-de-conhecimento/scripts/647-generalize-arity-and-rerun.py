# Generalize arity and rerun
# 28/08 18:29

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    fn conferir_aridade(&self, valores: &[Value]) -> Result<()> {
        let n = self.esquema().colunas().len();
        // A coluna de sistema pode vir ou nao: quem monta a linha declarou N-1
        // colunas e nao tem por que saber da setima. Ver `completar`.
        let sem_sistema = self.esquema.coluna_softdeleted().is_some() && valores.len() + 1 == n;
        if valores.len() != n && !sem_sistema {
            return Err(PhxError::Tipo(format!(
                "{}: esperado {n} valores, recebido {}",
                self.nome,
                valores.len()
            )));
        }
        Ok(())
    }'''
novo='''    /// Quantas colunas de sistema estao no FIM da lista, seguidas.
    ///
    /// Conta do fim para tras e para na primeira coluna do usuario: e o que
    /// permite a linha chegar sem elas. Uma coluna de sistema que estivesse no
    /// meio nao entraria nesta conta -- e nao esta, por construcao: elas
    /// entram sempre no fim, e ha teste que trava a ordem.
    fn colunas_de_sistema_no_fim(&self) -> usize {
        self.esquema
            .colunas()
            .iter()
            .rev()
            .take_while(|c| phxsql_core::schema::e_coluna_de_sistema(&c.nome))
            .count()
    }

    fn conferir_aridade(&self, valores: &[Value]) -> Result<()> {
        let n = self.esquema().colunas().len();
        // As colunas de sistema podem vir ou nao. Quem monta a linha declarou
        // as colunas dele e nao tem por que saber das do motor -- e um cliente
        // escrito antes de elas existirem continua funcionando. Ver `completar`.
        let minimo = n - self.colunas_de_sistema_no_fim();
        if valores.len() < minimo || valores.len() > n {
            return Err(PhxError::Tipo(format!(
                "{}: esperado {n} valores{}, recebido {}",
                self.nome,
                if minimo < n {
                    format!(" (ou {minimo}, sem as colunas do motor)")
                } else {
                    String::new()
                },
                valores.len()
            )));
        }
        Ok(())
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
