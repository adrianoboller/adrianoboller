# O alvo se diz no pedido, e o número da tela se lê do servidor — duas pontas da mesma heurística

**Estado:** PENDENTE

- **Quando:** 2026-10-02, 09:50
- **Onde:** `crates/phxsql-server/src/servidor.rs` (`op_encerrar_sessao`, `ping`),
  `ui/index.html`, `conferidor.rs` (`numeros_cravados`)
- **Pedidos:** 644 e 645

## O que aconteceu

`op_encerrar_sessao` separava «sessão web» de «conexão» por o id ter letra. Id
web é hex de 8 dígitos; 2,3 % saem só com algarismos e viram «número de
conexão». A tela dizia «porta 5000» e «5 arquivos por tabela» por motivo
parecido: o texto sabia de cabeça o que só o servidor sabe.

## O que eu concluí primeiro, e estava errado

Que o conserto de 644 era «olhar melhor o texto» (comprimento 8, hex). Não:
qualquer regra sobre a forma do texto tem um caso que cai do outro lado — o id
web de prefixo curto («1») continua igual a um número. A forma não carrega a
informação; só o pedido sabe. E que a contagem de arquivos do 645 era uma
constante a corrigir (5 → 11): a constante envelheceria de novo, então a lista
sai do motor (`extensoes_de_uma_tabela`), pelo `ping`.

## O que a medição disse

- RED do 644: com a heurística reposta, o pedido `id:"1", tipo:"web"` derrubou
  a conexão 1 e deixou a sessão web de pé (`testes_encerrar_sessao_644`).
- Guardas provadas: `encerrar-sessao-adivinha-web-pela-forma`,
  `ping-crava-a-porta-5000`, `conferidor-nao-ve-porta-cravada` (PROVADA).
- Conferidor de número cravado: 6 achados medidos (catraca nova, nasce em 6);
  `TETO_ROTULOS_E_CRASE` caiu 863 → 861.
- Prova de tela com o cravado reposto: o Profiler falha esperando `porta 6230`.

## A regra

Decisão que a forma do dado não carrega vai **no pedido**; número que só o
servidor sabe a tela **lê** do servidor — e o conferidor acusa o molde, não a
frase.

## Como está guardado hoje

Rust: `testes_encerrar_sessao_644`; web: casos 40 e 46 + `prova-real-botoes.mjs`;
catraca `TETO_NUMERO_CRAVADO_EM_TELA`. Buraco: 6 números do mesmo molde seguem
na tela (`nt_sete_arquivos`, `g_prompt_duplicar_nome`, `sb_bio_adriano`,
«7000 by default» do swagger) — dentro da catraca, fora destes pedidos.
