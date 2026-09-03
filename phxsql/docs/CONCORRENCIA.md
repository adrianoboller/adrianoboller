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

> **Lido em 03/09:** esta seção é da rodada de 02/09 e continua valendo como
> registro do método. A rodada de 03/09 **conseguiu** duas baterias limpas — o
> `quieta.Vigia` aprovou as duas —, e elas estão na §7.1. Foram necessárias
> **seis tentativas** para conseguir as duas, e as outras quatro não imprimiram
> número nenhum: é o arnês funcionando, e não um obstáculo a ele.

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
| **código do dono** (corpo de gatilho `BEFORE`) | **5** | 378 |
| rede ou espera | **0** | 0 |
| **escrita durável** (alcança `fsync`) | 19 | 514 |
| escrita | 14 | 293 |
| leitura com varredura | 28 | 1.844 |
| leitura curta | 10 | 101 |

Tamanho das seções: menor **3** linhas, mediana **26**, p90 **89**, maior
**243** (`op_esquema`), somando **3.130** linhas de código sob a trava.
**8 de 76** soltam a trava cedo por `drop` explícito; **40 de 76** têm laço
direto dentro da própria seção.

Cada seção aparece em **uma** classe, a mais grave — por isso a linha
«escrita durável» diz 19 e o §1.3 abaixo diz que **23** alcançam `fsync`: as
outras quatro estão na linha «código do dono», que é pior. As classes contam
seções; as afirmações do §1.3 contam **fatos**, e um fato pode valer para
seções de classes diferentes.

> **Os números desta tabela mudaram entre 02/09 e 03/09 sem ninguém mexer neles,
> e é por isso que o gerador existe.** A tabela dizia 20 / 27 / 24, e hoje o
> `mapa-da-trava.py` diz 19 / 28 / 23. Não foi esta frente: rodado contra a
> árvore em `5ae9319` — antes de qualquer commit meu — já dava 23. Foram as
> frentes vizinhas mexendo nas cadeias que o mapa percorre. *Gerador que existe
> e ninguém roda de novo é número digitado à mão com passo extra.*

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

> **E a correção acima também estava errada — ver §1.4.** «Existe teto» era
> verdade sobre os PASSOS e falso sobre a TRAVA, e a diferença entre as duas
> custou a medição de 03/09: **teto de passos não é teto de trabalho.**

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

### As 27 de varredura, medidas — e o alvo é UMA delas

Feito em 02/09 com `bancada/concorrencia/custo-da-varredura.py`, numa tabela de
50.000 linhas com coluna `Memo`. O arnês mede **dois vizinhos** e a diferença
entre eles é o experimento inteiro: o `ping` **não toma a trava** (responde
direto no despachar) e o `ler` **toma**. Sem esse par, a primeira rodada quase
publicou *«a varredura de 357 ms não atrapalha ninguém»* — o instrumento não
passava pelo que devia medir.

| a varredura | dura | o vizinho que toma a trava espera |
|---|---:|---:|
| ordem de digitação, qualquer `pular` | 4,2–6,1 ms | 2,1–4,2 ms |
| **por ÍNDICE** | **192,5 ms** | **190,0 ms — 1.847× a base** |

**Duas conclusões, e a primeira derruba outra premissa.**

**O `pular` NÃO anda linha a linha.** `pular 10.000` e `pular 190.000` custam o
mesmo, porque `pagina_por_posicao` bisseta pelo `rownum` — vinte leituras, não
cento e noventa mil. A frase «o `OFFSET` do SQL anda até lá» é verdadeira em
outros motores e **falsa aqui**.

**O caminho por ÍNDICE segura a trava por 98,7% da própria duração**, e é 40×
mais longo. A causa está no `op_varrer`: por índice ele faz
`varrer_indice` (todos os rowids, em ordem de chave) → `filtrar` (que **lê cada
um**) → `skip(pular).take(max)`. O custo cresce com a **tabela**, e não com a
página — que é exatamente o defeito que a paginação por cursor existe para não
ter, sobrevivendo no caminho por índice.

