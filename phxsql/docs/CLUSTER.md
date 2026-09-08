# Cluster, escala e instâncias

Três perguntas juntas, porque as respostas se encostam: **dá para rodar várias
instâncias?**, **dá para clusterizar?** e **dá para escalar?**

Desde o pedido 126 a resposta do meio mudou: **há cluster com eleição e
promoção automática**, medido em `bancada/cluster/`. A seção 2 descreve o que
ele garante — e, com o mesmo cuidado, o que ele **não** garante.

---

## 1. Várias instâncias, em portas diferentes — **sim, e está provado**

Cada `phxsqld` lê o `config.json` do diretório em que foi iniciado. Porta de
dados, porta web, base, usuários, papel de replicação: tudo é daquela
instância. Não há registro global, não há serviço único, não há porta fixa.

```bash
cd /srv/erp      && phxsqld     # bind 127.0.0.1:5000, web 5001
cd /srv/telemetria && phxsqld   # bind 127.0.0.1:5100, web 5101
cd /srv/arquivo  && phxsqld     # bind 127.0.0.1:5200, web 5201
```

Provado de três formas: `bancada/replicacao/montar.py` sobe **quatro** de uma
vez (5800–5803); `bancada/cluster/provar.py` sobe três em cluster
(5310–5312); e o `docker-compose.yml` sobe três em contêineres separados.

---

## 2. Cluster — **sim, com eleição e promoção automática (pedido 126)**

O que existe, item por item contra a lista do HFSQL(R):

| O que o cluster deles faz | PhxSql |
|---|---|
| Vários servidores aparecem como **um** para o cliente | ✓ **pela semântica de protocolo**: `cluster_estado` responde em qualquer nó quem é o master, e escrita numa réplica devolve `REDIRECIONA host:porta` (erro 4003). VIP de rede é infraestrutura, não banco — ver §2.5 |
| Falha de um não impede o acesso | ✓ leitura segue nas réplicas; escrita volta sozinha após a eleição (medido: **3,6–4,3 s** com janela de 4 s) |
| Replicação automática entre todos | ◐ um master, N réplicas seguindo o master **corrente** — não é multi-master, de propósito |
| Carga de leitura distribuída | ◐ aponta-se leitores para réplicas; não há balanceador embutido |
| Adicionar/remover servidor a quente | ✓ **desde o pedido 217**: `cluster_no_acrescentar` e `cluster_no_remover` mudam a lista VIVA, gravam o `config.json` e propagam aos outros nós — ninguém reinicia. Medido: **zero recusas em 42 escritas** contra os **0,369 s** de master fora do ar do caminho antigo. Ver §2.7 |
| Servidor que caiu ressincroniza ao voltar | ✓ e, se era o master, **se rebaixa sozinho** ao ver época maior no pulso |
| Cliente reconectado automaticamente | ◐ o protocolo diz **para onde** ir (`REDIRECIONA`); ir é do cliente |

### 2.1 O bloco `cluster` no config.json — pedido, não imposto

**Sem o bloco, NADA muda**: nenhuma thread sobe, nenhum portão muda, réplica
com origens fixas continua igualzinha. O teste que trava isso é
`sem_o_bloco_cluster_nada_muda`, e a fase (g) da bancada prova o mesmo pelo
soquete.

```json
"replicacao": { "papel": "source" },          // ou "replica" nos demais
"cluster": {
  "id": "no1",                       // qual nó da lista é ESTE servidor
  "prioridade": 0,                   // desempate de eleição (maior ganha)
  "janela_inatividade_s": 10,        // master calado além disto = caído
  "pulso_s": 3,                      // omitido = um terço da janela
  "avisar_cada_min": 5,              // aceita fração: 0.1 = 6 s
  "quorum_minimo": 0,                // GUARDADO e ainda NÃO imposto (§2.4)
  "token": "...", "usuario": "replicador",
  "senha_hash": "pbkdf2-sha256$...", // a MESMA tríade da origem de replicação
  "databases": [],                   // vazio = todos os do master
  "cifra": false,                    // true = cifra TODO o tráfego do cluster (§2.9)
  "nos": [
    {"id": "no1", "endereco": "10.1.1.102", "porta": 5000},
    {"id": "no2", "endereco": "10.1.1.103", "porta": 5000},
    {"id": "no3", "endereco": "10.1.1.104", "porta": 5000}
  ],
  "email": { "ligado": true, "servidor": "127.0.0.1", "porta": 25,
             "de": "phxsql@empresa.com.br", "para": ["dba@empresa.com.br"] }
}
```

