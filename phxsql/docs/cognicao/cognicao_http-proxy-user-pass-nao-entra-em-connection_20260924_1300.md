# `http-proxy-user-pass` não combina com blocos `<connection>` no 2.6

## O que aconteceu

A revisão SEC pediu que o proxy HTTP com credencial em arquivo usasse `auto`
(o método sai do 407) em vez de `basic` fixo. No 2.6.19 o `auto` só pega a
senha de um arquivo pela diretiva à parte `http-proxy-user-pass` (o terceiro
argumento é ou `auto`, ou o arquivo, options.c:6819-6841). O perfil da rede
UDP com queda para TCP leva blocos `<connection>`, e o proxy vai só no bloco
TCP.

## O que eu concluí primeiro, e estava errado

«Ponho `http-proxy-user-pass` fora dos blocos, antes deles, e ele vira padrão
para o bloco TCP, como `nobind` e `server-poll-timeout`.» Não virou.

## O que a medição disse

Laboratório em netns, 2.6.19, proxy de prova que exige Basic:

- **dentro do bloco:** `option 'http-proxy-user-pass' cannot be used in this
  context ([CONNECTION-OPTIONS])`. Ela não é opção de conexão
  (`OPT_P_GENERAL|OPT_P_INLINE`);
- **antes dos blocos:** `--http-proxy MUST be used in TCP Client mode` — ela
  cria as opções de proxy no padrão global, e o bloco UDP as herda;
- **depois dos blocos:** não vale para eles, e o cliente pergunta a senha
  («can't ask for 'Enter HTTP Proxy Username:'»);
- **sem bloco UDP** (rede TCP, bloco único): `auto` + `http-proxy-user-pass`
  **conectou** — na prova com o painel, em 2,1 s.

## A regra

Numa rede UDP com queda, o arquivo de credencial só entra no bloco TCP com
método fixo (`"arquivo" basic`). Onde o texto claro é recusado, a saída é
«pedir ao conectar» (`auto-nct`, que funciona dentro do bloco).

## Como está guardado hoje

- `phxvpn/src/alcance.rs`, `ProxyMembro::linhas(sem_texto_claro, em_bloco)`
  e `conferir_proxy`, que recusa arquivo + queda + «sem texto claro».
- `teste:linhas_de_proxy_por_tipo_e_credencial` e
  `teste:sem_texto_claro_recusa_o_que_mandaria_a_senha_em_claro`; RED em
  `phxvpn/provas/servidor-alcance/resultados.json` → `red_das_guardas`.
- Prova: `proxy_tcp_auto_arquivo_conectou_s` (2,1 s) e
  `proxy_nct_com_basic_conectou_em_30s` = null (o `auto-nct` recusa o Basic).

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/servidor-alcance/resultados.json`, `phxvpn/src/alcance.rs`, `phxvpn/provas/servidor-alcance/rodar.sh`
  (os testes citados moram em `phxvpn/src/alcance.rs`; o `classificar.py` só procura `teste:` dentro de `phxsql/`)
- **Validado em:** 24/09/2026
