# SEC — o que AINDA falta nos pedidos de segurança abertos/parciais (01/10/2026)

Papel SEC (revisor adverso, não autor). Conferido **contra o código de hoje**
(`HEAD 93fd1604`), achado por achado. Não edita código, `PENDENCIAS.md` nem
dossiê. Linha = arquivo:linha lida hoje; «prova» = leitura citada ou comando.

Legenda: **ATIVO** = defeito ativo, entra na conta · **⏸** = pode virar «depois
da versão» pela regra (A) do `CLAUDE.md` · **FECHADO** = consertado, com prova.

## Placar

| pedido | estado medido | ativos | ⏸ | fechados |
|---|---|---|---|---|
| 268 | migração não existe; honesta por tabela | 0 | 1 | 0 |
| 284 | aberto inteiro | 1 | 0 | 0 |
| 339 | (a) e (b) fechados; (c) é do dono | 0 | 1 | 2 |
| 344 | os três casos abertos | 2 | 0 | 0 |
| 345 | fechável | 0 | 0 | 1 |
| 357 | §11.7 ainda vale; redação por grau e aviso não entraram | 1 | 2 | 0 |
| 359 | protocolo ainda vaza; disco 0600 consertado | 1 | 1 | 1 |
| 436 | M4, M5, B4, B5 abertos; B1, B2 consertados | 4 | 2 | 2 (+M1–M3) |
| **total** | | **9** | **7** | |

Ordem sugerida para a onda: 344-1 → 359-a → 436-M5 → 284 → 357-§11.7 → 344-2 →
436-M4 → 436-B5 → 436-B4.

---

## 268 — `Criptografar`/`Descriptografar`

**268-1 · migração de tabela existente · ABERTO · média · ⏸.**
Não existe op, SQL nem CLA: `grep -rniE "criptografar|descriptografar" crates`
dá 2 linhas, as duas comentário (`config.rs:1732`, `:1740`). Marcar coluna em
tabela já cifrada **recusa** (`reg.rs:1220-1235`, «essa operação não existe»);
em tabela em claro, a marca entra e o `.reg` continua em claro.
Por que ⏸ e não ativo: a falta está **dita por tabela**, e não só na lista. O
esquema devolve `material: "cifrado"|"em_claro"` (`servidor.rs:1536-1542`,
`material_da_tabela`), o comentário do campo diz que a lista só DECLARA, e o
rodapé da tela também (pedido 196). É funcionalidade que falta, não garantia
que finge valer.
**Conserto mínimo, quando entrar:** reescrever slot a slot no molde do
`acrescentar_coluna`/`aplicar_troca` (`table.rs`, `TrocaDoEsquema`), com o
volume 1 como ponto de compromisso do `ALTER`. É do papel C, e o desenho da
§13.6 não muda.
**Barato e opcional, já agora:** um aviso de arranque, uma linha por tabela de
`cifra.tabelas` cujo `RegFile::cifrada()` é falso.

---

## 284 — `replicas_autorizadas` atrás de proxy/NAT

**284-1 · ABERTO · média · ATIVO** (é uma garantia de acesso que não vale na
montagem comum, e a documentação não avisa).
- O portão 2a-bis compara `sessao.ip` com a lista (`servidor.rs:11576-11583`).
  Pela porta HTTP, esse IP é o par do soquete, isto é, o **proxy**.
- O servidor **sabe** quando está atrás de proxy, porque
  `proxy_desta_porta_http` (`servidor.rs:9705-9710`) lê `web/rest.atras_de_proxy`.
  O portão de réplica não consulta isso.
- `REPLICACAO.md` §7 (`:420-470`) continua sem a ressalva, e o aviso de
  arranque (`servidor.rs:2423`) só fala da lista **vazia**.

