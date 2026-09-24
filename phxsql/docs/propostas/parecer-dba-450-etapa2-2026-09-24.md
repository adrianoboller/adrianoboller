# Parecer C (DBA) — pedido 450, etapa 2 (`config.phz`), 24/09/2026

Registrado na íntegra pelo integrador (papel A), como veio do papel C. Árvore conferida: a exata do commit, montada em scratch e idêntica ao working tree por `cmp`.

**Veredito: BLOQUEIA, por dois consertos pequenos** (uma linha de código e uma frase de garantia). O resto passa. `.reg`/`.ndx`/`PSCH` intocados; a migração mexe só no config, entra pedida e tem volta.

## (1) Migração atômica o bastante? RISCO — não perde config, mas tem 2 bloqueios

- **Desenho certo.** `trocar` (`config_phz.rs:464-516`) grava o `.phz` com tmp + `sync_all` + `rename` (`config.rs:2725-2740`), relê, compara e só então renomeia o `.json`. Queda entre `:474` e `:492` deixa **os dois, idênticos**; o próximo arranque dá CONFLITO (`:164`), e retirar qualquer um resolve sem perda.
- **Falta o fsync do diretório** (`config.rs:2739`): queda de energia depois do «sucesso» pode voltar a «só `.json` + `config.tmp`» ou «os dois». Nenhum perde nada; toda gravação de config desta casa já é assim. Não bloqueia.
- **BLOQUEIA (código):** `main.rs:314-315` imprime `gere um modelo com: phxsqld --exemplo 1 > config.json` para QUALQUER erro do `Config::ler`, inclusive CONFLITO e `.phz` corrompido. Quem segue a dica trunca o `.json` que o administrador extraiu para editar — o «caso real» do desenho (`config_phz.rs:29-32`). Conserto: só dar a dica quando nenhum dos dois do par existe; o teste `os_dois_presentes_o_binario_nao_sobe_e_nao_toca_em_nenhum` (`tests/config-phz.rs:241`) tem de reprovar se a dica aparecer.
- **BLOQUEIA (texto):** `FORMATO.md:3005` e `config_phz.rs:463` dizem «nunca os dois, nunca nenhum». Vale para erro tratado, não para kill/queda. Frase certa: «numa queda entre os dois passos ficam os dois, idênticos, e o arranque recusa até alguém retirar um; nada se perde».
- **Servidor no ar durante `--empacotar-config`:** sem trava de processo no crate. O servidor vivo continua com `caminho=config.json`, e toda gravação lê antes (`config.rs:5701` e irmãs, `servidor.rs:4535`) — falha alto com «nao consegui ler config.json», não recria, sem perda nem CONFLITO. Falta o MANUAL §7.5 dizer «pare o servidor antes». Não bloqueia.

## (2) Binário ANTERIOR diante de diretório migrado — SIM, recusa

HEAD: `config.rs:4317-4321` faz `read_to_string` → `NaoEncontrado` → `main.rs:250-256` sai FAILURE; não sobe com padrão nem escreve nada. Com `--config config.phz`, `InvalidData` → recusa. **RISCO (não bloqueia):** a dica do HEAD (`main.rs:254`) manda gerar `config.json` do zero, e quem obedece sobe o velho com o token/usuários do modelo (dado intacto; o novo depois dá CONFLITO). Falta em FORMATO §20 / MANUAL §7.5: «antes de voltar ao binário anterior, rode `--desempacotar-config` com o novo».

## (3) Backup/restaurar, replicação, cluster esperam `config.json` pelo nome? SIM — nenhum quebra calado

`op_backup` (`servidor.rs:20346-20366`) copia só `config.base`; `phxsql-store/src/backup.rs:36-38` diz que o config nunca entrou no backup; `restaurar_backup` (`servidor.rs:20562`) mexe só no database. Caminhos derivados saem do diretório pai do arquivo LIDO (`config.rs:86-98`, `:4342-4352`; chave do fio `:2142`) — `.json` e `.phz` no mesmo diretório → mesmos caminhos. `cluster.estado.json` e `replicacao-posicoes.json` ficam na base (`cluster.rs:316`, `servidor.rs:1301`); a lista do cluster grava pelo caminho resolvido (`servidor.rs:4535`) → continua `.phz`.

## (4) Deixar dblink.json e os outros de fora se sustenta? SIM — e o motivo medido é mais forte que o escrito

`dblink/mod.rs:1297` confirmado (ilegível → cadastro vazio). Os outros quatro têm o **mesmo molde**: `blacklist.rs:494-497`, `jobs.rs:434`, `bidirecional.rs:579-584`, `cluster.rs:321` — e o do cluster é o pior (papel volta ao do config: master destronado volta mandando; o próprio comentário `cluster.rs:308-310` diz). A §20 e a SEGURANCA §23 justificam com «estado/catálogo/regrava em funcionamento»; troquem pelo motivo medido: empacotar qualquer um quebra «binário velho recusa em vez de apagar». Não bloqueia.

## (5) A §20 basta para reimplementar a LEITURA? RISCO — descreve a gravação, não o que a leitura aceita

Falta: métodos aceitos na leitura (Copy, LZMA, LZMA2, 7zAES; o resto `[METODO_LEGADO]`/`[METODO_DESCONHECIDO]`, `phxzip/src/leitor.rs:115-116`); regra da entrada (UMA, não pasta, conteúdo pelo 7zAES; cabeçalho cifrado opcional na leitura — `phz.rs`, `desempacotar_com_limites`); o nome da entrada é ignorado na leitura (`config_phz.rs:193`); a derivação (contador 64 bits LE; referência ao `7zFormat.txt`/7zAES); a lista de erros incompleta (faltam `SEM_ENTRADA`, `ENTRADA_E_PASTA`, `METODO_*`, `SENHA_ERRADA_OU_CORROMPIDO`). **E o que tem de entrar agora:** a camada `.phz` não tem byte de versão — a `SENHA_DO_PHZ` **é** a versão, implícita. Trocá-la amanhã faz todo `config.phz` instalado virar `[SENHA_ERRADA]`, e isso é migração. A §20 tem de dizer: «a senha é parte do formato e não muda sem um leitor que aceite a anterior». Hoje custa um parágrafo. Não bloqueia.

## Achado menor (não bloqueia)

`par()` não é simétrica fora de `.json` (`config_phz.rs:149-158`, `:398-404`): com `--config meu.conf` a entrada no `.phz` é gravada como `meu.json`; quem extrai com o 7-Zip para editar recebe `meu.json`, que o par (`meu.conf`/`meu.phz`) não enxerga — edição ignorada calada, sem CONFLITO. Alcance hoje zero: todo `--config` documentado no repositório é `.json` (15 ocorrências). Registrado.
