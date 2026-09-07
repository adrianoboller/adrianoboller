# Z) atualização completa do dossiê verificando se ficou algo errado e colocando essas 26 perguntas com as suas devidas respostas

*Feito em 2026-09-07 16:51 UTC, sobre o commit `a56a165`; os números abaixo saem dos geradores, não desta prosa.*

## Resposta curta

O dossiê ganhou a **seção 36 — «As perguntas do dono, e a resposta exercitada
de cada uma»**, com a resposta curta das 29 (as três abertas + A–Z), lida dos
**mesmos arquivos** que geram o PDF (`docs/pdf/respostas/*.md`) por um gerador
novo, `docs/dossie/perguntas-no-dossie.py` — o décimo primeiro da receita. Uma
fonte, duas saídas: o resumo no dossiê e o PDF inteiro não podem divergir
porque não há segunda cópia.

«Verificando se ficou algo errado» foi feito **medindo**, e achou:

- o `.fts` faltava em **três** inventários — Figura 1, Figura 8 e a tabela
  do `FORMATO.md` — corrigidos e provados no navegador (resposta 0.1);
- a seção 4.3 do `TECNOLOGIAS.md` marcada `GERADO` trazia números digitados
  (corrigida na revisão completa da tarde, commit `a56a165`);
- **doze pedidos novos** (213–224) saíram das baterias desta rodada, do mais
  sério — `replicas_autorizadas` vazia libera todos (214) — ao mais miúdo
  (recusas do SQL dizendo a coisa errada, 219).

## Exemplo exercitado

O que os geradores imprimiram nesta rodada (cada linha é saída de script):

```text
dossie-phxsql-0.18.html: secao das perguntas regravada -- 29 respostas
224 pedidos: 198 feitos, 8 parciais, 18 planejados
html: docs/pdf/phxsql-26-perguntas.html (320,998 bytes, 29 respostas)
pdf:  docs/pdf/phxsql-26-perguntas.pdf (3,905,770 bytes, 83 paginas contadas pela marca /Type /Page)
dossie-phxsql-0.18.html: 30 figuras numeradas, 0 corrigidas
```

E o que a revisão dos inventários mediu antes e depois (resposta 0.1):

```text
Figura 1, condicionais ........ 3 -> 4  (+ .fts)
Figura 8, extensões ........... 8 -> 9  (+ .fts, pendurado no .ndx, «fora do desfazer»)
FORMATO.md, tabela ............ 9 -> 10 linhas (+ §17)
```

## O que NÃO existe, e é dispensa registrada

- **As figuras continuam à mão.** Gerá-las do código perderia o texto de
  decisão de cada caixa; o conserto de raiz é a **guarda** que compara a
  lista viva (`arquivos_da_tabela`) com as três cópias — pedido **213**.
- **O dossiê não republica sozinho.** As quatro páginas (dossiê, pedidos,
  testes, gráficos) foram republicadas nas mesmas URLs nesta rodada, à mão,
  como sempre.
- **Os doze pedidos novos estão abertos, não feitos** — este item pediu
  verificação, e verificação que conserta tudo o que acha vira outra rodada
  sem o dono decidir a ordem. O 214 (segurança) é o que eu poria à frente.

## Como se refaz

```bash
python3 docs/dossie/perguntas-no-dossie.py   # a seção 36
python3 docs/pdf/gerar.py                    # o PDF, das mesmas respostas
# e a receita inteira do dossiê, em docs/dossie/LEIA-ME.md (onze geradores)
```
