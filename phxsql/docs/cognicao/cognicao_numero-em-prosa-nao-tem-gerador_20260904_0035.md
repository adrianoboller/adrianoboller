# O «24 de 24 bytes» — número em PROSA não tem gerador, e ninguém percebe

*Descoberto em 04/09/2026, 00:35, na frente S-J-pesquisa (MVCC e trava).*

## 1. O que aconteceu

A §4.2 do `docs/CONCORRENCIA.md` afirma, sobre o cabeçalho do slot do `.reg`:

> «**(1) O cabeçalho do slot está CHEIO.** São 24 bytes (`SLOT_CAB`, no
> `reg.rs`), e os 24 estão usados.»

Dali a frase viajou: entrou no pedido 164 da `PENDENCIAS.md`, entrou no briefing
desta frente, e o orquestrador a repetiu **mais duas vezes** durante a rodada,
como restrição de projeto.

**Medido no `reg.rs`, varrendo quem escreve e quem lê cada faixa do cabeçalho:**

| faixa | escreve | lê |
|---|---|---|
| `status` 0 | sim | sim |
| **`flags` 1** | **ninguém** | **ninguém** |
| **`res` 2..4** | **ninguém** | **ninguém** |
| `crc32` 4..8 | sim | sim |
| `versao` 8..16 | sim | sim |
| **`tempero` 16..24** | **só se `material.cifrado()`** | **só se `material.cifrado()`** |

**São 3 bytes livres sempre, e 11 num `.reg` v4 (em claro).** «Os 24 estão
usados» descreve o **desenho** do cabeçalho, não o **estado** dele.

## 2. O que eu concluí primeiro, e estava errado

**Concluí que a diferença era irrelevante**, porque «3 bytes não dão um ponteiro
de undo de qualquer jeito, então a conclusão do documento continua de pé».

Isso estava errado, e errado do jeito que mais custa: **a frase certa muda a
resposta.** «Está cheio» é uma parede — encerra a conversa e manda direto para o
`.reg` v6. «Tem 3 e o InnoDB gasta 13» é uma **conta** — e conta se discute. Ao
discuti-la, com o fonte do `trx0undo.ic` na mão (onde o ponteiro de 7 bytes é
endereço **estruturado**, não deslocamento cru), apareceu o desenho em que **3
bytes bastam**: um índice de 24 bits num diretório por tabela.

**A parede me faria propor mudança de formato em disco que talvez não seja
necessária.** Num item cuja lei diz «mudança de formato entra cedo, porque
depois vira migração», propor a migração errada é o erro caro.

## 3. O que a medição disse

| | número |
|---|---:|
| bytes que a frase dizia usados | 24 de 24 |
| bytes de fato usados, `.reg` v5 (cifrado) | **21 de 24** |
| bytes de fato usados, `.reg` v4 (em claro) | **13 de 24** |
| o que o InnoDB gasta por linha (`DATA_TRX_ID_LEN` + `DATA_ROLL_PTR_LEN`) | **13** |
| documentos que repetiram o número errado | 2 (`CONCORRENCIA.md`, `PENDENCIAS.md`) |
| vezes que ele foi repetido nesta rodada como restrição | 3 |

E o achado de tabela, que é o que fecha: **o CRC do slot cobre
`slot[SLOT_CAB..]` — só o payload.** Escrever nesses 3 bytes **não** invalida o
CRC, e por isso eles são utilizáveis; e **não** os protege, e por isso o registro
que eles apontam tem de carregar o próprio `rowid` para se conferir no destino.

## 4. A regra

**Todo número visível sai de um gerador — e «visível» inclui a PROSA de
`docs/`.** O alcance da pétrea, hoje, é o dossiê: cinco geradores escrevem
título, selo, painel, rodapé, idiomas, bancada. **Número escrito no meio de um
parágrafo de `docs/` não tem gerador nenhum, não tem catraca nenhuma, e envelhece
exatamente como o selo da capa envelheceu por quatro lançamentos.**

E o agravante que este caso acrescenta: **número em prosa VIAJA.** O do dossiê
envelhece parado num lugar; este foi copiado para um segundo documento, para um
briefing e para três mensagens — e cada cópia é um lugar onde a correção não
chega.

## 5. Como está guardado hoje

* A medição está no `docs/PESQUISA-MVCC-E-FORMATO.md` §1.2, com a tabela de quem
  escreve e quem lê cada faixa, e a linha do `reg.rs` ao lado de cada uma.
* **O buraco, e ele é grande:** a §4.2 do `docs/CONCORRENCIA.md` e o pedido 164
  da `docs/PENDENCIAS.md` **continuam com o número errado**. Não os corrigi
  porque não são território desta frente, e a correção está **pedida** no
  entregável, nomeando arquivo e seção.
* **A guarda que NÃO existe:** nada confere afirmação numérica de `docs/` contra
  o fonte. O conferidor de textos fora da fábrica (`conferidor.rs`) varre `ui/`
  atrás de texto cravado; não há irmão dele para `docs/`. Um conferidor genérico
  aqui provavelmente reprovaria mais número legítimo do que defeito — foi o que
  aconteceu com o casador de erro cru, **8 interpolações e só 2 defeitos** —,
  então o que este arquivo registra é o **alcance**, e não uma proposta de
  catraca.
