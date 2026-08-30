# Compose the page with the grid and add the export flag
# 27/08 21:51

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
s=s.replace('''    if args.iter().any(|a| a == "--gerar-chave") {
        return gerar_chave();
    }''','''    if args.iter().any(|a| a == "--gerar-chave") {
        return gerar_chave();
    }

    // A pagina exatamente como o servidor a serve, para um arquivo.
    //
    // Sai da MESMA funcao que atende o navegador. Se saisse de outro lugar,
    // um dia o arquivo e a pagina servida diriam coisas diferentes -- e
    // ninguem descobriria ate alguem reclamar de um defeito que "aqui nao
    // acontece".
    if args.iter().any(|a| a == "--pagina") {
        let _ = std::io::stdout().write_all(phxsql_server::http::montar_pagina().as_bytes());
        return ExitCode::SUCCESS;
    }''')
s=s.replace('//! phxsqld --gerar-chave             gera um par de chaves Ed25519',
            '//! phxsqld --gerar-chave             gera um par de chaves Ed25519\n//! phxsqld --pagina                  escreve o Centro de Controle num arquivo')
s=s.replace('  phxsqld --gerar-chave             gera um par de chaves Ed25519 (2o fator)',
            '  phxsqld --gerar-chave             gera um par de chaves Ed25519 (2o fator)\n  phxsqld --pagina > centro.html    o Centro de Controle como arquivo unico')
open(p,'w').write(s)
