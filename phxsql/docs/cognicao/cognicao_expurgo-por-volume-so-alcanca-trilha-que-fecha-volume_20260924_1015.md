# O expurgo por volume só alcança a trilha que fecha volume — e a tabela padrão nunca fecha

**Estado:** PENDENTE

*24/09/2026, 10:15 — pedido 368 (expurgo da trilha `.lgpd`).*

## 1. O que aconteceu

O pedido 368 e o parecer do DBA partiam de uma premissa: «a trilha já é
volumada pelo mesmo corte dos diários (`trilha.rs`, `crate::diario::paginacao`),
então o expurgo por volume inteiro derruba a janela mais velha sem reescrever
arquivo nenhum». A frente implementou exatamente isso — volume inteiro, nunca o
ativo, sempre um prefixo — e, lendo o caminho do corte até o fim, a premissa
só vale para **metade** das tabelas.

`diario::paginacao(esquema)` devolve o esquema intacto quando ele não é
paginado (`if corte == 0 || !esquema.ligada()`), e `valores.rs::esquema_de_json`
só pagina quando o `criar_tabela` traz `registros_por_arquivo`. Sem ele — o
padrão do protocolo — o `.lgpd` é **um arquivo só** (`clientes.lgpd`, sufixo
vazio), que é o volume ativo para sempre.

## 2. O que eu concluí primeiro, e estava errado

Que o ponto delicado seria a **numeração contígua** na leitura da trilha
depois de o volume 1 sumir — era o que o pedido mandava medir. Não era: o
`Volumes::existentes` filtra `1..=max_arquivos` por existência e nunca supôs
contiguidade, e a prova (`a_trilha_continua_depois_do_expurgo`) passa; com o
defeito reposto (`take_while` no lugar de `filter`) ela cai. O buraco estava um
passo antes: **se há volume fechado para expurgar**.

## 3. O que a medição disse

- Tabela sem paginação: zero volumes fechados, sempre. A resposta do expurgo
  dizia `arquivo_unico` (a chave e o teste dela saíram com o formato B).
- Tabela paginada, corte padrão de 1 GiB: o medidor
  (`cargo run --release --example custo-do-expurgo -p phxsql-store -- 64 8`)
  mediu **139 bytes/registro** → **~7,7 milhões** de registros por volume; a
  mil eventos por dia (conta, não medida), **~21 anos** para fechar o primeiro.
- O plano custa **1,11 ms/MiB** de volume fechado com cache quente, e 0,94 ms
  quando nada venceu.

## 4. A regra

Antes de construir sobre «o arquivo já é volumado», siga o corte até o fim e
pergunte **quando o primeiro volume fecha na configuração padrão** — volume que
nunca fecha é arquivo único com outro nome.

## 5. Como está guardado hoje

O buraco levou ao papel C, e o parecer dele (24/09/2026) decidiu o **formato
B** na mesma versão: o ativo é sempre `<tabela>.lgpd`, os fechados são
`<tabela>_NNN.lgpd` sem teto, e a trilha fecha por tamanho (`lgpd.volume_mib`,
64), por **idade** do primeiro registro (`lgpd.volume_dias`, 30) ou a pedido —
independente da paginação do `.reg` (`docs/FORMATO.md` §7, `docs/LGPD.md`
§10). A tabela padrão passou a fechar volume e a expurgar:
`crates/phxsql-store/src/trilha.rs::a_tabela_sem_paginacao_fecha_por_idade_e_expurga`
e `crates/phxsql-store/tests/trilha-lgpd.rs::o_expurgo_pela_tabela_padrao_grava_o_rastro_e_so_derruba_volume_fechado`,
e contra o sistema operacional o `bancada/lgpd/prova-do-expurgo.py`, que usa a
tabela padrão. O expurgo continua dizendo o que não fez (`volume_ativo`,
`fronteira`, `retido_vencido_desde`), e o relógio nomeia as tabelas com dado
vencido retido.

O estado fica **PENDENTE**: a regra da seção 4 é de processo («siga o corte
até o fim antes de construir sobre ele»), e nenhuma prova mede se ela pega na
próxima vez.
