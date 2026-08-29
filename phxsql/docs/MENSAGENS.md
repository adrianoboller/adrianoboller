# As mensagens do servidor — texto por idioma, numa tabela de verdade

O que o servidor **diz** ao cliente virou dado: cada mensagem é uma linha da
tabela `phxsys.mensagens`, com um texto por idioma. O que o servidor **informa
por código** não mudou uma vírgula — `4002 EM_CARGA` é `4002 EM_CARGA` em
qualquer língua, e o cliente que trata pelo código nem percebe a novidade.

## Onde a tabela mora, e por quê

`phxsys` é um **database comum** do próprio motor, criado quando alguém pede.
Essa é a decisão central do desenho, e o motivo é reuso: a grade do Centro de
Controle já edita tabela, o portão de permissão por base já protege quem pode
mexer (restrinja `phxsys` como restringe qualquer base), o diário da tabela já
registra quem mudou o quê, e o backup já a leva junto. A alternativa — um
arquivo próprio tipo `mensagens.json` — exigiria um editor próprio, uma
permissão própria e uma auditoria própria, três mecanismos novos para fazer o
que o motor já faz.

## O esquema

| coluna | tipo | papel |
|---|---|---|
| `id` | `Uuid`, chave primária | FIXO: identidade da linha; nasce v7 na semeadura e não muda |
| `TextName` | `Str(80)`, índice único | FIXO: o nome que a programação busca (`erro.em_carga`, `erro.sem_direito`) |
| `Portugues` | `Str(250)` | o texto de fábrica; o degrau intermediário do fallback |
| `Frances`, `Ingles`, `Italiano`, `Alemao`, `Espanhol` | `Str(250)` | as traduções, editáveis |

Identificadores sem acento, como manda a regra da casa — por isso `Portugues`
e `Ingles`, e não «Português»/«Inglês». Os nomes das colunas são exatamente os
valores aceitos no campo `idioma` do `config.json`.

## A resolução, em três degraus

1. a célula do **idioma configurado** (`"idioma": "Ingles"` no `config.json`);
2. célula vazia → cai para a coluna `Portugues`;
3. linha ausente, tabela ausente, ou português também vazio → o **texto de
   fábrica**, que está em `crates/phxsql-server/src/mensagens.rs` e é byte a
   byte o que o servidor sempre respondeu.

Sem o campo `idioma` e sem a tabela, **nada muda** — o degrau 3 é o
comportamento de sempre, e há teste que compara com o `Display` de cada
variante de erro, byte a byte. Guarda nova entra pedida, não imposta.

Idioma desconhecido no config não derruba o servidor: vira AVISO no arranque
(o mesmo padrão do campo com nome errado) e cai em português.

## O que passa pela tabela

Todas as mensagens que o **servidor** devolve pelo protocolo, em duas camadas:

- as **molduras** dos doze erros (`erro.corrompido` … `erro.erro_de_es`) — o
  prefixo do `Display`, com `{detalhe}` no lugar da parte variável;
- os **textos dos portões**, criados por inteiro pelo servidor:
  `erro.token_invalido`, `erro.credencial_invalida`, `erro.faca_login`,
  `erro.sem_direito`, `erro.somente_leitura`, `erro.comando_proibido`,
  `erro.base_proibida`, `erro.nome_hostil`, `erro.grave_bloqueado`,
  `erro.grave_tentativa`, `erro.ip_bloqueado`, `erro.ip_nao_autorizado`,
  `erro.operacao_desconhecida`.

Os marcadores `{assim}` são posicionais por nome — a tradução pode
reordená-los. Célula vazia **não** é semeada com tradução inventada: cai para
o português, que é correto e honesto. O `{detalhe}` (o que cada `format!` do
motor escreve) continua no idioma do motor; traduzir o miolo de cada mensagem
do motor é outra rodada.

**O que não muda de propósito:** o `acessos.log` grava o texto de fábrica do
`Display`, sempre — filtro de log (fail2ban, §5.1 do
[SEGURANCA.md](SEGURANCA.md)) não pode quebrar porque alguém trocou o idioma.
A exceção inevitável: mensagens que os portões criam por inteiro entram no
log já resolvidas; o filtro certo casa `"ok":false` e o `codigo`, não o texto.

## Semeadura

`{"op":"mensagens_semear"}` (exige `administrar`) cria o database, a tabela e
grava uma linha por mensagem de fábrica **que ainda não existe** — idempotente
por `TextName`: semear de novo nunca desfaz a tradução de ninguém. Com
`idioma` preenchido no config, a semeadura acontece sozinha no primeiro
arranque; sem o campo, nada nasce até alguém clicar em «Semear» na tela.

A fábrica já traz o inglês (e francês, italiano, alemão e espanhol nas
mensagens curtas o bastante para tradução segura); o que não veio traduzido
ficou vazio de propósito.

## Editar aplica sem reiniciar

O servidor guarda a tabela num cache em memória e confere o `mtime` do `.reg`
no máximo a cada 2 s (`INTERVALO_DE_CONFERENCIA` — o mesmo desenho do
`recarregar_se_mudou` da blacklist). Editou pela grade, pelo SQL ou por outro
processo: vale em poucos segundos, sem reiniciar. Trocar o **idioma** é config
e exige reinício — a tela diz isso com todas as letras.

