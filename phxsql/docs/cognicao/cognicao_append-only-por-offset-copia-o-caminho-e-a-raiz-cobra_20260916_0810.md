# Append-only endereçado por offset copia o caminho — e a raiz cobra proporcional ao fanout

Descoberto em 16/09/2026, 08:10 UTC, ao fechar o CRUD da colmeia
(`bancada/colmeia/resultados-crud.json`, `docs/propostas/colmeia.md` §1.1).

## 1. O que aconteceu

O protótipo PSHV da colmeia ganhou a escrita do Padrão e do SQLite(R) em todo
regime com `fsync` e a N = 1.000 e 10.000 sem `fsync` — e **perdeu para o
Padrão a N = 100.000 sem `fsync`**: inserir 9,962 µs contra 4,390; atualizar
11,678 contra 8,146, fora do ruído. Os bytes anexados por operação subiram
500 B → 922 B → 5.144 B com N.

## 2. O que eu concluí primeiro, e estava errado

«Append é a escrita mais barata que existe; a colmeia vai ganhar escrita em
todo N, e a única pergunta é por quanto.» Estava errado porque esqueci o que a
pétrea do append-only força quando as células são endereçadas por **offset**:
trocar uma entrada de um NÓ exige um NÓ novo, o pai aponta para o velho, então
o pai nasce de novo — até a raiz. Cada escrita anexa a cadeia inteira, e o
maior elo é a raiz, cuja lista de subchaves cresce com o número de grupos
(391 a N = 100.000). O «append barato» era verdade só enquanto a raiz cabia
em poucas centenas de bytes.

## 3. O que a medição disse

- `bytes_anexados_por_op` (inserir): 500 B (N = 1.000), 922 B (10.000),
  5.144 B (100.000) — o crescimento é o da raiz, não das folhas.
- Sem `fsync`: colmeia 2,058 / 2,501 / 9,962 µs contra Padrão 3,729 / 3,911 /
  4,390 (inserir, N = 1.000 / 10.000 / 100.000). O cruzamento fica entre
  10.000 e 100.000.
- Com `fsync` por operação o custo some atrás do disco: 391 µs contra 997 µs a
  100.000 — dois `fsync` contra oito, medidos com `strace`.
- O que a cópia de caminho compra, e é real: atomicidade por construção, sem
  diário — a raiz nova só vale quando o bloco base aponta para ela.

## 4. A regra

**Desenho append-only por offset tem de nascer com o fanout limitado — meça os
bytes anexados por operação no maior N antes de prometer escrita.** A razão
«colmeia × Padrão» na escrita não é uma constante: é uma função do tamanho da
raiz.

## 5. Como está guardado hoje

No JSON (`bytes_anexados_por_op`, por operação e N), na §1.1 da proposta, na
linha C do `docs/STATUS-TIPOS.md` e na lista «a premissa que falta medir»:
fanout limitado (lista de subchaves em célula própria por bin, ou mais um
nível) é a premissa nova do formato PSHV. **Onde o buraco ficou:** o protótipo
não implementa o fanout limitado — mede o preço de não tê-lo; a variante com
ponteiro corrigido no lugar (que quebraria a atomicidade por construção) não
foi medida, de propósito, e quem quiser esse número precisa decidir primeiro se
abre mão da atomicidade sem diário.
