# Contar DEPOIS de gravar abre uma janela que a outra thread enxerga — e o floco de 1 em centenas era ela

**Estado:** PENDENTE

## 1. O que aconteceu

`escrita_local_sem_o_source_andar_rompe_pela_contagem_dizendo_a_causa`
(`crates/phxsql-server/tests/continuidade-da-replica.rs`, pedido 630) falhou
UMA vez numa corrida do `./portoes.sh` e passou sozinho e na seguinte.

Causa raiz, no **código** e não no teste: o pedido 630 mudou a conta da escrita
local na réplica para DEPOIS de a escrita responder `Ok`
(`executar_e_contar_escrita_local`, `servidor.rs`). Entre a escrita entrar no
diário (já visível ao `posicao` do source/da rodada) e o contador subir existe
uma janela. A rodada da réplica, noutra thread, que caia ali via o diário
andado (`no.eventos < posicao`), chamava `por_que_nao_continua` com ZERO
escritas locais e gravava a frase das duas causas («...foi escrita
localmente...»). A recusa fica guardada por posição (`continuidade_guardada`),
então nenhuma rodada seguinte a corrige, e o teste lia «escrita localmente»
onde esperava «escrita LOCAL».

## 2. O que eu concluí primeiro, e estava errado

Hipóteses escritas antes de medir: (a) relógio/prazo curto; (b) porta ocupada
ou réplica ainda não conectada; (c) interferência de outro teste do mesmo
binário; (d) rodada comparando a contagem antes de a escrita local contar.
Medido: (a), (b), (c) mortas por **0 falhas em 60 execuções isoladas, 0 em 30
do binário inteiro com `RUST_TEST_THREADS=4` e quatro laços de CPU ao lado** —
o floco não se reproduz por estresse, porque a janela é de microssegundos.
Só (d) sobrou, e ela se prova por leitura do código + gancho, não por laço.
Quem tentasse «provar por 1000 execuções» acharia o teste sempre verde.

## 3. O que a medição disse

Um gancho de teste (`GANCHO_APOS_ESCRITA`, só `cfg(test)`) roda no instante
exato da janela. Com o defeito reposto (contar depois do `Ok`), o gancho lê:
`a tabela foi apagada e recriada no source, ou esta replica (sem
somente_leitura) foi escrita localmente...` — a frase das duas causas, a mesma
do floco. Com o conserto (contar ANTES, desfazer se a escrita falhar): `ACEITOU
1 pedido(s) de escrita LOCAL`. O conserto não reabre a revisão SEC M2: a
entrada do mapa sai quando a operação falha e chega a zero
(`escrita_que_falha_nao_deixa_conta_nem_entrada_no_mapa`; sem o desfazer, esse
teste cai).

## 4. A regra

Estado que outra thread lê para DECIDIR (aqui, o contador que escolhe a causa da
recusa) sobe antes do efeito que ela pode ver e desce se o efeito falhar — nunca
depois. Floco que não reproduz por estresse é janela estreita: abra-a com um
gancho, não com mais execuções.

## 5. Como está guardado hoje

- Teste: `servidor::testes_janela_da_escrita_local::na_janela_entre_gravar_e_responder_a_causa_ja_e_a_escrita_local`
  (RED com o defeito reposto, GREEN com o conserto) e o irmão
  `escrita_que_falha_nao_deixa_conta_nem_entrada_no_mapa`.
- Guarda: `escrita-local-contada-depois-da-escrita-630` no catálogo (PROVADA
  1/1); a guarda `escrita-local-na-replica-calada` teve o trecho atualizado
  (a linha do `entry` mudou) e foi reprovada (1/1).
- Buraco: a janela de verdade (duas threads) não é exercitada pelo soquete;
  quem a fecha é o gancho. Se alguém mover a conta de volta para depois, o
  gancho acusa — o soquete, não.