Custo no caminho quente: **zero no sucesso** (mensagem só existe em resposta
de erro), um `HashMap` no erro, um `stat` a cada 2 s no pior caso. O portão
que decide vem antes do trabalho.

## A tela

Configurações → **Mensagens do servidor…**: mostra o idioma em uso e o estado
da tabela, semeia, e abre a **grade comum** de `phxsys.mensagens` — nenhum
editor novo. Na ficha, `id` e `TextName` aparecem travados (`readonly`, com o
motivo no `title`); na inclusão continuam editáveis, porque é ali que nascem.

## O que os testes provam, e a prova real

Em `mensagens.rs` e `servidor.rs` (`testes_firewall_e_mensagens`):

| o que se prova | como |
|---|---|
| sem tabela, byte a byte o texto de sempre | `texto_do_erro == Display` para as doze variantes |
| (g) idioma troca o texto e nunca o código | `Ingles` + tabela semeada: texto da coluna, `codigo/nome/repetir` idênticos |
| (h) célula vazia cai para o português | variante sem tradução de fábrica volta em português |
| (i) linha excluída volta à fábrica | `excluir` a linha pela op comum e comparar byte a byte |
| (j) sem `idioma`, português de sempre | segundo servidor na mesma base semeada |
| (k) editar vale sem reiniciar | `atualizar` pela op comum, esperar o intervalo de conferência, texto novo |
| semeadura idempotente | segunda chamada semeia 0 e não toca nada |
| sem config e sem tabela, `phxsys` não nasce | arranque sem `idioma`: o diretório não existe |

**Prova real, com o defeito reposto:** trocando o fallback de célula vazia por
«devolve a célula como está», caíram três testes — e o
`celula_vazia_nunca_vira_texto_vazio` mostra exatamente o estrago que se quer
impedir: o cliente receberia texto **vazio**, pior que sem tradução nenhuma.

## Os textos da TELA — a fábrica, o conferidor e a catraca

O que o servidor **diz** virou dado na 0.17.0; o que a tela **mostra** entrou
depois, na mesma tabela e com o mesmo desenho: `TextName` começando com
`tela.` em vez de `erro.`, a fábrica em `crates/phxsql-server/src/idiomas.rs`,
e os mesmos três degraus de resolução. Sem tabela e sem escolha, a tela é a de
sempre, em português — guarda nova entra pedida.

### O buraco, medido

A máquina funcionava e quase nada passava por ela. Medido antes desta rodada:
**11.987 linhas de interface e 16 `data-txt`**. O laço que existia olhava para
um lado só (todo `data-txt` tem de existir na fábrica), e esse pega o nome
escrito errado, não o buraco.

| | antes | depois |
|---|---|---|
| textos na fábrica | 31 | 258 |
| textos cravados em português | 2.185 | 1.994 |
| cobertura | 1% | 11% |
| chaves na `FABRICA_TELA` | 32 | 199 |

Os números saem do conferidor, não da mão:

```bash
cargo run --example textos-fora-da-fabrica -p phxsql-server            # o placar
cargo run --example textos-fora-da-fabrica -p phxsql-server -- --tudo  # arquivo e linha de cada um
cargo run --example textos-fora-da-fabrica -p phxsql-server -- --isentos
```

### Como acrescentar um texto novo

1. **A chave e as seis traduções**, uma linha em `FABRICA_TELA`
   (`crates/phxsql-server/src/idiomas.rs`), na ordem Português, Francês,
   Inglês, Italiano, Alemão, Espanhol:

   ```rust
   texto!("tela.mi_exportar", "Exportar…", "Exporter…", "Export…", "Esporta…", "Exportieren…", "Exportar…"),
   ```

   O português nunca é vazio: ele é o degrau 2. Célula que você não sabe
   traduzir fica **vazia**, e cai no português — melhor nenhuma tradução que
   uma inventada.

2. **O uso, na tela**, numa das quatro formas — e sempre com o português de
   fábrica ao lado, que é o que aparece antes de o pacote de idioma chegar:

   | onde | forma |
   |---|---|
   | HTML estático | `<button data-txt="tela.fechar">Fechar</button>` |
   | atributo que se lê | `data-txt-ph=`, `data-txt-tt=` (title), `data-txt-al=` (aria-label) |
   | HTML montado em JS | `` `<h3>${esc(txt("tela.painel", "Painel"))}</h3>` `` |
   | tabela lida antes do login (`MENUS`, `FERRAMENTAS`) | `{ rot:"Painel", txt:"tela.painel" }` — o par, **na mesma linha** |

   O par existe porque `MENUS` e `FERRAMENTAS` são lidos no arranque, quando
   ainda não há texto traduzido nenhum: `txt(…)` ali devolveria português para
   sempre. Quem desenha chama `txt(f.txt, f.rot)` na hora de pintar.

3. **Rode os portões.** `cargo test --workspace` já cobre os três laços.

