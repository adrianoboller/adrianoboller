# N) telas das configurações gerais e do banco de dados atual

Corrida em **2026-09-07 16:22–16:35 UTC**, commit `a56a165` (branch
`claude/capacidades-disponiveis-y6auxh`), contra `target/release/phxsqld`
(binário não compilado por esta frente).

## Resposta curta

Existem as duas telas, e as duas são as mesmas do menu **Configurações**
(`tela.mi_gerais_servidor` → `verConfigServidor()`, `tela.mi_do_banco` →
`verConfigBanco()`). Não são abas de uma janela com abas: são páginas de
rolagem única, e dentro da primeira o servidor divide o formulário em **10
seções editáveis** — todas cheias, porque as **49** entradas de
`CAMPOS_EDITAVEIS` (`config.rs`) cabem inteiras nos 10 grupos que
`GRUPOS_AJUSTE` declara; não sobrou nenhum campo para o grupo residual
"Outros ajustes". Depois delas vem uma 11ª seção, só de leitura: **"Só pelo
arquivo — e por quê"**, com o campo, o valor e o motivo de cada um não se
gravar pela web. O número que decide: **49 campos editáveis, 10 seções
graváveis + 1 somente-leitura, e um segredo — nenhum.**

## Exemplo exercitado

### 1. As 11 seções da tela "Gerais do servidor", capturadas nos dois temas

Descoberta ao vivo, lendo `<h3 class="secao">` filho de `#painel` depois de
`verConfigServidor()` — não uma lista digitada à mão:

```
Rede e limites
Gravação e durabilidade
Transações
Memória e CPU
Backup
Arquivo do Profiler
Webservice REST
Tabelas declaradas cifradas
Alerta de espaço em disco
Cores do painel de telemetria
Só pelo arquivo — e por quê
```

Cada uma virou duas capturas (1440×900, claro e escuro) em
`docs/dossie/capturas/config-<secao>-<tema>.png`:

**Rede e limites** — `bind`, `timeout_s`, `recursos.conexoes_max`,
`max_linhas`, `recursos.usuarios_max`, `web.sessao_minutos`.

![claro](../dossie/capturas/config-rede-e-limites-claro.png)
![escuro](../dossie/capturas/config-rede-e-limites-escuro.png)

**Gravação e durabilidade** — `recursos.durabilidade`,
`recursos.exclusao_na_janela`, `recursos.lote_operacoes`,
`recursos.lote_milissegundos`, `recursos.diario_volume_mib`,
`recursos.carga_prazo_min`, `espelho`, `somente_leitura`.

![claro](../dossie/capturas/config-gravacao-e-durabilidade-claro.png)
![escuro](../dossie/capturas/config-gravacao-e-durabilidade-escuro.png)

**Transações** — `recursos.transacao_prazo_min`,
`recursos.transacao_max_linhas`, `recursos.transacao_lock_timeout_ms`,
`recursos.transacao_statement_ms`.

![claro](../dossie/capturas/config-transacoes-claro.png)
![escuro](../dossie/capturas/config-transacoes-escuro.png)

**Memória e CPU** — `recursos.cache_paginas`, `recursos.memoria_max_mb`,
`recursos.threads`, `recursos.cpu_percentual`.

![claro](../dossie/capturas/config-memoria-e-cpu-claro.png)
![escuro](../dossie/capturas/config-memoria-e-cpu-escuro.png)

**Backup** — `backup.agendado`, `backup.hora`, `backup.cada_horas`,
`backup.destino`, `backup.zip`, `backup.manter`.

![claro](../dossie/capturas/config-backup-claro.png)
![escuro](../dossie/capturas/config-backup-escuro.png)

**Arquivo do Profiler** — `profiler.arquivo_mib`, `profiler.arquivos`.

![claro](../dossie/capturas/config-arquivo-do-profiler-claro.png)
![escuro](../dossie/capturas/config-arquivo-do-profiler-escuro.png)

