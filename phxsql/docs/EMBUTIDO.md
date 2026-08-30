# PhxSql embutido — o motor como biblioteca, com ABI de C

> Este documento é o **desenho**, e foi escrito antes do código. O que ele
> promete, o `crates/phxsql-ffi/` cumpre; o que ele recusa, recusa com o
> motivo. A seção 9 traz o que foi **medido depois**, e a 10 o que ficou de
> fora.

---

## 1. A pergunta, e a correção de rumo

O pedido chegou assim:

> «no HFSQL(R) não roda o servidor, apenas as tabelas soltas sem cuidado, mas
> julgo que poderia ter um mini servidor para rodar no Android e no iOS
> off-line e se conectar por TCP/IP MULTILINK DATABASE/dblink com o servidor»

**O objetivo está certo e é o alvo desta frente**: banco local no aparelho,
funcionando sem rede, que sincroniza com o servidor central quando houver
rede. É exatamente o que falta para um aplicativo de campo — o vendedor no
interior, o técnico no galpão, o entregador no elevador.

**A forma — «um mini servidor escutando porta» — é a peça a corrigir**, e o
motivo não é nosso, é do sistema operacional:

| | por que um daemon com porta não serve |
|---|---|
| **iOS** | não permite processo de longa duração em segundo plano, nem um app escutando porta para outros apps usarem. Não é difícil: é proibido. Um app suspenso perde o soquete. |
| **Android** | mata processo em segundo plano com liberdade (Doze, App Standby, o descarte de app em cache). Um daemon que escuta porta sobrevive mal fora do Termux. |

A forma certa é **a mesma máquina com outra porta de entrada**: o motor
**embutido no processo do aplicativo** — sem porta, sem daemon, sem
`localhost` — mais um **cliente de sincronia** que fala TCP com o servidor
central quando há rede.

E a boa notícia, que esta frente conferiu antes de escrever uma linha:

> **O `phxsql-store` já é o banco embutido.** O `phxsql-server` é um envelope
> de rede em volta dele.

O `crates/phxsql-store/examples/basico.rs` cria tabela, insere, busca por
índice e varre — **sem soquete, sem `config.json`, sem thread de aceitação**.
Nenhuma linha do motor sabe que existe rede. Então esta frente não reescreve
nada: **expõe o que existe por uma ABI de C**.

```
                  ANTES                              AGORA

   app  ──TCP──▶ phxsqld ──▶ phxsql-store     app ──▶ phxsql-ffi ──▶ phxsql-store
                (envelope)     (motor)            (ABI C)              (motor)
                                                       │
                                                       └── e o MESMO motor
                                                           continua atrás do
                                                           phxsqld, intacto
```

`phxsql-server` **não foi tocado**. Porta de entrada nova não é troca de porta.

---

## 2. Por que uma ABI de C, e não JNI ou Swift direto

Android quer JNI (C), iOS quer C/Objective-C/Swift, Flutter quer `dart:ffi`
(C), React Native quer JSI (C++), .NET MAUI quer `P/Invoke` (C), Python quer
`ctypes` (C). **A ABI de C é o degrau que serve a todos** — e é o único
degrau que não obriga a escolher a plataforma antes de escrever o código.

Escrever JNI primeiro daria uma biblioteca que só o Android usa. Escrever C
primeiro dá uma biblioteca que o Android usa **através de** ~200 linhas de
JNI, e que o iOS usa direto.

### Os dois formatos, e por que os dois

```toml
crate-type = ["cdylib", "staticlib"]
```

| formato | arquivo | quem exige |
|---|---|---|
| `cdylib` | `libphxsql_ffi.so` | **Android** — o `.so` que entra no APK e o `System.loadLibrary` carrega |
| `staticlib` | `libphxsql_ffi.a` | **iOS** — a Apple não aceita biblioteca dinâmica de terceiros dentro do app; tem de ser ligada estaticamente |

Não é escolha de gosto: é o que cada loja permite.

E os dois só são plausíveis por causa da regra mais antiga da casa — **zero
dependências externas**. Uma `cdylib` que arrasta uma crate de C precisaria
que essa crate compilasse cruzado para `aarch64-linux-android` e para
`aarch64-apple-ios`; é aí que a compilação cruzada costuma morrer. Foi o que
fez o ARM sair de primeira (`docs/EMPACOTAMENTO.md` §7.1), e é o que torna o
iOS plausível.

