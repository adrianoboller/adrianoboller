# Acrescentar as duas entradas ao CHANGELOG
# 29/08 03:09

import io
p='CHANGELOG.md'
s=io.open(p,encoding='utf-8').read()

adicionado = '''- **`docs/SQL.md`: o que a camada SQL precisa saber, antes de existir.** O
  motor tem hoje um protocolo de operações, e não uma linguagem. O documento
  mapeia cada construção de SQL na operação que já existe — e é curto de
  propósito: a maior parte de um `SELECT` já tem substrato, e o que **não**
  tem está listado com nome (expressão, planejador, `GROUP BY` geral,
  subconsulta, transação).

  Ele nasceu de uma pergunta específica: como o `BULKINSERT` entra numa
  linguagem. A resposta é que ele **não** é açúcar sintático, por três motivos
  que o analisador não pode ignorar — é palavra reservada; vale para a
  **sessão**, e não para o comando, então um driver que multiplexa conexões
  quebra a exclusividade sem avisar; e o `EM_CARGA` tem de virar
  *serialization failure* no SQLSTATE, e não *access denied*, senão o driver
  do outro lado desiste em vez de repetir.

  E a frase que o documento repete alto: **`BULKINSERT` não é transação.** Ele
  reserva a tabela; não desfaz nada. Quem ler «exclusiva até concluir» e
  entender `BEGIN` vai perder dado.

'''

mudado = '''- **A tela de configuração explica cada ajuste, em vez de despejar o JSON.**
  Ela mostrava o `config.json` cru — o que serve para conferir, e não para
  decidir. Agora cada campo de `recursos` vem com uma linha dizendo o que ele
  muda de verdade (`cache_paginas`, `carga_prazo_min`, `nucleos_efetivos`…),
  e há uma seção **«Cargas em andamento»** listando as reservas de
  `BULKINSERT` — quem, qual tabela, desde quando. O JSON continua embaixo.

  Conferida no navegador, e não só lida: foi assim que `nucleos_efetivos`
  apareceu com a explicação em branco. Quem não tem `administrar` vê a tela
  sem a seção de cargas, e não um erro.

'''

marca = '\n### Mudado\n\n- **`recursos.cache_paginas` passou a valer.**'
assert s.count(marca)==1
s = s.replace(marca, '\n' + adicionado + '### Mudado\n\n' + mudado + '- **`recursos.cache_paginas` passou a valer.**')
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
