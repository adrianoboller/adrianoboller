# Opção de conexão escrita depois de um `<connection>` não vale para ele

## O que aconteceu

Frente do alcance do modo servidor do phxvpn (queda UDP→TCP por blocos
`<connection>`). O perfil `.ovpn` tinha a ordem de sempre: diretivas, depois
`<ca>`/`<cert>`/`<key>`/`<tls-crypt>` embutidos no fim. O laboratório pôs os
dois blocos (`remote … udp`, `remote … 443 tcp-client`) logo depois de
`dev tun`, onde antes ficavam `proto` e `remote`.

## O que eu concluí primeiro, e estava errado

«O manual diz que opção fora do bloco vira padrão para os blocos — então
`server-poll-timeout`, `nobind` e a chave embutida valem para todos, onde
quer que estejam.» O manual diz outra coisa, e com a palavra que eu li por
cima: vira padrão **para os blocos que vêm DEPOIS dela**
(connection-profiles.rst: «used as a default for `<connection>` blocks which
follow it»).

## O que a medição disse

OpenVPN 2.6.19, cliente em netns, blocos no meio do perfil:

- o log avisa `Option 'server-poll-timeout' … is ignored by previous
  <connection> blocks`, e o mesmo para `connect-retry`, `nobind` e
  **`tls-crypt`** (options.c:5617-5626);
- sem o `server-poll-timeout 5` valendo, o cliente **não conectou em 20 s**
  (a espera do laboratório) e o ping pelo túnel deu `Network is
  unreachable` — ficou no bloco UDP esperando o padrão de 120 s;
- com os blocos por último: troca UDP→TCP em **5 s** (carimbos do log) e
  ping 3/3.

O `<tls-crypt>` embutido é opção de conexão: com os blocos no meio, o perfil
subiria sem a chave que o servidor exige.

## A regra

Em perfil com `<connection>`, os blocos são a ÚLTIMA coisa do arquivo; nada
que alguém pendure depois pode ser opção de conexão.

## Como está guardado hoje

- `phxvpn/src/ovpn.rs` (`perfil_membro`) põe `{fim}` — os blocos que
  `phxvpn/src/alcance.rs` (`partes`) gera — depois do `<tls-crypt>`.
- `teste:blocos_de_conexao_sao_a_ultima_coisa_do_perfil` e
  `teste:nada_pendurado_depois_dos_blocos_e_opcao_de_conexao` (este monta o
  perfil INTEIRO, com o que `saida.rs` e o autenticador penduram depois, e
  reprova se uma linha dali for opção de conexão — a lista saiu do
  `options.c`). Os dois reprovam com a guarda tirada:
  `phxvpn/provas/servidor-alcance/resultados.json` → `red_das_guardas`.
- O buraco que fica: a lista `OPCOES_DE_CONEXAO` do teste é da 2.6.19. Opção
  nova de conexão numa versão futura não entra sozinha.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/servidor-alcance/resultados.json`, `phxvpn/src/alcance.rs`, `phxvpn/provas/servidor-alcance/red.py`
  (os testes citados moram em `phxvpn/src/alcance.rs`; o `classificar.py` só procura `teste:` dentro de `phxsql/`)
- **Validado em:** 24/09/2026