**Exploração:** `replicas_autorizadas:["127.0.0.1"]` e `web.atras_de_proxy:true`.
Qualquer cliente de fora com o token faz `POST /api {"op":"replicar"}` pelo
proxy. O portão vê `127.0.0.1` e entrega o diário, que leva a linha inteira com
`imagem_da_linha`.

**Conserto mínimo:** no portão 2a-bis, quando a sessão veio por família HTTP
cujo `proxy_desta_porta_http(...).0` é `true` e a lista não está vazia, recusar
`OPS_DE_REPLICACAO`. O IP ali é o do proxy, e a réplica tem a porta de dados.
Juntar uma frase na §7 e uma linha no aviso de arranque. Trancar por credencial
é o conserto de fundo e fica com o dono.

**Teste adverso:** `replicar_pela_porta_web_atras_de_proxy_nao_passa_pela_lista`.
Subir com a configuração acima e pedir `replicar` por `/api` a partir de
127.0.0.1. Hoje passa; tem de recusar. O irmão `sem_replicas_autorizadas_nada_muda`
continua verde.

---

## 339 — parecer externo (guarda-chuva)

- **(a) FECHADO** (23/09): a chave da Claude está no `sessionStorage`, e os
  campos `#s/#t/#k` são limpos no sucesso (`index.html:2663`). O que sobrou
  virou o 436-M5/B4/B5, logo abaixo.
- **(b) FECHADO como mapa.** Os filhos 340, 341, 342, 343, 346 e 347 estão ☑️, e
  o 345 é fechável (abaixo). A §11.3 do `SEGURANCA.md` já traz o `.fts` como
  FECHADO, e o conselho de «tirar o índice» foi reescrito. O que sobra é
  **⏸**: as duas sondas das sete representações continuam fora do repositório.
  Nenhum teste em `crates/*/tests` mede as sete juntas: só
  `cifra-do-indice-de-texto.rs` tem controle positivo. Não é defeito, é guarda
  de regressão.
- **(c) NÃO É TÉCNICO.** Validação jurídica de base legal, direitos do titular e
  retenção é **produto/jurídico**: sobe ao dono por essa categoria, e não entra
  na conta de engenharia.

**Fechável** como pedido técnico. Fica uma linha ⏸ (sonda) e uma de mesa (c).

---

## 344 — coluna EXTERNA marcada na réplica

**344-1 · réplica SEM cofre grava o texto cifrado como se fosse o memo · ABERTO
· ALTA · ATIVO** (dado errado, calado).
`reg.rs:2141`:
```rust
if !self.material.cifrado() || !self.externa_marcada(coluna) || guardado.is_empty() {
    return Ok(guardado.to_vec());
```
O `||` continua juntando «tabela sem cofre» e «coluna não marcada» no mesmo
`return`. `decodificar_com_externos` (`table.rs:6873-6905`) chama isso com bytes
selados pela origem, e o `Bin` grava `[nonce][cifrado][tag]` sem erro. O 342
não alcança este caso: ele exige o **fio** cifrado (`servidor.rs:28236`), e não
o **cofre** da réplica.
**Exploração:** origem com cofre e coluna `Bin` marcada, réplica sem cofre,
`replicar`/`aplicar`. A réplica guarda lixo e o `spare_promover` promove esse
banco.

**344-2 · mesma senha, sal por arquivo → recusa · ABERTO · média · ATIVO.**
Replicar coluna externa marcada entre dois nós com cofre não funciona. A recusa
sai de `cofre.rs:766-771`: «ou o dado foi alterado, ou a chave … não é a que
gravou». Ela acusa adulteração que não houve. O comentário de `table.rs:6889-6893`
(«só funciona entre servidores que dividem a senha») continua lá e continua
falso.

