# Concorrência: o que a trava global segura, e qual desenho a substitui

Este documento responde a pergunta que a SP000011 e a SP000016 **deixaram em
aberto de propósito**. O roteiro diz, sobre a SP000011:

> «a premissa está confirmada […] mas **escolher entre trava por tabela,
> `RwLock` e MVCC é outra medição** — e a SP000016 responde parte dela.»
> — `docs/ROTEIRO-1.0.md`

Esta é a outra medição. Ela não repete a §14 do `DESEMPENHO.md` (que já provou
que a trava custa); ela pergunta **o que pôr no lugar**, e mede contra o nosso
formato, não contra o de um motor genérico.

---

## 0. O que esta frente NÃO entrega, e por quê

**Nenhum número novo de concorrência medido nesta rodada.** A máquina não
estava parada: outras frentes compilavam e rodavam testes ao lado durante todo
o trabalho.

Isso não é uma ressalva, é o resultado de uma medição. O `quieta.Vigia`
registrou, nas rodadas de hoje, a máquina entre **26% e 100% ocupada**, com até
**13 tarefas rodáveis além das do próprio arnês**, e — o número que fecha a
questão — **a curva de controle variou 120% dentro de uma única bateria**
(7.968 → 17.561 op/s no `ping`, que nem toma a trava de dados). Entre duas
baterias separadas por minutos, o mesmo controle deu 8.500 e 17.373 op/s.

Um medidor de concorrência tem um modo de falhar que não dá erro: **numa
máquina ocupada a curva achata, e a curva achatada é exatamente o sintoma que
se esperava da trava.** O ruído aponta para o mesmo lado que a hipótese, e sai
com casas decimais. Por isso o arnês desta pasta **recusa imprimir número**
quando a máquina se mexeu, em vez de imprimir com uma ressalva ao lado —
ressalva não viaja junto do número para o documento seguinte.

O que se entrega, então, é o que **não depende de máquina parada**: a medição
estática do código, a matriz de decisão contra o formato em disco, e o arnês
pronto para rodar no dia em que a máquina estiver quieta.

---

## 1. O mapa da trava: quantas seções, e o que cada uma segura

Contar tomadas não escolhe desenho nenhum. Uma seção crítica de 3 µs e uma de
40 ms são dois problemas diferentes, e a contagem não os distingue. O que
decide é **o que cada uma segura enquanto está com a trava na mão**.

O mapa sai de um gerador, `bancada/concorrencia/mapa-da-trava.py`, e não da
leitura de ninguém:

```bash
python3 bancada/concorrencia/mapa-da-trava.py            # o mapa
python3 bancada/concorrencia/mapa-da-trava.py --autoteste # as guardas dele
python3 bancada/concorrencia/mapa-da-trava.py --json      # para outro gerador
```

### 1.1 O número antigo das «13 tomadas fora do ponto único»: remedido, e vale

*Número citado é número que não se mede.* A conta foi refeita hoje:

| o que se conta | medido hoje |
|---|---|
| `self.dados.lock()` em `servidor.rs` | **3 ocorrências**, das quais **2 em comentário** |
| tomadas reais da trava fora do `travar_dados()` | **0** |
| chamadas a `travar_dados()` fora da definição e fora dos testes | **76** |

O item está fechado e **continua fechado**: a catraca `so_um_lugar_toma_a_trava`
lê o próprio fonte pelo `include_str!` e reprova a segunda tomada. O comentário
do `travar_dados()` (o `travar_dados`) voltou a ser verdade.

Mas o número que importa para a SP000011 **nunca foi esse**. «Uma tomada» é o
número de *portas*; **76** é o número de *seções críticas*, e é sobre elas que
qualquer desenho substituto tem de decidir.

### 1.2 As 76 seções, pelo que seguram

O gerador classifica cada seção pela coisa mais grave que ela alcança, e a
ordem da classificação é deliberada: uma seção que roda código do dono também
toca disco, e rotulá-la «disco» esconderia o que ela tem de pior.

