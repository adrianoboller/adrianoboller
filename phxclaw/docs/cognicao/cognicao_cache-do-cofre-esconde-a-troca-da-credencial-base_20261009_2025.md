# Cache do cofre esconde a troca da credencial base: a chave é a referência, não quem a leu

**Estado:** PENDENTE (a prova roda na árvore; falta o commit)

**Evidência:** `crates/phxclaw-agent/tests/cofres.rs::vault_token_le_versao_e_campo_e_diz_o_erro_sem_o_valor`.
Na primeira corrida, com o token do Vault trocado por um errado e a MESMA referência lida
antes (`app/db`, `senha`, versão 1), o `unwrap_err` caiu: veio `Ok` com o valor de antes.
Com outra referência (`usuario`, versão 1), o 403 aparece. O cache em
`phxclaw-secret-broker/src/cofre.rs` é chaveado por `ReferenciaExterna` (cofre, caminho,
campo, versão) e não pela credencial base que fez a leitura.

## O que aconteceu

Trocar a credencial base (`phxclaw cofre vault-token`) não remonta os cofres ligados ao
broker: `cofres::ligar_com` só remonta quando a CONFIGURAÇÃO (`cofres.*`) muda. O valor lido
com o token antigo continua servido do cache até o prazo (60 s por padrão, teto 300 s).

## O que eu concluí primeiro, e estava errado

Que o teste do 403 falhava por defeito no cliente do Vault (o 403 não virando erro). Não era:
o pedido nem saiu — o cache respondeu antes, e respondeu certo pelo contrato dele.

## O que a medição disse

O cache faz o que promete (a segunda leitura não vai ao cofre: `cache_curto_serve_a_segunda_leitura_e_zero_desliga`
mede 1 leitura no Vault para 2 resoluções), e é exatamente isso que esconde a troca. A janela
é limitada pelo prazo, e o 401 do destino do nó HTTP já esquece a entrada
(`Credenciada::invalidar`), então valor GIRADO no cofre se recupera numa repetição.

## A regra

Teste de erro que passa pelo cache usa referência que o cache ainda não viu — senão ele mede o
cache, não o erro. E revogar a credencial base de emergência não esvazia o cache: quem precisa
disso hoje reinicia o processo ou espera o prazo (decisão aberta, não defeito escondido).

## Como está guardado hoje

Comentário no teste («Outra referencia: a de antes esta no cache e nem chegaria ao Vault»);
o prazo e o teto estão em `cofre.rs` (`TTL_MAX`, `TETO_MAX`) e nas chaves
`cofres.cache_segundos` e `cofres.cache_max` do catálogo.
