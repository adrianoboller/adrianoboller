# Replace excluir with the three deletion paths
# 28/08 17:31

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()

velho = '''    /// Exclui a linha: tira as chaves dos indices, libera os blocos externos
    /// e marca o slot do `.reg` como livre.
    pub fn excluir(&mut self, rowid: RowId) -> Result<bool> {
        let payload = match self.reg.ler(rowid)? {
            None => return Ok(false),
            Some(p) => p,
        };
        let valores = self.decodificar(&payload, false)?;
        let chaves = self.todas_as_chaves(&valores)?;
        for (i, chave) in chaves.iter().enumerate() {
            self.ndx.remover(i, chave, rowid)?;
        }
        let ponteiros = self.ponteiros(&payload)?;
        self.liberar_externos(&ponteiros)?;
        let removeu = self.reg.excluir(rowid)?;
        if removeu {
            self.log.registrar(Operacao::Exclusao, rowid, 0)?;
        }
        Ok(removeu)
    }'''

novo = '''    /// Exclui de vez: guarda a linha inteira no `.trash`, **espera o disco
    /// confirmar**, e so entao libera o slot do `.reg`.
    ///
    /// # A ordem
    ///
    /// Guardar depois de liberar teria uma janela em que a linha nao existe em
    /// lugar nenhum -- e uma queda dentro dela nao tem conserto. Guardar
    /// antes tem a janela oposta: a linha aparece nos dois lugares, o que se
    /// resolve olhando. Entre perder e duplicar, duplica.
    ///
    /// O `sincronizar` esta dentro de `LixeiraFile::guardar`, e nao aqui,
    /// porque a garantia e daquele arquivo: "esta na lixeira" com a pagina
    /// ainda suja na memoria nao e uma garantia.
    pub fn excluir_de_vez(&mut self, rowid: RowId, motivo: &str) -> Result<bool> {
        let payload = match self.reg.ler(rowid)? {
            None => return Ok(false),
            Some(p) => p,
        };
        self.conferir_motivo(motivo)?;

        // O conteudo dos externos entra na lixeira junto: os ponteiros do
        // payload apontam para blocos que esta mesma exclusao vai liberar.
        let externos = self.conteudo_externo(&payload)?;
        let identidade = self.identidade(&payload)?;
        self.lixeira.guardar(rowid, &payload, externos)?;

        let valores = self.decodificar(&payload, false)?;
        let chaves = self.todas_as_chaves(&valores)?;
        for (i, chave) in chaves.iter().enumerate() {
            self.ndx.remover(i, chave, rowid)?;
        }
        let ponteiros = self.ponteiros(&payload)?;
        self.liberar_externos(&ponteiros)?;
        let removeu = self.reg.excluir(rowid)?;
        if removeu {
            self.motivos
                .registrar(Tipo::Fisica, rowid, motivo, &identidade)?;
            self.log.registrar(Operacao::Exclusao, rowid, 0)?;
        }
        Ok(removeu)
    }

    /// Marca a linha como excluida sem apagar nada.
    ///
    /// Devolve `false` quando o slot ja estava livre ou a linha ja estava
    /// marcada -- marcar duas vezes nao e erro, mas tambem nao gera um segundo
    /// motivo no `.reason`.
    pub fn excluir_suave(&mut self, rowid: RowId, motivo: &str) -> Result<bool> {
        self.exigir_softdeleted()?;
        self.conferir_motivo(motivo)?;
        if !self.marcar(rowid, true)? {
            return Ok(false);
        }
        let identidade = match self.reg.ler(rowid)? {
            Some(p) => self.identidade(&p)?,
            None => String::new(),
        };
        self.motivos
            .registrar(Tipo::Suave, rowid, motivo, &identidade)?;
        Ok(true)
    }

    /// Desfaz uma exclusao suave.
    pub fn restaurar(&mut self, rowid: RowId, motivo: &str) -> Result<bool> {
        self.exigir_softdeleted()?;
        if !self.marcar(rowid, false)? {
            return Ok(false);
        }
        let identidade = match self.reg.ler(rowid)? {
            Some(p) => self.identidade(&p)?,
            None => String::new(),
        };
        self.motivos
            .registrar(Tipo::Restauracao, rowid, motivo, &identidade)?;
        Ok(true)
    }

    /// Troca o valor da coluna de sistema sem reescrever os externos.
    ///
    /// Nao usa `atualizar` de proposito: aquele caminho decodifica a linha com
    /// os anexos, regrava cada um e libera os antigos -- marcar uma linha
    /// copiaria a foto dela de um bloco para outro sem nenhuma razao. Aqui o
    /// unico byte que muda e o da coluna, e os ponteiros ficam onde estao.
    fn marcar(&mut self, rowid: RowId, valor: bool) -> Result<bool> {
        let i = self.exigir_softdeleted()?;
        let Some(mut payload) = self.reg.ler(rowid)? else {
            return Ok(false);
        };
        let antes = self.decodificar(&payload, false)?;
        if matches!(antes[i], Value::Bool(v) if v == valor) {
            return Ok(false);
        }

        let off = self.esquema.offset_coluna(i)?;
        let fim = off + ColumnType::Bool.largura();
        let novo = Value::Bool(valor);
        escrever_inline(&novo, &ColumnType::Bool, &mut payload[off..fim])?;
        // A coluna e obrigatoria, mas a linha pode ter vindo de um caminho que
        // a deixou nula: limpa o bit de nulo junto, senao o valor gravado nao
        // seria lido de volta.
        payload[i / 8] &= !(1 << (i % 8));

        // A marca pode estar dentro de um indice -- e util que esteja, para
        // listar excluidas sem varrer. Entao as chaves mudam.
        let mut depois = antes.clone();
        depois[i] = novo;
        let chaves_antigas = self.todas_as_chaves(&antes)?;
        let chaves_novas = self.todas_as_chaves(&depois)?;

        let versao = self.reg.atualizar(rowid, &payload)?;
        for (j, (a, b)) in chaves_antigas.iter().zip(chaves_novas.iter()).enumerate() {
            if a != b {
                self.ndx.remover(j, a, rowid)?;
                self.ndx.inserir(j, b, rowid)?;
            }
        }
        self.log.registrar(Operacao::Alteracao, rowid, versao)?;
        Ok(true)
    }

    /// Recusa a exclusao sem motivo quando a tabela exige um.
    ///
    /// A escolha e da tabela, feita na criacao. Uma tabela de auditoria exige;
    /// uma tabela de rascunho nao, e obrigar ali so ensinaria todo mundo a
    /// digitar um ponto.
    fn conferir_motivo(&self, motivo: &str) -> Result<()> {
        if self.esquema.motivo_obrigatorio() && motivo.trim().is_empty() {
            return Err(PhxError::Esquema(format!(
                "a tabela {} exige motivo escrito para excluir",
                self.nome
            )));
        }
        Ok(())
    }

    /// Como esta linha se identifica, em texto, para o `.reason`.
    ///
    /// Na ordem: a chave primaria, senao a primeira coluna `Uuid`, senao a
    /// sequencia. Vazio quando a tabela nao tem nenhuma das tres -- e ai o
    /// rowid do proprio registro e tudo que se tem.
    fn identidade(&mut self, payload: &[u8]) -> Result<String> {
        let valores = self.decodificar(payload, false)?;
        let esquema = &self.esquema;
        if let Some(pk) = esquema.chave_primaria() {
            let partes: Vec<String> = pk
                .colunas
                .iter()
                .map(|ic| {
                    format!(
                        "{}={}",
                        esquema.colunas()[ic.coluna].nome,
                        valores[ic.coluna].para_texto()
                    )
                })
                .collect();
            return Ok(partes.join(", "));
        }
        for (i, c) in esquema.colunas().iter().enumerate() {
            if matches!(c.ty, ColumnType::Uuid | ColumnType::Sequence) {
                return Ok(format!("{}={}", c.nome, valores[i].para_texto()));
            }
        }
        Ok(String::new())
    }

    /// O conteudo de cada coluna externa da linha, para ir junto na lixeira.
    fn conteudo_externo(&mut self, payload: &[u8]) -> Result<Vec<(u16, Vec<u8>)>> {
        let mut saida = Vec::new();
        for i in 0..self.esquema.colunas().len() {
            let col = &self.esquema.colunas()[i];
            if !col.ty.externo() || payload[i / 8] & (1 << (i % 8)) != 0 {
                continue;
            }
            let ty = col.ty;
            let off = self.esquema.offset_coluna(i)?;
            let p = Ponteiro::ler(&payload[off..off + ty.largura()])?;
            let bytes = match ty {
                ColumnType::Bin => self.bin.ler(&p)?,
                ColumnType::Memo => self.memo.ler(&p)?,
                _ => continue,
            };
            saida.push((i as u16, bytes));
        }
        Ok(saida)
    }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