### O que o conferidor reprova

| teste | reprova |
|---|---|
| `conferidor::a_catraca_dos_textos_fora_da_fabrica` | texto de tela cravado **a mais** que o `TETO` — e também traduzir sem baixar a catraca |
| `idiomas::todo_data_txt_da_pagina_existe_na_fabrica` | chave que a tela pede e a fábrica não tem (a tela ficaria em português para sempre) |
| `idiomas::todo_texto_da_fabrica_e_pedido_por_alguem` | chave morta: traduzida nos seis idiomas e pedida por ninguém |
| `idiomas::a_fabrica_e_bem_formada` | nome repetido, português vazio, ou texto que não cabe nos 250 da coluna |

O `TETO` **só desce**. Traduziu um punhado: rode o exemplo, veja o número novo
e baixe a catraca no mesmo commit — catraca frouxa não segura nada.

### O que o conferidor enxerga, e o que não

Duas vias, porque a interface escreve texto de dois jeitos: **marcação** (o
texto entre `>` e `<` de uma etiqueta conhecida, mais `title`, `placeholder`,
`aria-label` e `alt`) e **rótulo** (o literal em posição de rótulo no
JavaScript: `rot:`, `{t:`, `diz:`, `dica:`, e o primeiro argumento de
`avisar(`, `confirm(`, `prompt(` e `folha(`).

Fora dessas formas ele não vê — por exemplo o segundo item de um par solto
`["registros", e.registros]`. Está declarado no `RECEITAS`: forma nova de
rótulo entra lá, e o número **sobe**. Subir o número é o conferidor
funcionando, não falhando.

**Dado não vira rótulo, e não é por lista: é por forma.** Antes de varrer, todo
`${…}` some e vira um marcador. O que a página interpola (o dado do banco)
desaparece; o que sobra é o que alguém digitou no fonte, que é a definição de
rótulo. É a lição do «Blumenau» virando «BLUMENAU» virada crivo.

Há uma terceira lista, a dos **isentos**: nome próprio, sigla, extensão de
arquivo e identificador que a pessoa digita em outro lugar (`config.json`,
`.reg`, `PK`, `rownum`, `PhxSql`). Cada um com a razão escrita. Rótulo que
apenas ainda não foi traduzido **não entra ali** — ele fica na conta do que
falta, e é essa conta que dirige a próxima leva.

### Trocar o idioma, pelos dois lados

Pelo **login** (as bandeiras, antes de entrar) e pela **tela de configuração**
— Configurações → Gerais do servidor, e Configurações → Idiomas da interface.
Os dois seletores são desenhados pela mesma função: um segundo seletor com a
sua própria lista de idiomas seria a segunda verdade, e é sempre a segunda que
envelhece.

A troca vale **na hora**, sem recarregar: `aplicarIdioma` repinta os quatro
atributos, o cromo (menu, barra, abas, árvore) e a tela aberta, através do
gancho `est.repintar` — que `folha()` limpa e a tela que sabe se redesenhar
repõe. Sem esse gancho, trocar o idioma na tela de Idiomas jogaria a pessoa no
Painel. A escolha fica no `localStorage` e atravessa o login e o sair.

### A prova real, exercitando

`node phxsql/testes-web/prova-idiomas.mjs --capturas <dir>` sobe um `phxsqld`
próprio (portas 6650/6651), dirige o Chromium e prova sete coisas, entre elas
o **comportamento velho** (sem escolher nada, a tela é a de sempre) e o texto
que **estica**: o alemão é ~30% mais longo que o português, e o passo 6 falha
se qualquer rótulo da barra de ferramentas ficar cortado ou se a página passar
a rolar de lado.

Foi ela que achou o defeito desta rodada que ler o código não acharia: o
`aplicarTema` do arranque passou a pedir um texto, e o `txt` era `const` — a
página morria com «Cannot access 'txt' before initialization», e o estrago
aparecia longe da causa (o botão de tema ficava sem `onclick`, porque a linha
que o liga vem depois da que estourou). Declaração de função sobe; `const`
não.

## O que falta, com precisão

Traduzido 100%: a tela de **entrada**, o **cromo** inteiro (barra do alto,
barra de menu com os nove menus e os setenta e três itens, barra de
ferramentas com os trinta e um botões, as cinco abas, a árvore e o painel
lateral), a tela de **Idiomas**, e da tela de **Configurações gerais** o
título, o subtítulo, os botões e a linha do idioma.

Falta o **miolo das telas**: 1.994 textos, com arquivo e linha no
`--tudo`. Por ordem do que a pessoa mais vê, a próxima leva é: os 99 títulos e
subtítulos de `folha(`, os 136 cabeçalhos de coluna distintos (`{t:"…"}`), os
39 recados de `avisar(` e as 19 perguntas de `confirm(`. Depois vêm os corpos
de texto longo — telemetria, LGPD, replicação, a tela da Claude — onde
tradução ruim é pior que português honesto, e por isso a célula vazia cai para
o português.

Traduzir os `{detalhe}` que o motor gera continua sendo outra metade, e essa
pede `TextName` por mensagem do motor, não só a moldura.