**Conserto mínimo dos dois (decisão C/SEC, sem formato novo):** desde o 342 o
fio é **obrigatoriamente cifrado** para tabela com coluna marcada. Então a
**origem** pode abrir os externos (`RegFile::abrir_externo`) **só na resposta
do `replicar`**, e a réplica sela com a chave dela no `inserir`. O `.log`/`.trash`
continuam selados como hoje. Isso fecha 344-1 e 344-2 de uma vez e torna
simétricas as metades inline e externa, que o 342 deixou cercadas. Enquanto
isso não entra, `decodificar_com_externos` deve **recusar nomeando**: «coluna
marcada externa veio selada pela origem; a réplica não tem a chave dela (sal
por arquivo)». A frase **substitui** a do cofre (não a envolve) e o comentário
é corrigido.
**Atenção:** quando a imagem passar a viajar aberta,
`tests/imagem-de-replicacao-com-coluna-marcada.rs` cai, porque exige o externo
AUSENTE. É o aviso previsto: reescrevê-lo junto.

**Teste adverso** (mede o CONTEÚDO, nunca o veredito):
`replica_sem_cofre_nao_grava_o_selado_como_memo` (o `Bin` lido na réplica ==
bytes originais, ou recusa nomeada; nunca bytes diferentes com `ok`) e
`mesma_senha_replica_coluna_externa_marcada` (conteúdo igual dos dois lados).

---

## 345 — permissão dos arquivos

**FECHADO, fechável.** Motor único `util::opcoes_do_banco`/`recriar_do_banco`
(`util.rs:82-134`, `:220-265`), 0600/0700. O `.lgpd`, o `.fts`, o Profiler
(`profiler.rs:690`), as diretivas (`diretivas.rs:239`), o log de jobs
(`jobs.rs:787`) e os cadastros de visões e rotinas
(`rotinas.rs:90` → `sincronia::gravar_duravel` → `recriar_do_banco`) passam
todos por ele.
**Prova, rodada hoje:**
`cargo test --offline -p phxsql-store --test permissao-dos-arquivos` → **8/8**.
O teste `a_copia_do_backup_e_a_restauracao_ficam_so_do_dono` cobre pasta, ZIP e
restauração com `umask 022`, e o `o_banco_nasce_0600_em_diretorio_0700` exige
reg/ndx/memo/log/trash/fts/lgpd/json presentes antes do veredito.
Residual que **não é defeito**: base antiga em 0644 só **alerta** e não aperta
(decisão do J pela régua, `util.rs:720-730`).

---

## 357 — trilha `.lgpd` e coluna marcada

**357-1 · o buraco da §11.7: volume ativo nascido em claro continua recebendo
registro em claro depois de o cofre ligar · ABERTO · média · ATIVO** (garantia
que não vale: cofre ligado, trilha gravando em claro).
`anexar` (`trilha.rs:1010-1033`) usa o `cab` do ativo que existe, sem perguntar
se ele é cifrado. O 368 limitou a janela (rodízio de 64 MiB / 30 dias), mas não
a fechou.
**Conserto mínimo:** em `anexar`, `if cofre::ligado() && !cab.cifrado() { self.fechar_ativo()? }`
antes de gravar. `fechar_ativo` (`:879`) → `nascer_ativo` → `Cabecalho::novo`
já nasce cifrado com o cofre ligado. Custo zero no laço quente: o `cab` já está
na mão.
**Teste adverso:** `ligar_o_cofre_fecha_o_volume_da_trilha_em_claro`. Gravar 1
registro sem cofre, ligar o cofre, gravar o segundo e fazer `grep` do valor nos
bytes do ativo. Hoje acha; tem de não achar.

**357-2 · redigir por GRAU (`Sensivel`), pedida por interruptor · ABERTO · ⏸.**
`valor_para_trilha` (`trilha.rs:435-450`) só olha o nome e o hash. É guarda
nova **pedida** (aprovada pelo DBA, item d2) e não defeito: com o cofre ligado,
o corpo já é cifrado. Prova prevista no pedido:
`a_trilha_redige_coluna_marcada_mesmo_com_nome_inocente`.

