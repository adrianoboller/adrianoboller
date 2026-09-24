# Uso por `cwd` não alcança o binário que roda de dentro do `target/`

**Estado:** PENDENTE

## O que aconteceu

O disco caiu a 521 MB em 24/09/2026, com quatro frentes compilando. O vigia
do zelador estava vivo e já tinha rodado o `--mesmo-assim`, mas uma cópia da
árvore no scratchpad da sessão, sem `.git`, seguia ocupando 1,5 GB — 99% dela
`target/`. O zelador listava a cópia como «copia SEM git: nao da para provar,
fica», e ela só saiu à mão.

## O que eu concluí primeiro, e estava errado

Que a regra certa era «cópia sem git fica», ponto: sem `.git` não há como
provar que a fonte é redundante, e apagar por palpite é o que o cabeçalho do
zelador proíbe. A conclusão juntava duas coisas diferentes. O que não se prova
é a FONTE; o `target/` é artefato, e artefato provado sem uso é exatamente o
que a regra manda apagar. A cópia inteira ficar não obriga o `target/` dela a
ficar.

E, ao escrever a prova de uso do `target/`, a segunda conclusão errada: que
o `em_uso` do zelador bastava. Ele olha só o `cwd` dos processos. Um binário
de teste roda de dentro do `target/` com o `cwd` em qualquer lugar — o
`cargo test` do teste de integração muda de diretório, e um processo filho
pode segurar um arquivo aberto sem nunca ter entrado ali.

## O que a medição disse

Três casos, na mesma cópia de prova no scratchpad (`--ver`, 24/09/2026 16:50):

| caso | o que o zelador disse |
|---|---|
| `target/` velho (2 h) e sem uso | apaga, «nenhum cwd, descritor ou mapa» |
| aberto por descritor, `cwd` em `/` | «aberto por PID … (sleep), fica» — o `em_uso` sozinho o teria apagado |
| mexido agora | «mexido há menos de 30 min, fica» |

## A regra que fica

Quando a pergunta é «alguém usa este diretório?», o `cwd` responde só por
quem ENTROU nele. Quem usa sem ter entrado aparece no descritor e no mapa.
O `aberto_por_alguem` do `zelador.sh` olha os três; o `em_uso` continua
servindo onde entrar é o único jeito de usar (a raiz de uma worktree).

## Para promover

Falta a prova versionada: uma guarda que reponha o `em_uso` no lugar do
`aberto_por_alguem` e veja o caso 2 apagar.
