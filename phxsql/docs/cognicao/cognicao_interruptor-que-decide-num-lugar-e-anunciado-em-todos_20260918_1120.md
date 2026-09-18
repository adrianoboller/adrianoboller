# Cognição: o interruptor que decide num lugar só, e é anunciado como se decidisse em todos

**Assunto:** `cifra_fio.exigir`, o alcance real dele, e o campo de estado que o
contradiz.
**Descoberto em:** 18/09/2026, ~11h20.
**Frente:** cifra obrigatória da comunicação (ordem do dono, 18/09/2026).

---

## 1. O que aconteceu

A ordem do dono foi *«A comunicação deve obrigatoriamente ser cifrada»*, com os
dois meios decididos por ele: `cifra_fio.exigir` passa a nascer `true` (com
`"exigir": false` como escape escrito), e o navegador/REST ganham TLS por proxy
reverso na frente.

A troca do padrão parecia ser uma linha —
`crates/phxsql-server/src/config.rs`, o `Default` do `CifraFio`. Ela é uma
linha. O que ela **não** é, é uma linha que entrega o que anuncia:

- `cifra_fio.exigir` é lido em **um** lugar que decide alguma coisa:
  `crates/phxsql-server/src/servidor.rs:9270`, dentro do laço da porta de
  dados. Os outros casamentos de `cifra_fio.exigir` no repositório são
  comentário, espelho da resposta de estado e ajudante de teste.
- `crates/phxsql-server/src/servidor.rs:5884-5887` publica
  `encryption_exigida: true` dentro de `diretivas_da_conexao` — a função cuja
  documentação própria diz «o que é verdade DESTA conexão». Uma conexão HTTP em
  claro que faz a pergunta recebe `true`.

Ou seja: ligar o interruptor faz a porta de dados recusar o texto claro, **e**
faz o servidor dizer a todas as outras portas que a cifra é exigida quando elas
continuam atendendo em claro, com o mesmo token e o mesmo login.

## 2. O que eu concluí primeiro, e estava errado

**«Trocar o padrão é uma linha e o resto é conserto de testes.»** Dois erros
numa frase.

O primeiro: eu tratei a lista de testes vermelhos como o custo da mudança. Ela
é o custo **visível**. O custo que não aparecia em teste nenhum é que nenhum
teste da suíte pergunta «e as outras portas?» — os 60 que caem caem por
*conectarem em claro na porta de dados*, e nenhum deles cai por a porta HTTP
ter continuado aberta. A suíte inteira ficaria verde depois do conserto,
anunciando uma proteção que não existe. **Teste que passa por engano é pior que
teste que falta**, e aqui seriam sessenta.

O segundo: eu procurei o interruptor antes de procurar o que ele faz. É a lição
do Profiler pelo avesso — lá, o observador trabalhava antes de olhar o próprio
interruptor; aqui, o interruptor é olhado por um caminho só e **anunciado por
todos**. A pergunta certa não é «onde o campo é lido», é «quem **publica** o
campo sem ser quem o obedece».

## 3. O que a medição disse

Medido pelo inventário de segurança, no mesmo servidor e no mesmo instante, com
`cifra_fio.exigir: true`:

| pedido | resposta |
|---|---|
| porta nativa, linha JSON em claro | recusada: `[SP000025] … peca o aperto de mao com {"op":"cifrar"}` |
| `POST /api {"op":"login", …}` | **200**, com `"sessao"` e a senha em claro |
| `POST /api {"op":"ping"}`, mesmo token que a nativa recusou | **200** |
| `POST /v1/login` (REST) | **200**, com sessão |
| `POST /mcp initialize` | **200**, catálogo inteiro de ferramentas |
| `GET /` (explorador da especificação) | **200**, 14.009 bytes |
| `POST /api {"op":"desafio"}` | **200**, com `sal` e 210.000 iterações de um usuário nomeado |

E o tamanho da virada, medido por mim numa árvore verde (2.553 testes passando,
nenhum falhando): trocar **só** o padrão derruba **62** testes — **60** por
conectarem em claro na porta de dados, **2** por travarem o padrão de ontem de
propósito.

E uma correção de rota medida no meio: a premissa que chegou comigo dizia que
`replica.rs` consultava `cifra_fio.exigir`. **Não consulta — zero menções.** O
`grep` que a produziu casava `cifra_fio` **ou** `fio::`, e o que apareceu foi um
`use phxsql_core::fio::`. A réplica tem interruptor próprio e diferente
(`replicacao.origens[].cifra`), o cluster tem o dele (`cluster.cifra`) e a
interface→remoto o dela (`web.servidores[].cifra`), e **os três nascem
desligados**. `exigir` é *inbound-only*.

## 4. A regra

**Interruptor de segurança se mede pelo que ele RECUSA, nunca pelo que ele
publica — e quando um campo de estado o anuncia para um canal que ele não
alcança, o campo é o defeito, não a documentação.**

Corolário de operação: enquanto o alcance for menor que o anúncio, **o servidor
diz o alcance em voz alta no arranque**. Calar é a única opção que não existe.

## 5. Como está guardado hoje — e onde o buraco ficou

**Guardado:**

- `Config::avisar_o_que_viaja_em_claro` (`crates/phxsql-server/src/config.rs`)
  diz no arranque, quando `cifra_fio.exigir` está ligado e há porta HTTP no ar,
  que o interruptor vale **só** para a porta de dados, nomeia as portas que
  continuam em claro, e nomeia os três interruptores de saída que nascem
  desligados. Travado por
  `config::tests::exigir_a_cifra_do_fio_com_porta_http_no_ar_avisa_o_alcance` e
  pelo irmão que prova que sem porta HTTP no ar ele **não** aparece.
- O escape escrito, provado pelo soquete nos dois sentidos, em
  `crates/phxsql-server/tests/cifra-do-fio.rs`:
  `o_escape_escrito_deixa_o_cliente_em_claro_entrar` e
  `exigir_escrito_no_arquivo_recusa_o_texto_claro`. Os dois sobem de um
  `config.json` de verdade — teste que **escreve** o campo não prova o padrão
  dele, armadilha já paga neste mesmo arquivo.
- A porta web (e as duas do REST) presas ao laço local, provadas contra o
  sistema operacional em `crates/phxsql-server/tests/cifra-da-porta-web.rs`.
- O número e a tabela de medição em `docs/SEGURANCA.md` §7.0.

**O buraco, e ele está nomeado em vez de escondido:**

- **O padrão NÃO virou.** `CifraFio::default().exigir` continua `false`. Virar
  hoje seria entregar a metade que é pior que nada: o servidor passaria a
  anunciar `encryption_exigida: true` para conexões que ele não recusa.
- **O conserto mora fora desta frente:** `servidor.rs:5884-5887` (o campo que
  mente) e a recusa das portas HTTP, mais o escape escrito nos 60 testes que
  conectam em claro. `servidor.rs`, `rest.rs`, `mcp.rs`, `pg/` e `dblink/`
  estavam com outras frentes nesta rodada.
