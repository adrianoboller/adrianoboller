# Parecer do DBA (papel C): pedido 368, segunda entrega — 24/09/2026

**Veredito: BLOQUEIA.** Dois bloqueantes, os dois baratos e nenhum muda byte em disco. O resto se sustenta: C1, C2, A5 e o formato B estão feitos e conferidos no código.

Worktree revisado: `.claude/worktrees/agent-a88017bcd787ed40c`, diff contra `82a17ef`.
Provas de leitura: programa externo em scratch (`dba-368b/prova`) que liga no `phxsql-store` do worktree. Build de depuração, Linux, ext4.

**Testes da frente, rodados no worktree:**

- no `phxsql-store`: filtro `trilha` da lib **32/32**, `--test cifra-dos-diarios` **16/16** e `--test trilha-lgpd` **22/22**, todos verdes;
- os testes do servidor (`testes_expurgo_da_trilha`) **não rodaram** nesta revisão.

## A) O que foi conferido e se sustenta

| item | onde | estado |
|---|---|---|
| C1: bit 0 do byte 9, tipo 4 | `motivo.rs` (`FLAG_EXPURGO_DA_TRILHA`, `buf[9] = self.flags`, `flags: src[9]`); FORMATO §6 | ok. Só o byte 9 mudou. O registro do `.lgpd` também não mudou: o CRC só virou função única (`crc_do_registro`) |
| C2: fora do `OPS_ESCRITA` | `servidor.rs:157-166`; teste `o_expurgo_pede_administrar_e_roda_no_servidor_somente_leitura` | ok. O `.reason` não entra na posição da réplica (a réplica anda pelo `.log`), então o rastro local não a desloca |
| A5: lista parcial no erro | `trilha.rs:1553-1569` | ok |
| A6: intervalo por `Instant` | `servidor.rs` (`subir_retencao_da_trilha`) | ok na leitura. Não há prova de vermelho → ⏸ |
| Três fases, trava do expurgo, bilhete | `servidor.rs:20555-20600`, `trilha.rs:1525-1574` | ok. Fechar por tamanho entre as fases só aumenta o ativo, e o plano fica abaixo dele |
| Formato B: nomes, rename, numeração sem teto | `volume.rs:462-483`, `trilha.rs:617-967` | ok, **fora do B1** |
| Migração sem reescrita: arquivo único, `_001.._K`, `_0001` | fixtures `trilha-antes-do-b/`, três testes | ok |
| Queda entre o rename e o nascimento | teste `a_queda_entre_o_rename_e_o_nascimento_cai_na_mesma_regra` | ok (ver ⏸ P3) |
| Cifra: fechar é rename, e decifra pelo nome novo | `cifra-dos-diarios.rs` | ok |
| DROP, RENAME e backup alcançam os fechados | `catalogo.rs` `pertence` (sufixo de dígitos); `backup::listar` pela listagem do diretório | ok |
| Ordem de digitação | nada toca no `.reg` | ok |

**O que um SIGKILL deixa em cada passo:**

- **Fechar:**
  - antes do `rename`: nada muda;
  - entre o `rename` e o `create_new`: só `_N`, e a listagem dá N+1 (testado);
  - entre o `create_new` e o `pwrite` do cabeçalho: **ativo de 0 byte**, e a tabela fica trancada (B2). A janela no SIGKILL é de dois syscalls;
  - depois do `pwrite`: ok.
- **Expurgo:**
  - antes do selo: nada sai;
  - entre o selo e o primeiro `unlink`: fica rastro sem apagamento, que já está documentado;
  - no meio dos `unlink`: sai um prefixo, e o erro lista o que saiu.
- **Sem fsync do diretório (467):** nesta frente a perda vai para o lado seguro. O `unlink` perdido faz o volume voltar e a passada seguinte o refaz. O `rename` perdido leva junto o `create_new`, porque o diário do ext4 é ordenado. Não abre item novo; fica no 467.

## B) Bloqueantes

### B1 — o expurgo de `x` apaga a trilha ATIVA da tabela `x_001`

**A garantia que cai:** «o ativo nunca sai». Cai de uma tabela para a outra, sem rastro no `.reason` da vítima.

