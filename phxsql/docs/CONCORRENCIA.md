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

> **APOSENTADA em 04/09 (§15).** Medido: a razão ler/gravar percorre **28×** — de 0,8×, onde a leitura é mais barata que a escrita, a 22,5× — e o teto do `RwLock` fica entre **1,79× e 2,15×** nas dez medições, sem tendência. **O perfil em que a escrita domina NÃO inverte a conclusão.** O texto abaixo fica como está porque a §11 e a §12 ainda o citam, e porque o motivo pelo qual ele não valia é o que se aprendeu: *o que serializa não é só o tempo sob a trava, é o pedido dela.*

> **Correção de 04/09 (§14):** onde este trecho diz «`varrer` de 50 linhas», a leitura medida era de **1.000** — a bancada mandava o campo `limite`, que o `op_varrer` não lê. As razões continuam valendo; o rótulo, não.

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

**(1) O cabeçalho do slot NÃO está cheio — e este parágrafo dizia que estava.**
São 24 bytes (`SLOT_CAB`, no `reg.rs`), mas os 24 não estão usados:

```text
[status u8][flags u8][res u16][crc32 u32][versao u64][tempero u64]
   1     +    1     +    2    +    4     +    8      +    8       = 24
            ^^^^^^^^^^^^^^^^^                        ^^^^^^^^^^^^
            3 bytes LIVRES SEMPRE                    só existe na tabela cifrada
```

**Medido varrendo o `reg.rs` por quem escreve e por quem lê cada faixa:**
`flags` (byte 1) e `res` (2..4) **não são tocados por ninguém** — nenhuma
escrita, nenhuma leitura. O `tempero` (16..24) só é gravado dentro do ramo da
cifra (`reg.rs:1585-1586`), então numa tabela **não cifrada** ele também está
livre. São **3 bytes livres sempre, 11 na v4**.

O número errado circulava em três lugares e foi repetido três vezes na rodada
de 04/09 antes de alguém ir ao `reg.rs` conferir. *Número citado é número que
não se mede* — e este mudava a **resposta**, não só a frase: com 3 bytes cabe
um índice de 24 bits para um diretório por tabela, e o ponteiro de undo do
InnoDB (7 bytes) mostra por que isso basta — ele é **endereço estruturado**, não
deslocamento cru. Logo **o `.reg` v6 pode não ser necessário**, e a escolha
volta a ser do DBA em vez de ser imposta pela aritmética.

A alternativa de crescer o slot continua na mesa. O precedente existe e é bom: a cifra de coluna já fez a v4
virar v5 crescendo o slot, com a versão no byte 8 decidindo quantos bytes ler
(o «Por que uma versao NOVA» do `reg.rs`). O mecanismo está provado — mas **é mudança de formato em
disco**, e a pétrea desta casa diz que mudança de formato entra **cedo**,
enquanto não há dado em produção. ~~Isso é um argumento *a favor* de decidir a
SP000016 agora, e não depois.~~

> **Envelheceu em 04/09, e quem a matou foi o dono.** As sete perguntas de
> formato foram postas uma a uma (`PESQUISA-MVCC-E-FORMATO.md` §8.0) e a
> resposta à primeira derrubou quatro: a versão velha mora **em RAM**, e a
> decisão de formato da SP000016 **desaparece** — nenhum `.reg` v6, nenhuma
> migração, nenhum arquivo novo. **Com isso o argumento da urgência cai
> inteiro**: o que era «decida agora ou pague migração depois» virou «decida
> quando precisar». O desenho e o custo estão em `docs/SOMBRA.md`.

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

> **E desde 05/09 ela deixou de ser uma frase e virou número.** O
> `docs/ACID.md` §4.1 e §4.3 medem os fenômenos **acontecendo**, cada um com o
> controle da mesma corrida: leitura não repetível (50 → 77), fantasma (2 → 3),
> perda de atualização e *write skew*. E a matriz mede o buraco sobre um
> invariante em vez de sobre uma linha: contra um escritor **em transação**, uma
> varredura única nunca vê o par quebrado (**0 de 400**) e duas leituras
> separadas o veem **73 de 400**. É esse 73 que a Sombra zera —
> `docs/SOMBRA.md` §1.

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
* **MVCC**: **duas** contas, e não uma — esta linha dizia uma só, e por isso
  creditava ao MVCC trabalho que é do `RwLock`.
  - o **p99** de um `varrer` com um escritor ao lado contra o p99 do mesmo
    `varrer` **sozinho** (`teto-do-mvcc-p99`): é a espera por inteiro. A
    redação anterior dizia que *«este é o único par que o `RwLock` não mexe»*,
    e **está errada**: parte desse custo é o de haver um segundo cliente
    qualquer, e é justamente o que o `RwLock` recupera.
  - o **p99** com um escritor ao lado contra o p99 com **dois leitores**
    (`teto-do-mvcc-exclusivo`): o que sobra depois desse desconto, e a única
    parte que só o MVCC compra. Medido em 04/09, §11.2: **0,91×–1,13×**,
    contra 1,19×–1,38× da primeira conta.

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
3. ~~**SP000016 (MVCC)**, com a decisão de formato tomada **cedo** — o `.reg` v6
   e a área de undo (§4.2). É o item mais caro dos três e o único que muda o
   disco; adiar não o torna mais barato, torna mais caro.~~ **Corrigido em
   05/09:** a decisão de formato foi tomada, e foi **não haver formato** — o
   dono respondeu em 04/09 que a versão velha mora em RAM, sem `.reg` v6 e sem
   arquivo novo (§4.2, e `PESQUISA-MVCC-E-FORMATO.md` §8.0). **Então a SP000016
   deixou de ser «o único que muda o disco», e com isso perdeu o argumento que a
   punha cedo**: adiar um item que não muda formato não o torna mais caro. O
   custo item a item, o que ela fecha dos quatro fenômenos medidos e as
   alternativas mais baratas estão em `docs/SOMBRA.md`; a ordem é decisão do
   dono.
4. **Trava por tabela**, se ainda sobrar disputa depois de (2) — e aí com o
   `escolher-o-desenho.py` a dizer quanto sobrou, em vez de por dedução.

**O motivo da troca, em uma frase:** o roteiro põe a SP000011 depois da
SP000016 porque «a SP000016 responde parte da escolha» — e a parte que ela
responde não é a parte que foi medida.

**A condição:** os itens 2 e 4 pedem **uma máquina parada** para fechar. O item
1 e a decisão de formato do item 3 não pedem, e é por isso que estão primeiro.

---

## 7.1 Quanto a trava fica PRESA, medido (03/09)

> **Correção de 04/09 (§14):** onde este trecho diz «`varrer` de 50 linhas», a leitura medida era de **1.000** — a bancada mandava o campo `limite`, que o `op_varrer` não lê. As razões continuam valendo; o rótulo, não.

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

## 7.1-bis REFEITA na carga de 50 (04/09) — a escrita fica, a leitura cai 12×

A §7.1 mediu com o `varrer` lendo **1.000 linhas** achando que lia 50 (§14).
Refeita com o instrumento consertado, duas baterias limpas (09:13 e 09:18):

| trava presa por operação | §7.1 (lendo 1.000) | A (lendo 50) | C (lendo 50) |
|---|---:|---:|---:|
| `por_lote`, **gravando** | 121–137 µs | 145,0 µs | 136,0 µs |
| `por_lote`, **lendo** | **3.122–3.187 µs** | **265,0 µs** | **240,0 µs** |
| `por_operacao`, gravando | 1.404–1.492 µs | 1.456,8 µs | 1.461,2 µs |
| `fsync` sob a trava | 1.267–1.371 µs | 1.311,8 µs | 1.325,2 µs |

**O achado central da §7.1 sobrevive inteiro**, e isso não é sorte: ele é um
número da **escrita**, e o defeito era da **leitura**. O `fsync` sob a trava
custa 1,3 ms e ~10× o tempo de trava de uma gravação sem ele — medido de novo,
com outro instrumento, e cai no mesmo lugar.

**O que estava errado por 12× era a leitura:** 265 e 240 µs contra os
3.122–3.187 µs publicados. É exatamente o que 50 linhas em vez de 1.000
prevê.

### 7.1-bis.1 E aqui fecha o mecanismo da §11.2-bis, pelos dois lados

Esta é a peça que faltava para a inversão do MVCC deixar de ser um número e
virar uma explicação:

| | trava presa lendo | `fsync` sob a trava | razão |
|---|---:|---:|---:|
| lendo 1.000 linhas | ~3.150 µs | ~1.320 µs | **0,42×** |
| lendo **50** linhas | ~250 µs | ~1.320 µs | **5,3×** |

Com a leitura segurando a trava por 3,1 ms, o `fsync` de 1,3 ms é **menos da
metade dela**: um escritor ao lado custa ao leitor quase o mesmo que outro
leitor, e o exclusivo do MVCC dá ~1,00×. Com a leitura em 250 µs, o `fsync`
passa a ser **cinco vezes** o tempo de trava de uma leitura — e o escritor ao
lado vira 2,77× o custo de outro leitor.

**Não é que o MVCC tenha ficado melhor: é que a leitura inflada estava
escondendo o `fsync` atrás de si.** Os dois medidores, o de espera (§11) e o de
trava presa (§7.1), erravam pelo mesmo campo e concordavam por isso — e agora,
consertados, concordam pelo motivo certo.

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

* ~~**O teto do MVCC**, que é o p99 do leitor com um escritor ao lado.~~
  **Medido em 04/09**, em duas baterias limpas: 1,30× e 1,19× no `por_lote`,
  1,38× e 1,24× no `por_operacao` — mas o que **só** ele compra (escritor ao
  lado contra outro LEITOR ao lado, que é o que o `RwLock` não recupera) deu
  **1,00× · 0,91× · 1,13× · 1,02×**, indistinguível do ruído. §11 aqui.
* ~~**O teto do `RwLock` na espera** (p99 de dois leitores contra um).~~
  **Medido em 04/09**: 1,30 · 1,31 · 1,22 · 1,22 — quatro medições dentro de
  7%. E o teto de **vazão** dele, que é o número grande: **2,48×–2,99×**. §11.
* ~~**A trava por tabela.**~~ **Respondida em 04/09**: ≈1,00× nas quatro
  medições (0,96 · 0,99 · 1,04 · 0,90). Não é a tabela que serializa. §11.
* ~~**O perfil de carga real.**~~ **MEDIDO em 04/09, §15**, na segunda
  tentativa — a primeira mandava um campo que o servidor não lê e está
  retratada na §13, com o alcance na §14. Com o campo certo, a razão
  ler/gravar percorre **28×** (de 0,8×, onde a leitura é mais barata que a
  escrita, a 22,5×) e o teto do `RwLock` fica entre **1,79× e 2,15×** nas dez
  medições, **sem tendência**. A ressalva da §3.1 **cai**: o perfil em que a
  escrita domina **não inverte a conclusão**.
* ~~**O custo do `fsync` sob a trava, em milissegundos.**~~ **Medido em 03/09**:
  1.267–1.371 µs por gravação, 10,3× a 12,3× o tempo de trava de uma gravação
  sem ele. §7.1 aqui, e a bateria inteira na §14.1 do `DESEMPENHO.md`.
* ~~**O comboio do fecho de janela**, que a medição de 03/09 deixou nomeado e não
  medido.~~ **MEDIDO em 04/09, e é real:** o p99 cresce **1,70× e 1,84×** para
  o escritor e **1,61× e 1,72×** para o leitor quando K vai de 1 a 4 — em duas
  baterias limpas, monotonicamente nas quatro séries. O leitor está numa tabela
  que ninguém escreve. A média dá 1,08×–1,17× e esconderia tudo. §12 aqui.
  **Nem `RwLock` nem MVCC consertam isto**, e o texto abaixo continua descrevendo
  o mecanismo. O `gravar_de_verdade` faz duas coisas quando a janela fecha: sincroniza
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

