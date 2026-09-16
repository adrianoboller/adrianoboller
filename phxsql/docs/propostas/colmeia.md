# Proposta: o segundo tipo de banco — a colmeia (hive), inspirada no REGF

Proposta **antes do código** (como `docs/SOMBRA.md` e `docs/MEMORIA.md`),
pedida pelo dono em 11/09/2026 depois de medirmos a velocidade do Registro do
Windows. Ideia: o PhxSql passa a ter **dois tipos de banco** —

1. **Padrão** — o que já temos: modelo de arquivos separados do HFSQL, tabelas
   relacionais, índices, integridade referencial, transações. Bom para dado
   **transacional e relacional**.
2. **Colmeia (hive)** — um armazém **hierárquico chave→valor**, mapeado em
   memória, inspirado no formato REGF do Registro do Windows. Cada arquivo é uma
   colmeia; uma camada de montagem junta várias colmeias numa árvore só (como o
   Regedit junta hives). Bom para **configuração e metadados** — lido o tempo
   todo, escrito raramente.

Isto **não é trocar** um pelo outro: são dois tipos para dois trabalhos.

## 1. A premissa, medida antes de virar plano

A lei manda medir a receita de fora contra o nosso gargalo antes de aceitá-la.
Nós medimos o Registro real (`bancada/registro/resultados.json`, Windows
WXSOLUCOES, 11/09):

| | Registro (medido) | PhxSql padrão (medido 08/09) |
|---|---|---|
| ler 1 ponto | **~2–5 µs/op** | 7,94 µs/op |
| gravar 1 ponto | **~255–495 µs/op** | 12,8 µs/op |

**O que a medição decide de cara:** a colmeia é um desenho **rápido para ler,
caro para escrever**. Ganha na leitura (2–4×) e **perde feio na escrita**
(20–40×). Isso não condena a ideia — **condena usá-la para a coisa errada**.
Para configuração (escreve-se um punhado de vezes, lê-se milhões), o custo de
escrita é irrelevante e a leitura rápida é ouro. Para dado transacional, o
padrão ganha e continua sendo a escolha.

**A premissa da NOSSA colmeia, agora MEDIDA (12/09/2026).** Os números acima são
do Registro (C, sobre o `mmap` do Windows). Faltava saber se a *nossa* colmeia,
em Rust e com o nosso cache, leria tão rápido. Medimos com um protótipo PSHV
mínimo (`crates/phxsql-store/examples/custo-da-colmeia.rs`,
`bancada/colmeia/resultados.json`), máquina parada (`esta-medindo.sh`: carga
0,20, 4 núcleos), **comparação justa** — ler o mesmo par chave→valor por busca de
ponto → busca de ponto nos dois lados (descida de árvore de células na colmeia ×
`buscar` por índice único + `ler` no Padrão de verdade), os dois quentes,
mediana de 15 repetições:

| N pontos | colmeia (µs/op) | Padrão (µs/op) | razão | faixas cruzam? |
|---:|---:|---:|---:|---|
| 1.000 | 0,162 [0,155; 0,193] | 1,716 [1,703; 1,751] | **10,6×** | não |
| 10.000 | 0,213 [0,199; 0,241] | 2,563 [2,188; 3,163] | **12,1×** | não |
| 100.000 | 0,289 [0,256; 0,360] | 3,968 [2,945; 5,436] | **13,7×** | não |

**A premissa confirma medida, não morre:** a colmeia lê config-shaped
**10,6×–13,7× mais rápido** que o Padrão, e a vantagem cresce com N (a árvore do
`.ndx` do Padrão fica mais alta; a colmeia, config-shaped, quase não aprofunda).
Ressalva honesta para quem repetir: parte do ganho é o protótipo devolver
`&[u8]` sem alocar contra o Padrão decodificar em `Vec<Value>` — isso é
*inerente* à diferença dos dois desenhos (config sem decodificação relacional é
o que um hive-store compra), não artifício de bancada. Isso **justifica seguir
ao desenho de formato** — mas continua só a premissa: nada aqui autoriza
`TipoDatabase::Hive` a sair de `motor_pronto()==false`; V3+ do formato PSHV segue
atrás do P0 e do aval do dono.

