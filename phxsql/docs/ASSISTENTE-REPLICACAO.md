# O assistente de replicação, na tela

O wizard do botão **Replicação** do Centro de Controle. Ele configura os quatro
modos, e o que o separa de um cadastro com etapas é que **nenhum passo avança
sem o anterior provado**: o teste de conexão vem antes de configurar, e o passo
final mostra a posição dos dois lados.

Este documento é da TELA. O motor da replicação está em `REPLICACAO.md`.

## Os quatro modos, e o que cada cartão promete

| Modo | Papel local | Para quê |
|------|-------------|----------|
| **A** Primary → Replica | `replica` | distribuição e cópia: central → filial, relatórios, datacenter secundário |
| **B** Multi-Master | `multi` | os dois recebem escrita; conflito pela modificação mais recente; exige chave única |
| **C** Primary → Standby | `spare` | reserva de contingência; não atende cliente; assume por `spare_promover` |
| **D** Read Replica | `read_replica` | aceita leitura, recusa escrita apontando o primário |

O desenho de cada cartão é SVG à mão, só com variáveis do tema: **cilindro
pintado recebe escrita, contorno só lê, tracejado está de reserva**. O raio
âmbar do C é a promoção; a lupa azul do D é a consulta.

## O que o exercício provou, com número

Dois `phxsqld` de verdade — primário em 5340 (papel `source`) e réplica em
5345 —, o wizard rodado no navegador contra eles:

- **modo A de ponta a ponta:** 5 → 205 → 405 → 20.605 → **180.605 eventos**,
  com os dois lados iguais e o painel dizendo «em dia» ao fim de cada onda;
- **o atraso aparece na tela:** sob carga sustentada, o painel flagrou
  **40.000 eventos de atraso** (180.605 lá contra 140.605 aqui), com o pino
  vermelho «ATRÁS 40.000» na coluna «estado», e voltou a «em dia» sozinho.

### Degrau único não mede atraso — carga sustentada mede

A primeira tentativa de flagrar o atraso inseriu 200 linhas e depois 20.000, e
**as duas falharam**: a réplica aplicou o lote inteiro em menos de um tique de
3 s do painel, e a tela só via «em dia» dos dois lados. O que faz o atraso
existir na tela não é o tamanho do degrau, é a escrita **continuar** enquanto a
réplica corre atrás: oito lotes de 20.000 sem pausa (160.000 linhas em 2,73 s)
puseram o pino vermelho na tela na primeira medida.

A lição serve para qualquer medida de fila: **um degrau menor que a janela de
medida é invisível**, por maior que ele pareça no absoluto.

## Os defeitos que o exercício achou

**1. Botão mini com cor de ação nascia cinza e ficava vermelho no hover.**
O atalho «acompanhar a replicação» é `botao mini consultar`. A regra
`.botao.mini` vem **depois** de `.botao.consultar` no arquivo, com a mesma
especificidade (0,2,0) — então ela vencia e pintava o botão de cinza; e
`.botao.mini:hover{color:var(--log)}` pintava de **vermelho**, a cor de
excluir, um botão que só consulta. Atingia também os três minis que já
existiam antes deste (um `incluir`, um `alterar`, um `consultar`). Conserto:
cinco pares de regras `.botao.mini.<acao>` e `:hover`, depois da regra do mini.
Medido com `getComputedStyle` nos dois temas: contorno azul parado
(`#5fa6e8` no escuro, `#1f5c93` no claro), preenchido só no hover.

É a mesma lição já escrita no CLAUDE.md — **o CSS global morde todo componente
novo** —, agora por ordem de cascata em vez de por seletor de elemento.

**2. O campo de hora cortava o valor.** `width:8em` mostrava «03:00 AN» no
lugar de «03:00 AM» com o ícone do relógio. Foi para `10.5em`.

**3. O glifo `⇄` não existe na IBM Plex Mono** do `.v-sql`, e o navegador
pintava um substituto que parecia `≠` — o cartão do Multi-Master dizia o
contrário do que queria dizer. Trocado por `↔`.

**4. `.sub` sem escopo trocava o título da tela de trás.** Ao escolher outro
cartão, `$(".sub").textContent = ...` pegava o subtítulo do painel, não o do
diálogo. Passou a buscar dentro do corpo do diálogo.

## O que o assistente aprendeu a NÃO fazer

**Não fingir que aplicou.** O `phxsqld` de hoje não muda a replicação em
execução, e não existe operação que o faça. Em vez de gravar um arquivo pelas
costas ou dizer «pronto» sem nada ter mudado, o passo 6 entrega **o bloco exato
do `config.json`** para os dois lados, manda reiniciar, e o botão «já apliquei
e reiniciei — conferir» relê o `config` do servidor e **recusa avançar**
enquanto o papel e a origem não aparecerem. Foi assim que o modo A foi provado:
o bloco que a tela entregou é literalmente o que entrou no config da réplica.

**Não adivinhar o que trava um modo.** A lista de impedimentos vem do outro
servidor (`impedimentos` e `sem_chave_unica` da sonda), e não de palpite da
tela. O que a tela ainda diz por conta é o que ela sabe: que uma réplica sem
`somente_leitura` diverge, e que reserva agendada perde o que aconteceu desde a
última rodada.

## O campo do token: por que `token_remoto`, e não `token`

O pedido do protocolo já tem um `token` — o de **quem pede**. Uma operação que
liga a outro servidor precisa de um segundo token, o **de lá**, e os dois não
cabem no mesmo nome: no `/api`, a interface monta
`{ token: est.token, op, ...params }`, então um `token` vindo nos parâmetros
**sobrescreve o da sessão** e o pedido é recusado pelo próprio servidor local
antes de sair. É por isso que a sonda lê `token_remoto`.

Vale como regra: **quando uma operação passar a falar com outro servidor,
procure que campo do pedido já tem dono.**
