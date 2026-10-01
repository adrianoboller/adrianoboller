# Inventário de QA, 01/10/2026: pedidos de teste e guarda abertos (245, 263, 321, 352, 401, 437)

Papel G, só leitura. Medido em 01/10/2026 entre 01:30 e 01:40 UTC, com a árvore em `93fd1604`.
Nenhum código, `PENDENCIAS.md` ou dossiê foi tocado. A única execução de teste foi
`laco_do_unico_secundario::o_conflito_de_unicidade_para_o_par_marcado`, 5 vezes, com o
binário já compilado às 01:18 (`target/debug/deps/laco_do_unico_secundario-b5cd437fc4b0cade`).

| pedido | estado no PENDENCIAS | estado medido hoje | o que falta |
|---|---|---|---|
| 263 | ◐ | catracas em 0, mas **177 de 550** guardas nunca foram julgadas | provar por lotes; o prazo do provador continua sem medida |
| 321 | ☐ | **consertado pelo 485 (24/09)**, e a linha não foi fechada | fechar, apontando o 485 |
| 352 | ☐ | aberto; a causa provável **não é** o parceiro: é `porta_livre()` mais o `escutar()` engolido | conserto no apoio do teste (abaixo) |
| 401 | ◐ | 24 arquivos fechados por construção; 3 arquivos e 6 chamadas ficam com a janela | é o mesmo resto do 352 |
| 437 | ☐ | aberto; o assert que cai **nunca foi capturado** | ler o tid em vez da listagem `/proc/self/task` |
| 245 | ◐ | O1 e O3–O6 fechados; **O2 tem decisão do dono (17/09) que não foi implementada** | implementar a recusa; (b) `calculada` cabe ao papel C |

## 263: guardas quebradas

- `python3 bancada/guardas/trecho-vivo.py --numeros` dá tudo `0`: `TETO_TRECHO_MORTO`,
  `TETO_TRECHO_AMBIGUO`, `TETO_TESTE_MORTO`, `TETO_TESTE_FORA_DO_BINARIO`,
  `TETO_TESTE_SEM_MODULO` e `TETO_NAO_JULGADA_ESCONDIDA`. `PISO_DAS_ENTRADAS = 551` (551 medido).
  **Não há trecho morto nem ambíguo hoje.**
- `provar-guardas.py --listar` mostra **550** guardas no catálogo.
- `ultima-corrida.json` tem 374 vereditos: 370 PROVADA, 4 REDUNDANTE, 0 reprovada e 0 quebrada.
  Um desses vereditos é de uma guarda aposentada (`cifra-do-fio-imposta`). Sobram **373 guardas
  vivas julgadas e 177 que NUNCA foram julgadas** (32%). A régua diz isso:
  «177 sem veredito … 177 nomeadas na tabela publicada, 0 escondidas».
- **Idade dos vereditos**:

  | data | vereditos |
  |---|---|
  | 16/09 | 121 |
  | 17/09 | 11 |
  | 18/09 | 3 |
  | 24/09 | 186 |
  | 30/09 | 39 |
  | 01/10 | 14 |

  As **121 mais antigas são de 16/09 15:25**, há 15 dias, e começam por `profiler-recorta`,
  `profiler-recorta-largo`, `evento-linha-sem-escape`, `profiler-sem-portao`,
  `pivotar-sem-portao`, `sequencias-sem-portao`, `duplicar-sem-destino`,
  `regra-de-tabela-imposta`, `cadeia-sem-teto` e `backup-sem-sha256`. Desde então, nenhuma delas
  foi reposta contra o defeito.
- **Guardas sem nenhuma prova, por família**, das 177 (amostra da cabeça da lista):
  - **senha e segredo**: `senha-sobra-no-erro-do-cadastro`, `portao-da-senha-por-espaco`,
    `eco-do-sql-com-a-senha`, `login-sem-o-teto-da-senha`, `core-leva-a-senha-do-cofre`.
  - **jobs e backup**: `job-recusado-roda-mesmo-assim`, `backup-agendado-falha-calado`.
  - **atestado e fecho do `.ndx`**: `fechar-baixa-o-byte-52-sem-fsync`,
    `atestado-sobrevive-a-escrita`, `phx-reindexar-nao-reindexa`.
  - **FK**: `fk-antes-do-default`.

  Para a pétrea «senha nunca em texto puro», isso significa que a guarda existe e ainda não
  está provada.