---

## 3. As seis decisões da ABI

### 3.1 Nenhum `panic` atravessa a fronteira

Um `panic` do Rust desenrolando a pilha para dentro de um quadro de C é
**comportamento indefinido** — e num aplicativo de celular isso não aparece
como um erro tratável: aparece como o app fechando sozinho, sem log, com uma
avaliação de uma estrela.

Regra: **toda função exportada roda dentro de `catch_unwind`.** Isso é
teste, não comentário — ver §7.

E o corolário, que é a parte que se esquece: **capturar o pânico não conserta
o objeto.** Um `panic` no meio de um `inserir` pode ter deixado o `.reg` com
o cabeçalho gravado e o payload não. Continuar chamando aquele punho é
espalhar a inconsistência — é a mesma lição do `aplicar_evento`, que **para**
em vez de seguir quando a réplica divergiu.

Então o punho é **envenenado**: depois de um pânico capturado, toda chamada
naquele punho devolve `PHX_ERRO_ENVENENADO` e não toca no motor. Só o
`fechar` continua funcionando, porque o chamador precisa poder liberar a
memória. O conserto é reabrir — e reabrir passa pela recuperação de abertura,
que é o caminho certo.

> **Pegadinha registrada:** `panic = "abort"` no perfil de compilação
> transformaria todo `catch_unwind` em enfeite, sem um aviso sequer. O teste
> `panico_nao_atravessa_a_fronteira` é a catraca disso: com `abort`, ele não
> falha — ele **derruba o binário de teste inteiro**, e o tamanho do estrago
> é a prova de que a garantia sumiu.

### 3.2 Como o erro volta: **código de retorno + último-erro por thread**

Escolhido: toda função devolve `int32_t`; `0` é sucesso. Quem tem resultado
escreve por ponteiro de saída. A mensagem fica num espaço **por thread**,
lida com `phx_ultimo_erro`.

**Por que assim, e não uma struct de resultado:**

1. **Os códigos já existem e são públicos.** `PhxError::codigo()` é uma tabela
   estável desde sempre — 3002 é chave duplicada em qualquer idioma e em
   qualquer redação, e a regra escrita é *«número nunca muda e número
   aposentado nunca volta»*. Uma struct de resultado obrigaria a inventar um
   segundo vocabulário de erro para a mesma casa. **O cliente de C e o cliente
   de rede tratam o mesmo número.**
2. **O caminho feliz não paga nada.** Uma struct com mensagem exige alocar (e
   liberar) texto em toda chamada, inclusive nas que deram certo. Num laço de
   inserção de dez mil linhas isso é dez mil alocações jogadas fora. O código
   de retorno cabe num registrador.
3. **Por thread, e não por punho.** Um erro pode acontecer **antes de existir
   punho** — `phx_base_abrir` falhando é exatamente o primeiro erro que alguém
   vai encontrar, e uma vaga presa ao punho tem um buraco justamente aí.
   Global seria pior: duas threads escrevendo na mesma vaga fazem uma ler a
   mensagem da outra. `errno` é o precedente, e ele é por thread pelo mesmo
   motivo.

**As faixas:**

| faixa | o que é |
|---|---|
| `0` | deu certo |
| `1` (`PHX_NAO_HA`) | **não é erro**: a linha não existe, ou o cursor acabou |
| `1001…6001` | os códigos do PhxSql, os mesmos da porta de dados |
| negativos | problemas **da fronteira**, que o motor não tem vocabulário para descrever |

Os negativos, e por que cada um precisa existir:

| | | |
|---|---|---|
| `-1` | `PHX_ERRO_PANICO` | um pânico foi capturado na fronteira |
| `-2` | `PHX_ERRO_PONTEIRO` | argumento nulo onde não pode ser |
| `-3` | `PHX_ERRO_UTF8` | o texto recebido não é UTF-8 válido |
| `-4` | `PHX_ERRO_BUFFER` | o buffer do chamador é pequeno; o tamanho necessário sai no `precisa` |
| `-5` | `PHX_ERRO_USO` | a chamada não faz sentido (índice fora da faixa, cursor de outra tabela) |
| `-6` | `PHX_ERRO_ENVENENADO` | o punho sofreu um pânico e recusa trabalho (§3.1) |

