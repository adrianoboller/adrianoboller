# Aplicar o delta do dblink no servidor
# 29/08 07:45

import io,re
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

velho='''    fn ligar(&self, p: &Json) -> Result<(Definicao, mysql::Conexao)> {
        let d = {
            let r = self.dblink.lock().map_err(|_| trava_envenenada())?;
            r.achar(p.texto_ou("dblink", p.texto_ou("nome", "")))?
                .clone()
        };
        let c = d.conectar()?;
        Ok((d, c))
    }'''
novo='''    fn ligar(&self, p: &Json) -> Result<(Definicao, crate::dblink::Conexao)> {
        let d = {
            let r = self.dblink.lock().map_err(|_| trava_envenenada())?;
            r.achar(p.texto_ou("dblink", p.texto_ou("nome", "")))?
                .clone()
        };
        // `abrir` escolhe a conexao pelo motor da definicao -- e por aqui que
        // o PostgreSQL(R) entra sem que nenhuma operacao precise saber dele.
        let c = d.abrir()?;
        Ok((d, c))
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# os cinco corpos viram delegacao; o SQL por motor mora em operacoes.rs
ini=s.index('    fn op_dblink_testar(&self, p: &Json) -> Result<Json> {')
fim=s.index('    /// Uma instrucao escrita a mao contra o outro servidor.')
novo_bloco='''    fn op_dblink_testar(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::testar(&d, c)
    }

    /// As bases do outro servidor.
    fn op_dblink_bancos(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::bancos(&d, c)
    }

    /// As tabelas de uma base do outro servidor, com tamanho e comentario.
    fn op_dblink_tabelas(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::tabelas(&d, c, p)
    }

    /// A estrutura de uma tabela do outro servidor.
    fn op_dblink_estrutura(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::estrutura(&d, c, p)
    }

    /// O conteudo de uma tabela do outro servidor, para a grade.
    fn op_dblink_ler(&self, p: &Json) -> Result<Json> {
        let (d, c) = self.ligar(p)?;
        crate::dblink::operacoes::ler(&d, c, p)
    }

'''
s=s[:ini]+novo_bloco+s[fim:]

velho2='''    fn op_dblink_consultar(&self, p: &Json) -> Result<Json> {
        let sql = p.texto_ou("sql", "").trim().to_string();
        if sql.is_empty() {
            return Err(PhxError::Esquema("dblink_consultar sem \\"sql\\"".into()));
        }
        let (d, mut c) = self.ligar(p)?;
        if !crate::dblink::so_consulta(&sql) {
            if d.somente_leitura {
                return Err(PhxError::Autorizacao(format!(
                    "a ligacao {:?} esta em somente leitura e a instrucao nao e consulta",
                    d.nome
                )));
            }
            if self.config.somente_leitura {
                return Err(PhxError::Autorizacao(
                    "este servidor esta em somente leitura: nao escreve nem pelo dblink".into(),
                ));
            }
        }
        let limite = p
            .inteiro_ou("limite", d.max_linhas as i64)
            .clamp(1, d.max_linhas as i64) as u64;
        let comeco = Instant::now();
        let r = c.consultar(&sql, limite);
        c.encerrar();
        let r = r?;
        let mut saida = crate::dblink::resultado_para_json(&r);
        if let Json::Objeto(campos) = &mut saida {
            campos.push(("dblink".into(), Json::texto_de(&d.nome)));
            campos.push((
                "ms".into(),
                Json::de_u64(comeco.elapsed().as_millis() as u64),
            ));
        }
        Ok(saida)
    }'''
novo2='''    fn op_dblink_consultar(&self, p: &Json) -> Result<Json> {
        let sql = p.texto_ou("sql", "").trim().to_string();
        if sql.is_empty() {
            return Err(PhxError::Esquema("dblink_consultar sem \\"sql\\"".into()));
        }
        // As duas travas vem ANTES de conectar: recusar depois de abrir a
        // conexao gasta uma ida a rede para dizer nao. A da ligacao precisa da
        // definicao, entao ela e achada primeiro, sem conectar.
        let d = {
            let r = self.dblink.lock().map_err(|_| trava_envenenada())?;
            r.achar(p.texto_ou("dblink", p.texto_ou("nome", "")))?
                .clone()
        };
        if !crate::dblink::so_consulta(&sql) {
            if d.somente_leitura {
                return Err(PhxError::Autorizacao(format!(
                    "a ligacao {:?} esta em somente leitura e a instrucao nao e consulta",
                    d.nome
                )));
            }
            if self.config.somente_leitura {
                return Err(PhxError::Autorizacao(
                    "este servidor esta em somente leitura: nao escreve nem pelo dblink".into(),
                ));
            }
        }
        let limite = p
            .inteiro_ou("limite", d.max_linhas as i64)
            .clamp(1, d.max_linhas as i64) as u64;
        let c = d.abrir()?;
        crate::dblink::operacoes::consultar(&d, c, &sql, limite)
    }'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