### 1.1 — CRUD medido: colmeia × SQLite × padrão, 16/09/2026

Pergunta do dono, literal: *«Compare o tipo colmeia com o sqlite e phxsql —
Insert, update, delete e select.»* O H2 acima media só a leitura; esta seção
mede as **quatro operações de ponto** nos **três lados** — o protótipo PSHV
(agora com escrita por *append*, uma operação de cada vez), o SQLite(R) 3.45.1
da biblioteca padrão do Python e o motor Padrão de verdade — com a mesma
disciplina: mesmos dados por construção (o exemplo Rust grava o conjunto num
JSON e o Python o lê), 4.000 operações por repetição, 15 repetições, mediana e
faixa min–máx, vencedor só com a faixa inteira abaixo das outras duas, máquina
parada conferida antes de cada seção. Fonte de todo número abaixo:
`bancada/colmeia/resultados-crud.json`; como refazer e o que cada lado faz por
operação: `bancada/colmeia/LEIA-ME.md`.

**Trabalho comparado — e o que NÃO é igual, dito antes do número.** O Padrão
mantém o índice único no `.ndx`, o diário `.log`, a lixeira `.trash` e o
descritor; o SQLite(R) mantém a b-tree da tabela, a b-tree do índice da chave e
o *rollback journal* de cada transação; o protótipo da colmeia **não tem**
índice separado, diário, transação, lixeira nem tipos. A escrita dele é a
**cópia de caminho** que a pétrea do append-only força (§2: célula nunca se
reescreve nem se reusa): a célula nova nasce no fim, e folha, grupo e raiz
nascem de novo atrás dela; só o bloco base de 128 bytes é escrito no lugar. O
que isso compra é atomicidade por construção, sem diário; o que custa aparece
em «bytes anexados por operação» abaixo. A frase «a colmeia é N× mais rápida»
só vale com «fazendo menos» ao lado — e a tabela de chamadas de sistema por
operação, medida com `strace`, é o «menos» em número.

<!-- gerado por `python3 bancada/colmeia/medir-crud.py --markdown` de `bancada/colmeia/resultados-crud.json` (2026-09-16 06:50 UTC) -->

Medido em 2026-09-16 06:50 UTC (container Linux, 4 nucleos, kernel 6.18.44-fc-v33; SQLite 3.45.1, Python 3.11.15; binário de 2026-09-16 06:50 UTC); carga(1 min) no início 1,93; 4000 operações por repetição, 15 repetições, mediana e faixa min–máx em µs/op.

Linha de base do Python (mesmo laço, sem tabela): `execute('SELECT ?')` + `fetchone()` = **0,838 µs/op** [0,825; 0,858]; laço vazio = 0,030 µs/op. Publicada, nunca subtraída.

**Regime `por_operacao`** — Padrão: sincronizar() após cada escrita; colmeia: fsync após as células e fsync após o bloco base; SQLite: synchronous=FULL + autocommit (fsync do journal e do banco em cada transação).

