# Refactor Table to hold schema without per-row clones
# 27/08 17:54

p='crates/phxsql-store/src/table.rs'
s=open(p).read()

# 1. Guardar o esquema no proprio Table, em vez de clonar a cada linha.
s=s.replace('''pub struct Table {
    nome: String,
    diretorio: PathBuf,
    reg: RegFile,''','''pub struct Table {
    nome: String,
    diretorio: PathBuf,
    /// Copia do esquema que mora no `.reg`. Fica aqui para nao ser clonada a
    /// cada linha lida ou gravada.
    esquema: Schema,
    reg: RegFile,''')

s=s.replace('''        let ndx = NdxFile::criar(caminho(&diretorio, &nome, EXT_NDX), &esquema)?;
        let bin = BlobFile::criar(caminho(&diretorio, &nome, EXT_BIN), MAGIC_BIN)?;
        let memo = BlobFile::criar(caminho(&diretorio, &nome, EXT_MEMO), MAGIC_MEMO)?;
        let reg = RegFile::criar(caminho(&diretorio, &nome, EXT_REG), esquema)?;

        Ok(Table {
            nome,
            diretorio,
            reg,''','''        let ndx = NdxFile::criar(caminho(&diretorio, &nome, EXT_NDX), &esquema)?;
        let bin = BlobFile::criar(caminho(&diretorio, &nome, EXT_BIN), MAGIC_BIN)?;
        let memo = BlobFile::criar(caminho(&diretorio, &nome, EXT_MEMO), MAGIC_MEMO)?;
        let reg = RegFile::criar(caminho(&diretorio, &nome, EXT_REG), esquema.clone())?;

        Ok(Table {
            nome,
            diretorio,
            esquema,
            reg,''')

s=s.replace('''        Ok(Table {
            nome: nome.to_string(),
            diretorio,
            reg,''','''        let esquema = reg.esquema().clone();
        Ok(Table {
            nome: nome.to_string(),
            diretorio,
            esquema,
            reg,''')

s=s.replace('''    pub fn esquema(&self) -> &Schema {
        self.reg.esquema()
    }''','''    pub fn esquema(&self) -> &Schema {
        &self.esquema
    }''')

# 2. montar_payload / decodificar passam a usar o campo, sem clonar.
s=s.replace('''        let esquema = self.reg.esquema().clone();
        let mut payload = vec![0u8; esquema.payload_len()];
        let bitmap = esquema.bitmap_len();

        for (i, col) in esquema.colunas().iter().enumerate() {''','''        let mut payload = vec![0u8; self.esquema.payload_len()];

        for i in 0..self.esquema.colunas().len() {
            let col = &self.esquema.colunas()[i];''')
s=s.replace('''            let off = esquema.offset_coluna(i)?;
            let fim = off + col.ty.largura();
            match col.ty {
                ColumnType::Bin => {''','''            let off = self.esquema.offset_coluna(i)?;
            let fim = off + col.ty.largura();
            let ty = col.ty;
            let nome_col = col.nome.clone();
            match ty {
                ColumnType::Bin => {''')
s=s.replace('''                            return Err(PhxError::Tipo(format!(
                                "coluna {} espera Bin, recebeu {outro:?}",
                                col.nome
                            )))''','''                            return Err(PhxError::Tipo(format!(
                                "coluna {nome_col} espera Bin, recebeu {outro:?}"
                            )))''')
s=s.replace('''                            return Err(PhxError::Tipo(format!(
                                "coluna {} espera Memo, recebeu {outro:?}",
                                col.nome
                            )))''','''                            return Err(PhxError::Tipo(format!(
                                "coluna {nome_col} espera Memo, recebeu {outro:?}"
                            )))''')
s=s.replace('''                _ => escrever_inline(valor, &col.ty, &mut payload[off..fim])?,
            }
        }
        let _ = bitmap;
        Ok(payload)''','''                _ => escrever_inline(valor, &ty, &mut payload[off..fim])?,
            }
        }
        Ok(payload)''')

s=s.replace('''        let esquema = self.reg.esquema().clone();
        let mut linha = Vec::with_capacity(esquema.colunas().len());

        for (i, col) in esquema.colunas().iter().enumerate() {
            if payload[i / 8] & (1 << (i % 8)) != 0 {
                linha.push(Value::Null);
                continue;
            }
            let off = esquema.offset_coluna(i)?;
            let fim = off + col.ty.largura();
            let valor = match col.ty {''','''        let mut linha = Vec::with_capacity(self.esquema.colunas().len());

        for i in 0..self.esquema.colunas().len() {
            if payload[i / 8] & (1 << (i % 8)) != 0 {
                linha.push(Value::Null);
                continue;
            }
            let ty = self.esquema.colunas()[i].ty;
            let off = self.esquema.offset_coluna(i)?;
            let fim = off + ty.largura();
            let valor = match ty {''')
s=s.replace('''                _ => ler_inline(&col.ty, &payload[off..fim])?,
            };''','''                _ => ler_inline(&ty, &payload[off..fim])?,
            };''')

# 3. Demais usos de self.reg.esquema() passam ao campo.
s=s.replace('''    fn ponteiros(&self, payload: &[u8]) -> Result<Vec<(ColumnType, Ponteiro)>> {
        let esquema = self.reg.esquema();''','''    fn ponteiros(&self, payload: &[u8]) -> Result<Vec<(ColumnType, Ponteiro)>> {
        let esquema = &self.esquema;''')
s=s.replace('''    fn codificar_chave(&self, idx: usize, valores: &[Value]) -> Result<Vec<u8>> {
        let esquema = self.reg.esquema();''','''    fn codificar_chave(&self, idx: usize, valores: &[Value]) -> Result<Vec<u8>> {
        let esquema = &self.esquema;''')
s=s.replace('''        (0..self.reg.esquema().indices().len())''','''        (0..self.esquema.indices().len())''')
s=s.replace('''    fn espalhar(&self, idx: usize, chave: &[Value]) -> Result<Linha> {
        let esquema = self.reg.esquema();''','''    fn espalhar(&self, idx: usize, chave: &[Value]) -> Result<Linha> {
        let esquema = &self.esquema;''')
open(p,'w').write(s)
print("table.rs reestruturado")
