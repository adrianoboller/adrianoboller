# Parecer SEC — pedido 450, etapa 2 (`config.phz`), 24/09/2026

Registrado na íntegra pelo integrador (papel A), como veio do papel SEC. Revisão só de leitura, com prova pelo binário e pelo sistema operacional numa cópia da árvore exata (9 testes de `config_phz` verdes na cópia; cópia apagada depois de provar que nada a usava).

**Veredito: BLOQUEIA, por um único achado (A1), de conserto pequeno.** Os demais não bloqueiam.

## ALTO (bloqueia)

**A1 — a cópia em claro que a migração deixa continua com o token e os hashes legíveis, e em dois casos o aviso de arranque se cala ou mente.** `config_phz.rs:492` (a renomeação mantém a permissão), `config_phz.rs:293` (`aviso_de_arranque` monta o nome da cópia com `par(real)`), `main.rs:228`.

- **Caso 1, provado:** `config.json` 644 → `--empacotar-config` → `config.json.migrado-para-phz` continua **644**. Antes, a primeira gravação pela tela apertava o `config.json` para 600 (achado A4); a cópia renomeada nunca é regravada, então fica aberta para sempre.
- **Caso 2, provado:** `run/config.json` é link simbólico para `etc/config.json`. A migração renomeia o LINK e diz «o original foi renomeado para run/…migrado-para-phz» — falso. O `etc/config.json` real fica 644 com o token; o administrador faz o que a mensagem manda e apaga o `.migrado-para-phz`, o aviso some, e o segredo continua legível no alvo.
- **Caso 3, provado:** com `--config meu.conf` a cópia nasce `meu.conf.migrado-para-phz`, mas o aviso procura `meu.json.migrado-para-phz` e **não avisa nunca** (0 linhas no arranque).
- **Conserto:** apertar a cópia guardada para 600; recusar migrar um `config.json` que seja link simbólico, dizendo qual é o alvo; no aviso, usar o mesmo nome de cópia que a migração usou.
- **Teste adverso:** 644 → empacotar → cópia em 600; o par de testes do link simbólico e de `meu.conf`, afirmando que a mensagem nomeia o arquivo real.

## MÉDIO (não bloqueiam; o 1 e o 2 valem o mesmo commit)

1. **A migração pode apagar o config, e a mensagem mente (provado).** Com `--config servidor.tmp`, o `gravar_privado` (`config.rs:2727-2728`) usa como temporário `with_extension("tmp")`, que é o próprio config: apaga o config, o `rename` da `:492` falha com ENOENT, o desfazer (`config_phz.rs:493`) apaga o `.phz` → **pasta vazia**, com «servidor.tmp continua valendo». Quebra a promessa «nunca nenhum». Conserto: o desfazer só apaga o novo se o velho ainda existe; o temporário ganha nome que não colide com o config.
2. **`--desempacotar-config` só roda depois de `Config::ler` aceitar o arquivo (provado, `main.rs:311-325`).** Um `.phz` que abre mas falha na validação (sem token; recusado por versão futura mais estrita) não sai pela ferramenta; o erro diz «erro no config.json» e sugere `--exemplo 1 > config.json` (`main.rs:314-315`); quem segue cai no CONFLITO, cuja mensagem oferece «retire o .phz» — o caminho leva a perder o cadastro ou a abrir com o 7-Zip usando a senha tirada do fonte. Conserto: a troca de forma roda antes do `Config::ler`, e a mensagem nomeia o arquivo resolvido.
3. **A autenticidade depende da permissão do arquivo, e nada a confere na leitura** (já existia com o claro). O servidor sobe calado de um `config.phz` 666 ou de outro dono; o caminho pelo 7-Zip, que o MANUAL §7.5 recomenda, cria conforme o `umask` (em geral 644); `sudo phxsqld --empacotar-config` cria o `.phz` como root 600, o serviço deixa de ler, e o operador «conserta» com `chmod 644`. Vira pedido: avisar/recusar no arranque se `modo & 0o077` ≠ 0 ou dono ≠ processo; o MANUAL mandar migrar como o usuário do serviço.

## BAIXO

- **B1 — corrida conferir/agir e renomeação por cima** (`config_phz.rs:467/473/492`, `config.rs:2739`): entre o `exists()` e o `rename`, um arquivo que surja no destino é sobrescrito. Não há escrita através de link simbólico em lugar nenhum (`rename`/`unlink` não seguem links; `create_new` é `O_EXCL`) — conferido. Explorar pede escrever na pasta, e quem escreve na pasta já troca o `config.json`. Conserto: `hard_link` + `remove`.
- **B2 — CONFLITO como negação de serviço:** não é pior que o velho (quem cria arquivo na pasta já apaga/troca o `config.json`; o `profiler_ligar` do administrador já acrescenta lixo no próprio config). Exceção: pasta com sticky bit — ali se cria o `.phz` sem poder mexer no `.json`, e isso é negação de serviço nova.
- **B3 — migrar com o servidor rodando:** as gravações falham fechado (provado: `config_gravar` → «não consegui ler config.json»); o config não diverge. O MANUAL devia dizer «pare o servidor antes».
- **B4 — disco cheio ao desempacotar:** sobra `config.tmp` parcial em claro, em 600 (`config.rs:2733-2735`), apagado só na gravação seguinte; falta fsync da pasta depois das renomeações. Na queda, o pior caso são os dois presentes idênticos (CONFLITO), nunca nenhum.
- **B5 — redação:** `SEGURANCA.md:4765` diz «recusado antes de ir à memória», mas o código lê até 17 MiB + 1 e só então recusa (memória limitada; é a frase). `phxzip/src/erro.rs:155` diz «.phz protegido por senha». Nenhum documento chama o `.phz` de cifra nem promete sigilo ou integridade — o ponto (6) está OK.

## Conferido e certo

- **Senha e conteúdo decifrado não vazam:** nenhuma mensagem interpola a senha (a dica de senha errada diz onde a constante está, não o valor); erro de JSON mostra só 1 caractere (provado); o `Debug` de `Troca` só tem caminhos; o temporário nasce 600; a op `config` devolve só o caminho do arquivo.
- **Um leitor só:** `config_phz.rs:194`, com `limites_de_leitura()`; os 6 leitores antigos passam por ele; não há `desempacotar`/`desempacotar_com_teto`/`Arquivo::abrir` fora do phxzip. Medido pelo binário (debug): ciclos 10 abre em 41 ms; ciclos 19 abre em 2.025 ms; ciclos 20 e 24 → `[CICLOS_DEMAIS]` em 6–7 ms; link para `/dev/zero` → `[GRANDE_DEMAIS]`, limitado.
- A op `config` relê o `.phz` a cada chamada: com um arquivo que o 7-Zip regravou em 2^19, ~2 s em debug até a primeira gravação reempacotar em 2^10. Só o administrador chama, sem trava segurada — BAIXO.
