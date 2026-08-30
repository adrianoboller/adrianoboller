# Fix reg imports and volume discovery
# 27/08 18:24

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()

s=s.replace('''use phxsql_core::schema::Schema;
use phxsql_core::RowId;''','''use phxsql_core::schema::Schema;
use phxsql_core::{RowId, EXT_REG};''')
s=s.replace("crate::EXT_REG","EXT_REG")

# Sonda de abertura: descobrir o primeiro volume por varredura de diretorio,
# em vez de chutar a largura do sufixo.
s=s.replace('''    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<RegFile> {
        // A paginacao esta dentro do esquema, que esta dentro do volume 1.
        // Como o volume 1 tem o mesmo nome com ou sem sufixo, abrimos primeiro
        // sem paginacao para ler o cabecalho e so depois montamos o conjunto.
        let sem = Volumes::novo(&diretorio, nome, EXT_REG, Paginacao::DESLIGADA);
        let paginado = Volumes::novo(
            &diretorio,
            nome,
            EXT_REG,
            Paginacao::nova(1, 999)?,
        );
        let mut sonda = if sem.existe(1) { sem } else { paginado };

        let mut cab = [0u8; CAB_LEN];
        sonda.ler(1, 0, &mut cab)?;
        let nome_arq = sonda.caminho(1).display().to_string();''','''    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<RegFile> {
        // A paginacao mora dentro do esquema, que mora dentro do primeiro
        // volume -- e a largura do sufixo faz parte dela. Para nao chutar,
        // acha-se o primeiro volume varrendo o diretorio e le-se o cabecalho
        // direto, antes de montar o conjunto de volumes.
        let primeiro = achar_primeiro_volume(diretorio.as_ref(), nome, EXT_REG)?;
        let nome_arq = primeiro.display().to_string();
        let bruto = std::fs::read(&primeiro)?;
        if bruto.len() < CAB_LEN {
            return Err(PhxError::Corrompido(format!("{nome_arq} truncado")));
        }
        let mut cab = [0u8; CAB_LEN];
        cab.copy_from_slice(&bruto[..CAB_LEN]);''')

s=s.replace('''        let mut bytes_esquema = vec![0u8; schema_len];
        sonda.ler(1, CAB_LEN as u64, &mut bytes_esquema)?;
        if crc32(&bytes_esquema) != schema_crc {''','''        if bruto.len() < CAB_LEN + schema_len {
            return Err(PhxError::Corrompido(format!(
                "{nome_arq} nao contem o esquema inteiro"
            )));
        }
        let bytes_esquema = bruto[CAB_LEN..CAB_LEN + schema_len].to_vec();
        if crc32(&bytes_esquema) != schema_crc {''')

s=s.replace('''fn alinhar(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}''','''fn alinhar(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

/// Acha o primeiro volume de um conjunto sem saber, de antemao, se a tabela e
/// paginada nem qual a largura do sufixo.
///
/// Procura primeiro `nome.ext` (tabela em arquivo unico); se nao existir,
/// varre o diretorio atras de `nome_<digitos>.ext` e devolve o menor.
fn achar_primeiro_volume(diretorio: &Path, nome: &str, ext: &str) -> Result<PathBuf> {
    let simples = diretorio.join(format!("{nome}.{ext}"));
    if simples.exists() {
        return Ok(simples);
    }
    let prefixo = format!("{nome}_");
    let mut candidatos: Vec<PathBuf> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if p.extension().and_then(|s| s.to_str()) != Some(ext) {
                return false;
            }
            match p.file_stem().and_then(|s| s.to_str()) {
                Some(base) => match base.strip_prefix(&prefixo) {
                    Some(sufixo) => {
                        !sufixo.is_empty() && sufixo.chars().all(|c| c.is_ascii_digit())
                    }
                    None => false,
                },
                None => false,
            }
        })
        .collect();
    candidatos.sort();
    candidatos.into_iter().next().ok_or_else(|| {
        PhxError::NaoEncontrado(format!(
            "nenhum volume de {nome}.{ext} em {}",
            diretorio.display()
        ))
    })
}''')
open(p,'w').write(s)
print("reg.rs ajustado")
