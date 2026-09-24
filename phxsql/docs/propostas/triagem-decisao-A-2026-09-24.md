# Triagem da decisão A nos pedidos abertos — proposta do papel G (QA)

24/09/2026. Só leitura: nada do repositório foi editado. Fonte: `phxsql/docs/PENDENCIAS.md`, lida linha a linha (título e corpo inteiros) por script em scratch, com o mesmo `LINHA` do `pagina-dos-pedidos.py`; a porcentagem usa a fórmula de `percentual_falta` do gerador: (parcial + planejado) ÷ (feito + parcial + planejado), sem os ⏸.

## Contagem

- Estado hoje (medido): ☑️ 361 · ◐ 16 · ☐ 89 · ⏸ 60 = 526. **Abertos: 105** (◐ 16 + ☐ 89) — os 105 conferem.
- Por classe: **DEFEITO 67** · **DONO 28** · **BLOQUEIO 2** · **ADIAR 8** (todos ☐; nenhum ◐ vai para ⏸).
- Falta hoje: 105 ÷ 466 = **22,5%**.
- Falta com a proposta: 97 ÷ (361 + 97) = 97 ÷ 458 = **21,2%**; ⏸ passa de 60 para 68.
- Sensibilidade: se os nove casos de fronteira que ficam na conta (326, 411, 431, 338, 381, 401, 445, 452, 462) também fossem para ⏸: 88 ÷ 449 = 19,6%. Esta proposta **não** os manda para ⏸ — na dúvida, ficam.

## ADIAR (proposta de ⏸)

311, 391, 397, 400, 412, 413, 420, 468

## Casos de fronteira, cada um com o motivo de ter ficado onde ficou

- **326, 411 e 431 — DONO, e o assunto é página.** Pela regra das páginas seriam ADIAR. Ficam porque cada um espera um ato ou uma escolha do dono (refazer o compartilhamento; escolher entre (a), (b) e (c); tirar o base64 ou dar o `force`). Se o dono disser «depois», os três vão para ⏸ sem perda.
- **338 — BLOQUEIO.** O pedido é prioridade e processo, mas é o único lugar onde dois P0 do parecer externo estão escritos: «prevencao de dois lideres escrevendo» e «resposta ambigua de commit com repeticao segura». Varri o PENDENCIAS e nenhum dos dois tem pedido próprio (o 281 fechou um caso de dois masters, não o requisito). Pôr o 338 em ⏸ tiraria os dois da conta sem ninguém ver. Saída limpa: abrir os dois P0 como pedidos próprios e então pôr o 338 em ⏸.
- **381, 401, 445, 452 e 462 — DEFEITO por dúvida.** 381: um dos dois caminhos viola a garantia, seja qual for a certa. 401: sobram 6 chamadas com a janela de corrida. 445: BAIXOS com vetor. 452: pânico deixa o par sem pulso. 462: o 497 mostra que o eco vai para o `acessos.log`.
- **397 e 468 — ADIAR, e o motivo vai escrito.** 397: o CONTRATO diz `PSCH` 7 (o código está em 10), mas a promessa G (ler tudo o que 1.y escreveu) não é desmentida; é número velho, não garantia. 468: o papel C julgou a troca de chave «ACEITAVEL enquanto pedido»; o risco é a chave perdida, que a parte (b) do próprio pedido alivia.

## Notas de QA (papel G), uma linha cada

- **Nenhuma catraca sobe** com esta reclassificação. Nenhuma lê a porcentagem, e o `conferidor_vermelhas` só procura o nome do teste no PENDENCIAS.
- **Guarda que não olha o campo novo:** o `conferidor_vermelhas.rs` (linha 70) confere só se o nome da prova vermelha aparece no PENDENCIAS, e não em que estado. Com o ⏸, uma prova `VERMELHA de propósito` pode ficar ligada a um pedido fora da conta e a catraca continua verde. Hoje existem **0** provas vermelhas na árvore (varri), então nada escapa agora; a porta está aberta para a próxima.
- **Ordem do dono não cumprida:** no 249, o dono mandou em 17/09 a sonda de disco a cada 5 minutos, e o `config.rs:1122` ainda tem `checar_segundos: 60`.
- **391 e 420 são o mesmo defeito da lei, com duas contagens diferentes** (11 de 140 e 10 de 141), e o 429 (⏸) diz que o 420 já nasceu seis sítios curto. É a lei que lista menos casos do que existem. O código está certo nas onze; o que está errado é o texto da lei.
- **422:** a catraca `alcancam-fsync-2` está verde enquanto três irmãs (`declarar_fk`, `excluir_fk`, `marcar_lgpd`) seguram a trava global. O mapeador é estático e não vê caminho condicional, e o próprio pedido diz que catraca verde não quer dizer pronto.