Regras que o arranque impõe: o `id` tem de constar de `nos`; menos de dois
nós não sobe; papel `isolado` não sobe; `imagem_da_linha` liga em **todo**
papel (qualquer nó pode ser promovido) e desligá-la de propósito é erro; um
`chave_do_fio` torto em qualquer nó é recusado na declaração, com o nó
nomeado (§2.9). Com o bloco presente, `replicacao.origens` é ignorada (com
aviso): a origem passa a ser o master **corrente**, descoberto pelo pulso.

### 2.2 Como funciona por dentro

- **Pulso.** Cada nó mantém uma conexão com cada outro e troca
  `cluster_pulso` a cada `pulso_s`, autenticado como a réplica já se
  autentica (token + desafio-resposta a partir do `senha_hash`; permissão
  `replicar`). O pulso carrega id, papel vivo, época, posição do diário
  (soma dos eventos das tabelas replicadas) e prioridade — o pedido leva os
  meus, a resposta traz os do outro.
- **Papel vivo e época.** O papel do `config.json` é só o inicial. O vivo
  mora em `base/cluster.estado.json` junto com a **época** — um contador que
  cresce a cada eleição. O arquivo ganha do config no arranque: um master
  destronado que reiniciasse pelo config voltaria mandando.
- **Detecção.** Master sem pulso além de `janela_inatividade_s` abre
  eleição nos nós vivos. Há uma graça de uma janela no arranque, senão todo
  cluster nasceria "degradado" antes do primeiro pulso.
- **Eleição** (função pura `cluster::vencedor`, com a bateria de testes em
  volta): só há eleito se os vivos passam da **metade dos nós
  configurados** — metade exata não basta, senão os dois lados de uma
  partição ao meio elegeriam um master cada. Entre os elegíveis vence a
  maior posição do diário; empate quebra pela prioridade e depois pelo menor
  id (este último só para a conta dar igual em todo nó). Cada nó faz a conta
  localmente e **só quem se vê vencedor se promove**, com época =
  maior época vista + 1. «Vivos» é *quem pulsou dentro da janela* — o que não
  é a mesma coisa que *quem está de pé agora*, e a diferença tem consequência
  medida: §2.4, item 5.
- **Promoção.** `Servidor::promover_a_master(motivo)` é o **único** caminho:
  época nova, papel persistido, escrita liberada, aviso agendado, registro no
  log de acessos (`cluster_promocao`). O laço de réplica para sozinho, porque
  confere o papel vivo a cada volta. Promoção manual futura deve chamar o
  mesmo lugar — dois caminhos de promover é a porta dos fundos clássica.
- **Rebaixamento.** Qualquer nó que se acha master e vê época maior no
  pulso se rebaixa sozinho e passa a seguir o novo master. Dois masters na
  mesma época (dois configs `source`, ou empate de partição) resolvem pelo
  mesmo critério da eleição — os dois lados fazem a mesma conta e o perdedor
  cede.
- **Master isolado não escreve.** Um master que deixa de enxergar a maioria
  recusa escrita ("cluster degradado") até a maioria voltar. É o que limita
  o split-brain ao tempo de detecção — ver §2.4.

### 2.3 O endereço único — `cluster_estado` e `REDIRECIONA`

O cliente valida com **qualquer** nó:

```json
{"op":"cluster_estado"}
{"ok":true,"resultado":{"papel":"replica","epoca":1,
  "master":{"id":"no2","endereco":"10.1.1.103:5000"},
  "escrita_liberada":false,"degradado":[],
  "nos":[{"id":"no1","papel":"replica","posicao":3801,"vivo":true,...},...]}}
```

E escrita que chega numa réplica volta com nome `REDIRECIONA`, código
**4003**, e a mensagem começando com o pedaço que o cliente recorta:

```
REDIRECIONA 10.1.1.103:5000 -- este no e replica; o master do cluster e no2 (epoca 1)
```

Se o master conhecido está calado além da janela, a recusa diz "eleição em
curso" em vez de apontar um endereço morto.

**VIP de rede é infraestrutura, não banco.** Um IP flutuante, um
balanceador, um DNS de peso — tudo isso pode ser posto NA FRENTE do cluster,
e é problema de rede. O que o banco entrega é a **semântica** de endereço
único pelo protocolo: qualquer nó sabe dizer quem manda, e diz.

