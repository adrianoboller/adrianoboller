# Record the gate lesson
# 29/08 00:37

import pathlib
p = pathlib.Path("/home/user/adrianoboller/CLAUDE.md")
s = p.read_text()
alvo = '''**Número digitado à mão envelhece calado.**'''
novo = '''**Portão de permissão é UM só — e o campo que ele lê é o furo.** O direito por
tabela entrou no despachar, que confere o campo `"tabela"` do pedido. Duas
operações não têm esse campo: `juntar` guarda as tabelas em `a.tabela` e
`b.tabela`, e `unir` guarda numa **lista**. Sem conferência própria, bastaria
pedir a tabela negada como o lado B de uma junção. **Quando o portão passar a
olhar um campo novo, procure quem não tem esse campo** — e não espalhe o portão
por quarenta operações, porque a que alguém esquecer vira a porta dos fundos e
ninguém acha por leitura.

E o teste que mais importa numa regra de permissão nova é o do comportamento
**velho**: `sem_regra_de_tabela_nada_muda`. Regra que muda o significado da
configuração que já existe tira o direito de alguém sem ninguém ter pedido.

**Configuração que não é lida mente.** `recursos.cache_paginas` estava no
`config.json`, no MANUAL e na tela desde a 0.13.0, e **nenhuma linha de código
o lia** — o campo dizia "4096 páginas do `.ndx` em memória" quando não havia
cache nenhum. Campo de configuração sem leitor é pior que campo ausente: o
ausente ninguém ajusta esperando efeito.

**Número digitado à mão envelhece calado.**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