| classe | seções | linhas de código sob a trava |
|---|---:|---:|
| **código do dono** (corpo de gatilho `BEFORE`) | **5** | 369 |
| rede ou espera | **0** | 0 |
| **escrita durável** (alcança `fsync`) | 20 | 566 |
| escrita | 14 | 293 |
| leitura com varredura | 27 | 1.795 |
| leitura curta | 10 | 101 |

Tamanho das seções: menor **3** linhas, mediana **26**, p90 **89**, maior
**243** (`op_esquema`), somando **3.124** linhas de código sob a trava.
**8 de 76** soltam a trava cedo por `drop` explícito; **40 de 76** têm laço
direto dentro da própria seção.

Cada seção aparece em **uma** classe, a mais grave — por isso a linha
«escrita durável» diz 20 e o §1.3 abaixo diz que **24** alcançam `fsync`: as
outras quatro estão na linha «código do dono», que é pior. As classes contam
seções; as afirmações do §1.3 contam **fatos**, e um fato pode valer para
seções de classes diferentes.

### 1.3 Os três fatos que o mapa achou, e que decidem a matriz

**(a) O `fsync` acontece com a trava global na mão.** `24 de 76` seções
alcançam `sync_all` por caminho próprio, com confiança 1,0. Conferido à mão, o
caminho do `op_inserir` é:

```
op_inserir
  -> gravar_de_verdade            <- `t.sincronizar()?`, quando a janela fecha
     -> Table::sincronizar        lixeira, bin, memo, log, motivos,
                                  trilha, ndx, reg, e o .pag
        -> NdxFile::sincronizar   DOIS sync_all
```

Não é toda gravação: o `Janela::hora_de_gravar` fecha por
lote — **200 operações ou 200 ms**, o que vier primeiro, no padrão
`durabilidade: por_lote`. Mas com `durabilidade: por_operacao` é **toda**
gravação, e aí cada `INSERT` segura a trava global através de um `fsync`.

**(b) Cinco seções rodam código do dono do banco sob a trava.** Os gatilhos
`BEFORE` (`rodar_gatilhos_antes`) executam o programa do
usuário **dentro** da seção crítica; os `AFTER` não — o próprio comentário do
`rodar_gatilhos_depois` diz «roda os AFTER, já SEM a trava de dados», e o
`drop(_trava)` está no fonte, antes da chamada. As cinco são `op_inserir`,
`op_atualizar`, `op_excluir`, `op_inserir_lote` e `empilhar` (a transação).

~~A duração dessas cinco **não tem teto**: é o que quem escreveu o gatilho
quiser.~~ **Falso, e corrigido em 02/09 com medição.** A frase acima foi
escrita lendo o mapa e não o avaliador. Existe teto, e ele estava no código o
tempo todo: `PASSOS_MAX = 1_000_000` em `phxsql-sql/src/rotina.rs`, e o
comentário dele já nomeia **esta** razão exata — *«num gatilho, roda com a
trava de dados na mão»*.

Só que teto em **passos** não limita a trava: o que limita é teto em **tempo**,
e ninguém tinha feito a conversão. Feita agora, com
`cargo run --release --example custo-do-gatilho -p phxsql-sql`:

| caso | mediana de 3 |
|---|---:|
| pior caso honesto — `WHILE TRUE DO SET x = x + 1`, gastando o orçamento inteiro | **18,3 ms** |
| corpo comum — `IF x < 10 THEN SET x = x + 1` | **1 µs** |

**18,3 ms é o pior que um gatilho consegue impor a todas as outras conexões**,
e ele já é o pior de propósito: um laço infinito. Um corpo honesto custa
dezoito mil vezes menos.

O gatilho `BEFORE` também **não alcança o motor** — ele roda sobre `MotorNulo`,
sem ler nem gravar tabela nenhuma. Não há I/O ali para esticar o número.

**Consequência para a ordem de trabalho:** estas cinco deixam de ser o primeiro
alvo. O argumento de que «nenhum desenho de trava conserta isso» cai junto — um
teto de 18,3 ms é da ordem de grandeza de uma gravação com `fsync`, e não de
uma catástrofe. As **27 seções de leitura com varredura** (1.795 linhas) passam
a ser as candidatas — mas isso é **hipótese, não conclusão**: elas ainda não
foram medidas em tempo de trava, e trocar um palpite por outro não é medir.

