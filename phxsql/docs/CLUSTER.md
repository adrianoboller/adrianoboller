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
| Adicionar/remover servidor a quente | ✗ a lista de nós é do `config.json`; mudar é editar e reiniciar |
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
  "token": "...", "usuario": "replicador",
  "senha_hash": "pbkdf2-sha256$...", // a MESMA tríade da origem de replicação
  "databases": [],                   // vazio = todos os do master
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
papel (qualquer nó pode ser promovido) e desligá-la de propósito é erro. Com
o bloco presente, `replicacao.origens` é ignorada (com aviso): a origem passa
a ser o master **corrente**, descoberto pelo pulso.

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

### 2.5 Roteiro de operação

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

### 2.6 Números e aprendizados da bancada (`bancada/cluster/`)

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

---

## 3. Escalar — o que escala e o que não

| | Escala | Como |
|---|---|---|
| **Leitura por consulta** | ✓ | réplicas, e `SelectMemory` (87× medido) |
| **Leitura sequencial** | ✓ | slot de largura fixa: 4,8× o MySQL(R) |
| **Tabela grande** | ✓ | partição em volumes por quantidade, período ou letra |
| **Escrita** | ✗ | um master, e o `.ndx` é 83,5% do custo |
| **Concorrência** | ✗ | **trava global única** — todo acesso a dado se serializa |
| **Tamanho do dado** | ✓ | volumes; o teto é `registros_por_arquivo × max_arquivos` |

**O gargalo de escala mais próximo não é o cluster: é a trava.** Um servidor
com muita gente lendo ao mesmo tempo hoje serializa tudo, inclusive leitura
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