- **Achado lateral**: neste instante há um provador de outra frente rodando em worktree. O
  `ultima-corrida.json.tranca` da árvore principal aponta para o pid 20696, que não existe mais
  (tranca órfã). Não mexi nela.
- **Conserto proposto** (o mesmo caminho que o pedido já nomeia): corridas `--so` **agrupadas
  por tupla `(pacote, alvo)`**, começando pelas 177 sem veredito e depois pelas 121 de 16/09,
  com `du -sh ~/.cache/phx-guardas` conferido a cada lote. Hoje o cache tem **2,5 GiB**.
  O prazo de 420 s continua sem medida.

## 321: réplica que não chegou (continuidade-da-replica)

**Já consertado, pelo pedido 485** (`6fd7d5f9`, 24/09). O `esperar_eventos` passou a usar
`eventos_de_clientes_se_ja_chegou`, que trata `NAO_ENCONTRADO` como «ainda não chegou» e deixa
qualquer outro erro derrubar o teste. Esse é exatamente o conserto «esperar pelo FATO» que o
321 pede. Prova registrada no 485: 4 quedas em 13 corridas antes, e 20 de 20 depois, com a
máquina carregada.

**Proposta**: fechar o 321 como duplicata do 485. O resíduo que fica é só de **cobertura**,
não de floco: `:282` dorme 1,5 s para provar que «nada chegou». Sob carga, essa espera passa
por engano, mas nunca cai em vermelho.

## 352: «database loja já existe» no par marcado

**A hipótese do pedido não se sustenta lendo o código.** O `beta` (parceiro) é `Multi` e não
tem nenhuma origem, então ninguém replica para ele. E o `alfa` cria `loja` antes de qualquer
parceiro existir.

**Causa provável (não medida)**:

1. `terreno_com` (`tests/laco-do-unico-secundario.rs:253-254`) ainda usa `porta_livre()`.
   É uma das 3 exceções do 401: o teste sorteia a porta, solta e liga depois.
2. Se outro teste **do mesmo binário** (mesmo `TOKEN`, rodando em paralelo) ligar aquele
   número primeiro, o `bind` do `beta` falha. Ninguém vê a falha, porque `subir()` faz
   `let _ = copia.escutar()` (`:108-109`).
3. Mesmo assim o `esperar_porta(porta_b)` passa, porque a porta **está** aberta, só que é a
   do vizinho.
4. O `criar_clientes(porta_b)` cai então no servidor do outro teste, que já tem `loja`.
   Resultado: «loja ja existe».

O mesmo desenho explica a segunda ocorrência registrada no próprio 352: o
`continuidade-da-replica` ainda usava `porta_livre` no dia 18/09.

Medido hoje: **5/5 verdes isolado** (5,14 a 5,26 s). É o esperado, porque sozinho não há
vizinho com quem colidir.

**Conserto proposto (no apoio, sem produção nova)**:

1. Em `subir()`, depois do `esperar_porta`, exigir `s.porta_dos_dados() == Some(porta)`. Se a
   porta não for nossa, o teste cai dizendo «outro processo tem a porta». Isso troca a
   conversa cruzada calada por um erro que nomeia a causa.
2. Um `TOKEN` por teste (`format!("unico-secundario-{nome}")`): a conversa cruzada vira
   recusa de token em vez de mexer no dado do vizinho.
3. Para fechar por construção: `Servidor` aceitar um `TcpListener` já aberto, que o 401
   nomeou como produção nova fora do escopo. Com isso, o teste segura `porta_b` aberta desde
   o sorteio. Isso é decisão de papel B/C.

O mesmo vale para os outros dois portadores da exceção: `cluster-cifrado.rs`, que tem faixa
fixa 7400-7448 com `bind`/`drop`, e `identidade-do-pulso.rs`. O 352 e o resto do 401 são **um
pedido só**.

## 401: corrida de porta (cifra-das-portas-http)

