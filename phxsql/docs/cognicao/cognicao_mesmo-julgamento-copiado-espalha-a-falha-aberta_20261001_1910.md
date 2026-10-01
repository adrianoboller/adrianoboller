# «Mesmo julgamento do `conferir_filhas`» copiado espalha a falha aberta

**Estado:** INFRUTÍFERO

**Causa:** a decisão «irmã que não abre fica de fora, para uma tabela quebrada não trancar o banco» respondia «ninguém aponta para mim» pela irmã que não responde; ela estava escrita em quatro lugares (`irmas.rs`, `planejar_ao_alterar_com`, `quem_aponta_para`, e o comentário do `fks_que_apontam_para_mim`), três deles justificados pelo comentário «mesmo julgamento do `conferir_filhas`» — e a falha aberta viajou junto com a cópia. Medido: com o `.reg` da filha truncado a 16 bytes, a mãe com filha saiu de vez, suave, pelo `excluir_tabela` e pela troca de chave.

**Prevenção:** a decisão mora numa função só (`irmas::abrir_irma`), que recusa nomeando a irmã e a causa; as três buscas passam por ela, e cada uma tem guarda própria no catálogo (`irma-que-nao-abre-some-do-*`). A saída da recusa é o `excluir_tabela` da própria irmã, que pula a si mesma; a irmã em troca interrompida abre pelo `RegFile::abrir` (guarda `irma-em-troca-vira-recusa-eterna`).

## O que aconteceu

Pedido 631 (revisão SEC M3, 01/10/2026). A busca reversa da integridade abria
cada irmã com `RegFile::abrir` e, no erro, `continue`. O pedido 259 reproduziu
o mesmo `Err(_) => continue` no `irmas.rs` ao trocar a varredura por uma
memória com carimbo, e o comentário dele dizia «irmã que não abre fica de
fora» — fiel ao original, e por isso fiel ao defeito.

## O que eu concluí primeiro, e estava errado

Que o defeito estava em **um** lugar, o `irmas.rs` que o pedido nomeava, e
que o «irmão em `table.rs`» era o `fks_que_apontam_para_mim`. Não era: esse só
delega ao `irmas.rs`. O irmão de verdade — quem chama as mesmas funções na
mesma ordem — era o `planejar_ao_alterar_com` (cascata do `ao_alterar`), e um
terceiro morava no catálogo (`quem_aponta_para`, que serve o `excluir_tabela`
e o renomear). Os dois se acharam procurando o **comentário** que citava o
julgamento, não o nome da função.

## O que a medição disse

Com a falha reposta nas três cópias: 4 dos 5 testes de
`tests/irma-que-nao-abre.rs` caem, cada um no caminho dele (excluir de vez e
suave, `excluir_tabela`, troca de chave, e a porta de saída); o do
comportamento velho (irmã legível sem chave) segue verde. Com a irmã aberta
sem curar (`abrir_sem_escrever`), a irmã em troca decidida tranca a exclusão
da vizinha — o teste `a_irma_em_troca_interrompida_nao_tranca_a_exclusao_da_vizinha`
cai no `unwrap_or_else`.

## A regra

Quando um comentário diz «mesmo julgamento de X», a decisão está escrita duas
vezes: mova-a para uma função e faça as duas chamarem a função — e procure as
outras cópias pelo texto do comentário.

## Como está guardado hoje

`irmas::abrir_irma` é a única cópia; quatro guardas no catálogo, provadas. O
`integridade::conferir_diretorio` continua pulando a tabela que não abre, e
de propósito: é relatório, e ela sai nomeada em `nao_abriram` — o comentário
dele deixou de dizer «mesmo julgamento». Buraco nomeado: duas irmãs quebradas
no mesmo diretório se trancam uma à outra no `excluir_tabela` (cada uma pode
ser a mãe da outra); a saída aí é o `reparar` ou tirar os arquivos à mão.