| operação | N | colmeia (µs/op) | Padrão (µs/op) | SQLite (µs/op) | Padrão/colmeia | SQLite/colmeia | SQLite/Padrão | vencedor |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| ler | 1.000 | 0,142 [0,137; 0,158] | 1,443 [1,411; 1,617] | 4,440 [4,391; 4,532] | 10,18× | 31,34× | 3,08× | colmeia ganha |
| inserir | 1.000 | 275,390 [263,504; 289,119] | 909,301 [874,879; 983,281] | 814,834 [769,669; 872,138] | 3,30× | 2,96× | 0,90× | colmeia ganha |
| atualizar (1000 distintas) | 1.000 | 275,866 [249,818; 391,515] | 814,064 [775,449; 1.119,993] | 767,519 [694,082; 832,928] | 2,95× | 2,78× | 0,94× (cruza) | colmeia ganha |
| excluir (1000 distintas) | 1.000 | 308,627 [241,352; 347,189] | 1.157,598 [1.107,573; 1.276,543] | 771,874 [715,495; 818,576] | 3,75× | 2,50× | 0,67× | colmeia ganha |
| ler | 10.000 | 0,200 [0,184; 0,249] | 2,084 [2,002; 2,272] | 5,644 [5,049; 6,276] | 10,40× | 28,18× | 2,71× | colmeia ganha |
| inserir | 10.000 | 288,657 [260,304; 313,229] | 953,064 [847,911; 1.034,398] | 799,526 [766,635; 1.699,200] | 3,30× | 2,77× | 0,84× (cruza) | colmeia ganha |
| atualizar | 10.000 | 357,470 [299,161; 565,185] | 935,680 [887,645; 1.073,537] | 806,391 [735,173; 5.642,834] | 2,62× | 2,26× | 0,86× (cruza) | colmeia ganha |
| excluir | 10.000 | 291,177 [270,657; 307,373] | 1.290,409 [1.204,240; 1.985,317] | 850,707 [808,672; 937,520] | 4,43× | 2,92× | 0,66× | colmeia ganha |
| ler | 100.000 | 0,390 [0,373; 0,517] | 3,269 [2,971; 3,592] | 6,232 [6,051; 7,047] | 8,38× | 15,97× | 1,91× | colmeia ganha |
| inserir | 100.000 | 391,190 [345,125; 6.548,020] | 996,874 [903,556; 28.878,549] | 818,970 [776,116; 890,869] | 2,55× (cruza) | 2,09× (cruza) | 0,82× | dentro do ruido |
| atualizar | 100.000 | 382,865 [337,837; 537,774] | 914,966 [878,101; 951,045] | 829,580 [763,690; 925,421] | 2,39× | 2,17× | 0,91× (cruza) | colmeia ganha |
| excluir | 100.000 | 373,811 [323,645; 420,352] | 1.236,677 [1.138,466; 1.374,521] | 826,359 [797,489; 889,129] | 3,31× | 2,21× | 0,67× | colmeia ganha |

Bytes anexados por operação na colmeia (o preço da cópia de caminho — a raiz cresce com N): inserir N=1.000: 500 B; atualizar N=1.000: 622 B; excluir N=1.000: 518 B; inserir N=10.000: 922 B; atualizar N=10.000: 1.062 B; excluir N=10.000: 984 B; inserir N=100.000: 5.144 B; atualizar N=100.000: 5.276 B; excluir N=100.000: 5.212 B.

Chamadas de sistema por operação em `por_operacao` (N = 1.000, `strace -c`, por diferença 1.000 − 200 ops):

| operação | lado | fsync | fdatasync | write | pwrite64 | openat | unlink |
|---|---|---:|---:|---:|---:|---:|---:|
| ler | colmeia | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 |
| ler | padrao | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 |
| ler | sqlite | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 |
| inserir | colmeia | 2,0 | 0,0 | 2,0 | 0,0 | 0,0 | 0,0 |
| inserir | padrao | 8,0 | 0,0 | 8,1 | 0,0 | 1,0 | 0,0 |
| inserir | sqlite | 0,0 | 4,0 | 0,0 | 14,4 | 2,0 | 1,0 |
| atualizar | colmeia | 2,0 | 0,0 | 2,0 | 0,0 | 0,0 | 0,0 |
| atualizar | padrao | 8,0 | 0,0 | 6,0 | 0,0 | 1,0 | 0,0 |
| atualizar | sqlite | 0,0 | 4,0 | 0,0 | 10,1 | 2,0 | 1,0 |
| excluir | colmeia | 2,0 | 0,0 | 2,0 | 0,0 | 0,0 | 0,0 |
| excluir | padrao | 9,0 | 0,0 | 13,0 | 0,0 | 6,0 | 0,0 |
| excluir | sqlite | 0,0 | 4,0 | 0,0 | 14,3 | 2,0 | 1,0 |

**Regime `sistema`** — Padrão: nunca sincroniza (lixeira na janela); colmeia: só write; SQLite: synchronous=OFF + autocommit (1 transação por operação, sem fsync).