* ~~**Refazer a §7.1 com o instrumento consertado.**~~ **FEITA em 04/09
  (§7.1-bis)**, e ela fecha o mecanismo da §11.2-bis pelos dois lados: o
  achado da escrita sobrevive inteiro (o `fsync` custa 1,3 ms e ~10×), e a
  leitura cai **12×** — de ~3.150 µs para ~250 µs. É essa queda que faz o
  `fsync` passar de 0,42× para 5,3× o tempo de trava de uma leitura, e é por
  isso que o exclusivo do MVCC sai de ~1,00× para 2,77×. ~~E a §11.~~ **A §11 foi
  refeita em 04/09 (§11.2-bis)**, e o resultado foi o maior desta rodada: o
  exclusivo do MVCC, que parecia ruído com 1.000 linhas, dá **2,77× e 3,23×**
  na carga de 50 com `por_operacao`. Refazer não é formalidade — mudou o
  número duas vezes, e uma delas inverteu uma recomendação.
  O que sobra da linha antiga: Até 04/09 as cinco
  bancadas liam 1.000 linhas dizendo 50 (§14). As razões publicadas valem para
  a carga de 1.000; a série para a carga de 50 **não existe** nessas duas. Uma
  corrida curta de fumaça sugere que a trava presa lendo cai de ~3.150 µs para
  a ordem de 500 µs — **não medido em bateria limpa, e não estimado.**
  ~~E a §12.~~ **A §12 foi refeita em 04/09 (§12.4-bis)**, e o comboio
  **cresceu** na carga certa: 2,25× e 2,13× no escritor, contra 1,70× e 1,84×.
  Refazer as outras duas não é formalidade — pode mudar o número para cima,
  como mudou aqui.
* ~~**Uma catraca para o mapa.**~~ **FEITA em 04/09**, e a razão pela qual ela
  não tinha entrado estava errada: *«exigiria mexer em `crates/`»* — não exige.
  A régua é o próprio `mapa-da-trava.py`, que lê o fonte, e quem a roda é a
  bateria de ponta a ponta, que já é Python. Hoje
  `mapa-da-trava.py --catraca` guarda três tetos, e é o **item 0** da
  `prova-bateria.py`, antes de qualquer servidor subir:

  | catraca | teto | o que ela impede |
  |---|---:|---|
  | `codigo-do-dono` | **5** | código do dono do banco (gatilho `BEFORE`) sob a trava global |
  | `alcancam-fsync` | **22** | o que um `RwLock` **não** conserta — 1,3 ms de trava presa cada (§7.1-bis) |
  | `rede-ou-espera` | **0** | uma só já ata o servidor ao tempo de outra máquina |

  **Ela desce e nunca sobe**, e reprova nos dois sentidos: medir mais que o teto
  é alguém acrescentando o que a lei proíbe; medir menos também reprova, porque
  quem melhorou tem de baixar o teto no **mesmo commit** — catraca frouxa não
  segura nada. Provada nos dois sentidos com o teto deslocado. E se a régua um
  dia passar a medir mais coisas, a catraca **aposenta e renasce** no número
  medido do dia, como o `TETO_TABELA_NA_MAO`; ela não sobe.

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
| 24 bytes do cabeçalho do slot, **3 deles livres sempre e 11 na tabela não cifrada** | `SLOT_CAB` e o layout do módulo, em `crates/phxsql-store/src/reg.rs`; medido por quem escreve e quem lê cada faixa, e não pela soma do layout |
| `Instancia` com um campo | `pub struct Instancia`, em `crates/phxsql-store/src/catalogo.rs` |
| 200 operações / 200 ms | `lote_operacoes` / `lote_milissegundos`, em `crates/phxsql-server/src/config.rs` |
| 1,99× / 1,51–1,59× | `docs/DESEMPENHO.md` §14 — **medição anterior, não refeita aqui** |
| 26–100% ocupada, 13 vizinhos, 120% no controle | `bancada/concorrencia/quieta.py`, nas rodadas de hoje |

---

## 10. O ruído do controle, medido (04/09) — o teto do próprio vigia

Esta rodada não mede a trava: mede a **premissa da bancada que mede a trava**.
O `docs/CONCORRENCIA.md` (§0, §6.2) já citava «o controle variou 120%» como o
motivo de o `quieta.Vigia` existir, e o `quieta.py` já trazia
`tolerancia_controle=0.15` com a justificativa em prosa — "e a dispersao que
o ping mostra numa maquina parada" — mas sem a corrida que sustenta o número.
*Número citado é número que não se mede.* Esta seção mede.

### 10.1 O método

`bancada/concorrencia/ruido-do-controle.py`, novo nesta rodada, reaproveita
por **importação direta** o `rodada("sem-trava", 1, …)` do
`a-trava-serializa.py` — o mesmo controle que o Vigia usa em toda bateria
desta pasta, não uma cópia dele. Roda **30 corridas** de `ping` puro, 1
cliente, 1 s cada, de nariz a nariz, e para cada uma registra a vazão junto
com a ocupação e os vizinhos rodáveis do momento (`quieta.Amostra`), o que
separa as corridas **quietas** (vizinhos ≤ 1: só este arnês) das **sujas**
(outra frente do lado) em vez de misturar as duas num único número.

```bash
python3 bancada/concorrencia/ruido-do-controle.py
```

### 10.2 O número, medido hoje — e a máquina não parou

Esta árvore tinha **três outras frentes ativas** durante a medição — o mesmo
tipo de dia que gerou o «120%» herdado. Das 30 corridas, **20 caíram sujas**
(vizinhos > 1) e só **10 ficaram no critério de quietas**:

| grupo | n | média (op/s) | desvio | CV | mín–máx | salto |
|---|---:|---:|---:|---:|---:|---:|
| todas as 30 | 30 | 10.029 | 2.715 | 27,1% | 4.129–14.097 | 99,4% |
| **quietas** (vizinhos ≤ 1) | 10 | 8.631 | 1.348 | 15,6% | 7.057–11.329 | 49,5% |
| sujas (vizinhos > 1) | 20 | 10.728 | 2.946 | 27,5% | 4.129–14.097 | 92,9% |

**O achado que decide:** mesmo no subconjunto QUIETO por `vizinhos`, o salto
entre corridas chegou a 49,5% — mais que o triplo do teto em vigor (15%).
`vizinhos ≤ 1` não é o mesmo que "máquina parada" nesta árvore hoje; é
necessário, e não é suficiente (mais em §10.4).

### 10.3 A decisão sobre o teto — e por que ele NÃO desce

O protocolo que o script aplica: a proposta de novo teto é
`max(salto_quieto × 1,5; CV_quieto × 3)` — margens de segurança sobre o número
medido, não o teto em si, do mesmo jeito que o `quieta.py` já rotula as suas
próprias margens como escolhidas e não medidas. Hoje isso deu **74,2%**.

**74,2% não é menor que os 15% em vigor — então a catraca fica em 15%.** A
cláusula pétrea só deixa a catraca descer, nunca subir, mesmo quando a régua
que a mede muda; e aqui a régua **não achou base para apertar**, porque a
própria base (a máquina quieta) não apareceu por tempo suficiente hoje. Isso
não invalida os 15%: ao contrário, confirma que eles já eram **conservadores**
em relação ao ruído natural desta caixa — um teto solto teria deixado passar
como "limpa" uma bateria com bem mais do que 15% de deriva. A hipótese
("15% é frouxo demais, dá para apertar") foi medida e **morreu**; é resultado
tão válido quanto um aperto teria sido, e o `quieta.py` foi atualizado com a
data e o número (comentário do `Vigia`, `tolerancia_controle`).

Uma checagem menor, minutos depois (3 corridas de 0,5 s, feita para validar o
`--json` do script), pegou a máquina de fato parada e mostra por que 15%
segue sendo o número certo e não só o que restou: `vizinhos=0` nas três,
ocupação estável em 25-28%, CV **1,1%**, salto **2,6%** -- bem dentro do
teto. O ruído "quieto" varia de minuto para minuto nesta máquina
compartilhada; apertar o teto com a amostra de 30 corridas, que não pegou uma
janela igualmente calma por tempo suficiente, teria ajustado à sujeira do
dia -- o mesmo erro que este documento existe para evitar.

O «120%» herdado também sai daqui explicado, e não só repetido: é a mesma
ordem de grandeza do subconjunto **sujo** de hoje (salto 92,9%, CV 27,5%) —
o número de um dia contaminado, não uma dispersão de base a projetar um teto
em cima. Citar o 92,9%-hoje ou o 120%-de-02/09 como "o ruído do ping" sem
dizer qual condição gerou qual é exatamente o erro que este documento evita.

### 10.4 O achado extra: `vizinhos` tem ponto cego, medido e não só plausível

Nas 10 corridas quietas por `vizinhos`, a ocupação ainda variou de 50% a 83%.
A explicação óbvia — `steal` do hospedeiro, invisível ao `procs_running` deste
contêiner — foi **testada e não confirmada**: medido direto do `/proc/stat` no
momento em que isto foi escrito, `steal` estava em 0,0%. A hipótese morre
como causa geral, e fica registrada morta em vez de silenciosa. A explicação
que os números sustentam é mais prosaica: o `Amostra` lê `procs_running` em
só 3–5 instantes por janela, e um pico de tarefa rodável entre dois desses
instantes passa batido, enquanto a ocupação (`idle` acumulado pelo kernel ao
longo da janela inteira) não perde nenhum. **Não medido** qual dos dois pesa
mais neste dia — só que o sinal de `vizinhos` sozinho tem esse ponto cego,
isso sim medido. É por isso que o vigia nunca decide por um sinal só: cada um
dos três cobre um ponto cego diferente, e foi o **controle** que pegou este.
Detalhe em `bancada/concorrencia/LEIA-ME.md`.

### 10.5 A prova real, nos dois sentidos — com a saída de verdade

**Lado que recusa**, quatro vezes hoje com esta árvore do jeito que estava
(nenhuma carga acrescentada por mim), cada uma com o motivo impresso:

| tentativa | ocupação antes→depois | vizinhos (pico) | controle antes→depois | motivo(s) citado(s) |
|---|---|---:|---|---|
| 1 | 56%→98% | 7 | 9.403→13.640 (45%) | ocupação mudou 42 pontos; 7 vizinhos; controle mudou 45% |
| 2 | 95%→28% | 4 | — | ocupação mudou 68 pontos; 4 vizinhos |
| 3 | 50%→96% | 9 | — | ocupação mudou 45 pontos; 9 vizinhos |
| 4 | 46%→26% | 7 | 7.376→13.105 (78%) | ocupação mudou 20 pontos; 7 vizinhos; controle mudou 78% |

**Lado que recusa, com carga artificial CONTROLADA** — 4 processos Python
próprios, um por núcleo, em laço fechado de CPU por 90 s, subidos e depois
**derrubados pelo PID** (nunca `pkill`, para não afetar as outras frentes):

```
antes:  100% ocupada, ate 4 vizinho(s)
durante: 99% a 100% ocupada, mediana 100%, ate 11 vizinho(s)
depois: 100% ocupada, ate 5 vizinho(s)
controle: 10753 -> 2936 op/s
VEREDITO: NAO.
  - ate 11 tarefas rodaveis alem do medidor: ha outra frente trabalhando nesta maquina
  - a curva de CONTROLE mudou 73% entre o comeco e o fim (10753 -> 2936 op/s)
```

