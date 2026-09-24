# A marca na cascata solta custa MENOS que o `fsync` das filhas que ela tira

**Estado:** PENDENTE

## O que aconteceu

Pedido 540 (frente «integridade 2», 24/09/2026): a alteração solta que muda a
chave de uma mãe com filhas passou a gravar uma marca `.tx` antes, como uma
transação de uma instrução, e a aplicar pela passada do `COMMIT`
(`Servidor::alterar_solto` → `atualizar_com_a_marca` → `passada_sob_a_marca`).
O parecer do DBA dava o preço como «um `fsync` de marca por troca de chave com
filhas, operação rara», e a ordem da frente era medir a catraca
`alcancam-fsync-2` antes.

No meio do caminho, a mesma mudança quebrou um teste que nada tinha a ver com
cascata: `o_json_da_trilha_distingue_antes_ilegivel_de_antes_vazio` passou a
recusar a alteração de uma linha com o `.memo` corrompido («CRC do bloco …
não confere»).

## O que eu concluí primeiro, e estava errado

1. Que o conserto só podia **custar**: um `fsync` a mais, sob a trava global,
   por cascata. Escrevi isso no rascunho do `ACID.md` antes de medir.
2. Que planejar a cascata um passo antes era neutro — «é o mesmo plano que o
   `atualizar` faz por dentro». Não era o mesmo: o servidor lia a linha velha
   pelo `Table::ler`, que carrega as colunas externas, e o `atualizar` a lê por
   `decodificar(.., false)`, sem elas. A pergunta nova lia MAIS do que a
   escrita lia, e a linha que a escrita sabia gravar (com a trilha dizendo
   «antes ilegível») passou a ser recusada só porque alguém perguntou antes.

## O que a medição disse

- Sonda da frente, modo `custo`, 300 alterações, três rodadas intercaladas
  (mediana / p90, µs), mãe com duas filhas:
  - `por_lote`: 2.351–2.620 / 2.591–2.822 **antes**, 1.143–1.182 / 1.294–1.658
    **depois** — 2,0–2,2× mais rápida. O p90 de depois fica abaixo do mínimo de
    antes; os máximos (5 a 8 ms) se cruzam.
  - `por_operacao`: 2.848–3.272 antes, 2.538–2.940 depois — faixas cruzadas,
    empate.
  - sem filha: 87–88 µs nas duas versões.
- O motivo está no código, e não é palpite: a cascata do store sincroniza a mãe
  e cada tabela filha na hora (`Table::aplicar_ao_alterar`, o `sincronizar` de
  que a neta precisa entre duas linhas). A passada troca esses `fsync` por UM,
  o da marca, e põe as tabelas na janela — a marca só sai depois do `fsync`
  delas, então a durabilidade não cai.
- Catraca `alcancam-fsync-2`: 23 antes, 23 depois — a seção do `op_atualizar`
  já alcançava `fsync` pela janela.
- O teste da trilha voltou a passar com `Table::ler_sem_externos`, a mesma
  leitura do `atualizar`.

## A regra

Quando uma decisão muda de lugar para caber uma garantia na frente dela, ela
tem de ler **o mesmo** que lia no lugar de antes — e o preço da garantia se
mede contra o caminho que ela SUBSTITUI, não contra o que ela ACRESCENTA.

## Como está guardado hoje

- `testes_do_panico_sob_a_trava::a_cascata_solta_sem_queda_grava_inteira_e_a_marca_espera_o_fsync`
  (a marca espera a janela) e as três provas de queda do 540; a guarda
  `cascata-solta-sem-marca` as derruba com o caminho de antes.
- O teste da trilha (`o_json_da_trilha_distingue_antes_ilegivel_de_antes_vazio`)
  é quem pega a leitura a mais — não há guarda própria para «o plano lê o que o
  `atualizar` lê»; o buraco fica dito aqui.
- O número do custo não tem bancada: saiu da sonda do rascunho da frente, e
  não de um `resultados.json`. Refazer exige a sonda.