| operação | N | colmeia (µs/op) | Padrão (µs/op) | SQLite (µs/op) | Padrão/colmeia | SQLite/colmeia | SQLite/Padrão | vencedor |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| ler | 1.000 | 0,149 [0,142; 0,179] | 1,546 [1,480; 1,845] | 4,580 [4,474; 5,054] | 10,36× | 30,69× | 2,96× | colmeia ganha |
| inserir | 1.000 | 2,058 [1,967; 2,321] | 3,729 [3,601; 3,978] | 27,982 [25,678; 29,775] | 1,81× | 13,60× | 7,50× | colmeia ganha |
| atualizar (1000 distintas) | 1.000 | 2,109 [2,049; 2,245] | 4,947 [4,733; 5,159] | 25,749 [24,412; 27,748] | 2,35× | 12,21× | 5,20× | colmeia ganha |
| excluir (1000 distintas) | 1.000 | 2,006 [1,887; 2,852] | 24,184 [23,144; 26,917] | 28,105 [26,994; 29,232] | 12,05× | 14,01× | 1,16× | colmeia ganha |
| ler | 10.000 | 0,192 [0,180; 0,235] | 2,140 [2,026; 2,304] | 4,974 [4,849; 5,557] | 11,17× | 25,96× | 2,32× | colmeia ganha |
| inserir | 10.000 | 2,501 [2,429; 2,684] | 3,911 [3,600; 4,243] | 29,256 [28,118; 31,566] | 1,56× | 11,70× | 7,48× | colmeia ganha |
| atualizar | 10.000 | 3,086 [2,891; 3,265] | 6,286 [5,856; 6,897] | 27,250 [26,040; 30,559] | 2,04× | 8,83× | 4,34× | colmeia ganha |
| excluir | 10.000 | 3,162 [2,961; 3,349] | 26,262 [24,709; 29,169] | 28,864 [27,901; 29,733] | 8,31× | 9,13× | 1,10× (cruza) | colmeia ganha |
| ler | 100.000 | 0,459 [0,443; 0,628] | 3,124 [2,891; 3,682] | 6,042 [5,660; 6,751] | 6,81× | 13,17× | 1,93× | colmeia ganha |
| inserir | 100.000 | 9,962 [9,556; 11,564] | 4,390 [4,307; 6,292] | 29,204 [28,446; 30,857] | 0,44× | 2,93× | 6,65× | padrao ganha |
| atualizar | 100.000 | 11,678 [11,261; 12,560] | 8,146 [7,762; 8,927] | 32,205 [30,751; 36,177] | 0,70× | 2,76× | 3,95× | padrao ganha |
| excluir | 100.000 | 11,368 [10,844; 12,548] | 27,531 [26,467; 32,080] | 33,344 [31,959; 35,346] | 2,42× | 2,93× | 1,21× (cruza) | colmeia ganha |

Bytes anexados por operação na colmeia (o preço da cópia de caminho — a raiz cresce com N): inserir N=1.000: 500 B; atualizar N=1.000: 622 B; excluir N=1.000: 518 B; inserir N=10.000: 922 B; atualizar N=10.000: 1.062 B; excluir N=10.000: 984 B; inserir N=100.000: 5.144 B; atualizar N=100.000: 5.276 B; excluir N=100.000: 5.212 B.

Chamadas de sistema por operação em `sistema` (N = 1.000, `strace -c`, por diferença 1.000 − 200 ops):

| operação | lado | fsync | fdatasync | write | pwrite64 | openat | unlink |
|---|---|---:|---:|---:|---:|---:|---:|
| ler | colmeia | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 |
| ler | padrao | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 |
| ler | sqlite | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 | 0,0 |
| inserir | colmeia | 0,0 | 0,0 | 2,0 | 0,0 | 0,0 | 0,0 |
| inserir | padrao | 0,0 | 0,0 | 3,1 | 0,0 | 0,0 | 0,0 |
| inserir | sqlite | 0,0 | 0,0 | 0,0 | 13,4 | 1,0 | 1,0 |
| atualizar | colmeia | 0,0 | 0,0 | 2,0 | 0,0 | 0,0 | 0,0 |
| atualizar | padrao | 0,0 | 0,0 | 3,0 | 0,0 | 0,0 | 0,0 |
| atualizar | sqlite | 0,0 | 0,0 | 0,0 | 9,1 | 1,0 | 1,0 |
| excluir | colmeia | 0,0 | 0,0 | 2,0 | 0,0 | 0,0 | 0,0 |
| excluir | padrao | 0,0 | 0,0 | 8,0 | 0,0 | 4,8 | 0,0 |
| excluir | sqlite | 0,0 | 0,0 | 0,0 | 13,3 | 1,0 | 1,0 |

