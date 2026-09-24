<!-- Parecer do papel C (DBA), 24/09/2026, somente leitura, sobre a frente 372
antes do commit. Guardado verbatim pelo integrador. -->

## Parecer do papel C, pedido 372 (formato 2 do `dblink.json`): somente leitura

Rodei `cargo test -p phxsql-server --lib dblink`: **91 verdes, 0 vermelhos**. O parecer anterior (`docs/propostas/parecer-dba-372-e-255.md` §1.2–§1.5) foi **honrado nos 11 itens**: `formato: 2`, material único, envelope por ligação com nonce próprio, exclusão mútua, AAD com nome e campo, chave de fora recusada dentro das três pastas pelo caminho real, compatibilidade para trás, migração pedida e dita, chave ausente sem derrubar o arranque, downgrade e claro residual escritos. O contrato do 378 (`cifra`/`chave_do_fio` só vão ao disco quando divergem, e a herança do pino) continua intacto em `dblink/mod.rs:485-501`.

**Referências a §19/§20: certas.** Nenhum documento nem o código apontava para a antiga §19. Só as três novas usam §19 (`DBLINK.md:190`, `MANUAL.txt:3848`, `CHANGELOG.md:22`), e nenhum gerador numera as seções do FORMATO.

### 1. Compatibilidade: CONDICIONAL
- **Formato 1 abre e regrava igual: SIM.** Sem `formato`, com `formato: 1` ou com lista crua (`:1223-1241`). O teste está em `:2845-2877`.
- **Cadastro sem chave não ganha `formato`: SIM.** `escrito` só nasce de duas fontes: do arquivo, ou de `chave.de_fora()` com credencial para selar (`:1471-1485`). Com `None` sai só `{"dblink":[…]}` (`:1510-1513`).
- **Formato maior que 2 é recusado: SIM**, com `VERSAO_NAO_SUPORTADA` (`:1226-1231`). Mas a recusa sobe por `servidor.rs:1285` com `?`, e aí **o motor inteiro não sobe**. Isso não está escrito em lugar nenhum (CHANGELOG, MANUAL, FORMATO §19).
- **Binário antigo com formato 2: DEFEITO DE TEXTO.** Ele não só lê a senha vazia; **a primeira gravação dele apaga os envelopes de todas as ligações.**
  - O `abrir` do HEAD (`:757-784`) só lê `dblink` e descarta `formato`, `cifra_do_cadastro` e `*_cifrada`.
  - O `para_disco` do HEAD (`:383-387`) regrava `"senha": ""`.
  - Qualquer `dblink_salvar`, `dblink_excluir` ou `dblink_ligar` no binário velho basta.
  - De volta ao novo, o arquivo é formato 1 com senha vazia. O aviso do texto puro não dispara, porque o vazio não conta como escrito (`config.rs:1455-1457`), e a ligação conecta com senha vazia calada.
  - Os quatro documentos dizem só «lê vazia»: `FORMATO.md:2857`, `DBLINK.md:185`, `CHANGELOG.md:58`, MANUAL 15.8.

### 2. Atomicidade: SIM, com um furo que já existe na casa inteira
- `gravar_privado` (`config.rs:2661-2676`): apaga o `.tmp` velho, cria com `create_new` e 0600, `write_all`, `sync_all`, `rename`. Numa queda sobra o arquivo antigo (claro, formato 1) ou o novo inteiro, **nunca meio a meio**. O `.tmp` que sobrar tem o conteúdo novo, já selado, e a próxima gravação o apaga.
- **A memória só muda depois que o disco aceitou: SIM.** `salvar`/`excluir` trabalham numa cópia (`:1416-1437`), e a troca acontece em `:1520-1556`.
- **Furo:** falta o `fsync` do diretório depois do `rename`, e não há esse `fsync` em lugar nenhum do repositório. Queda logo depois do `rename` pode trazer de volta o arquivo em claro depois de a resposta dizer `ligacoes_cifradas_agora: N`. O estrago é limitado: o aviso de arranque volta a acusar o texto puro, e a próxima gravação migra de novo.

### 3. Migração: todas as ligações, SIM; arquivo misto é defeito, não estado legítimo
- A primeira gravação sela **todas** as ligações, não só a tocada (`:1470`, `:1506-1509`). A conta só inclui quem estava em claro no disco (`:1529-1538`). Teste em `:2883-2933`: 2 de 2, zero ocorrência do claro no disco.
- **Estados legítimos no formato 2:** selado, `_env`, `"senha": ""` e envelope trancado devolvido igual (`:450-452`).
- **O código nunca produz claro não vazio ao lado do material:** ou sela (`:463-467`), ou recusa (`:468-475`, `:1486-1505`).
- **O misto só nasce editando à mão.** Em campos diferentes, o arranque avisa e a próxima gravação sela. No mesmo campo, a abertura recusa (`:883-891`), e isso também derruba o motor por `servidor.rs:1285`, a mesma política que já vale para arquivo torto.

