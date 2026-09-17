# Teto que corta a RESPOSTA não é teto que limita a LEITURA

*Descoberto em 17/09/2026, 02:41 UTC, na revisão SEC adversária da replicação.*

O alcance da pétrea *«instrumentação desligada tem de custar zero — e o portão
que decide isso vem ANTES do trabalho»*, aplicada a **teto** em vez de a
interruptor. A lei estava certa; o que eu aprendi é que ela vale para uma peça
que ninguém tinha chamado de portão.

## 1. O que aconteceu

O `TETO_DO_LOTE_SERVIDO` (`crates/phxsql-server/src/servidor.rs:567`) são 16
MiB, e o comentário dele faz a conta certa: *«500 linhas com um memo de 200 KiB
sao 100 MiB — que, em hexadecimal, viram 200 MiB de texto montados de uma vez
dos dois lados»*. O `docs/REPLICACAO.md:1346-1357` promete o que ele resolve:
*««quanto isso pode crescer» virou pergunta com resposta obrigatória — e a
resposta não podia ser «o que o outro lado mandar»»*.

O teto é real. Ele só chega **tarde** — `servidor.rs:21796-21805`:

```rust
let mut eventos = t.diario_com_imagem(desde, max)?;   // 21796: le TUDO primeiro
// O corte por BYTES, depois do corte por eventos
if let Some(corte) = eventos.iter().position(|(_, imagem)| {
    somados += imagem.len();
    somados > TETO_DO_LOTE_SERVIDO
}) {
    eventos.truncate(corte.max(1));                    // 21804: corta o que ja alocou
}
```

E o `max` que deveria segurar a primeira linha não segura: `let max =
p.inteiro_ou("max", 500).max(0) as u64` (`21770`) devolve 500 só quando o campo
**está ausente**, e `limite == 0` quer dizer **sem limite** no percurso do
diário (`crates/phxsql-store/src/log.rs:692`: `if limite > 0 && …`). Um
`"max":0` no pedido lê o diário inteiro, decifrando cada imagem, **com a trava
global de dados na mão** (`servidor.rs:21771` → `self.dados.write()` em
`servidor.rs:1404`).

## 2. O que eu concluí primeiro, e estava errado

Concluí que o buraco era o `max` **não ter clamp superior** — que o ataque era
`"max": 99999999` e que um `min(TETO_DE_EVENTOS)` resolveria. Estava errado por
dois motivos, e os dois importam:

1. o ataque mais barato é o **oposto**: `"max":0`, que é *menos* que o padrão e
   ainda assim significa «sem limite». Clamp superior não o pega;
2. mesmo com clamp de eventos, o teto continuaria no lugar errado. Evento não
   tem tamanho fixo — é o que o próprio comentário do teto diz —, então
   **qualquer** limite contado em eventos deixa o tamanho em bytes nas mãos de
   quem escreveu a linha mais gorda. O teto tem de morar onde se sabe o tamanho
   **antes de alocar**, que é dentro do `Log::percorrer`.

O erro é o de sempre com número: eu li o teto, vi o número certo, e supus que
ele estava no caminho. O número estava certo; o caminho, não.

## 3. O que a medição disse

Contado no fonte:

| medida | número |
|---|---|
| tetos de leitura no caminho da replicação, lado **réplica** | **1**, e **antes** de alocar: `TETO_DO_REGISTRO` = 128 MiB em `phxsql-core/src/fio.rs:495,523` |
| tetos de leitura no caminho da replicação, lado **source** | **0** — o de 16 MiB corta depois |
| chamadas de `diario_com_imagem` / `diario` com limite **0** (sem limite) | **3**: `servidor.rs:4130` (`absorver_diario_local`), `servidor.rs:21426` (`op_diario`, sem imagem), e `servidor.rs:21796` quando `max` vem 0 |
| teto de memória que alcança este caminho | **0** — `recursos.memoria_max_mb` só é lido pelo `memoria_carregar` (`servidor.rs:21234`, `23391`) |
| bytes de pedido para disparar | **~90** |

E a comparação que fecha: o lado réplica **acertou**. O `replica.rs:54-57`
registra por que o teto **desceu** para o `Canal`: *«Um teto nesta camada
voltaria a deixar o caminho cifrado sem nenhum»* — ou seja, a casa já tinha
aprendido a pôr o teto na camada que lê. Aprendeu de um lado do fio e não do
outro. É o padrão *«conserto entra no caminho que o motivou, e o caminho IRMÃO
fica»*, com o irmão sendo a outra ponta da mesma conversa.

## 4. A regra

**Teto que corta depois do trabalho não é teto: é formatação de resposta. Ponha
o teto onde o tamanho se conhece antes de alocar, e trate `0` como o padrão,
nunca como «sem limite».**

## 5. Como está guardado hoje

**Não está.** É o achado A2 de
`docs/propostas/revisao-sec-replicacao-2026-09-17.md`, severidade alta, com os
dois irmãos nomeados (`op_diario` e `absorver_diario_local`) para o conserto não
entrar só no caminho que o motivou.

O buraco de processo, dito como buraco: **o `docs/REPLICACAO.md` §18 afirma a
garantia que o código não entrega**, e afirmou por dezenas de dias sem ninguém
ver — porque a frase fala do teto e o teto existe. Documento que descreve a
intenção de uma guarda passa no olho de quem a procura pelo nome. A prova de que
uma guarda funciona nunca é o nome dela no documento; é o teste que falha com o
defeito reposto — e neste caso ele tem de medir **quanto foi lido**, não **se
respondeu**, que é exatamente o erro que esta casa já pagou uma vez.
