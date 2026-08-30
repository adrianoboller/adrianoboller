# Add pagina_por_posicao and friends
# 28/08 19:43

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()

# --- rowid_do_rownum: guardar a bisseccao do modo alfanumerico
antigo = """    pub fn rowid_do_rownum(&mut self, alvo: u64) -> Result<Option<RowId>> {
        if self.esquema.coluna_rownum().is_none() {
            return Ok(None);
        }
        let (mut baixo, mut alto) = (1u64, self.reg.slots());"""
novo = """    pub fn rowid_do_rownum(&mut self, alvo: u64) -> Result<Option<RowId>> {
        if self.esquema.coluna_rownum().is_none() {
            return Ok(None);
        }
        // Na particao alfanumerica o `rownum` NAO cresce com o rowid, e ai a
        // bisseccao nao vale: a Silva digitada primeiro mora no `_S`, com
        // rowid alto, e a Alves digitada depois mora no `_A`, com rowid 1 --
        // rownum 1 num rowid maior que o do rownum 2. Bissetar uma sequencia
        // que nao esta ordenada devolve resposta errada em silencio, que e
        // pior que devolver devagar. Ali se varre.
        if self.reg.paginacao().modo.por_letra() {
            return self.rowid_do_rownum_varrendo(alvo);
        }
        let (mut baixo, mut alto) = (1u64, self.reg.slots());"""
assert antigo in s
s = s.replace(antigo, novo)

# --- o varredor, e o resto da API nova, logo depois de pagina_desde_rownum
antigo = """    pub fn pagina_desde_rownum(
        &mut self,
        alvo: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<Vec<RowId>> {
        let Some(inicio) = self.rowid_do_rownum(alvo)? else {
            return Ok(Vec::new());
        };
        // `depois_de` exclui o proprio cursor, e aqui o inicio ENTRA.
        self.pagina_depois_de(inicio.saturating_sub(1), limite, visao)
    }"""