**Leitura honesta.**

- **Ler é onde a colmeia é outro desenho:** 6,8×–11,2× sobre o Padrão e
  13×–31× sobre o SQLite(R), em todo N e nos dois regimes (o 6,8× é o
  N = 100.000 sem `fsync`; os outros cinco ficam entre 8,4× e 11,2×) —
  confirma o H2 com outra corrida. Do número do SQLite(R), 0,838 µs são a chamada Python → C
  (linha de base publicada; nunca subtraída).
- **Com `fsync` por operação (`por_operacao`) manda o disco, e o que se mede é
  quantos `fsync` cada desenho paga:** a colmeia 2 (células, bloco base), o
  SQLite(R) 4 `fdatasync` mais criar e apagar o `-journal`, o Padrão **8 a 9**
  (o `sincronizar()` sincroniza cada arquivo aberto da tabela, sem pular os
  limpos). Daí 275–391 µs contra 768–851 µs contra 814–1.290 µs. A colmeia
  ganha 2,1×–4,4× **fazendo menos**; e neste regime o SQLite(R) fica **à frente
  do Padrão** nas medianas de toda escrita (0,66×–0,94×) — fora do ruído no
  excluir (0,66×–0,67× em todo N) e no inserir a 1.000 e 100.000; nas outras
  quatro as faixas se cruzam. O único
  «dentro do ruído» é o inserir a 100.000, por um estouro de 6,5 ms na colmeia
  e 28,9 ms no Padrão numa das quinze repetições — a faixa está publicada.
- **Sem `fsync` (`sistema`) a cópia de caminho mostra o preço:** a 1.000 e
  10.000 a colmeia insere e atualiza 1,6×–2,4× mais rápido que o Padrão
  (2,0–3,1 µs contra 3,7–6,3), mas os bytes anexados por operação crescem com a raiz —
  500 B → 922 B → **5.144 B** — e a **100.000 o Padrão ganha** inserir (4,39 µs
  contra 9,96) e atualizar (8,15 contra 11,68), fora do ruído. É a raiz com
  391 grupos sendo copiada a cada escrita. **Isto é resultado, não defeito de
  bancada**: o desenho de formato (`colmeia-estrutura.md`) precisa de fanout
  limitado — lista de subchaves em célula própria por bin, ou mais um nível —
  antes de prometer escrita config-shaped a 100.000 chaves. Fica como premissa
  a medir do formato, não do protótipo.
- **O excluir do Padrão custa 24–28 µs mesmo sem `fsync`**, contra 3,7–4,4 do
  inserir: são 8 `write` e **~5 `openat` por exclusão** (lixeira e motivos
  abrindo arquivo a cada linha). É observação medida, não diagnóstico — vai
  para a pendência como item de medição do `custo-do-excluir`.
- **O SQLite(R) em autocommit paga uma transação por instrução**: 9–14
  `pwrite64`, um `openat` e um `unlink` do `-journal` em cada operação, 26–33 µs
  sem `fsync`. É o trabalho **a mais** que o casamento «1 operação = 1 unidade
  durável» impõe a ele; agrupado em transação ele seria outro número, e o
  `bancada/sqlite/medir.py` (bancada C) já mostra esse lado.

O que a medição **não** muda: nada aqui autoriza `TipoDatabase::Hive` a sair de
`motor_pronto()==false`. Ela acrescenta à premissa do H2 uma segunda, agora com
número: a colmeia paga a escrita append-only com bytes proporcionais ao fanout
da raiz, e o formato tem de nascer com isso limitado.

## 2. As divergências que as nossas pétreas FORÇAM — e é isto que a torna nossa

Lógica que saiu diferente da de origem não é cópia, e a prova é a divergência.
Onde a colmeia do PhxSql diverge do REGF, e por qual restrição nossa:

