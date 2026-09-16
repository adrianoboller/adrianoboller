# Auditoria SEC — criptografia, segredo e privacidade (16/09/2026)

**Papel:** SEC (revisor adversário), separado do engenheiro e da QA.
**Alcance:** `crates/phxsql-core/src/` (cifra, hash, senha, ed25519, hkdf,
sha1/512, keyenc, desafio, frogcript, x25519), `crates/phxsql-store/src/`
(cofre, reg, blob, log/lixeira/motivo/trilha) e os portões de
`crates/phxsql-server/src/` (servidor, usuarios, direito_coluna, profiler,
config).
**Método:** leitura + **prova viva**. Subi um `phxsqld` próprio em
`127.0.0.1:5399` num diretório temporário e o derrubei pelo PID que eu mesmo
criei; as provas de cripto rodaram em dois projetos `cargo` de rascunho que
dependem de `phxsql-core`/`phxsql-store` **por caminho**, sem tocar na árvore.
**Este documento não conserta nada.** Conserto e commit têm outro dono.

Regra que segui: **achado sem prova não é achado**, e achado que eu não
consegui reproduzir **não entra**. Dois candidatos morreram medidos e estão na
§7, porque recusa medida é resultado.

---

## 1. Achados priorizados

| # | Gravidade | Achado | Quem explora, com que acesso |
|---|---|---|---|
| **A1** | **Alta** | A cifra em repouso **não amarra o conteúdo de coluna externa (`.memo`/`.bin`) à linha**. Trocar o ponteiro entre duas linhas entrega o conteúdo confidencial de uma como se fosse da outra, **sem erro de etiqueta**. | Quem tem **os arquivos** e **não tem a chave** — disco levado, backup vazado, cópia noutra máquina: exatamente o modelo de ameaça que `cofre.rs` declara defender |
| **A2** | **Alta** | `procurar_texto` sobre índice FTS de uma coluna **negada pelo direito por coluna** é um oráculo de palavra: devolve a contagem e os rowids que casam. A peneira tira o valor e a resposta continua completa. | Usuário autenticado **com** `ler` na tabela e **sem** `ler` na coluna |
| **A3** | **Média** | A aleatoriedade inteira do produto degrada, em silêncio, para `SHA-256(nanos ‖ contador ‖ pid ‖ endereço de heap)` onde não há `/dev/urandom` — **Windows, alvo declarado**. Alimenta nonce de desafio, chave privada Ed25519, nonce SCRAM, sal do cofre e a semente do CSPRNG. | Quem estima instante de arranque e faixa de PID de um `phxsqld` em Windows |
| **A4** | **Média** | A sonda de **travessia de diretório** (violação grave, bloqueio na 1ª) lê `tabela`/`database`/`schema` e **não** os campos onde `juntar`, `unir` e `pivotar` escondem tabela. Medido: **358.680 sondas/min**, `blacklist.json` vazio. | Qualquer cliente com o token |
| **A5** | **Média** | `juntar` **abre a tabela antes** de conferir a permissão dela, e a mensagem distingue «não existe» de «acesso negado» → enumeração do catálogo por quem não pode lê-lo. | Usuário autenticado sem direito na tabela |
| **A6** | **Baixa** | O catálogo de guardas tem 143 entradas e **nenhuma** cobre quatro das cinco provas da pétrea «senha nunca em texto puro». Ninguém consegue dizer hoje se elas ainda pegariam o defeito. | — (é risco de processo, não de ataque) |
| **A7** | **Baixa** | Os **11 vetores do Apêndice A.3 da RFC 8439** (Poly1305) **passam** e **nenhum está na bateria**. O único vetor de Poly1305 no módulo é o §2.5.2. | — (correto hoje, desguardado amanhã) |
| **A8** | **Baixa** | `pbkdf2_sha256` com `c = 0` **não recusa**: devolve byte a byte o mesmo que `c = 1`. E `senha::destrinchar` só recusa `0` — **não há piso** para o hash de usuário, enquanto o cofre tem `ITERACOES_MINIMAS = 10_000` escrito com o motivo. | Operador que gera hash fraco; a fraqueza fica invisível |

---

## 2. A1 — a coluna externa não está amarrada à linha (ALTA)

### O que o documento promete

`docs/SEGURANCA.md` §11.11, *«A amarração ao endereço são DUAS fechaduras —
medido, e a ficha dizia uma»*: o endereço da linha entra **no nonce**
(`cofre::nonce_de_pedaco(rowid, volume, versao, tempero)`) **e no dado
associado** (`reg.rs:2287 aad_do_slot(volume, rowid, versao)`), e cada uma
segura sozinha — o teste só cai quando as duas somem.

E o comentário de `crates/phxsql-store/src/reg.rs:1740-1743` diz por que a
selagem é **uma por linha** e não uma por coluna:

> *«Cifrar cada coluna sozinha custaria 16 bytes por coluna marcada e
> **permitiria trocar a coluna A de uma linha pela coluna A de outra sem a
> etiqueta reclamar**.»*

### O que o código faz

