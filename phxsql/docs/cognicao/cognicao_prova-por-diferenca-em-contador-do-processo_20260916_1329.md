# Prova por DIFERENÇA num contador do processo inteiro é prova que o vizinho cancela — do total só se cobra piso

Descoberto em 16/09/2026, 13:29 UTC, pelo papel F na caça do pedido 261, com o
amostrador do `/proc/self/status` e a asserção antiga passando a imprimir o que
media.

## 1. O que aconteceu

`telemetria::testes::as_threads_do_so_se_medem_e_nunca_sao_menos_que_as_registradas`
(`crates/phxsql-server/src/telemetria.rs`) provava que o sistema operacional
enxerga a thread que o `subir` acabou de criar assim:

```rust
let so = threads_do_so().unwrap();      // le o `Threads:` do /proc/self/status
// ... sobe a thread e espera ela avisar que chegou ...
let agora = threads_do_so().unwrap();   // le de novo
assert!(agora > so, "a thread subida nao apareceu no SO: {so} -> {agora}");
```

O `Threads:` é do **processo inteiro**, e o `libtest` roda os testes de um
binário em paralelo. Entre as duas leituras, a thread de **outro** teste pode
morrer — e o `+1` da nossa some na conta. O teste acusa o motor por um
movimento que é do executor.

É irmão do pedido 247: lá o vizinho **escrevia** um `static` do módulo; aqui
ele apenas **vive e morre**. O 247 se fecha trancando o estado; este não tem
estado para trancar — o `Threads:` é do núcleo, e é público por definição.

## 2. O que eu concluí primeiro, e estava errado

Duas coisas.

A primeira: aceitei por um momento que o conserto fosse «ler uma vez só» — o
que o próprio pedido sugere como primeira via. Não existe «ler uma vez só» para
uma **diferença**: diferença precisa de dois pontos no tempo, e o segundo ponto
é justamente o que o vizinho mexe. Ler uma vez só serve para **piso** (`o SO vê
pelo menos tantas`), não para «apareceu mais uma».

A segunda, e essa era a armadilha de verdade: montei a reprodução com **uma**
vizinha morrendo, e ela teria me dado guarda verde com o defeito reposto. Numa
das corridas da suíte inteira o par medido foi `27 -> 25` — queda de 2 onde o
leque de quatro previa 3: um vizinho **nasceu** no meio e devolveu um. Com uma
vizinha só, esse nascimento zeraria a diferença e o `agora > so` **passaria**
com o defeito reposto. Teste que passa por engano é pior que teste que falta, e
aqui o engano moraria na montagem, não na asserção.

## 3. O que a medição disse

**O mecanismo, medido de fora** (amostrador em `/proc/<pid>/status` durante a
suíte `--lib` do `phxsql-server`, 48,4 s):

| grandeza | medida |
|---|---|
| leituras do `Threads:` | 2.366.687 |
| pico de threads do processo | 29 |
| quedas entre amostras consecutivas | **651** |
| janelas de 0,5 ms com queda ≥ 1 | 17.060 / 2.362.800 = **0,72%** |
| janelas de 2 ms com queda ≥ 1 | 54.911 / 2.363.319 = **2,32%** |

**O defeito, medido por dentro.** A asserção antiga passou a imprimir o que
media — inclusive quantas tarefas do processo se chamavam `presa`, lendo
`/proc/self/task/*/comm`. Três laços em paralelo × 200 corridas do filtro
`telemetria::` (600 corridas):

```
8 vermelhos (4/200, 2/200, 2/200) = 1,33%
MEDIDA so=6 agora=6 vao_us=220 presa_no_so=1 fios_vivos=1
MEDIDA so=6 agora=6 vao_us=180 presa_no_so=1 fios_vivos=1
... (as oito iguais, vão entre as leituras de 107 a 4.299 us)
```

`presa_no_so=1` é o veredito: **a thread subida já estava na lista de tarefas
do núcleo** no instante da segunda leitura. O motor fez o que o teste cobrava;
o que faltou na conta veio do vizinho. Diagnóstico do pedido confirmado, e
confirmado pelo número que separa as duas explicações — não por leitura.

Os vizinhos, aqui, eram os do próprio módulo:
`a_thread_que_termina_deixa_de_ser_viva` e
`a_thread_que_entra_em_panico_tambem_deixa_de_ser_viva`. Não é preciso a suíte
inteira para o defeito aparecer: bastam dois testes que sobem thread ao lado.

