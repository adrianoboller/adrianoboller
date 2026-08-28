# Cluster, escala e instâncias

Três perguntas juntas, porque as respostas se encostam: **dá para rodar várias
instâncias?**, **dá para clusterizar?** e **dá para escalar?**

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

Provado nesta sessão de duas formas: `bancada/replicacao/montar.py` sobe
**quatro** de uma vez (5800–5803) e mede a replicação entre eles; e o
`docker-compose.yml` sobe três em contêineres separados.

O arranque recusa duas portas iguais no mesmo processo — dados, web, envio e
retorno são conferidas umas contra as outras. Entre processos diferentes quem
recusa é o sistema, com «endereço já em uso».

---

## 2. Cluster — **não, e vale dizer exatamente o que falta**

O HFSQL(R) chama de cluster o seguinte conjunto:

| O que o cluster deles faz | PhxSql |
|---|---|
| Vários servidores aparecem como **um** para o cliente | ✗ o cliente escolhe o endereço |
| Falha de um não impede o acesso | ✗ |
| Replicação automática entre todos, em tempo real | ◐ um caminho só, source → réplica |
| Carga de **leitura** distribuída entre todos | ◐ dá para apontar leitores para réplicas, à mão |
| Adicionar e remover servidor a quente | ✗ |
| Servidor que caiu ressincroniza ao voltar | ✓ **isto existe** — medido: 4.000 eventos em 1,0 s |
| Cliente reconectado automaticamente a um servidor válido | ✗ |

Ou seja: **a peça mais difícil já está pronta** — a réplica que alcança sozinha
e que para quando diverge. O que falta é o que fica *em volta* dela.

### 2.1 O que exatamente falta

1. **Um endereço só.** Hoje o cliente conecta num servidor. Num cluster ele
   conecta no cluster. As saídas são um balanceador na frente (barato, externo,
   e não sabe quem é primário) ou uma lista de endereços no cliente com escolha
   e reconexão (mais trabalho, e é o que o MySQL(R) faz com o *router*).
2. **Saber quem é o primário.** Sem isso não há para onde mandar a escrita
   depois de uma queda. Exige eleição — e eleição correta exige consenso
   (Raft), que é um projeto por si.
3. **Promoção automática.** Hoje é trocar `papel` de `replica` para `source`,
   desligar `somente_leitura` e apontar as outras. É seguro **quando as
   réplicas estão na mesma posição**, e exige conferência quando não estão.
4. **Escrita em mais de um nó.** Não está no roteiro e não deveria estar: sem
   transações, multi-master é uma forma elaborada de perder dado.

### 2.2 O que dá para fazer hoje, sem cluster

O arranjo que funciona **agora**, medido:

```
                      escrita
                         │
                         ▼
                   ┌──────────┐
                   │  MASTER  │  papel: source
                   └────┬─────┘  imagem_da_linha: true
          ┌─────────────┼─────────────┐
          ▼             ▼             ▼
     ┌─────────┐  ┌─────────┐  ┌─────────┐
     │ RÉPLICA │  │ RÉPLICA │  │ RÉPLICA │  somente_leitura: true
     └────┬────┘  └────┬────┘  └────┬────┘
          └────────────┴────────────┘
                    leitura
```

- **Escala de leitura: sim**, e é o ganho grande. Cada réplica atende consulta
  com o dado completo. Quem faz relatório aponta para uma delas.
- **Escala de escrita: não.** Um master só, e a escrita dele é o teto.
- **Disponibilidade: parcial.** Se o master cai, a leitura continua nas
  réplicas e a escrita para até alguém promover uma. É *failover manual*, e
  está descrito em `docs/REPLICACAO.md` §8.

E há um limite medido que precisa ser dito: **a réplica aplica mais devagar do
que o master escreve** — 4.273 eventos/s contra 18.773 linhas/s. Sob carga
sustentada de escrita elas ficam para trás, e a leitura nelas fica velha. A
razão está em `docs/DESEMPENHO.md`.

### 2.3 Cascata, que existe e ajuda

Uma réplica pode ser origem de outra, desde que ela também grave a imagem no
diário. Medido: Master → Slave01 → Slave03 custou 1.827 ms contra 1.679 ms do
primeiro salto. Serve para não pendurar dez réplicas no master.

---

## 3. Escalar — o que escala e o que não

| | Escala | Como |
|---|---|---|
| **Leitura por consulta** | ✓ | réplicas, e `SelectMemory` (87× medido) |
| **Leitura sequencial** | ✓ | slot de largura fixa: 4,8× o MySQL(R) |
| **Tabela grande** | ✓ | partição em volumes por quantidade, período ou letra; a página custa a página |
| **Escrita** | ✗ | um master, e o `.ndx` é 83,5% do custo |
| **Concorrência** | ✗ | **trava global única** — todo acesso a dado se serializa |
| **Tamanho do dado** | ✓ | volumes; o teto é `registros_por_arquivo × max_arquivos` |

**O gargalo de escala mais próximo não é o cluster: é a trava.** Um servidor com
muita gente lendo ao mesmo tempo hoje serializa tudo, inclusive leitura contra
leitura. Trava por tabela — e depois leitura concorrente com escrita — rende
mais, e é mais barato, que qualquer coisa distribuída.

---

## 4. Docker

`Dockerfile` e `docker-compose.yml` na raiz. A imagem final é `scratch`: sem
shell, sem gerenciador de pacotes, só o binário — o que só é possível porque o
projeto não tem dependência externa nenhuma.

```bash
docker build -t phxsql .
docker compose up -d          # um master e duas réplicas
```

Duas coisas medidas e uma não:

- O alvo **musl** produz binário `static-pie`: 3,4 MB o servidor, 1,2 MB o
  cliente. O alvo padrão (gnu) linka `libc.so.6`, `libgcc_s.so.1` e o
  carregador dinâmico — com ele `FROM scratch` **não sobe**.
- O binário musl **roda**: subiu um servidor com ele e o `ping` respondeu.
- O `docker build` em si **não foi executado** — não há daemon Docker na
  máquina em que isto foi escrito. O `Dockerfile` está correto pelo que dá para
  conferir; quem rodar primeiro, confirme.