**Lado que aceita**, na 5ª tentativa (mesmo padrão da rodada de 03/09, que
precisou de seis para duas baterias limpas — não é regressão, é o custo já
conhecido de medir numa máquina compartilhada):

```
antes:  28% ocupada, ate 2 vizinho(s)
durante: 50% a 100% ocupada, mediana 76%, ate 2 vizinho(s)
depois: 26% ocupada, ate 1 vizinho(s)
controle: 7821 -> 8223 op/s
VEREDITO: quieta o bastante. Os numeros valem.
```

E os números que ela liberou incluem, pela primeira vez nesta árvore com
`LINHAS=300`, o modo **`gravar`** (escritores, não só leitores) — a mesma
carga cuja ausência derrubou a premissa da SP000011 registrada na §5(1):

| modo | 1 cliente | 2 clientes | 4 clientes |
|---|---:|---:|---:|
| `sem-trava` (controle) | 7.821 op/s | 14.529 (1,86×) | 28.774 (3,68×) |
| `ler` | 446 op/s | 703 (1,58×) | 719 (1,61×) |
| **`gravar`** | 900 op/s | 458 (**0,51×**) | 706 (0,79×) |
| `ler-tabelas-separadas` | 447 op/s | 726 (1,62×) | 654 (1,46×) |
| `gravar-tabelas-separadas` | 985 op/s | 402 (0,41×) | 174 (0,18×) |

Esta bateria é curta (`SEGUNDOS=1`, para caber numa janela quieta pequena) e
**não substitui** a medição formal da SP000011/SP000016 — mas o `gravar` caiu
para menos da metade da vazão de 1 cliente com 2 clientes concorrentes, o
oposto do `sem-trava` ao lado. É o sinal de que o protocolo, quando aceita,
está medindo o par certo (leitor **e** escritor), não só leitor com leitor.

### 10.6 A resposta que esta frente devolve: a SP000011/SP000016 pode ser medida aqui?

**Sim.** O arnês (`a-trava-serializa.py`, `escolher-o-desenho.py`,
`quanto-a-trava-fica-presa.py`) mais o `quieta.Vigia` já formam o protocolo
que a §10 pedia: recusa publicar acima de um teto medido, e o teto (15% no
controle, confirmado e não afrouxado nesta rodada) resistiu a uma tentativa
honesta de apertá-lo. A condição não é «máquina livre de vizinho», que é rara
nesta árvore compartilhada — é **rodar mais de uma vez**: hoje foram 5
tentativas para 1 bateria limpa; em 03/09 foram 6 para 2. O protocolo já paga
esse custo sozinho, recusando em silêncio numérico (nenhum número sai) em vez
de publicar sujo. Quem abrir a próxima frente pode rodar `escolher-o-desenho.py`
e `quanto-a-trava-fica-presa.py` do mesmo jeito — com `--json` para automação,
e com paciência para repetir até o `quieta.Vigia` aceitar.

---

## 11. O teto do MVCC e o do `RwLock`, MEDIDOS (04/09) — e o do MVCC morre no ruído

> **Correção de 04/09 (§14):** onde este trecho diz «`varrer` de 50 linhas», a leitura medida era de **1.000** — a bancada mandava o campo `limite`, que o `op_varrer` não lê. As razões continuam valendo; o rótulo, não.

A §8 listava os dois primeiros itens como **«não medido, e não estimado — o
arnês mede; falta a máquina parada»**. A máquina parou às 06:03 de 04/09, e
duas baterias limpas saíram — o `quieta.Vigia` aprovando as duas, como a §7.1
exigiu para o `fsync`. As corridas cruas estão em
`bancada/concorrencia/corridas/`.

O custo do protocolo, medido e não citado: **quatro tentativas para duas
baterias limpas.** As duas reprovadas não foram azar da árvore compartilhada —
**a outra frente era eu**, rodando o aviso de comunicação e um `git push`
dentro da janela de medição. O vigia pegou as duas (`até 7 vizinhos`, depois
`até 3`), e é exatamente para isso que ele existe.

### 11.1 Os tetos, nas duas corridas

| teto | A, `por_lote` | B, `por_lote` | A, `por_operacao` | B, `por_operacao` |
|---|---:|---:|---:|---:|
| trava por tabela | 0,96× | 0,99× | 1,04× | 0,90× |
| `RwLock`, vazão de leitura | **2,48×** | **2,65×** | **2,69×** | **2,99×** |
| `RwLock`, na espera (p99) | 1,30× | 1,31× | 1,22× | 1,22× |
| MVCC, na espera (p99) | 1,30× | 1,19× | 1,38× | 1,24× |

**A trava por tabela está respondida: ≈1,00× nas quatro medições.** Ler em
tabelas separadas não é mais rápido que ler na mesma tabela, e as duas corridas
concordam. Não é a tabela que serializa — é a trava global, e trocá-la por uma
trava por tabela move o gargalo de lugar sem tirá-lo do caminho. O próprio
documento já dizia que *um desenho cujo teto é 1,05× está respondido antes de
começar*; este é.

**O `RwLock` é o número grande, e é o estável.** Entre 2,48× e 2,99× nas quatro
medições, contra um controle que escala 4,0×–5,0× com quatro clientes enquanto
a leitura escala 1,5×–1,6×. É paralelismo de leitura que a exclusividade está
comendo, e ele se recupera sem mexer em byte nenhum do formato em disco.

### 11.2 A conta que decide, e que o relatório não imprime

A linha «MVCC, na espera» compara o leitor **com um escritor ao lado** contra o
leitor **sozinho**. Só que uma parte desse custo é de ter *qualquer* segundo
cliente na máquina, e o `RwLock` já a recupera. **O que só o MVCC compra é a
diferença entre um ESCRITOR ao lado e outro LEITOR ao lado** — e essa conta é
a última coluna:

| corrida | sozinho | 2 leitores | c/ escritor | MVCC/sozinho | **só o MVCC** |
|---|---:|---:|---:|---:|---:|
| A, `por_lote` | 6.583 µs | 8.546 | 8.586 | 1,30× | **1,00×** |
| B, `por_lote` | 7.317 µs | 9.580 | 8.704 | 1,19× | **0,91×** |
| A, `por_operacao` | 6.906 µs | 8.407 | 9.525 | 1,38× | **1,13×** |
| B, `por_operacao` | 7.071 µs | 8.638 | 8.780 | 1,24× | **1,02×** |

**1,00× · 0,91× · 1,13× · 1,02×.** Duas baterias limpas, e o número que
justifica o MVCC oscila em torno de **1,00×** — com uma corrida em que o
escritor ao lado saiu **mais barato** que outro leitor ao lado. O ganho
exclusivo do MVCC **não se distingue do ruído desta bancada**.

Compare-se com o que a mesma bancada mede sem hesitar: o `RwLock` na espera deu
1,30 · 1,31 · 1,22 · 1,22 — quatro medições dentro de 7% umas das outras. O
instrumento **não** é cego a diferenças reais; ele é cego a esta.

### 11.2-bis REFEITO na carga de 50 (04/09) — e o MVCC RESSUSCITA no `por_operacao`

As baterias da §11.2 liam **1.000 linhas** dizendo 50 (§14). Refeito com
`LINHAS_LIDAS=50`, duas baterias limpas (08:38 e 08:52) — e **quatro
reprovadas** pelo vigia no meio, que é o preço que a §10.6 já dizia que o
protocolo cobra. Corridas em `corridas/desenho-CERTO-50-*`.

| teto | A · `por_lote` | B · `por_lote` | A · `por_operacao` | B · `por_operacao` |
|---|---:|---:|---:|---:|
| trava por tabela | 1,02× | 0,98× | 0,99× | 1,09× |
| `RwLock`, vazão | 2,15× | 1,54× | 2,73× | 2,56× |
| `RwLock`, na espera | 1,06× | 1,31× | 1,14× | 1,31× |
| **MVCC, EXCLUSIVO** | **1,21×** | **1,00×** | **3,23×** | **2,77×** |

**O achado, e ele desmente a §11.2:** o que **só** o MVCC compra dava ~1,00× na
carga de 1.000 linhas e eu escrevi que morria no ruído. Na carga de **50** com
durabilidade `por_operacao`, dá **2,77× e 3,23×**.

O mecanismo é limpo, e sai das próprias medianas de B:

```
por_operacao, leitor com pagina de 50 linhas:
  sozinho ................ p99    738 us
  com outro LEITOR ....... p99    911 us   (+23%: e o que o RwLock recupera)
  com um ESCRITOR ........ p99  2.527 us   (3,4x o sozinho)
```

Um escritor ao lado custa ao leitor **2,77× o que outro leitor custa**, e essa
diferença é o `fsync` sob a trava global — 1.267–1.371 µs medidos na §7.1. Com
a leitura de 1.000 linhas, ela custava 6.500 µs e **afogava** esse `fsync`:
o gap sumia na razão. Com a leitura em 738 µs, o `fsync` passa a dominar.

**O instrumento errado escondia exatamente o que só o MVCC compra** — e
escondia no sentido mais traiçoeiro, fazendo-o parecer ruído em vez de fazê-lo
parecer grande.

**No `por_lote`, que é o padrão, o exclusivo continua ~1,00×–1,21×.** A janela
de gravação faz o `fsync` acontecer uma vez a cada 200 gravações, e aí o
escritor volta a custar o que outro leitor custa. **Os dois números valem, e a
diferença entre eles é a durabilidade que o dono do banco escolher.**

### 11.3 O que isto decide, e o que NÃO decide

**Decide o desempenho — e a §11.2-bis mudou esta resposta pela metade.** Na
carga de 1.000 linhas, e no padrão `por_lote` em qualquer carga, a Sombra não
se justifica por p99 de espera: não há 1,3× para ela recuperar, há ~1,0×, e
quem paga esse número é o `RwLock`.

**Mas em `por_operacao` com a carga da tela (50 linhas) o exclusivo dela é
2,77× e 3,23×**, medido em duas baterias limpas, e é o `fsync` sob a trava que
o `RwLock` não toca. Então a frase honesta não é «a Sombra não compra
desempenho»; é **«a Sombra compra desempenho onde o `fsync` acontece em toda
gravação, e não compra onde ele acontece uma vez por janela»**. Qual dos dois
mundos é o do dono do banco é a configuração `recursos.durabilidade` — e ela é
escolha dele, não nossa.

**E aqui esta seção quase mentiu.** A primeira redação dela dizia que o
`RwLock` «custa muito menos — trocá-lo é uma decisão de uma linha». **É
exatamente a premissa que o pedido 164 já tinha matado**, e a §2 deste mesmo
documento a desmente em letras maiúsculas: `RwLock<Instancia>` **compila de
primeira e está errado**, porque nenhum método pede `&mut` e o tipo não tem o
que proteger — dois escritores com guarda de leitura abririam dois `Table`
sobre os mesmos arquivos, sem um erro do compilador. O marcador `!Sync` da
§1.3 existe **para transformar esse erro silencioso em erro de compilação**, e
ler «o `RwLock` não compila hoje» como «falta uma linha» é ler a guarda ao
contrário: ela não está no caminho, ela é o aviso.

O que o `RwLock` custa, então, é o **invariante**: separar de verdade os
caminhos que só leem dos que escrevem, que hoje são o mesmo `&self`. É trabalho
menor que um gestor de versões, e não é uma linha. *Documento que se contradiz
na mesma página é pior que documento que falta, porque quem lê a página certa
sai convencido.*

**NÃO decide a correção, e é aqui que a Sombra continua de pé.** A §4.3 já
tinha nomeado a única coisa que só o MVCC dá: **leitura repetível**. Uma
varredura longa hoje enxerga linhas gravadas no meio dela, e nenhum `RwLock`
conserta isso — ele torna os leitores simultâneos, não consistentes. Esse é um
defeito de *resultado*, não de *tempo*, e nenhuma medição de p99 o mostraria.

