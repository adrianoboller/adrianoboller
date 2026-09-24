# `todas.py` — todas as catracas em Python, num comando só

```bash
python3 bancada/catracas/todas.py            # roda e da o veredito
python3 bancada/catracas/todas.py --lista    # so lista, nao roda nada
python3 bancada/catracas/todas.py --com-rust # tambem RODA os testes das TETO_* (caro)
```

**Por que existe** (pedido 476): as réguas em Python com `--catraca`
(`bancada/concorrencia/mapa-da-trava.py`, `mapa-das-threads.py`,
`bancada/guardas/debug-com-segredo.py`, `pkill-sem-pid.py`, `trecho-vivo.py`)
só eram chamadas de dentro do item 0 da `bancada/bateria/prova-bateria.py`, e
algumas também do `comunicacao.sh` — cada arquivo com a própria lista escrita
à mão. Um commit passava por `trecho-vivo.py --catraca`, pela suíte e pelo
clippy sem reprovar nada, e subia outra catraca sem ninguém ver (cognição de
24/09/2026,
`docs/cognicao/cognicao_catraca-que-so-roda-dentro-da-bateria-nao-segura-a-integracao_20260924_0455.md`).

**A lista nunca se digita.** Ele varre todo `.py` do repositório atrás de uma
linha que junte `sys.argv` (ou `add_argument`) com `--catraca` — é assim que
um script DECLARA o próprio modo, e não apenas o menciona ao chamar outro. Uma
catraca nova entra na próxima corrida sem editar nada aqui.

**As `TETO_*` do Rust** já são conferidas por `cargo test`; este comando as
LISTA (varridas de `crates/**/*.rs`) e diz qual teste cita cada uma — mas não
roda `cargo test` por padrão, porque é caro (medido: ~19 s frio só para
compilar os quatro crates onde mora a maioria). Use `--com-rust` para rodar de
verdade.

Detalhe completo, custo medido e as três provas reais (defeito reposto,
catraca nova achada sozinha, medidor mudo reprovando): `docs/CATRACAS.md` §18.

**E o portão de commit que chama este comando junto com a suíte** é o
`portoes.sh` da raiz (pedido 421, `docs/CATRACAS.md` §19): suíte verde com
catraca reprovada sai vermelho, num código de saída só.
