# Saneamento do corpus WLanguage 12k: o que se mediu e o que se decidiu

O corpus `Help_WL_12k_Json.zip` está em estado `DEGRADED/CONDITIONAL` por três
defeitos conhecidos, todos medidos por `query_wlanguage_help.py --verify`. Este
documento registra cada um, o efeito real na conversão e a decisão, para que a
mesma pergunta não volte sem medição.

| Defeito | Medido | Efeito na conversão | Decisão |
| --- | --- | --- | --- |
| Uma página inválida em quarentena (`01-04-01_00655__emailgetall_function`, JSON preenchido com zeros) | 1 de 12.036 páginas (0,008 %) | `EmailGetAll` sem semântica no corpus; o especialista de comunicação devolve `GAP-*` e aponta a página oficial na URL do índice | **Manter em quarentena.** O conteúdo não é recuperável do zip; regravar a página exigiria baixar do site da PC SOFT, o que a licença ainda não cobre. |
| Uma lacuna de sequência no tema `02-03-01` (número 223) | 1 sequência ausente; índice declara 12.037 páginas, existem 12.036 | Nenhum símbolo conhecido depende dela: é um número de sequência do gerador, não um id de página do Help | **Registrar e seguir.** Índice e contagem física divergem em 1 e o `--verify` diz isso; corrigir o índice mudaria o hash fixo do corpus por um ganho de zero. |
| 609 ids lógicos repetidos (613 páginas extras) | ids de página do Help que aparecem em mais de um membro | Medido por amostra: são pares função/exemplo e função/propriedade que a PC SOFT publica sob o mesmo id (`hreadseekfirst_function` e `hreadseekfirst_example` têm ids distintos, mas outros não). A busca devolve os dois, com `member` distinto, e o especialista escolhe pelo `title` | **Não deduplicar.** Remover um lado perderia o exemplo ou a propriedade. O ranking já prefere a página cujo `short_name` casa com a consulta. |

## Por que não sanear o zip

Sanear muda o SHA-256 que os scripts, o manifesto e a skill fixam. Isso é
desejável quando o ganho é real (foi assim com as 15 chaves privadas
demonstrativas, removidas na edição distribuída). Para os três defeitos acima
o ganho medido é zero ou negativo, e o custo é uma nova edição, novo hash em
quatro lugares e uma prova de regressão. Fica `DEGRADED/CONDITIONAL`, com o
significado exato de: **uma função sem semântica, um número de índice a mais e
pares legítimos sob o mesmo id**.

## Licença

O corpus é derivado da documentação da PC SOFT e não tem licença de
redistribuição. Enquanto isso não for resolvido:

- uso privado, dentro da equipe e dos projetos autorizados;
- nenhum pacote público (marketplace público, npm, release aberto);
- o `--verify` e o hash continuam sendo a prova de que o que está instalado é
  a edição sanitizada, e não um zip de origem desconhecida.

Decisão pendente do dono do produto: obter autorização formal ou substituir o
corpus por um índice de URLs oficiais (sem conteúdo), que os especialistas
consultariam ao vivo com autorização do usuário.