### 4. Sem troca de chave: ACEITÁVEL como pedido novo, não é meia funcionalidade
Tradeoff em uma linha: sem recifrar, trocar a chave custa redigitar as credenciais — e a troca por vazamento exige isso de qualquer jeito, porque a cópia vazada abre com a chave vazada; só a rotação de rotina fica sem caminho.

Achado: **com a chave perdida, a tela não tem saída.** O material fica no arquivo mesmo com zero ligações (`:1471`), e toda credencial nova em claro é recusada (`:1486-1505`). Só sobram `_env` ou apagar `cifra_do_cadastro` à mão. O §19 (`FORMATO.md:2860-2866`) diz «não há caminho sem editar o arquivo», mas não diz **qual** edição.

### 5. AAD e renomear: SIM, seguro hoje
- **Não existe renomear no produto.** O nome fica desabilitado na edição (`ui/index.html:10912`, `:10960`), e o protocolo substitui pelo nome sem distinguir caixa (`servidor.rs:22831`, `dblink/mod.rs:1418-1424`).
- Trocar só a caixa é seguro, porque o AAD vai em minúsculas (`:913`).
- Renomear à mão tranca aquele envelope, com o motivo dito (`:960-968`). O envelope volta ao disco igual, então desfazer o nome o abre de novo, e redigitar a credencial pela tela a sela de novo. Nada se perde calado.
- Guarda para o futuro: um `renomear` que venha a existir tem de abrir e selar de novo, e recusar enquanto a ligação estiver trancada.
- Limite a escrever no §19: o AAD não amarra host, usuário nem pino. Quem **escreve** no arquivo redireciona a ligação e recebe a senha. Está fora do modelo declarado (a cópia), mas o texto deveria dizer.

### Nota para SEC (fora do meu domínio)
Com `chave_mestra_*` pronta, o sal não entra na chave (`cofre.rs:341`), e a prova usa nonce zero. Cadastros que dividem a mesma chave repetem o par chave/nonce na prova. Pela leitura, isso só permite forjar a prova, não abrir envelope. Se SEC quiser uma subchave por sal, é mudança de formato e entra antes do lançamento.

### Veredito: NÃO BLOQUEIA o commit, com uma condição de texto no mesmo commit
**Condição:** uma frase no CHANGELOG, no MANUAL 15.8, no FORMATO §19 e no DBLINK: «**a primeira gravação do binário anterior apaga os envelopes de todas as ligações**». E dizer que formato maior que 2 **impede o servidor de subir**. «Lê vazia» é meia verdade sobre um estrago que não tem volta.

**Pedidos novos:**
1. **Formato, papel C, decidir antes do lançamento que levar o formato 2.** Mover a lista do formato 2 para outra chave (por exemplo `ligacoes`). O binário velho passa a recusar alto (HEAD `:773-778`) em vez de apagar calado. É o mesmo preço que este binário já escolheu para o formato maior que 2 (`:43-50`). Hoje é barato, porque nada foi lançado; depois vira migração.
2. **Cadastro ilegível tranca o DbLink, não o motor.** Vale para arquivo torto, formato maior que 2, credencial duplicada no mesmo campo e material torto. Todos sobem pelo `?` de `servidor.rs:1285`. «O DbLink é acessório; o motor não é.»
3. **`fsync` do diretório depois do `rename`**, em `gravar_privado` e nos irmãos que também fazem `rename`. Dívida da casa inteira.
4. **Recifrar com a chave velha e a nova.** E o alívio barato: quando nenhuma ligação tem mais envelope, o material trancado não protege nada e pode ser trocado por um novo com a chave atual. Escrever também o procedimento da chave perdida.

Arquivos lidos:
- `/home/user/adrianoboller/phxsql/crates/phxsql-server/src/dblink/mod.rs`
- `/home/user/adrianoboller/phxsql/crates/phxsql-server/src/config.rs`
- `/home/user/adrianoboller/phxsql/crates/phxsql-store/src/cofre.rs`
- `/home/user/adrianoboller/phxsql/crates/phxsql-server/src/servidor.rs`
- `/home/user/adrianoboller/phxsql/docs/FORMATO.md`
- `/home/user/adrianoboller/phxsql/docs/DBLINK.md`
- `/home/user/adrianoboller/phxsql/MANUAL.txt`
- `/home/user/adrianoboller/phxsql/CHANGELOG.md`
- `/home/user/adrianoboller/phxsql/docs/propostas/parecer-dba-372-e-255.md`