**(c) Nenhuma seção atravessa a rede.** O conserto registrado na §4.13 do
`DESEMPENHO.md` **continua valendo**, e o mapa o confirma por outro caminho,
sem ler aquele documento: zero seções na classe `rede-ou-espera`.

> Isto quase saiu errado, e o erro fica registrado: a primeira versão do mapa
> acusava `op_juntar` de «atravessar a rede com a trava na mão», com confiança
> 1,0. Conferido à mão, `juncao.rs` tem **zero** `TcpStream`. A causa eram
> duas: o marcador casava com o **tipo** `TcpStream` numa *assinatura*
> (`Cliente::montar(fluxo: TcpStream, …)` do `replica.rs`), e o `juntar` chama
> uma **fechadura local** chamada `montar`, que o resolvedor por nome
> confundiu com aquela função. Marcador que casa com declaração de tipo mede o
> vocabulário do arquivo, não o que ele faz.

---

## 2. O achado que muda a conversa: a trava não protege a `Instancia`

`crates/phxsql-store/src/catalogo.rs`, o tipo inteiro:

```rust
pub struct Instancia {
    base: PathBuf,
}
```

Um `PathBuf`. Só. E **todos** os métodos dela são `&self` —
`abrir_database`, `databases`, `abrir_tabela`, `abrir_qualificada`,
`todas_as_tabelas`, e até o `criar_database`.

O `dados: Mutex<Instancia>` do `Servidor` **não está protegendo a
`Instancia`**: não há estado mutável nela para proteger. Ele é um **fichas de
exclusão global** para o caminho de dados inteiro, e o estado que ele de fato
guarda está **no disco** — `.reg`, `.ndx`, `.log`, `.trash` —, alcançado por um
`Table` que é **aberto e fechado a cada operação**. O comentário do campo
`sujas` diz isso com todas as letras:

> «Existe porque a tabela é aberta e fechada a cada operação.»

Isso tem uma consequência que precisa estar escrita antes de alguém começar a
SP000011:

> **`RwLock<Instancia>` compila de primeira e está errado.** Como nenhum método
> pede `&mut`, trocar `Mutex` por `RwLock` e pôr `.read()` em toda leitura
> passa no compilador **sem um único erro** — e passa porque o tipo não tem o
> que proteger. Dois escritores segurando guardas de leitura abririam dois
> `Table` sobre os mesmos arquivos, e o `borrow checker` não tem como reclamar.
> A refação mecânica é fácil e silenciosamente errada.

O invariante real da trava é **«uma operação toca os arquivos de cada vez»**.
Quem decide o desenho substituto tem de escrever esse invariante primeiro, e
só então escolher a estrutura que o mantém.

---

## 3. A matriz de decisão, contra o NOSSO formato

| | trava por tabela | `RwLock` (leitor com leitor) | MVCC |
|---|---|---|---|
| **o que compra** | leitores e escritores de tabelas **diferentes** deixam de se ver | leitores da **mesma** tabela deixam de se ver | leitor deixa de esperar **escritor**, e ganha leitura repetível |
| **o que NÃO compra** | nada entre dois clientes na mesma tabela | nada entre leitor e escritor: o escritor continua exclusivo | nada entre dois **escritores**; e nada entre dois leitores, que continuam disputando a estrutura |
| **o invariante que precisa manter** | um conjunto de arquivos por tabela, uma operação de cada vez | leitura não pode abrir `Table` que escreva; hoje **abrir já escreve** quando `recursos.espelho` está ligado | versão velha visível a quem começou antes |
| **o que custa no nosso formato** | um mapa de travas por `"database/tabela"`, e a ordem canônica para não trocar um gargalo por um abraço mortal — a ordem já existe no `TRANSACOES.md` §11.3 | **precisa de um `Instancia` com estado**, ou o `RwLock` não protege nada (§2) | mudança de formato: `.reg` v6, área de undo, `.ndx` com visibilidade, purga (§4) |
| **quanto há para recuperar** | não medido nesta máquina | teto estático: **37 de 76** seções não alcançam marcador de escrita por caminho próprio | teto estático: **24 de 76** alcançam `fsync`, e **5 de 76** rodam código do dono, sob a trava |
| **quebra alguma pétrea?** | não | não | **não, se e só se** a versão velha ficar **fora** do `.reg` (§4.2) |

