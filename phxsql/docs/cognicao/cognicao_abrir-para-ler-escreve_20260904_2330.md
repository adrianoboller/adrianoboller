# Abrir uma tabela para LER pode ESCREVER — e a garantia por tipo não cobria a abertura

**Descoberto em 04/09/2026, ~23:30**, escrevendo a ficha compartilhada do
`varrer` (leitor deixa de esperar leitor).

## 1. O que aconteceu

A lei desta casa já dizia, e com todas as letras, que
`RwLock<Instancia>` **compila de primeira e está errado**: todo método da
`Instancia` é `&self`, que é exatamente o que um guard de leitura entrega, e
dois escritores abririam dois `Table` sobre os mesmos arquivos sem um erro do
compilador. O marcador `!Sync` (`catalogo.rs`, campo `_so_com_a_ficha`) existe
para transformar esse engano silencioso em erro de compilação.

A lei nomeia os **métodos**. O buraco não estava neles.

Contadas uma a uma, uma varredura — o caminho que todo mundo chama de
«leitura» — escreve em **seis** lugares:

| onde | o quê | quando |
|---|---|---|
| `LixeiraFile::abrir` | **cria** o `.trash` | tabela nascida antes do arquivo existir |
| `MotivoFile::abrir` | **cria** o `.reason` | idem |
| `LogFile::abrir` → `curar` | regrava o cabeçalho do `.log` | diário que ficou para trás numa queda |
| `RegFile::abrir` → `terminar_troca_interrompida` | `rename` de volume | alteração de estrutura interrompida |
| `Servidor::abrir_travada` | `espelhar()` cria o `.bkp` | `recursos.espelho` ligado e tabela sem espelho |
| `op_varrer` | `registrar_acesso` grava a trilha | tabela com coluna de dado pessoal |

Quatro delas estão **dentro do construtor**, e nenhuma aparece na lista de
métodos que uma fachada de leitura esconderia.

## 2. O que eu concluí primeiro, e estava errado

Que a garantia por tipo era **esconder os métodos de escrita**: uma
`TabelaLeitura` sem `inserir`, `atualizar`, `excluir` e `sincronizar` estaria
provada, e o `compile_fail` provaria isso nos dois sentidos.

Está provado, e **não bastava**. A fachada teria compilado, o doctest teria
passado, e dois leitores simultâneos sobre uma tabela sem `.trash` criariam o
mesmo arquivo ao mesmo tempo: o `create_new` do segundo falha, ou — pior — ele
lê o cabeçalho que o primeiro ainda não terminou de gravar e a tabela «não
abre» sem motivo aparente.

O erro tem forma conhecida: **eu procurei a escrita na lista de verbos, e ela
estava no substantivo**. `abrir` não parece um verbo de escrita, e é por isso
que ninguém olha ali.

## 3. O que a medição disse

Não é medição de tempo: é contagem no fonte, e ela vale por ser exaustiva.
**Seis** escritas num caminho de leitura; **quatro** delas na abertura, que a
fachada não cobriria; **duas** raras a ponto de nunca terem aparecido como
defeito (a cura do diário e a troca interrompida acontecem depois de uma
queda), e **duas** que acontecem em toda tabela criada antes da versão que
introduziu o arquivo.

A quinta e a sexta são o oposto: acontecem **sempre** que a configuração ou o
esquema mandam. `espelho` ligado e tabela sem `.bkp`: toda abertura cria.
Coluna marcada como dado pessoal: toda varredura grava.

E a assimetria dos modos de falha decide o desenho: a fachada que aceita
demais perde **dado** (a trilha some, o espelho não nasce, dois processos
criam o mesmo arquivo); a que recusa demais perde **velocidade** — cai na
ficha exclusiva e atende como antes. Por isso a recusa é generosa e o recuo é
transparente.

## 4. A regra

> **Fachada de leitura se prova na ABERTURA, não só na lista de métodos.**
> Antes de deixar N leitores num arquivo, conte as escritas do construtor — e
> conte também as de quem chama, que é onde moram o espelho e a trilha.

E o corolário do recuo: quando a leitura descobre que precisaria escrever,
**solte a ficha compartilhada antes de pedir a exclusiva**. Pedir as duas na
mesma thread é o abraço mortal que a `COM_A_TRAVA` acusa — e num `RwLock` ele é
pior que num `Mutex`, porque com um escritor na fila a segunda leitura trava as
três pontas.

## 5. Como está guardado hoje

* `Table::abrir_para_ler` devolve `SemEscrever::PrecisaEscrever(motivo)` em vez
  de escrever, e o motivo **nomeia o componente** — quem lê o log precisa saber
  se foi a lixeira que faltava ou o diário que pedia cura, porque as duas se
  consertam diferente. As quatro recusas vêm de `RegFile::abrir_sem_escrever`,
  `LogFile::abrir_sem_escrever`, `LixeiraFile::abrir_sem_escrever` e
  `MotivoFile::abrir_sem_escrever`, cada uma ao lado da irmã que escreve.
* As duas de fora — espelho e trilha — são conferidas em
  `Servidor::abrir_para_ler_travada`, e cada uma tem teste com o **defeito
  reposto**: tirar a conferência do espelho derruba
  `o_espelho_continua_nascendo_no_varrer`; tirar a do dado pessoal derruba
  `a_trilha_de_dado_pessoal_sobrevive_a_pista_de_leitura`.
* O recuo tem teste próprio: `sem_a_ficha_compartilhada_nada_muda` apaga o
  `.trash`, refaz o mesmo pedido, e exige duas coisas — a mesma resposta e o
  `.trash` de volta. Sem a segunda, ele compararia a mesma pista com ela mesma.

**Onde o buraco ficou:** a contagem é de hoje e do fonte de hoje. Um arquivo
novo de tabela — como o `.lgpd` foi, e como o `.pag` foi — entra no motor e
ninguém volta aqui, que é a mesma armadilha da lista de extensões do
`excluir_tabela`, paga duas vezes (seis extensões quando eram nove, nove quando
eram dez). **Não há conferidor que cobre isso**: `Table::abrir_com` recebe
`escrever: bool` e quem acrescentar um componente novo precisa lembrar de
tratá-lo, exatamente como quem acrescenta uma extensão precisa lembrar da
lista. A guarda que existe é indireta e fraca — o componente novo que criasse
arquivo na abertura faria `a_tabela_que_precisaria_escrever_para_abrir_manda_para_a_exclusiva`
continuar passando, porque ele testa a lixeira e não o conjunto.
