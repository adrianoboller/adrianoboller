# PhxJev

Plugin do Claude Code, **so markdown**, no molde do Jev (TypeSafe): em vez de
mais uma opiniao em prosa, um **juiz tipado** — perguntas `noul`/`choice`/`score`,
probabilidade por opcao, e o veredito tirado de **limiar fixo**.

## O que copia do Jev, e onde diverge

| | Jev | PhxJev | Restricao nossa que causou a divergencia |
|---|---|---|---|
| Quem julga | modelo proprio, via API paga | o proprio agente, pela skill | zero dependencia externa; sem chave |
| Calibracao | medida | **nao medida** (declarada na saida) | nao ha bancada ainda — skill §7 |
| Limiar | codigo | tabela fixa na skill | pedido: plugin em markdown |
| Presets | review / why / pick / ask | revisar / porque / escolher / perguntar | + `defeito_ativo` (☐ × ⏸, decisao de 24/09) e `fere_petrea` + regua PG4/MDB3/MY2/SQ1 |
| Evidencia | nao exige | **cada `p` cita `arquivo:linha`**; sem evidencia = 0,5 | modo honesto; diagnostico plausivel nao e medido |

## Instalar

```
/plugin marketplace add adrianoboller/adrianoboller
/plugin install phxjev@phoenix
```

Local, sem marketplace: `claude --plugin-dir plugins/phxjev`.

## Usar

```
/phxjev-revisar  <achados | PR | diff>
/phxjev-porque   <sintoma> [hipA | hipB]
/phxjev-escolher <decisao> : <opA> | <opB>
/phxjev-perguntar noul "o .reg reaproveita slot excluido?"
```

## Limite

Veredito PhxJev **nao e evidencia validada**: aprendizado que nasce dele e
PENDENTE e nao vira FRUTIFERO por causa dele.
