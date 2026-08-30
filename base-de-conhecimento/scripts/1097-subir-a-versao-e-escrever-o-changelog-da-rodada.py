# Subir a versao e escrever o CHANGELOG da rodada
# 29/08 07:47

import io
p='CHANGELOG.md'
s=io.open(p,encoding='utf-8').read()
anc='''## 0.17.0 — 2026-08-29'''
assert s.count(anc)==1
novo='''## 0.18.0 — 2026-08-29

A rodada dos concorrentes. Três motores lidos no fonte — InnoDB, Aria e
Cassandra (`docs/CONCORRENTES.md` e `docs/CASSANDRA.md`, toda citação com
`arquivo:linha`) — e cada ideia **medida aqui antes de entrar**. O resultado
que resume a versão: **a bancada de dez milhões passou a ganhar do MySQL(R) no
insert** — 91,5 s contra 112,4 (109.300 linhas/s contra 88.994), ganhando
também buscar (13×), varrer (11×) e atualizar (12×). Só excluir ainda perde.

### Corrigido

- **O medidor com binário velho media o passado.** `cargo build --release` não
  recompila os *examples*, e a bancada chama `target/release/examples/carga`
  direto: uma rodada inteira de ganhos ficou invisível, e a conclusão «o
  esquema custa 2,2×» nasceu — com tabela e tudo — dessa diferença. É o sétimo
  diagnóstico plausível que a medição derruba, e este era nosso duas vezes. A
  receita do `bancada/LEIA-ME.md` já mandava certo; a lição foi para o
  `CLAUDE.md`.
- **`Table::abrir` lia o volume inteiro do `.reg`** para tirar dele 128 bytes
  de cabeçalho e o bloco de esquema — 69 ms por milhão de linhas, a cada
  abertura, e o servidor abre a tabela a cada pedido. Duas leituras curtas:
  138,80 → 0,03 ms, e plano em vez de linear. Buscar na bancada caiu de 4,04
  para 0,20 s.
- **A réplica não ficava para trás por culpa dela.** A causa registrada
  («aplicar reencoda o payload») custa 0,35 µs de 229; o custo era o **source**
  varrendo o diário desde o começo a cada lote — quadrático. Marca de posição:
  45×, e 4.273 → 17.450 eventos/s por réplica. As três juntas passam o master.
- **Reabrir a tabela reescrevia o esquema**, e com o bloco v6 mais longo a
  primeira gravação comeria o slot 1 em silêncio, com CRC batendo. Agora os
  bytes do disco são preservados; o teste fabrica um arquivo antigo de verdade.

### Adicionado

- **Cache de páginas *write-back* no `.ndx`**, a ideia central dos três
  concorrentes: a página modificada fica suja em RAM e o CRC-32 e o `write`
  saem no despejo ou no `sincronizar` — não por chave. A garantia trocada é
  comprada de volta pela **marca de sujo** (byte 52 do cabeçalho): vai ao disco
  **antes** da primeira página suja, sai **depois** de todas, e um `.ndx`
  aberto sujo recusa toda operação e manda reconstruir — queda **detectada**,
  nunca silenciosa. Sem migração: arquivo antigo tem zero ali, que é a verdade.
  O empilhamento medido da rodada: 16,4 → 14,5 (cabeçalho do `.ndx` fora do
  caminho da chave — a **terceira vez** do mesmo defeito) → 13,1 (CRC
  slice-by-16, mesmo polinômio, nada muda de valor) → **7,5 µs por linha**.
- **Construção em lote da B+tree** (`construir_em_lote`): ordena, enche as
  folhas em sequência, monta os níveis por cima — 7,72 s → 0,31 s por milhão
  de chaves (23×), com o enchimento de 80% **medido** contra 70/90/95/100.
  Todo `reindexar` anda nisso. O adiamento de índice que ela destravaria foi
  medido e **recusado**: 1,22× no melhor caso, prejuízo abaixo de M≈N/3.
- **`BULKINSERT` medido no fio**: 43.500 → 66.500 linhas/s (1,53×) — a reserva
  mantém a janela de durabilidade aberta e a carga vira um `fsync` só.
- **Cifra nos diários** (pedido 101): ChaCha20-Poly1305 (RFC 8439, todos os
  vetores oficiais) ligada ao `.log`, `.trash` e `.reason` — **desligada por
  padrão**, arquivo antigo abre igual, nonce derivado do offset que o arquivo
  já tem, chave por PBKDF2 e por volume. Com o defeito «cifra imposta»
  reposto, 43 testes antigos quebram. A replicação continua: `posicao` conta
  pelos cabeçalhos claros e `replicar` devolve imagens decifradas pela sessão
  autenticada. E a **compactação foi medida de novo e recusada de novo**, agora
  com o corte do diário configurável (`recursos.diario_volume_mib`): mesmo a
  1 MiB ela poupa 14,7% — o `.ndx` sozinho pouparia 2,1× mais.
- **Marca de dado pessoal por coluna** (pedido 125): PSCH v6, três graus
  (LGPD art. 5º I e II), op `dados_pessoais` que audita a base — com
  conferência própria porque não tem o campo `tabela` que o portão lê — e a
  tela que diz *que não sabe* quando o esquema não traz a marca.
- **Jobs de execução** (pedido 51): agenda, corridas em diário próprio, e **o
  job roda com o poder do usuário dele** — os portões do `despachar` foram
  extraídos para uma função só em vez de copiados.
- **Parar e subir o serviço pela tela** (pedido 40), trocando a porta: um
  despertador no próprio endereço em vez de *polling*; a porta nova é presa
  antes de a velha ser solta, e a web é sempre o caminho de volta.
- **Diagrama ER** (pedido 127, primeira metade) — e `criar_tabela` passou a
  **declarar chave estrangeira pelo protocolo**, com o teste que trava que
  *declarar não é aplicar*. Sete defeitos de tela achados abrindo no navegador.
- **A camada SQL nasceu** (pedido 83): crate `phxsql-sql` (léxico, sintaxe,
  tradutor) e a op `sql` ligada **pelo portão que já existe** — com o teste
  `o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada`. Ligar achou o que a
  unidade não achava: `WHERE id = 2` chegava como texto; o motor alargou, o
  tradutor não apertou.
- **O catálogo de operações** (`op catalogo`): as 79 operações do protocolo
  descritas por dados — parâmetros, permissão, exemplo — com um teste que
  deriva a lista do próprio `despachar`. Ajuda escrita à mão não existe para
  envelhecer.
- **`phxsqlcmd`** (pedido 130): console interativo com `/help` e
  `/help comando` vindos do catálogo pela rede, autenticando pelo mesmo
  desafio-resposta da réplica.
- **Servidor MCP com transporte** (pedido 6): `phxsqld --mcp` por stdio, com o
  `tools/list` lendo o catálogo e a senha por variável de ambiente.
- **Cliente e dialeto PostgreSQL(R) no DbLink** (pedido 86): SCRAM-SHA-256
  conferido contra o RFC 7677, dialeto de SQL por motor, e as operações do
  DbLink reescritas para não saberem qual motor atendem. A prova contra um
  PostgreSQL(R) de verdade fica pendente e está dita.
- **`docs/CONCORRENTES.md` e `docs/CASSANDRA.md`** — o que cada motor faz na
  inserção, o que cabe aqui, o que não cabe e por quê. Do Cassandra, a
  resposta à pergunta do quórum: o OK de `QUORUM` **não significa disco** no
  modo padrão — significa recebido em W processos.

### Sabido

- **Excluir ainda perde** (6,27 contra 4,73 s) — próximo alvo.
- O `sincronizar` a cada 200 operações no servidor **dobra** o custo por linha
  (§4.9); tirá-lo do caminho muda o contrato de durabilidade e é decisão do
  Adriano.
- A prova do dialeto PostgreSQL(R) contra um servidor real está pendente.
- O editor visual do modelo (pedido 127, segunda metade) não começou.

---

## 0.17.0 — 2026-08-29'''
s=s.replace(anc,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('CHANGELOG ok')
