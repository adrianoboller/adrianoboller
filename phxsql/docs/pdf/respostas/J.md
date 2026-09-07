# J) exemplo de script de Clusterizacao escalonamento

> Corrida em 2026-09-07T16:29Z–16:31Z (escalonamento) e 16:31Z–16:34Z
> (eleição/promoção) UTC · commit `a56a165` · `target/release/phxsqld` ·
> reproduzido por `python3 bancada/cluster/provar.py` e
> `python3 bancada/cluster/escalonar.py` (este segundo, novo nesta rodada)

## Resposta curta

**Clusterização** existe, com eleição e promoção automática por maioria
(pedido 126): o bloco `cluster` do `config.json` declara `nos` (id, endereço,
porta — **todos, este incluído**), prioridade, janela de inatividade e
credenciais; `cluster_estado` responde igual em qualquer nó; escrita numa
réplica devolve `REDIRECIONA host:porta`. `bancada/cluster/provar.py` prova
isso com três nós de verdade, matando o master pelo PID.

**Escalonamento** — acrescentar um nó a um cluster que já está no ar — a
bancada existente **não cobre** (ela sobe três nós fixos desde o arranque).
Escrevi `bancada/cluster/escalonar.py` para isso, e a resposta tem duas
metades medidas contra o motor vivo: **a quente não funciona** — cada nó
recusa o pulso de um id fora da própria lista `cluster.nos`, na hora, pelo
protocolo — e **o caminho que funciona é editar a lista dos nós ANTIGOS e
reiniciá-los** (o novo já nasce sabendo dos quatro, e por isso não precisa
reiniciar): cada réplica reinicia em ~0,37 s sem a escrita parar; o master
reinicia em 0,367 s **sem eleição** (época intacta); o nó novo aparece "vivo"
nos outros em menos de um pulso (104–370 ms, com `pulso_s: 1`); os quatro
retratos SHA-256 batem depois de uma escrita nova alcançar todo mundo.

## Exemplo exercitado

### Parte 1 — eleição e promoção automática (`provar.py`, três nós, portas 5310–5312)

Bloco `cluster` de um dos três nós (janela 4 s, pulso 1 s, prioridades
no2=2/no3=1 para desempate):

```json
"cluster": {
  "id": "no2", "prioridade": 2,
  "janela_inatividade_s": 4, "pulso_s": 1, "avisar_cada_min": 0.1,
  "token": "pulso", "usuario": "adm", "senha_hash": "pbkdf2-sha256$...",
  "nos": [
    {"id": "no1", "endereco": "127.0.0.1", "porta": 5310},
    {"id": "no2", "endereco": "127.0.0.1", "porta": 5311},
    {"id": "no3", "endereco": "127.0.0.1", "porta": 5312}
  ],
  "email": {"ligado": true, "servidor": "127.0.0.1", "porta": 5316,
             "de": "phx@bancada.local", "para": ["dba@bancada.local"]}
}
```

Saída real, `python3 bancada/cluster/provar.py /tmp/phx-f3-4084/cluster-provar`:

```
(a) tres nos sobem; cluster_estado igual nos tres
  ok  master unico e e o no1 -- {'no1': {'id': 'no1', 'endereco': '127.0.0.1:5310'}, 'no2': {'id': 'no1', ...}, 'no3': {'id': 'no1', ...}}
  ok  epoca 0 nos tres
  ok  tres vivos na visao de cada um
(b) escreve no master; replicas acompanham; REDIRECIONA na replica
  ok  carga inicial gravada
  ok  replicas alcancam a carga -- alvo 3000, no2=3000, no3=3000
  ok  retratos identicos nos tres -- {'no1': (3000, '204ff74ebe1651ef'), 'no2': (3000, '204ff74ebe1651ef'), 'no3': (3000, '204ff74ebe1651ef')}
  ok  escrita na replica redireciona para 5310 -- erro: '[SP000028] REDIRECIONA 127.0.0.1:5310 -- este no e replica; o master do cluster e no1 (epoca 0)'

(c) mata o no1 pelo PID; mede a promocao do no2
  ok  no2 se promoveu -- 4.3s
  ok  novo master aceita escrita -- 4.3s
  ok  no3 segue o novo master -- alvo 3501, no3=3501
  ok  REDIRECIONA do no3 aponta o novo master (5311) -- '[SP000028] REDIRECIONA 127.0.0.1:5311 -- este no e replica; o master do cluster e no2 (epoca 1)'
  ok  epoca subiu para 1 no no3 -- 1

(d) e-mails: degradacao repetida a cada 6s e promocao unica
  ok  exatamente 1 e-mail de promocao -- 1 capturados
  ok  degradacao cita o no1 caido -- 6
  ok  aviso repete no mesmo no (a cada ~6s) -- {'no3': 3, 'no2': 3}

(f) o antigo master volta e se rebaixa sozinho
  ok  no1 voltou como replica do no2 -- {'id': 'no2', 'endereco': '127.0.0.1:5311'}
  ok  no1 alcanca o que perdeu -- alvo 3701, no1=3701

(e) particao sem maioria: o no3 sozinho NAO se promove
  ok  no3 continua replica -- replica
  ok  no3 nao abriu eleicao: epoca intacta -- 1 -> 1
  ok  no3 com a escrita travada -- False
  ok  no3 diz por que esta degradado -- ['no no1 sem pulso ha 14s', 'no no2 sem pulso ha 12s', 'master calado ha 12s e sem maioria visivel (1 de 3): NAO promovo']
  ok  escrita recusada no no isolado, sem apontar um master morto
  ok  e-mail de sem-maioria capturado -- 1
    sobe no1 e no2 de volta: o cluster sara sozinho
  ok  no2 volta master pela epoca persistida -- master
  ok  os tres convergem -- alvo 3801
  ok  retratos finais identicos -- {'no3': (3801, 'fe057d23b9a39478'), 'no2': (3801, 'fe057d23b9a39478'), 'no1': (3801, 'fe057d23b9a39478')}

(g) sem o bloco cluster, tudo como hoje
  ok  replicacao classica continua funcionando -- alvo 500
  ok  cluster_estado sem cluster e erro claro
  ok  replica sem cluster recusa como sempre (somente leitura)
  ok  nenhum e-mail novo na fase sem cluster -- 0 chegaram

RESULTADO {"promocao_s": 4.3, "escrita_aceita_s": 4.3, "emails_promocao": 1,
"emails_degradacao": 6, "no3_nao_promoveu": true, "epoca_do_isolado": 1,
"linhas_no_fim": 3801, "sem_cluster_nada_muda": true, "falhas": []}
```

**A época e quem virou master**, nomeados: `no1` (5310) morre; **`no2` se
promove em 4,3 s** e a época sobe **0 → 1**; `no3` passa a segui-lo; quando o
`no1` volta, ele lê a época maior no pulso e **se rebaixa sozinho** a
réplica do `no2`. Todos os 20 asserts da bateria passaram (`"falhas": []`).

### Parte 2 — escalonamento a quente NÃO funciona (`escalonar.py`, novo)

Cluster de três nós já vivo (no1 master, no2/no3 réplica, portas
6300–6302), 300 linhas gravadas. Pulso cru simulando um `no4` de fora,
mandado direto para o `no1` já no ar — pedido e resposta REAIS:

```
--> {"op":"cluster_pulso","id":"no4","papel":"replica","epoca":0,"posicao":0,"prioridade":0}
<-- {"ok": false, "op": "cluster_pulso",
     "erro": "[SP000025] acesso negado: o no \"no4\" nao esta na lista de nos deste cluster",
     "codigo": 4001, "nome": "ACESSO_NEGADO", "classe": "acesso"}
```

Subindo o `no4` de verdade, com a lista de **quatro** já na PRÓPRIA
configuração (os três antigos continuam com três), e esperando a janela
inteira (8 s): ele nunca vê ninguém, e ninguém tenta vê-lo —

```
no4 (isolado): {"papel": "replica", "epoca": 0, "master": null,
  "escrita_liberada": false,
  "degradado": ["no no1 nunca deu pulso", "no no2 nunca deu pulso",
                "no no3 nunca deu pulso",
                "master calado ha 8s e sem maioria visivel (1 de 4): NAO promovo"],
  "nos": [{"id":"no1","vivo":false}, {"id":"no2","vivo":false},
          {"id":"no3","vivo":false}, {"id":"no4","este":true,"vivo":true}]}

no1 (nunca soube do no4): nos = ["no1", "no2", "no3"]   -- no4 nem aparece
```