`PHX_NAO_HA` ser `1` e não um erro é decisão consciente: `ler` de um rowid que
não existe é uma resposta, não uma falha. Confundir os dois é o que faz
aplicativo tratar «não achei» com caixa de erro vermelha.

### 3.3 Quem aloca e quem libera

**Regra de ferro: quem alocou, libera. A biblioteca nunca devolve um ponteiro
para o chamador chamar `free()`.**

Isso não é preciosismo. Numa `.dll` do Windows a biblioteca e o aplicativo
podem ter *CRTs diferentes*, e `free()` de um bloco alocado no outro monte de
memória derruba o processo. No Android o mesmo vale entre o alocador do Rust
e o `malloc` do bionic. É um defeito que só aparece na máquina do cliente.

Duas formas, cada uma pelo que ela resolve:

**(a) Punho que a biblioteca libera** — para o que o chamador **não consegue
dimensionar de antemão**: uma linha lida (um `Memo` pode ter 10 bytes ou 10
MB), a imagem de um evento de replicação.

```c
PhxLinha *l = NULL;
if (phx_ler(t, 1, &l) == PHX_OK) {
    const PhxValor *v; size_t qtd;
    phx_linha_valores(l, &v, &qtd);   /* ponteiros EMPRESTADOS */
    ...
    phx_linha_liberar(l);             /* aqui eles morrem */
}
```

Os `PhxValor` apontam **para dentro** do punho. Valem até o `liberar` — e
isso está dito na assinatura: `phx_linha_valores` não copia nada, é uma vista.

**(b) Buffer do chamador, com capacidade explícita** — para tudo que é
**pequeno e limitado**: mensagem de erro, nome de tabela, nome de coluna,
vetor de rowids de uma busca.

```c
char msg[512]; size_t precisa = 0;
phx_ultimo_erro(msg, sizeof msg, &precisa);
```

Buffer pequeno devolve `PHX_ERRO_BUFFER` e escreve em `precisa` quanto falta —
o chamador cresce e repete. Nada é perdido e nada é truncado em silêncio.

E há um motivo específico para a **mensagem de erro** ser desta forma: um
leitor de erro que aloca pode falhar ao alocar **enquanto relata uma falha**.
O caminho de erro é o último lugar onde se quer uma alocação.

**Vazamento em celular não aparece como vazamento**: aparece como o sistema
matando o app «sem motivo» meia hora depois. Por isso todo punho tem
exatamente um `liberar`, e todos eles têm nome com o mesmo formato.

### 3.4 Segurança de thread — dita com todas as letras

O que **é** garantido, e testado:

- **Punhos diferentes, threads diferentes: pode.** Duas threads, cada uma com
  a sua `PhxBase` e a sua `PhxTabela` sobre **tabelas diferentes**, trabalham
  ao mesmo tempo. Há teste (`duas_threads_duas_tabelas`).
- **O último-erro é por thread.** Um erro na thread A nunca aparece na
  thread B. Há teste (`ultimo_erro_e_por_thread`).

O que **não** é garantido, e por que:

- **O mesmo punho em duas threads ao mesmo tempo: não.** Todo método do
  `Table` recebe `&mut self` — **inclusive `ler`**, porque a leitura mexe no
  cache de páginas do `.ndx`. Não há trava dentro do motor: quem serializa,
  no `phxsqld`, é a trava global de dados do servidor, que mora **fora** do
  store. A ABI não inventa uma trava escondida, porque uma trava escondida
  custaria em todo aplicativo de uma thread só — que é o caso do celular.
- **Dois punhos sobre a MESMA tabela, em threads diferentes: não testado.**
  Os arquivos aceitariam, mas os cabeçalhos em memória divergiriam. Não
  prometemos o que não medimos.

A regra prática que o aplicativo segue, e que é a mesma de um SQLite embutido
em modo serializado: **um punho por thread, ou uma trava do lado do
aplicativo.** Os punhos podem migrar de thread; nunca ser usados por duas ao
mesmo tempo.

### 3.5 Strings: UTF-8 com tamanho explícito

Todo texto entra como **par `(ponteiro, tamanho)`**. Nenhuma função da ABI
chama `strlen` no dado do chamador.

Porque **dado de cliente tem byte zero**. Um `Bin` é binário por definição; um
`Memo` colado de um arquivo pode ter um `\0` no meio; uma senha gerada por
gerenciador pode ter qualquer byte. Um `NUL`-terminado trunca isso **em
silêncio** — grava metade e não avisa —, que é a pior classe de defeito que
existe: o que não dá erro.

