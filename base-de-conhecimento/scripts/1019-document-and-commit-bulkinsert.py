# Document and commit BULKINSERT
# 29/08 03:01

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''- **O `.log` deixou de atrasar o `.reg`.**'''
novo = '''- **`BULKINSERT`: a tabela reservada para a carga** (pedido 128). Uma carga
  longa quer duas coisas que o servidor não dava: ninguém mais mexendo naquela
  tabela enquanto ela entra, e uma sincronização só, no fim.

  ```
  {"op":"bulkinsert","database":"Z","tabela":"Clientes","ligado":true}
  ... as inserções ...
  {"op":"bulkinsert","database":"Z","tabela":"Clientes","ligado":false}
  ```

  **1,53× medido** — 43.044 e 44.026 sem reserva contra 65.737 e 67.339
  linhas/s com ela, dois pares de corridas. O ganho vem da janela de
  durabilidade: reservada, ela não fecha, e a carga inteira vira um `fsync` só.

  Os outros recebem **erro na hora**, e não espera: o novo **4002
  `EM_CARGA`**, dizendo **quem** reservou e **desde quando** — sem isso,
  «tabela em carga» manda a pessoa procurar sozinha quem está segurando. Ele
  vem com `repetir: true`, e passa a ser o **segundo** erro do protocolo que
  pede nova tentativa (o outro é o de E/S): é o que separa «espere um pouco»
  de «você não pode». A leitura também para, e é de propósito — deixar ler
  durante a carga é o que impediria adiar o índice mais tarde.

  Contra reserva órfã há **duas** redes, e não uma: a **queda da conexão**
  solta na hora, por qualquer caminho de saída; e o **prazo**
  (`recursos.carga_prazo_min`, padrão 30 min) solta o soquete que ficou
  pendurado vivo com o cliente morto do outro lado — que é exatamente o caso
  em que a primeira não pega.

  Só pela porta de dados: HTTP não tem conexão para cair. Pela tela,
  `inserir_lote` já é uma operação só.

  10 testes, mais a prova pelo soquete em `bancada/carga/bulkinsert.py` — e foi
  ela que achou o que os testes unitários não achavam.

- **O `.log` deixou de atrasar o `.reg`.**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