## Por classe

- **DEFEITO** (67): 229, 262, 268, 276, 284, 290, 293, 297, 300, 321, 322, 329, 344, 345, 352, 357, 359, 364, 365, 369, 381, 401, 416, 422, 424, 427, 428, 436, 437, 443, 444, 445, 452, 458, 462, 463, 464, 466, 467, 470, 472, 473, 486, 487, 490, 491, 492, 497, 498, 499, 502, 503, 504, 509, 510, 511, 513, 514, 515, 516, 517, 518, 520, 521, 522, 523, 524
- **DONO** (28): 175, 190, 197, 207, 238, 245, 249, 251, 255, 259, 263, 294, 309, 324, 325, 326, 330, 331, 333, 335, 339, 350, 411, 431, 454, 455, 495, 496
- **BLOQUEIO** (2): 338, 508
- **ADIAR** (8): 311, 391, 397, 400, 412, 413, 420, 468

## Tabela

| nº | estado | classe | motivo em uma linha (com a frase do item que decide) |
|---:|:---:|---|---|
| 175 | ☐ | DONO | espera decisão do dono: «DECISÃO do dono, 04/09/2026: MEDIR PRIMEIRO … só se escolhe depois do número»; medido zero sem base real, as duas saídas seguem «na mesa» |
| 190 | ◐ | DONO | «Ordem do dono, 05/09/2026» (bateria de todos os botões); falta: 119 botões sem prova (TETO_BOTAO_SEM_PROVA) |
| 197 | ◐ | DONO | «DECISAO DO DONO, 17/09/2026 05:38: FAZER, comecando pelos dois defeitos de desenho»; «O segundo ja tem tranca provada; o primeiro nao» (a regravação que tenta «PARA SEMPRE» sem sessão com a chave) |
| 207 | ◐ | DONO | «DECIDIDO pelo dono em 07/09/2026: construir, pela rota do CANAL ABERTO» + «DECISAO DO DONO 17/09: o ok significa APLIQUEI E GRAVEI»; escrita com quórum «NAO foi implementada» |
| 229 | ◐ | DEFEITO | dado errado: (a) «bidirecional na MESMA faixa perde linha» só ganhou detecção («o conserto pleno … fica») e (c) «reemissao de numeros que a replica atrasada nunca recebeu» segue; também DECISÃO DO DONO 17/09 (formato no PSCH v10) |
| 238 | ◐ | DONO | «DECISAO DO DONO, 17/09/2026 05:35: fazer a sonda viva quando o disco permitir, MEDINDO O DISCO ANTES»; falta a sonda viva OUT/INOUT contra phxsqld real |
| 245 | ◐ | DONO | falta só O2: «DECISAO DO DONO, 17/09/2026 05:41 … RECUSAR A DECLARACAO, nomeando quantas linhas violam» (evita «a tabela com duas verdades») |
| 249 | ◐ | DONO | falta o meio do SMS: «O MEIO do SMS continua parado, para parecer proprio»; e a ordem de 17/09 05:35 («intervalo da sonda 5 minutos») NÃO foi cumprida — conferido: `config.rs:1122` ainda tem `checar_segundos: 60` |
| 251 | ☐ | DONO | «Parado por decisão do dono: é formato, e formato se decide com ele antes de gravar» (AAD do selo × identidade sem domínio) |
| 255 | ☐ | DONO | «Parado por decisão do dono: reconstruir sozinho no arranque muda o tempo de subida» (.ndx sujo sem marca, silencioso para quem opera) |
| 259 | ◐ | DONO | «DECISAO DO DONO, 17/09/2026 05:35: pular a busca reversa da integridade quando NENHUMA tabela declara chave apontando para esta»; duas candidatas seguem paradas com número |
| 262 | ◐ | DEFEITO | garantia que não vale: parecer J «e DEFEITO, nao contrato» (10 a 0 nos quatro motores); etapa 1 só tornou a perda barulhenta — «a etapa 2 (o AFTER antes da marca) continua em 0%», a gravação do gatilho no COMMIT segue não acontecendo |
| 263 | ◐ | DONO | «DECISAO DO DONO, 17/09/2026 05:32: rodar o provador INTEIRO no fecho»; medido que não cabe no disco, falta a corrida em lotes por --so |
| 268 | ☐ | DEFEITO | segurança/garantia que não vale: «a lista cifra.tabelas declara mais do que o disco cumpre» — declarar não cifra o já gravado; é formato do .reg (papel C) que «entra cedo» |
| 276 | ☐ | DEFEITO | garantia que não vale (na dúvida fica): o catálogo publicado declara `token` «onde a operação lê token_remoto» — cliente que segue o catálogo manda o nome errado, e a régua de segredos fica «cega naquele parâmetro»; sem vazamento (token_remoto está em SEGREDOS, conferido) |
| 284 | ☐ | DEFEITO | segurança: «replicas_autorizadas: ["127.0.0.1"] autoriza todo cliente que chegar pelo proxy» — posicao/replicar/aplicar alcançáveis pelo /api |
| 290 | ☐ | DEFEITO | dado errado (e DECIDIDO PELO DONO): reaberto — o `inicio` do nó tem «zero chamadores de producao», «todo phxsqld roda com INICIO = 0», a colisão de faixa «continua 100%» (4 inserções podem virar 2 linhas) |
| 293 | ☐ | DEFEITO | dado errado (e DECIDIDO PELO DONO «recusar no motor agora — ESPERA ENGENHARIA»): réplica com cifra desligada «grava 63 bytes de texto cifrado como se fossem o conteudo, sem erro nenhum» |
| 294 | ☐ | DONO | «DECIDIDO PELO DONO 17/09 07:10 (manter a soma, medir por tabela AO LADO) — ESPERA ENGENHARIA»; o vetor por tabela no pulso/painel não existe |
| 297 | ☐ | DEFEITO | garantia que não vale (promessa ao cliente): «o INTEGRIDADE.md §3 promete fidelidade sem essa ressalva» — motivo/uuid/carimbo/usuário do .trash/.reason divergem na réplica |
| 300 | ☐ | DEFEITO | replicação que perde dado: escrita local na réplica faz «o evento N do source nunca e aplicado … perda silenciosa de atualizacao» (+ órfã permanente sem contador) |
| 309 | ☐ | DONO | espera decisão: «Decisão do dono: se vale fechar também os buracos históricos (via b), ou se a via (a) … é suficiente» |
| 311 | ☐ | ADIAR | cosmético de tela: o passo 3 do assistente mostra «PhxSql» sem versão/ms porque `sondar_origem` não os devolve — campo vazio, nenhum dado errado nem garantia |
| 321 | ☐ | DEFEITO | teste que floca: `continuidade-da-replica.rs:132` «cai com a maquina carregada» (3 ocorrências) e «BLOQUEIA O GERADOR DO CAPABILITIES.json» |
| 322 | ☐ | DEFEITO | garantia que não vale (na dúvida fica; «NÃO MEDIDO» pelo soquete): a reserva do BULKINSERT «se contorna pelo lado B de uma junção», furando a promessa de carga.rs:8-9 |
| 324 | ☐ | DONO | «DECISÃO DO DONO» / «Pedido do dono em 17/09/2026» — adiar o .ndx em thread é «troca de SLA», três saídas na mesa |
| 325 | ☐ | DONO | «PEDIDO DO DONO — 20 caixas de supermercado e 1 servidor»; três das seis pernas não existem |
| 326 | ☐ | DONO | «Fica parado em acao do dono» (refazer o compartilhamento da página fixada; «NAO e consertavel por nos»); seria ADIAR pela regra de página se não esperasse o dono |
| 329 | ☐ | DEFEITO | replicação que perde dado (na topologia das 20 caixas, perna do 325): hash_id de 16 bits conferido por PAR — «o servidor central suprime os eventos de um caixa … e nada grita» |
| 330 | ☐ | DONO | perna do PEDIDO DO DONO 325: «Por que isso mata a promessa do dono … "ao subir e imediata"»; mapa de toques «sem teto»; «Nao implementar antes de medir» |
| 331 | ☐ | DONO | perna do PEDIDO DO DONO 325: «O pedido das 20 caixas (325) e esse alguem» — chave composta recusada no bidirecional (recusa com motivo, não perde dado) |
| 333 | ☐ | DONO | «PEDIDO DO DONO — chat estilo WhatsApp …»; «DUAS DECISOES SAO DO DONO» |
| 335 | ◐ | DONO | falta a metade 2: «a catraca que reprova a publicacao quando a prosa contradiz a celula — continua ABERTA, na mesa do dono» (A × B) |
| 338 | ☐ | BLOQUEIO | fronteira: é prioridade/processo, mas lista «P0, antes de producao critica» dos quais «prevencao de dois lideres escrevendo» e «resposta ambigua de commit com repeticao segura» NÃO têm pedido próprio (varrido) — ⏸ aqui os tiraria da conta calados; saída limpa: abrir os dois P0 órfãos como pedidos e então ⏸ este |
| 339 | ◐ | DONO | faltam (b) e (c): «(c) … pedem validacao juridica, e isso e decisao do dono»; (b) ainda com a §11.3 do SEGURANCA.md que «aconselha tirar o indice» e deixa vazar pelo .fts |
| 344 | ☐ | DEFEITO | dado errado: réplica sem cofre «GRAVOU 63 bytes que NAO sao o conteudo -- e o texto cifrado, sem erro nenhum» (+ erro que acusa adulteração inexistente) |
| 345 | ☐ | DEFEITO | segurança: «644 clientes.fts» com dado em claro «inclusive com a cifra LIGADA … legivel para todo usuario da maquina» |
| 350 | ☐ | DONO | «NOMEADO, nao consertado, porque pode ser DECISAO … a palavra e do dono» (jobs devolve pedido salvo com valor de coluna marcada) |
| 352 | ☐ | DEFEITO | teste que floca: corrida com o parceiro de replicação («flaky nao e causa raiz»), duas ocorrências em famílias diferentes |
| 357 | ☐ | DEFEITO | segurança/garantia que não vale (SEC MÉDIO-ALTO): «a trilha passou a gravar, e grava o claro. A lei esta no repositorio e o codigo a desmente» (§11.7); volume .lgpd anterior à cifra segue recebendo claro |
| 359 | ☐ | DEFEITO | segurança: parecer do DBA «E' vazamento vivo pelo protocolo» — `visoes` (Ler, PorColuna::Nenhum) devolve o literal de coluna negada; comentário no corpo da visão leva senha a qualquer leitor |
| 364 | ☐ | DEFEITO | garantia que não vale: tabela que perdeu a declaração fica com .fts órfão, «o reconstruir_fts devolve Ok(0) anunciando sucesso», e não há caminho de volta sem recriar a tabela |
| 365 | ☐ | DEFEITO | segurança: o Profiler publica no perfil.txt «o INSERT inteiro, com o valor» de tabela cifrada («furo de 05/09») |
| 369 | ☐ | DEFEITO | segurança: `op_esquema` (classe Estrutura, sem peneira) publica `baldes[].registros`, «agregado do DADO viajando na unica resposta que ninguem filtra» |
| 381 | ☐ | DEFEITO | fronteira — garantia que não vale: o mesmo `atualizar` sobre .memo corrompido recusa sem `softdeleted` e «com "softdeleted": false grava normalmente»; qualquer que seja a resposta certa, um dos dois caminhos a viola |
| 391 | ☐ | ADIAR | lei do CLAUDE.md com contagem velha (regra do pedido): «O código está certo nas onze; a lei é que está errada»; trocar o número é do dono, e fica igual parado ou ⏸ |
| 397 | ☐ | ADIAR | gerador de documento: «o pedido e o gerador» das versões de arquivo; o CONTRATO diz PSCH 7 (é 10), mas a promessa G — «lê tudo que 1.y escreveu» — não é desmentida (fronteira: é texto de cliente) |
| 400 | ☐ | ADIAR | processo/gerador: versão digitada em CAPABILITIES.json parado, tag Docker de bancada e distância de commits no CHANGELOG — nenhum toca o produto |
| 401 | ◐ | DEFEITO | teste que floca (resto): 24 de 27 fechados por construção, mas «3 arquivos ficam com porta_livre() … 6 chamadas» com a janela de corrida aberta; fronteira — nenhuma falha nova registrada nesses 3 |
| 411 | ☐ | DONO | «Tres saidas, para o dono escolher» e «A decisao do dono deixou de ser teorica e passou a bloquear» (o dossiê de 2,64 MiB não se republica); é página, e seria ADIAR se não esperasse o dono |
| 412 | ☐ | ADIAR | portão dos geradores: «Isto nao e defeito de gerador: e impossibilidade logica» — falta o modo `sem-ultimo-commit`; processo interno |
| 413 | ☐ | ADIAR | comentário de `log.rs` × código e reserva do REPLICACAO.md gasta duas vezes; «O docs/FORMATO.md esta CERTO»; documento interno, sem promessa ao cliente |
| 416 | ☐ | DEFEITO | replicação que diverge dado: «O DELETE replicado tem o mesmo defeito do 405» (sobreposição calada entre origens) «e NAO se conserta do mesmo jeito» — pede formato, «cedo» |
| 420 | ☐ | ADIAR | lei do CLAUDE.md com contagem velha (regra do pedido): «7 das 116» contra 10 de 141; «Alterar o CLAUDE.md e decisao do dono»; mesmo assunto do 391 (duplicado de fato) |
| 422 | ◐ | DEFEITO | travamento: a migração saiu da trava em 2 de 5 irmãs; «Ficaram declarar_fk, excluir_fk e … marcar_lgpd» segurando a trava global, e «a catraca esta VERDE com as tres ainda segurando a trava» |
| 424 | ☐ | DEFEITO | segurança: a combinação proibida do ledger sem sal (SEC ALTO, 355) «CRESCE por replica, para nos que nunca a tiveram»; «documento que mente por omissao mente igual» |
| 427 | ☐ | DEFEITO | garantia que não vale: volume nascido na FASE A fura o retrato e a tabela passa a recusar abrir com `Corrompido`; no Windows (alvo de compilação) «o furo mora exatamente onde ninguem mediu» |
| 428 | ☐ | DEFEITO | segurança e dado: «em NTFS e APFS … inserir em "Clientes" grava no volume vivo durante a FASE A» (escrita perdida) + o erro publica o caminho absoluto do servidor |
| 431 | ☐ | DONO | falta (b): «tirar as capturas do corpo … continua sendo decisao do dono», e o `force` da republicação «so com ordem explicita do dono»; a metade (a), guarda de tamanho no gerador, seria ADIAR sozinha |
| 436 | ◐ | DEFEITO | segurança: M1–M3 feitos, «M4, M5 e B1-B6 continuam abertos» — M5: `perguntar()` «nunca consulta oficial(c)» e o endpoint na própria origem manda o x-api-key ao PhxSql; M4: escorregão de config «vira failover» |
| 437 | ☐ | DEFEITO | teste que floca: `as_threads_do_so_se_medem…` e `o_escritor_nao_passa_fome_entre_leitores` caem sob disputa de CPU entre processos («Dois pontos nao sao taxa») |
| 443 | ☐ | DEFEITO | segurança (MÉDIO, só leitura): o DbLink MySQL faz `Vec::with_capacity(quantas)` com `quantas` do par, «ate 2^64 … aborto do processo inteiro» |
| 444 | ☐ | DEFEITO | segurança (e DECIDIDO PELO DONO, espera engenharia): «Anonimo enche o disco pelo acessos.log: 266 B de log por 2 B recebidos … Sem credencial nenhuma» |
| 445 | ☐ | DEFEITO | segurança BAIXA, sem prova de inerte no item: B2 o atacante escolhe o momento e o journald «pode descartar o REBAIXANDO»; B4 `tem_pino` sai na op `config`; B6 a porta web recusa sem linha no acessos.log (fronteira) |
| 452 | ☐ | DEFEITO | travamento latente no cluster: thread de pulso que morre por pânico fica marcada e «o par nunca mais e pulsado» (1 pulso em 4,5 s medido com o gatilho reposto); fronteira — o gatilho conhecido fechou no 446 |
| 454 | ☐ | DONO | «PEDIDO DO DONO, 24/09/2026 -- PhxZip e PRODUTO» |
| 455 | ◐ | DONO | «PEDIDO DO DONO, 24/09/2026 -- o pacote so do PhxZip completo»; falta servidor web, PhxZipCmd, manual e pacote |
| 458 | ☐ | DEFEITO | travamento: «um panico com transacoes na mao mata toda transacao de toda conexao ate reiniciar» |
| 462 | ☐ | DEFEITO | segurança (fronteira): «A recusa da expressao ecoa o texto INTEIRO do onde»; e o 497 prova o sumidouro — o erro vai cru ao acessos.log (`servidor.rs:9509`), então o literal do onde (CPF etc.) chega ao log; sobrepõe o «irmão» do 497 |
| 463 | ☐ | DEFEITO | travamento: o relé SMTP que mande `250-` sem fim «segura a thread de aviso indefinidamente» (o carteiro dos avisos de disco) |
| 464 | ☐ | DEFEITO | segurança/privacidade: «o curto (ate 48 bytes -- um CPF, um e-mail) sai inteiro na mensagem de erro», família do 357 |
| 466 | ☐ | DEFEITO | travamento: «Cadastro do DbLink ilegivel derruba o MOTOR inteiro … o phxsqld nao sobe» |
| 467 | ☐ | DEFEITO | garantia que não vale: sem `fsync` do diretório depois do `rename`, «o claro volta depois de a resposta ter dito ligacoes_cifradas_agora: N» |
| 468 | ☐ | ADIAR | capacidade nova: rotação da chave mestra do DbLink; o papel C a julgou «ACEITAVEL enquanto pedido (nao e meia funcionalidade…)»; fronteira — com a chave PERDIDA «a tela nao tem saida» |
| 470 | ☐ | DEFEITO | segurança (exige sessão de admin, e a regra diz que conta): «Trocar o host … HERDA a senha: sessao de administrador roubada leva a credencial para fora» |
| 472 | ☐ | DEFEITO | dado errado: pânico entre o .reg e o indexar_texto deixa o .fts atrás «e com a marca limpa: a busca de texto perde a linha sem avisar» |
| 473 | ☐ | DEFEITO | garantia que não vale: a recusa diz «o arquivo esta SAO: nao repare nada» no caso em que reparar é o conserto (imperativo desmentido — o critério de defeito da lei do erro cru) |
| 486 | ☐ | DEFEITO | dado errado/regra primordial: «a linha fica sem trilha e a filha fica orfa» quando o .lgpd falha («Fere a regra primordial da integridade») |
| 487 | ☐ | DEFEITO | dado errado: expurgo entre páginas faz «k registros nao aparecem na exportacao, sem erro» — o auditor pula registros calado |
| 490 | ☐ | DEFEITO | dado errado/regra primordial: pânico na cascata deixa «filhas … apontando para a chave velha … sem recusa nenhuma»; ficam falsas MANUAL e ACID.md («nunca silenciosa») |
| 491 | ☐ | DEFEITO | regra primordial: «excluir de vez o chefe que tem subordinado responde Ok, e o subordinado fica orfao» (FK para a própria tabela) |
| 492 | ☐ | DEFEITO | dado errado: «alterar uma linha excluida suave a RESSUSCITA» dentro da transação |
| 495 | ☐ | DONO | «Ordem do dono, 24/09/2026» (IA analisa crime de cibersegurança); J decidiu o desenho, «Sobe ao dono uma decisao de produto» |
| 496 | ☐ | DONO | «Ordem do dono, 24/09/2026» (IA com cognição de DBA sênior; primeira fatia `esgota_em`) |
| 497 | ☐ | DEFEITO | segurança: «O acessos.log grava o texto do erro sem redigir … com o dado pessoal dentro»; «Entra na conta porque e vazamento de dado» |
| 498 | ☐ | DEFEITO | dado errado/regra primordial: «a linha fica gravada com o valor novo» sem diário; disco cheio real deu 294 linhas sem evento e «Orfa provada … fere a regra primordial» |
| 499 | ☐ | DEFEITO | garantia que não vale, pela frase do revisor: «Nao medido … na duvida, fica na conta» (réplica/somente-leitura não esvaziam o próprio .trash) |
| 502 | ☐ | DEFEITO | travamento: job/backup em pânico com a trava «derruba o servidor A CADA ARRANQUE … ate o systemd desistir» |
| 503 | ☐ | DEFEITO | dado errado/durabilidade: seguem (1) e (3) — «um commit confirmado some calado com EIO permanente» e o COMMIT responde «abortada» e o arranque aplica «por cima de gravacao mais nova» |
| 504 | ☐ | DEFEITO | segurança: core dump com a chave derivada do cofre; «Seguranca, e na duvida fica na conta»; «Deve entrar na mesma versão que leva o abort do 509» |
| 508 | ☐ | BLOQUEIO | formato que tem de entrar antes da entrega: «trocar o _ … antes de haver dado em producao» — «a petrea "mudanca de formato entra cedo" decide o momento»; hoje a recusa do 368 guarda, sem defeito ativo |
| 509 | ☐ | DEFEITO | dado errado/durabilidade: «fsync que falhou e repetido responde Ok … o dado some depois da remontagem»; o resto aberto: o mesmo boot confia no cache e apagaria a marca |
| 510 | ☐ | DEFEITO | garantia que não vale: «Backup agendado que falha nao avisa ninguem … O dono so descobre que nao tem backup no dia de restaurar»; o 496 diz que «os defeitos ativos viraram os pedidos 509-513» |
| 511 | ☐ | DEFEITO | garantia que não vale: o `rowstamp` empurrado por evento replicado «da a volta: release devolve u64::MAX, 0, 1» — «Fere a regra primordial vista pelo tempo»; está na lista «defeitos ativos» 509-513 do 496 |
| 513 | ☐ | DEFEITO | travamento (hipótese a medir; na dúvida fica): «O backup segura a trava global de escrita durante a copia inteira: 100 GB sao 50 a 64 minutos»; listado entre os «defeitos ativos» 509-513 |
| 514 | ☐ | DEFEITO | regra primordial: «A FK e conferida ANTES do DEFAULT e da coluna calculada: filha orfa gravada» (ALTO, medido) |
| 515 | ☐ | DEFEITO | dado errado: o elo planejado pelo disco sobrescreve a lista e até «RESSUSCITA a filha» (ALTO, medido) |
| 516 | ☐ | DEFEITO | garantia que não vale: «leitura repetivel le 5 e depois 6» — «A garantia do nivel declarado nao vale para a filha alcancada pela cascata» |
| 517 | ☐ | DEFEITO | dado errado (só leitura): no bidirecional «NULL casa com NULL e sobrescreve a linha de outro» |
| 518 | ☐ | DEFEITO | dado errado na saída do produto: o `diff` «some com linhas do relatorio … o relatorio mente por omissao» |
| 520 | ☐ | DEFEITO | segurança: enumeração de usuários pelo relógio (3,10 s × 0,03 s), e o comentário afirma o contrário; «Seguranca: entra na conta» |
| 521 | ☐ | DEFEITO | segurança (negação de serviço): senha de 1 MiB passou de 300 s «sem resposta e sem linha no log»; «entra na conta» |
| 522 | ☐ | DEFEITO | garantia que não vale/durabilidade: o `fechar` baixa o byte 52 sem fsync e o .ndx fica «marcado LIMPO sobre pagina que nao chegou ao disco»; «Formato, entra cedo»; pré-requisito do 509 |
| 523 | ☐ | DEFEITO | garantia que não vale (biblioteca/FFI): a recusa do 509 é «por GRAFIA do caminho» — pelo symlink o mesmo diretório «sincroniza Ok depois de um fsync recusado» |
| 524 | ☐ | DEFEITO | garantia que não vale: «backup.rs tem 0 sync_all» e o backup responde «concluído» sem estar no disco; o comentário `volume.rs:1347` diz que faz |
