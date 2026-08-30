# Add tolerant arity and helpers
# 28/08 17:30

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()

velho = '''    fn conferir_aridade(&self, valores: &[Value]) -> Result<()> {
        let n = self.esquema().colunas().len();
        if valores.len() != n {
            return Err(PhxError::Tipo(format!(
                "{}: esperado {n} valores, recebido {}",
                self.nome,
                valores.len()
            )));
        }
        Ok(())
    }'''
novo = '''    fn conferir_aridade(&self, valores: &[Value]) -> Result<()> {
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
    }

    /// Acrescenta o valor da coluna de sistema quando quem chamou nao o
    /// mandou. `None` quando nao ha nada a fazer.
    ///
    /// Numa alteracao o valor herdado e o que a linha JA TINHA: um `atualizar`
    /// comum nao pode ressuscitar uma linha marcada como excluida por
    /// distracao de quem montou os valores.
    fn completar(&self, valores: &[Value], anterior: Option<&Linha>) -> Option<Vec<Value>> {
        let i = self.esquema.coluna_softdeleted()?;
        if valores.len() != i {
            return None;
        }
        let mut novos = valores.to_vec();
        novos.push(match anterior {
            Some(linha) => linha[i].clone(),
            None => Value::Bool(false),
        });
        Some(novos)
    }

    /// A linha esta marcada como excluida?
    ///
    /// Falso numa tabela sem a coluna de sistema -- ali nenhuma linha esta
    /// marcada, porque nao ha onde marcar.
    pub fn esta_excluida(&self, linha: &[Value]) -> bool {
        match self.esquema.coluna_softdeleted() {
            Some(i) => matches!(linha.get(i), Some(Value::Bool(true))),
            None => false,
        }
    }

    /// Posicao da coluna de sistema, ou o erro que explica por que nao ha.
    fn exigir_softdeleted(&self) -> Result<usize> {
        self.esquema.coluna_softdeleted().ok_or_else(|| {
            PhxError::Esquema(format!(
                "a tabela {} foi criada antes da coluna {} existir e nao tem \\
                 exclusao suave; recrie a tabela para ganhar a coluna",
                self.nome,
                phxsql_core::schema::COLUNA_SOFTDELETED
            ))
        })
    }'''
assert velho in s
s = s.replace(velho, novo, 1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
