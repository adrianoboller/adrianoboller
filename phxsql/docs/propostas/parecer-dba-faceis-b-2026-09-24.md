# Parecer do papel C — lote «fáceis B» (345, 473, 518, 524), 24/09/2026

**Veredito: LIBERA COM CONDIÇÃO.** 345, 473 e 518 não derrubam garantia de dado. O 524 **troca
um defeito por outro**: o backup passa a ter `fsync`, e ganha um `abort` do servidor inteiro
quando o disco de DESTINO recusa. C1–C3 antes do commit. Se C1 não couber agora, o 524 sai do
lote e os outros três entram com C3.

Base lida: worktree `agent-a7a5654450e5abfbc`, base `49b5426`, `faceisB.patch` (12 arquivos).
**Medido:** catraca do `mapa-da-trava.py --catraca` no worktree, `alcancam-fsync-2` = 23 (teto 23);
`umask` do ambiente = 0022. **Não medido:** não compilei nem rodei a suíte (sem `target` no
worktree, disco apertado). Todo o resto é leitura, com arquivo e linha.

## Condições

- **C1 — o `fsync` do backup não pode derrubar o servidor.** `backup::sincronizar_arquivo` chama
  `sincronia::sync_all`, que na recusa chama o gancho do processo. No servidor o gancho é
  `fsync_recusado_derruba_o_processo` (`servidor.rs:1378`, e `:27235-27246` faz o
  `std::process::abort()`). Um disco de backup (USB, NFS, provisionamento fino) que recuse vira
  produção fora do ar. O agendado repete isso a cada rodada, e o diagnóstico manda
  «remonte o volume ou reinicie a máquina», apontando para o disco errado. O teste novo
  (`fsync_recusado_na_copia_vira_erro`) roda no `phxsql-store`, que não tem gancho, e por isso não vê
  isto. O certo é erro ao cliente, backup não concluído e servidor de pé. A prova tem de passar
  **pelo servidor** (`op_backup` com `falha_de_teste` armada no destino, e o pedido seguinte
  respondido).
- **C2 — o manifesto tem de ser ESCRITO depois do `fsync` das cópias, não só sincronizado por
  último.** Hoje ele é escrito sob a trava, antes do primeiro `fsync` (`backup.rs:409`). Uma recusa
  no meio deixa no destino uma pasta com o `backup.json` completo. O `op_backups` lista como backup
  «pasta com manifesto dentro» e todo `.zip` (`servidor.rs:21684`). No mesmo boot, o `verificar` lê
  do cache e aprova: é a mentira do cache que o 509 P1 já mediu. A frase de `backup.rs:88-93`,
  «um backup que parou no meio nunca chega a se dizer completo», é falsa hoje. O conteúdo do
  manifesto está em memória (`Relatorio`) e não precisa da trava. No ZIP o equivalente é nome
  temporário até o `fsync` e o nome final depois; a durabilidade do `rename` fica com o 467.
- **C3 — textos que afirmam mais do que o código faz.**
  (a) Os três testes do 473 perderam os dentes do 176. As novas asserções proíbem só «esta sao» e
  «nao repare nada». O erro cru (`ndx.rs:1323-1326`) diz «ficou para tras numa queda … reconstrua
  com `reparar indice`» e não contém nenhuma das duas. Se o `({e})` voltar colado à mensagem nova
  (o defeito do 176), os três passam. O «Prova real» escrito acima deles afirma o contrário.
  Conserto: voltar `!contains("ficou para tras") && !contains("reconstrua")`, que estão ausentes da
  mensagem nova, e corrigir os três comentários.
  (b) `util.rs:82` diz «todo arquivo que nasce com essa classe de dado chama esta função». É falso:
  `.reg`, `.ndx`, `.log` e `.trash` carregam o mesmo dado e nascem 0644, como o próprio 345 registra.
- **C4 — registro ao fechar.** 524 fica **◐**: falta o `fsync` de diretório que o texto do próprio
  pedido exige. 513 fica **☐**: a cópia continua inteira sob a trava, e a catraca apenas não subiu.
  345 fica **◐** (ver §2). 473 e 518 fecham **☑**.

## 1. 524 — `fsync` fora da trava

- **Janela entre soltar a trava e o `fsync`:** não existe para o dado. A cópia é arquivo NOVO no
  destino (`File::create`, `backup.rs:69`), e o destino dentro da raiz é recusado
  (`backup.rs:381-386`). Escrita posterior toca a raiz, nunca a cópia, e a cópia continua inteira
  sob `travar_dados()`, então o retrato continua consistente. A única concorrência é outro backup
  no MESMO destino (o `File::create` trunca), e ela já existia antes do lote.
- **«Concluído» só depois do `fsync`:** sim. `sincronizar` roda antes da resposta
  (`servidor.rs:21600`), antes do `anotar` e da rotação no agendado (`:5685`, `:5704`) e no CLI
  (`main.rs:672`, `:693`). O manifesto é o último da lista (`backup.rs:413`), mas só na ordem do
  `fsync` (C2).