**A premissa que morreu é a de velocidade, e ela era a que estava escrita.** Foi
a §5 deste documento que disse *«MVCC não é substituto da SP000011»*; agora há
o número, e ele diz mais: **MVCC não é acelerador de coisa nenhuma nesta
bancada.** Se a Sombra for feita, é por leitura repetível — e o documento dela
tem de dizer isso na primeira linha, senão daqui a três meses alguém a defende
com um ganho de desempenho que ninguém mediu.

> **Escrito em 05/09: `docs/SOMBRA.md`**, e a primeira linha é esta. O
> documento acrescenta três coisas que esta seção não tinha: **quais** dos
> quatro fenômenos medidos no `docs/ACID.md` ela fecha (leitura não repetível e
> fantasma) e quais não (perda de atualização, já fechada por outros dois
> caminhos; e *write skew*, que ela torna **sistemático** em vez de ocasional);
> o custo item a item, com o que ficou nomeado e não medido; e as alternativas
> mais baratas. Duas ressalvas de lá valem para esta seção: o teto exclusivo de
> **3,23× e 2,77×** foi medido com a trava ainda em `Mutex`, e a §16 baixou o
> denominador da conta — **remedir antes de citar**; e a cadeia de versões, por
> construção, **não fecha o fantasma**: linha que nasceu depois da visão não tem
> versão velha, e o filtro que a esconde é outra peça.

*Hipótese que morre medida é resultado tão válido quanto ganho, e é o que
impede a mesma ideia de voltar sem medição.*

---

## 12. O comboio do fecho de janela, MEDIDO (04/09) — e ele é real

> **Correção de 04/09 (§14):** onde este trecho diz «`varrer` de 50 linhas», a leitura medida era de **1.000** — a bancada mandava o campo `limite`, que o `op_varrer` não lê. As razões continuam valendo; o rótulo, não.

A §8 deixou isto **nomeado e não medido**, e era a última hipótese em pé
apontando para as 23 seções de `fsync` no padrão `por_lote`. Está medido, em
duas baterias limpas (07:22 e 07:31), com o `quieta.Vigia` aprovando as duas.
Corridas cruas em `bancada/concorrencia/corridas/comboio-*`.

### 12.1 A premissa, conferida no fonte ANTES de virar medição

`servidor.rs`, `gravar_de_verdade` — que já roda **com a trava global na mão**:

```rust
// A janela fechou: esta vai agora, e as outras da janela junto.
t.sincronizar()?;
if let Ok(mut s) = self.sujas.lock() { s.remove(&chave); }
self.descarregar_sujas_com(dados);
```

E `descarregar_sujas_com` drena o conjunto inteiro e faz, **por tabela suja**,
`abrir_database → abrir_qualificada → sincronizar` — um `open` mais um `fsync`
cada, em laço, sem soltar a trava. O comentário dela já dizia o custo («um
`open` por tabela, uma vez por janela»); o que faltava era **quanto isso
aparece para quem está esperando**.

*Alvo certo com causa errada já custou uma rodada aqui* — o pedido 113 — e por
isso a premissa se lê no fonte antes de o medidor existir.

### 12.2 O que o medidor segura parado, e por que isso decide tudo

Se eu variasse o número de escritores, mediria **contenção** e chamaria de
comboio. Então os escritores ficam **fixos em 4** e varia só **em quantas
tabelas distintas eles escrevem**: K=1 (todos em `w0`), K=2 (dois e dois), K=4
(um em cada). Mesma carga, mesmos clientes, mesmo número de `fsync` no total.

E o leitor lê sempre `quieta`, **uma tabela que ninguém escreve** — para que a
espera dele seja espera de **trava**, e não disputa pela própria tabela.

### 12.3 O número

| série | K=1 | K=2 | K=4 | K=4 ÷ K=1 |
|---|---:|---:|---:|---:|
| A · escritor, p99 | 5.011 µs | 6.153 | 8.494 | **1,70×** |
| B · escritor, p99 | 4.645 µs | 6.125 | 8.542 | **1,84×** |
| A · leitor, p99 | 6.985 µs | 8.036 | 11.243 | **1,61×** |
| B · leitor, p99 | 6.474 µs | 8.584 | 11.157 | **1,72×** |
| A · escritor, média | 1.446 µs | 1.491 | 1.561 | 1,08× |
| B · escritor, média | 1.306 µs | 1.519 | 1.533 | 1,17× |

**O p99 cresce com K nas quatro séries, monotonicamente, nas duas corridas.** E
o leitor — que lê uma tabela que ninguém toca — passa de 6,5 ms para 11,2 ms
só porque *outras* tabelas ficaram sujas. É o servidor inteiro parando, que é
exatamente o que a trava global faz.

**A média esconde, como a §8 previu:** 1,08× e 1,17×. Quem olhasse a média
concluiria que não há nada aqui.

### 12.4 O número que eu destaquei primeiro, e que a segunda corrida derrubou

Na primeira corrida o **pior caso** saltou de 10,8 ms para 38,2 ms (3,53×) no
escritor e de 9,7 para 36,2 ms (3,75×) no leitor, e foi **esse** o número que
eu relatei primeiro. A segunda corrida deu 1,11× e 1,89×, e **não é monotônico
em K em três das quatro séries**.

O pior caso é **uma amostra**: ele diz que o comboio pode custar dezenas de
milissegundos — o que é informação —, mas não sustenta um fator. *O p99 é o
achado; o pior é a anedota que o p99 explica.* Fica escrito porque eu o
publiquei antes de ter a segunda corrida, e apagar deixaria a lição perdida.

### 12.4-bis REFEITO na carga certa (04/09, 08:17 e 08:24) — e o comboio CRESCE

As duas baterias acima liam **1.000 linhas** dizendo 50: é o defeito do campo,
na §14. Com o instrumento consertado, duas baterias limpas na carga de **50
linhas de verdade**:

| p99, K=4 ÷ K=1 | A (08:17) | B (08:24) | as de 1.000 linhas |
|---|---:|---:|---:|
| escritor | **2,25×** | **2,13×** | 1,70× e 1,84× |
| leitor em `quieta` | **2,01×** | **1,96×** | 1,61× e 1,72× |
| média (escritor) | 1,16× | 1,12× | 1,08× e 1,17× |

Monotônico nas quatro séries das duas corridas, como antes. E o número **subiu**
— o que era de se esperar assim que se pensa nele: o comboio é um custo **fixo
por fecho de janela** (K × (`open` + `fsync`)), e quanto mais barata a leitura,
maior a fatia que ele ocupa da espera. Com 50 linhas o leitor sai de 4,0 ms
para 7,9 ms; com 1.000 ele saía de 6,5 para 11,2.

**A carga de 50 linhas é a que a tela usa** — uma página de grade —, então é
esta a leitura que mais importa das duas. *O conserto do instrumento não
derrubou o achado: engrandeceu-o.*

### 12.5 O que isto decide

**Aponta para o código de HOJE, e não para um desenho futuro.** Nem `RwLock`
nem MVCC consertam o comboio: ele não é leitor-contra-escritor, é um escritor
segurando a trava global por trabalho que **não é dele** — as tabelas dos
outros. Um `RwLock` deixaria os leitores simultâneos e eles continuariam todos
parados atrás do mesmo comboio.

E o teto é o K de uma base real. Aqui K vale 4 e custa 1,7×; numa base com
dezenas de tabelas ativas, K é dezenas.

**O que NÃO se mediu aqui**, e não se estima: se soltar a trava entre uma
tabela e a seguinte é seguro. O `descarregar_sujas_com` apaga as marcas
pendentes **depois** de todas sincronizarem, e o comentário diz que *«esta é a
ordem que faz o group commit ser seguro, e ela não se inverte»*. Quebrar o laço
em pedaços mexe nessa ordem, e é decisão de formato e durabilidade — do papel
**C**, não desta medição.

> **Respondido em 05/09, na §12.6:** é **inseguro** como estava proposto, e a
> matriz de queda mostra o instante. E a saída que se tomou não estava na
> lista — as K tabelas passaram a ir ao disco **juntas**, sem soltar trava
> nenhuma: 2,52× em K=16, com o mesmo número de `fsync`.

### 12.6 A matriz de queda, e a decisão do papel C (05/09)

A §12.5 deixou uma pergunta e um dono: *soltar a trava entre uma tabela e a
seguinte é seguro?* — decisão de durabilidade, não de medição. A resposta está
abaixo, e ela é **não**, com a matriz que a sustenta. E a saída que se tomou no
lugar não estava na lista: **o comboio encolhe sem soltar a trava nenhuma.**

#### 12.6.1 O que o laço protege, lido no fonte antes de qualquer proposta

O `op_commit` faz quatro coisas, todas com a **mesma** tomada da trava global:

1. grava a marca `.tx` e a **sincroniza**, antes de tocar em arquivo de dado;
2. aplica a passada, tabela por tabela — e cada tabela chama
   `gravar_de_verdade`, que sincroniza **se** a janela fechou e apenas marca a
   tabela como suja se não;
3. pergunta `tabelas_ainda_sujas`: se alguma tabela desta transação continua
   devendo ao disco, a marca vai para `marcas_pendentes`; senão, sai agora;
4. e o fecho de janela, quando acontece, roda `descarregar_sujas_com`: drena o
   conjunto de sujas, sincroniza cada uma, e **só então** apaga *todas* as
   marcas pendentes.

O invariante que isso entrega, escrito com precisão:

> **Uma marca `.tx` só pode sair do disco quando todas as tabelas que ela nomeia
> tiverem tido um `fsync` POSTERIOR à última escrita daquela transação.**

O código não implementa esse invariante: implementa uma regra mais barata —
*apaga todas as marcas pendentes quando todas as tabelas sujas sincronizaram*.
As duas só são a mesma coisa por **duas** razões, e a segunda é a que ninguém
tinha escrito:

* **a união fecha** — no instante em que uma marca entra em `marcas_pendentes`,
  cada tabela dela ou já sincronizou nesta passada, ou está no conjunto de
  sujas. É o que o passo 3 confere;
* **e ninguém escreve no meio** — entre o passo 3 de uma transação e a drenagem
  das marcas do passo 4, nenhuma outra thread grava, porque tudo isso acontece
  sob a mesma tomada da trava global.

**É a segunda que o comentário do laço não diz.** Ele diz «esta é a ordem que
faz o group commit ser seguro, e ela não se inverte» — e a ordem, sozinha, não
é o que faz. O que faz é o **encontro ser atômico**. Lei que lista menos casos
do que existem não protege menos hoje; protege menos no dia em que alguém usar
a lista como inventário — a mesma forma do portão de permissão que não olhava o
`juntar`.

#### 12.6.2 A matriz de queda — o laço de HOJE

Instante da queda × o que fica no disco × o que a recuperação faz. As linhas
saem do `transacao.rs`: a marca é sincronizada inteira antes da primeira
escrita, a reaplicação anda **para a frente** e é **idempotente pelo rowid**.

| # | instante da queda | o que fica no disco | o que a recuperação faz | |
|---|---|---|---|---|
| Q1 | depois do `fsync` da marca, antes da passada | a marca; nenhuma escrita | reaplica tudo (`reaplicadas`) | ✔ |
| Q2 | no meio da passada | a marca; parte das escritas | reaplica o que falta; o que já estava conta em `ja_aplicadas` | ✔ |
| Q3 | passada inteira, janela **aberta** (marca pendurada) | a marca; as escritas só no cache do núcleo | reaplica o que a queda de energia levou | ✔ |
| Q4 | no meio do laço do fecho — `A` sincronizada, `C` não | a marca; `A` durável, `C` no cache | reaplica `C` | ✔ |
| Q5 | todas sincronizadas, **antes** de apagar as marcas | a marca; tudo durável | reaplica **zero** — 100% `ja_aplicadas` | ✔ |
| Q6 | depois de apagar as marcas | nenhuma marca; tudo durável | nada a fazer | ✔ |

