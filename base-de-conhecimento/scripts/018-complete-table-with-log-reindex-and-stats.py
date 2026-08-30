# Complete Table with log, reindex and stats
# 27/08 18:27

p='crates/phxsql-store/src/table.rs'
s=open(p).read()

# --- registrar no diario ---
s=s.replace('''                let _ = self.reg.excluir(rowid);
                let _ = self.liberar_externos(&ponteiros);
                return Err(e);
            }
        }
        Ok(rowid)
    }''','''                let _ = self.reg.excluir(rowid);
                let _ = self.liberar_externos(&ponteiros);
                return Err(e);
            }
        }
        self.log.registrar(Operacao::Inclusao, rowid, 1)?;
        Ok(rowid)
    }''')

s=s.replace('''        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        self.reg.atualizar(rowid, &payload)?;''','''        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        let versao = self.reg.atualizar(rowid, &payload)?;''')

s=s.replace('''        self.liberar_externos(&ponteiros_antigos)?;
        Ok(())
    }''','''        self.liberar_externos(&ponteiros_antigos)?;
        self.log.registrar(Operacao::Alteracao, rowid, versao)?;
        Ok(())
    }''')

s=s.replace('''        let ponteiros = self.ponteiros(&payload)?;
        self.liberar_externos(&ponteiros)?;
        self.reg.excluir(rowid)
    }''','''        let ponteiros = self.ponteiros(&payload)?;
        self.liberar_externos(&ponteiros)?;
        let removeu = self.reg.excluir(rowid)?;
        if removeu {
            self.log.registrar(Operacao::Exclusao, rowid, 0)?;
        }
        Ok(removeu)
    }''')

# --- verificar completo ---
s=s.replace('''        let registros = self.reg.verificar()?;
        let indices = self.ndx.verificar()?;
        let blocos_bin = self.bin.verificar()?;
        let blocos_memo = self.memo.verificar()?;''','''        let registros = self.reg.verificar()?;
        let indices = self.ndx.verificar()?;
        let blocos_bin = self.bin.verificar()?;
        let blocos_memo = self.memo.verificar()?;
        let eventos = self.log.verificar()?;''')

s=s.replace('''        Ok(Relatorio {
            tabela: self.nome.clone(),
            registros,
            slots: self.reg.slots(),
            indices,
            blocos_bin,
            blocos_memo,
        })
    }''','''        Ok(Relatorio {
            tabela: self.nome.clone(),
            registros,
            slots: self.reg.slots(),
            indices,
            blocos_bin,
            blocos_memo,
            eventos,
            volumes: (
                self.reg.volumes().len(),
                self.bin.volumes().len(),
                self.memo.volumes().len(),
                self.log.volumes().len(),
            ),
        })
    }

    /// Recria o `.ndx` inteiro a partir do `.reg`.
    ///
    /// Resolve tres coisas de uma vez: indice corrompido ou apagado, arvore
    /// subocupada depois de muitas exclusoes (a remocao nao rebalanceia), e
    /// indice novo acrescentado a uma tabela que ja tem dados.
    ///
    /// A varredura e feita na ordem de digitacao, entao a arvore sai com os
    /// rowids inseridos em ordem crescente dentro de cada chave.
    pub fn reindexar(&mut self) -> Result<Vec<(String, u64)>> {
        // `NdxFile::criar` trunca o arquivo: a arvore antiga vai embora
        // inteira, em vez de ser remendada.
        self.ndx = NdxFile::criar(caminho(&self.diretorio, &self.nome, EXT_NDX), &self.esquema)?;

        let quantos_indices = self.esquema.indices().len();
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            let valores = self.decodificar(&payload, false)?;
            for i in 0..quantos_indices {
                let chave = self.codificar_chave(i, &valores)?;
                self.ndx.inserir(i, &chave, id)?;
            }
            rowid = id + 1;
        }
        self.ndx.verificar()
    }

    /// Eventos do diario em ordem cronologica. `limite` zero devolve todos.
    pub fn diario(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        self.log.ler(pular, limite)
    }

    /// Eventos de um registro especifico.
    pub fn historico(&mut self, rowid: RowId) -> Result<Vec<Evento>> {
        self.log.historico(rowid)
    }

    /// Total de eventos registrados no diario.
    pub fn eventos(&mut self) -> Result<u64> {
        self.log.total()
    }

    /// Define quem assina as proximas operacoes no diario.
    pub fn definir_usuario(&mut self, usuario: u32) {
        self.log.usuario = usuario;
    }''')

s=s.replace('''    /// Ocupacao dos arquivos externos: `(.bin, .memo)`.
    pub fn estatisticas_externas(&self) -> (crate::blob::EstatisticaBlob, crate::blob::EstatisticaBlob) {
        (self.bin.estatistica(), self.memo.estatistica())
    }''','''    /// Ocupacao dos arquivos externos: `(.bin, .memo)`.
    pub fn estatisticas_externas(
        &mut self,
    ) -> Result<(crate::blob::EstatisticaBlob, crate::blob::EstatisticaBlob)> {
        Ok((self.bin.estatistica()?, self.memo.estatistica()?))
    }

    /// Volumes existentes de cada arquivo paginado.
    pub fn volumes(&self) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
        (
            self.reg.volumes(),
            self.bin.volumes(),
            self.memo.volumes(),
            self.log.volumes(),
        )
    }''')

s=s.replace('''        self.bin.sincronizar()?;
        self.memo.sincronizar()?;
        Ok(())''','''        self.bin.sincronizar()?;
        self.memo.sincronizar()?;
        self.log.sincronizar()?;
        Ok(())''')
open(p,'w').write(s)
print("table.rs completo")