### 2.4 O que isto NÃO garante — leia antes de confiar

**Não há quórum de escrita.** O master confirma a gravação sem esperar réplica
nenhuma, e o que ele aceitou entre o início de uma partição e o momento em que
se vê sem maioria não chegou a ninguém. O campo `cluster.quorum_minimo` já
existe no formato e a op `config` o devolve ao lado de `"quorum_imposto":
false` — porque **mudança de formato entra cedo** —, mas nada o lê. O parecer
medido do que falta está em `docs/propostas/quorum-de-escrita.md`, e o achado
que ele derruba vale ser lido antes de qualquer plano: o canal aberto que o
quórum precisaria **já existe**, é o do pulso, e custa **0,089 ms** de ida e
volta.


**Não é Raft.** Não há log replicado por quórum de escrita: o master
confirma a escrita **sem esperar réplica nenhuma** (replicação assíncrona,
como sempre foi). As consequências práticas, sem eufemismo:

1. **Perda de cauda.** Se o master morre ou é isolado, as escritas que ele
   confirmou e as réplicas ainda não puxaram **morrem com ele**. O novo
   master começa do que alcançou. Quando o antigo volta e se rebaixa, o
   diário local dele pode estar à frente do novo master — o nó fica
   degradado avisando ("provável cauda de escritas perdidas") e a saída é
   **ressemear** o nó a partir do master (a replicação para na divergência
   de rowid em vez de espalhá-la; nunca apagamos dado sozinhos).
2. **Janela de dois masters.** O destronamento é por época no pulso, então
   um master antigo pode aceitar escrita por até ~uma janela antes de se ver
   sem maioria ou ver a época nova. Essas escritas são cauda (item 1). A
   eleição por maioria garante que **não há dois masters duradouros**: só
   uma partição pode enxergar mais da metade dos nós configurados.
3. **Sem maioria, sem escrita.** Numa partição minoritária ninguém promove
   e o master isolado recusa escrita. Cluster de **dois nós nunca se promove
   sozinho** (1 de 2 não é maioria) — o arranque avisa; para failover
   automático são três nós ou mais.
4. **A posição comparada é a soma dos eventos** das tabelas replicadas. Sem
   transação entre tabelas não há ordem global, e a soma é o agregado
   honesto disponível; a prova fina de igualdade continua sendo o retrato
   SHA-256 (a bancada confere os dois).
