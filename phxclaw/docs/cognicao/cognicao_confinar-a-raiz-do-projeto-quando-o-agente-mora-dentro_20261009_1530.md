# Confinar à raiz do projeto quando o agente mora dentro dele

**Estado:** INFRUTÍFERO (a falha observada; o conserto abaixo é PENDENTE até o commit onde a
prova roda)

**Evidência:** `crates/phxclaw-agent/tests/ide_web.rs::o_minimapa_nao_serve_a_pasta_do_agente_nem_o_cofre`.
Na árvore de antes do conserto, 8 de 8 portas devolveram 200 com o segredo no corpo — entre
elas `var/agente/segredos/master.key -> 200 OK: {"bytes":44,…,"texto":"vjyccHdK…kUI="}` (chave
de teste, descartada), o envelope do cofre, o `api.token` e um symlink do projeto para a chave.
Com o conserto, as 8 dão 403 sem o texto.

## O que aconteceu

`GET /v1/ide/arquivo` (o minimapa) confinava à **raiz do projeto** pelo `confine` das
ferramentas. No layout padrão a raiz do projeto é a pasta corrente e a pasta do agente é
`var/agente` **relativa a ela** — ou seja, dentro do projeto. A chave-mestra do broker é base64
em texto, passa no filtro de binário, e saía inteira.

## O que eu concluí primeiro, e estava errado

Que bastava recusar «tudo dentro da pasta do agente» no `confine`. As tarefas moram em
`<agente>/tasks/<id>/work`: a pasta de trabalho de toda ferramenta está **dentro** da área
recusada, e a regra cega mataria todo `read_file`. A recusa certa tem a exceção da própria
tarefa (e só dela: a vizinha e o `evidence.jsonl` da própria continuam fora), e reconhece o
broker **pela forma** (`segredos/` com `master.key`) em qualquer lugar, porque uma segunda pasta
de agente no projeto não tem a pasta padrão como prefixo.

## O que a medição disse

- O furo não era só do IDE: o `confine` aceita absoluto dentro de uma raiz do workspace, e
  `{"raizes": ["."]}` no `.phxclaw/workspace.json` punha `var/agente` ao alcance do `read_file`
  pelo caminho absoluto. Por isso a recusa entrou no `confine` (a porta única) e não na rota.
- O teto de 2 MiB só era provado pelo 413: sem o `take`, um esparso de 8 GiB devolvia o mesmo
  413 depois de **19,0 s** lendo tudo. A prova passou a medir o tempo (< 2 s; com o `take`, o
  teste inteiro roda em < 1 s).

## A regra

Confinar a uma raiz não diz nada sobre o que mora **dentro** dela. Quando a casa do agente pode
estar dentro da raiz servida, a área dele é recusada na porta única, depois do canônico, e o
caminho real é reconferido no descritor aberto (o nome conferido não é o arquivo lido).

## Como está guardado hoje

`tarefa::confine` → `area_reservada` (pasta do agente pela mesma conta de quem abre o broker,
`config::pasta_padrao`, mais a forma do broker pelos nomes `canais::PASTA_DO_BROKER` e
`CHAVE_MESTRA`); `tarefa::confere_aberto` para o descritor; `ide::ler_para_o_minimapa` abre uma
vez, sem esperar escritor de FIFO, dentro de `spawn_blocking`. Prevenção: teste do layout padrão
com broker de verdade e as quatro portas (relativo, absoluto por raiz, symlink, cofre).
Fechado depois, no mesmo dia (seção abaixo): o terminal Helix do IDE rodava sem sandbox e
`:open var/agente/segredos/master.key` mostrava a chave na grade.

## O alcance (09/10/2026, tarde)

O `confine` fecha a porta de **disco**. A de **processo** é outra: tudo que roda no bwrap com o
projeto em `/work` via `var/agente` inteira — a regra «confinar a uma raiz não diz o que mora
dentro dela» vale igual para uma **montagem**. Medido com a máscara retirada (`// REPOSTO`):
o `cat var/agente/segredos/master.key` dentro do bwrap devolveu a chave
(`…/c6TCBIiXciXwDjh6g9Z+NBS8=LISTA[api.token`, chave de teste), o Helix fora do bwrap desenhou
`1  U9uzHECMOR3DYorSYjNFHHf0EjltNhXha/Tlp41+MhQ=` na grade, e o `cargo test` do explorador leu
a chave (`falhou: 1`).

**O que eu concluí primeiro, e estava errado:** que o conserto cabia no `processo.rs`
(`espec_no_bwrap`), como pedia a tarefa. Ali passam só servidores de linguagem e MCP; o
explorador de testes, o túnel, o git, os hooks e o Python chamam `run_in_workdir_com` direto —
são **irmãos** pela regra da casa (chamam a mesma função na mesma ordem). O lugar único é a
montagem (`phxclaw_sandbox::workdir_sandbox_command_com`), e a máscara sai da **lista de
montagens já feitas**, não de quem chamou: a raiz do workspace montada no próprio caminho, que
ninguém lembraria, entra sozinha. O sandbox não sabe onde mora a pasta do agente; recebe a
função (`ocultar_com`) do `achar_bwrap`, por onde todo processo desta crate passa. A exceção da
tarefa sai pela forma: `<agente>/tasks/<id>/work` mora dentro da pasta, não a contém.

Segundo erro, do Helix: pô-lo no bwrap de sempre. O `--new-session` tira o terminal de controle e
o `SIGWINCH` nunca chega — medido com PTY: com a opção, redimensionar não redesenha; sem ela,
redesenha. O que ela protege (TIOCSTI num shell de fora) não existe num PTY que nasce para o hx.
E o PTY herda o ambiente do agente inteiro: `--setenv` poria o Bearer no argv; `--unsetenv` do
herdado mantém o valor só no ambiente do bwrap.

**Estado do alcance:** PENDENTE até o commit onde as provas rodam
(`tests/sandbox_agente.rs`, `tests/ide_web.rs::o_helix_do_ide_nao_abre_a_chave_da_pasta_do_agente`
e `os_testes_do_ide_nao_leem_a_pasta_do_agente`; RED 5/5 com a máscara retirada).
