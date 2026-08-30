# Add read paths and admin methods, then build
# 28/08 17:32

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()

# varrer passa a filtrar, mais os acessos de administrador
velho='''    /// Percorre a tabela na ORDEM DE DIGITACAO, direto do `.reg`.
    pub fn varrer(&mut self) -> Result<Vec<(RowId, Linha)>> {
        let mut saida = Vec::new();
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            saida.push((id, self.decodificar(&payload, true)?));
            rowid = id + 1;
        }
        Ok(saida)
    }'''
novo='''    /// Exclui de vez, sem motivo escrito. Recusa se a tabela exigir um.
    ///
    /// Continua sendo exclusao FISICA, como sempre foi -- o que mudou e que
    /// agora a linha passa pelo `.trash` antes de sair.
    pub fn excluir(&mut self, rowid: RowId) -> Result<bool> {
        self.excluir_de_vez(rowid, "")
    }

    /// Percorre a tabela na ORDEM DE DIGITACAO, direto do `.reg`.
    ///
    /// **Sem as linhas marcadas como excluidas.** Se elas continuassem
    /// aparecendo, marcar nao faria nada: a exclusao suave so vale se o
    /// caminho comum passar a nao enxergar a linha.
    pub fn varrer(&mut self) -> Result<Vec<(RowId, Linha)>> {
        self.varrer_com(Visao::Ativas)
    }

    /// Percorre escolhendo o que enxergar. Ver [`Visao`].
    pub fn varrer_com(&mut self, visao: Visao) -> Result<Vec<(RowId, Linha)>> {
        let mut saida = Vec::new();
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            let linha = self.decodificar(&payload, true)?;
            if visao.aceita(self.esta_excluida(&linha)) {
                saida.push((id, linha));
            }
            rowid = id + 1;
        }
        Ok(saida)
    }

    /// Tira da lista os rowids de linhas marcadas como excluidas.
    ///
    /// Os caminhos por indice devolvem rowid, e a marca esta no registro:
    /// filtrar exige ler cada um. Numa tabela sem a coluna de sistema a lista
    /// volta como veio, sem leitura nenhuma.
    pub fn filtrar_ativos(&mut self, rowids: &[RowId]) -> Result<Vec<RowId>> {
        if self.esquema.coluna_softdeleted().is_none() {
            return Ok(rowids.to_vec());
        }
        let mut saida = Vec::with_capacity(rowids.len());
        for &r in rowids {
            if let Some(p) = self.reg.ler(r)? {
                let linha = self.decodificar(&p, false)?;
                if !self.esta_excluida(&linha) {
                    saida.push(r);
                }
            }
        }
        Ok(saida)
    }

    // -------------------------------------------------- so administrador

    /// As linhas que sairam do `.reg`, da mais antiga para a mais recente.
    ///
    /// `com_externos` falso deixa os anexos de fora -- a tela que lista a
    /// lixeira nao precisa carregar as fotos de mil linhas.
    pub fn lixeira(
        &mut self,
        pular: u64,
        limite: u64,
        com_externos: bool,
    ) -> Result<Vec<Descartada>> {
        self.lixeira.ler(pular, limite, com_externos)
    }

    /// Quantas linhas a lixeira guarda, e quantos bytes ela ocupa.
    pub fn lixeira_tamanho(&mut self) -> Result<(u64, u64)> {
        Ok((self.lixeira.total()?, self.lixeira.bytes()?))
    }

    /// Decodifica uma linha da lixeira usando o esquema ATUAL da tabela.
    ///
    /// Se o esquema mudou depois do descarte, o payload guardado nao bate com
    /// ele -- e por isso a conferencia do tamanho vem antes, com uma mensagem
    /// que diz o que aconteceu em vez de devolver campo trocado.
    pub fn linha_da_lixeira(&mut self, d: &Descartada) -> Result<Linha> {
        if d.payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Esquema(format!(
                "a linha descartada tem {} bytes de payload e o esquema atual de {} \\
                 espera {}: a tabela mudou depois do descarte",
                d.payload.len(),
                self.nome,
                self.esquema.payload_len()
            )));
        }
        let mut linha = self.decodificar(&d.payload, false)?;
        // Os externos vem do proprio registro da lixeira, e nao do `.bin` /
        // `.memo`: aqueles blocos foram liberados na exclusao e podem ja ter
        // sido reaproveitados por outra linha.
        for (coluna, bytes) in &d.externos {
            let i = *coluna as usize;
            let Some(col) = self.esquema.colunas().get(i) else {
                continue;
            };
            linha[i] = match col.ty {
                ColumnType::Bin => Value::Bin(bytes.clone()),
                ColumnType::Memo => Value::Memo(String::from_utf8_lossy(bytes).into_owned()),
                _ => continue,
            };
        }
        Ok(linha)
    }

    /// Esvazia a lixeira. Registra o expurgo no `.reason` ANTES de apagar:
    /// o motivo tem de sobreviver ao dado.
    pub fn esvaziar_lixeira(&mut self, motivo: &str) -> Result<u64> {
        self.conferir_motivo(motivo)?;
        let quantos = self.lixeira.total()?;
        self.motivos.registrar(Tipo::Expurgo, 0, motivo, "")?;
        self.motivos.sincronizar()?;
        self.lixeira.esvaziar()
    }

    /// Os motivos registrados, em ordem cronologica.
    pub fn motivos(&mut self, pular: u64, limite: u64) -> Result<Vec<Motivo>> {
        self.motivos.ler(pular, limite)
    }

    /// Os motivos de um registro.
    pub fn motivos_de(&mut self, rowid: RowId) -> Result<Vec<Motivo>> {
        self.motivos.de(rowid)
    }

    pub fn total_de_motivos(&mut self) -> Result<u64> {
        self.motivos.total()
    }'''