- **Recusa vira erro:** no CLI e na biblioteca, sim. No servidor vira `abort` (C1).
- **`fsync` do diretório:** nenhum, nem no destino, nem nas subpastas criadas, nem na pasta dos zips.
  A rotação (`limpar_backups_velhos`, `:5704`) apaga o backup velho confiando numa entrada de
  diretório nova sem `fsync`, que é justamente o cenário que motivou o 524. Isso vai para o 467,
  com `backup.rs` e a rotação nomeados, e usa o ajudante único de lá, não um segundo.
- **Contra o 509:** o lote herda o certo (`conferir` e `recusar`: a repetição não responde Ok),
  herda o errado (o `abort`, C1) e herda a metade aberta do 509 (o cache do mesmo boot, C2).
  O `abort` existe porque o cache do diretório DE DADOS pode mentir. O `sincronia.rs:66-68` já
  deixa de fora o que não é dado de tabela. O erro de *writeback* é por inode: se o disco for o
  mesmo, o `fsync` do próprio dado cai e derruba sozinho.
- **Reabrir para sincronizar** (`backup.rs:82`): fechar e reabrir perde o erro de *writeback* se o
  inode sair da memória no intervalo (o caso *fsyncgate*). Num backup grande, o primeiro arquivo é
  reaberto minutos depois. Não medido: vira N3.

## 2. 345 — permissão

- O `.fts` nasce 0600 em `criar` e em `recriar` (`fts.rs:163`), pelo mesmo motor da trilha. Certo.
- **A cópia do backup NÃO preserva, e este lote não fecha o achado.** `escrever_sem_sync` usa
  `File::create`, que cria com `0666 & ~umask`; com o `umask` medido de 0022, o arquivo sai 0644.
  O ZIP, que carrega `.lgpd` e `.fts`, também sai 0644, e a pasta 0755. Na volta,
  `restaurar.rs:560` recria o arquivo com `File::create`, também 0644. O 345 se desfaz no destino
  e na restauração: vira N1.
- O `.fts` criado antes do lote continua 0644. Só `criar` e `recriar` apertam, igual ao `.lgpd`.
  Sem dado em produção, fica como nota.
- O teste depende do `umask`: com `umask 077` ele passa mesmo com o defeito reposto. Isso vale para o
  papel F: comparar com um arquivo de controle criado sem aperto.

## 3. 473 e 518 — o texto diz o que o código faz?

- **473: sim.** Quem confere vê só o byte 52, por um segundo descritor
  (`indice_precisa_reconstruir`, `table.rs:7029`). O estado `escrita_interrompida` do 456 mora no
  punho que escreveu, não no que confere. «Ler o estado», como o pedido pedia, não existe para esse
  leitor, e nomear os dois casos é o honesto. Isso tem de ficar escrito ao fechar. A recusa continua
  `Err(Integridade)`, e nenhum ramo passou a aceitar. Ressalva: C3(a).
- **518: sim, e o comportamento converge.** NULL na chave nunca casa, como no `FULL JOIN ON a.k=b.k`
  dos quatro e no UNIQUE com vários NULL. A conta fecha: cada linha cai em um balde só. O limite
  está na resposta, que publica só a chave (`servidor.rs:~23654`): N linhas com chave NULL viram N
  `[null]` indistinguíveis, e «só em A» aparece para uma linha cujo gêmeo está em B. É honesto na
  contagem e pobre na identificação: vira N4.

## 4. Garantia de dado

Nenhuma cai:

- **Ordem de digitação:** nenhum caminho de escrita do `.reg` ou do `Volumes` muda. Em `volume.rs`
  só muda a constante do teste (backup 3→2, catraca desceu).
- **`rowstamp`:** intocado.
- **FK:** só o texto muda. `pendente` e o `Err` são os mesmos.
- **Byte 52:** intocado.
- **Retrato do backup:** a cópia continua sob a trava.

O que cai é a **disponibilidade** (C1) e a **honestidade do artefato de backup** (C2).

## Pedidos novos (proposta)

- **N1 ☐ (segurança):** cópia, ZIP e restauração regravam 0644 e desfazem o 345 no destino e na
  volta. O conserto vem do mesmo motor (criar em 0600 ou `apertar_permissao`) nos três caminhos, com
  prova contra o SO.
- **N2 ☐:** alcance do 345. `.reg`, `.ndx`, `.log` e `.trash` nascem 0644. Hipótese para o J, **não
  conferida aqui**: PG 0600, MySQL/MariaDB sem acesso para «outros» (UMASK), SQLite 0644. Se os
  três maduros convergirem em «outros não leem», entra sem pergunta, para todo arquivo de dado.
- **N3 ☐ a medir:** `fsync` após fechar e reabrir pode responder Ok sem o dado, com o inode
  despejado. Medir na `bancada/catastrofes/`. Direção: `syncfs` do destino ao fim, por
  `extern "C"`, sem crate.
- **N4 ⏸:** `diff`: balde `sem_chave` (ou a linha inteira) para chave com NULL, em vez de `[null]`
  repetido nas listas «só em».
- **Para o J, junto do C1:** conferir se algum dos três maduros derruba o servidor por falha no
  destino de backup. Hipótese: não, porque `pg_basebackup` e `mariabackup` são clientes; falta
  conferir o alvo `server` do PG 15+.