`crates/phxsql-store/src/reg.rs:1691-1700`:

```rust
pub fn selar_externo(&self, coluna: u16, dados: &[u8]) -> Vec<u8> {
    …
    let mut nonce = [0u8; cifra::XNONCE_LEN];
    cifra::sortear(&mut nonce);                      // nonce SEM endereço
    let mut fora = nonce.to_vec();
    fora.extend_from_slice(&self.material.selar(&nonce, &coluna.to_le_bytes(), dados));
    fora                                             // AAD = SÓ o nº da coluna
}
```

e o par, `reg.rs:1827-1845`:

```rust
pub fn abrir_externo(&self, coluna: u16, guardado: &[u8]) -> Result<Vec<u8>>
```

**A assinatura já é o achado: não há rowid.** Para o conteúdo externo a
amarração ao endereço não é «única» — é **zero**. O nonce é sorteado inteiro e
guardado em claro ao lado; o AAD são dois bytes com o número da coluna. E a
selagem é exatamente a *uma por coluna* que o comentário duas telas acima
rejeitou pelo motivo certo.

O ponteiro que liga a linha ao bloco fica **fora** da proteção, por decisão
escrita (`reg.rs:2232-2240`, `faixas_pessoais` pula `col.ty.externo()`), e o
bloco do `.memo`/`.bin` não carrega rowid nenhum: o cabeçalho é
`[status u8][res 3][tamanho u32][crc32 u32][res 4][conteúdo]`
(`crates/phxsql-store/src/blob.rs:8-10`). Sobre o ponteiro só há **CRC-32**, que
não tem chave.

### O que um atacante ganha — prova viva, nos dois sentidos

Tabela `fichas(id Int8, nome Str(40) [Pessoal], obs Memo [Sensível])`, cofre
ligado, duas linhas. O atacante tem **os arquivos** e **não tem a senha**.
Ele troca os 16 bytes do ponteiro da coluna externa entre os dois slots e
refaz o CRC-32.

```
ANTES
  rowid 1: [Int(1), Str("Presidente"), Memo("SALARIO DO PRESIDENTE: 1.000.000"), …]
  rowid 2: [Int(2), Str("Estagiario"), Memo("salario do estagiario: 1.000"), …]

CONTROLE (recalcula o CRC e não troca byte nenhum)
  rowid 1: … Memo("SALARIO DO PRESIDENTE: 1.000.000") …      ← segue de pé
  rowid 2: … Memo("salario do estagiario: 1.000") …

TROCA (offset 49 do payload — o ponteiro; CRC refeito, chave NÃO usada)
  rowid 1 (Presidente) devolveu: Memo("salario do estagiario: 1.000")
  rowid 2 (Estagiario)  devolveu: Memo("SALARIO DO PRESIDENTE: 1.000.000")
```

**Nenhum erro.** Não é «a etiqueta não confere»; é a linha aberta e servida
como autêntica. O AEAD não reclama porque o AAD é `[2, 0]` nos dois lados.

A prova é nos dois sentidos de propósito: o **controle** (mesmo recálculo de
CRC, zero bytes trocados) passa, então o que derruba é a troca e não o meu
remendo do CRC. E a coluna `nome`, que é **inline** e marcada, continuou
abrindo — a etiqueta do slot verificou normalmente: o ataque é cirúrgico e não
precisa vencer a fechadura dupla, ele **contorna** a porta que não tem nenhuma.

O offset saiu de varredura (0..90) e **só o 49** produz troca limpa — é o
ponteiro, depois do bitmap de nulos de 1 byte. Qualquer outro offset cai na
faixa marcada e a etiqueta do slot reclama, como deve.

### Teste adverso que demonstraria (não escrito aqui — é do engenheiro)

Um `tests/cifra-dos-dados.rs` novo, no molde do
`o_texto_cifrado_mora_no_lugar_do_claro`:

1. cofre ligado, duas linhas com `Memo` marcado e conteúdos distinguíveis;
2. trocar os `PONTEIRO_LEN` bytes da coluna externa entre os slots e refazer
   o CRC-32 do slot;
3. `assert!(t.ler(2).is_err())` — hoje **passa** com `Ok`, e é isso que o
   teste tem de reprovar.

E o irmão que fecha o laço, no molde do que §11.11 já faz para o slot: tirar
`coluna.to_le_bytes()` do AAD de `selar_externo` **não derruba teste nenhum
hoje** — é a assinatura de amarração que não existe.

### O alcance, dito sem enfeite

O que continua valendo: quem **não** tem a chave continua **sem ler** o
conteúdo. O que se perde é a **integridade**, e ela é metade do que um AEAD
promete. Num banco com `.memo`/`.bin` de documento, laudo, foto ou anotação
clínica — que é onde o dado sensível de fato mora —, o atacante embaralha a
quem pertence cada um, e o banco confirma a mentira.

---

## 3. A2 — `procurar_texto` é oráculo sobre a coluna negada (ALTA)

### O que o documento promete