**Webservice REST** — `rest.ligado`, `rest.bind`, `rest.nome`,
`rest.database`, `rest.tabelas`, `rest.swagger_ligado`, `rest.swagger_bind`.
O rodapé (`rodapeDoRest`) diz se este servidor **tem** `rest.token` próprio
— nunca qual é.

![claro](../dossie/capturas/config-webservice-rest-claro.png)
![escuro](../dossie/capturas/config-webservice-rest-escuro.png)

**Tabelas declaradas cifradas** — só `cifra.tabelas` (o resto da seção
`cifra` — senha, iterações, modo, salto, separador — não se edita pela
tela, de propósito).

![claro](../dossie/capturas/config-tabelas-declaradas-cifradas-claro.png)
![escuro](../dossie/capturas/config-tabelas-declaradas-cifradas-escuro.png)

**Alerta de espaço em disco** — `alertas.ligado`,
`alertas.livre_minimo_percentual`, `alertas.livre_minimo_mb`,
`alertas.checar_minutos`, `alertas.repetir_horas`.

![claro](../dossie/capturas/config-alerta-de-espaco-em-disco-claro.png)
![escuro](../dossie/capturas/config-alerta-de-espaco-em-disco-escuro.png)

**Cores do painel de telemetria** — `telemetria.cor_normal`,
`telemetria.cor_alto`, `telemetria.cor_stress`,
`telemetria.cor_encerrando`, `telemetria.alto_uso_ms`,
`telemetria.stress_ms`, com o contraste do rótulo medido ao vivo (aqui,
7,30:1 / 11,91:1 / 6,36:1 / 9,01:1 no tema escuro) e o botão "Voltar às
cores de fábrica".

![claro](../dossie/capturas/config-cores-do-painel-de-telemetria-claro.png)
![escuro](../dossie/capturas/config-cores-do-painel-de-telemetria-escuro.png)

**Só pelo arquivo — e por quê** — a 11ª seção, **somente leitura**: uma
tabela campo → valor agora → para que serve, cobrindo `token`, `base`,
`log_acessos`, `dblink`, `jobs`, `ips_permitidos`, `web.ligado`,
`web.bind`, `web.servidores`, `seguranca.*` inteiro, `cifra.ligada`,
`cifra.modo`, `cifra.salto`, `cifra.separador`, `cifra.iteracoes`,
`cifra.senha`, `alertas.caminhos`, `alertas.email.*` e `replicacao.*`.

![claro](../dossie/capturas/config-so-pelo-arquivo-e-por-que-claro.png)
![escuro](../dossie/capturas/config-so-pelo-arquivo-e-por-que-escuro.png)

### 2. A tela "Do banco atual" (`verConfigBanco`), banco `Comercial`, 1 tabela

Cenário: banco `Comercial`, tabela `clientes` (5 colunas, 2 índices, 3
linhas — uma delas "Blumenau", de propósito, contra a armadilha do
`text-transform:uppercase`).

![claro](../dossie/capturas/banco-atual-claro.png)
![escuro](../dossie/capturas/banco-atual-escuro.png)

Ela tem duas seções próprias (não vêm de `CAMPOS_EDITAVEIS` — são fichas e
tabelas de leitura, montadas na hora com `tabelas`/`sistabelas`/`config`):
**"Onde ele mora"** (`base`, a pasta do database, os schemas) e **"O que
herda do servidor"** (`espelho`, `somente_leitura`, `max_linhas`,
`backup.destino` — herdados, e a própria tela diz "a tela de Configurações
gerais mostra onde mexer"). Ela é **só leitura**: não há campo, nem botão de
salvar — só o atalho "← Gerir {banco}".

Nota lateral: existe uma tela IRMÃ, "Gestão de tabelas" (`gerirTabelasAtual`,
menu Tabelas → Gerir), que é a grade de tabelas com registros/slots/colunas
do banco — já capturada no dossiê (`docs/dossie/capturas/tabelas-*.png`).
Ela não é a mesma tela que este item N) pede: o item nomeia
"configurações... do banco de dados atual", que é `verConfigBanco`, atrás do
mesmo menu "Configurações" da tela gerais.

