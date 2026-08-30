# Os quatro modos de replicação, em contêineres

```bash
rustup target add x86_64-unknown-linux-musl        # uma vez
cargo build --release --target x86_64-unknown-linux-musl --bin phxsqld
python3 bancada/replicacao/docker/provar.py        # ~20 min, tudo
```

Um comando só. Ele constrói a imagem, sobe e derruba dez contêineres em cinco
`compose` diferentes, mede, grava `resultados.json` aqui do lado e **remove
tudo** — os contêineres, as redes e os volumes — mesmo quando falha, porque a
limpeza está no `finally`.

O que ele **não** apaga é `PHX_BASE` (por padrão
`/tmp/phx-docker-replicacao`): os `config.json` de cada nó e os diários dos
servidores ficam lá para quem quiser olhar depois de uma falha. Um `rm -rf`
nele fecha a conta.

Rodar um estágio só: `provar.py a`, `b`, `c`, `d` ou `e`. Para a corrida ser
mais curta, `PHX_LINHAS=5000 provar.py a` (o padrão são 100.000 linhas).

| variável | o que faz | padrão |
|---|---|---|
| `PHX_LINHAS` | linhas da carga do modo A | `100000` |
| `PHX_BASE` | onde os volumes e os `config.json` moram | `/tmp/phx-docker-replicacao` |
| `PHX_IMAGEM` | nome da imagem | `phxsql-bancada:local` |

Portas do hospedeiro: **6801-6853**, uma faixa só desta bancada. Nada aqui usa
`pkill`: os contêineres têm o prefixo `phxrep-` e os processos do estágio de
comparação são derrubados pelo `Popen` que este script guardou.

## Por que Docker, se a bancada de processos já provava os quatro modos

Ela prova que eles **funcionam**. Ela não prova — e não tem como — três coisas,
e são elas a razão desta frente existir.

**1. Endereço.** Com tudo em `127.0.0.1`, o `bind` do source e o endereço que a
réplica procura são o mesmo por acidente. Em contêineres não são: a origem no
`config.json` é o **nome de serviço** (`host: "fonte"`), resolvido pelo DNS
embutido do Docker, e isso obriga o `bind` a ser `0.0.0.0:5000`. O estágio (0)
repõe o defeito de propósito — `bind: 127.0.0.1:5000` dentro do contêiner — e
mede o silêncio: **o vizinho na mesma rede não abre a porta, e a réplica fica
em 0 evento por 20 s sem um único erro em lugar nenhum**. Trocando uma linha
do config, ela alcança em 0,5 s.

**2. Firewall e isolamento.** A §7 do `REPLICACAO.md` desenha um source que
aceita entrada **só** do IP da réplica e **não alcança ninguém**. No loopback
não há o que trancar. Aqui há: rede própria com IPAM fixo, um **intruso** com
o `config.json` de réplica vazado, e `iptables` de verdade dentro do namespace
de rede do source. É o estágio (e), e ele achou o furo desta frente.

**3. Queda e partição.** `docker kill` mata o processo sem chance de fechar
arquivo — o que uma máquina que perde energia faz. E cortar a rede **sem matar
ninguém** só existe aqui: os dois lados continuam vivos, continuam aceitando
escrita, e não se enxergam. É a lição do `BULKINSERT` um degrau adiante —
teste unitário não prova queda de conexão, soquete prova, e contêiner prova o
que soquete no loopback não alcança.

## O que cada arquivo é

| arquivo | o que é |
|---|---|
| `Dockerfile` | `FROM scratch` + o binário musl. Sem shell, sem gerenciador de pacotes: **6,42 MB** de camada, 2,69 comprimidos |
| `compose-a-primary-replica.yml` | modo A — source e réplica na rede `tunel` |
| `compose-b-multi-master.yml` | modo B — alfa e beta, cada um origem do outro |
| `compose-c-spare.yml` | modo C — primário, spare e uma read replica de testemunha |
| `compose-d-read-replica.yml` | modo D — primário e leitor |
| `compose-e-firewall.yml` | a §7 — IPs **fixos** (172.28.90.10/.20/.30) e um intruso |
| `provar.py` | a bancada inteira, com o esperado escrito antes de cada estágio |
| `resultados.json` | a última corrida completa, crua |

