# `bancada/usuarios` — o cadastro pelo protocolo, exercitado

**Por que existe.** O pedido 221 dizia: *«Não existe operação de protocolo que
crie usuário: as diretivas se escrevem no `config.json` e reiniciam.»* Agora
existem três — `usuario_criar`, `usuario_alterar` e `usuario_excluir` —, e o
que elas prometem é justamente o tipo de coisa que teste unitário não prova:
que o efeito é **a quente, no mesmo processo**, e que a senha não sobrou em
nenhum dos arquivos que o servidor escreve sozinho.

**As quatro coisas que só o motor vivo dá.**

1. **A quente, sem reiniciar.** O `phxsqld` sobe **uma** vez, com **um**
   administrador no `config.json`. O resto do cadastro nasce por pedido de
   protocolo, e cada login acontece numa **conexão nova contra o mesmo
   processo**. Um teste unitário prova a estrutura; só o processo vivo prova
   que nada ficou preso na fotografia do arranque.
2. **A senha em três arquivos que o unitário não percorre**: o `config.json`
   no disco, o `acessos.log` escrito pelo laço de conexão, e o `perfil.txt` do
   **Profiler ligado** — que existe justamente para mostrar o texto cru dos
   pedidos, e é o único lugar do servidor onde a senha de um `usuario_criar`
   apareceria inteira.
3. **A sessão do excluído**, na conexão que já estava aberta e autenticada:
   ela não é derrubada, ela deixa de valer no pedido seguinte — com o «faça
   login» que todo cliente trata, e não com um soquete cortado.
4. **O controle positivo do varredor.** Antes de acreditar em «não achei a
   senha», a parte 0 mostra que o mesmo varredor **acha** a senha quando ela
   está lá. Varredura que nunca acha nada protege igual e não prova nada.

**O defeito que ela achou, e que ler o código não acharia.** Na primeira
corrida, a parte 8 reprovou: `CREATE USER c PASSWORD 'x'` pela op `sql` punha a
senha **inteira** no anel do Profiler. A redação do Profiler é por **nome de
campo** — e ali a senha não está num campo: está no meio de uma frase, num
campo chamado `texto`, que é exatamente o que o Profiler existe para mostrar.
O teste unitário que existia passava, verde, porque provava o caminho **JSON**,
que já estava certo. Conserto e cognição em
`docs/cognicao/cognicao_a-senha-dentro-da-frase-escapa-da-lista-de-nomes_20260907_1758.md`.

**A segunda coisa que ela ensinou** foi sobre si mesma: duas afirmações da
parte 5 diziam provar a guarda «não se apaga a própria conta» e disparavam a
guarda «sem administrador ativo», que vem antes. As duas recusas estavam
certas; a bancada é que descrevia a errada. Por isso as seis recusas casam o
**pedaço da mensagem**, e não só o veredito — ver
`cognicao_a-guarda-que-dispara-antes-esconde-a-que-se-queria-provar_20260907_1810.md`.

**Medido.** 53 afirmações, 0 falhas (07/09/2026, `phxsqld 0.18.0`).

**O que ele também cobre.** As seis recusas (leitor sem `administrar`, a
própria conta, o `root`, `senha_hash` pronto, login com espaço na ponta, login
repetido), os três comandos SQL — inclusive a guarda de que `ALTER` de outra
coisa **não** é roubado por este caminho, que é a fronteira com a frente do
`ALTER SERVER` —, e o rastro: `cadastro gravado por <quem>` no erro padrão.

**Como roda.**

```bash
./cargo-da-frente.sh build --release -p phxsql-server --bin phxsqld
bancada/esta-medindo.sh && echo "ha medicao em curso -- espere"
python3 bancada/usuarios/provar.py
```

Sobe **um** servidor na porta 7501 em 127.0.0.1, com tudo em
`/tmp/phx-f5u-<pid>`, e derruba **por PID** no fim — nunca `pkill`. A porta sai
de `PHX_F5_PORTA_USUARIOS`. Os números vão para `resultados.json`.

**O que ele NÃO mede.** Tempo. É uma bancada de *veredito*, não de
desempenho: não há número de µs aqui, e por isso ela não entra nos gráficos.
O que ela publica é «quantas afirmações, quantas falhas» — e falha nenhuma é
o único resultado aceitável.