Uma convenção só, inclusive para nomes de tabela e de coluna, onde
`NUL`-terminado funcionaria. Duas convenções no mesmo cabeçalho é como se
erra: o dia em que alguém usar a errada, ninguém percebe.

Para literais o cabeçalho traz o açúcar:

```c
#define PHX_T(s)  ((const uint8_t *)(s)), (sizeof(s) - 1)

phx_base_abrir(PHX_T("/data/app"), PHX_T("vendas"), PHX_CRIAR, &base);
```

Na saída, o mesmo: `(buffer, capacidade)` e o tamanho escrito. Os buffers de
saída **também** recebem um `\0` no fim quando cabe, por conforto de quem vai
`printf` — mas o tamanho é a verdade, e o `\0` é cortesia.

### 3.6 Punho com etiqueta, e o que ela pega

Todo punho começa com uma etiqueta de 64 bits, própria de cada tipo. Toda
chamada confere; `fechar` zera antes de liberar.

Isso **pega**, na prática: punho já fechado (uso-depois-de-liberar no caso
comum), punho do tipo errado passado na posição errada, memória zerada. Isso
**não pega** e não promete pegar: memória recém-liberada e reocupada por outra
coisa com o mesmo padrão de bytes. É uma rede, não um contrato — e está dito
assim no cabeçalho, porque prometer mais seria o mesmo erro de dizer
*ACID compliant* sem transação.

---

## 4. A superfície

**Quarenta e quatro funções**, contadas com `nm -D` pelo
`bancada/embutido/provar.sh` e não digitadas aqui — número visível que se
digita envelhece calado.

O critério de corte: **o que um aplicativo offline precisa para guardar dado e
sincronizar** — e nada além, porque cada função exportada é um compromisso que
não se desfaz.

### 4.1 Casa

| função | o que faz |
|---|---|
| `phx_versao` | a versão do motor, em texto |
| `phx_ultimo_erro` | a mensagem do último erro **desta thread** |
| `phx_erro_nome` | o nome simbólico de um código (`"DUPLICADO"`) — `const char*` estático, nunca liberado |

### 4.2 Base

| função | o que faz |
|---|---|
| `phx_base_abrir` | abre a raiz de dados e o database; `PHX_CRIAR` cria se faltar |
| `phx_base_fechar` | libera |
| `phx_base_tabelas_qtd` / `phx_base_tabela_nome` | lista as tabelas, sem alocar |

### 4.3 Esquema (construtor)

Uma tabela precisa de colunas, tipos e índices. Duas formas eram possíveis:
um texto JSON, ou um construtor passo a passo. **Escolhido o construtor.**

Motivo: JSON criaria um *dialeto* — um segundo lugar onde o esquema é
descrito, que envelhece separado do `Schema` e que nenhum compilador confere.
O construtor é chato de escrever e **impossível de errar em silêncio**: tipo
desconhecido é `PHX_ERRO_USO` na hora, não uma coluna faltando descoberta em
produção.

| função | o que faz |
|---|---|
| `phx_esquema_novo` | começa um esquema com nome |
| `phx_esquema_coluna` | acrescenta coluna: tipo, largura (`Str`), precisão/escala (`Decimal`), `PHX_COL_OBRIGATORIA` |
| `phx_esquema_indice` | começa um índice: `PHX_IDX_UNICO`, `PHX_IDX_PRIMARIA` |
| `phx_esquema_indice_coluna` | acrescenta coluna ao índice em construção: `PHX_IDX_DESC`, `PHX_IDX_SEM_CAIXA` |
| `phx_esquema_liberar` | libera |

O esquema **não é consumido** ao criar a tabela: quem criou libera. Um
esquema serve para criar a mesma tabela em vários databases.

### 4.4 Tabela

| função | o que faz |
|---|---|
| `phx_tabela_criar` | cria a partir de um `PhxEsquema` |
| `phx_tabela_abrir` | abre pelo nome (aceita `schema.tabela`) |
| `phx_tabela_fechar` | libera |
| `phx_tabela_registros` | quantas linhas ativas |
| `phx_tabela_colunas` / `phx_tabela_coluna_nome` / `phx_tabela_coluna_tipo` | o esquema de volta |
| `phx_sincronizar` | descarga dos arquivos em disco |
| `phx_verificar` | confere integridade e devolve os contadores |