5. **A fresta entre «o master calou» e «os pares envelheceram».** A eleição
   conta quem **pulsou** dentro da janela, e o silêncio do master sai do
   **mesmo relógio**. Os dois prazos não vencem juntos quando os nós caem em
   momentos *diferentes*: um par que morre **depois** do master ainda está
   dentro da janela no instante em que o master é declarado calado — e nesse
   instante um nó minoritário enxerga **maioria** e se elege. A eleição não
   está errada em relação ao que vê; o que vê é que está velho.

   Medido nos dois sentidos por `bancada/cluster/fresta.py` — mesmo binário,
   mesma configuração, mesmos três nós, mudando **só a ordem das mortes**
   (1,5 s entre elas):

   | ordem das mortes | o nó que sobra fica | época | o que ele registrou |
   |---|---|---|---|
   | master primeiro, par 1,5 s depois | **master** | 0 → **1** | `PROMOVIDO a master na epoca 1 -- master calado ha 4s; eleito entre 2 vivos de 3 configurados` |
   | par primeiro, master 1,5 s depois | replica | 0 → 0 | `master calado ha 4s e sem maioria visivel (1 de 3): NAO promovo` |

   **O que a fresta NÃO quebra: a escrita.** O portão é `escrita_liberada`,
   recalculado pelo árbitro a cada 500 ms a partir de `e_maioria(vivos)`, e
   ele é **independente do papel** — nas duas ordens a escrita foi recusada,
   nas duas os três convergiram para um master só, uma época só, e retratos
   SHA-256 idênticos. Nenhuma linha se perdeu, e não houve dois masters.

   **O que ela custa: a liderança.** Um nó que estava **sozinho** sobe a
   época, e é a época que manda no rebaixamento. Quando a maioria volta, os
   dois nós que juntos *eram* a maioria se rebaixam e passam a seguir o que
   esteve isolado — medido: `{"no1":"no3","no2":"no3","no3":"no3"}`, época 1
   nos três. Não é perda de dado; é liderança entregue ao nó **pior
   informado**, e se o diário dele estiver atrás os outros passam a segui-lo
   do ponto em que ele parou.

   **Partição de rede não abre a fresta.** Ali os enlaces caem todos ao mesmo
   tempo: o silêncio do master e o envelhecimento dos pares vencem juntos, e
   o lado minoritário vê 1 de 3. A fresta é da **queda em sequência** — o
   caso do reinício em rolagem e o do desligamento de rack.

   Por isso a fase (e) da bancada mata o **par primeiro e o master por
   último**, com 2 s entre eles: só assim «1 de 3» é verdade *antes* de o
   árbitro olhar, e o passo mede a garantia que o nome dele promete em vez de
   sair cara ou coroa. A ordem contrária está em `fresta.py`, que **mede** a
   fresta sem afirmá-la — guarda que afirmasse o defeito viraria catraca
   contra o próprio conserto.

   **DECISÃO do dono, 04/09/2026: o motor NÃO é tocado.** As três saídas
   estavam na mesa com preço — (A) contar só pares ouvidos depois do master
   **morreu medida** (na corrida real o par foi ouvido 1 s depois e passaria
   pelo filtro); (B) janela mais curta só para eleger custa um segundo número
   mágico e falso-negativo em enlace lento; (C) pedir o quórum de forma
   síncrona antes de promover fecha **por construção**, e é o `RequestVote` do
   Raft sem a persistência do voto — mas **muda o comportamento de todo cluster
   que já existe**, e *guarda nova entra pedida, não imposta*.

   A fresta fica **documentada e medida**: ela não custa dado, só liderança, e
   só aparece em **queda em sequência**. O `fresta.py` continua medindo sem
   afirmar. Se um dia houver um cluster em produção com reinício em rolagem, a
   saída (C) volta como pedido próprio — com quem precisa dela e por quê.

### 2.5 Escalonar a quente — acrescentar e remover nó sem reiniciar ninguém

**Pedido 217.** Até 07/09/2026 a lista de nós era o retrato do `config.json` no
arranque, e isso tinha uma consequência que só apareceu exercitando: um nó novo
com a lista completa na própria configuração ficava **isolado a janela
inteira** — o pulso dele era recusado na hora pelos antigos
(`ACESSO_NEGADO: o no "no4" nao esta na lista de nos deste cluster`), e os
antigos nunca tentavam falar com ele, porque ele não estava na lista deles.

A recusa **continua**, e continuar é a decisão certa: aceitar um id
desconhecido deixaria qualquer credencial de replicação inflar o denominador da
maioria com nós fantasmas e travar toda promoção. O que mudou foi a **lista**.

```json
{"op":"cluster_no_acrescentar","id":"no4","endereco":"10.0.0.4","porta":5000}
{"op":"cluster_no_remover","id":"no4"}
```

**Três coisas, nesta ordem, e a ordem importa:**

1. **a lista viva**, para o pulso do nó novo ser aceito agora;
2. **o `config.json` deste nó**, para o nó acrescentado não sumir calado no
   próximo arranque — perder um nó assim é pior que não acrescentar, porque o
   cluster segue com um denominador menor do que o operador acredita;
3. **a propagação**, uma vez por nó, com a credencial do próprio cluster.

Gravar antes de aplicar deixaria o arquivo prometendo o que a memória ainda não
faz; propagar antes de aplicar mandaria os outros aceitarem um nó que este aqui
ainda recusa.

**A propagação fala quando falha.** A resposta traz um veredito por nó:

```json
{"acrescentado":true,"id":"no4","nos":5,
 "propagado":{"no2":"ok","no3":"ok","no4":"ok"}}
```

Nó que não aceitou aparece com **o erro inteiro**, e não sumindo da lista:
metade do cluster escalonada em silêncio é exatamente o estado em que uma
eleição conta votos diferentes em cada lado. A tela de Cluster mostra esse
veredito.

**Consequência aceita e escrita:** a propagação autentica com
`cluster.usuario`, então **esse usuário precisa poder `administrar`**. Sem
isso a ordem local vale e a propagação volta recusada, nomeando o nó — que é
melhor que dar o poder de mexer na maioria a quem só tem credencial de réplica.