### 3. Exercício — salvar um valor, recarregar a página, provar que persistiu

Comando: mudar `max_linhas` de 1000 para 1111 pelo campo, clicar
"Salvar no config.json", dar `page.reload()` **de verdade** (não um
redesenho em JS), logar de novo, reabrir "Gerais do servidor" e ler o
campo — e ler o `config.json` em disco por fora da tela.

Saída real do script (`node testes-web/capturas-config.mjs`, mesma corrida):

```
exercicio salvar/recarregar: antes=1000 novo=1111 depois-do-F5=1111 config.json=1111
```

O aviso que a tela mostrou ao salvar (`#aviso`, capturado no mesmo instante):

```
1 campo(s) gravado(s) em /tmp/phx-f8-17959-KO3C90/config.json
```

Os três valores batem: **1111** no campo antes do reload, **1111** depois
do F5, **1111** no arquivo. É exatamente a garantia do `SEGURANCA.md` §9 —
gravação atômica, campo mostra o valor gravado — e aqui ela foi **provada**,
não só lida no documento.

### 4. Exercício — segredo não volta em texto puro

Comando: subir o servidor com `alertas.email.senha` e `cifra.senha`
apontando para dois marcadores só deste teste
(`MARCA-SEGREDO-RELE-8f2c`, `MARCA-SEGREDO-COFRE-9a1d` — nunca usados em
lugar nenhum do produto), entrar na tela, e procurar as duas strings em
**dois lugares**: no `outerHTML` inteiro do documento depois de
`verConfigServidor()`, e no JSON bruto que a operação `config` devolve pelo
protocolo (não só o que a tela escolhe mostrar).

Saída real:

```
exercicio segredo: senha no JSON de config=false  senha no DOM=false  alertas.email.senha="(oculta)"  cifra.senha="(oculta)"  "cluster" aparece em config()=false
```

