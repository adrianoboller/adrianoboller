# Add the bulk insert operation
# 28/08 19:21

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    fn op_atualizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {'''
novo='''    /// `inserir_lote`: muitas linhas de uma vez, ou uma carga colada.
    ///
    /// # De onde vem o ganho
    ///
    /// Nao e do disco. Cada linha custa o mesmo la dentro -- montar o payload,
    /// conferir a unicidade, gravar o slot, manter cada indice. O ganho e de
    /// tudo que acontecia POR LINHA e passa a acontecer uma vez: abrir a
    /// tabela (sete arquivos), tomar a trava, e o `fsync`.
    ///
    /// Vinte mil insercoes pela rede eram vinte mil aberturas de tabela.
    ///
    /// # Duas formas de mandar
    ///
    /// `"linhas"` com uma lista de objetos, ou `"texto"` com uma carga colada
    /// mais `"formato"` -- json, csv, txt, html ou xml. Sem formato, ele e
    /// adivinhado pelo primeiro caractere.
    fn op_inserir_lote(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let parar = p.booleano_ou("parar_no_erro", true);

        // A carga colada vira lista de objetos ANTES de a trava ser tomada:
        // analisar texto com a trava de dados na mao seguraria todo mundo por
        // causa de um CSV malformado.
        let (itens, formato) = match p.campo("texto").and_then(Json::texto) {
            Some(texto) => {
                let f = match p.texto_ou("formato", "").trim() {
                    "" | "auto" => crate::importar::adivinhar(texto),
                    outro => crate::importar::Formato::de_texto(outro)?,
                };
                let carga = crate::importar::ler(texto, f)?;
                (carga.para_json(), f.nome().to_string())
            }
            None => (
                p.campo("linhas")
                    .or_else(|| p.campo("valores"))
                    .cloned()
                    .ok_or_else(|| {
                        PhxError::Esquema(
                            "informe \\"linhas\\" com a lista, ou \\"texto\\" com a carga colada"
                                .into(),
                        )
                    })?,
                "lista".to_string(),
            ),
        };
        let itens = itens.lista().map(|l| l.to_vec()).ok_or_else(|| {
            PhxError::Esquema("\\"linhas\\" precisa ser uma lista de objetos".into())
        })?;
        if itens.is_empty() {
            return Err(PhxError::Esquema("a carga nao tem nenhuma linha".into()));
        }

        let inicio = Instant::now();
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        // A conversao de tipo acontece aqui, com o esquema na mao, e uma linha
        // que nao converte entra na lista de recusadas em vez de derrubar a
        // carga inteira -- a menos que `parar_no_erro` mande parar.
        let mut linhas = Vec::with_capacity(itens.len());
        let mut recusadas: Vec<(usize, String)> = Vec::new();
        for (i, item) in itens.iter().enumerate() {
            match json_para_linha(item, t.esquema()) {
                Ok(l) => linhas.push(l),
                Err(e) => {
                    recusadas.push((i, e.to_string()));
                    if parar {
                        return Ok(Self::resposta_do_lote(
                            p,
                            &formato,
                            itens.len(),
                            &[],
                            &recusadas,
                            inicio,
                        ));
                    }
                }
            }
        }

        let lote = t.inserir_lote(&linhas, parar)?;
        // Uma carga inteira e um `sincronizar`, e nao um por linha.
        t.sincronizar()?;
        for (i, e) in &lote.recusadas {
            recusadas.push((*i, e.clone()));
        }
        // A copia em RAM acompanha dentro da mesma trava.
        for (rowid, linha) in lote.rowids.iter().zip(linhas.iter()) {
            let (r, l) = (*rowid, linha.clone());
            self.residente_mut(p, move |m| m.anotar_insercao(r, &l));
        }
        Ok(Self::resposta_do_lote(
            p,
            &formato,
            itens.len(),
            &lote.rowids,
            &recusadas,
            inicio,
        ))
    }

    fn resposta_do_lote(
        p: &Json,
        formato: &str,
        recebidas: usize,
        rowids: &[u64],
        recusadas: &[(usize, String)],
        inicio: Instant,
    ) -> Json {
        let ms = inicio.elapsed().as_millis() as u64;
        Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("formato", Json::texto_de(formato)),
            ("recebidas", Json::de_u64(recebidas as u64)),
            ("gravadas", Json::de_u64(rowids.len() as u64)),
            ("recusadas", Json::de_u64(recusadas.len() as u64)),
            ("primeiro_rowid", Json::de_u64(rowids.first().copied().unwrap_or(0))),
            ("ultimo_rowid", Json::de_u64(rowids.last().copied().unwrap_or(0))),
            ("ms", Json::de_u64(ms)),
            (
                "por_segundo",
                Json::de_u64(if ms == 0 {
                    0
                } else {
                    (rowids.len() as u64) * 1000 / ms
                }),
            ),
            // A POSICAO na carga, e nao o rowid: a linha recusada nao tem
            // rowid, e quem mandou precisa achar a linha no arquivo dele.
            (
                "erros",
                Json::Lista(
                    recusadas
                        .iter()
                        .take(50)
                        .map(|(i, e)| {
                            Json::objeto(vec![
                                ("linha", Json::de_u64(*i as u64 + 1)),
                                ("erro", Json::texto_de(e)),
                            ])
                        })
                        .collect(),
                ),
            ),
            // Sem transacao, uma carga que para no meio DEIXA gravado o que ja
            // entrou. Dizer isso na resposta e melhor que quem chamou
            // descobrir contando as linhas depois.
            (
                "aviso",
                Json::texto_de(if recusadas.is_empty() {
                    ""
                } else {
                    "nao ha transacao: as linhas gravadas antes do erro ficaram gravadas"
                }),
            ),
        ])
    }

    fn op_atualizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {'''
assert velho in s
s=s.replace(velho,novo,1)

# despacho
s=s.replace('''            "inserir" => self.op_inserir(p, sessao),''',
            '''            "inserir" => self.op_inserir(p, sessao),
            "inserir_lote" | "importar" | "carga" => self.op_inserir_lote(p, sessao),''',1)
io.open(p,'w',encoding='utf-8').write(s)
