# Cache do uv montado só leitura não instala nada — nem o que já está nele

**Estado:** PENDENTE

**Evidência (a validar pelo integrador):** `crates/phxclaw-agent/tests/python.rs`. Com o
`UV_CACHE_DIR` apontando direto para o cache do hospedeiro montado só leitura, 3 dos 4 testes
falham com `failed to open file .../sdists-v9/.git: Read-only file system`; com a pasta de
atalhos (`ATALHOS_DO_CACHE` em `src/python.rs`), os 4 passam. Medido em 01/10/2026, antes de
commit.

## O que aconteceu

O `python_project` instala dependência sem rede, então a única fonte é o cache do `uv` do
hospedeiro. Montá-lo gravável no bwrap deixaria o código do projeto envenenar o cache que o
hospedeiro usa depois; montei só leitura com `UV_OFFLINE=1`.

## O que eu concluí primeiro, e estava errado

Que instalação offline de pacote que já está no cache só LÊ o cache. O `uv` 0.8.17 abre para
escrita as marcas de cada balde (`sdists-v9/.git`) e regrava a ficha do interpretador
(`interpreter-v4/...`) até numa instalação que não baixa nada — inclusive no `uv venv`.
E o atalho para cada balde não basta: o `archive-v0` atrás de atalho falha na cópia
(«source path is neither a regular file nor a symlink to a regular file»).

## O que a medição disse

- bwrap 0.9.0 desta máquina não tem `--overlay-src`/`--tmp-overlay` (seria a saída natural).
- Funciona: `archive-*` montados direto (só leitura) dentro do cache gravável do tmpfs, e os
  outros baldes como pasta real com um atalho por entrada; `interpreter-*` fora. `requests` e
  quatro dependências instalaram em 8 ms, e a dependência ausente volta «not found in the
  cache … network was disabled», que a ferramenta traduz nomeando o pacote.

## A regra

Ferramenta que «só lê» um cache tem de provar isso montando-o só leitura: a gravação
escondida aparece na primeira chamada, não lendo a documentação.

## Como está guardado hoje

`dependencia_do_cache_instala_e_a_de_fora_recusa_dizendo` e
`mypy_pytest_e_ruff_devolvem_diagnostico_estruturado` em `tests/python.rs`.
