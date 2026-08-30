# Commit the fixes found by the video
# 28/08 22:20

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
antigo = """### Corrigido

- **A bissecção pelo `rownum` estava errada na partição alfanumérica"""
novo = """### Corrigido

- **Salvar e incluir pela tela estavam quebrados desde que o `rownum` entrou.**
  A ficha tirava só a **primeira** coluna de sistema (`find(c => c.sistema)`),
  então o `rownum` continuava no formulário — editável, o que já é errado — e o
  cliente mandava 8 valores para uma tabela de 9 colunas. Toda gravação pela
  interface morria com «a lista tem 8 valores». Achado **gravando o vídeo de
  demonstração**: o erro aparece no canto da tela num quadro do capítulo 9.
  Teste novo trava a linha curta dos dois lados — inclusão e alteração — e
  confirma que as duas colunas de sistema são herdadas, não zeradas.

- **A tela da Replicação dizia que a replicação não existia.** Ela ainda
  explicava que «as portas são configuração, não serviço» e que faltava o
  `.log` v2 — texto verdadeiro na 0.14.0 e falso agora. Passou a mostrar o
  papel, se a imagem está ligada, de onde a réplica puxa e a **posição de cada
  tabela**, que é o número que diz se ela está em dia.

- **O erro da réplica saía sempre como «acesso negado».** Um database que ainda
  não existe no master aparecia no log como problema de autorização — o pior
  tipo de mensagem, a que manda procurar no lugar errado. O erro do outro lado
  já vem classificado (`nome` e `classe` estão na resposta) e agora é
  reembalado com a classe certa.

- **A bissecção pelo `rownum` estava errada na partição alfanumérica"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