### 4.5 Dado

| função | o que faz |
|---|---|
| `phx_inserir` | devolve o rowid |
| `phx_atualizar` | por rowid |
| `phx_atualizar_se` | **com versão esperada** — a janela de conflito |
| `phx_versao_da_linha` | a versão atual, para depois passar ao `atualizar_se` |
| `phx_excluir` | exclusão física, com motivo |
| `phx_excluir_suave` | marca (o excluir que volta) |
| `phx_restaurar` | desmarca |
| `phx_ler` | devolve `PhxLinha*`, ou `PHX_NAO_HA` |
| `phx_linha_valores` / `phx_linha_liberar` | a vista e a liberação |
| `phx_buscar` | rowids por chave exata de um índice, em buffer do chamador |

> **`phx_atualizar_se` existe por causa de uma regra da casa:** *guarda nova
> entra pedida, não imposta*. Quem chama `phx_atualizar` continua gravando
> como sempre; quem chama `phx_atualizar_se` ganha a garantia. Num aplicativo
> de celular a janela entre abrir a ficha e tocar em salvar é de minutos —
> é exatamente onde a garantia vale — mas impô-la quebraria todo chamador
> que não a conhece. E o teste que trava isso é o do comportamento **velho**.

### 4.6 Cursor

| função | o que faz |
|---|---|
| `phx_cursor_abrir` | varredura na ordem de digitação, com `PhxVisao` |
| `phx_cursor_abrir_indice` | varredura na ordem de um índice |
| `phx_cursor_proximo` | o próximo rowid; `PHX_NAO_HA` no fim |
| `phx_cursor_liberar` | libera |

**O cursor não guarda ponteiro para a tabela.** `phx_cursor_proximo` recebe os
dois punhos:

```c
phx_cursor_proximo(tabela, cursor, &rowid);
```

Assim um cursor que sobreviva à sua tabela **não pode apontar para memória
morta** — não há para onde apontar. E, para o caso de alguém cruzar os
punhos, o cursor carrega o número de série da tabela que o abriu, e a
chamada devolve `PHX_ERRO_USO`. A alternativa (o cursor guardando o ponteiro)
troca um erro diagnosticável por um uso-depois-de-liberar.

O cursor de digitação lê em **lotes** de rowids por `pagina_depois_de` —
o *keyset* do PhxSql, em que continuar depois do rowid 500.000 é uma conta e
não uma procura. Uma tabela de um milhão de linhas nunca vira um vetor de um
milhão de rowids na memória do celular. O cursor de índice, esse, materializa
a ordem do `.ndx` de uma vez — e isso está dito no cabeçalho, porque é a
diferença que decide quando o aparelho é pequeno.

### 4.7 Replicação — os ganchos

Este é o pedaço que atende ao «se conectar por TCP/IP com o servidor». O
motor **já** tem os dois lados; o que faltava era alcançá-los de fora.

| função | lado | o que faz |
|---|---|---|
| `phx_imagem_no_diario` | escrita | liga a imagem da linha no `.log` — sem ela o diário diz *que* mudou, não *para quê*, e não dá para replicar |
| `phx_diario_qtd` | ambos | quantos eventos já existem; é a **posição** da sincronia |
| `phx_diario_ler` | envio | lê eventos a partir de uma posição, em buffer do chamador |
| `phx_diario_evento_com_imagem` | envio | um evento **com** os bytes que a réplica vai gravar |
| `phx_imagem_bytes` / `phx_imagem_liberar` | envio | a vista e a liberação da imagem |
| `phx_aplicar_evento` | recepção | aplica um evento vindo do outro lado |
| `phx_forcar_proximo_evento` | ambos | carimbo e **origem** do próximo evento — a origem é o que mata o laço infinito do bidirecional |

O contrato de `phx_aplicar_evento` é o do motor, e vale repetir porque é o que
faz a sincronia ser confiável: **o `.reg` nunca reaproveita slot**, então
aplicar todos os eventos na ordem produz rowids **idênticos** aos da origem,
sem negociar nada. Se o rowid gerado não bate com o do evento, a réplica já
divergiu, e a chamada **para** em vez de espalhar — o mesmo comportamento da
thread SQL do MySQL(R).

---