### Parte 3 — o caminho que funciona: editar e reiniciar os TRÊS antigos

```
(3) editar cluster.nos de no1/no2/no3 para incluir no4, e reiniciar cada um
  ok  escrita no master continuou aceita durante o reinicio das replicas -- 0.37s + 0.369s
  ok  no1 volta MASTER sem eleicao -- epoca nao mudou -- 0 -> ('master', 0)

(4) os quatro convergem?
  ok  no4 aparece vivo em no1 depois dos tres reinicios --
      [{'id':'no1','papel':'master','posicao':320,'vivo':True},
       {'id':'no2','papel':'replica','posicao':300,'ultimo_pulso_ms':361,'vivo':True},
       {'id':'no3','papel':'replica','posicao':300,'ultimo_pulso_ms':360,'vivo':True},
       {'id':'no4','papel':'replica','posicao':0,'ultimo_pulso_ms':104,'vivo':True}]
  ok  no4 alcanca a posicao do master (nao so os antigos) -- alvo 370
  ok  os quatro retratos SHA-256 batem --
      {'no1': (370,'15a0c89c86780c04'), 'no2': (370,'15a0c89c86780c04'),
       'no3': (370,'15a0c89c86780c04'), 'no4': (370,'15a0c89c86780c04')}

reinicio_no2_s: 0.37   reinicio_no3_s: 0.369
reinicio_no1_total_s (indisponibilidade do master): 0.367
escalonamento_total_s (os tres reinicios seguidos): 1.741
falhas: []
```

**O achado que não estava documentado em lugar nenhum:** só os nós **antigos**
precisam reiniciar. O nó novo já nasce sabendo dos quatro; quem não sabia
dele são os outros três, e é a lista **deles** que muda. Reiniciar as
réplicas não tirou a escrita do ar (inseri no master enquanto `no2`/`no3`
reiniciavam, e aceitou); reiniciar o master pausou o cluster por 0,367 s —
bem abaixo da janela de 8 s — e ele **não perdeu a época nem foi reeleito**,
porque `base/cluster.estado.json` sobrevive ao reinício.

## O que NÃO existe, e é dispensa registrada

- **Não existe escalonamento a quente.** É medido acima, não suposto: o
  próprio protocolo recusa o pulso de quem não está na lista de quem recebe.
  Escalonar hoje é sempre "editar e reiniciar" — a linha do
  `docs/CLUSTER.md` estava certa; faltava a prova.
- **Não existe remoção de nó a quente**, pela mesma razão espelhada (não
  exercitada nesta rodada: o pedido do dono foi escalonamento, não remoção).
- **Não existe cadastro de nós pela tela.** `docs/PENDENCIAS.md` #208: a
  palavra `cluster` aparece **zero** vezes na interface web; das operações do
  cluster, a tela só chama `spare_promover`. `cluster_nos`/`cluster_estado`
  não têm botão.
- **Não existe quórum de ESCRITA no cluster** (isto não é Raft, por
  desenho): a eleição decide **quem é o master**, não **se uma escrita
  chegou**. O master confirma sem esperar réplica nenhuma — é o assunto
  inteiro do item T.
- **Cluster de dois nós nunca se promove sozinho** (1 de 2 não é maioria):
  para failover automático são precisos três nós ou mais — não exercitado
  nesta rodada (já medido e documentado em `docs/CLUSTER.md` §2.4, item 3).
- **A fresta entre "o master calou" e "os pares envelheceram"** existe e é
  documentada (`docs/CLUSTER.md` §2.4, item 5; `fresta.py`) — não custa dado
  nem abre dois masters, só entrega a liderança ao nó pior informado numa
  queda em sequência. Não reexercitada nesta rodada porque não é o pedido de
  hoje (eleição/promoção/escalonamento).

## Como se refaz

```bash
# eleicao e promocao, com epoca e quem vira master:
python3 bancada/cluster/provar.py /tmp/algum-diretorio

# escalonar um cluster de tres para quatro nos, com os dois caminhos:
python3 bancada/cluster/escalonar.py /tmp/outro-diretorio
```

Os dois limpam os próprios processos ao sair (por PID guardado, nunca
`pkill`). `bancada/cluster/LEIA-ME.md` documenta os dois.