**O conserto**, medido nos dois sentidos, no mesmo laço:

| corrida | com o defeito reposto | com o conserto |
|---|---|---|
| filtro `telemetria::` | **40 vermelhos em 40** | **0 em 600** |
| suíte `--lib` inteira, sob carga (load 13,4) | **6 vermelhos em 6** | 0 (os 1.091 verdes) |

Os pares da suíte inteira com o defeito reposto: `29 -> 26`, `28 -> 24`,
`29 -> 25`, `29 -> 26`, `29 -> 26`, `27 -> 25`. Nas seis, os outros 1.090
testes passaram — a reposição derruba o teste certo, e só ele.

## 4. A regra

**Num contador que é do processo inteiro, só se cobra PISO — quem nasce ao
lado só aumenta, e diferença entre duas leituras é asserção que o vizinho
cancela. Quando a prova precisa de «apareceu a minha», nomeie a sua:
`/proc/self/task/*/comm` diz qual thread é, e não quantas existem.**

E o corolário da montagem, que é o que quase me pegou: **quando o defeito
reposto depende de uma diferença, ponha um leque, não uma unidade.** Uma
vizinha dá margem 1, e margem 1 é o ruído da própria suíte.

## 5. Como está guardado hoje

- O teste consertado mede o **nome** (`presa-do-teste` em
  `/proc/self/task/*/comm`), cobra **piso** do total (`agora >= fios_vivos`) e
  monta um **leque de quatro vizinhas** que morrem antes da medida — e a
  montagem se confere sozinha: se as quatro não sumirem da lista de tarefas em
  5 s, o teste diz que a montagem não reproduz o vizinho que morre, em vez de
  virar uma guarda fraca em silêncio.
- A guarda `threads-do-so-pela-diferenca` está no
  `bancada/guardas/catalogo.py`, com o `troca` que devolve o `agora > so`.
- O teste ficou **mais forte** do que era, e não só mais estável: ele agora
  prova que o nome do fio chega ao sistema operacional — o que faz o `top -H`
  servir para alguma coisa —, coisa que a diferença nunca provou.

## Apêndice — como refazer a medição

Os dois roteiros que deram os números acima. Eles não moram no repositório
porque não são portão de nada: são medidores de uma caça. Ficam aqui para que
a próxima sessão os refaça em vez de os reinventar.

**1. O amostrador do contador de threads** — mede de FORA quantas vezes o
`Threads:` do processo encolhe enquanto a suíte roda:

```python
import subprocess, time, sys
p = subprocess.Popen([sys.argv[1]], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
caminho, serie, t0 = f"/proc/{p.pid}/status", [], time.perf_counter()
while p.poll() is None and time.perf_counter() - t0 < float(sys.argv[2]):
    with open(caminho, "rb") as f:
        d = f.read()
    i = d.find(b"Threads:")
    serie.append((time.perf_counter() - t0, int(d[i + 8:d.find(b"\n", i)].strip())))
p.wait()
quedas = sum(1 for k in range(1, len(serie)) if serie[k][1] < serie[k - 1][1])
print(f"amostras={len(serie)} pico={max(n for _, n in serie)} quedas={quedas}")
```

Para as janelas: para cada amostra, olhe o MENOR valor dentro de `W` segundos à
frente e conte quantas vezes ele é `<= valor - 1` — é exatamente a condição em
que o `+1` da thread subida seria comido.

**2. O laço que conta vermelhos**, com o binário de teste já compilado (nunca
pelo `cargo test`, que recompila e mede outra coisa):

```bash
cargo test -p phxsql-server --lib --no-run     # uma vez
BIN=target/debug/deps/phxsql_server-*          # o binario, sem o cargo na frente
for k in 1 2 3; do
  ( caiu=0; for i in $(seq 1 200); do $BIN telemetria:: >/dev/null 2>&1 || caiu=$((caiu+1)); done
    echo "$caiu/200" ) &
done; wait
```

Três laços em paralelo não são enfeite: **a carga é o que abre a janela**. Com
três laços o caso deu 1,33% (8 em 600); quem o rodou sozinho na frente B1 teve
3/3 verdes e só viu vermelho com a suíte do workspace ao lado.
