# Add the preview operation and permissions
# 28/08 19:27

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    fn resposta_do_lote('''
novo='''    /// `importar_conferir`: le a carga e devolve o que entendeu, SEM gravar.
    ///
    /// Existe porque uma carga que entra errada e pior que uma que nao entra.
    /// A tela mostra a amostra e as colunas casadas antes de o botao de gravar
    /// ficar disponivel.
    ///
    /// Le pelo MESMO caminho da gravacao. Uma previa escrita no navegador
    /// seria uma segunda implementacao do leitor, e as duas divergiriam no
    /// primeiro caso esquisito -- que e justamente onde a previa serve.
    fn op_importar_conferir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let texto = p
            .campo("texto")
            .and_then(Json::texto)
            .ok_or_else(|| PhxError::Esquema("informe \\"texto\\" com a carga".into()))?;
        let f = match p.texto_ou("formato", "").trim() {
            "" | "auto" => crate::importar::adivinhar(texto),
            outro => crate::importar::Formato::de_texto(outro)?,
        };
        let carga = crate::importar::ler(texto, f)?;

        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let t = self.abrir_travada(&_trava, p, sessao)?;
        let e = t.esquema();

        // As duas listas que decidem se a carga serve: o que a tabela nao tem
        // (erro) e o que a carga nao traz (fica nulo).
        let desconhecidas: Vec<Json> = carga
            .colunas
            .iter()
            .filter(|c| e.coluna_por_nome(c).is_none())
            .map(Json::texto_de)
            .collect();
        let faltando: Vec<Json> = e
            .colunas()
            .iter()
            .filter(|c| {
                !phxsql_core::schema::e_coluna_de_sistema(&c.nome)
                    && !carga.colunas.contains(&c.nome)
            })
            .map(|c| Json::texto_de(&c.nome))
            .collect();

        const AMOSTRA: usize = 20;
        let amostra: Vec<Json> = carga
            .linhas
            .iter()
            .take(AMOSTRA)
            .map(|l| Json::Lista(l.iter().map(Json::texto_de).collect()))
            .collect();

        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("formato", Json::texto_de(f.nome())),
            ("linhas_lidas", Json::de_u64(carga.linhas.len() as u64)),
            (
                "colunas",
                Json::Lista(carga.colunas.iter().map(Json::texto_de).collect()),
            ),
            ("desconhecidas", Json::Lista(desconhecidas)),
            ("faltando", Json::Lista(faltando)),
            ("amostra", Json::Lista(amostra)),
        ]))
    }

    fn resposta_do_lote('''
assert velho in s
s=s.replace(velho,novo,1)
s=s.replace('''            "inserir_lote" | "importar" | "carga" => self.op_inserir_lote(p, sessao),''',
            '''            "inserir_lote" | "importar" | "carga" => self.op_inserir_lote(p, sessao),
            "importar_conferir" => self.op_importar_conferir(p, sessao),''',1)
io.open(p,'w',encoding='utf-8').write(s)