**O alvo da SP000011 muda de forma:** não é «encurtar as 27», é **fazer o
caminho por índice paginar**. O `.ndx` é uma B+tree; descer até a posição e ler
só a página é o que o caminho de digitação já faz. Enquanto isso não existe,
uma consulta ordenada por índice numa tabela grande **para o servidor inteiro**
pelo tempo dela.

### O invariante da trava, agora escrito para o COMPILADOR

Segundo item da ordem recomendada, e ele vinha antes de qualquer refação: *«o
invariante se escreve antes de mexer na trava».* Feito em 02/09.

**O que a trava protege, e o que ela não protege.** O `servidor.rs` guarda a
`Instancia` num `Mutex`, e é fácil ler isso como «o mutex protege a instância».
Não protege: ela tem **um** campo, um `PathBuf` imutável, e **todo** método é
`&self` — inclusive `criar_tabela`, `excluir_tabela` e `renomear_tabela`, que
escrevem. Não há estado mutável ali dentro.

O mutex é uma **ficha de exclusão**: quem a tem mexe no disco. O estado
protegido está lá fora, nos arquivos. Isso é convenção — e convenção que o
compilador não conhece é convenção que uma refação apaga em silêncio.

**A enforcement custa uma linha e zero bytes.** `Mutex<T>` exige `T: Send`;
`RwLock<T>` exige `T: Send + Sync`. Um campo marcador de tamanho zero
(`PhantomData<Cell<()>>`) torna a `Instancia` **`!Sync`**: o `Mutex` continua
compilando, e o `RwLock` **para de compilar**.

Prova real, nos dois sentidos:

| o que se faz | o que acontece |
|---|---|
| tirar o campo marcador | o teste reprova: *«a Instancia virou Sync…»* |
| trocar por `RwLock<Instancia>` | **o projeto não compila** — `` `Cell<()>` cannot be shared between threads safely ``, *required for `RwLock<Instancia>` to implement `Sync`* |

E o custo é **medido, não afirmado**: o teste confere que
`size_of::<Instancia>() == size_of::<PathBuf>()`.

**Isto não proíbe mudar o desenho** — exige que quem mudar **veja** o motivo
primeiro, no comentário para onde o erro do compilador aponta, em vez de
descobri-lo por um dado corrompido. Trocar a trava passa a exigir decidir antes
o que protege o disco; aí o campo sai junto, de propósito e por escrito.

### O conserto: o caminho por índice PARA na página

Feito em 02/09, e ele é o que a medição pediu. `Table::pagina_por_indice`
substitui `varrer_indice` → `filtrar` → `skip().take()` por um laço que **para
quando a página fecha**. A conta passa a ser `pular + limite` leituras, e não o
tamanho da tabela — na primeira página, que é a esmagadora maioria, são
`limite`.

Na mesma tabela de 50.000 linhas com `Memo`, e com o arnês confirmando a
máquina quieta nas duas pontas:

| a varredura por índice | dura | o vizinho espera |
|---|---:|---:|
| antes, com a regressão do `filtrar` | 101,9 ms | 96,9 ms |
| depois de consertar o `filtrar` | 40,9 ms | 34,7 ms |
| **depois de PARAR na página** — `pular 20.000` | **22,1 ms** | **17,3 ms** |
| **primeira página** (o caso comum) | **12,7 ms** | **7,9 ms** |

**8× mais rápida que o ponto de partida, e 12× menos espera** para quem estava
na fila. O que restou de custo é honesto: `pular 20.000` ainda lê vinte mil
linhas, porque o `pular` conta linhas **visíveis** e não entradas do índice.
Descer a B+tree até a posição exigiria o índice guardar quantas linhas vivas
existem abaixo de cada nó — outra medição, outra sprint.

**A prova real desta mudança quase passou por engano**, e o caso está no teste:
a primeira versão dele usava `excluir`, que é exclusão **física** — ela tira a
entrada do índice junto, então não sobrava linha invisível na lista e filtrar
antes ou depois do `pular` dava o mesmo resultado. Com `excluir_suave` os dois
caminhos divergem, e o defeito reposto reprova nomeando o caso:
`visao Ativas, pular 3, limite 5` → `[4,5,7,8,10]` contra `[5,7,8,10,11]`.

