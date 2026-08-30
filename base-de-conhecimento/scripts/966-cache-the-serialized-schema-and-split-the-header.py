# Cache the serialized schema and split the header write
# 29/08 01:07

import pathlib
p = pathlib.Path("crates/phxsql-store/src/reg.rs")
s = p.read_text()

# 1) campos novos
s = s.replace('''pub struct RegFile {
    volumes: Volumes,
    esquema: Schema,''','''pub struct RegFile {
    volumes: Volumes,
    esquema: Schema,
    /// O bloco de esquema ja serializado, e o CRC dele.
    ///
    /// O esquema NAO MUDA depois que a tabela e criada ou aberta -- e o
    /// cabecalho e regravado a cada insercao, para os contadores irem ao
    /// disco. Sem isto, cada linha inserida reserializava o esquema inteiro e
    /// recalculava o CRC dele: trabalho identico, resultado identico, uma vez
    /// por linha.
    esquema_bytes: Vec<u8>,
    esquema_crc: u32,''',1)

# 2) construtor `criar`
s = s.replace('''        let mut r = RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, paginacao),
            esquema,
            slot_size,''','''        let esquema_crc = crc32(&bytes_esquema);
        let mut r = RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, paginacao),
            esquema,
            esquema_bytes: bytes_esquema,
            esquema_crc,
            slot_size,''',1)

# 3) construtor `abrir`
s = s.replace('''        let mut r = RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, esquema.paginacao()),
            esquema,
            slot_size,''','''        let bytes_esquema = esquema.serializar();
        let esquema_crc = crc32(&bytes_esquema);
        let mut r = RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, esquema.paginacao()),
            esquema,
            esquema_bytes: bytes_esquema,
            esquema_crc,
            slot_size,''',1)

# 4) gravar_cabecalho passa a usar o cache, e ganha um irmao so de contadores
alvo = '''    fn gravar_cabecalho(&mut self, volume: u32) -> Result<()> {
        let bytes_esquema = self.esquema.serializar();
        let mut buf = [0u8; CAB_LEN];'''
novo = '''    /// So os 128 bytes do cabecalho do volume 1, com os contadores.
    ///
    /// # Por que existe, separado do `gravar_cabecalho`
    ///
    /// Toda insercao precisa levar `slot_count` e companhia ao disco -- sao
    /// eles que dizem onde a proxima linha entra. Nao precisa reescrever o
    /// BLOCO DE ESQUEMA junto, que e imutavel e ja esta la desde a criacao do
    /// volume; nem conferir o tamanho do arquivo, que so encolheria se alguem
    /// o truncasse por fora.
    ///
    /// Antes disto, cada linha inserida custava: serializar o esquema inteiro,
    /// calcular o CRC-32 dele, gravar o cabecalho, gravar o bloco de esquema
    /// de novo e perguntar o tamanho do arquivo. Cinco coisas, das quais uma
    /// era necessaria.
    fn gravar_contadores(&mut self) -> Result<()> {
        let buf = self.montar_cabecalho(1);
        self.volumes.escrever(1, 0, &buf)
    }

    fn gravar_cabecalho(&mut self, volume: u32) -> Result<()> {
        let buf = self.montar_cabecalho(volume);
        self.volumes.escrever(volume, 0, &buf)?;
        self.volumes
            .escrever(volume, CAB_LEN as u64, &self.esquema_bytes.clone())?;
        if self.volumes.tamanho(volume)? < self.data_offset {
            self.volumes.definir_tamanho(volume, self.data_offset)?;
        }
        Ok(())
    }

    fn montar_cabecalho(&self, volume: u32) -> [u8; CAB_LEN] {
        let mut buf = [0u8; CAB_LEN];'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# 5) o corpo do antigo gravar_cabecalho vira o montar_cabecalho
s = s.replace('''        por_u32(&mut buf, 52, bytes_esquema.len() as u32);
        por_u32(&mut buf, 56, crc32(&bytes_esquema));''',
'''        por_u32(&mut buf, 52, self.esquema_bytes.len() as u32);
        por_u32(&mut buf, 56, self.esquema_crc);''',1)

s = s.replace('''        let crc = crc32(&buf[..124]);
        por_u32(&mut buf, 124, crc);

        self.volumes.escrever(volume, 0, &buf)?;
        self.volumes
            .escrever(volume, CAB_LEN as u64, &bytes_esquema)?;
        if self.volumes.tamanho(volume)? < self.data_offset {
            self.volumes.definir_tamanho(volume, self.data_offset)?;
        }
        Ok(())
    }''','''        let crc = crc32(&buf[..124]);
        por_u32(&mut buf, 124, crc);
        buf
    }''',1)
p.write_text(s)
print("ok")