> **O «teto estático» conta SEÇÕES, e não tráfego.** Trinta e sete de setenta e
> seis seções serem de leitura pura não quer dizer que 49% do trabalho seja de
> leitura pura: quem decide isso é a frequência com que cada operação é
> chamada, e isso o fonte não diz. O número serve para o que ele é — um teto, e
> um teto que **descarta** desenho (um teto de 1,05× fecharia a questão) —, e
> não para prever ganho. Prever ganho é a medição do `escolher-o-desenho.py`, e
> ela precisa de máquina parada.

E há uma **quarta linha, que nenhuma das duas sprints lista** e que o mapa
tornou visível:

> **Encurtar a seção crítica antes de trocar a trava.** Oito seções já soltam a
> trava cedo por `drop` explícito; as outras 68 a seguram até o fim do bloco. A
> maior tem 243 linhas. Encurtar não pede desenho novo, não pede formato novo,
> e **melhora os três desenhos acima** em vez de competir com eles: seja qual
> for a trava, ela é segurada por menos tempo. O caso mais gritante é o (b) do
> §1.3 — o corpo do gatilho `BEFORE` roda dentro da seção, e o `AFTER` já
> mostrou que dá para sair dela antes.

### 3.1 A ressalva que a matriz precisa carregar: o formato da carga

Os números da §14 do `DESEMPENHO.md` comparam um `varrer` de **50 linhas**
contra um `inserir` de **uma**. Nessa forma, a leitura custa 20× mais por
operação (153 contra 3.462 op/s) e portanto **segura a trava 20× mais tempo** —
o que favorece o `RwLock` por construção.

Isso não invalida a medição; invalida **generalizá-la**. Um perfil de carga em
que a escrita domina inverte a conclusão, e a resposta certa muda com ele. O
`escolher-o-desenho.py` mantém as mesmas formas de propósito, para as curvas
serem comparáveis com a §14 — e é por isso que este parágrafo existe ao lado
delas.

---

## 4. MVCC contra o nosso formato: a §11.1 do `TRANSACOES.md` ainda vale?

O `docs/TRANSACOES.md` §11.1 diz **«MVCC — não implementar»**, e o motivo é:

> «Uma segunda versão da linha pede um segundo slot, logo um segundo rowid — e
> isso quebra **duas** coisas ao mesmo tempo: a regra pétrea da ordem de
> digitação, e a replicação, cujo `aplicar_evento` **para** quando o rowid
> diverge do que o source mandou.»

### 4.1 O motivo está certo — e é objeção a UMA implementação, não ao MVCC

Conferido no fonte, cada elo daquela frase é verdadeiro:

* o endereço sai de uma conta e não de uma busca —
  `offset = data_offset + (slot - 1) * slot_size` (o cabeçalho do `reg.rs`);
* `aplicar_evento` **para** quando o rowid diverge, e a mensagem diz isso:
  «a replicação para aqui em vez de espalhar a divergência»
  (`Table::aplicar_evento`);
* e a réplica só tem rowid igual ao do source porque **ninguém mais escreve
  nela** e todo evento é aplicado na ordem (`REPLICACAO.md` §5).

O que a frase assume, sem dizer, é **onde a versão velha mora**: ela assume que
mora num slot novo do `.reg`. É a suposição que carrega a conclusão inteira, e
é exatamente a que o oráculo do InnoDB derruba — lá a versão velha **não** vai
para a tabela: vai para a **área de undo**, e a linha da tabela carrega um
ponteiro para ela (`DB_ROLL_PTR`). A chave primária nunca muda.

Trazido para cá: se a versão velha ficar **fora** do `.reg`, então