A linha **Q5 é o preço da ordem**, e é o preço certo: uma janela em que a
recuperação faz trabalho à toa. A ordem inversa — apagar antes de sincronizar —
trocaria esse trabalho à toa por uma janela em que o commit confirmado não tem
bilhete nenhum, e essa não tem conserto depois.

#### 12.6.3 A matriz de queda — o laço QUEBRADO (a trava se solta entre tabelas)

As seis linhas acima **continuam ✔**: uma queda sozinha não quebra o laço
quebrado. O que muda são os instantes que **só existem quando a trava se
solta**, e por isso a coluna do meio ganha um ator.

| # | instante | o que fica no disco | o que a recuperação faz | |
|---|---|---|---|---|
| Q7 | trava solta entre `A` e `C`; **ninguém entra**; queda | a marca; `A` durável, `C` no cache | reaplica `C` | ✔ |
| Q8 | trava solta; outro escritor **W** grava em `A` e comete. A janela reabriu no fecho, então `gravar_de_verdade` só marca `A` como suja e a marca de **W** vai para `marcas_pendentes`. O fecho volta, sincroniza `C` e apaga **todas** as marcas pendentes — a de **W** inclusive. Queda de **energia** | nenhuma marca de **W**; a escrita de **W** só no cache do núcleo | **nada** — não há bilhete | ✘ **perde um commit confirmado, em silêncio** |
| Q9 | trava solta; **W** grava (o que põe a tabela dele no conjunto de sujas, que o outro fecho já tinha drenado) e a janela fecha para ele: **W** entra em `descarregar_sujas_com` também. São dois fechos ao mesmo tempo, e o de **W** tem lista não-vazia, então ele chega até a drenagem das marcas — e apaga também as marcas do fecho que ainda está em curso, cuja tabela `C` não sincronizou. Queda de energia | nenhuma marca; `C` só no cache | nada | ✘ mesma perda, por outro caminho |
| Q10 | igual ao Q8, mas queda de **processo** (`SIGKILL`) e não de energia | as escritas já estão no núcleo | nada a fazer, e o dado está lá | ✔ — **e é isto que torna o defeito invisível** |

**Três coisas que esta metade da matriz decide.**

**(a) O Q8 não é corrida rara: é o caminho comum.** Quem fecha a janela a
*reabre* ao fechá-la. Então toda gravação que chegar durante o fecho cai
exatamente nesse estado — marca pendurada, tabela suja — e o fim do laço a
apagaria. Não é «se dois threads se alinharem»; é «se alguém gravar enquanto o
fecho acontece», que é o que um servidor ocupado faz o tempo todo.

**(b) A bateria de durabilidade NÃO acusaria.** A linha Q10 é a mesma lição do
pedido 186, oito horas depois e no mesmo arquivo: a `bancada/durabilidade`
prova com `SIGKILL`, e página suja no cache do núcleo **sobrevive** a processo
morto. O defeito do Q8 só aparece em queda de energia — e nenhum processo em
espaço de usuário provoca uma. *`SIGKILL` prova o protocolo; quem prova
durabilidade é a contagem de `fsync`.*

**(c) O que a quebra custaria para ser segura, dito para não voltar sem
número.** Não é impossível — é caro, e o preço é uma contabilidade por marca:
`marcas_pendentes` passaria a guardar `(caminho, [(tabela, geração)])`, o
conjunto de sujas viraria `tabela → geração`, entraria um `tabela → geração
sincronizada`, e um portão de reentrância para dois fechos não se atropelarem.
**Nada disso muda o formato em disco** — é tudo RAM —, mas muda o **protocolo**
do group commit, que é a peça que a §5.7 do `docs/TRANSACOES.md` prova célula a
célula. Ficou **recusado**, e o motivo não é o risco sozinho: é que existe ganho
maior de graça, abaixo.

#### 12.6.4 A saída que não estava na lista: as K vão ao disco JUNTAS

O `o-comboio-por-dentro` já tinha dividido o fecho: `abrir` 5–7%, `fsync`
93–96%. A pergunta que ninguém tinha feito é mais barata que a da trava:
**esse `K × fsync` precisa mesmo ser em série?**

Não precisa, e a razão é que **não há ordem ENTRE tabelas para preservar**. A
ordem que existe é a de dentro de cada tabela — `.trash` antes do `.reg`,
escrita no `Table::sincronizar` —, e ela continua inteira. O encontro continua
atômico: as marcas saem depois do `join`, sob a mesma tomada da trava. Nenhuma
linha da matriz muda, porque nenhum instante novo nasce.

E o sistema de arquivos ajuda: num `ext4` os `fsync` de arquivos diferentes se
juntam no mesmo diário. Medido com `--example o-comboio-em-paralelo`, com as
tabelas semeadas em 2.000 linhas e 30 janelas por arranjo, **alternando os dois
arranjos dentro da mesma corrida** para que uma variação da máquina caia nos
dois lados:

| K | A · série (µs) | A · paralelo | A · ganho | B · série (µs) | B · paralelo | B · ganho |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1.390 | 1.391 | 1,00× | 1.342 | 1.555 | 0,86× |
| 2 | 2.304 | 1.916 | 1,20× | 2.769 | 2.180 | 1,27× |
| 4 | 4.821 | 2.975 | **1,62×** | 5.459 | 3.484 | **1,57×** |
| 8 | 9.467 | 4.593 | **2,06×** | 10.516 | 5.339 | **1,97×** |
| 16 | 21.678 | 8.603 | **2,52×** | 21.200 | 8.766 | **2,42×** |

Duas baterias limpas, cada uma conferida contra o `esta-medindo.sh` **antes e
depois** da corrida — as contaminadas foram descartadas na hora, e há três
delas nesta rodada.

**O K=1 é a linha que mais ensina depois do 2,52×.** As duas baterias
discordam — 1,00× e 0,86× — e a discórdia é do *medidor*, não do servidor: o
exemplo passa pelo `thread::scope` mesmo com uma tabela só, e o que aparece ali
é o custo de subir um fio para não sobrepor nada. **O servidor não paga isso**:
`descarregar_sujas_com` tem o atalho de `abertas.len() == 1`, que sincroniza na
própria thread. O caminho de uma tabela só continua sendo exatamente o de
antes, **por construção e não por medição** — e é o comportamento velho que
mais precisa de garantia numa mudança destas.

**E o ganho não vem de `fsync` a menos**, que é a única forma de esse número
ser falso. Contado por `strace` num processo filho, com K=4: **32 `fsync` nos
dois arranjos**, 8 por tabela — que é a `TETO_FSYNC_POR_FECHO_V2`. A catraca
não se mexe, e a guarda que cobra isso é
`crates/phxsql-store/tests/fecho-em-paralelo-conta-os-mesmos-fsync.rs`.

#### 12.6.4-bis O mesmo pelo LADO DE FORA — e o que a máquina não deixou medir

O número acima é o **tempo de trabalho** do fecho: quanto a trava fica presa. A
§12.3 mede a outra ponta — a **espera de quem está atrás**, pela rede. Para que
os dois arranjos fossem comparados no mesmo dia e na mesma árvore, os dois
`phxsqld` foram construídos lado a lado e a bancada rodou com `PHX_PHXSQLD`
apontando para cada um.

O `quieta.Vigia` da própria bancada reprovou a maioria das corridas — «há outra
frente trabalhando nesta máquina» —, e o que passou foram **duas de cada
arranjo**, intercaladas ao longo de duas horas:

| p99 do escritor | K=1 | K=4 | K=4 ÷ K=1 |
|---|---:|---:|---:|
| série · corrida A | 3.055 µs | 7.372 | 2,41× |
| série · corrida B | 2.797 µs | 7.808 | 2,79× |
| **paralelo · corrida A** | 3.049 µs | **6.209** | **2,04×** |
| **paralelo · corrida B** | 2.727 µs | **5.827** | **2,14×** |

**A coluna do K=1 é a que dá confiança na tabela inteira**: 3.055, 2.797, 3.049
e 2.727 µs — as quatro iguais dentro do ruído, e tinham de ser, porque com uma
tabela suja só não há comboio nenhum e o código é literalmente o mesmo (o
atalho do `abertas.len() == 1`). O que muda é o K=4, que cai de **7,4–7,8 ms
para 5,8–6,2 ms**, e com ele o fator do comboio: de 2,41×–2,79× para
**2,04×–2,14×**.

**E os dois lados da medição concordam**, que é o que fecha a conta: por dentro
o fecho de K=4 encolhe 1,57×–1,62×, e por fora o p99 do escritor em K=4 encolhe
1,26× — menor, e tinha de ser menor, porque o fecho é só uma parte da espera de
quem está atrás. Se o de fora tivesse encolhido *mais* que o de dentro, uma das
duas medições estaria errada.

**E a coluna do LEITOR desta bancada não entra**, de propósito: nesta árvore ela
saiu com p99 de centenas de milissegundos nas três corridas — contra os 4–8 ms
da §12.4-bis —, e a diferença é grande demais para ser o comboio. Há outra
frente mexendo no caminho de leitura do `.ndx` nesta mesma rodada. O número não
é meu para explicar, e publicá-lo como se fosse seria pior que omiti-lo: as
três corridas viram o mesmo, então a **comparação** entre os arranjos continua
válida, mas o **valor** não descreve o servidor que vai ser lançado.

#### 12.6.4-ter O conserto quebrou um INSTRUMENTO, e o número disso

Contra o `phxsqld` de pé, a `bancada/durabilidade/prova-do-fecho.py` — que
anexa `strace` no PID do servidor e conta `fsync` por arquivo — passou a sair
com a matriz quase toda **zerada**, acusando o próprio relógio: *«o `strace` foi
solto antes de `descarregar_sujas_com` terminar»*.

**Não era a máquina nem o `strace`; era a leitura do traço.** `strace -f` parte
uma chamada em duas linhas — `fsync(13</…/tab01.trash> <unfinished ...>` e, mais
adiante, `<... fsync resumed>) = 0` — sempre que outra thread entra num
`syscall` antes de a primeira voltar. **Só a primeira traz o caminho e só a
segunda traz o resultado**, e a expressão daquele script exigia o `= 0` na mesma
linha do caminho. Num fecho serial isso nunca acontecia; num fecho paralelo é o
caso normal.

| critério, sobre o mesmo traço de um fecho K=4 | `fsync` contados |
|---|---:|
| linhas contendo `fsync(` (a entrada, partida ou não) | **480** |
| a expressão antiga do `prova-do-fecho.py` | **310** |
| linhas `<unfinished ...>` / `<... fsync resumed>` | 170 / 170 |

Com o parser consertado — três expressões, e a volta casada com a ida **pelo
pid** —, a mesma bateria contra o servidor de verdade mostra o que se queria
ver, e é a prova de queda desta rodada pelo lado do sistema operacional:

| tabela suja | reg | ndx | bin | memo | log | trash | reason | total |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `a`, `b` (fecho por gravação) | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |
| `d`, `e` (fecho pelo relógio de fundo) | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |

Oito por tabela, o `.reg` entre eles, **em toda tabela suja e não só na que
disparou o fecho** — que é exatamente o que o pedido 186 consertou e o que o
arranjo novo tinha de preservar.

