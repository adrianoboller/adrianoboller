# Update changelog and pending list
# 28/08 20:35

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
antigo = """## 0.15.0 — 2026-08-28

**Carga em lote** e o **salto para a página 500** — que a versão anterior tinha
deixado escrito como o que faltava."""
novo = """## 0.15.0 — 2026-08-28

**Replicação funcionando**, **carga em lote** e o **salto para a página 500** —
os três estavam escritos como o que faltava, e os três saíram."""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """### Adicionado

- **`inserir_lote`: várias linhas num pedido só.**"""
novo = """### Adicionado

- **A replicação Master → Réplica está no ar.** Quatro servidores medidos em
  `bancada/replicacao/`, com o Master e três espelhos:

  | | |
  |---|---|
  | Master, com a imagem no diário | 18.773 linhas/s |
  | Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |
  | Atraso de uma escrita até as três | 1,3 s a 2,1 s |
  | Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 343 ms + 1,0 s |
  | Retrato SHA-256 das quatro tabelas, no fim | idênticos |

  A bancada não compara «quantas linhas»: compara um SHA-256 de **cada linha
  inteira**, com `rowid` e `rownum` juntos. O `rowid` entrar na conta é o
  ponto — ele não é transmitido: o `.reg` nunca reaproveita slot, então uma
  réplica que aplicou tudo na ordem chega ao mesmo número sozinha. Se não
  chegar, divergiu, e a replicação **para ali** em vez de espalhar.

- **`.log` v2 com a imagem da linha.** Era a única peça que faltava, e ela é
  o payload **cru** do `.reg` mais o **conteúdo** dos anexos — não os
  ponteiros, que são offsets desta máquina e apontariam para qualquer coisa na
  outra. Atrás de `replicacao.imagem_da_linha`, ligada sozinha num `source`.
  Medido, mesma tabela e mesmas 100.000 linhas: **10% mais devagar e um diário
  5,1× maior** (44 → 223 bytes por evento).

- **`posicao`, `replicar` e `aplicar` no protocolo**, e o laço da réplica
  dentro do próprio `phxsqld` — uma thread por origem, `papel: replica` e uma
  origem no `config.json` bastam. A tabela que ainda não existe na réplica
  nasce do **bloco de esquema cru** do source, e não de uma remontagem coluna a
  coluna a partir de JSON.

- **A senha da réplica não fica em claro nem viaja.** Ela se autentica pelo
  mesmo desafio-resposta do resto do protocolo, com a chave derivada do
  `senha_hash` que mora no `config.json` dela.

- **Cascata**: uma réplica pode ser origem de outra. Master → Slave01 → Slave03
  mediu 1.827 ms contra 1.679 ms do primeiro salto.

- **`inserir_lote`: várias linhas num pedido só.**"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """### Mudado

- **`.reg` v3 → v4**: o contador `marcadas` nos bytes 108..116 do volume 1."""
novo = """### Mudado

- **`.log` v1 → v2**: o cabeçalho do evento passou de 36 para 44 bytes, e o
  evento deixou de ter largura fixa. Isso cobra um preço: até a v1 o evento N
  morava no offset `64 + N × 36` e pular era uma conta; agora chegar ao evento
  N é caminhar pelos anteriores. O que salva a leitura é o `qtd_eventos` do
  cabeçalho de cada volume — um volume inteiro se pula sem abrir.

- **O CRC do evento passou a cobrir a imagem**, e não só o cabeçalho. A imagem
  é o que a réplica grava **como dado**: um byte trocado ali entraria na
  réplica sem ninguém notar.

- **`.reg` v3 → v4**: o contador `marcadas` nos bytes 108..116 do volume 1."""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """### Sabido

- **Não há transação, e o lote não muda isso.**"""
novo = """### Sabido

- **A réplica aplica mais devagar do que o master escreve** — 4.273 eventos/s
  contra 18.773 linhas/s, com as três competindo pela mesma máquina. Sob carga
  sustentada elas ficam para trás. A razão está no caminho: aplicar decodifica
  a imagem para `Value` e **reencoda** o payload, em vez de gravar os bytes que
  vieram. Gravar o payload direto, remendando só os ponteiros dos anexos, é o
  próximo ganho grande.

- **O atraso da réplica é o intervalo do laço, não o trabalho.** Com
  `reconectar_em: 2` uma escrita leva de 1,3 s a 2,1 s para chegar. Baixar o
  intervalo baixa o atraso e sobe o tráfego de perguntas em vão; o `long-poll`
  — o source segurar a resposta até ter novidade — ainda não existe.

- **O JSON da replicação vai em claro**, e a imagem vai em hexadecimal, que
  dobra o tamanho. Não há TLS no transporte: por enquanto ele depende do túnel.

- **Não há transação, e o lote não muda isso.**"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
