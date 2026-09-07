# Cognição: capacidade que funcionou também envelhece — o ligador ARM

**Descoberta:** 07/09/2026 17:20 UTC, ao provar os alvos cruzados antes de
montar o pacote de entrega da rodada do time.

## 1. O que aconteceu

`cargo build --release --target aarch64-unknown-linux-musl` **quebrou no
ligador**, com:

```
/usr/bin/ld: .../self-contained/crt1.o: Relocations in generic ELF (EM: 183)
/usr/bin/ld: .../self-contained/crt1.o: error adding symbols: file in wrong format
collect2: error: ld returned 1 exit status
```

EM 183 é `EM_AARCH64`: o `/usr/bin/ld` da máquina é x86 e recusa o objeto ARM.
O `docs/EMPACOTAMENTO.md` §7.3 dizia, com número, que os dois alvos ARM
**saíam de primeira** — e o binário em disco era de **30/08/2026**. Ou seja: a
capacidade estava documentada como funcionando, e tinha parado de funcionar sem
ninguém tocar no código.

## 2. O que eu concluí primeiro, e estava errado

Que faltava um **gcc cruzado** (`aarch64-linux-gnu-gcc`) e que o conserto era
instalá-lo — o caminho clássico de compilação cruzada. Estava errado por dois
motivos: instalar um gcc cruzado contraria o espírito do «zero dependências
externas» que fez a compilação cruzada da casa funcionar sem toolchain, e não
explicava por que **antes funcionava sem ele**.

## 3. O que a medição disse

O `musl` traz o `crt` autocontido no próprio alvo (`.../self-contained/`), e o
`rust-lld` — que já vem com a ferramenta — liga objeto ARM sem gcc nenhum. O
que mudou no ambiente foi o **ligador padrão**: passou a existir um `cc` que
vira o driver padrão do `musl`, e o `cc` chama o `/usr/bin/ld` x86. Forçar o
ligador devolve o comportamento:

```
RUSTFLAGS="-Clinker=rust-lld -Clinker-flavor=ld.lld -Clink-self-contained=yes"
```

- `aarch64-unknown-linux-musl`: ligou em 52 s; o `phxsqld` **rodou sob
  `qemu-aarch64-static`** e pediu o `config.json` (erro limpo e esperado —
  `SP000018`), prova de que executa, não só de que liga.
- `armv7-unknown-linux-musleabihf`: ligou pelo mesmo caminho.

Uma armadilha medida no meio: `-Clink-self-contained=+linker` **não é estável**
no toolchain pinado (1.94.1 stable) — o rustc recusa com «only `y`/`yes`/`on`…
are stable». O valor estável é `yes` (liga tudo o que é autocontido), e é o que
ficou.

## 4. A regra

**Capacidade registrada com número também envelhece — o mesmo que já valia para
limitação registrada. Alvo de build que «saiu de primeira» num dia se remede
antes de cada pacote, porque o ambiente muda por baixo sem tocar no código.**

## 5. Como está guardado hoje

- `.cargo/config.toml` fixa `linker = "rust-lld"` e os dois `rustflags` para os
  dois alvos ARM `musl`, com o motivo no comentário. Assim o `empacotar.sh`
  (que chama `cargo` cru) liga sem `RUSTFLAGS` no ambiente — provado: `cargo`
  cru gerou o ELF aarch64 sem nada exportado.
- **O buraco:** o `docs/EMPACOTAMENTO.md` §7.3 ainda diz «saíram de primeira,
  sem um gcc cruzado» — verdade em 30/08, meia-verdade hoje: saem sem gcc, mas
  precisam do ligador fixado. A frente que montar o pacote atualiza a §7.3 com
  esta ressalva.