O arquivo do pedido está fechado por construção: as quatro portas são pedidas em `:0` e lidas
de volta por `porta_web`, `porta_rest` e `porta_swagger`.

A **segunda queda**, em `Servidor::novo(c).unwrap()`, tem uma explicação coerente que o pedido
não escreveu. Na época, o arquivo sorteava **quatro** portas com `porta_livre()`. Dois sorteios
seguidos podem devolver o mesmo número, porque o ouvinte cai antes do próximo `bind`; é a
cognição `bind-zero-nao-e-livre-de-corrida-se-o-ouvinte-cai`. Com dois números iguais, o
`Config::validar` recusa «duas portas no mesmo endereço», e é o `novo` que cai, não o
`escutar`. Essa explicação é coerente, mas não foi medida.

**Resta**: as 6 chamadas `porta_livre()` em 3 arquivos. O conserto é o mesmo do 352.

## 437: threads do SO (telemetria)

O teste é `telemetria::testes::as_threads_do_so_se_medem_e_nunca_sao_menos_que_as_registradas`
(`src/telemetria.rs:2077`). Os asserts que podem cair são três:

- `:2117`, «as vizinhas não saíram do SO em 5 s»;
- `:2142`, `tarefas_chamadas("presa-do-teste") == 1`;
- `:2103` e `:2137` são pisos e não caem.

Não há colisão de nome: grep de `vizinha-` e `presa-do-teste` em `crates/` acha só este teste.

Os dois que podem cair dependem de `tarefas_chamadas`, que faz `read_dir("/proc/self/task")`
inteiro enquanto o libtest cria e mata threads de outros testes. A listagem de
`/proc/PID/task` retoma pela posição. Quando a tarefa do cursor morre entre dois `getdents`,
o kernel recomeça pelo índice e **pode pular entradas**, e também falha com
`read_to_string(comm)` de quem morreu (`unwrap_or(false)`). Isso é plausível e **não está
medido**: a mensagem nunca foi capturada, como o próprio pedido diz.

**Conserto proposto, por construção e só com a `std`**: a thread `presa` e cada vizinha mandam
o **próprio tid** pelo canal (lido de `readlink /proc/thread-self` = `PID/task/TID`). O teste
passa a olhar `/proc/self/task/<tid>/comm` (existe e tem o nome) e a esperar
`/proc/self/task/<tid>` sumir. Assim não sobra nenhuma listagem de diretório disputada com os
vizinhos.

**Antes de consertar**, a medição que o pedido pede continua valendo: `--workspace` com
`2>&1` para arquivo, sem cano.

O 581 **não** é o mesmo defeito: ali era a linha de stderr lida pela metade (`porta_no_texto`).
O que ele e o 437 têm em comum é só a forma: ler um estado do SO no meio de uma escrita ou
mudança alheia.

## 245: O1–O6

- O1, O3, O4, O5 e O6 estão fechados, com prova.
- **O2(a) tem decisão do dono, de 17/09 05:41**: «RECUSAR A DECLARAÇÃO, nomeando quantas
  linhas violam». **O código não a implementa.** `op_acrescentar_coluna`
  (`src/servidor.rs:19596`, por volta da linha 107) ainda só devolve `avisos` («a que o violar
  só será recusada no próximo …»). Não há recusa nem contagem de violadoras. É decisão
  tomada e não cumprida, então entra na conta.
  - **Conserto**: com `registros > 0` e `coluna.check` presente, varrer as linhas uma vez,
    avaliar o CHECK com o valor novo (padrão ou `NULL`) e recusar **antes** de reescrever o
    volume, nomeando N.
  - **Ressalva**: a coluna nova vale o `padrao` em toda linha, então um CHECK que só fala da
    coluna nova é decidido pela linha de prova que já existe. Só o CHECK que cita coluna velha
    precisa varrer.
  - **Prova**: um teste vermelho com o defeito reposto, mais o controle «tabela vazia aceita»
    (o comportamento velho).
- **O2(b)**, `calculada` acrescentada nula na linha velha: segue com o papel C (STORED dentro
  do `RegFile::acrescentar_coluna`, ou VIRTUAL com um byte no `PSCH`). Não é código pendente
  de QA.

