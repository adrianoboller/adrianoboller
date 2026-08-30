# Correct the overclaim in all three places
# 29/08 02:15

import pathlib

# --- DESEMPENHO: a frase sobre o mutex estava errada
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
alvo = '''A regra que fica: **instrumentação desligada tem de custar zero, e o portão que
decide isso vem antes do trabalho, não depois.** Um mutex por pedido é pior do
que parece: além do custo, ele *serializa* — todo mundo esperando na mesma
fila para descobrir que não havia nada a registrar.'''
novo = '''A regra que fica: **instrumentação desligada tem de custar zero, e o portão que
decide isso vem antes do trabalho, não depois.**

### Qual das duas coisas custava

```bash
cargo run --release --example quem-custava
```

| | custo |
|---|---:|
| um `lock`/`unlock` sem disputa | **13,2 ns** |
| `Json::analisar` de 1 linha (140 B) | 1,44 µs |
| `Json::analisar` de 5.000 linhas (304 KB) | **3.456 µs** |

Por lote de 5.000 linhas, o ponto de captura pagava **6.912 µs de parse contra
0,03 µs de lock** — o parse custava 262.000× o mutex.

**Não era o mutex.** Numa primeira redação deste documento eu escrevi que ele
era «o pior pedaço, porque serializa». A segunda parte é verdade sobre mutex em
geral e **não era verdade aqui**, por dois motivos: sem disputa ele custa
nanossegundos, e neste servidor toda operação de dado já se serializa na trava
global — que é tomada *depois* e segurada por muito mais tempo. O mutex do
profiler nunca foi o gargalo de concorrência.

O que custava era analisar meio megabyte de JSON **duas vezes para jogar fora**.
Foi por isso que a carga em lote melhorou 7% e o caminho linha a linha quase não
se moveu: lá o corpo tem 140 bytes.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)

# --- CHANGELOG
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''  O portão passou a ser um `AtomicBool` lido antes de qualquer trabalho:
  **40.600 → 43.450 linhas/s (1,07×)** na carga em lote, dois pares de corridas.
  O mutex por pedido era o pior pedaço — além do custo, ele *serializava*.'''
novo = '''  O portão passou a ser um `AtomicBool` lido antes de qualquer trabalho:
  **40.600 → 43.450 linhas/s (1,07×)** na carga em lote, dois pares de corridas.

  Qual das duas coisas custava, medido em `--example quem-custava`: um
  `lock`/`unlock` sem disputa custa **13,2 ns**, e analisar o corpo de um lote
  de 5.000 linhas custa **3.456 µs**. Por lote eram 6.912 µs de parse contra
  0,03 de lock — **262.000×**. Não era o mutex; era analisar meio megabyte de
  JSON duas vezes para jogar fora. É também por isso que o caminho linha a
  linha quase não se moveu: lá o corpo tem 140 bytes.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)

# --- CLAUDE.md
p = pathlib.Path("/home/user/adrianoboller/CLAUDE.md")
s = p.read_text()
alvo = '''mutex, e só então perguntava se estava ligado. Num lote de cinco mil linhas era
analisar meio megabyte de JSON duas vezes para jogar fora. O mutex era o pior
pedaço: além do custo, ele **serializa** — todo mundo na mesma fila para
descobrir que não havia o que registrar. Quando entrar um observador novo,
procure o que ele faz antes de olhar o próprio interruptor.'''
novo = '''mutex, e só então perguntava se estava ligado. Num lote de cinco mil linhas era
analisar meio megabyte de JSON duas vezes para jogar fora. Quando entrar um
observador novo, procure o que ele faz antes de olhar o próprio interruptor.

E o corolário sobre a **explicação** disso, que eu errei primeiro: escrevi que
«o mutex era o pior pedaço, porque serializa». Medido, o `lock` sem disputa
custa **13,2 ns** e o parse do lote custa **3.456 µs** — 262.000× mais. O mutex
nunca foi o gargalo, e neste servidor nem poderia ser: a trava global de dados
já serializa tudo, e é tomada depois e segurada por mais tempo. **Diagnóstico
plausível não é diagnóstico medido** — e o errado sobrevive melhor quando o
conserto funcionou por outro motivo.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