## 5. O que a sincronia do aparelho fica parecendo

Com os ganchos acima, o cliente de sincronia do aplicativo é um laço curto —
e ele mora **no aplicativo**, não na biblioteca, porque quem sabe quando há
rede, quando há bateria e quando o usuário quer gastar dados é o app:

```
   APARELHO (offline)                        SERVIDOR CENTRAL

   phx_inserir ─┐
   phx_atualizar┼─▶ .reg + .ndx + .log        phxsqld
                │        │                       ▲
   (sem rede)   │        │  há rede:             │
                │        ├─ phx_diario_qtd ──────┤  de onde parei
                │        ├─ phx_diario_evento_com_imagem ──▶ enviar
                │        └─ phx_aplicar_evento ◀────────── receber
```

O que **falta** para isso virar produto está na §10 — e falta de verdade.

---

## 6. O que o `phxsql-ffi` deliberadamente **não** faz

- **Não fala SQL.** O `phxsql-sql` existe e poderia ser exposto; não é o
  degrau. Um aplicativo de celular escreve `phx_buscar` num índice, e não
  carrega mais um analisador de SQL. Fica registrado como possível.
- **Não abre porta, não sobe thread, não tem `config.json`.** É o ponto todo.
- **Não mexe na cifra nem na replicação.** Expõe o que já existe.
- **Não gerencia usuários nem permissões.** Dentro do processo do aplicativo
  não há a quem negar: quem chamou já é o dono do processo. Permissão de
  verdade continua sendo do servidor central, do outro lado da rede.

---

## 7. A prova

O código só vale o que a prova mostra. Três camadas:

1. **Testes de unidade em Rust** chamando as funções `extern "C"` como um
   chamador de C chamaria — ponteiros crus e tudo.
2. **Um programa em C** (`crates/phxsql-ffi/c/prova.c`) que liga contra a
   biblioteca de verdade, cria tabela, grava, lê, varre, replica e exercita os
   caminhos de erro. Compilado e **rodado**.
3. **O mesmo programa em ARM64**, sob `qemu-aarch64-static`, pelo caminho que
   o `bancada/arm/provar.sh` abriu — porque *compila* não é *roda*, e essa
   distinção esta casa já pagou uma vez.

```bash
bancada/embutido/provar.sh          # x86-64 e ARM64
```

E as guardas: cada defeito que esta ABI poderia ter entra no
`bancada/guardas/catalogo.py`, com o trecho de hoje e o trecho do estrago,
para que a máquina consiga **repor o defeito** e provar que o teste cai.

---

## 8. Como um app usa, do lado do C

```c
#include "phxsql.h"

PhxBase  *base = NULL;
PhxEsquema *e  = NULL;
PhxTabela *t   = NULL;

phx_base_abrir(PHX_T("/data/data/com.exemplo/files/phx"),
               PHX_T("vendas"), PHX_CRIAR, &base);

phx_esquema_novo(PHX_T("pedidos"), &e);
phx_esquema_coluna(e, PHX_T("id"),      PHX_COL_INT8, 0, 0, 0, PHX_COL_OBRIGATORIA);
phx_esquema_coluna(e, PHX_T("cliente"), PHX_COL_STR, 60, 0, 0, PHX_COL_OBRIGATORIA);
phx_esquema_indice(e, PHX_T("porId"), PHX_IDX_UNICO | PHX_IDX_PRIMARIA);
phx_esquema_indice_coluna(e, 0, 0);
phx_tabela_criar(base, NULL, 0, e, &t);
phx_esquema_liberar(e);

PhxValor linha[2];
linha[0] = phx_int(7);
linha[1] = phx_texto(PHX_T("Adriano Boller"));
uint64_t rowid = 0;
if (phx_inserir(t, linha, 2, &rowid) != PHX_OK) {
    char m[512]; size_t p;
    phx_ultimo_erro(m, sizeof m, &p);
    /* trata */
}

phx_sincronizar(t);
phx_tabela_fechar(t);
phx_base_fechar(base);
```

---

## 9. Medido

### 9.1 O que saiu