Nenhuma das duas strings-marcadoras apareceu em nenhum dos dois lugares.
`alertas.email.senha` e `cifra.senha` **existem** como campos no JSON — mas
carregam sempre um dos três rótulos de estado (`"(oculta)"`, `"(vazia)"`,
`"(do ambiente)"`), nunca o valor, nunca uma máscara do tamanho certo (o
próprio comentário do `config.rs` diz por quê: *"o tamanho já é
informação"*). É a mesma regra do `token` de nível de servidor, que sempre
volta `"(oculto)"` (`config.rs:2908`).

De quebra, a mesma checagem provou uma dispensa mais funda: com
`cluster` configurado no arquivo (2 nós, `replicacao.papel: "source"`),
a chave `"cluster"` **não aparece em lugar nenhum** da resposta de `config`
— nem oculta, nem ausente por engano: `Config::para_json` simplesmente não
a escreve. Ver seção seguinte.

## O que NÃO existe, e é dispensa registrada

- **Não há abas de navegador dentro da tela.** As duas telas são páginas de
  rolagem única; "seção" aqui quer dizer `<h3 class="secao">` dentro do
  mesmo `#painel`, não uma aba clicável separada. Nenhuma das duas rola a
  *página* — quem rola é o próprio `#painel` (por isso a captura de cada
  seção precisou rolar o painel e medir depois, não tirar uma foto de
  página inteira).

- **"Segurança" não é uma seção editável — é dispensa por decisão, com o
  motivo escrito na própria 11ª seção.** `seguranca.comandos_proibidos`,
  `seguranca.bases_proibidas`, `seguranca.tentativas_ate_bloquear`,
  `seguranca.janela_minutos`, `seguranca.bloqueio_minutos`,
  `seguranca.blacklist`, `seguranca.firewall` só se leem, nunca se gravam
  pela web: uma sessão roubada não pode abrir o firewall nem esvaziar a
  lista de comandos proibidos (`SEGURANCA.md` §9, tabela "fica de fora").

- **"Replicação" não é uma seção editável, pela mesma razão.**
  `replicacao.papel`, `replicacao.envio`, `replicacao.retorno`,
  `replicacao.imagem_da_linha` aparecem só na 11ª seção: virar este
  servidor para outro `source` pela web é o exemplo que o próprio
  `SEGURANCA.md` usa para explicar a régua inteira.

- **"Cluster" não existe em NENHUMA das duas telas — nem para editar, nem
  para ler.** É o achado que ler o código não bastaria para confirmar
  sozinho: `grupoDeAjustes(...)` (a tabela somente-leitura) lista `web.*`,
  `seguranca.*`, `cifra.*`, `alertas.*`, `replicacao.*` e `usuarios`, mas
  nenhuma linha de `cluster.*`. O exercício do item 4 confirmou ao vivo que
  a *resposta* de `config` também não traz a chave `"cluster"` quando ela
  está no arquivo — `Config::para_json` (`config.rs:2892-3097`) não a
  serializa. `cluster.token`, `cluster.usuario` e `cluster.senha_hash`
  carregam credencial (a mesma família de `token` e `replicacao.*`), e
  `cluster.nos`/`prioridade`/`janela_s`/`pulso_s`/`avisar_cada_min`/
  `databases`/`email` são política de topologia — mas a ausência aqui não é
  "editável não, leitura sim" como replicação: é ausência total. Quem quer
  ver o cluster vivo usa a tela **Replicação** (`verReplicacao`, que lê
  `papel`/posição do diário) ou `docs/CLUSTER.md` — não esta tela.

- **"E-mail/alertas" tem uma metade editável e uma que não.** O *limiar* de
  disco (`alertas.ligado`, os dois `livre_minimo_*`, `checar_minutos`,
  `repetir_horas`) é uma seção cheia; o **relé de e-mail**
  (`alertas.email.*` inteiro — servidor, porta, de, para, usuário e senha)
  é só leitura na 11ª seção, e a senha nunca aparece (provado no item 4).

- **"Idioma" não é uma seção do `GRUPOS_AJUSTE`.** É um controle à parte no
  topo da tela "Gerais do servidor" (`#idiomasAqui`, `desenharIdiomas`) que
  troca o idioma **deste navegador, na hora** — e a própria tela avisa que
  o campo `idioma` do `config.json` é outro, e manda nas mensagens que o
  *servidor* devolve pelo protocolo, não nesta tela.

- **Usuários e Diretivas de acesso são telas irmãs, não seções desta.** O
  menu Configurações tem `mi_dos_usuarios` e `mi_diretivas_acesso` ao lado
  de `mi_gerais_servidor`/`mi_do_banco` — mas são `verConfigUsuarios()` e
  outra função, com URL de tela própria; o item N) nomeia só "gerais" e "do
  banco atual".

- **Nenhum defeito de tela achado nesta rodada.** As 24 capturas (11 seções
  × 2 temas + banco-atual × 2 temas) saíram legíveis nos dois temas, sem
  campo esticado pelo `input{width:100%}` global, sem rótulo de dado em
  caixa alta, e sem `pageerror` no console em nenhuma das duas passagens.

## Como se refaz

```bash
node testes-web/capturas-config.mjs
```

Sobe um `phxsqld` isolado (portas 6800/6801, dentro da faixa 6800–6819 da
frente F8), cria o banco `Comercial` com a tabela `clientes`, entra pela
tela de login de verdade (não por atalho), fotografa as 11 seções da tela
"Gerais do servidor" e a tela "Do banco atual" nos dois temas em 1440×900,
roda os dois exercícios do item 3 e 4, e derruba o servidor pelo PID — nunca
por `pkill -f`. O script está em `testes-web/capturas-config.mjs`, com o
comentário de cabeçalho.
