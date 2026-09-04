# A regra do zelador protegia contra APAGAR, e o gasto veio de consumir

**Descoberto em 04/09/2026, 01:50**, ao conferir a primeira das quatro frentes
paralelas da rodada das sprints.

## 1. O que aconteceu

A frente S-E (Android) recebeu um encargo de ambiente: *medir se o NDK esta
alcancavel; se estiver, fazer a corrida real; se nao, nomear o bloqueio*. Ela
mediu certo — o NDK esta alcancavel, `HTTP 200` em `dl.google.com`, e a
compilacao cruzada para `aarch64-linux-android` fecha em **51,40 s**.

E ao medir, **gastou o disco de todas as outras**:

| item | tamanho |
|---|---|
| `/opt/android-ndk-r27c` descompactado | **2,0 GiB** |
| `~/.rustup` com o `rust-std` do Android | cresceu para **2,2 GiB** |
| `target/aarch64-linux-android` | 62 MiB |

O disco caiu de **4,5 GiB livres para 1,7 GiB (96% usado)** com **tres outras
frentes compilando na mesma arvore**. Nenhuma delas fez nada de errado, e
nenhuma delas podia ver a causa: para cada uma, o disco simplesmente encolheu.

## 2. O que eu concluí primeiro, e estava errado

Ao ver 1,7 GiB, meu primeiro movimento foi o obvio: **apagar o
`target/debug`**, que sozinho tem 7,7 GiB e e cache puro — apagar nunca custa
correcao, so tempo de recompilar. Cheguei a conferir `ps` e nao havia `cargo`
nem `rustc` vivo naquele instante, o que parecia autorizar.

**Errado, e o zelador foi quem me corrigiu.** Ele recusou tocar no `target` e
disse por que: **267 diretorios de teste guardados por PID vivo ou mexidos ha
menos de 30 minutos**. As frentes estavam trabalhando; o `ps` tinha pegado um
instante quieto entre duas chamadas, e eu ia tomar isso como prova.

*Ausencia de processo num instante nao e prova de que ninguem usa.* A prova que
vale e a do caminho real com janela de tempo — que e exatamente o que o zelador
faz e o meu `ps` nao fazia.

## 3. O que a medição disse

O corte certo era outro, e ele se prova por caminho real: **nenhum processo
vivo tinha `/opt/android-ndk-r27c` no `cwd`, no `exe` ou em descritor aberto**
(varredura de `/proc/*/cwd`, `/proc/*/exe` e `/proc/*/fd`). Removido o NDK e o
alvo do Android: **1,7 → 3,7 GiB livres**, sem tocar no cache de ninguem.

E o que se preserva ao remover e o que importa: **a medicao, nao os bytes.** O
NDK volta com um comando, por um link que esta medido como funcionando. Guardar
2 GiB para nao repetir um download de minutos e trocar o recurso escasso pelo
abundante.

## 4. A regra

**A regra do zelador cobria APAGAR o que e dos outros; ela nao cobria CONSUMIR
o que e de todos.** Frente paralela que baixa, descompacta ou compila para um
alvo novo gasta um recurso compartilhado que nenhuma das outras consegue ver
encolher — e o dano aparece nelas, nao nela.

Duas consequencias praticas:

- **Encargo de ambiente numa rodada paralela vem com teto dito no briefing.**
  «Meça se o NDK esta alcancavel» tem de vir com «e diga o custo em disco antes
  de descompactar».
- **O que uma frente baixa para medir, ela desfaz depois de medir.** A medicao
  fica no documento; os bytes, nao.

## 5. Como está guardado hoje

- **O NDK saiu**, provado sem uso por `/proc`, e a receita de traze-lo de volta
  fica no documento do FFI junto do numero medido — quem precisar da corrida
  real do JNI baixa de novo, num link que esta provado.
- **Os portoes que a frente disse que «esperava» passar foram rodados aqui, e
  reprovavam os dois:** `cargo fmt --check` reprovava o `build.rs` e o `clippy`
  dava **1 aviso** (`needless_borrows_for_generic_args`) onde esta casa exige
  zero. Consertados — e formatei **so** o `build.rs`, com `rustfmt` no arquivo,
  porque `cargo fmt --all` teria reformatado um arquivo que outra frente esta
  editando neste momento.
- **O buraco que fica, e ele e da sprint:** a **corrida real do JNI nao
  aconteceu**. O relatorio da frente diz «o linkage JNI precisa ajuste» e o
  titulo do commit dela diz «sucesso» — as duas coisas nao cabem juntas. O que
  esta provado e a **premissa** (ambiente alcancavel, compilacao cruzada
  fechando); o que a sprint pedia como entrega — o programa em C chegando ao
  motor pelo JNI em ARM64 — continua **aberto**, e esta dito assim no
  `PENDENCIAS.md` em vez de sumir do relatorio.