**A lição, e ela não é sobre `strace`:** *quem torna um caminho concorrente tem
de reler os instrumentos que o mediam em série.* Os outros quatro leitores de
traço da casa foram conferidos um a um e estão sãos por construção — contam por
`contains("fsync(")`, que casa a linha de entrada esteja ela partida ou não.
Está registrado em
`docs/cognicao/cognicao_strace-parte-syscall-concorrente-e-o-parser-perde-um-terco_20260905_1120.md`.

#### 12.6.5 A decisão, e o que ficou por fazer

**Papel C, 05/09:**

* **RECUSADO** soltar a trava entre uma tabela e a seguinte — matriz Q8/Q9, e
  o agravante do Q10: nenhuma bateria desta casa acusaria. Volta à mesa quando
  houver a contabilidade por marca da §12.6.3(c), e aí com a matriz refeita.
* **RECUSADO** fechar a janela por tabela, pelo mesmo Q8 por outro caminho: as
  marcas pendentes das *outras* transações continuariam precisando de um
  encontro, e sem contabilidade por marca esse encontro é o mesmo que existe
  hoje.
* **ACEITO** sincronizar as K tabelas ao mesmo tempo, com teto de fios
  (`FIOS_DO_FECHO = 16`, e acima disso vai em pedaços): 1,62× em K=4 e 2,52× em
  K=16, mesmo número de `fsync`, mesma ordem dentro de cada tabela, nenhuma
  linha nova na matriz de queda e nenhuma mudança de formato.

**O que fica por fazer, nomeado:**

* **A contabilidade por marca** da §12.6.3(c) — o único caminho conhecido que
  torna a quebra da trava segura. Desenhada, não feita. Quem a fizer refaz a
  matriz **antes** de escrever código.
* **O ganho depende do sistema de arquivos, e está medido num só.** O 2,52×
  vem de o `ext4` desta máquina juntar `fsync` de arquivos diferentes no mesmo
  diário. Num sistema que não junte, o que sobra é o custo dos fios — e esse
  custo **não está medido em separado**: as duas baterias limpas divergem no
  K=1 (1,00× e 0,86×), o que põe o preço de subir um fio em algum lugar entre
  zero e ~150 µs, e uma faixa dessas não é número. *O servidor não paga esse
  preço no K=1*, pelo atalho, mas paga em K≥2 se o sistema de arquivos não
  ajudar. **Quem publicar o motor sobre XFS, ZFS ou disco de rede roda o
  `--example o-comboio-em-paralelo` antes de acreditar nesta seção** — o
  medidor responde em segundos e o veredito dele já está escrito para os dois
  desfechos.
* **`FIOS_DO_FECHO = 16` não é catraca** — é teto de recurso, e não mede
  promessa nenhuma. Não se aplica a ele a lei do «só desce»; aplica-se a de
  medir com o `--example o-comboio-em-paralelo` antes de mudá-lo.

---

## 13. RETRATADA — a medição não mediu o que dizia medir

**Publicada às 07:25 de 04/09 e retirada às 07:52, pelo autor.** Ela dizia:

> «O teto do `RwLock` fica em ~2,0× com o `varrer` em limite 1, 10, 50 e 200 —
> uma faixa de 200× no número de linhas — porque *o formato da leitura não muda
> o custo dela*. A ressalva da §3.1 se aposenta.»

**O medidor mandava um campo que o servidor não lê.** O `op_varrer` pega o
tamanho da página em `Servidor::limite(p)`, que lê **`max`**; a bancada mandava
**`limite`**, que não existe nesse pedido. Toda leitura caía no teto de
configuração (`max_linhas`, padrão **1.000**). As quatro «variações» custaram
o mesmo porque **eram a mesma leitura**.

Provado com o servidor de pé, tabela de 1.500 linhas:

| pedido | linhas devolvidas |
|---|---:|
| `"limite": 1` | **1000** |
| `"limite": 50` | **1000** |
| `"limite": 200` | **1000** |
| `"max": 1` | 1 |
| `"max": 50` | 50 |
| `"max": 200` | 200 |
| nada | 1000 |

**O pedido 182 morre junto.** Ele perguntava «onde vão os ~4,8 ms dentro do
caminho de leitura», e não há mistério: os 5 ms são **mil linhas** lidas,
convertidas em JSON e mandadas pelo fio. O `--example onde-doi-na-leitura`,
escrito para caçá-los, mediu **44 µs** para `abrir + pagina + ler` com uma
linha e **513 µs** com duzentas — as linhas custam, e quase linearmente. Era a
bancada de rede que não pedia o que dizia pedir.

**Como apareceu:** não pela bancada, que era coerente consigo mesma e passou no
vigia duas vezes. Apareceu porque o medidor **em processo** discordou dela —
lá dentro 200 linhas custavam 11,6× uma linha, e pela rede custavam o mesmo.
*Dois instrumentos que discordam é uma sorte: um deles está errado e agora se
sabe disso.*

---

## 14. O alcance do campo errado — quatro bancadas, e o rótulo que ninguém conferiu

O defeito da §13 não era só meu: **os quatro medidores de concorrência
mandavam `"limite"`**, e três deles são de 03/09.

| medidor | mandava | lia de verdade |
|---|---|---:|
| `a-trava-serializa.py` | `"limite": 50` | 1.000 linhas |
| `quanto-a-trava-fica-presa.py` | `"limite": 50` | 1.000 linhas |
| `escolher-o-desenho.py` | `"limite": 50` | 1.000 linhas |
| `o-comboio-do-fecho.py` | `"limite": 50` | 1.000 linhas |
| `bancada/bateria/prova-bateria.py` | `"max": 500` | **500 — certo** |

A bateria de ponta a ponta sempre mandou `max`. **A família da concorrência
inteira mandava `limite`** — e como todas mandavam o mesmo, nenhuma discordava
de nenhuma, e o rótulo «`varrer` de 50 linhas» atravessou seis seções deste
documento sem que ninguém tivesse motivo para conferi-lo.

### 14.1 O que isto muda no que já está publicado

**As razões continuam valendo; os RÓTULOS não.** Cada bateria comparou curvas
medidas com a mesma leitura, então os tetos são comparações honestas entre
desenhos. O que estava errado é *qual carga* elas descrevem:

| seção | dizia | é |
|---|---|---|
| §3.1 | «um `varrer` de **50 linhas**» | de **1.000** |
| §7.1 | «trava presa LENDO (`varrer` 50)» | `varrer` de 1.000 |
| §11 | tetos com leitura de 50 | com leitura de 1.000 |
| §12 | o leitor do comboio, 50 | 1.000 |

E há uma consequência que **fortalece** a ressalva da §3.1 em vez de aposentá-la:
ela dizia que a forma escolhida favorece o `RwLock` por construção, porque a
leitura segura a trava ~20× mais que a escrita. Com 1.000 linhas contra 1, os
20× estão **inteiramente explicados** — e a ressalva vale mais, não menos.
**A §13 tentou aposentá-la e o que aconteceu foi o contrário.**

### 14.2 O conserto, e o que ele deliberadamente NÃO faz

Os cinco medidores passam a mandar `max`. O `escolher-o-desenho.py` ganha
`LINHAS_LIDAS`, com padrão **1.000** — e o padrão é 1.000 **de propósito**:
é o que as baterias de 03/09 e 04/09 mediram de fato, e trocar o número junto
com o campo tornaria toda corrida nova incomparável com as publicadas. Quem
quiser o perfil de leitura curta manda `LINHAS_LIDAS=50`.

*Consertar o instrumento e mudar a régua no mesmo commit é perder a série.*

E as quatro bancadas passam a chamar `quieta.confira_a_pagina` antes de
qualquer número sair: ela pede 1, 7 e 50 linhas **pelo construtor de pedido da
própria bancada** e confere que vieram 1, 7 e 50. Receber o construtor, e não
montar o pedido dentro da guarda, é a diferença entre conferir o servidor e
conferir a bancada — a primeira versão dela montava `{"max": n}` sozinha e
teria passado com o defeito de pé. *Guarda que não percorre o caminho do
defeito é decoração.*

### 14.3 O que ISTO abre, e que fica nomeado

Com o campo certo, o `quanto-a-trava-fica-presa.py` passa a ler **50 linhas de
verdade** — e numa corrida curta de fumaça a trava presa lendo caiu de
~3.150 µs para a ordem de **500 µs**, que é o que se espera de 50 linhas em vez
de 1.000. **Não publico esse número**: a corrida foi curta e a máquina não foi
conferida pelo vigia. Mas ele diz o que falta: **as baterias da §7.1, §11 e §12
merecem ser refeitas com o instrumento consertado**, agora que «`varrer` 50»
quer dizer 50. As razões publicadas continuam válidas para a carga de 1.000
linhas; o que não existe ainda é a série para a carga de 50.

---

## 15. O perfil da carga, medido DE VERDADE (04/09) — e a ressalva cai

A §13 tentou isto e mediu quatro vezes a mesma leitura. Com o campo certo
(`max`), duas baterias limpas às 07:59 e 08:05, o `quieta.Vigia` aprovando as
duas. Corridas cruas em `corridas/perfil-CERTO-*`.

### 15.1 Agora a leitura VARIA — e o teto não

| `max` | A · ler 1 cli | A · razão ler/gravar | A · teto | B · razão | B · teto |
|---:|---:|---:|---:|---:|---:|
| 1 | 4.968 op/s | **0,9×** | 1,80× | **0,8×** | 1,79× |
| 10 | 4.118 op/s | 1,1× | 2,05× | 1,0× | 1,98× |
| 50 | 2.078 op/s | 2,1× | 1,86× | 1,9× | 1,81× |
| 200 | 819 op/s | 5,3× | 2,15× | 5,0× | 1,95× |
| 1.000 | 192 op/s | **22,5×** | 2,06× | **19,5×** | 2,05× |

A razão de custo entre uma leitura e uma escrita percorre **28×** — de 0,8×,
onde a leitura é mais **barata** que a escrita, a 22,5×. O teto do `RwLock`
fica entre **1,79× e 2,15×** nas dez medições, **sem tendência**.

### 15.2 A ressalva da §3.1 cai, com o número

Ela dizia:

> «Um perfil de carga em que a escrita domina **inverte a conclusão**, e a
> resposta certa muda com ele.»

**Não inverte.** Em `max = 1` a leitura custa **menos** que a escrita — é o
perfil «escrita domina» que a ressalva descrevia — e o teto do `RwLock` ainda
é **1,80× e 1,79×**. A recomendação do desenho **generaliza**, e isso agora
está medido em vez de suposto.

**Por que ela não vale, e é aqui que se aprende algo:** a ressalva supunha que
o teto vem da *duração* da seção crítica — leitura longa, trava presa por mais
tempo, mais a ganhar em soltá-la. Se fosse só isso, uma leitura de 201 µs
(mais curta que a escrita de 231 µs) daria teto ~1,0×. Dá 1,80×. **O que
serializa não é só o tempo sob a trava; é o pedido dela.** Uma leitura curta
espera pela trava tanto quanto uma longa.

### 15.3 A §13 chegou à conclusão certa, e isso não a salva

A seção retratada afirmava «o teto não depende do formato da leitura» — que é
o que a §15 mede. **Conclusão certa tirada de medição inválida não é acerto.**
A §13 dizia que o teto não varia *porque o custo da leitura não varia*, e o
custo varia 28×. Ela acertou o resultado e errou o mundo inteiro em volta
dele — e, pior, teria «aposentado» a ressalva com um argumento que qualquer um
derrubaria no primeiro exame, levando a conclusão certa junto.

É a versão desta casa de uma lei que já está escrita: *o errado sobrevive
melhor quando o conserto funcionou por outro motivo.*

