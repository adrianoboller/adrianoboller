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

## O próximo passo natural (não entrou nesta rodada)

A tradução da **interface** do console — os milhares de rótulos do
`index.html` — não entrou. O mecanismo ficou pronto para ela: bastaria semear
os rótulos como linhas (`tela.botao_salvar`, …), a página pedir o pacote do
idioma numa op (a tabela já se lê pela `varrer`) e aplicar nos elementos. O
que isso exigiria de verdade é o trabalho editorial: milhares de textos
curtos, onde tradução ruim é pior que inglês nenhum — exatamente a razão de a
célula vazia cair para o português. Traduzir os `{detalhe}` gerados pelo motor
é a outra metade, e essa pede TextName por mensagem do motor, não só a moldura.
