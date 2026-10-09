# Hipótese morta: a E5 do desenho único («completar por id através das famílias») conserta o F5

**Estado:** INFRUTÍFERO

**Causa:** O id do `COMMIT` sai no BEGIN e o da marca do grupo (bidirecional e réplica fiel) nasce fora da trava, antes de ela ser tomada: o id menor pode entrar depois. O id diz quando o bilhete nasceu, não quando a trava o aplicaria.

**Prevenção:** Recupere na ordem em que a trava teria aplicado, não na ordem de nascimento do bilhete; o primeiro termo da chave é «nasce fora da trava», lido da versão do cabeçalho da marca (5/6), nunca do nome.

## 1. O que aconteceu

O desenho único da recuperação (`docs/propostas/recuperacao-e-replica-desenho-unico.md`) propôs, para o furo F5 (pedido 715), a E5: ordenar todas as marcas pelo id numérico, através das famílias `transacao_*` e `bidi_*` (invariante I13). Origem: `cognicao_ordem-de-aplicacao-nao-e-ordem-do-id-da-marca_20261008_1700.md`.

## 2. O que eu concluí primeiro, e estava errado

Que o F5 era defeito de ordem a consertar no escalonador, ordenando pelo id.

## 3. O que a medição disse

Duas coisas. Medida: depois do conserto do F1/F3 (marca v7), a marca de passada terminada grava zero eventos no arranque (`crates/phxsql-server/tests/recuperacao-nao-volta-o-valor.rs::completar_a_marca_de_passada_terminada_nao_acrescenta_eventos`). Medida também: completando pelo id, a linha 2 de `t` saiu a da ORIGEM («o2») e o `COMMIT` local perdeu a inclusão (`crates/phxsql-server/tests/ordem-da-recuperacao.rs::o_commit_que_entrou_antes_do_grupo_e_completado_antes_dele`). A E5 morreu com esse RED como prova.

## 4. A regra

Ao ordenar marcas, itens ou bilhetes para reaplicar, pergunte quem os cria dentro e fora da trava antes de ordenar pelo id.

## 5. Como está guardado hoje

`marca::marcas_com_prefixo` ordena por `(nasce_fora_da_trava, id)`. O aprendizado vivo continua no arquivo da ordem (sem guarda no catálogo, PENDENTE). Esta entrada alimenta o `AVOID.md` para a E5 não voltar pelo desenho.
