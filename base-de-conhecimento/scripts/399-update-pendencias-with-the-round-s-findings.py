# Update PENDENCIAS with the round's findings
# 28/08 14:07

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md'); s = p.read_text()
v = '''| ☑️ | 77 | **Group dinâmico'''
n = '''| ☑️ | 79 | **Seção de cache, memória, CPU, threads e usuários no `config.json`** | seção `recursos`, com sete ajustes. `cache_paginas` e `memoria_max_mb` são lidos e mostrados mas **ainda não impostos** — o buffer pool é o trabalho seguinte |
| ☑️ | 80 | **Validar e revisar o motor de insert; deixar a gravação mais rápida** | medido: **95% do tempo era `fsync`**. Durabilidade configurável dá **20,4×**. E a medição achou uma **perda silenciosa de dado** sob gravação concorrente, corrigida |
| ☑️ | 81 | **Tabela `sequences` na raiz do banco**, com todas as tabelas e um BigInt ajustável pelo admin | operações `sequencias` e `ajustar_sequencia`. O contador continua no cabeçalho de cada `.reg`: a operação junta para mostrar, e não cria uma segunda cópia que divergiria |
| ☑️ | 82 | **Bancos em pastas, cada schema uma subpasta** | já era assim desde o início — conferido: `dados/loja/matriz/estoque.reg` |
| ◐ | 83 | **Comandos SQL reconhecem `matriz.estoque` e `filial.estoque`** | o **endereçamento** funciona hoje em toda operação: `tabela: "matriz.estoque"` abre a pasta certa. O que falta é o **SQL** — não há parser, e ele é o planejado nº 6 |
| ☑️ | 77 | **Group dinâmico'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''**70 feitos · 2 parciais · 6 planejados**, de 78 pedidos.'''
n = '''**74 feitos · 3 parciais · 6 planejados**, de 83 pedidos.'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''### O que a revisão do dossiê achou'''
n = '''### O que a revisão do motor de insert achou

- **Perda silenciosa de dado sob gravação concorrente.** O servidor tomava a
  trava para abrir a tabela, **soltava**, e só então tomava de novo para
  gravar. Abrir lê o cabeçalho, e o cabeçalho traz o `slot_count` — o contador
  que decide onde a próxima linha vai. Nessa fresta duas operações abriam a
  tabela, as duas guardavam `slot_count = N`, e as duas gravavam no rowid N+1:
  a segunda por cima da primeira, **sem erro nenhum**.

  Com índice único sobre a coluna, o índice pegava e virava «chave duplicada» —
  foi assim que apareceu. Sem índice único, a linha sumia em silêncio.

  A trava passa a cobrir abrir *e* gravar, num bloco só. Um teste deixa o
  contrato escrito.

- **95% do tempo da inserção era `fsync`.** O diagnóstico anterior — «97% CPU,
  disco parado, a culpa é da B+tree» — foi medido com a *biblioteca*, que não
  sincroniza por linha. Pelo *servidor*, que sincronizava, o gargalo era outro.
  As duas medições estavam certas; era a conclusão que estava sendo aplicada ao
  caminho errado.

- E um susto meu que não era defeito: caçei por meia hora um contador de
  sequência que «zerava sozinho». Era o meu próprio teste, com um caso rotulado
  «tabela sem Sequence» apontando para uma tabela que tinha Sequence — ele
  zerou o contador porque foi exatamente isso que eu pedi.

### O que a revisão do dossiê achou'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('PENDENCIAS')
