# Teach completar about both system columns
# 28/08 18:27

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()

# completar() passa a cuidar das DUAS colunas de sistema
velho='''    /// Acrescenta o valor da coluna de sistema quando quem chamou nao o
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
    }'''
novo='''    /// Completa as colunas de sistema que quem chamou nao mandou.
    ///
    /// `None` quando nao ha nada a fazer. Aceita a linha faltando UMA ou as
    /// DUAS colunas do fim: quem monta a linha declarou as colunas dele e nao
    /// tem por que saber das do motor.
    ///
    /// Numa alteracao o valor herdado e o que a linha JA TINHA -- nas duas. Um
    /// `atualizar` comum nao pode ressuscitar linha marcada nem renumerar a
    /// ordem de chegada por distracao de quem montou os valores.
    fn completar(&self, valores: &[Value], anterior: Option<&Linha>) -> Option<Vec<Value>> {
        let n = self.esquema.colunas().len();
        if valores.len() >= n {
            return None;
        }
        let mut novos = valores.to_vec();
        for i in valores.len()..n {
            let c = &self.esquema.colunas()[i];
            if c.nome != phxsql_core::schema::COLUNA_SOFTDELETED
                && c.nome != phxsql_core::schema::COLUNA_ROWNUM
            {
                // A linha esta curta por outro motivo que nao as colunas de
                // sistema. Deixa a aridade reclamar, com a mensagem dela.
                return None;
            }
            novos.push(match anterior {
                Some(linha) => linha[i].clone(),
                // Zero e o "ainda nao numerado": `numerar_linha` troca por um
                // numero de verdade antes de a linha ir para o disco.
                None if c.nome == phxsql_core::schema::COLUNA_ROWNUM => Value::UInt(0),
                None => Value::Bool(false),
            });
        }
        Some(novos)
    }

    /// Poe o proximo `rownum` na linha, se ela ainda nao tiver um.
    ///
    /// Quem chama nao escolhe o numero: `rownum` e ordem de chegada, e um
    /// valor escolhido a mao seria uma ordem inventada. Valor diferente de
    /// zero que chegue de fora e ignorado -- e o caso de uma linha remontada
    /// por um cliente antigo que devolveu tudo que recebeu.
    fn numerar_linha(&mut self, valores: &mut [Value], anterior: Option<&Linha>) {
        let Some(i) = self.esquema.coluna_rownum() else {
            return;
        };
        if let Some(linha) = anterior {
            // Alteracao: mantem o numero que a linha ja tinha.
            if let Value::UInt(n) = linha[i] {
                if n > 0 {
                    valores[i] = Value::UInt(n);
                    return;
                }
            }
        }
        if !matches!(valores[i], Value::UInt(n) if n > 0) || anterior.is_none() {
            valores[i] = Value::UInt(self.reg.proximo_do_rownum());
        }
    }

    /// Proximo `rownum` que a tabela vai entregar.
    pub fn rownum_atual(&self) -> u64 {
        self.reg.rownum_atual()
    }

    /// O `rownum` desta linha, lido direto do payload -- sem decodificar nada.
    fn rownum_do_payload(&self, payload: &[u8]) -> Result<u64> {
        let Some(i) = self.esquema.coluna_rownum() else {
            return Ok(0);
        };
        let off = self.esquema.offset_coluna(i)?;
        Ok(u64::from_le_bytes(
            payload[off..off + 8].try_into().map_err(|_| {
                PhxError::Corrompido("payload curto demais para o rownum".into())
            })?,
        ))
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
