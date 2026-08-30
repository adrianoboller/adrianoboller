# Add cross-database copy
# 28/08 11:23

import pathlib
p = pathlib.Path('crates/phxsql-store/src/catalogo.rs')
s = p.read_text()
v = '''    pub fn existe_tabela(&self, schema: Option<&str>, nome: &str) -> Result<bool> {'''
n = '''    /// Copia uma tabela para OUTRO database -- o "colar" da tela.
    ///
    /// O `duplicar_tabela` copia dentro do mesmo database; este atravessa. E a
    /// mesma copia byte a byte, e pela mesma razao: a copia nasce com os
    /// mesmos rowids e na mesma ordem de digitacao.
    pub fn copiar_tabela_para(
        &self,
        origem: &str,
        destino_db: &Database,
        destino: &str,
    ) -> Result<usize> {
        let (schema_o, nome_o) = separar_qualificado(origem);
        let (schema_d, nome_d) = separar_qualificado(destino);
        let (schema_o, nome_o) = (schema_o.as_deref(), nome_o.as_str());
        let (schema_d, nome_d) = (schema_d.as_deref(), nome_d.as_str());
        validar_nome("tabela", nome_o)?;
        validar_nome("tabela de destino", nome_d)?;
        if destino_db.existe_tabela(schema_d, nome_d)? {
            return Err(PhxError::Duplicado(format!(
                "a tabela {destino} ja existe em {}",
                destino_db.nome()
            )));
        }
        let dir_o = self.diretorio(schema_o)?;
        // Colar num schema que ainda nao existe cria a pasta -- e o que quem
        // cola espera, e o mesmo que `criar_tabela` faz.
        let dir_d = match schema_d {
            None => destino_db.caminho().to_path_buf(),
            Some(sc) => destino_db.garantir_schema(sc)?,
        };
        if dir_o == dir_d && nome_o == nome_d {
            return Err(PhxError::Duplicado(
                "origem e destino sao a mesma tabela".into(),
            ));
        }

        let mut copiados = 0usize;
        for ext in Self::EXTENSOES {
            for arq in std::fs::read_dir(&dir_o)?.flatten() {
                let f = arq.file_name();
                let f = f.to_string_lossy();
                if pertence(&f, nome_o, ext) {
                    let novo = format!("{nome_d}{}", &f[nome_o.len()..]);
                    std::fs::copy(arq.path(), dir_d.join(&novo))?;
                    copiados += 1;
                }
            }
        }
        if copiados == 0 {
            return Err(PhxError::NaoEncontrado(format!(
                "tabela {origem} nao existe em {}",
                self.nome()
            )));
        }
        Ok(copiados)
    }

    pub fn existe_tabela(&self, schema: Option<&str>, nome: &str) -> Result<bool> {'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