### 15.4 Uma ressalva NOVA, e esta é honesta

O teto absoluto **não se compara entre bancadas**: aqui deu 1,79×–2,15× e na
§11 deu 2,48×–2,99×, com a mesma leitura de 1.000 linhas. A diferença está no
**controle**: o `ping` escalou 4,01×–4,95× na §11 e 3,18×–3,21× aqui, e o teto
é a razão entre o ganho do controle e o da leitura. Então o número honesto é
**«o `RwLock` tem entre ~2× e ~3× para recuperar»**, e não uma casa decimal.

O que **é** comparável, e é o que esta seção entrega, são as dez medições
**dentro da mesma bateria**: ali o controle é o mesmo para todas as linhas, e
a ausência de tendência ao longo de 28× de razão é o achado.

---

## 16. O `RwLock` ENTROU — a ficha compartilhada do `varrer` (04–05/09)

As §11 e §15 mediram o teto e pararam ali: *«o `RwLock` tem entre ~2× e ~3×
para recuperar»*. Esta seção é a entrega, e ela é deliberadamente pequena —
**uma operação**, o `varrer`, que é a leitura de grade e o que a tela faz o dia
inteiro. As outras 75 seções continuam exatamente como estavam.

### 16.1 O que NÃO se fez, e por quê

**`RwLock<Instancia>` continua não compilando, e o marcador `!Sync` continua no
lugar.** A §2 explica por quê, e nada nela envelheceu: todo método da
`Instancia` é `&self`, e `&self` é o que um guard de LEITURA entrega. Se a
`Instancia` fosse o conteúdo do `RwLock`, dois escritores tomariam guard de
leitura e abririam dois `Table` sobre os mesmos arquivos — **sem um erro do
compilador**.

O que mudou é o que está DENTRO da trava:

```rust
dados: RwLock<Raiz>          // era Mutex<Instancia>
```

A `Raiz` tem um campo, o `PathBuf`, e separa as duas fichas **pelo tipo do
empréstimo**, que é a única coisa que um `RwLock` sabe distinguir:

| empréstimo | quem o entrega | o que ele alcança |
|---|---|---|
| `&Raiz` | o guard de LEITURA, a N threads ao mesmo tempo | `Raiz::abrir_para_ler`, e **só** ela |
| `&mut Raiz` | o guard de ESCRITA, a uma de cada vez | `Raiz::exclusiva()`, e por ela a `Instancia` inteira |

A `Instancia` **não mora dentro da `Raiz`**, e isso não é economia: um campo
`!Sync` faria a `Raiz` também `!Sync`, e aí o `RwLock` voltaria a não compilar.
Ela nasce do `PathBuf` a cada `exclusiva()`, presa por tempo de vida ao
empréstimo mutável.

### 16.2 A garantia é do COMPILADOR, e ela se prova nos dois sentidos

Convenção documentada foi recusada de propósito: *convenção que o compilador
não conhece é convenção que uma refação apaga em silêncio*. A ficha
compartilhada devolve uma `TabelaLeitura`, que **não tem** método de escrita —
ela envolve o `Table` em vez de derivar dele, porque um `Deref<Target = Table>`
seria uma linha e devolveria `inserir`, `excluir` e `sincronizar` junto.

A prova é um par de *doctests*, e o par é o ponto: um `compile_fail` sozinho
passaria também por um erro de digitação.

```
compile_fail:  fn grava(t: &mut TabelaLeitura) { let _ = t.inserir(&[]); }
compila:       fn grava(t: &mut Table)         { let _ = t.inserir(&[]); }
```

E o corpo da varredura é **genérico sobre `Legivel`**, um trait de doze métodos
em que **nenhum escreve**. Os dois efeitos são de uma vez só: não há segunda
cópia da varredura para divergir, e o corpo não consegue escrever em ficha
nenhuma. O que ele deixaria para a trilha volta como valor, e quem tem a ficha
exclusiva é que grava.

### 16.3 O achado que quase custou a entrega: abrir para LER escreve

A lista de métodos não era o buraco. **Uma varredura escreve em seis lugares**,
e quatro deles estão dentro do construtor — está contado, com arquivo e
condição, em `docs/cognicao/cognicao_abrir-para-ler-escreve_20260904_2330.md`.
Resumo:

| onde | o quê | quando |
|---|---|---|
| `LixeiraFile::abrir` | **cria** o `.trash` | tabela nascida antes do arquivo existir |
| `MotivoFile::abrir` | **cria** o `.reason` | idem |
| `LogFile::abrir` → `curar` | regrava o cabeçalho do `.log` | diário que ficou para trás numa queda |
| `RegFile::abrir` → `terminar_troca_interrompida` | `rename` de volume | alteração de estrutura interrompida |
| `Servidor::abrir_travada` | `espelhar()` cria o `.bkp` | `recursos.espelho` ligado, tabela sem espelho |
| `op_varrer` | `registrar_acesso` grava a trilha | tabela com coluna de dado pessoal |

As quatro primeiras viraram recusa: `Table::abrir_para_ler` devolve
`SemEscrever::PrecisaEscrever(motivo)`, e o motivo **nomeia o componente** — a
lixeira que falta e o diário que pede cura se consertam diferente. As duas
últimas são conferidas por `Servidor::abrir_para_ler_travada`.

Nenhuma das seis é erro para quem chama: a ficha compartilhada é **solta** e o
trabalho é refeito na exclusiva. Soltar antes de pedir é obrigatório — pedir as
duas na mesma thread é o abraço mortal que a `COM_A_TRAVA` acusa, e num
`RwLock` ele é pior que num `Mutex`: com um escritor na fila, a segunda leitura
trava as três pontas.

### 16.4 As guardas, e o defeito reposto de cada uma

**O teste que mais importa é o do comportamento velho**, e ele é
`sem_a_ficha_compartilhada_nada_muda`: o mesmo pedido, respondido pelas duas
fichas, tem de dar o mesmo JSON. A ficha exclusiva é forçada **sem mexer no
pedido** — apagando o `.trash`, o que faz a abertura precisar criar arquivo —,
e o teste exige duas coisas: a resposta igual **e** o `.trash` de volta. Sem a
segunda, ele compararia a mesma pista com ela mesma e passaria por engano.

| guarda | defeito reposto | o que caiu |
|---|---|---|
| `a_trilha_de_dado_pessoal_sobrevive_a_pista_de_leitura` | a pista aceita tabela com coluna marcada | a trilha fica **vazia** — e trilha que perde registro em silêncio parece completa |
| `o_espelho_continua_nascendo_no_varrer` | a pista aceita tabela sem `.bkp` com `espelho` ligado | o espelho **não nasce** |
| `sem_a_ficha_compartilhada_nada_muda` | o recuo vira erro em vez de recuo | a segunda varredura **falha** |
| `a_tabela_que_precisaria_escrever_para_abrir_manda_para_a_exclusiva` | `abrir_sem_escrever` cria a lixeira | abre criando arquivo **sob a ficha compartilhada** |
| doctest `compile_fail` | — | o par prova que o `compile_fail` não passa por engano |

Mais duas que exercitam o que a mudança existe para permitir:
`quatro_leitores_ao_mesmo_tempo_leem_a_mesma_pagina` (quatro threads dentro da
ficha compartilhada, cem varreduras, mesma resposta) e
`o_escritor_nao_passa_fome_entre_leitores` — que é a pergunta que o `Mutex` não
levantava: **num `RwLock`, quatro leitores em laço fechado podem deixar o
escritor esperando para sempre?** As vinte gravações chegam, e o prazo do teste
é o que acusaria a fome, porque fome não dá erro: ela demora.

### 16.5 As catracas

`so_um_lugar_toma_a_trava` passa a cobrar **um** `self.dados.write()` e **um**
`self.dados.read()`, cada um dentro da função que o batiza. O teto não subiu —
nasceu outro ao lado, com o mesmo valor 1: uma catraca de «duas tomadas no
arquivo» aceitaria as duas na mesma função, ou a de leitura solta no meio de um
`op_`.

E nasce `so_uma_operacao_usa_a_ficha_compartilhada`, teto **1**. A decisão do
dono foi «só o `varrer`», e sem catraca a segunda leva entra por distração:
`op_ler`, `op_buscar` e `op_sistabelas` são todas leituras e todas parecem
óbvias — e **cada uma tem escrita escondida própria para achar antes**, como
esta teve três.

O `mapa-da-trava.py --catraca` continua em **5 / 22 / 0**, e as 76 seções
continuam 76: o `varrer` mantém a tomada exclusiva do recuo. Mas fica nomeado o
que o mapa **não** vê: ele conta `travar_dados()`, e a ficha compartilhada é
uma seção crítica que não passa por lá. *Régua que passa a medir mais aposenta
a catraca e faz nascer outra* — quem for medir a segunda leva mexe na régua
primeiro, e recomeça os três tetos no número do dia.

### 16.6 O ganho, MEDIDO — quatro baterias limpas (05/09, 00:16 a 01:02)

O protocolo é o das §11 e §15: `escolher-o-desenho.py` com `LINHAS_LIDAS=50`
(a carga da tela, uma página de grade), `SEGUNDOS=5`, clientes 1/2/4, nas duas
durabilidades, e o `quieta.Vigia` aprovando cada bateria. **Duas baterias
limpas de cada lado**, com o binário de ANTES guardado antes de a primeira
linha ser escrita — o mesmo `phxsqld`, não uma reconstrução do passado.

O custo do protocolo, contado: **quatro reprovadas** pelo vigia para as quatro
limpas (uma no ANTES, três no DEPOIS). Nenhuma delas imprimiu número.

Corridas cruas em `bancada/concorrencia/corridas/ficha-{antes,depois}-*.txt`.

#### O escalonamento da leitura, que é o que a mudança compra

Quanto quatro clientes rendem sobre um, lendo a **mesma** tabela:

| durabilidade | ANTES · A | ANTES · B | DEPOIS · A | DEPOIS · B |
|---|---:|---:|---:|---:|
| `por_lote` (o padrão) | 1,59× | 1,71× | **3,93×** | **3,87×** |
| `por_operacao` | 1,76× | 1,66× | **3,81×** | **3,89×** |

Quatro clientes em quatro núcleos rendiam **1,6×–1,8×**; passam a render
**3,8×–3,9×**. O que sobrava de paralelismo era a exclusividade comendo, e é
exatamente o que a §11 media como teto.

#### A vazão absoluta, e por que ela precisa de âncora

Comparar op/s entre duas baterias só vale com o **controle** ancorando as duas,
e a §15.4 é a razão. Na bateria B, o `ping` de um cliente deu **10.069** op/s
antes e **9.835** depois no `por_lote` (2,4% de distância), e 9.602 contra
9.850 no `por_operacao` (2,6%). Com isso:

| medida | ANTES | DEPOIS | |
|---|---:|---:|---:|
| `varrer` de 50 linhas, **1** cliente, `por_lote` | 1.997 op/s | 1.928 op/s | 0,97× |
| `varrer` de 50 linhas, **4** clientes, `por_lote` | 3.412 op/s | **7.465 op/s** | **2,19×** |
| `varrer` de 50 linhas, **1** cliente, `por_operacao` | 2.015 op/s | 1.980 op/s | 0,98× |
| `varrer` de 50 linhas, **4** clientes, `por_operacao` | 3.349 op/s | **7.708 op/s** | **2,30×** |
| `inserir`, 1 cliente, `por_lote` | 4.067 op/s | 3.890 op/s | 0,96× |
| `inserir`, 4 clientes, `por_lote` | 5.770 op/s | 5.469 op/s | 0,95× |
| `inserir`, 1 cliente, `por_operacao` | 651 op/s | 649 op/s | 1,00× |
| `inserir`, 4 clientes, `por_operacao` | 698 op/s | 739 op/s | 1,06× |

