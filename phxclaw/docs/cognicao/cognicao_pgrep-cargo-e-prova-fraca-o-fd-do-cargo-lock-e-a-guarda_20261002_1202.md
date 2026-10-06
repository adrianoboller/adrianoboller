# `pgrep cargo` é prova fraca: a guarda antes de podar o `target` é o fd do `.cargo-lock`

Data da descoberta: 02/10/2026 12:02 UTC (papel D, zelador, rodada «equipe inteira»).
Estado: **FRUTÍFERO** — evidência: corrida do zelador em 02/10 12:02–12:10, inventário em
`scratchpad/rodada-equipe/pareceres.md`; o `cargo check -p phxclaw-agent -p phxclaw` (pid 8418)
estava vivo, `pgrep -a cargo` não o listou na primeira chamada, e ele apareceu pela varredura de
`/proc/*/fd` apontando para `target/debug/.cargo-lock`.

## 1. O que se queria
Podar o `target` compartilhado (16,7 GB) sem derrubar a compilação de nenhuma das oito frentes
paralelas. A pétrea do zelador: nada se apaga sem provar que nenhum processo vivo usa.

## 2. O que se descobriu
- A prova pelo nome do processo falha: um `cargo` vivo não apareceu no `pgrep`, e apareceu pelo
  descritor aberto em `target/debug/.cargo-lock`. A guarda certa é «existe fd aberto em
  `.cargo-lock`/`.cargo-build-lock`», varrendo `/proc/*/fd`, `/proc/*/cwd` e `/proc/*/maps`.
- A premissa da ordem («executáveis de teste >20 MB») não se sustentou: eram 5 executáveis,
  43 MB. O disco está nos rlib/rmeta duplicados (12,9 GB; 9 cópias de `libphxclaw_agent-*.rlib`
  ≈ 1,1 GB), que só se podam em rodada sem cargo vivo.
- Medido: livre 1.077 MB → 2.548 MB apagando examples antigos (951 MB), `/tmp/phx-*` (119),
  caches uv/pip (400) e 153 crates órfãs de `registry/src` que têm o `.crate` no cache (159).

## 3. O que eu concluí primeiro, e estava errado
Que «`pgrep -a cargo` vazio» provava ausência de compilação, e que o espaço estava nos
executáveis de teste — porque foi assim nas rodadas anteriores (13 GB de executáveis). O
ambiente mudou (`incremental=false`, `debug=0`) e a conta velha envelheceu calada.

## 4. Como se aplica
Antes de podar `target/`: varrer `/proc/*/fd` por `.cargo-lock`; zero fd = pode podar hash
antigo (>30 min, sem fd/maps). Nunca por nome de processo. Nunca rlib/rmeta com cargo vivo.

## 5. Número
+1.471 MB livres medidos no `df`; 0 processos tocados; 0 arquivos versionados tocados.