| | |
|---|---:|
| funções exportadas (`nm -D`, contadas, não digitadas) | **44** |
| `libphxsql_ffi.so` (cdylib, release) | **1.155.480 B** — 962.664 B depois do `strip` |
| `libphxsql_ffi.a` (staticlib, x86-64) | 9.941.768 B |
| `libphxsql_ffi.a` (staticlib, **aarch64**) | 10.484.230 B |
| o programa em C, ligado **estaticamente** ao motor | **2,7 MB** (x86-64) / 3,0 MB (ARM64) |
| linhas da camada (`src/`, sem os testes) | 2.180 |
| testes da camada | **26**, verdes |
| passos do programa em C | **40**, zero falhas |

Um `.so` de **menos de 1 MB** com o motor inteiro dentro — B+tree, CRC-32,
ChaCha20-Poly1305, SHA-256, JSON, diário, LGPD — é consequência direta da regra
de zero dependências: não há runtime de terceiro para arrastar junto.

### 9.2 As três provas, e o que cada uma pegou

```
cargo test -p phxsql-ffi          26 verdes
bancada/embutido/provar.sh        40 passos x 3 ligações, zero falhas
python3 bancada/guardas/provar-guardas.py --so ffi-...   6 guardas PROVADAS
```

O programa em C roda **três vezes**: contra o `.a` em x86-64, contra o `.so`
em x86-64 (que é o formato do Android) e contra o `.a` em ARM64 sob
`qemu-aarch64-static`. As três dão 40/40.

### 9.3 Os quatro defeitos que a prova achou

Nenhum dos quatro aparecia lendo o código, e três deles só apareceram porque
alguém *rodou* — que é a mesma lição do vídeo da interface, por outro caminho.

**(1) «Não há essa linha» voltava de duas formas.** Achado pelo programa em C
na **primeira rodada dele**. Dentro do motor, um slot livre devolve `Ok(None)`
e um rowid além do fim devolve `NaoEncontrado`. A diferença é real lá dentro e
invisível para quem chama — sem a dobra, o aplicativo mostra caixa vermelha
para metade dos «não achei» e lista vazia para a outra metade, sem nenhum
critério que o programador dele consiga enxergar. O conserto é o
`resultado_do_rowid`, e ele é **só** nas funções endereçadas por rowid: no
`phx_buscar` o mesmo 3001 quer dizer «esse índice não existe», que é defeito de
quem chamou e tem de doer. Guarda `ffi-rowid-fora-e-erro`.

**(2) No ARM64 o `catch_unwind` era enfeite.** Este só apareceu na perna ARM,
e é o mais instrutivo dos quatro:

```
fatal runtime error: failed to initiate panic, error 5, aborting
qemu: uncaught target signal 6 (Aborted)
```

A causa **não estava na biblioteca**: estava na linha de ligação. Chamando o
`ld.lld` na mão, sem `--eh-frame-hdr`, o binário sai sem `PT_GNU_EH_FRAME`, o
desenrolador não acha a tabela de FDE, e **todo `catch_unwind` vira um
aborto**. A garantia central desta camada — nenhum pânico atravessa — some
calada por causa de uma bandeira do ligador, e o sintoma é o app do cliente
fechando sozinho.

O `cc` e o `clang` passam essa bandeira sozinhos. Quem chama o ligador na mão
— e um projeto de iOS ou de Android com um script de ligação próprio pode
acabar chamando — **tem de passar**. É o item mais importante desta seção para
quem for fazer a §10.1 e a §10.2.

**(3) «Quantas linhas tem a tabela» tinha três respostas e a ABI dava uma.**
O `phx_tabela_registros` chamava `Table::registros()`, que conta slots
ocupados, e não `contar(visão)`. Com exclusão suave a tela diria 2 e listaria
1. A visão virou parâmetro: perguntar «quantas» sem dizer «de quê» é a
pergunta incompleta.

**(4) Um teste que passava por engano.** O executor de guardas devolveu
`NAO PEGOU` no `ffi-erro-global`: com a vaga de erro global — o defeito — o
teste `ultimo_erro_e_por_thread` **continuava passando**, porque os outros
testes rodam em paralelo e o `limpar()` de qualquer um deles esvaziava a vaga
global bem a tempo. A casa considera teste que passa por engano pior que teste
que falta. O conserto foi trocar «a outra thread vê vazio» por uma **ordem
estrita**: A escreve, B escreve depois, e A tem de continuar lendo o de A.