**Duas recusas do `cluster_no_remover`, e as duas são decisão:** este nó não se
remove (um servidor fora da própria lista não passa mais no `Cluster::validar` e
não subiria de novo) e o **master corrente** não se remove (tirá-lo da lista
deixaria o cluster sem para onde redirecionar a escrita).

**As threads acompanham sozinhas.** Um supervisor (`pulso-supervisor`) mantém
uma thread de pulso por nó da lista viva: nó acrescentado ganha pulso em até
meio segundo, nó removido — ou que mudou de endereço — vê a thread dele morrer
e, no segundo caso, outra nascer no endereço novo.

**Medido** (`bancada/cluster/escalonar.py`, 07/09/2026, cinco nós em
`127.0.0.1`):

| | caminho antigo (editar e reiniciar) | a quente (pedido 217) |
|---|---|---|
| master fora do ar | **0,369 s** | **0 s** — zero recusas em 42 escritas batendo de 10 em 10 ms |
| escalonamento inteiro | 1,11 s (três reinícios) | **0,207 s** até o nó novo aparecer vivo nos quatro antigos |
| a ordem em si | — | **5 ms** |
| `config.json` dos antigos | editado à mão | gravado pelo próprio motor, conferido nos quatro |
| retratos SHA-256 no fim | batem | batem, nos cinco |

O `master fora do ar = 0 s` é medido, e não deduzido: uma batida de escrita de
10 em 10 ms roda durante a ordem inteira e conta as **recusas** e o **maior
buraco entre dois `ok`** (11,5 ms — o próprio intervalo da batida). Sem o
buraco, «não recusou» poderia ser «parou sem dar erro», que é pior.

### 2.6 A tela de Cluster

**Pedido 208.** Ferramentas → **Cluster** (e no menu Ferramentas pelo teclado).
Ela mostra o papel deste nó, quem é o master, a época, quantos nós há, se a
escrita está liberada e a janela de inatividade; a grade lista cada nó com
papel, estado (`este servidor` / `vivo` / `calado` / `nunca pulsou`), época,
posição do diário e idade do último pulso. Os motivos de degradação aparecem em
vermelho, como o servidor os escreveu.

Três ações, com as cores da convenção: **acrescentar nó** (verde), **remover
nó** (vermelho) e **gravar o quórum mínimo** (âmbar) — todas por contorno,
nunca fundo cheio.

Duas honestidades que a tela carrega de propósito:

- **Servidor sem bloco `cluster` mostra a nota e nenhum botão.** Não há nada a
  operar, e desenhar controles que recusariam tudo seria pior que não desenhar.
- **O campo do quórum diz que não é imposto**, e quem diz isso é o **servidor**
  (`"quorum_imposto": false` na resposta de `config`), não uma frase da tela —
  duas telas divergem no dia em que uma for atualizada e a outra não.

Ela é exercitada contra três servidores de verdade em
`testes-web/capturas-cluster.mjs` (dez passos, capturas em claro e escuro em
`docs/dossie/capturas/cluster-*.png`). Não entra na `bateria.mjs` porque a
bateria sobe um `phxsqld` isolado, e aí a tela não desenha botão nenhum — a
dispensa está registrada no `conferidor_botoes.rs`, apontando para cá.

### 2.7 Roteiro de operação

- **Subir:** um nó com `papel: source` (o master inicial), os demais
  `replica` + `somente_leitura: true`, todos com o mesmo bloco `cluster`
  (mudando só `id` e `prioridade`). Réplicas de cluster não precisam de
  `origens`.
- **Validar:** `cluster_estado` em qualquer nó; os três têm de apontar o
  mesmo master e a mesma época.
- **Failover:** não há o que fazer — é o ponto. O e-mail de promoção conta
  quem assumiu; o de degradação repete a cada `avisar_cada_min` enquanto
  durar (cada nó vivo avisa o que **vê**; aviso em dobro num cluster
  degradado é melhor que aviso nenhum quando quem avisaria é o nó que caiu).
- **Volta do nó caído:** sobe igual; se era master, se rebaixa sozinho. Se
  ficou degradado acusando cauda, ressemeie: pare o nó, apague a base local
  (e o `cluster.estado.json` se quiser zerar o papel), suba de novo — a
  réplica puxa tudo do master corrente.
- **`somente_leitura` num nó promovido** deixa de valer — senão a promoção
  não promoveria nada. Sem o bloco `cluster`, vale como sempre valeu.

