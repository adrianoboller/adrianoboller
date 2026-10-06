# Commit de salvaguarda no meio da prova real comita o defeito reposto

Data da descoberta: 06/10/2026 20:56 UTC (orquestrador; denunciado pela frente da onda 2).
Estado: **INFRUTÍFERO** — falha observada, com causa e prevenção.

## 1. O que se queria
Não perder trabalho (ordem do dono) enquanto frentes paralelas rodavam: o orquestrador
comitou a árvore inteira no branch de rascunho a cada pedido do gancho de parada.

## 2. O que aconteceu
O commit `15ce026b` levou o `fluxos.rs` no instante em que a frente da onda 2 tinha **8 defeitos
repostos de propósito** (prova real nos dois sentidos): recusa de segredo, recusa de ciclo, teto de
itens e semáforo desligados. O HEAD do rascunho ficou com quatro guardas de segurança desligadas.
A frente avisou; a árvore já estava restaurada; o commit seguinte (o desta cognição) as religou.

## 3. O que eu concluí primeiro, e estava errado
Que «salvaguarda em branch de rascunho é inofensiva porque ninguém integra dali». O dano não é a
integração: é o HEAD de um branch publicado dizer que um defeito de segurança é o código do dia,
e o próximo a retomar dali (outra sessão, outro agente) partir dele.

## 4. Causa e prevenção
- **Causa:** o estado da árvore durante a prova real é, por desenho, um estado ERRADO; comitar
  sem perguntar às frentes vivas captura esse estado.
- **Prevenção:** salvaguarda só com as frentes paradas, ou pedindo antes a cada frente viva que
  restaure os defeitos repostos e confirme; e conferir o diff com
  `grep -E '&& false|if false|_vaga = \(\)|REPOSTO'` antes do commit. Frente que repõe defeito
  marca a linha com `// REPOSTO` para que a conferência ache por padrão.

## 5. Número
1 commit com 8 defeitos repostos, 4 guardas de segurança desligadas no HEAD por 1 commit;
0 integrados no `phxclaw/v070-nativo`.
