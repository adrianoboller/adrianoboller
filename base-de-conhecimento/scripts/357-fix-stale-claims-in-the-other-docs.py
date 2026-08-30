# Fix stale claims in the other docs
# 28/08 13:13

import pathlib
trocas = {
 'README.md': [
   ('| Barra de menu tradicional — sete menus, atalhos e navegação por teclado | pronto |',
    '| Barra de menu tradicional — nove menus, atalhos e navegação por teclado | pronto |'),
 ],
 'docs/PENDENCIAS.md': [
   ('**View Database com edição** e **gestão de tabelas** — 30 das 33 operações. Fora: `buscar`, `desbloquear` e `criar_schema`, que acontece sozinho quando a tela cria tabela dentro de um schema |',
    '**View Database com edição**, **gestão de tabelas** e **gestão do banco** — 33 das 36 operações. Fora: `buscar`, `desbloquear` e `criar_schema`, que acontece sozinho quando a tela cria tabela dentro de um schema |'),
   ('| 15 ferramentas, ícone colorido; **10 funcionam**, 5 apagadas dizendo o que falta |',
    '| 20 ferramentas hoje, ícone colorido; **16 funcionam**, 4 apagadas dizendo o que falta |'),
   ('| ☑️ | 63 | **Barra de menu superior tradicional** | seis menus, 22 recursos, Alt/setas/Esc |',
    '| ☑️ | 63 | **Barra de menu superior tradicional** | nove menus e 53 itens hoje, Alt/setas/Esc |'),
 ],
 'MANUAL.txt': [
   ('    corrente. Um clique numa linha abre as oito operacoes sobre ela:',
    '    corrente. Um clique numa linha abre as operacoes sobre ela:'),
 ],
}
for arq, ts in trocas.items():
    p = pathlib.Path(arq); s = p.read_text()
    for v, n in ts:
        assert s.count(v) == 1, f'{arq}: {v[:50]}'
        s = s.replace(v, n)
    p.write_text(s)
    print(f'{arq}: {len(ts)} correção(ões)')