* o `.reg` continua ganhando **um slot por linha nova**, e nenhum por versão —
  a **ordem de digitação** fica intacta;
* o rowid continua sendo o endereço da versão **corrente** — o **O(1)** fica
  intacto;
* a réplica continua alocando os mesmos slots na mesma ordem — a
  **replicação** fica intacta, e a área de undo é estado **local** de cada
  servidor, como um cache: **não se replica**. Se alguém a replicar, a objeção
  da §11.1 volta inteira e com razão.

**Então a §11.1 e a SP000016 não se contradizem: a SP000016 passa ao lado da
objeção, em vez de derrubá-la.** E isso muda o que a §11.1 deve dizer: não
«MVCC não cabe», e sim **«MVCC com as versões dentro do `.reg` não cabe»** — que
é uma afirmação mais forte, porque descarta um desenho específico com um motivo
específico, em vez de fechar o assunto.

### 4.2 O que ninguém contou ainda: o preço, no nosso formato

O roteiro diz que «o `rowid` daqui já é» a identidade estável que ancora a
cadeia. É verdade, e é a boa notícia. O que ele não diz é o resto da conta, e o
resto da conta é do DBA:

**(1) O cabeçalho do slot está CHEIO.** São 24 bytes
(`SLOT_CAB`, no `reg.rs`), e os 24 estão usados:

```text
[status u8][flags u8][res u16][crc32 u32][versao u64][tempero u64]
   1     +    1     +    2    +    4     +    8      +    8       = 24
```

Um ponteiro para a versão anterior não cabe. Ele exige `slot_size` maior, logo
**`.reg` versão 6**. O precedente existe e é bom: a cifra de coluna já fez a v4
virar v5 crescendo o slot, com a versão no byte 8 decidindo quantos bytes ler
(o «Por que uma versao NOVA» do `reg.rs`). O mecanismo está provado — mas **é mudança de formato em
disco**, e a pétrea desta casa diz que mudança de formato entra **cedo**,
enquanto não há dado em produção. Isso é um argumento *a favor* de decidir a
SP000016 agora, e não depois.

**(2) O `versao u64` que já existe não serve para visibilidade.** Ele é um
contador **por linha**, começando em 1, e existe para a guarda de conflito de
escrita (`conferir_versao_pedida`). MVCC precisa de uma
ordem **global** — um identificador de transação e uma visão de leitura — para
responder «esta versão já existia quando eu comecei?». São duas coisas
diferentes com o mesmo nome, e confundi-las custaria uma implementação inteira.

**(3) O `.ndx` não tem noção de visibilidade.** Ele mapeia chave → rowid. Uma
alteração que mude coluna indexada precisa manter a entrada **velha** legível
para quem começou antes, e hoje não há onde marcar isso. É o mesmo problema que
o InnoDB resolve com registro *delete-marked* no índice secundário mais o
undo — e é trabalho que não aparece em «cadeia de versões ancorada no rowid».

**(4) A purga parece a SP000014, e não é.** Versão velha acumula enquanto
houver leitor aberto — medido no próprio oráculo do roteiro: a *history list*
foi de 7 a 207 e voltou a 0. Recolher esse espaço é **reuso de espaço**, que é
justamente o que a SP000014 teve **recusado pelo dono**. A distinção precisa
estar escrita antes de alguém trombar nela: a recusa é sobre o **`.reg`
reaproveitar slot excluído**, e a área de undo é **outro arquivo**. Reusar
espaço lá não toca a ordem de digitação de nada. Mas quem ler «purga» vai
lembrar da recusa, e tem de encontrar esta linha.

**(5) MVCC não é substituto da SP000011.** Ele tira o leitor de trás do
escritor. Ele **não** tira o leitor de trás de outro leitor, nem o escritor de
trás de outro escritor. Com a trava global no lugar, um servidor com MVCC
continua serializando tudo — porque a trava é tomada antes de qualquer noção de
versão existir.

### 4.3 A parte da §11.1 que ENVELHECEU

O último parágrafo da §11.1 diz:

> «O que continua faltando — e que só o MVCC daria — é **leitura repetível** ao
> longo de um leitor longo, e **ler o que a própria transação escreveu**.»

**A segunda metade deixou de ser verdade em 02/09**: a SP000006
(*read-your-own-writes*) está **feita**, pela `Sobreposicao` presa ao handle da
tabela, e sem MVCC nenhum. O próprio roteiro registra a medição por soquete
(1→**2**→2→3→2).

Fica faltando **uma** coisa, e não duas: leitura repetível. A frase precisa ser
corrigida, e a correção **enfraquece** o argumento a favor do MVCC, não o
contrário — meia razão a menos para implementá-lo.

*(Mudança proposta, não aplicada: `docs/TRANSACOES.md` não é desta frente.)*

---

## 5. As premissas que não sobreviveram a esta rodada

**(1) «A SP000016 responde parte da escolha da SP000011.»** O roteiro põe a
SP000011 depois da SP000016 por esse motivo. A parte que a SP000016 responderia
é a do leitor esperando escritor — e **não é a parte que foi medida**.

A medição da §14 que confirmou a premissa da SP000011 rodou **N leitores, sem
nenhum escritor** (o modo `ler` do `a-trava-serializa.py` roda `n` clientes
idênticos, todos `varrer`). O ganho de 1,51–1,59× contra 1,99× do controle é,
portanto, **serialização de leitor com leitor** — e isso é exatamente o que o
MVCC **não** conserta e o `RwLock` conserta. A escolha que a SP000016
responderia é sobre outro par.

Isso não é contradição escondida: a própria §14 do `DESEMPENHO.md` já escrevia
«o que favorece o `RwLock`, mas *favorecer* não é medir». O que esta frente
acrescenta é que a **ordem do roteiro anda no sentido contrário dessa pista**.

**(2) «Trava por tabela não compra nada, porque tabelas separadas escalam
igual.»** Esta já estava morta e bem enterrada na §14, e fica registrada aqui
para não voltar: com uma trava **global**, clientes em tabelas diferentes
disputam exatamente como na mesma tabela. O experimento confirma a construção,
não julga um desenho que ninguém escreveu. O que aquele número diz, e é muito,
é que **não há um segundo gargalo escondido embaixo da trava**.

**(3) «`RwLock` é a refação barata.»** Morta pelo §2: barata de escrever,
silenciosamente errada de semântica, porque a `Instancia` não tem estado a
proteger e o compilador não tem o que reclamar.

---

## 6. O arnês, e como ele recusa

Três arquivos em `bancada/concorrencia/`:

| arquivo | responde |
|---|---|
| `mapa-da-trava.py` | **estático.** Quantas seções críticas, e o que cada uma segura. Não depende de máquina parada |
| `a-trava-serializa.py` | «a trava custa?» — vazão com N clientes, com curva de controle |
| `escolher-o-desenho.py` | **«o que pôr no lugar?»** — o teto de cada um dos três |
| `quieta.py` | o vigia que decide se algum número da rodada vale |

```bash
python3 bancada/concorrencia/mapa-da-trava.py --autoteste   # as guardas dele
python3 bancada/concorrencia/mapa-da-trava.py               # o mapa
python3 bancada/concorrencia/a-trava-serializa.py           # a trava custa?
python3 bancada/concorrencia/escolher-o-desenho.py          # qual desenho?
SEGUNDOS=5 CLIENTES=1,2,4 python3 bancada/concorrencia/escolher-o-desenho.py
```

Os dois medidores dinâmicos precisam de `cargo build --release` antes, e usam
**portas 7600–7699**, escolhidas livres dentro da faixa. Matam o servidor pelo
PID — nunca `pkill`, que derrubaria o de outra frente.

### 6.1 Como o `escolher-o-desenho.py` separa os três

Nenhum dos três desenhos existe, então não há o que cronometrar contra o quê. O
que se mede é o **teto** de cada um — quanto de paralelismo ele teria para
recuperar, se fosse perfeito. Um desenho cujo teto é 1,05× está respondido
antes de começar.

* **trava por tabela**: `leitura em tabelas separadas` contra `leitura na mesma
  tabela`;