novo = """    pub fn pagina_desde_rownum(
        &mut self,
        alvo: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<Vec<RowId>> {
        let Some(inicio) = self.rowid_do_rownum(alvo)? else {
            return Ok(Vec::new());
        };
        // `depois_de` exclui o proprio cursor, e aqui o inicio ENTRA.
        self.pagina_depois_de(inicio.saturating_sub(1), limite, visao)
    }

    /// O mesmo que [`Table::rowid_do_rownum`], varrendo.
    ///
    /// Existe para a particao alfanumerica, onde a sequencia de `rownum` nao
    /// esta ordenada pelo rowid. Procura o MENOR `rownum` maior ou igual ao
    /// alvo -- e nao o primeiro que aparecer na varredura, que ali sairia do
    /// balde e nao da ordem de digitacao.
    fn rowid_do_rownum_varrendo(&mut self, alvo: u64) -> Result<Option<RowId>> {
        let mut melhor: Option<(u64, RowId)> = None;
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            rowid = id + 1;
            let n = self.rownum_do_payload(&payload)?;
            if n < alvo {
                continue;
            }
            if n == alvo {
                // Nao existe candidato melhor: pode parar aqui.
                return Ok(Some(id));
            }
            if melhor.is_none() || n < melhor.unwrap().0 {
                melhor = Some((n, id));
            }
        }
        Ok(melhor.map(|(_, id)| id))
    }

    /// Quantas linhas vivas estao marcadas como excluidas. Sai do cabecalho.
    pub fn marcadas(&self) -> u64 {
        self.reg.marcadas()
    }

    /// Reconta as marcadas varrendo, e corrige o cabecalho. Devolve o total.
    ///
    /// O contador do cabecalho e um cache, como o `live_count`. Este e o
    /// caminho que o refaz quando ha duvida -- um arquivo que veio de uma
    /// versao anterior, uma queda no meio de uma exclusao, um reparo.
    pub fn recontar_marcadas(&mut self) -> Result<u64> {
        if self.esquema.coluna_softdeleted().is_none() {
            self.reg.definir_marcadas(0)?;
            return Ok(0);
        }
        let mut n = 0u64;
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            rowid = id + 1;
            if self.marcada_no_payload(&payload)? {
                n += 1;
            }
        }
        self.reg.definir_marcadas(n)?;
        Ok(n)
    }

    /// A posicao de uma linha na listagem e o `rownum` dela menos um?
    ///
    /// # Por que a pergunta importa
    ///
    /// Se a resposta e sim, pular para a posicao 500.000 deixa de ser meio
    /// milhao de passos e vira uma bisseccao de vinte leituras: basta procurar
    /// o `rownum` 500.001. Se e nao, a conta erraria -- e erraria calada, que
    /// e o jeito pior de errar numa tela de paginacao.
    ///
    /// # As quatro coisas que a quebram
    ///
    /// 1. **Tabela sem a coluna** -- nao ha numero de ordem para procurar.
    /// 2. **Particao alfanumerica** -- a leitura sai balde a balde e o
    ///    `rownum` guarda a digitacao; as duas ordens sao diferentes de
    ///    proposito.
    /// 3. **Exclusao fisica** -- a linha saiu, o numero dela nao volta, e
    ///    todo mundo depois dela anda um para tras. Da para ver em tempo
    ///    constante: `rownum_atual() - 1` e quantas linhas ja entraram, e
    ///    `registros()` e quantas ficaram.
    /// 4. **Exclusao suave**, na visao comum -- a linha continua no arquivo e
    ///    continua com o numero, mas some da lista. Por isso o cabecalho
    ///    carrega quantas estao marcadas.
    ///
    /// Nenhuma das quatro custa leitura: as duas ultimas saem de contadores
    /// que ja moram no cabecalho do volume 1.
    pub fn posicao_e_rownum(&self, visao: Visao) -> bool {
        if self.esquema.coluna_rownum().is_none() || self.reg.paginacao().modo.por_letra() {
            return false;
        }
        // A lista de excluidas nao tem relacao nenhuma com a ordem de
        // chegada: a decima marcada pode ser a linha numero tres.
        if visao == Visao::Excluidas {
            return false;
        }
        if self.reg.rownum_atual() - 1 != self.reg.registros() {
            return false;
        }
        visao == Visao::Todas || self.reg.marcadas() == 0
    }

    /// A pagina que comeca na posicao `pular`, pelo caminho mais barato que
    /// ainda estiver certo.
    ///
    /// E o `OFFSET` do SQL, e o que a caixa «ir para a pagina» da grade usa.
    /// Devolve tambem COMO chegou la, porque as duas formas custam ordens de
    /// grandeza diferentes e quem esta do outro lado merece saber qual pagou:
    ///
    /// - [`Salto::Bissecao`] -- a posicao e o `rownum`, e ai o inicio da
    ///   pagina sai de uma busca binaria. Custa `log2 N` leituras, e nao `N`.
    /// - [`Salto::Passo`] -- a tabela tem buraco, ou e alfanumerica, ou a
    ///   visao e a das excluidas. Ai anda ate a posicao, uma linha por vez.
    ///
    /// Nos dois casos a resposta e a MESMA pagina. O que muda e o preco.
    pub fn pagina_por_posicao(
        &mut self,
        pular: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<(Vec<RowId>, Salto)> {
        if pular > 0 && self.posicao_e_rownum(visao) {
            // A posicao e base zero e o rownum comeca em 1.
            let rowids = self.pagina_desde_rownum(pular + 1, limite, visao)?;
            return Ok((rowids, Salto::Bissecao));
        }
        // Pular zero tambem passa por aqui: a primeira pagina nao tem o que
        // pular, e a bisseccao so acrescentaria uma busca para achar o comeco.
        Ok((self.pagina(pular, limite, visao)?, Salto::Passo))
    }"""
assert antigo in s
s = s.replace(antigo, novo)

p.write_text(s)
print("ok")