### A regressão que esta medição achou, e ela era minha

Medindo, apareceu que o `filtrar` — o laço quente acima — passou a decodificar
a linha **com os anexos** (`.bin` e `.memo`) para olhar **um bit** da coluna de
sistema. Entrou na SP000006, junto com a sobreposição do read-your-own-writes,
e **não apareceu na primeira medição porque a tabela de prova não tinha coluna
externa**.

Prova real, na mesma tabela com `Memo`:

| | dura | o vizinho espera |
|---|---:|---:|
| com o defeito | 101,9 ms | 96,9 ms |
| **sem o defeito** | **40,9 ms** | **34,7 ms** |

**2,5× mais rápida, e 2,8× menos espera para quem está na fila.** O conserto é
ler o *payload* e perguntar ao byte da coluna de sistema, em vez de montar a
linha inteira — que é o que o código fazia antes de eu mexer, e o que o
`visao_aceita_payload` já sabia fazer.

### 1.4 O teto que existia não era o teto que se precisava (03/09)

O §1.3(b) fechou dizendo que os 18,3 ms do `PASSOS_MAX` resolviam as cinco
seções que rodam código do dono, e que **elas deixavam de ser o primeiro alvo**.
A conclusão estava errada, e o erro é o mesmo de sempre com outra roupa: o
medidor só media corpos cujo **passo** é barato.

**Teto de PASSOS não é teto de TRABALHO.** O `PASSOS_MAX` limita quantos passos
um corpo dá; ele não limita o que **um** passo faz. E havia um passo sem fundo:

```sql
CREATE TRIGGER incha BEFORE INSERT ON t FOR EACH ROW
  WHILE TRUE DO SET NEW.x = CONCAT(NEW.x, NEW.x); END WHILE
```

`CONCAT(s, s)` dobra o texto a cada volta. Trinta passos de um orçamento de um
milhão chegam a um gigabyte — e o corpo **não morre no teto**: morre no
alocador. Em Rust, alocação que falha **aborta o processo**. Medido, com o
processo limitado a 2 GiB por `ulimit -v 2000000`:

```text
memory allocation of 536870912 bytes failed
```

**10,2 s com a trava global de dados na mão, e então o servidor inteiro cai** —
todas as conexões, por um gatilho que o dono do banco escreveu. Não é uma
conexão lenta; é uma negação de serviço com a assinatura de quem modelou o
banco.

E o segundo furo, que sobrevive a qualquer teto de tamanho: um corpo cujo passo
é **caro mas limitado**. `WHILE TRUE DO SET s = CONCAT('x', s)` sobre um texto
de 512 KiB cresce um byte por volta — nenhum teto de tamanho morde — e gasta o
orçamento inteiro de passos copiando meio megabyte por vez.

#### Os três tetos, e o que cada um pega

| teto | limita | o pior caso, medido |
|---|---|---:|
| `PASSOS_MAX` (já existia) | quantos passos o corpo dá | 27,2 ms de aritmética |
| **`TEXTO_MAX`** (03/09), 64 MiB | o que **um** passo aloca | 905,9 ms, com 0,0163% do orçamento de passos |
| **prazo de parede** (03/09), 500 ms | o que a **trava** segura | 28.590 ms → **500,2 ms**, 57× menos |

O corpo honesto custa **1 µs** e não vê nenhum dos três.

**Por que 500 ms, e não um número de gosto.** Sai de dentro desta casa:
`transacao_lock_timeout_ms` já vale 500, e é a resposta que este servidor já deu
para «quanto uma conexão pode fazer outra esperar». O pior corpo que ainda
*termina* custa 27,2 ms — 22× de folga, então nenhum corpo que hoje funciona
quebra.

