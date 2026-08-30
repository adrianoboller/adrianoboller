# Make the CLI page instead of scanning
# 28/08 19:58

import pathlib
p = pathlib.Path("crates/phxsql-cli/src/main.rs")
s = p.read_text()

antigo = """    let mut indice: Option<String> = None;
    let mut max = 20usize;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--indice" if i + 1 < args.len() => {
                indice = Some(args[i + 1].clone());
                i += 2;
            }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or(20);
                i += 2;
            }"""
novo = """    let mut indice: Option<String> = None;
    let mut max = 20usize;
    let mut pular = 0u64;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--indice" if i + 1 < args.len() => {
                indice = Some(args[i + 1].clone());
                i += 2;
            }
            "--max" if i + 1 < args.len() => {
                max = args[i + 1].parse().unwrap_or(20);
                i += 2;
            }
            "--pular" if i + 1 < args.len() => {
                pular = args[i + 1].parse().unwrap_or(0);
                i += 2;
            }"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    let rowids: Vec<u64> = match &indice {
        Some(nome_idx) => t.varrer_indice(nome_idx)?,
        None => t.varrer()?.into_iter().map(|(r, _)| r).collect(),
    };

    diga!(
        "{} linhas, na ordem {}",
        rowids.len(),
        match &indice {
            Some(n) => format!("do indice {n}"),
            None => "de digitacao (.reg)".to_string(),
        }
    );
    diga!("{:>8}  {}", "rowid", colunas.join(" | "));

    for rowid in rowids.iter().take(if max == 0 { usize::MAX } else { max }) {
        if let Some(linha) = t.ler(*rowid)? {
            let campos: Vec<String> = linha
                .iter()
                .zip(tipos.iter())
                .map(|(v, ty)| mostrar(v, ty))
                .collect();
            diga!("{rowid:>8}  {}", campos.join(" | "));
        }
    }
    if max != 0 && rowids.len() > max {
        diga!(
            "... (+{} linhas; use --max 0 para tudo)",
            rowids.len() - max
        );
    }
    Ok(())
}"""
novo = """    // O teto entra na LEITURA, e nao depois dela. Ler a tabela inteira para
    // mostrar vinte linhas custava a tabela inteira -- numa de 200 mil linhas
    // com memo, segundos de espera para uma tela que cabe num terminal.
    let teto = if max == 0 { 0 } else { max as u64 };
    let (rowids, salto) = match &indice {
        // Por indice a ordem e a da CHAVE, e nao a do arquivo: ali nao ha
        // conta que leve a posicao N, e a lista de rowids ja veio inteira.
        Some(nome_idx) => {
            let todos = t.varrer_indice(nome_idx)?;
            let corte: Vec<u64> = todos
                .into_iter()
                .skip(pular as usize)
                .take(if max == 0 { usize::MAX } else { max })
                .collect();
            (corte, None)
        }
        None => {
            let (r, como) = t.pagina_por_posicao(pular, teto, Visao::Ativas)?;
            (r, Some(como))
        }
    };

    let total = t.contar(Visao::Ativas);
    diga!(
        "{} linha(s) de {total}, na ordem {}{}",
        rowids.len(),
        match &indice {
            Some(n) => format!("do indice {n}"),
            None => "de digitacao (.reg)".to_string(),
        },
        match salto {
            // Vale dizer: e a diferenca entre vinte leituras e `pular` delas.
            Some(Salto::Bissecao) => ", achada por bisseccao no rownum".to_string(),
            Some(Salto::Passo) if pular > 0 => format!(", andando {pular} linha(s)"),
            _ => String::new(),
        }
    );
    diga!("{:>8}  {}", "rowid", colunas.join(" | "));

    for rowid in &rowids {
        if let Some(linha) = t.ler(*rowid)? {
            let campos: Vec<String> = linha
                .iter()
                .zip(tipos.iter())
                .map(|(v, ty)| mostrar(v, ty))
                .collect();
            diga!("{rowid:>8}  {}", campos.join(" | "));
        }
    }
    let vistas = pular + rowids.len() as u64;
    if max != 0 && vistas < total {
        diga!(
            "... (+{} linha(s); use --pular {vistas} para a proxima pagina, ou --max 0 para tudo)",
            total - vistas
        );
    }
    Ok(())
}"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """  phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>]"""
novo = """  phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>] [--pular <n>]"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