`docs/SEGURANCA.md` §15, *«A leitura: a peneira, e o que chega antes dela»*,
lista **seis** perguntas que recusam, porque *«a peneira sozinha não fecha o
caso: ela tira o valor DEPOIS de o filtro já ter respondido»*. `procurar_texto`
aparece na lista das que **a peneira cobre** (`direito_coluna.rs:104`,
`PorColuna::Le(Onde::Lista)`).

### O que o código faz

`servidor.rs:6111-6134`, `recusar_pergunta_sobre_coluna_negada`:

```rust
let indice = pedido.texto_ou("indice", "").trim();
…
let (chave, filtro) = self.colunas_do_indice(base, tabela, indice, sessao)?;
```

e `colunas_do_indice` (servidor.rs:6151-6180) resolve o nome contra o campo
**`"indices"`** da resposta de `esquema`. O índice de texto **não está lá**: ele
sai numa lista separada, `"indices_texto"` (servidor.rs:16284, alimentada por
`Schema::indices_de_texto()`). Resultado: `colunas_do_indice` devolve duas
listas vazias, nada casa, e a operação segue.

**É a pétrea da casa, na letra: o campo que o portão lê é o furo.** O portão
aprendeu a ler `indice`; quem não tem esse campo *naquela lista* passa.

### O que um atacante ganha — prova viva

Usuário `leitor`: `ler` em `b`, `ler` em `b.folha`, e
`colunas: { segredo: { ler: false } }`. Índice de texto `ftsegredo` sobre
`segredo`. Três linhas.

```
--- 1. varrer: a peneira tira a coluna? ---
[{"rowid":1,"id":1,"nome":"Ana",…},{"rowid":2,"id":2,"nome":"Beto",…},
 {"rowid":3,"id":3,"nome":"Cida",…}]                       ← "segredo" some. OK.

--- 2. onde pela coluna negada ---
acesso negado: a coluna "segredo" … e onde responderia sobre ela sem mostrá-la

--- 3. expressao pela coluna negada ---
acesso negado: a coluna "segredo" … e a expressao responderia sobre ela …

--- 4. procurar_texto no índice FTS da coluna negada ---
 palavra=confidencial   -> {"encontrados": 2, "linhas": [rowid 1 (Ana), rowid 3 (Cida)]}
 palavra=publico        -> {"encontrados": 1, "linhas": [rowid 2 (Beto)]}
 palavra=salario        -> {"encontrados": 3, "linhas": [1, 2, 3]}
 palavra=inexistente    -> {"encontrados": 0, "linhas": []}
```

O `onde` e a `expressao` recusam — as guardas de §15 funcionam. O
`procurar_texto` **responde**, e responde melhor que as duas: não são vinte
perguntas para um bit, é uma pergunta por palavra, com a lista exata de quem
casa. O conteúdo real da coluna era
`"salario alto confidencial"` / `"salario baixo publico"` — e o `leitor`
reconstruiu a partição inteira sem que a coluna aparecesse uma vez.

Pior que o `onde`: o `encontrados` **não passa pelo `max`**, é
`achado.rowids.len()` cru (servidor.rs:16896).

### Teste adverso que demonstraria

Irmão de `perguntar_pela_coluna_negada_recusa` (que hoje cobre `onde`,
`ordenar`, `colunas`, `expressao`, `tendo` e `indice`): tabela com
`indices_texto` sobre a coluna negada, `op: "procurar_texto"`, e
`assert!(r.is_err())`. Hoje **passa com `Ok`**.

E o laço que a casa gosta: um conferidor que exija que **toda** lista de
índice devolvida pelo `esquema` seja consultada por `colunas_do_indice` —
porque `"indices_texto"` foi a primeira lista nova, e não será a última.

---

## 4. A3 — a aleatoriedade fora do Linux (MÉDIA)

### O que o comentário promete

`crates/phxsql-core/src/senha.rs:118-124`:

> *«Tenta `/dev/urandom` primeiro. Onde ele não existe (Windows), cai numa
> mistura de relógio em nanossegundos, PID, endereço de heap … **O que um sal
> exige é ser ÚNICO por senha, não imprevisível, e a mistura garante isso.**»*

A frase é **correta para um sal** — e era verdadeira quando `bytes_aleatorios`
tinha um consumidor só.

### O que o código faz hoje

`senha::bytes_aleatorios` (senha.rs:92) ganhou seis consumidores, e a
justificativa não viajou com nenhum deles:

| consumidor | arquivo:linha | o que é |
|---|---|---|
| `desafio::nonce()` | `core/desafio.rs:169` | o nonce do desafio-resposta — **o anti-replay do login** |
| `ed25519::chave_nova()` | `core/ed25519.rs:648` | a **chave privada** Ed25519 |
| nonce SCRAM | `server/pg/scram.rs:177` | o nonce do protocolo PostgreSQL |
| sal do `Material` | `store/cofre.rs:352, 629` | o sal do PBKDF2 de **cada arquivo cifrado** |
| prefixo da `Sequencia` | `store/log.rs:281` | os 4 bytes do **nonce** do `.log` |
| semente do CSPRNG | `core/cifra.rs:623` | a semente de `cifra::sortear` → `tempero` do slot, nonce do `.reg`, x25519, FrogCript, UUID v4 |