E um quinto achado, do mesmo executor, que corrigiu a *entrada do catálogo* e
não o código: o truncamento no byte zero não derruba só o memo do usuário —
derruba a **replicação inteira**, porque a imagem de um evento é payload cru
do `.reg`, cheio de bytes zero, e ela entra pelo mesmo `bytes()`. A entrada
listava a replicação em `seguem`; medido, ela é `caem`.

### 9.4 O que continua sem medida

O **desempenho num aparelho de verdade**. O que se mede sob `qemu-user` é o
custo da emulação, que traduz instrução por instrução; e o que se mede em
x86-64 é outra máquina. Medir isso continua exigindo o aparelho — é a mesma
fronteira honesta da §7.3 do `docs/EMPACOTAMENTO.md`.

---

## 10. O que falta, com o motivo

Esta rodada entrega **a ABI de C e a prova de que ela roda**. Não entrega o
aplicativo, e a diferença é grande. O que falta, na ordem em que faria sentido
fazer:

### 10.1 A camada JNI para Android — só o desenho

Não existe código. O que ela precisa ser:

- Uma classe `br.com.phxsql.Phx` com métodos `native`, e um
  `System.loadLibrary("phxsql_ffi")` no bloco estático.
- Um `phxsql_jni.c` (ou um segundo crate `phxsql-jni`) com as funções
  `Java_br_com_phxsql_Phx_abrir`, que **convertem** entre os tipos do Java e a
  ABI desta rodada. É conversão, não lógica: `GetByteArrayElements` para o
  par `(ponteiro, tamanho)`; `NewStringUTF` **não** serve, porque ele é
  `NUL`-terminado e é exatamente o problema da §3.5 — o certo é
  `NewByteArray` mais o tamanho.
- Um `build.gradle` com `abiFilters` para `arm64-v8a` e `x86_64` (o
  emulador), e o `cargo-ndk` ou o NDK direto no `PATH`.
- O erro do C vira **exceção** do Java, porque código de retorno em Java
  ninguém confere.
- E o item que a §9.3 (2) achou: se o `build.gradle` ou um script próprio
  chamar o ligador na mão, **`--eh-frame-hdr` tem de estar lá**. Sem ele todo
  `catch_unwind` vira aborto, e a garantia mais importante desta camada some
  sem um aviso sequer.

**Por que não foi feito agora:** o NDK não está nesta máquina — o alvo
`aarch64-linux-android` compila e falha no ligador por não achar o bionic
(`docs/EMPACOTAMENTO.md` §7.5). Escrever JNI sem poder ligar nem rodar seria
entregar código nunca executado, e esta casa já escreveu que *«compila» não é
«rodou»*.

### 10.2 A camada Swift/Objective-C para iOS — só o desenho

Também não existe código. O que ela precisa ser:

- Um `module.modulemap` apontando para o `phxsql.h` desta rodada, e o
  `libphxsql_ffi.a` (num `.xcframework` com `arm64-ios` e
  `arm64-ios-simulator`).
- Um invólucro em Swift que transforme o par `(ponteiro, tamanho)` em `Data`
  e o código de retorno em `throws`.
- O caminho dos arquivos tem de ser o diretório do próprio app; e a pasta
  precisa da marca de **não fazer cópia no iCloud** se o dado for cache, senão
  a Apple reprova na revisão.
- Atenção à proteção de dados: com o aparelho bloqueado, o sistema pode
  **negar leitura de arquivo**. Um banco embutido que grava em segundo plano
  precisa da classe de proteção certa, e isso é decisão de produto.
- E o mesmo alerta do ligador: o Xcode passa as bandeiras de desenrolamento
  sozinho, mas uma fase de ligação customizada pode não passar. Ver §9.3 (2).

**Por que não foi feito agora:** o alvo `aarch64-apple-ios` exige o SDK da
Apple e o Xcode, que só existem em macOS. Não dá nem para tentar aqui.

### 10.3 O cliente de sincronia

Os ganchos existem (§4.7); o **laço** que decide quando sincronizar, o que
fazer com conflito de duas pontas e como retomar de queda de rede é trabalho
de outra rodada — e é onde mora a decisão de produto, não a de motor.

### 10.4 O que continua verdade, e não vira promessa

- Não há transação, então **não** se escreve *ACID compliant* aqui tampouco.
- O desempenho numa placa ou num aparelho de verdade continua sem medida: o
  que se mede sob `qemu-user` é o custo da emulação, e a emulação traduz
  instrução por instrução. **Medir isso continua exigindo o aparelho.**