## Reinícios do contêiner: o que se mediu e os suspeitos

**Medido agora**:

- `memory.max_usage_in_bytes` = **11,3 GB** com 22 minutos de uptime, em 15 GB sem swap;
  `failcnt 0`.
- load 7,45 em 4 CPUs.
- Um `rustc` do provador vizinho com **1,06 GB RSS** compilando `phxsql_server --test` com
  debug desligado.
- O `portoes.sh` não limita `-j` nem `--test-threads`.

O pico de memória já está perto do total **sem a suíte rodar**. A compilação concorrente
(portões + provador com outro `target`) é o suspeito número 0, e não é teste.

**Achado medido**: servidores de teste **nunca param**. `subir()` solta `escutar()` numa thread
e ninguém chama parar; só 2 dos 48 arquivos de `tests/` param o servidor. Os laços de serviço,
inclusive a replicação com `reconectar_em: 1`, continuam vivos até o binário sair, **depois do
`DirTemp` apagado**. Por isso recriam a pasta. Prova no disco agora:

- `/tmp/phxsrv-it-25885-unico-secundario-parceiro-535-1/loja/clientes.{reason,trash}`
- `…-puxa-535-0/loja`
- `/tmp/phxsrv-it-23474-unico-secundario-parceiro-535-1`

São três pastas de hoje (00:17 e 00:36) escritas por servidor órfão. Além delas, há 38
`phxsrv-ut-*` (`panico-451-*`, `cluster-nonce-gigante`).

Ordem de suspeita (nada foi rodado):

1. **`tests/laco-do-unico-secundario.rs`**: 2 servidores `Multi` por teste e 6 testes, laço
   bidirecional a cada 1 s, nenhum para. É a fonte comprovada das pastas recriadas acima. Usa
   `porta_livre`.
2. **`phxsql-store/tests/pag-se-troca-inteiro.rs:82`**: leitor `while !pare` dentro de um
   `thread::scope`. Se `gravar_pag().unwrap()` cair (disco perto da cota, ou defeito reposto
   no provador), o `scope` espera a thread que nunca para. O resultado é um **trava eterna
   com `read_to_string` em laço sem teto**, até o prazo externo.
3. **`tests/porta-do-psch-v10.rs:551,570`**: escritor `while !acabou` inserindo sem teto de
   voltas pela rede. Se um assert antes de `acabou.store(true)` cair, ele grava sem parar no
   servidor órfão até o binário acabar.
4. **`tests/continuidade-da-replica.rs`** e **`tests/lixeira-da-replica.rs`**: source e
   réplica por teste, `reconectar_em: 1`, nenhum para. É o mesmo padrão do 1.
5. **`tests/trava-atras-da-rede.rs:87`**: fonte falsa que **guarda soquetes presos** (`presos`)
   e sobe uma thread por conexão. A réplica órfã reconecta a cada 1 s até o binário sair, e
   threads e descritores crescem com o tempo de vida do binário.

Seguem de perto:

- `tests/teto-do-aperto.rs` e `src/replica.rs:1033`: despejo de **192 MiB**. Com o defeito
  reposto no provador, o `read_line` cresce até cerca de 256 MiB por teste.
- `tests/corte-do-diario-pelo-config.rs` (20k + 30k inserções) e
  `phxsql-store/tests/corte-do-diario.rs` (2 × 20k): E/S pesada com `fsync`.
- `tests/identidade-do-pulso.rs` e `tests/cluster-cifrado.rs`: nós de cluster com pulso, sem
  parar.

**Conserto proposto** (papel B, sem mexer na produção):

- Um `Drop` no apoio (`comum::ServidorDeTeste`) que chama a parada do serviço e junta a thread
  do `escutar`. A ordem tem de ser servidor primeiro e `DirTemp` depois, invertendo a ordem
  de queda dos `let`.
- Teto de tempo ou de voltas nos dois laços `while !flag` dos itens 2 e 3: `&& Instant::now() <
  ate`. No item 2, o mais seguro é `pare.store(true)` num guarda de `Drop` dentro do `scope`.