**O prazo é do CHAMADOR, e isso é decisão.** O mesmo avaliador roda o `BEFORE`
(com a trava global) e o corpo de um **procedimento**, que roda sem trava
nenhuma e pode legitimamente varar a tarde inserindo. Um prazo fixo dentro do
avaliador serviria a um e quebraria o outro. Então quem toma a trava paga o
prazo, e quem não toma **não paga nem a leitura do relógio** — o portão vem
antes do trabalho, e é um teste de `Option` por passo.

#### A prova real, nos dois sentidos — e o teste que passava por engano

| defeito reposto | o que acontece |
|---|---|
| `TEXTO_MAX` sem efeito | o teste **não falha: ABORTA** o `cargo test`, em 13,9 s |
| `.com_prazo(...)` removido do `rodar_gatilhos_antes` | o mesmo corpo roda **79,8 s** em vez de 50 ms |

E o achado que justifica a catraca estática: **com o prazo removido, o teste de
ponta a ponta continua passando**, em 0,97 s. Ele promete «não derruba o
servidor», e isso continua verdade — o teto de texto segura *aquele* corpo. Quem
perde a garantia é o corpo de passo caro, que nenhum teto de tamanho vê. Por
isso a guarda é a catraca `o_before_roda_com_prazo_e_o_after_nao`, que lê o
próprio fonte pelo `include_str!`: ela é a **única** que acusa.

O irmão dela fica **manual, e isso é decisão escrita**: repor o `TEXTO_MAX` faz
o binário alocar até o alocador falhar — 8 a 16 GiB nesta máquina, com risco de
o kernel matar o processo de outra frente. A receita é `ulimit -v 2000000`, e
está no comentário do teste. *Guarda que derruba o trabalho do vizinho é a mesma
falha do zelador que apaga o `target` de quem está compilando.*

```bash
cargo run --release --example custo-do-gatilho -p phxsql-sql
```

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
| `quanto-a-trava-fica-presa.py` | **«quanto a trava fica PRESA?»** — o µs de posse por operação, lido por dentro (telemetria), com o par `por_lote` × `por_operacao` isolando o `fsync` |
| `quieta.py` | o vigia que decide se algum número da rodada vale |