* **`RwLock`**: a curva de **controle** (o `ping`, que não toma a trava) contra
  a de leitura — a distância entre as duas é o paralelismo que a exclusividade
  entre leitores está comendo;
* **MVCC**: o **p99** de um `varrer` com um escritor ao lado contra o p99 do
  mesmo `varrer` sozinho. Este é o único par que o `RwLock` não mexe, porque o
  escritor continua exclusivo nos dois.

A distribuição, e não a média, porque **é a média que esconde o que se
procura**: uma parada de dezenas de milissegundos num `varrer` de centenas de
microssegundos, uma vez a cada 200 gravações, some na média e aparece inteira
no p99.

E a bateria roda **duas vezes**, em `durabilidade: por_lote` e
`durabilidade: por_operacao` — porque §1.3(a) mostrou que é a durabilidade que
decide se o `fsync` sob a trava acontece a cada 200 gravações ou em todas. São
dois servidores diferentes vestidos com a mesma roupa, e o desenho certo pode
não ser o mesmo para os dois.

### 6.2 Como ele recusa

O `quieta.Vigia` mede três coisas e reprova por qualquer uma:

1. **a ocupação de fundo** (`/proc/stat`) nas duas pontas da bateria e durante
   cada rodada — as pontas sozinhas não pegam o vizinho que começa no meio;
2. **as tarefas rodáveis** (`procs_running`), **descontando as do próprio
   arnês** — a primeira versão acusava «4 tarefas além do medidor» numa rodada
   de dois clientes, e as quatro eram os dois clientes, o servidor e o
   amostrador. *Instrumento que acusa a si mesmo recusa sempre, e recusar
   sempre não é mais útil que nunca recusar;*
3. **a curva de controle no começo e no fim** — se o `ping`, que nem toma a
   trava, desacelerou entre as duas pontas, quem desacelerou foi a máquina, e a
   bateria inteira perdeu a comparação. Foi este critério que pegou os 120% de
   hoje.

Reprovado, **nenhum número sai**. O `--mesmo-sujo` existe só para depurar o
próprio arnês, carimba tudo o que imprime e sai com código 1.

### 6.3 A prova real do medidor estático

Um medidor estático nunca quebra: ele passa a responder outra coisa. Por isso o
`--autoteste` repõe, uma a uma, **as seis falhas que este medidor de fato
teve**, e cada guarda falha com o defeito de volta:

| guarda | o defeito que ela repõe |
|---|---|
| comentário e texto não contam | varrer o fonte cru — o cabeçalho do próprio medidor escreve `sync_all` e viraria uma tomada de disco |
| `'{'` não vira chave | contar chave no fonte cru: o bloco fecharia no lugar errado e a seção sairia com o tamanho de outra |
| fechadura local não resolve para função de fora | o `op_juntar` classificado «atravessa a rede» pelo `montar` do `replica.rs` |
| a cadeia de três saltos é alcançada | a guarda de ciclo por nome, que cortava em `sincronizar -> sincronizar` e escondia o `fsync` do `op_inserir` |
| homônimo vira confiança medida | unir todos (o `op_varrer` gravaria) ou exigir unanimidade (o `op_inserir` não faria `fsync`) |
| tomada em `#[cfg(test)]` fica de fora | procurar «`mod testes`» achava o primeiro dos **dez** deste arquivo |

O quinto merece uma linha própria, porque é o método e não um detalhe. Nesta
árvore há nome com **23** definições (`abrir`), **33** (`para_json`), **35**
(`nome`). Escolher uma seria chutar; unir todas seria dizer que o `op_varrer`
grava em disco. Então **não se escolhe: conta-se** — cada salto do caminho
carrega a fração das definições daquele nome que alcançam o marcador
(`sincronizar(9/9)`), e a confiança do caminho é a **menor** fração dele.
Confiança 1,0 é afirmação; abaixo disso é suspeita com endereço, e quem lê
confere o caminho em vez de acreditar no rótulo.

---

## 7. A recomendação sobre a ordem SP000011 × SP000016

