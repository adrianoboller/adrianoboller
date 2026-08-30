# Document, commit and push
# 29/08 02:13

import pathlib
p = pathlib.Path("CHANGELOG.md")
p_s = p.read_text()
alvo = '''- **`--example custo-do-log`**'''
novo = '''- **O Profiler desligado custava 7% da carga pela rede.** O ponto de captura
  fazia o trabalho **antes** de conferir se havia o que capturar: dois
  `Json::analisar` do corpo inteiro, três `String` e um mutex, para no fim
  `chegou` olhar `ligado` e devolver `None`. Num `inserir_lote` de 5.000 linhas
  isso é analisar meio megabyte de JSON duas vezes, para nada.

  O portão passou a ser um `AtomicBool` lido antes de qualquer trabalho:
  **40.600 → 43.450 linhas/s (1,07×)** na carga em lote, dois pares de corridas.
  O mutex por pedido era o pior pedaço — além do custo, ele *serializava*.

  Cinco testes travam o que pode dar errado: o espelho atômico divergir do
  estado real. Preso em `true`, o servidor pagaria o parse para sempre; preso em
  `false`, o Profiler não veria nada estando ligado. Inclusive o caso do
  `profiler_ligar` que **falha** — ele não pode levantar o espelho.

- **`--example custo-do-log`**'''
assert p_s.count(alvo) == 1
p.write_text(p_s.replace(alvo, novo, 1))
