# Binário velho não erra alto com flag nova: ele a ignora e vira servidor

## O que aconteceu

Provando o pedido 478 (o pacote de demonstração empacotando em `.phz`), rodei
`./empacotar.sh demonstracao <dir>`, que por dentro chama
`./target/release/phxsqld --empacotar-config --config <dir>/demonstracao/config.json`.
O comando nunca retornou: ficou pendurado até o prazo de 120 s do meu próprio
shell, e o processo continuou vivo depois disso — com quatro threads
(`ouvinte-web`, `relogio-gravacao`, `sonda-disco`, `amostrador`) e um
`acessos.log` aberto dentro do diretório da demonstração.

O `target/release/phxsqld` do repositório era de um commit anterior a
`c544247` — `phxsqld 0.19.0 (a8f62b3f9d4b-sujo)` —, de antes de
`--empacotar-config` existir no `main.rs`. Recompilado (`cargo build --release
--offline -p phxsql-server --bin phxsqld`), a mesma chamada retornou em menos
de 1 s, gravou o `.phz` e nada mais.

## O que eu concluí primeiro, e estava errado

Com o processo pendurado e sem porta aberta ainda visível, a hipótese que
formei foi que `trocar_a_forma`/`empacotar_arquivo` estava travado dentro do
`config_phz.rs` — um laço no `hard_link`, ou a conferência bytea-byte lendo de
volta indefinidamente. Cheguei a olhar `/proc/<pid>/stack` esperando achar uma
chamada de `phxzip`. A pilha real (`inet_csk_accept`) e as quatro threads
nomeadas mostraram outra coisa: aquilo não era o empacotador travado, era um
SERVIDOR completo, rodando `accept()` na porta de dados — o binário nunca
tinha entendido `--empacotar-config` como flag, só como argumento
desconhecido, e caiu direto no arranque normal usando o `--config` que sobrou.

## O que a medição disse

`phxsqld --version` do binário preso na hora do incidente: commit
`a8f62b3f9d4b`, marcado `-sujo`. O `HEAD` da árvore de trabalho já estava em
`c5442470e287` (o commit que introduziu `config_phz.rs` e os dois novos
comandos). A diferença não apareceu em erro nenhum — `phxsqld` não reconhece
uma flag e segue em frente, porque o `main` só reage às flags que conhece e
ignora o resto; não há “flag desconhecida” que pare a execução.

## A regra

**Binário desatualizado que ganha uma flag nova não erra: ele boota como se
ela não existisse.** Antes de qualquer prova manual pela linha de comando —
não só antes de medir desempenho —, confira `phxsqld --help` (ou `--version`
contra `git rev-parse HEAD`) para o binário que a prova vai chamar. Um
processo que não retorna pode ser o comando novo travado, ou pode ser o
comando antigo nunca tendo existido.

## Como está guardado hoje

É o **alcance** da pétrea já escrita no `CLAUDE.md` — *"Medidor com binário
velho mede o passado"* —, que até aqui só tinha sido paga em números de
desempenho (16,4 → 7,5 µs invisíveis por `examples` não recompilados). Aqui o
sintoma foi funcional, não numérico, e mais caro de diagnosticar porque
parecia um defeito NOVO na feature que eu tinha acabado de mexer, não um
binário antigo. Nenhuma guarda automática confere isso hoje — `empacotar.sh`
chama `./target/release/phxsqld` sem checar a versão contra o `HEAD`; o
`garante_host` só confere que o arquivo *existe*, não que é o mesmo commit. O
buraco fica registrado aqui em vez de virar guarda nova, porque o corretivo
óbvio (recompilar sempre) já é o que `garante_host` deveria fazer e não é
escopo deste pedido mudar.

**Atualizado na integração (papel A, mesmo dia):** o buraco não ficou só registrado. A própria mudança do 478 é que passou a chamar uma flag nova pelo `empacotar.sh`. Medido numa cópia com um binário velho simulado, a receita ficava pendurada com o config em claro. Por isso ela entrou consertada no mesmo commit: o `garante_host` sempre chama o `cargo build` (o cargo decide se o binário está velho), e a demonstração roda com prazo e confere o **resultado**, não o código de saída. A causa raiz, o `phxsqld` que aceita calado uma flag desconhecida, virou o pedido 483.
