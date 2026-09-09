# Cognição: a conferência mora onde o esquema existe, não onde a configuração é lida

Data e hora da descoberta: 09/09/2026, 05:40 UTC (frente C20-DIREITO, pedido 235).

## 1. O que aconteceu

`Usuario::de_json` (`crates/phxsql-server/src/usuarios.rs`) gravava qualquer
nome dentro de `bases.<b>.tabelas.<t>.colunas` sem conferir contra o esquema
da tabela. `"colunas": {"salrio": …}` carregava calado e `salario` continuava
sem regra: quem escreveu o cadastro achava que restringiu e não restringiu
nada. O brief oferecia dois desenhos — um resolvedor de esquema passado a
`Cadastro::de_json`/`Usuario::de_json`, ou uma passada de conferência depois
da carga — e mandava **medir** qual não obriga quem lê o mesmo `config.json`
sem abrir tabela a quebrar.

## 2. O que eu concluí primeiro, e estava errado

Três vezes, e as três plausíveis:

- **«A conferência vai onde a coluna é lida, dentro de `Usuario::de_json`».**
  Parecia o lugar natural — conferir ao lado de quem lê. Errado porque o
  leitor não tem esquema: teria de recebê-lo pela assinatura, e a assinatura
  chega a quem não tem disco nenhum (ver a medição). Cada um deles passaria
  um resolvedor vazio, que é «não conferir» com uma linha a mais — e a
  linha a mais é a que um dia alguém copia para o lugar em que o disco
  existia.
- **«O lugar certo é a abertura da tabela, não a carga».** Estava escrito
  no `docs/SEGURANCA.md` §15 desde 08/09, pela frente anterior, e li como
  decisão tomada. Era diagnóstico plausível: a abertura acontece a cada
  operação (é o laço quente — a peneira do direito por coluna já paga para
  não custar nada ali), e quem escreveu a regra **não está olhando** a
  abertura. Uma recusa na abertura chegaria ao usuário errado, no momento
  errado, N vezes.
- **«O aviso já chega a alguém pela lista `Cadastro.avisos`».** Contei
  antes de confiar: a lista é impressa no arranque (`main.rs:470`) e no
  `--usuarios` (`main.rs:410`), e as três operações de cadastro
  **descartavam** `novo.cadastro.avisos` inteira. Aviso empurrado numa
  lista que a operação joga fora é aviso para ninguém.

## 3. O que a medição disse

- `Cadastro::de_json` tem **28** pontos de chamada: 25 em testes de
  `servidor.rs`, 1 em `Config::de_json` (`config.rs:2966`, que roda **antes**
  de `Raiz::nova` existir — `main.rs:250` lê o config, `Servidor::novo`
  abre a raiz), 1 em `phxsql-cmd/tests/console.rs`, 1 em
  `usuarios::aplicar_na_arvore`. O `phxsqld --usuarios` (`main.rs:401`)
  lista o cadastro e sai sem subir servidor. Um resolvedor obrigatório na
  assinatura quebraria os três lugares sem disco ou os faria fingir.
- Com a passada separada (`Cadastro::conferir_colunas`, resolvedor
  injetado, **dois** chamadores — `Servidor::novo` e `op_usuario`): os
  cinco testes que afirmam o comportamento novo **caem** com o código de
  08/09 (`subiu com uma regra que cita coluna que a tabela nao tem`,
  `gravou uma regra…`, `a tabela futura passou sem aviso na resposta`,
  `assertion left == right: []`, `a regra inerte passou calada`) e os dois
  controles já passavam; com o conserto, **22 de 22** no módulo e **200**
  nos quatro módulos tocados mais `config::`.
- Custo para quem não pediu: cadastro sem `colunas` **não chama o
  resolvedor** — provado com um resolvedor que panica
  (`sem_colunas_a_conferencia_nao_pergunta_nada`).
- A régua de nome tem de ser a da peneira (`direito_coluna::mesmo_nome`,
  sem caixa e aparada): uma conferência mais dura recusaria `Salario`, que
  a peneira aplica hoje.

## 4. A regra

**Conferência de configuração contra o estado do disco é uma passada com o
resolvedor injetado, chamada por quem TEM o disco — nunca dentro do leitor
da configuração, que fica puro para quem lê sem disco.** E o corolário:
antes de empurrar um aviso numa lista, conte quem lê a lista.

## 5. Como está guardado hoje

- `Cadastro::conferir_colunas` e `ResolvedorDeEsquema` em `usuarios.rs`;
  `colunas_em_disco`, a chamada no arranque (`Servidor::novo`) e a chamada
  dentro do fecho de `gravar_a_secao` (`op_usuario`) em `servidor.rs`;
  `regras_de_coluna_inertes` no `op_criar_tabela`.
- 14 testes (7 unitários em `usuarios::tests`, 7 de servidor em
  `testes_direito_por_coluna`), e a guarda
  `regra-de-coluna-com-typo-carrega-calada` no `bancada/guardas/catalogo.py`.
- `docs/SEGURANCA.md` §15 («A conferência na carga») e `MANUAL.txt` 14.3.2
  e 14.5.
- **Onde o buraco ficou:** `duplicar_tabela`, `copiar_tabela`,
  `restaurar_backup` e `dblink_ligar` também fazem tabela nascer e não
  avisam da regra inerte; `renomear_tabela` e `excluir_tabela` deixam a
  regra citando tabela que não existe mais, e só o arranque seguinte a
  nomeia. A op `usuarios` continua devolvendo só fichas, sem os avisos do
  cadastro vivo.