- **Sem reúso de célula livre — append-only.** O REGF reutiliza célula apagada
  (tamanho positivo entra na lista livre). A nossa pétrea *«a ordem de digitação
  é sagrada, nunca reaproveita slot»* **proíbe**. A colmeia do PhxSql marca a
  célula apagada e **nunca a reusa**; compactar é operação explícita (um
  `VACUUM`), nunca silenciosa. Consequência boa: o «undelete» que o Registro dá
  por acidente forense vira **propriedade projetada** aqui.
- **Durabilidade pela nossa máquina, não pelo log do Registro.** O REGF usa
  `.LOG1`/`.LOG2`. Nós já temos a marca `.tx` write-ahead e a disciplina de
  `fsync`. A colmeia usa **a mesma** máquina de transação do padrão — e herda o
  conserto de atomicidade do P0 (o commit pai+filho) quando ele entrar. Não se
  reinventa o log do Registro.
- **Integridade, se houver relação.** O Registro não tem FK. Uma colmeia de
  config é quase plana, mas a pétrea primordial vale na árvore por construção:
  **não se apaga uma chave que tem subchaves** sem tratar as filhas — que é o
  `Restrict` da casa, de graça, numa hierarquia.
- **UTF-8 e zero dependência.** O REGF é UTF-16LE; nós gravamos o formato à mão,
  em UTF-8, só com a `std` — o mesmo método que deu o SHA-256 e o SQL daqui.
- **Segurança pela nossa cifra.** Onde o REGF tem o `sk` (descriptor Windows), a
  colmeia usa a cifra em repouso e os direitos que o PhxSql já tem.

O desenho interno pode ser **inspirado** no REGF (bloco base + bins + células
endereçadas por offset, a árvore nascendo de uma célula raiz), porque é um bom
desenho para leitura mapeada — mas cada decisão passa pelo crivo acima. O que
não passar, não entra.

## 3. Onde ela mora: o Phoenix OS

O parecer do Sprint 0010 já apontou: o PhxSql pode dar ao Phoenix OS a
persistência de **configurações, tarefas, catálogo e contexto**. Isso é
exatamente o trabalho de uma colmeia — hierárquico, lido o tempo todo, escrito
raramente —, e é o que o Registro É para o Windows. O padrão relacional fica
para o dado de negócio; a colmeia, para o estado do sistema.

## 4. O que isto NÃO é, e a ordem certa

- **Não é para agora.** O mesmo parecer que sugeriu a integração mandou
  **pausar a ampliação de funcionalidades até o núcleo estar sólido** — e o
  núcleo tem um **P0 de atomicidade aberto** (o commit pai+filho que deixa
  estado parcial visível). Construir um segundo tipo de banco antes de consertar
  o commit do primeiro é exatamente o anti-padrão que o parecer alertou.
- **Não é cópia.** Ver §2: as divergências forçadas pelas nossas pétreas são a
  prova de que o desenho passou pela nossa cabeça.
- **Não é decidido.** É proposta. O que a destrava é **medir a premissa**: um
  protótipo mínimo de colmeia (ler/gravar um valor por caminho) medido contra o
  padrão, para config-shaped data, com a mesma disciplina da
  `bancada/comparacao` (mesmo trabalho, faixa min–máx, data).

## 5. Recomendação

1. **Primeiro o P0** (atomicidade do commit) e os P1 de SQL — o núcleo sólido
   que o parecer exige.
2. **Depois, medir a premissa** da colmeia (protótipo mínimo × padrão), antes de
   qualquer formato novo. Se a nossa colmeia não ler bem mais rápido que o
   padrão para config, a ideia morre medida — e a recusa com número impede que
   ela volte sem medição.
3. **Se a premissa passar**, a colmeia entra como frente de formato (PSCH-like
   para colmeia, a camada de montagem que junta várias numa árvore), decidida
   contra as restrições da §2, com o dono aprovando o formato antes de gravar.

O desenho concreto da estrutura em disco — bloco base, bins, células, os três
níveis (database → colmeias → árvore montada) e as divergências que as pétreas
forçam — está em [`colmeia-estrutura.md`](colmeia-estrutura.md). É proposta de
formato: nada gravado, nada decidido, à espera do dono e da premissa medida.