Onde não há `/dev/urandom`, os seis viram
`SHA-256(nanos ‖ contador ‖ pid ‖ endereço de heap)` — algumas dezenas de bits
de entropia real, num alvo (`x86_64-pc-windows-gnu`, `docs/TECNOLOGIAS.md`:185)
que esta casa compila e empacota.

E o `cifra::sortear` é o caso mais grave, porque ele **semeia uma vez**: um
CSPRNG de fluxo com semente adivinhável entrega **toda** a sequência seguinte,
e é dela que sai o `tempero` que `reg.rs:1731-1736` descreve como *«o que
segura o nonce diferente quando a versão repete»* — a única defesa contra o
reuso de par (chave, nonce), que o próprio `cifra.rs:388` chama de *«a falha que
quebra a cifra»*.

### O choque de pétrea, que aparece em vez de ficar calado

`std` não expõe gerador criptográfico, e **zero dependências externas é
pétrea**. É o mesmo formato do choque do TLS (`SEGURANCA.md` §7.1): o
**comportamento** (aleatoriedade imprevisível em todo alvo suportado) é
obrigatório; o **meio** não passa sem o dono. A diferença deste para o do TLS é
que aqui há um caminho que **não** fura a pétrea — `BCryptGenRandom` /
`RtlGenRandom` são API do sistema, alcançáveis por `extern "system"` sem
*crate*, do mesmo jeito que o SHA-256 desta casa é norma lida e reescrita. Mas
puxar API de plataforma é decisão de arquitetura: **nomeio, não decido.**

### Teste adverso que demonstraria

O que depende do sistema operacional se prova contra o sistema operacional:
rodar o binário Windows sob Wine/VM, colher 1.000 `desafio` de processos
recém-nascidos e medir a entropia dos nonces; e, em Linux, um teste que force o
caminho de fallback (`sal_por_mistura` exposto ao teste, ou `/dev/urandom`
indisponível num namespace) e exija que `bytes_aleatorios(32)` de dois
processos com PID e instante próximos **não** colidam em nenhum prefixo.

---

## 5. A4 e A5 — o portão de travessia e o `juntar` que abre antes de perguntar

### A4 — a sonda grave só existe no campo que o portão lê

`servidor.rs:8818-8836` confere `nome_hostil` em **três** campos:

```rust
for (rotulo, valor) in [
    ("database", &base),
    ("tabela",   &pedido.texto_ou("tabela", "").to_string()),
    ("schema",   &pedido.texto_ou("schema", "").to_string()),
] { … violacao_grave(ip, &op, "tentativa de travessia de diretorio") … }
```

A conferência de **permissão** já foi copiada para as quatro operações que
escondem tabela (`op_juntar` 19529-19543, `op_unir` 19854-19863, `op_pivotar`
11886-11897, `op_diferencas` 19664-19673). A sonda de **travessia** foi copiada
para **uma só**: `op_diferencas` (servidor.rs:19675-19682, com o comentário
certo — *«os dois nomes chegam por campos que o `despachar` não olha»*). As
outras três ficaram.

**Medido, no soquete:**

| caminho | sondas | bloqueios |
|---|---|---|
| `{"op":"varrer","tabela":"../../../../etc/passwd"}` | **1** | IP bloqueado por 60 min, `blacklist.json` gravado |
| `{"op":"juntar","a":{"tabela":"../../../../etc/passwd"}}` | **200** | **0** — `blacklist.json` nem chegou a existir |
| rajada pelo mesmo caminho | **17.934 em 3 s = 358.680/min** | **0** |