assert velho in s
s=s.replace(velho,novo,1)

# Visao, definida logo depois de Linha
velho2='''/// Uma linha: um valor por coluna do esquema.
pub type Linha = Vec<Value>;'''
novo2='''/// Uma linha: um valor por coluna do esquema.
pub type Linha = Vec<Value>;

/// O que uma varredura enxerga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visao {
    /// So as linhas nao marcadas. E o que todo mundo ve.
    #[default]
    Ativas,
    /// So as marcadas como excluidas. A tela do administrador.
    Excluidas,
    /// Tudo que esta no `.reg`, marcado ou nao.
    Todas,
}

impl Visao {
    fn aceita(self, excluida: bool) -> bool {
        match self {
            Visao::Ativas => !excluida,
            Visao::Excluidas => excluida,
            Visao::Todas => true,
        }
    }
}'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

# usuario nos tres arquivos
velho3='''    pub fn definir_usuario(&mut self, usuario: u32) {
        self.log.usuario = usuario;
    }'''
novo3='''    pub fn definir_usuario(&mut self, usuario: u32) {
        self.log.usuario = usuario;
        self.lixeira.usuario = usuario;
        self.motivos.usuario = usuario;
    }'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

# sincronizar e verificar
velho4='''        self.log.sincronizar()?;
        Ok(())
    }
}'''
novo4='''        self.log.sincronizar()?;
        self.lixeira.sincronizar()?;
        self.motivos.sincronizar()?;
        Ok(())
    }
}'''
assert velho4 in s
s=s.replace(velho4,novo4,1)

velho5='''        let eventos = self.log.verificar()?;
'''
novo5='''        let eventos = self.log.verificar()?;
        let descartadas = self.lixeira.verificar()?;
        let motivos = self.motivos.verificar()?;
'''
assert velho5 in s
s=s.replace(velho5,novo5,1)

velho6='''            blocos_memo,
            eventos,
            volumes: ('''
novo6='''            blocos_memo,
            eventos,
            descartadas,
            motivos,
            volumes: ('''
assert velho6 in s
s=s.replace(velho6,novo6,1)

velho7='''    /// Eventos conferidos no `.log`.
    pub eventos: u64,'''
novo7='''    /// Eventos conferidos no `.log`.
    pub eventos: u64,
    /// Linhas conferidas no `.trash`.
    pub descartadas: u64,
    /// Registros conferidos no `.reason`.
    pub motivos: u64,'''
assert velho7 in s
s=s.replace(velho7,novo7,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
