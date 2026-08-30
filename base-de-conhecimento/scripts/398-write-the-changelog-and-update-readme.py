# Write the changelog and update README
# 28/08 14:07

import pathlib
p = pathlib.Path('docs/USUARIOS.md'); s = p.read_text()
v = '`sistabelas`, `siscolunas`, `pivotar` |'
n = '`sistabelas`, `siscolunas`, `pivotar`, `sequencias` |'
assert s.count(v) == 1
s = s.replace(v, n)
v = '`usuarios`, `excluir_tabela` |'
n = '`usuarios`, `excluir_tabela`, `ajustar_sequencia` |'
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('USUARIOS.md')

p = pathlib.Path('Cargo.toml'); s = p.read_text()
p.write_text(s.replace('version = "0.9.0"', 'version = "0.10.0"'))

ENTRADA = '''## 0.10.0 — 2026-08-28

Uma correção de **perda silenciosa de dado** sob gravação concorrente, a
gravação **20× mais rápida** com durabilidade configurável, e a seção
`recursos` no `config.json`.

### Corrigido

- **Duas gravações simultâneas na mesma tabela sobrescreviam uma a outra.**
  Abrir uma tabela lê o cabeçalho, e o cabeçalho traz `slot_count` — o contador
  que decide onde a próxima linha vai. O servidor tomava a trava para abrir,
  **soltava**, e só então tomava de novo para gravar. Nessa fresta duas
  operações abriam a tabela, as duas guardavam `slot_count = N`, e as duas
  gravavam no rowid N+1: a segunda por cima da primeira, sem erro nenhum.

  Aparecia como «chave duplicada» quando havia índice único sobre a coluna
  — o índice pegava. **Sem índice único, a linha simplesmente sumia.**

  A trava passa a cobrir abrir *e* gravar, como um bloco só. Um teste em
  `tests/tabela.rs` deixa o contrato escrito: duas aberturas disputam o mesmo
  rowid, e por isso quem abre precisa serializar.

### Adicionado

- **Seção `recursos` no `config.json`**: durabilidade, tamanho do lote, cache
  de páginas, teto de memória, threads, percentual de CPU, conexões e usuários
  simultâneos. `conexoes_max` no topo continua valendo, para config antigo não
  parar de subir.

- **Durabilidade configurável**, e é o que acelera a gravação. Medido com
  20.000 linhas na mesma tabela:

  | quando sincroniza | linhas/s | ganho |
  |---|---:|---:|
  | a cada linha (o que o servidor fazia) | 1.289 | — |
  | a cada 100 | 18.264 | 14,2× |
  | a cada 1.000 | 24.858 | 19,3× |
  | só no fim | 26.301 | 20,4× |

  **95% do tempo de uma inserção era `fsync`.** Depois de tirá-lo, a inserção
  custa 37,5 µs, dos quais 65% são os dois índices — que é o gargalo seguinte,
  não este.

  Os bytes vão para o sistema operacional em toda gravação, sempre: um `write`
  direto, sem buffer nosso. Outro processo vê o dado na hora, sincronizado ou
  não. O `fsync` protege de uma coisa só: perder energia antes de o sistema
  descarregar a página.

- **Relógio de fundo** que fecha a janela de durabilidade quando ninguém grava.
  Sem ele, a última venda do dia às 18h ficaria sem `fsync` a noite inteira.

- **`sequencias`** e **`ajustar_sequencia`**: o contador de cada tabela do banco
  num lugar só, e o caminho do administrador para zerar ou pular uma faixa. O
  número continua morando no cabeçalho do `.reg` de cada tabela — a operação
  junta para mostrar, não cria uma segunda cópia.

- **`custo-do-sync`**, o medidor que produziu a tabela acima.

### Sabido

- `por_lote` é o padrão. Quem precisa de durabilidade por operação — um
  livro-razão, por exemplo — põe `"durabilidade": "por_operacao"` e paga os 20×.
- O `cpu_percentual` não é cota do sistema operacional: é quantos núcleos o
  trabalho dividido usa.
- `cache_paginas` e `memoria_max_mb` são lidos e mostrados, mas ainda **não são
  impostos**: o buffer pool do `.ndx` é o trabalho seguinte, e é ele quem vai
  usá-los.

---

'''
p = pathlib.Path('CHANGELOG.md'); s = p.read_text()
v = '## 0.9.0 — 2026-08-28'
assert s.count(v) == 1
p.write_text(s.replace(v, ENTRADA + v, 1))

p = pathlib.Path('README.md'); s = p.read_text()
trocas = [
 ('O motor de armazenamento está completo e testado: **367 testes**, sem nenhuma',
  'O motor de armazenamento está completo e testado: **375 testes**, sem nenhuma'),
 ('| Tabela dinâmica com assistente — cruzamento somado no servidor | pronto |',
  '| Tabela dinâmica com assistente — cruzamento somado no servidor | pronto |\n'
  '| Durabilidade configurável — gravação 20× mais rápida, medida | pronto |\n'
  '| Seção `recursos`: memória, CPU, threads, conexões e usuários | pronto |\n'
  '| `sequencias` — o contador de cada tabela, ajustável pelo admin | pronto |'),
]
for v, n in trocas:
    assert s.count(v) == 1, v[:50]
    s = s.replace(v, n)
p.write_text(s)
print('CHANGELOG 0.10.0 e README')