**357-3 · avisar ao marcar coluna com o cofre desligado · ABERTO · ⏸.**
`cofre::ligado()` não tem nenhum chamador no servidor
(`grep -rn "cofre::ligado()" crates` → só store e teste). O motor do aviso é o
mesmo de `material_da_tabela`.

---

## 359 — o literal de SQL guardado

**359-a · op `visoes` devolve o SQL verbatim a quem tem só `Ler` · ABERTO ·
média-alta · ATIVO** (acesso cruzado pelo protocolo).
`op_visoes` (`servidor.rs:13330-13345`) → `Visao::para_json` (`visoes.rs:51-58`,
o campo `sql` inteiro); `"visoes" => Atividade::Ler` (`usuarios.rs:155`);
`("visoes", PorColuna::Nenhum)` (`direito_coluna.rs:130`). O comentário
`:125-128` continua afirmando a premissa que o literal desmente. O comentário
SQL dentro do corpo (acréscimo de 24/09) sai pelo mesmo campo.
**Exploração:** o admin cria `v AS SELECT … WHERE cpf='52998224725' -- senha: x`.
Um usuário com `ler` no database, coluna `cpf` negada e sem direito na tabela,
pede `{"op":"visoes","database":"b"}` e lê o CPF e o comentário.
**Conserto mínimo («analisar, nunca recortar»):** na saída, quem não é
`Administrar` nem `criado_por` recebe o SQL **reserializado pelo analisador**
de `phxsql_sql`, com cada literal trocado por `'***'`. Reserializar já tira o
comentário. Se não analisar, recebe só `{nome, bytes}`. O molde existe:
`segredos::achar_segredo` → `phxsql_sql::usuario::sem_a_senha`
(`segredos.rs:112-140`).
**Teste adverso:** `visoes_nao_entrega_literal_a_quem_tem_a_coluna_negada` (o
`cpf` e o texto do comentário ausentes da resposta; o dono continua vendo
inteiro).

**359-b · cadastros em 0644 · FECHADO** (595 + 542): `rotinas.rs:90-105` →
`sincronia.rs:241-251` → `util::recriar_do_banco`. O listar de rotinas exige
`Administrar` (`servidor.rs:20810-20812`, `:20860-20862`) e por isso não vaza.

**359-c · `jobs.json` guarda o literal do `onde` · ABERTO · baixa · ⏸.** O arquivo
é 0600, a resposta é peneirada por `PorColuna::PedidoSalvo`
(`direito_coluna.rs:306-307`, 350 ☑️) e a recusa é só de credencial
(`jobs.rs:259`). Recusar o literal exige que `colunas_do_onde` desça aos
sub-pedidos (parecer do DBA): não embarcar antes disso.

---

## 436 — os médios e baixos da noite de 23/09

M1, M2 e M3: feitos (já registrados).

**M4 · TOFU irreversível e surdo ao interruptor · ABERTO · média · ATIVO**
(escorregão de configuração vira failover).
`conferir_identidade` recusa «sem prova de quem já provou» **antes** de olhar
`exigir_prova_do_pulso`, e esse ramo só roda com o interruptor desligado
(`cluster.rs:783-799`). `provaram` só cresce (`:639-641`). E `campos_da_prova`
devolve `None` **calado** quando falta pino ou a assinatura falha
(`cluster.rs:605`, `:619`, `.ok()?`).
**Conserto mínimo:** (1) `campos_da_prova` que devolve `None` num nó que **já
assinou** nesta vida passa a avisar uma vez no stderr, pelo molde de
`anunciar_que_passou_sem_prova`; (2) a decisão de o TOFU expirar (por exemplo,
na troca de época), ou de obedecer ao interruptor, fica com o dono, porque
mexe na garantia anti-rebaixamento.
**Teste adverso:** `o_no_que_perdeu_a_estatica_nao_derruba_o_cluster`
(processo filho, ler o stderr do lado que deixou de provar).