**Onde:**

- `trilha.rs:707-735` (`maior_fechado_no_disco` adota qualquer `x_<dígitos>.lgpd`);
- `volume.rs` `existentes()` (desce do ativo e aceita o `x_001.lgpd` alheio);
- `trilha.rs:782-791`: a conferência de cabeçalho **não protege** quando o número coincide.

**Medido (P2a):**

- `x_001` com trilha (ativo volume 1, em `x_001.lgpd`), e a tabela `x` sem paginação.
- `x` nasce no volume **2**: a listagem achou o `x_001.lgpd`.
- A trilha de `x` lê **1** registro de `x_001`.
- O expurgo de `x` derrubou `[1..12]`: o `x_001.lgpd` sumiu, e a trilha de `x_001` ficou com **0** registros.
- Na passada diária com prazo, o caso vira o inverso. O volume de `x_001` com registro recente faz de `x` uma «fronteira» para sempre, e `x` retém dado vencido. Isto saiu da leitura do código, **não foi medido**.

**Por que é do 368:**

- **Antes do B**, a tabela **sem paginação** nunca gerava `_NNN.lgpd`. O par `x`/`x_001` só colidia no catálogo.
- **O catálogo já está quebrado para esse nome** (P2c, anterior ao 368):
  - `todas_as_tabelas` = `["x"]`;
  - `excluir_tabela("x")` apagou **16 arquivos**, 8 deles de `x_001`.
- **O B leva esse defeito** da operação rara (DROP) para a **rotineira**: o relógio diário, ligado de fábrica em 5 anos, e o `expurgar_trilha` do administrador com `ate` = agora.

**O texto que afirma o contrário, e está errado:**

- `FORMATO.md:1731` diz: «impede ler, como volume desta trilha, o `.lgpd` de outra tabela cujo nome termina em `_NNN`»;
- o comentário em `trilha.rs:782-784` repete a mesma coisa.

A medição acima desmente os dois.

**O conserto está na declaração.** Isso é pétrea: a tabela nasce uma vez e grava um milhão de vezes.

- `criar_tabela` (`catalogo.rs:599`) e `duplicar_tabela` (`:949`) recusam o nome que `nome_da_tabela` não lê de volta como ele mesmo.
- É a mesma pergunta que o `renomear_tabela` já faz em `catalogo.rs:868`. As três portas devem chamar uma função só, porque a lei manda que função e comando venham do mesmo motor.
- O par de testes tem de cobrir os dois lados: `x_001` recusado, e `x_historico` aceito.

**Impacto de migração:** nenhum. Não muda byte nenhum, e não há dado em produção. A tabela `_NNN` que já existe continua abrindo, com os defeitos de catálogo que já tinha.

### B2 — o ativo novo não vai ao disco, e ativo curto tranca a tabela inteira

**A garantia que cai:** a tabela abrir depois de uma queda. Hoje ela trava.

**Medido (P1):**

- Depois de `sincronizar()`, `familias_devendo_em` = **0**.
- Depois de `fechar_volume_da_trilha(true)`, `familias_devendo_em` = **1**.
- Com o `clientes.lgpd` de 0 byte, `Table::abrir` falha com **`[SP000010] erro de E/S: failed to fill whole buffer`**.
- O arquivo de 0 byte foi **simulado com `set_len(0)`**. A queda de energia que o produz é o caso conhecido do delalloc do ext4, e **não foi reproduzida contra o sistema operacional** nesta revisão. O que está medido é o efeito, e não a probabilidade.
- É a tabela inteira que não abre, leitura inclusive, até alguém apagar o arquivo à mão.

**Onde:**

- `servidor.rs:20575` (fase 1): fecha e faz nascer o ativo, e nem o retorno cedo (sem volume a derrubar) nem a fase 2 (`servidor.rs:20588`, que sela **só** o `.reason`) levam a trilha ao disco. O irmão `op_esvaziar_lixeira` faz `t.sincronizar()` (`servidor.rs:20416`).
- `trilha.rs:678`: a abertura não sabe o que é um nascimento interrompido.

**Por que é do 368:**

