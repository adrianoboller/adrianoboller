# A última prova vermelha virar verde quebrou o conferidor das vermelhas

**Descoberta:** 10/09/2026, construindo o pedido 211.

## 1. O que aconteceu

O 211 mandou virar VERDE a guarda vermelha
`tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio` (tirar o
`#[ignore = "VERMELHA de proposito"]` e consertar o `posicao_do_diario`). Ela
era a **última** prova vermelha da árvore — a outra, a da cifra em coluna
externa (pedido 210), já tinha virado verde em 08/09.

Com a árvore limpa, `cargo test --workspace` quebrou num lugar que o 211 não
tocou: `conferidor_vermelhas::testes::o_conferidor_enxerga_as_vermelhas_que_existem`,
em `crates/phxsql-server/src/conferidor_vermelhas.rs:192`. Esse teste fazia
`varrer()` na árvore e exigia `!todas.is_empty()` — «o conferidor tem de achar
PELO MENOS uma vermelha, senão o casador quebrou». No dia em que o projeto
consertou a última vermelha, a afirmação passou a ser falsa, e o teste **falhou
por SUCESSO**.

## 2. O que eu concluí primeiro, e estava errado

Que virar uma guarda vermelha em verde era uma mudança **local**: mexe no
`posicao_do_diario`, na guarda, e acabou. A frente do 211 estava fechada e
verde nos seus próprios testes (cluster + guarda). Só a suíte inteira, no
`--workspace`, revelou o defeito — que não estava em nenhuma das duas frentes,
estava **no encontro delas**: o meu conserto zerou o denominador de um
conferidor de QA que eu nem sabia que existia. É exatamente o defeito que «só
aparece no encontro das frentes», e por isso a integração é papel próprio e não
sobra de ninguém.

O segundo errado, mais sutil: o próprio comentário do teste dizia *«ou a árvore
não tem mais (e aí a catraca vira código morto e sai), ou o casador quebrou»* —
oferecendo REMOVER o conferidor como saída. Quase removi. Removê-lo perderia a
catraca útil (`TETO_VERMELHA_SEM_PEDIDO = 0`, que obriga toda vermelha FUTURA a
ter pedido no `PENDENCIAS.md`) por causa de um estado momentâneo — árvore limpa
hoje não quer dizer árvore limpa amanhã.

## 3. O que a medição disse

A árvore tem hoje **zero** provas vermelhas (`sem_pedido()` e `varrer()`
devolvem lista vazia). O conferidor tem três testes: a catraca
(`toda_prova_vermelha_tem_pedido_no_pendencias`) passa com zero (`0 <= 0`), o
controle do negativo (`nome_que_nao_esta_no_pendencias_conta_como_sem_pedido`)
passa, e só a prova-real do casador falhava — **1 de 1020** testes do
`phxsql-server`. Reposto o defeito do 211 (devolver a soma com `incompleta`
sempre `false`), a guarda do 211 falha na asserção da bandeira
(`servidor.rs:38094`), confirmando que ela pega nos dois sentidos.

## 4. A regra

**Um conferidor que se prova exigindo uma instância viva daquilo que ele conta
falha justamente quando o projeto zera a contagem — punindo o sucesso. Prove o
casador contra um fixture sintético, não contra a árvore.** A prova-real de um
detector tem de valer com a árvore vazia OU cheia; o que ela verifica é que o
casador RECONHECE a marca, e isso não depende de haver uma marca de verdade no
disco naquele instante.

## 5. Como está guardado hoje

O casamento por arquivo saiu do laço que lê o disco para uma função pura
`casar_arquivo(rel, texto, pendencias)`; `varrer()` a chama por arquivo, e a
prova-real virou `o_casador_enxerga_uma_vermelha_sintetica`, que monta um fonte
sintético (com a marca partida em `"VERMELHA de " , "proposito"`, pelo mesmo
motivo da `MARCA`, para o próprio conferidor não se achar) e exige que o casador
ache a marca, o nome da função logo abaixo, e a conte como sem pedido com o
`PENDENCIAS` vazio. A catraca `TETO_VERMELHA_SEM_PEDIDO = 0` continua de pé para
a próxima vermelha. `docs/CATRACAS.md` §7 e `docs/TESTES.md` §16 foram
atualizados; a lei disso já existe no `CLAUDE.md` («guarda registrada com o
defeito que a motivou, provada periodicamente contra ele») — o que este arquivo
acrescenta é o **alcance**: a prova periódica não pode depender de o defeito
ainda existir.