**M5 · `perguntar()` não consulta `oficial(c)` · ABERTO · média · ATIVO.**
`claude.js:318-329` faz `fetch(c.endpoint || ENDPOINT_OFICIAL)` sem portão.
O `connect-src 'self' https://api.anthropic.com` (`http.rs:435`) deixa passar
endpoint na **própria origem**, e aí o `x-api-key` vai ao PhxSql e a qualquer
proxy à frente. `migrarDoDisco` (`:189-201`) promove para cada aba nova um
endpoint plantado no `localStorage`.
**Conserto mínimo:** em `perguntar`, `if (!oficial(c) && !c.endpointConfirmado) throw`,
com o texto da fábrica que já existe para o aviso (`iaEnderecoPerigo`). Em
`migrarDoDisco`, não promover `endpoint`, só `chave`.
**Teste adverso** (`testes-web/`): plantar
`localStorage["phxsql.ia"]={"endpoint":location.origin+"/x"}`, ir direto à
Query e conferir que nenhum `fetch` com `x-api-key` sai.

**B1 · `Assinado::mensagem` não injetiva · FECHADO pelo M1.** O `nonce` tem de ser
`^[0-9a-f]{32}$` antes da fila (`pulso.rs:398-400`, `:487-494`), e então não
cabe `\n` no último campo livre; `de` e `para` passam pelo crivo da lista
(`cluster.rs:770-777`).

**B2 · «mais de 0 MiB» · FECHADO.** `medida(teto)` escreve bytes abaixo de 1 MiB
e o conselho só sai no teto do registro (`fio.rs:553-557`, `:665-671`).

**B3 · `cluster` fora de `SECOES_CONHECIDAS` · ABERTO · baixa · ⏸.**
`config.rs:4519` (16 seções, nenhuma é `cluster`). Um `exigir_prova_do_pulos`
digitado errado desliga a guarda calado. Atenuante: o valor efetivo aparece no
`config_ler`. Conserto: uma entrada na tabela, com os campos de `cluster` lidos
em `config.rs`.

**B4 · «some ao fechar a aba» é falso com janela destacada · ABERTO · baixa ·
ATIVO** (afirmação de segurança mostrada a quem usa).
`idiomas.rs:812` e `:818`, nos seis idiomas. `multitela.js:986` faz
`window.open` sem `noopener`, e a janela filha nasce com uma cópia do
`sessionStorage`.
**Conserto mínimo:** corrigir a frase pela fábrica («…e de cada janela
destacada dela»), ou abrir as destacadas sem herdar o cofre.

**B5 · login recusado deixa a chave privada Ed25519 no DOM · ABERTO · baixa ·
ATIVO.** `index.html:2663` só limpa no sucesso, e o `catch` (`:2668-2671`) não
toca em `#k`.
**Conserto mínimo:** no `catch`, `$("#k").value = ""`. `#s` e `#t` ficam, pela
decisão documentada.
**Teste adverso:** em `testes-web/`, login recusado com chave colada, e
`page.$eval("#k", e => e.value)` tem de dar `""`. Medir o `.value`, nunca o
`page.content()`, que foi a armadilha do 339.

**B6 · réplay de 60 s após reinício, cluster sem cifra · ABERTO · baixa · ⏸.**
O desenho é de memória (`cluster.rs` `provaram`, fila do pulso) e o
`docs/CLUSTER.md` não fala de réplay (`grep -i replay` → 0). Conserto: uma
frase no `CLUSTER.md`.

---

## Limites desta revisão

- Só o 345 foi **executado** (8/8). Os demais vereditos são leitura do fonte de
  hoje, com arquivo:linha. Nenhum teste adverso acima foi escrito ou rodado.
- 344-1 e 359-a são exposição/dado errado: **bloqueiam**, pela regra SEC, até o
  conserto ter prova real nos dois sentidos.