### 2.8 Números e aprendizados da bancada (`bancada/cluster/`)

| medido | resultado |
|---|---|
| Promoção após matar o master (janela 4 s, pulso 1 s) | **3,6–4,3 s** |
| Primeira escrita aceita no novo master | **3,6–4,5 s** |
| E-mail de promoção | exatamente **1** |
| E-mails de degradação em ~14 s (aviso a cada 6 s) | **6 a 8** (3 a 4 por nó vivo). O que se prova é a **repetição**; o total varia com onde a queda cai entre dois avisos de 6 s |
| Nó isolado (1 de 3) por 3× a janela, **par morto antes do master** | **não** se promove; época intacta; escrita recusada |
| Nó isolado com o par morto **depois** do master (`fresta.py`) | **promove-se** (vê 2 de 3 dentro da janela) e ainda assim **recusa a escrita**; convergem os três — §2.4, item 5 |
| Retratos SHA-256 após queda, promoção, volta e rebaixamento | **idênticos nos três** |
| Fase sem bloco `cluster` (4 servidores como hoje) | replicação intacta, `cluster_estado` dá erro claro, zero e-mails |

Prova real da bateria: removida a conferência de maioria de `vencedor()`, o
teste `sem_maioria_visivel_nao_promove` **falhou** (e só ele); restaurada,
passou. A fase (e) da bancada prova o mesmo pelo soquete.

Aprendizados que ficaram no código:

- **`Cliente::databases()` estava quebrado desde sempre** — o `bancos`
  responde uma lista direta e o leitor procurava um campo `"bancos"` que não
  existe. Consequência: origem com `databases: []` (= todos) **não replicava
  nada, em silêncio**. Ninguém viu porque a bancada de replicação sempre
  fixou a lista; o laço do cluster, que precisa descobrir os databases
  sozinho, pisou ali primeiro. É a versão de protocolo da regra da casa:
  caminho que nenhum teste percorre mente igual a configuração que ninguém
  lê.
- **Todo cluster nascia "degradado"** na primeira versão: o árbitro roda seu
  primeiro tique meio segundo depois do arranque, antes do primeiro pulso —
  e mandava e-mail. Entrou a graça de uma janela a partir do nascimento do
  estado.
- **O master não se apontava como master** em `cluster_estado` (o
  `master_id` só era preenchido por pulso recebido, e o master não pulsa a
  si mesmo). A fonte mais confiável respondia "sem master".
- **Redirecionar para um master morto** é pior que recusar explicando: a
  recusa agora olha a idade do último pulso do master antes de mandar o
  cliente para lá.
- Hipótese que **morreu**: usar `replicas_autorizadas` como lista de
  autenticação dos pulsos. Ao ler o código para reusar, descobriu-se que o
  campo **não é lido por ninguém** — está no config e nunca foi consultado.
  Fica registrado aqui como pendência de outra frente (é exatamente o campo
  que mente, da regra da casa); o cluster autentica como a réplica, por
  usuário e permissão `replicar`.

### 2.9 Cifrar o tráfego do cluster — o INTEIRO, não a metade

Até 08/09/2026 o cluster falava **em claro**: o pulso, o `cluster_pulso` e a
replicação entre nós andavam sem túnel, ao contrário da porta de dados
(`docs/CIFRA-DO-FIO.md`). O item aberto trazia a lei que guiou o conserto:
**cifrar só metade do tráfego do cluster é pior que não cifrar nenhuma, porque
parece protegido.** Por isso o cluster ganha **um** interruptor, que liga os
dois caminhos de uma vez.

```json
"cluster": {
  "cifra": true,                     // pulso E replicação, juntos
  "nos": [
    {"id":"no1","endereco":"10.1.1.102","porta":5000,"chave_do_fio":"<64 hex>"},
    {"id":"no2","endereco":"10.1.1.103","porta":5000,"chave_do_fio":"<64 hex>"},
    {"id":"no3","endereco":"10.1.1.104","porta":5000,"chave_do_fio":"<64 hex>"}
  ]
}
```

- **`cluster.cifra`** (padrão `false`) reaproveita o aperto de mão estilo Noise
  que já protege a porta de dados. O pulso e a replicação passam os dois pela
  mesma `replica::Cliente`, que já sabia cifrar — não houve mudança de formato
  em disco, só fiação.
