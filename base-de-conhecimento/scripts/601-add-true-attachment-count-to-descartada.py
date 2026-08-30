# Add true attachment count to Descartada
# 28/08 17:54

import io
p='crates/phxsql-store/src/lixeira.rs'
s=io.open(p,encoding='utf-8').read()

velho='''    /// Conteudo de cada coluna externa: `(indice da coluna, bytes)`.
    pub externos: Vec<(u16, Vec<u8>)>,
}'''
novo='''    /// Quantas colunas externas a linha TEM, sempre.
    ///
    /// Separado de `externos.len()` de proposito: a listagem pode nao carregar
    /// os anexos, e ai `externos` vem vazio. Se o contador saisse dele, a tela
    /// da lixeira diria "0 anexos" para uma linha que tem tres -- e quem
    /// investiga concluiria que a foto nunca existiu.
    pub n_externos: u8,
    /// Conteudo de cada coluna externa: `(indice da coluna, bytes)`.
    ///
    /// Vazio quando a leitura pediu sem anexos. Compare com `n_externos` para
    /// saber se e "nao tem" ou "nao carregou".
    pub externos: Vec<(u16, Vec<u8>)>,
}'''
assert velho in s
s=s.replace(velho,novo,1)

# escrever: n_externos vem do vetor
s=s.replace('''        if self.externos.len() > u8::MAX as usize {''',
            '''        debug_assert_eq!(self.n_externos as usize, self.externos.len());
        if self.externos.len() > u8::MAX as usize {''',1)

# ler: n_externos do cabecalho
s=s.replace('''        Ok(Descartada {
            uuid: Uuid::de_bytes(src[28..44].try_into().unwrap()),
            carimbo: c.u64(0) as i64,
            rowid: c.u64(12),
            usuario: c.u32(20),
            payload,
            externos,
        })''','''        Ok(Descartada {
            uuid: Uuid::de_bytes(src[28..44].try_into().unwrap()),
            carimbo: c.u64(0) as i64,
            rowid: c.u64(12),
            usuario: c.u32(20),
            n_externos: src[9],
            payload,
            externos,
        })''',1)

# guardar
s=s.replace('''        let d = Descartada {
            uuid: Uuid::v7(),
            carimbo: agora_ms(),
            rowid,
            usuario: self.usuario,
            payload: payload.to_vec(),
            externos,
        };''','''        let d = Descartada {
            uuid: Uuid::v7(),
            carimbo: agora_ms(),
            rowid,
            usuario: self.usuario,
            n_externos: externos.len().min(u8::MAX as usize) as u8,
            payload: payload.to_vec(),
            externos,
        };''',1)

# a listagem sem anexos limpa o conteudo, nao o contador
s=s.replace('''                    let mut d = Descartada::ler(&buf)?;
                    if !com_externos {
                        d.externos.clear();
                    }''','''                    let mut d = Descartada::ler(&buf)?;
                    if !com_externos {
                        // So o CONTEUDO sai; `n_externos` fica.
                        d.externos.clear();
                    }''',1)

# `tamanho()` usa o vetor, e com o vetor vazio daria errado -- mas so e
# chamado na gravacao. Deixa explicito.
s=s.replace('''    /// Bytes que este registro ocupa no arquivo.
    pub fn tamanho(&self) -> usize {''','''    /// Bytes que este registro ocupa no arquivo.
    ///
    /// So vale com os anexos carregados: numa `Descartada` que veio de uma
    /// listagem leve, `externos` esta vazio e a conta sai menor.
    pub fn tamanho(&self) -> usize {''',1)
io.open(p,'w',encoding='utf-8').write(s)
