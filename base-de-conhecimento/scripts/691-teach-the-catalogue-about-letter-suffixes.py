# Teach the catalogue about letter suffixes
# 28/08 19:02

import io
p='crates/phxsql-store/src/catalogo.rs'
s=io.open(p,encoding='utf-8').read()

velho='''fn pertence(arquivo: &str, tabela: &str, ext: &str) -> bool {
    let Some(sem_ext) = arquivo.strip_suffix(&format!(".{ext}")) else {
        return false;
    };
    let Some(sufixo) = sem_ext.strip_prefix(tabela) else {
        return false;
    };
    // Ou e o nome exato, ou e o nome mais `_` e so digitos.
    sufixo.is_empty()
        || (sufixo.starts_with('_')
            && sufixo.len() > 1
            && sufixo[1..].bytes().all(|b| b.is_ascii_digit()))
}'''
novo='''fn pertence(arquivo: &str, tabela: &str, ext: &str) -> bool {
    let Some(sem_ext) = arquivo.strip_suffix(&format!(".{ext}")) else {
        return false;
    };
    let Some(sufixo) = sem_ext.strip_prefix(tabela) else {
        return false;
    };
    // Ou e o nome exato, ou e o nome mais `_` e um sufixo de volume: so
    // digitos, ou uma das 37 letras da particao alfanumerica.
    if sufixo.is_empty() {
        return true;
    }
    let Some(s) = sufixo.strip_prefix('_') else {
        return false;
    };
    !s.is_empty() && (s.bytes().all(|b| b.is_ascii_digit()) || e_balde(s))
}

/// Este sufixo e o nome de um balde da particao alfanumerica?
///
/// Comparacao contra a lista EXATA, e nao "uma letra qualquer": a lista tem 37
/// nomes e nenhum outro serve. Sem isso, `precos_historico.reg` viraria volume
/// de `precos` -- que e o defeito que esta conferencia existe para evitar,
/// agora com um caso a mais.
fn e_balde(s: &str) -> bool {
    BALDES.contains(&s)
}'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''fn nome_da_tabela(caminho: &Path) -> Option<String> {
    if caminho.extension().and_then(|s| s.to_str()) != Some(EXT_REG) {
        return None;
    }
    let base = caminho.file_stem()?.to_str()?;
    match base.rsplit_once('_') {
        Some((antes, sufixo))
            if !antes.is_empty()
                && !sufixo.is_empty()
                && sufixo.chars().all(|c| c.is_ascii_digit()) =>
        {
            Some(antes.to_string())
        }
        _ => Some(base.to_string()),
    }
}'''
novo2='''fn nome_da_tabela(caminho: &Path) -> Option<String> {
    if caminho.extension().and_then(|s| s.to_str()) != Some(EXT_REG) {
        return None;
    }
    let base = caminho.file_stem()?.to_str()?;
    let Some((antes, sufixo)) = base.rsplit_once('_') else {
        return Some(base.to_string());
    };
    if antes.is_empty() || sufixo.is_empty() {
        return Some(base.to_string());
    }
    if sufixo.chars().all(|c| c.is_ascii_digit()) {
        return Some(antes.to_string());
    }
    // Sufixo de LETRA so conta como balde quando o volume 1 esta ali do lado.
    //
    // A conferencia existe por causa de uma ambiguidade real: uma tabela
    // chamada `dados_X` e o balde X de uma tabela `dados` se escrevem igual.
    // O volume 1 (`_A`) nasce com a tabela alfanumerica e nunca falta, entao a
    // presenca dele e o que separa os dois casos -- e uma tabela `dados_X`
    // sozinha continua sendo ela mesma.
    if e_balde(sufixo)
        && caminho
            .with_file_name(format!("{antes}_{}.{EXT_REG}", BALDES[0]))
            .exists()
    {
        return Some(antes.to_string());
    }
    Some(base.to_string())
}'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
