# A senha dentro da FRASE escapa da lista de nomes

*Descoberto em 07/09/2026, 17:58, exercitando `bancada/usuarios/provar.py`
contra o motor vivo — pedido 221.*

## 1. O que aconteceu

O Profiler redige por **nome de campo**: `SEGREDOS`, em
`crates/phxsql-server/src/profiler.rs`, lista `senha`, `senha_b64`,
`senha_hash`, `nova_senha`, `prova`, `token`, `chave`… A lista está certa, é
por nome e não por heurística, e o comentário dela explica por quê — adivinhar
o sensível pelo formato do valor erra nos dois sentidos, e errar para o lado de
mostrar é irreversível.

O pedido 221 acrescentou `usuario_criar` em JSON — coberto pela lista, porque
a senha vai num campo chamado `senha` — **e** os comandos SQL:

```sql
CREATE USER carlos PASSWORD 'a-senha-do-carlos';
```

Que chegam assim:

```json
{"op":"sql","texto":"CREATE USER carlos PASSWORD 'a-senha-do-carlos'"}
```

Aqui a senha **não está num campo**. Está no meio de uma frase, num campo
chamado `texto` — que é exatamente o que o Profiler existe para mostrar. Com o
Profiler ligado, a senha ia inteira para o anel e para o `perfil.txt`.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o caminho SQL estava coberto, porque eu **já tinha escrito** a
redação do lado da resposta: `phxsql_sql::usuario::sem_a_senha`, que analisa o
comando e devolve `PASSWORD '***'`. Havia teste, e o teste passava.

O erro é de **alcance**, e é o de sempre nesta casa: a redação que eu escrevi
cobria a **resposta** da op `sql`. O Profiler não captura a resposta — ele
captura o **pedido**, antes de qualquer coisa acontecer, e por outro caminho.
Dois lugares, uma redação só, e a que faltava era a do lado por onde a senha
entra.

E há um agravante que vale registrar: o teste unitário
`o_profiler_tapa_a_senha_do_pedido_de_criar_usuario` passava, verde, sobre o
pedido **JSON**. Ele provava a metade que já estava certa.

## 3. O que a medição disse

A parte 8 da bancada liga o Profiler, manda um `usuario_criar` e um
`ALTER USER … PASSWORD '…'` pela op `sql`, e varre o anel:

```
=== 8. O Profiler LIGADO, que e onde a senha apareceria
  [OK   ] profiler_ligar respondeu ok
  [FALHA] o anel do Profiler nao traz senha nenhuma -- ['a-senha-trocada-2026']
  [OK   ] e ele MOSTRA o pedido, com o campo tapado
```

Uma senha, em claro, no anel — e a mesma varredura dizendo «OK» para o campo
JSON, que estava tapado. **Duas afirmações no mesmo lugar, e só uma delas
falhou**: é o retrato do alcance parcial.

O conserto é por **análise**, não por recorte, e não tapa o campo inteiro:
`texto`/`sql` só paga o léxico quando as duas primeiras palavras são
`<CREATE|ALTER|DROP> USER` — o portão vem antes do trabalho. Tapar o `texto`
inteiro deixaria o Profiler cego para todo SQL, que é o uso principal dele; há
teste para esse lado também (`o_sql_de_sempre_continua_visivel_no_anel`, com
`WHERE cidade = 'Blumenau'` tendo de continuar legível).

Prova real, com o defeito reposto (a chamada a `sql_sem_senha` removida de
`limpar`):

```
thread 'profiler::testes::a_senha_dentro_do_texto_sql_tambem_sai' panicked at:
  a senha ficou no anel: {"op":"sql","texto":"CREATE USER c PASSWORD 'segredo1'"}
```

## 4. A regra

**Quando uma funcionalidade nova puser um segredo DENTRO de um texto livre,
procure quem mostra texto livre — a lista de campos secretos não o alcança.**

O corolário, que é o que dói: **redação escrita numa ponta não cobre a outra.**
Redigir a resposta e redigir o pedido são dois trabalhos, e o teste que prova
um passa alegremente sobre o outro.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/profiler.rs`, `sql_sem_senha` — a redação por
  análise dos campos `texto`/`sql`, com o portão de duas palavras antes do
  léxico.
- `crates/phxsql-sql/src/usuario.rs`, `sem_a_senha` e `e_de_cadastro` — a
  análise em si, provada contra três formas de escrita (espaçamento variado e
  `''` desdobrado) e contra o texto que o léxico recusa, que vira o tamanho em
  bytes em vez de virar texto.
- Dois testes nos dois sentidos: `a_senha_dentro_do_texto_sql_tambem_sai` e
  `o_sql_de_sempre_continua_visivel_no_anel`.
- `bancada/usuarios/provar.py`, parte 8 — contra o motor vivo, com o Profiler
  ligado, que é onde isto foi achado.

**Onde o buraco ficou.** A varredura é por *função conhecida*: hoje só os três
comandos de cadastro põem segredo dentro de frase. No dia em que outro comando
SQL carregar um — um `ALTER SERVER SET token = '…'`, por exemplo — ele passa
inteiro, e nada acusa. Não há conferidor genérico para isso, e não haveria como
haver sem o Profiler interpretar linguagem: o que existe é esta regra escrita e
o hábito de exercitar com o Profiler **ligado**.
