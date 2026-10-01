# O redirecionamento leva o token do GitLab e não o do GitHub

**Estado:** PENDENTE (a evidência existe na árvore, falta o commit onde a prova roda)

**Evidência:** `crates/phxclaw-agent/tests/codigo.rs::forja_que_redireciona_nao_leva_o_token_e_nao_vira_diff_vazio`,
RED com `follow_redirects = true` (o `glpat-REDIR-GL` chegou ao host do redirecionamento) e GREEN com o
guarda que trata todo 3xx como erro em `forja.rs::chamar`.

## O que aconteceu

A forja desliga redirecionamento e o teste da frente passava. A prova real ligou o redirecionamento de
propósito e mediu: o GitHub (cabeçalho `Authorization`) não vazou, o GitLab (`PRIVATE-TOKEN`) vazou.
E, com o redirecionamento desligado, um 301 virava sucesso: `pr_diff` devolvia diff vazio, e uma revisão
de PR sem nada revisado passaria no portão `--falhar-em alta`.

## O que eu concluí primeiro, e estava errado

Que «o reqwest limpa credencial no redirecionamento». Ele limpa **os cabeçalhos sensíveis que conhece**
(`Authorization`, cookies) ao trocar de origem; cabeçalho próprio de API, como o `PRIVATE-TOKEN`, segue
junto. E que «redirecionamento desligado» encerrava o assunto: desligado, o 3xx chega como resposta, e
quem só confere `>= 400` o aceita como corpo.

## O que a medição disse

8 mutantes novos na frente de git; 6 viviam (4 passavam por engano, 2 eram defeitos ativos). Todos
mortos. Dois deles só morreram quando o teste passou a medir o DANO antes do veredito: conferindo o
`is_err` primeiro, o mutante do redirecionamento falhava pelo motivo errado e o vazamento não aparecia.
O teste do `.gitignore` aninhado só mata o mutante se a pasta com a regra não for a primeira irmã na
ordem do percurso (por isso a filha chama `z/`).

## A regra

Credencial de API em cabeçalho próprio não é protegida pelo cliente HTTP: o redirecionamento se desliga
E todo 3xx é erro. E prova de vazamento mede o que chegou ao outro lado antes de olhar o veredito.
