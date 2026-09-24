# O SIGSEGV do ARM era o punho morto, e não o pânico

*24/09/2026, 04h10 — papel B com o F. Detalhe e números em
`docs/EMBUTIDO.md` §9.5.*

## 1. O que aconteceu

A bateria da `0f7aab6` derrubou a perna ARM64 do `bancada/embutido/provar.sh`
(staticlib `aarch64-unknown-linux-musl` ligada à mão com `ld.lld`, rodando sob
`qemu-aarch64-static`). O log terminava assim: backtrace do pânico
`capacity overflow` vindo de `phx_inserir` e, na linha seguinte,
`qemu: uncaught target signal 11 (Segmentation fault)`.

## 2. O que eu concluí primeiro, e estava errado

Que o `catch_unwind` da fronteira falhava no ARM estático — a garantia central
da camada quebrada —, porque o mesmo `provar.sh` já tinha registrado um
«failed to initiate panic» sem `--eh-frame-hdr`, e o log punha o pânico como
última coisa antes da queda. As quatro hipóteses da encomenda (emulador,
receita de ligação, nosso caminho do pânico, toolchain) partiam todas dessa
leitura, e uma quinta minha (`RUST_BACKTRACE`) também.

## 3. O que a medição disse

- Rust puro estático musl aarch64 sob o mesmo qemu: **captura**. Biblioteca
  mínima + `main` em C pela mesma linha do `ld.lld`: **captura** (`r=-1`).
- A saída do `prova.c` num terminal: **«ok contagem absurda vira erro»** — o
  pânico tinha sido capturado; a seção 6 passou inteira.
- `lldb` no gdbstub do qemu: SIGSEGV em `punho::com`, `ldr x8, [x19, #0xd78]`
  (a etiqueta), vindo de `prova.c:266` — a seção 7, `phx_tabela_registros`
  sobre o punho recém-fechado.
- `strace` no mesmo programa ligado ao musl em **x86-64 nativo**: `munmap` de
  4.096 bytes no `free` do punho (3.464 bytes) e `SEGV_MAPERR` na leitura
  seguinte. Código 139, sem emulador nenhum. O teste de unidade
  `punho_liberado_nao_volta_a_ser_usado` derruba o binário em
  `--target x86_64-unknown-linux-musl`; no glibc ele passa por acaso.
- Por que o log mentiu: stdout num cano é bufferizado, stderr não. A queda
  levou as linhas «ok» da seção 6; o pânico, impresso no stderr pelo gancho
  padrão, ficou por último.

## 4. A regra

**Num log de processo que caiu, a última linha visível é a última que não
tinha buffer — não a última que aconteceu.** Antes de diagnosticar pela ordem
do log, rode com a saída num terminal ou descarregue a cada passo. E: **teste
que confere o comportamento sobre memória liberada prova o alocador, não o
código** — a conferência tem de decidir sem ler o endereço.

## 5. Como está guardado hoje

- `crates/phxsql-ffi/src/punho.rs`: registro de punhos vivos (64 gavetas),
  consultado antes de tocar a memória; `com`, `conferir` e `liberar` passam
  pela mesma decisão.
- `copia_de_punho_vivo_nao_e_punho` (`testes.rs`): cai com a conferência
  antiga em qualquer alocador — glibc, musl e ARM64 sob qemu.
- `crates/phxsql-ffi/c/prova.c`: `fflush` a cada passo.
- **Buraco que fica:** a portaria da casa roda `cargo test` só no glibc, onde a
  leitura de memória morta não dói. Nenhuma corrida periódica roda a suíte do
  `phxsql-ffi` em `--target x86_64-unknown-linux-musl` (nativo, segundos) —
  e é ela que teria achado isto antes da bateria. E o catálogo de guardas
  (`bancada/guardas/catalogo.py`) ainda não tem a entrada deste defeito.