**O cliente sozinho não ficou mais rápido, e não devia**: sem disputa, tomar a
ficha compartilhada custa o mesmo que tomar a exclusiva. Os 0,97× e 0,98× são o
ruído desta bancada.

**A escrita não pagou a conta**, que era o risco: 0,95×–0,96× no `por_lote` e
1,00×–1,06× no `por_operacao`. Só a bateria B trouxe número absoluto, então os
4% do `por_lote` estão **medidos uma vez**, não duas — e a bateria A, que só
publica razão, mostra a escrita com quatro clientes escalando **melhor** depois
(1,32× → 1,43×). Fica registrado como está: pequeno, num só sentido, e não
confirmado em duas baterias.

#### O teto que sobrou

O «teto do `RwLock`» é o ganho do controle dividido pelo ganho da leitura — o
que ainda haveria para recuperar:

| durabilidade | ANTES · A | ANTES · B | DEPOIS · A | DEPOIS · B |
|---|---:|---:|---:|---:|
| `por_lote` | 2,43× | 2,04× | **0,81×** | **0,91×** |
| `por_operacao` | 2,49× | 2,93× | **1,22×** | **0,94×** |

E o teto **na espera** (p99 de dois leitores contra um leitor sozinho), que era
1,34 · 1,23 · 1,30 · 0,96, passa a **1,08 · 0,99 · 0,98 · 1,19**.

**Um teto abaixo de 1,00× não é regressão**, e vale explicar antes que alguém
leia o «0,81×» como perda: ele quer dizer que a leitura passou a escalar
**melhor que o próprio controle**. O `ping` é uma operação minúscula, dominada
por chamada de sistema e escalonador, e satura em 3,2×–3,7× com quatro
clientes; o `varrer` de 50 linhas é trabalho de verdade e escala 3,9×. A conta
do teto continua certa; o que ela diz é que **não sobrou nada para recuperar
neste par**.

#### Contra a previsão

A previsão que abriu esta frente era **1,54×–2,15×** no `por_lote` e
**2,56×–2,73×** no `por_operacao` — os tetos medidos em 04/09. As minhas
próprias baterias de ANTES mediram **2,04×–2,43×** e **2,49×–2,93×**, e o ganho
realizado foi **2,19×** e **2,30×**.

Ou seja: **dentro do teto medido no mesmo dia, e perto do topo dele.** O
`por_lote` superou a faixa prevista em 04/09 e o `por_operacao` ficou abaixo
dela — e a §15.4 já dizia por quê: *o teto absoluto não se compara entre
bancadas*, porque o controle escala diferente em cada uma. É por isso que o
ANTES foi remedido aqui em vez de citado de lá. **Número citado é número que
não se mede** — e o citado teria errado a faixa nos dois sentidos.

#### O que esta medição NÃO diz

* **Nada sobre o comboio do fecho de janela** (§12): ele continua real,
  continua sendo um escritor segurando a trava por trabalho que não é dele, e
  nem o `RwLock` nem o MVCC o consertam. A bancada roda com uma tabela.
* **Nada sobre leitura repetível** (§4.3, §11.3): a ficha compartilhada torna
  os leitores simultâneos, **não consistentes**. Uma varredura longa continua
  enxergando linha gravada no meio dela. É defeito de *resultado*, e nenhuma
  medição de p99 o mostraria.
* **Nada sobre as outras 75 seções.** Elas continuam exclusivas por decisão, e
  a segunda leva entra medida.

#### Uma ressalva sobre o INSTRUMENTO, nomeada e não medida

O `quieta.Vigia` desconta `len(perfis) + 1` tarefas rodáveis como suas: os
clientes mais **o servidor**. O servidor é um processo e `procs_running` conta
**tarefas**, e este servidor atende com uma thread por conexão — então quatro
clientes podem deixar quatro threads do servidor rodáveis ao mesmo tempo, e o
desconto tira uma.

A conta fecha com o que se viu (as reprovações vieram todas com «até 3» ou
«até 6», e 3 é exatamente `4 − 1`), **e conta que fecha não é medição**. Fica
como hipótese nomeada, com o motivo para não mexer: mudar o desconto **afrouxa**
o vigia, e afrouxar catraca é decisão do papel de QA, não de quem está sendo
medido por ela — e nunca no mesmo commit da mudança que ela mede.

O que se pode dizer com número: foram **quatro reprovações para quatro
baterias limpas**, e três das quatro reprovações caíram do lado DEPOIS.

### 16.7 A RAM, que a trava segurava de graça — e agora não segura mais

Isto precisa estar escrito onde a próxima pessoa vê, porque **é a única coisa
que a ficha compartilhada piora**.

O cache de páginas do `.ndx` é **por `Table` aberta**: 2.048 páginas de 4 KiB,
**8 MiB de teto por tabela aberta**. O comentário que justifica esse teto, no
`ndx.rs`, diz «o servidor abre e fecha a tabela a cada operação, então o teto
vale enquanto a operação dura» — e essa frase era verdadeira **porque a trava
global serializava tudo**. Com N leitores simultâneos, N × o que cada um ocupa.

O uso real está medido (`--example quanto-cache-uma-leitura-usa`, corridas em
`corridas/cache-por-leitura-*-CERTO.txt`), e o resultado é o que mais importa
aqui: **depende do caminho, e o caro é o ordenado**.

| a leitura | páginas (antes) | páginas (**hoje**) | residente hoje | do teto |
|---|---:|---:|---:|---:|
| grade **sem ordem** (50 ou 1.000 linhas) | 0 | **0** | 0,00 MiB | 0,0% |
| grade **ORDENADA** (50 linhas) | 1.668 | **3** | 0,01 MiB | 0,1% |
| grade **ORDENADA** (1.000 linhas) | 1.668 | **11** | 0,04 MiB | 0,5% |
| busca por chave | 3 | 3 | 0,01 MiB | 0,1% |
| varredura do índice inteiro | 1.668 | 1.668 | 6,52 MiB | 81,4% |

**A leitura por ordem de digitação não paga cache nenhum** — ela percorre o
`.reg` e não toca o índice.

A coluna «antes» é o que esta seção publicou em 05/09 de manhã, e ela media um
defeito: `pagina_por_indice` chamava `varrer_indice`, que percorre o índice
INTEIRO, e só depois recortava — por isso **50 linhas custavam o mesmo que
1.000**. Consertado na mesma data pelo **pedido 188** (§16.9): a varredura
passou a ler o índice em pedaços, e o custo da página voltou a ser o da
**página**. A última linha da tabela fica como régua: quem realmente pede o
índice inteiro continua pagando 1.668 páginas, e é assim que se sabe que o
medidor não ficou cego.

Multiplicado, e é aqui que o conserto se sente:

| leitores simultâneos | antes (uso medido) | **hoje** | pelo teto |
|---:|---:|---:|---:|
| 2 | 13,0 MiB | **0,02 MiB** | 16,0 MiB |
| 4 | 26,1 MiB | **0,05 MiB** | 32,0 MiB |
| 8 | **52,1 MiB** | **0,10 MiB** | 64,0 MiB |
| 16 | 104,2 MiB | **0,19 MiB** | 128,0 MiB |

**O teto continua segurando, e o que ele troca é RAM por trabalho:** acima de
2.048 páginas o cache despeja, a RAM para de crescer e a releitura volta. Quem
for mexer em `recursos.cache_paginas` está mexendo num número que se multiplica
por leitor, e não mais num número que a trava segurava sozinha — só que agora a
grade ordenada deixou de ser quem enche o cache.

E o número de leitores tem teto próprio: `conexoes_max` do `config.json`. Quem
o subir para centenas com `cache_paginas` no padrão está escolhendo, sem saber,
um teto de RAM de centenas × 8 MiB — e é por isso que esta tabela está aqui, e
não só no exemplo que a mediu.

### 16.8 A varredura de mutação achou duas guardas MORTAS — e nenhum portão as viu

Rodada depois da mudança, a varredura das guardas
(`bancada/guardas/provar-guardas.py`) julgou **81 guardas: 75 provadas, 4
redundantes, 0 não pegaram, 0 estragaram, 2 QUEBRADAS** — e as duas quebradas
eram do território desta frente:

| guarda | por quê |
|---|---|
| `trava-fora-do-ponto-unico` | o defeito reposto chamava `self.dados.lock()`, e com o `RwLock` esse método não existe: **o código com o defeito não compila** |
| `trava-sem-guarda-de-reentrancia` | o `trecho` passou a aparecer **duas vezes** no arquivo — a porta nova começa com a mesma pergunta de reentrância da porta velha, e trocar a errada provaria outra coisa |

**Os três portões passaram inteiros com as duas guardas mortas**: `fmt`,
`clippy` sem aviso, e `cargo test --workspace` com 1.585 testes verdes. A
varredura de mutação não roda no `cargo test`, e é por isso que ela pegou o que
ele não pega.

O conserto virou **três** entradas, e não duas: a de reentrância se dividiu
pelas duas portas (`trava-sem-guarda-de-reentrancia` para a exclusiva, com o
`trecho` carregando o comentário de cima para ficar único, e a nova
`leitura-sem-guarda-de-reentrancia` para a compartilhada), e a segunda precisou
de um teste que não existia — `as_duas_fichas_na_mesma_thread_viram_erro` —
porque o teste antigo pede a **mesma** porta duas vezes e não cobre nenhum dos
dois cruzamentos. O cruzamento é o pior caso: num `RwLock`, pedir a leitura com
a leitura na mão **e um escritor na fila** trava três pontas em vez de uma.

Depois: **82 guardas, 78 provadas, 4 redundantes, 0 quebradas**, 779 s de
mutação (`docs/TESTES.md` §12, gerado do `--json` da rodada).

O aprendizado inteiro, com o que eu concluí primeiro e estava errado, está em
`docs/cognicao/cognicao_porta-nova-quebra-a-guarda-da-porta-velha_20260905_0135.md`.

### 16.9 O pedido 188: a grade ordenada lia o índice inteiro (05/09)

Foi esta seção que achou o defeito, medindo o teto de RAM e não procurando
defeito nenhum: **50 e 1.000 linhas tocavam as mesmas 1.668 páginas**, porque
`Table::pagina_por_indice` chamava `varrer_indice` e só depois recortava.

A **ordem do dono foi medir na TELA primeiro** — número de motor não decide se
há frente, decide se o custo aparece para quem está olhando. Apareceu: num
navegador de verdade, contra o `phxsqld` de verdade, trocar o «Percorrer por»
numa tabela de 1.000.000 custava **98 ms com ordem contra 48 ms sem** — o motor
sozinho **dobrava** a espera da pessoa, sobre um piso de tela de ~48 ms que
nenhum conserto de motor remove.

O conserto (varredura em pedaços com cursor), as três escalas, a prova real nos
dois sentidos e o que ele **não** comprou estão em **`docs/DESEMPENHO.md` §19**.
O que interessa a este documento é o efeito na concorrência, e ele é a tabela da
§16.7: o pior leitor desta casa deixou de deixar **6,52 MiB residentes** para
deixar **0,01 MiB**, e o teto de RAM que a onda 1 mediu — 8 leitores ordenados
a ≈ 52 MiB — deixou de existir por outro caminho que não o teto do cache.

E há um segundo custo que a §16.7 não contava, porque ela contava páginas de
cache e não **alocação**: o `Vec<RowId>` intermediário carregava uma entrada por
linha da tabela, `RowId = u64` — **7,63 MiB por leitura ordenada, por leitor**, a
um milhão de linhas. Hoje ele carrega o pedaço que a página precisa.
