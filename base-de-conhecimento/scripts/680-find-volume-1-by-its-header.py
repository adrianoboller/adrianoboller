# Find volume 1 by its header
# 28/08 18:52

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()
velho='''/// Acha o primeiro volume de um conjunto sem saber, de antemao, se a tabela e
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
}'''

novo='''/// Acha o volume 1 de um conjunto sem saber, de antemao, se a tabela e
/// paginada, qual a largura do sufixo, nem se o sufixo e numero ou letra.
///
/// Procura primeiro `nome.ext` (tabela em arquivo unico). Se nao existir,
/// varre o diretorio e escolhe pelo **cabecalho**, e nao pelo nome: o volume 1
/// e o que se declara volume 1 nos bytes 12..16.
///
/// Pelo nome nao daria. Na particao alfanumerica os sufixos sao `_A`.. `_Z`,
/// `_0`.. `_9` e `_Outros`, e ordenar texto poria `_0` antes de `_A` -- o que
/// escolheria como volume 1 um arquivo que nao tem os contadores da tabela.
/// Ler 128 bytes de cada candidato uma vez, na abertura, custa nada: volume e
/// coisa que se conta em dezenas.
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
                Some(base) => base
                    .strip_prefix(&prefixo)
                    .is_some_and(|sufixo| !sufixo.is_empty()),
                None => false,
            }
        })
        .collect();
    candidatos.sort();

    for c in &candidatos {
        let mut cab = [0u8; CAB_LEN];
        let Ok(mut f) = File::open(c) else { continue };
        if f.read_exact(&mut cab).is_err() {
            continue;
        }
        if &cab[0..8] == MAGIC_REG && Campos(&cab).u32(12) == 1 {
            return Ok(c.clone());
        }
    }

    // Nenhum se declarou volume 1. Devolve o menor por nome, para a mensagem
    // de erro seguinte falar do cabecalho e nao do diretorio vazio.
    candidatos.into_iter().next().ok_or_else(|| {
        PhxError::NaoEncontrado(format!(
            "nenhum volume de {nome}.{ext} em {}",
            diretorio.display()
        ))
    })
}'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
