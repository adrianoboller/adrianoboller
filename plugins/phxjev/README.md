# PhxJev

Plugin do Claude Code, **so markdown**, no molde do Jev (TypeSafe): em vez de
mais uma opiniao em prosa, um **juiz tipado** — perguntas `noul`/`choice`/`score`,
probabilidade por opcao, e o veredito tirado de **limiar fixo**.

## O que copia do Jev, e onde diverge

| | Jev | PhxJev | Restricao nossa que causou a divergencia |
|---|---|---|---|
| Quem julga | modelo proprio, via API paga | o proprio agente, pela skill | zero dependencia externa; sem chave |
| Calibracao | medida | registro + `desfecho` + `calibrar` (Brier, faixas); **nao medida** abaixo de 50 desfechos | medida, nao declarada |
| Limiar | codigo | codigo: `scripts/phxjev.py` (so `std`) | exercitado ao vivo, o juiz em markdown fugiu do formato |
| Presets | review / why / pick / ask | revisar / porque / escolher / perguntar | + `defeito_ativo` (☐ × ⏸, decisao de 24/09) e `fere_petrea` + regua PG4/MDB3/MY2/SQ1 |
| Evidencia | nao exige | **cada `p` cita `arquivo:linha`**; sem evidencia = 0,5 | modo honesto; diagnostico plausivel nao e medido |

## Instalar

Direto do GitHub (a branch vai no `#`; sem ela, o marketplace le a branch
padrao, que ainda nao tem o plugin):

```
/plugin marketplace add adrianoboller/adrianoboller#claude/phxjev-markdown-plugin-vdvios
/plugin install phxjev@phoenix
```

Pelo pacote: `bash plugins/phxjev/empacotar.sh` gera `dist/phxjev-<versao>.zip`,
`.tar.gz` e `.sha256` (so com testes verdes e plugin validado); descompacte e
`/plugin marketplace add <pasta>`. O `bancada/` do pacote e a do PhxSql: serve
de referencia, e os casos dela apontam para o codigo daqui.

Local, sem marketplace: `claude --plugin-dir plugins/phxjev`.

## Usar

```
/phxjev-revisar  <achados | PR | diff>
/phxjev-porque   <sintoma> [hipA | hipB]
/phxjev-escolher <decisao> : <opA> | <opB>
/phxjev-perguntar noul "o .reg reaproveita slot excluido?"
```

## Juiz local e bancada

Ollama compilado do fonte (o binario da release nao passa pelo proxy), modelo
pequeno, probabilidade pelo logprob da letra. `bancada/comparar.py` mede os
dois juizes nas mesmas perguntas com verdade conferida; resultado com data em
`bancada/resultados.json`.

## Testes

```
python3 plugins/phxjev/scripts/teste_phxjev.py
```

## Limite

Veredito PhxJev **nao e evidencia validada**: aprendizado que nasce dele e
PENDENTE e nao vira FRUTIFERO por causa dele.
