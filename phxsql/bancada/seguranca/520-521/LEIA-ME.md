# Sondas dos pedidos 520 e 521

O login que dizia pelo relógio quem existe (520) e a senha longa que multiplicava
o PBKDF2 (521). O que mediram e o que se consertou: `docs/SEGURANCA.md` §26.

```bash
cargo build -p phxsql-server --bin phxsqld         # debug; o que vale é a razão
B=target/debug/phxsqld
python3 bancada/seguranca/520-521/sonda-520.py     $B depois   # tudo, ~1 min em debug
python3 bancada/seguranca/520-521/sonda-prova.py   $B depois   # prova errada, intercalada
python3 bancada/seguranca/520-521/sonda-desafio.py $B depois   # o desafio, intercalado
python3 bancada/seguranca/520-521/sonda-sal.py     $B depois   # o sal falso, sem relógio
```

Com o rótulo `antes` (um binário sem o conserto), a `sonda-520.py` pula os casos
que levariam horas: a senha de 64 KiB a 1 MiB com 210.000 iterações que resumem a
chave inteira em cada volta.

Duas lições de medida, pagas aqui:

- **Intercalado, na mesma conexão.** Medido um caminho depois do outro, a prova
  errada dava 0,161 / 0,134 / 0,167 ms e parecia ruído; intercalada, a diferença
  de 24 µs apareceu igual nas duas rodadas.
- **Login do mesmo tamanho.** `nao_existe` contra `ana` deixava 2 µs depois do
  conserto, e eram o tamanho do login; `zzz` contra `ana` dá 0,0.

Porta `PHX_520_PORTA` (padrão 47731, e a web em `+1`). Temporários em
`/tmp/phx-520-<pid>`, apagados no fim; o servidor cai pelo PID guardado, nunca
por nome. As medidas do dia estão em `medida-antes.txt`, `medida-depois.txt` e
`medida-intercalada.txt`.
