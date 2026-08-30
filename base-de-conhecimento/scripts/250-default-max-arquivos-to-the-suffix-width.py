# Default max_arquivos to the suffix width
# 28/08 10:41

import pathlib
p = pathlib.Path('crates/phxsql-server/src/valores.rs')
s = p.read_text()
v = '''    let por_arquivo = j.inteiro_ou("registros_por_arquivo", 0);
    Ok(if por_arquivo > 0 {
        let max = j.inteiro_ou("max_arquivos", 0).max(0) as u32;
        esquema.com_paginacao(Paginacao::nova(por_arquivo as u64, max)?)
    } else {
        esquema
    })
}'''
n = '''    let por_arquivo = j.inteiro_ou("registros_por_arquivo", 0);
    Ok(if por_arquivo > 0 {
        let digitos = j.inteiro_ou("digitos", DIGITOS_PADRAO as i64).clamp(1, 9) as u8;
        // Teto omitido nao quer dizer "sem teto": o sufixo tem largura fixa, e
        // com tres digitos o volume 1000 simplesmente nao tem nome. Entao o
        // padrao e o maior que cabe no sufixo, e nao zero -- que o validador
        // recusaria com uma mensagem que nao ajuda quem preencheu a tela.
        let cabem = 10u32.pow(digitos as u32) - 1;
        let max = match j.inteiro_ou("max_arquivos", 0).max(0) as u32 {
            0 => cabem,
            outro => outro,
        };
        esquema.com_paginacao(Paginacao::nova(por_arquivo as u64, max)?.com_digitos(digitos)?)
    } else {
        esquema
    })
}'''
assert s.count(v) == 1
s = s.replace(v, n)
s = s.replace("use phxsql_core::paginacao::Paginacao;",
              "use phxsql_core::paginacao::{Paginacao, DIGITOS_PADRAO};")
p.write_text(s)
print('ok')