```bash
python3 bancada/concorrencia/quanto-a-trava-fica-presa.py   # quanto ela fica presa
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

1. ~~**Encurtar as seções críticas**~~ — **feito o pedaço que dava para fazer,
   em 03/09**, e ele mudou de forma no caminho. As cinco que rodam gatilho
   `BEFORE` sob a trava não precisavam ser *encurtadas*: precisavam de **teto**,
   porque o comprimento delas não é escrito aqui — é escrito por quem modelou o
   banco. Hoje há dois (§1.4), e o pior caso caiu de «até o processo abortar»
   para **500 ms**. As 68 que a seguram até o fim do bloco continuam como
   estavam, e a §7.1 diz por quê: o pedaço grande da trava, no padrão, não está
   no `fsync` — está na leitura, que segura 23× mais.
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

## 7.1 Quanto a trava fica PRESA, medido (03/09)

O §8 listava como **não medido** o custo do `fsync` sob a trava, em
milissegundos. Está medido, e o número está inteiro na §14.1 do
`DESEMPENHO.md`. O que interessa aqui é o que ele decide:

| durabilidade | trava presa GRAVANDO | trava presa LENDO (`varrer` 50) |
|---|---:|---:|
| `por_lote` (padrão) | **121–137 µs** | 3.122–3.187 µs |
| `por_operacao` | **1.404–1.492 µs** | 2.955–3.207 µs |

**O `fsync` sob a trava custa 1.267–1.371 µs por gravação — 10,3× a 12,3×** o
tempo que uma gravação segura a trava sem ele. Duas baterias limpas, com o
`quieta.Vigia` aprovando as duas.

**O controle é a linha do meio**, e ele é o que separa isto de palpite: a
leitura toma a **mesma** trava e não sincroniza nada. Entre as duas baterias ela
andou **1,01×** e **0,95×** — ficou parada, como tinha de ficar.

**E o que isso faz com a ordem de trabalho:** no padrão desta casa, **uma
leitura segura a trava 23× mais tempo que uma gravação**. A pista da §14 do
`DESEMPENHO.md` — *«a leitura custa 20× mais por operação, o que favorece o
`RwLock`, mas favorecer não é medir»* — deixa de ser inferência e vira número.

Então «atacar as 23 seções que alcançam `fsync`» **não é** a prioridade no
padrão: elas custam 137 µs de trava, contra 3.122 µs de um `varrer(50)`. Com
`durabilidade: por_operacao` a conta muda e as duas ficam da mesma ordem
(1.404 contra 2.955 µs) — e é aí que encurtar o caminho do `fsync` compra
alguma coisa. Isto conta **por operação**, e não por carga: quem decide o total
é a frequência de cada uma, e isso o medidor não sabe.

```bash
python3 bancada/concorrencia/quanto-a-trava-fica-presa.py
```

---

## 7.2 A SP000012 no mesmo território: o que existe, contado

A SP000012 é «deadlock, cancelamento e governança de recursos», e o roteiro a
dava como parcial com uma frase: *«há prazo e recusa de reentrância»*. Frase não
é número. O que existe, contado:

### Abraço mortal — três guardas, e as três com o defeito reposto

| guarda | o que impede | onde |
|---|---|---|
| `COM_A_TRAVA` | a mesma thread pedir a trava que já tem — **pendurava** antes, sem log e sem pilha | `travar_dados`, teste `a_trava_pedida_duas_vezes_pela_mesma_thread_vira_erro` |
| nenhuma leitura de rede sob a trava | o abraço mortal do bidirecional | `DESEMPENHO.md` §4.13, e o mapa confirma por outro caminho: **0 de 76** seções na classe `rede-ou-espera` |
| `transacao_lock_timeout_ms` (500 ms) | a espera por linha travada por outra transação virar thread pendurada | `config.rs` |

### Cancelamento — **4 de 76**, e este é o número que faltava

O `KILL` e o `STATEMENT TIMEOUT` desta casa só mordem onde alguém chamou
`Atividade::siga`, entre duas unidades de trabalho seguras. O mapa passou a
contá-los, e o número é **4 de 76**: a leitura da página, a conversão da carga,
a soma da tabela e a exportação.

**Nas outras 72, mandar parar não para.** Isso não é, por si, defeito — uma
seção de 3 linhas não precisa de ponto de cancelamento, e pôr um no meio de uma
gravação deixaria a tabela e o índice discordando (é o que o campo
`transacao_statement_ms` já documenta). O que era defeito é o caso em que a
seção pode durar **o que quem escreveu o gatilho quiser** — e esse é o §1.4, que
esta rodada fechou pelo outro lado: sem ponto de cancelamento, mas com teto.

**A consequência honesta, e ela fica escrita:** um `KILL` sobre uma conexão
parada num corpo de gatilho não a interrompe. Ele passa a ser atendido em **no
máximo 500 ms**, que é o prazo — e não em «quando o gatilho quiser», que era
antes. *Cancelamento que não cancela é pior que cancelamento ausente*; o que se
entrega aqui não é o cancelamento, é o **teto** que o torna dispensável nesse
caminho.

### Governança de recursos — os tetos que existem, e o que faltava

`conexoes_max` (64), `usuarios_max`, `memoria_max_mb`, `transacao_max_linhas`
(100.000), `transacao_prazo_min` (5), `carga_prazo_min` (30), `cpu_percentual`,
`threads`, `diario_volume_mib` — todos lidos, todos em `config.rs`.

O buraco era o que **nenhum deles cobria**: a memória que o código do dono do
banco aloca. Um corpo de gatilho podia alocar até o alocador falhar e **abortar
o processo**, e nenhum teto de conexão, de linha ou de transação chegava perto
disso. É o `TEXTO_MAX` do §1.4.

### O que a SP000012 NÃO fecha, e por quê

* **`Atividade::siga` não alcança o interpretador.** Levá-lo até lá exigiria o
  `phxsql-sql` conhecer a telemetria do servidor — uma dependência de camada que
  hoje não existe e que o `MotorNulo` foi escrito para não ter. Com o teto de
  500 ms, o ganho seria de 500 ms para «imediato», e o preço é a camada. Fica
  **nomeado e não feito**, que é diferente de esquecido.
* **Ordem canônica de travas entre tabelas**: existe escrita no `TRANSACOES.md`
  §11.3, e não foi reconferida nesta rodada.

---

## 8. O que continua por medir, e nomeado

* **O teto do MVCC**, que é o p99 do leitor com um escritor ao lado. O arnês
  mede; falta a máquina parada. **Não medido, e não estimado.**
* **O teto do `RwLock` na espera** (p99 de dois leitores contra um). Idem.
* **O perfil de carga real.** A §3.1 mostra que a resposta muda com a razão
  entre o tempo de leitura e o de escrita, e o número de hoje vem de um
  `varrer(50)` contra um `inserir(1)`.
* ~~**O custo do `fsync` sob a trava, em milissegundos.**~~ **Medido em 03/09**:
  1.267–1.371 µs por gravação, 10,3× a 12,3× o tempo de trava de uma gravação
  sem ele. §7.1 aqui, e a bateria inteira na §14.1 do `DESEMPENHO.md`.
* **O comboio do fecho de janela**, que a medição de 03/09 deixou nomeado e não
  medido. O `gravar_de_verdade` faz duas coisas quando a janela fecha: sincroniza
  a **própria** tabela e chama `descarregar_sujas_com`, que **reabre e sincroniza
  todas as outras sujas** — tudo com a trava global na mão. Com K tabelas sujas,
  o escritor azarado que fecha a janela segura o servidor por `K × (open +
  fsync)`, e os outros K−1 não pagaram nada.

  A §7.1 **não** mede isso: ela roda com **uma** tabela, e com uma tabela o
  conjunto de sujas fica vazio e o comboio nunca acontece. E a média não o
  mostraria mesmo com K tabelas — a conta dá +19,5 µs sobre 4.000 gravações,
  enquanto a seção individual segura 5,2 ms. *É a média que esconde o que se
  procura*, e o instrumento certo é o **p99 de um leitor com um escritor ao
  lado**, que é a forma que o `escolher-o-desenho.py` já usa para o MVCC.

  Fica escrito porque é a única hipótese de hoje que ainda aponta para as 23
  seções do `fsync` **no padrão** `por_lote` — as outras a §7.1 derrubou.

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
| 76 seções, as seis classes, 38/23/5/**4**/8/40, os tamanhos | `bancada/concorrencia/mapa-da-trava.py` |
| 4 de 76 com ponto de cancelamento | idem — marcador `cancelavel`, acrescentado em 03/09 |
| 27,2 ms / 1 µs / 905,9 ms / 28.590 → 500,2 ms | `cargo run --release --example custo-do-gatilho -p phxsql-sql` |
| 10,2 s e o aborto do processo | o mesmo, com `ulimit -v 2000000` — medição manual, registrada porque a automática derrubaria a máquina |
| 121–137 / 1.404–1.492 / 3.122–3.187 µs de trava | `bancada/concorrencia/quanto-a-trava-fica-presa.py`, duas baterias aprovadas pelo `quieta.Vigia` |
| 0 tomadas fora do ponto único | `grep -c 'self\.dados\.lock()' servidor.rs`, e a catraca `so_um_lugar_toma_a_trava` |
| 24 bytes do cabeçalho do slot | `SLOT_CAB` e o layout do módulo, em `crates/phxsql-store/src/reg.rs` |
| `Instancia` com um campo | `pub struct Instancia`, em `crates/phxsql-store/src/catalogo.rs` |
| 200 operações / 200 ms | `lote_operacoes` / `lote_milissegundos`, em `crates/phxsql-server/src/config.rs` |
| 1,99× / 1,51–1,59× | `docs/DESEMPENHO.md` §14 — **medição anterior, não refeita aqui** |
| 26–100% ocupada, 13 vizinhos, 120% no controle | `bancada/concorrencia/quieta.py`, nas rodadas de hoje |