### A imagem: por que não é a oficial

A oficial (`phxsql/Dockerfile`) compila **dentro** do contêiner, com
`cargo build --offline` — e isso é o que prova a promessa de zero dependência
externa, então ela fica como está. Só que baixa ~1,5 GB e leva minutos, e esta
bancada sobe e derruba mais de dez contêineres por corrida. Aqui o binário vem
do `cargo build --release --target x86_64-unknown-linux-musl` da máquina:
**mesmo alvo, mesmo `scratch`, mesma ausência de carregador dinâmico**. O
contexto de build é um diretório com um arquivo só, para não mandar o
`target/` inteiro ao daemon.

O alvo é musl e não o padrão porque `FROM scratch` com o alvo gnu não sobe:
falta o `ld-linux` dentro da imagem. É a mesma nota que já está no
`phxsql/Dockerfile`.

## As quatro regras da bancada, aplicadas aqui

O `bancada/LEIA-ME.md` diz que comparação vale quando o **trabalho** é igual,
não só a pergunta. Comparar contêiner com processo é exatamente o tipo de
comparação em que é fácil mentir com número, então:

1. **Mesmos dados.** O gerador é o mesmo `for k in range(...)`, sem sorteio.
2. **Mesmo esquema.** A mesma `criar_tabela`, com a mesma chave primária.
3. **Mesma forma de pergunta.** O mesmo cliente Python, os mesmos lotes de
   5.000, as mesmas sete escritas de atraso.
4. **Mesma quantidade de trabalho.** A função `carga_modo_a` **não sabe** se
   está falando com um contêiner ou com um processo — ela recebe duas
   conexões. É a única forma de garantir que o trabalho é igual: ser o mesmo
   código.

E uma quinta, que esta bancada aprendeu sozinha: **uma amostra de atraso não
é atraso**. A primeira corrida mediu a mesma inserção em 2.035 ms no contêiner
e 53 ms no processo, e a diferença inteira era *onde no ciclo de 2 s do
`reconectar_em`* a escrita caiu. O que se publica agora é a faixa de doze
amostras.

## Duas armadilhas que esta bancada pagou, e que valem para qualquer uma

**O servidor fecha a conexão ociosa em `timeout_s` (30 s).** O laço de
atendimento tem `Err(_) => return`, e o prazo de leitura conta como erro.
Razoável para um servidor; mortal para uma bancada que mede corte de rede de
45 s: a primeira versão morria com `broken pipe` **depois** do corte, e o
número saía como «não se recuperou» quando quem tinha ido embora era o
cliente. Um teste que falha por engano é tão ruim quanto um que passa por
engano. Hoje a classe `Ligacao` refaz a conexão e repete o pedido uma vez.

**A lista negra sobrevive à corrida.** O estágio (e) bate na porta do source
com um intruso, e depois de umas dezenas de recusas o servidor **bloqueia o
IP** — que é exatamente o que ele deve fazer. Só que `blacklist.json` mora no
volume: na corrida seguinte a fase 1, que é a fase **sem tranca nenhuma**,
mediu zero evento roubado. O número estava certo e a conclusão seria errada.
`limpar_dados` apaga `blacklist.json` e `acessos.log` junto com os dados.

## O que a última corrida mediu

Os números crus estão em `resultados.json`; o que eles significam está em
`docs/REPLICACAO.md` §17. Os quatro achados que só o contêiner produziu:

1. **`replicas_autorizadas` não era lido por ninguém** — o intruso levava
   200 de 200 eventos com a lista preenchida. Consertado.
2. **A trava de dados fica presa atrás de uma leitura de rede.** Num corte
   silencioso, `varrer` na réplica esperou ~30 s enquanto `ping` respondia em
   milissegundos. No bidirecional os dois lados se trancam um ao outro, e isso
   acontece **sem corte nenhum** — só com carga simultânea dos dois lados.
3. **O `REDIRECIONA` aponta o endereço da origem configurada**, que dentro de
   um `compose` é um nome de serviço que o cliente do hospedeiro não resolve.
4. **`bind: 127.0.0.1` dentro do contêiner não replica e não avisa.**
