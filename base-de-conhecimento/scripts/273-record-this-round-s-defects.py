# Record this round's defects
# 28/08 11:02

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md')
s = p.read_text()
v = '''- A receita de medição do `LEIA-ME.md` do dossiê agora lista **exatamente** os
  arquivos contados.

## 5. Ninguém pediu, mas a medição aponta'''
n = '''- A receita de medição do `LEIA-ME.md` do dossiê agora lista **exatamente** os
  arquivos contados.

### O que a rodada da gestão de tabelas achou

Quatro defeitos, três deles nas próprias operações novas — construir a tela é o
que os fez aparecer.

- **Um servidor `somente_leitura` teria deixado apagar tabela.** As três
  operações novas entraram no despacho e ficaram de fora de `OPS_ESCRITA`, a
  lista que o modo somente-leitura consulta. Criar e *excluir* tabela passariam
  num servidor marcado como só de leitura. A lista é escrita à mão, então o
  conserto veio com um teste que a percorre: quem acrescentar operação que
  grava e esquecer da lista quebra o teste.

- **`criar_schema` estava prometido em dois lugares e não existia.** Aparecia na
  tabela de permissões do `docs/USUARIOS.md` e na lista de operações de
  escrita; pedir pela rede respondia «operacao desconhecida». A biblioteca já
  sabia criar a pasta — faltava a porta. Agora existe, e a tela de nova tabela
  tem o campo.

- **A largura do sufixo entrava depois do teto de volumes.** `Paginacao::nova`
  confere o teto contra os três dígitos do padrão, então pedir 9.999 volumes
  era recusado *antes* de o quarto dígito existir. Entrou
  `com_max_arquivos`, e a ordem passou a ser: largura primeiro, teto depois.
  Também virou explícito que **«sem teto» não existe**: o sufixo tem largura
  fixa, e com três dígitos o volume 1000 não teria nome de arquivo. Teto
  omitido agora vira o maior que cabe, em vez de zero — que o validador
  recusava com uma mensagem que não ajudava quem preencheu a tela.

- **A árvore roubava a tela de quem pintasse depois dela.** `montarArvore`
  terminava sempre clicando no Painel; criar uma tabela redesenhava a árvore,
  voltava para a grade — e meio segundo depois o painel chegava por cima. Só
  apareceu no teste de navegador, e só depois que o formulário ficou maior.
  Quem vai pintar a própria tela agora passa `montarArvore(false)`.

## 5. Ninguém pediu, mas a medição aponta'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