- A passada diária fecha o ativo das tabelas **paradas**, e ninguém escreve nelas depois. A janela é o writeback do núcleo, de uns 30 s, e alcança todas as tabelas que fecharam na mesma passada.
- O fechamento por tamanho a cada 64 MiB também acontece numa **leitura**: o registro de acesso que não cabe. Leitura não passa por `gravar_de_verdade`: são 9 chamadas, todas em escrita, reparo ou reindexação, e nenhuma numa leitura.

**Conserto preferido, sem `fsync` sob a trava (não mexe na `alcancam-fsync-2`):**

- Em `TrilhaFile::abrir`, o ativo **menor que o cabeçalho** é nascimento interrompido. Por construção não tem registro.
- Ele cai na mesma regra da queda entre o `rename` e o nascimento: o número sai da listagem (maior fechado + 1), que dá o mesmo N.
- Um conserto cobre os quatro caminhos: o relógio, o tamanho na escrita, o tamanho na leitura e o primeiro evento.
- A prova é o P1 como teste, falhando com a regra retirada.

**Conserto mínimo, se o B preferir:** o selo da fase 2 leva também a trilha, pelo número e com `Volumes::sincronizar_volume`, fora da trava, sempre que `fechou.is_some()`, inclusive no retorno cedo. Só fecha o caminho do relógio.

**Impacto de migração:** nenhum. Não muda byte nenhum.

## C) O que foi declarado fora do escopo, julgado

| item | é defeito ativo? | estado |
|---|---|---|
| nome de tabela terminando em `_NNN` | **sim** (P2a, P2c) | **entra agora como B1** |
| teto do `.log` | **sim, anterior ao 368.** P5: com `max_arquivos 3` e 2 KiB por volume, a alteração **135** recusa por «diario chegou ao teto de 3 volumes», e a linha **fica gravada com o valor novo**, com a coluna marcada ou sem ela. Linha sem diário quer dizer réplica divergindo e cascata pulada | ☐ pedido próprio, na conta; **não bloqueia o 368**, que não o toca |
| `esvaziar_lixeira` no `OPS_ESCRITA` | provável: a réplica não consegue esvaziar o `.trash` dela, que é o mesmo furo que o C2 fechou para a trilha. **Não medido nesta revisão** | ☐ pedido próprio (na dúvida, fica na conta), medir primeiro; não bloqueia |
| A4 (paginação da `op_trilha` por `pular`) | não | ⏸ |
| fsync de diretório | não, para esta frente (seção A) | segue no 467 |

## D) ⏸ (depois da versão)

- **P3, número reusado:**
  - Ocorre depois de uma queda entre o `rename` e o nascimento, seguida de expurgo antes do próximo evento.
  - Medido: os volumes 1 e 2 saíram, e o evento seguinte nasceu no **1**.
  - Pede SIGKILL na janela de dois syscalls, e o rastro se distingue pelas datas.
  - Conserto provável: sem o ativo no disco, o motor faz nascer o N+1 **antes** do plano, e o número passa a morar num cabeçalho. Hoje o expurgo leva junto o último volume, que era o único que guardava o número.
- **P4, custo da passada em tabela sem trilha:**
  - Uma listagem do diretório por tabela: **319 µs/tabela** com 800 arquivos, e **1.282 µs/tabela** com 3.200 arquivos (**513 ms** para 400 tabelas).
  - O custo cresce com N² por passada, mas é diário e é pago por fatias sob a trava.
- **Ativo com cabeçalho zerado** (não só curto), em sistema de arquivos sem `data=ordered`.
- **Downgrade da tabela sem paginação depois do primeiro fechamento:** o binário antigo só enxerga o ativo. Hoje isso está só insinuado no FORMATO §7; deve ser dito por inteiro.
- **A6:** falta a prova de vermelho do intervalo monotônico.
- **Relógio que salta para a frente** faz a retenção apagar antes. Não é achado: MySQL (`binlog_expire_logs_seconds`) e PG (`log_rotation_age`) também decidem pelo relógio de parede, e isso é convergência.

**Sobe ao dono: nada.** Os dois bloqueantes são de engenharia, e nenhum toca em pétrea.