- **`nos[].chave_do_fio`** é o **pino** daquele nó (a chave pública do servidor
  dele, no estilo `known_hosts`), obtida com `phxsqld --chave-do-fio` **naquele
  nó**. Cada nó confere a chave de quem alcança. Vazio com a cifra ligada =
  túnel **sem pino** (só escuta passiva), e o arranque nomeia os nós sem pino em
  voz alta.
- **Padrão desligado** porque guarda nova entra **pedida**: um cluster que já
  rodava continua em claro na atualização. Ligar exige que **todo** nó atenda o
  aperto (`cifra_fio.ligada`, que já nasce ligada) — é decisão do cluster
  inteiro, como o `origem.cifra`.
- **Não é TLS**, e não protege o `config.json` (é lá que estão o pino, o token e
  o `senha_hash`). O que ele dá: escuta passiva fechada sempre; homem-no-meio
  fechado quando há pino em todos os nós.

Provado pelo **soquete** (`tests/cluster-cifrado.rs`): dois nós com
`cifra_fio.exigir: true` só se enxergam vivos no `cluster_estado` se o pulso
atravessa o túnel. As duas metades têm guarda própria no
`bancada/guardas/catalogo.py` — `pulso-do-cluster-em-claro` e
`replicacao-do-cluster-em-claro` —, uma por caminho, para que esconder uma
delas não passe.

---

## 3. Escalar — o que escala e o que não

| | Escala | Como |
|---|---|---|
| **Leitura por consulta** | ✓ | réplicas, e `SelectMemory` (87× medido) |
| **Leitura sequencial** | ✓ | slot de largura fixa: 4,8× o MySQL(R) |
| **Tabela grande** | ✓ | partição em volumes por quantidade, período ou letra |
| **Escrita** | ✗ | um master, e o `.ndx` é 83,5% do custo |
| **Concorrência** | ◐ | **trava global**, com uma ficha compartilhada: desde 05/09 o `varrer` (leitura de grade) deixa de esperar outro `varrer`; toda escrita, e as outras 75 seções, continuam exclusivas. `docs/CONCORRENCIA.md` §16 |
| **Tamanho do dado** | ✓ | volumes; o teto é `registros_por_arquivo × max_arquivos` |

**O gargalo de escala mais próximo não é o cluster: é a trava.** Um servidor
com muita gente lendo ao mesmo tempo hoje serializa quase tudo — a exceção é o
`varrer`, e ela é de 05/09 —, inclusive leitura
contra leitura. Trava por tabela — e depois leitura concorrente com escrita —
rende mais, e é mais barato, que qualquer coisa distribuída.

---

## 4. Docker

`Dockerfile` e `docker-compose.yml` na raiz. A imagem final é `scratch`: sem
shell, sem gerenciador de pacotes, só o binário — o que só é possível porque o
projeto não tem dependência externa nenhuma.

```bash
docker build -t phxsql .
docker compose up -d          # um master e duas réplicas
```

Duas coisas medidas nesta seção, e as duas envelheceram desde que foram
escritas:

- O alvo **musl** produz binário `static-pie`. O tamanho não vai aqui — esta
  linha dizia "3,4 MB o servidor, 1,2 MB o cliente" e ficou parada enquanto o
  binário crescia; o número medido de verdade (7,66 MB o servidor, com
  `strip`, em 02/09/2026) está em
  [`docs/dossie/relatorio-conteineres.html`](dossie/relatorio-conteineres.html)
  e no `docs/PENDENCIAS.md` #167. O alvo padrão (gnu) linka `libc.so.6`,
  `libgcc_s.so.1` e o carregador dinâmico — com ele `FROM scratch` **não
  sobe**.
- O binário musl **roda**: subiu um servidor com ele e o `ping` respondeu.
- **"`docker build` não foi executado" era verdade quando esta seção foi
  escrita, e deixou de ser**: o `docs/PENDENCIAS.md` #118 registra o `docker
  build` rodando de verdade, com o daemon no ar, mais de dez vezes numa
  corrida — inclusive achando e corrigindo dois defeitos que só apareciam
  construindo (o `Dockerfile` não construía por um caminho de `COPY` errado,
  e o par de modelos de réplica não se enxergava). Ver `docs/PENDENCIAS.md`
  #118 e #146 para o estado medido de hoje; esta seção fica como registro do
  que se sabia antes disso.