**Trocar a ordem, e partir a SP000011 em duas.**

1. **Encurtar as seções críticas** (a quarta linha da matriz, §3). Não é
   sprint nova, não muda formato, não escolhe desenho, e melhora os três. Começa
   pelas cinco que rodam gatilho `BEFORE` sob a trava — o `AFTER` já provou que
   dá para sair antes.
2. **A parte da SP000011 que a medição já justifica**: separar leitor de leitor.
   A premissa está medida e é **de leitor com leitor** (§5.1), que é o par que a
   SP000016 não toca. E escrever antes o invariante do §2, porque a refação
   óbvia compila e está errada.
3. **SP000016 (MVCC)**, com a decisão de formato tomada **cedo** — o `.reg` v6
   e a área de undo (§4.2). É o item mais caro dos três e o único que muda o
   disco; adiar não o torna mais barato, torna mais caro.
4. **Trava por tabela**, se ainda sobrar disputa depois de (2) — e aí com o
   `escolher-o-desenho.py` a dizer quanto sobrou, em vez de por dedução.

**O motivo da troca, em uma frase:** o roteiro põe a SP000011 depois da
SP000016 porque «a SP000016 responde parte da escolha» — e a parte que ela
responde não é a parte que foi medida.

**A condição:** os itens 2 e 4 pedem **uma máquina parada** para fechar. O item
1 e a decisão de formato do item 3 não pedem, e é por isso que estão primeiro.

---

## 8. O que continua por medir, e nomeado

* **O teto do MVCC**, que é o p99 do leitor com um escritor ao lado. O arnês
  mede; falta a máquina parada. **Não medido, e não estimado.**
* **O teto do `RwLock` na espera** (p99 de dois leitores contra um). Idem.
* **O perfil de carga real.** A §3.1 mostra que a resposta muda com a razão
  entre o tempo de leitura e o de escrita, e o número de hoje vem de um
  `varrer(50)` contra um `inserir(1)`.
* **O custo do `fsync` sob a trava, em milissegundos.** O mapa prova que ele
  acontece ali; quanto ele dura é medição, e não foi feita.
* **Uma catraca para o mapa.** O `mapa-da-trava.py` está pronto para virar
  guarda de QA — «nenhuma seção nova roda código do dono sob a trava», «o
  número de seções que alcançam `fsync` só desce». Não entrou porque exigiria
  mexer em `crates/`, que não é desta frente.

---

## 9. Fontes deste documento

Tudo o que está numerado aqui saiu de um destes lugares, e nenhum número foi
digitado à mão.

**As referências são por NOME, e não por linha, e isso é decisão.** O
`servidor.rs` tem 22 mil linhas e é editado por várias frentes ao mesmo tempo:
enquanto este documento era escrito, o `gravar_de_verdade` andou de 9.006 para
9.016 e o `travar_dados` de 869 para 906. Número de linha em arquivo assim é
número que envelhece calado — e um endereço errado é pior que endereço nenhum,
porque manda procurar no lugar errado com ar de precisão. Nome se acha com
`grep` e sobrevive à edição de quem está ao lado.

| número | de onde |
|---|---|
| 76 seções, as seis classes, 37/24/5/8/40, os tamanhos | `bancada/concorrencia/mapa-da-trava.py` |
| 0 tomadas fora do ponto único | `grep -c 'self\.dados\.lock()' servidor.rs`, e a catraca `so_um_lugar_toma_a_trava` |
| 24 bytes do cabeçalho do slot | `SLOT_CAB` e o layout do módulo, em `crates/phxsql-store/src/reg.rs` |
| `Instancia` com um campo | `pub struct Instancia`, em `crates/phxsql-store/src/catalogo.rs` |
| 200 operações / 200 ms | `lote_operacoes` / `lote_milissegundos`, em `crates/phxsql-server/src/config.rs` |
| 1,99× / 1,51–1,59× | `docs/DESEMPENHO.md` §14 — **medição anterior, não refeita aqui** |
| 26–100% ocupada, 13 vizinhos, 120% no controle | `bancada/concorrencia/quieta.py`, nas rodadas de hoje |