A travessia em si **não acontece** (o `validar_nome` do store recusa `/` e `\`,
`store/catalogo.rs:120-135`) — o furo não é o arquivo, é a **contagem**: quem
sonda pelo campo escondido sonda a noite inteira sem nunca ser barrado, que é
exatamente o que o §3 do `SEGURANCA.md` diz que a gravidade existe para
impedir. E paga-se em disco: as 17.934 sondas escreveram **7,2 MB** de
`acessos.log` — alavanca de enchimento de partição de graça.

**Teste adverso:** o irmão de `nome_hostil_nao_passa`, mas pelo soquete e
contando o `blacklist.json`: uma sonda por `a.tabela` tem de deixar a mesma
marca que uma por `tabela`. Hoje deixa zero.

### A5 — `juntar` abre a tabela antes de perguntar se pode

`servidor.rs:19524-19527`:

```rust
let (na, nb) = (pa.texto_ou("tabela", ""), pb.texto_ou("tabela", ""));
let mut ta = db.abrir_qualificada(na)?;      // <-- abre
let mut tb = db.abrir_qualificada(nb)?;      // <-- abre
let (ea, eb) = (ta.esquema().clone(), tb.esquema().clone());

// O portao geral confere o campo `tabela` … Sem esta conferencia, juntar
// seria a porta dos fundos para ler uma tabela negada …
if let Some(u) = &sessao.usuario { … }       // <-- só agora pergunta
```

O portão 4 do `portoes_do_pedido` (servidor.rs:9183-9196, o da reserva de
carga) declara o princípio oposto, com todas as letras: *«Depois do de permissão, e
não antes: **quem não pode nem ler a tabela não precisa descobrir que ela está
em carga**»*. Aqui a ordem está invertida.

**Prova viva.** `carlos`: `ler` em `b`, negado em `b.sigilosa`.

```
varrer sigilosa            -> acesso negado: carlos nao tem permissao de ler em b.sigilosa
juntar a.tabela=sigilosa   -> acesso negado: carlos nao tem permissao de ler em b.sigilosa
juntar a.tabela=nao_existe -> nao encontrado: a tabela nao_existe_mesmo nao existe em b
```

As duas mensagens são diferentes: quem não pode ler `b.sigilosa` **descobre que
ela existe**, e enumera o catálogo inteiro do banco a 358.680 perguntas por
minuto (A4 tira o freio). Ainda paga o `read_dir`, o `.reg` e o `.ndx` de uma
tabela que a sessão não tem direito de tocar — portanto também é alavanca de
E/S.

**Teste adverso:** `juntar` com `a.tabela` numa tabela negada e `juntar` com
`a.tabela` numa tabela inexistente têm de devolver **a mesma** recusa, e o
`abrir_qualificada` não pode ter sido chamado. Hoje as duas divergem.

---

## 6. O que promete e o que entrega — a tabela

| Onde | O documento/comentário promete | O código faz | O que o atacante ganha com a diferença |
|---|---|---|---|
| `SEGURANCA.md` §11.11 | «a amarração ao endereço são **DUAS** fechaduras … cada uma segura sozinha» | vale para o slot; para `.memo`/`.bin` são **zero** (`reg.rs:1691`) | **A1** — troca de conteúdo entre linhas sem erro de etiqueta |
| `reg.rs:1740-1743` | selar por coluna «permitiria trocar a coluna A de uma linha pela coluna A de outra sem a etiqueta reclamar» — por isso **não** se faz | `selar_externo` faz exatamente isso | **A1**, provado |
| `SEGURANCA.md` §15 | «as **seis** recusam» — a peneira não fecha sozinha e por isso a pergunta também recusa | `procurar_texto` sobre índice FTS **não** recusa | **A2** — oráculo de palavra sobre coluna negada |
| `senha.rs:123` | «um sal exige ser ÚNICO, não imprevisível, e a mistura garante isso» | a mesma função semeia CSPRNG, chave Ed25519 e nonce de login | **A3** — em Windows, sementes estimáveis |
| `SEGURANCA.md` §3 | travessia de diretório = **grave**, bloqueia na 1ª tentativa | vale por `tabela`/`database`/`schema`; por `a.tabela`, `tabelas[]` e `juntar[].tabela`, **nunca** | **A4** — 358.680 sondas/min, zero bloqueios |
| `portoes_do_pedido` portão 4 | «quem não pode nem ler a tabela não precisa descobrir que ela está em carga» | `op_juntar` abre a tabela **antes** de perguntar | **A5** — enumeração do catálogo |
| `cifra.rs:120-125` | «**# Pânico** / Não há: o contador **satura** em vez de dar a volta» | `contador.checked_add(1).expect("mensagem maior que o teto…")` — **entra em pânico** | nada hoje (exige 256 GiB numa mensagem, e o fio corta em 128 MiB), mas é doc que desmente código na função mais sensível do módulo |
| `cofre.rs:57-58` | `ITERACOES_MINIMAS = 10_000` — «abaixo disto o PBKDF2 vira enfeite» | o hash **de usuário** não tem piso: `senha::destrinchar` só recusa `0` | **A8** — hash de 1 iteração passa calado, e o `desafio` publica `iteracoes` a quem tem o token |

E o que **não** diverge, e merece registro porque eu fui procurar: `Usuario::ficha`
(`usuarios.rs:902`) e `Config::para_json` (`config.rs:3431`) são **lista de
permitidos** — montam campo a campo, não apagam campo de uma árvore. É o padrão
certo, é o que a pétrea «analisando, nunca recortando» pede, e é o que faz o
`config_gravar` devolver a configuração inteira sem segredo. `profiler::limpar`
(`profiler.rs:973`) também analisa a árvore e reserializa, em qualquer
profundidade, e `sql_sem_senha` acha a senha **dentro da frase** por léxico do
próprio motor. Nenhum dos três recorta texto.

---

## 7. Os vetores oficiais: cobrem o que dizem cobrir?

Bateria rodada: `cargo test -p phxsql-core --lib` → **350 passam, 0 falham,
1 ignorado**. Contagem por módulo e norma citada:

| módulo | testes | normas citadas no fonte |
|---|---|---|
| `cifra.rs` | 17 | RFC 8439, draft-irtf-cfrg-xchacha-03 |
| `ed25519.rs` | 11 | RFC 8032 |
| `hash.rs` | 9 | FIPS 180-4, RFC 2104, RFC 2898, RFC 4231 |
| `hkdf.rs` | 6 | RFC 5869 |
| `x25519.rs` | 8 | RFC 7748 |
| `sha512.rs` | 4 | FIPS 180-4 |
| `sha1.rs` | 3 | FIPS 180-4 |
| `senha.rs` | 10 | **nenhuma** |
| `desafio.rs` | 8 | **nenhuma** |
| `keyenc.rs` | 7 | **nenhuma** |
| `frogcript.rs` | 10 | RFC 8439 (herdada da primitiva) |

### O que passou, e por isso NÃO é achado (recusa medida)

**Poly1305, RFC 8439 Apêndice A.3 — os 11 vetores passam, 11/11.** Rodei todos
contra `cifra::poly1305`, inclusive os que existem justamente para quebrar
aritmética de membros de 26 bits escrita à mão: `r = 0` (#2), `s = 0` (#3),
`2^130-5` (#5), `2^130-6` (#6), `5·(2^130-1)` (#7 e #8), `2^130-1` (#9) e os
dois de propagação de vai-um entre membros (#10 de 64 B e #11 de 48 B).

> Aviso de método, porque eu mesmo quase publiquei um defeito que não existe:
> na primeira passada eu casei a entrada do vetor #10 com a etiqueta esperada
> do #11 e o programa gritou FALHA. A implementação devolveu
> `14000000000000005500000000000000`, que é **exatamente** a etiqueta certa do
> #10. *Diagnóstico plausível não é diagnóstico medido* — o defeito era do meu
> vetor.

**Ed25519 — as arestas que não têm vetor, mas têm comportamento certo.** Medi
as três que a RFC 8032 §5.1.3 exige e que `vetores_da_rfc_8032` não exercita:

```
assinatura boa confere ......................................... true
chave pública TODA ZEROS confere assinatura de outrem .......... false
R = p (codificação NÃO canônica, y ≥ 2²⁵⁵-19) confere .......... false
A = p (codificação NÃO canônica) confere ....................... false
```

As três recusam. Os cinco vetores de §7.1 (TEST 1, 2, 3, 1024, SHA(abc)) estão
todos presentes, e a maleabilidade `S ≥ L` de §5.1.7 tem teste próprio.

### Os casos-limite que NÃO têm vetor — e o que cada falta esconderia

| Norma, seção | Caso-limite | Está? | O que a falta esconderia |
|---|---|---|---|
| **RFC 8439 §A.3** | os **11** vetores do Poly1305 sozinho | **não** (só o §2.5.2, uma mensagem, sem aresta de vai-um) | passam hoje, medido. Uma reescrita dos membros de 26 bits (ou um alvo sem `u64` rápido) quebraria **em silêncio**: a única prova viva é uma mensagem que não toca redução nem carry |
| **RFC 8439 §A.5** | o AEAD completo na **decifragem**, com AAD longo | **não** | só o §2.8.2 (selagem) está provado contra vetor |
| **RFC 4231** | são **7** casos (§4.2–§4.8); estão os **1, 2, 3 e 6** | **faltam 4, 5 e 7** | o **7** é o que importa: chave de 131 B **com mensagem de 152 B** — pré-hash da chave **junto** com mensagem multibloco. O 6 tem a chave longa mas mensagem curta, e não exercita a contagem de comprimento do hash interno. O 5 é truncagem a 128 bits, que não se aplica aqui (a função devolve 32 B sempre) — vale escrever que foi dispensado, em vez de faltar calado |
| **RFC 6070 / PBKDF2** | **não existe vetor oficial de PBKDF2-HMAC-SHA256.** Os quatro usados (`password`/`salt` com c=1, 2, 4096 e o de 40 B) são a transposição de fato dos casos SHA-1 da RFC 6070 | usados como se fossem oficiais | a casa exige «vetor oficial»; aqui o que há é **de facto**, e o `hash.rs` não diz isso. Falta também o **caso 6 da RFC 6070** (`pass\0word` / `sa\0lt`), o que pega truncagem em NUL — medi: **não trunca**, mas nada guarda isso |
| **RFC 2898 §5.2** | `c = 0` deve ser recusado (`c` é inteiro **positivo**) | **não recusa** | medido: `c=0` devolve **byte a byte** o mesmo que `c=1`. Ver **A8** |
| **FIPS 180-4** | comprimento **exatamente** na fronteira de bloco (55/56/63/64/65 B) para SHA-256 | **não** (o `sha512.rs` tem `na_fronteira_do_bloco`; o `sha256` não) | o 56 é onde o enchimento precisa de um segundo bloco. O `sha256_alimentado_em_pedacos` quebra uma mensagem de **48 B**, então nunca chega lá. Medi os sete comprimentos: saem valores estáveis e o `update` em pedaços bate — mas **sem vetor externo** não é prova, é coerência consigo mesmo |
| **RFC 8439 §2.3** | contador de 32 bits **estourando** | **não** | o código entra em **pânico** (`.expect`) onde o comentário promete saturação. Inalcançável hoje; é doc que mente |
| **AEAD, geral** | **nonce repetido** com a mesma chave | **não há teste que o proíba** | `a_sequencia_nunca_repete_nonce` prova que a `Sequencia` não repete — não prova o que acontece se repetir, nem cobre os dois caminhos que **sorteiam** nonce (`selar_externo`, `frogcript.rs:222`) |
| **AEAD, chave** | chave toda-zeros | parcialmente: o A.3 #1 a usa para Poly1305 | não há caso de `selar`/`abrir` com chave toda-zeros; o cofre recusa senha vazia (`cofre.rs:203`), o que cobre o caminho de produção |

---

## 8. A6 — as pétreas de segurança e as guardas que faltam

Medido: `bancada/guardas/catalogo.py` tem **143** guardas; **20** tocam
senha/segredo/cifra. Das provas da pétrea «senha nunca em texto puro»:

| prova | arquivo | no catálogo |
|---|---|---|
| `profiler::a_senha_nunca_aparece` | `profiler.rs:1050` | **sim** |
| `usuarios::a_ficha_nunca_devolve_a_senha` | `usuarios.rs:1799` | **não** |
| `servidor::a_senha_nunca_aparece_no_arquivo_nem_na_resposta` | `servidor.rs:31111` | **não** |
| `profiler::a_senha_dentro_do_texto_sql_tambem_sai` | `profiler.rs:1015` | **não** |
| `servidor::o_segredo_nao_entra_nem_sai` | `servidor.rs:32068` | **não** |
| `usuarios::senha_em_texto_puro_funciona_mas_avisa` | `usuarios.rs:1784` | **não** |

O lado do Profiler está guardado; **os outros cinco não**. Isso não quer dizer
que estejam errados — exercitei os quatro lugares e os quatro seguram:

```
usuarios / quem_sou / config / usuario_criar / usuario_alterar /
acessos / sessoes / jobs / dblink / diretivas
    → hash: não   pbkdf2: não   senha: não   token: não

anel do Profiler (11 eventos, incluindo usuario_criar e usuario_alterar
com senha e nova_senha em claro no pedido)
    → hash: não   senha: não   nova_senha: não   token: não

no disco, depois de tudo: o único arquivo que contém o hash é o
config.json (por desenho). acessos.log e diretivas.log: limpos.
```

Quer dizer outra coisa, e é o que a casa cobra: **ninguém consegue dizer hoje
se elas ainda falhariam com o defeito reposto.** É a pergunta que o
`provar-guardas.py` existe para responder, e sobre as cinco ele não responde.

---

## 9. O que os quatro motores maduros fazem e nós não — e o preço

| Prática | PG / MariaDB / MySQL / SQLite | PhxSql hoje | Custo, e o choque quando há |
|---|---|---|---|
| **Cifra do canal (TLS)** | os **três** maduros cifram | Noise próprio na 5000 (§7), proxy TLS para o resto (§7.1) | **choque conhecido, já na mesa e já decidido pelo dono**: comportamento entra como meta, meio (crate de TLS) não passa. Nada a acrescentar |
| **Aleatoriedade do sistema** | os quatro usam a API do SO | `/dev/urandom`, e **mistura** onde ele não existe | **choque novo, e este eu levanto** — ver **A3**. Diferente do TLS: há caminho que não fura a pétrea (API do sistema por `extern "system"`, sem *crate*). Decisão do dono |
| **Cifra em repouso (TDE)** | PG por extensão, MariaDB/MySQL nativo por tablespace | por coluna marcada, ChaCha20-Poly1305 (§11) | **já temos, e mais fino que os três.** O que falta é a amarração de **A1** |
| **Rotação de chave** | MariaDB/MySQL têm *key rotation* com versão de chave no cabeçalho da página | **não há**: trocar a senha do cofre não reescreve nada, e `definir_com` esvazia o cache para não abrir com a chave velha (`cofre.rs:219-226`) | o formato **já comporta**: cada arquivo carrega o próprio sal e as próprias iterações (`MATERIAL_LEN`, cofre.rs:300-308). Falta a migração que reescreve slot a slot — e ela está nomeada e **não entregue** em §13.6. É item de DBA (mudança de formato entra cedo), não de SEC |
| **Derivação por tabela / por banco** | MySQL por tablespace | uma senha do processo; o parecer medido está em §12.5 e §13.7 e **recusa** senha por tabela com número | recusa medida — não reabro |
| **Autenticação do canal** | SCRAM com *channel binding* (`SCRAM-SHA-256-PLUS`) | existe: `amarrar_canal` + `cifra_fio.exigir_amarra` (servidor.rs:9330-9375) | **convergimos**, e com o mesmo cuidado dos três: a transcrição é a da conexão, nunca a do pedido |
| **Piso de custo do KDF** | os quatro recusam parâmetro degenerado | cofre sim (10.000), hash de usuário **não** | **A8** |

---

## 10. Trilha de dado pessoal (LGPD) — o que exercitei

A pergunta da casa é *«a peneira tira o valor depois de o filtro já ter contado
as linhas que casam; vinte perguntas dizem o dado sem ele nunca aparecer»*. O
desenho de `direito_coluna.rs` responde isso bem e por princípio — a tabela
`CLASSES` é **exaustiva** contra o catálogo (teste
`a_lista_e_o_catalogo_sao_a_mesma_lista`, direito_coluna.rs:492) e o padrão de
operação desconhecida é `PorColuna::Recusa` (direito_coluna.rs:322-328), que é
o lado certo de errar.

Exercitei os caminhos de pergunta, um a um:

| caminho | resultado |
|---|---|
| `varrer` — a peneira | tira a coluna. **OK** |
| `varrer` + `onde` na coluna negada | **recusa**, nomeando |
| `varrer` + `expressao` na coluna negada | **recusa**, nomeando |
| `varrer` + `ordenar` na coluna negada | **recusa**, nomeando |
| `varrer` + `ordem` na coluna negada | ignorado — `varrer` não lê esse campo. Sem vazamento |
| `consultar` + `ordem` na coluna negada | recusa: a coluna já não existe no resultado do sub-pedido |
| `consultar` + `expressao` na coluna negada | idem |
| `sql` `SELECT … WHERE segredo = …` | recusa (por índice inexistente, antes do direito) |
| `juntar` numa tabela com regra de coluna | **recusa** — `PorColuna::Recusa` |
| **`procurar_texto` sobre índice FTS da coluna negada** | **RESPONDE — achado A2** |

Ou seja: os seis casos que §15 lista estão fechados e os fechei por prova, não
por leitura. O sétimo, que §15 não lista, é o **A2**.

Duas notas de privacidade que não são achado e ficam registradas porque a
pergunta volta: o `.ndx` sobre coluna marcada **continua em claro** — está
medido (200 de 200 pares legíveis, §13.6) e tem teste que trava o fato
(`o_indice_sobre_a_coluna_marcada_continua_em_claro`). E o gap «coluna externa
marcada **sozinha** ia em claro» (pedido 210) **está fechado**, com teste
próprio (`coluna_externa_marcada_sozinha_nao_pode_ir_em_claro`,
`tests/cifra-dos-dados.rs:638`) — confirmei. O A1 é outro problema, na mesma
coluna: aquele era **confidencialidade**, este é **integridade**.

---

## 11. O que eu bloqueio

Como revisor adversário, e preferindo sempre a saída mais conservadora:

- **A1 e A2 bloqueiam.** São exposição de dado — um de integridade de dado
  pessoal cifrado, outro de acesso cruzado a coluna negada — e os dois estão
  provados com entrada e efeito. Não há caminho em que «é teórico».
- **A3 bloqueia para o pacote Windows**, e só para ele: no alvo Linux não é
  alcançável. Enquanto não houver decisão do dono, um binário Windows não
  deveria sair anunciando as mesmas garantias criptográficas do de Linux.
- **A4, A5, A6, A7 e A8 não bloqueiam** — são endurecimento e disciplina de
  prova, e entram pelo fluxo normal de pendência.

E o que **não** bloqueia e eu quero dito em voz alta, porque um revisor
adversário que só lista defeito mente por omissão: as duas pétreas de segredo
estão **sustentadas no código** pelo padrão certo — lista de permitidos e
análise com reserialização, nunca recorte —, e eu não achei um único lugar em
que senha, hash ou token saiam por arquivo, log ou resposta de protocolo. O que
falta ali é **guarda**, não conserto (A6).

---

## 12. Como reproduzir

Nada disto tocou a árvore do repositório. O servidor de prova subiu num
diretório temporário e foi derrubado **pelo PID que esta sessão criou**;
nenhum processo alheio foi tocado.

```bash
# bateria de cripto da casa
cargo test -p phxsql-core --offline --lib          # 350 passam, 0 falham

# servidor de auditoria (porta 5399, diretório temporário, config próprio)
phxsqld --config config.json                        # com o usuário `leitor`,
                                                    # colunas: { segredo: { ler: false } }
# A2
{"op":"procurar_texto","database":"b","tabela":"folha",
 "indice":"ftsegredo","palavra":"confidencial"}

# A4 — a assimetria, no soquete
{"op":"varrer","tabela":"../../../../etc/passwd"}                 # bloqueia na 1ª
{"op":"juntar","a":{"tabela":"../../../../etc/passwd"},…}          # 200x, zero bloqueios

# A5
{"op":"juntar","a":{"tabela":"<negada>"},…}   vs   {"a":{"tabela":"<inexistente>"}}
```

As provas de cripto (A.3 do Poly1305, arestas do Ed25519, PBKDF2 `c=0`,
fronteira de bloco do SHA-256) e a troca de ponteiro de A1 rodaram em dois
projetos `cargo` de rascunho com `phxsql-core` e `phxsql-store` **por
caminho** — a receita cabe em vinte linhas de `Cargo.toml` e está descrita nas
seções correspondentes, para que a próxima sessão a refaça em vez de
redescobri-la.
