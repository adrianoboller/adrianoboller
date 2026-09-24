# Um `port-share` para o loopback põe na internet o que só escutava no loopback

## O que aconteceu

A frente do alcance do phxvpn (commit b8e4b91) aceitava como alvo do
`port-share` qualquer `host:porta` válido para uma linha de perfil (a mesma
validação do `http-proxy`). A revisão SEC bloqueou o commit: com
`127.0.0.1:8470`, a porta TCP pública da rede (443) repassava tudo o que não
fosse OpenVPN ao painel. E o painel escuta em loopback justamente por falar
HTTP sem TLS (achado A4). O mesmo valia para a ponte da queda e para a
diretiva do OpenVPN numa rede TCP.

## O que eu concluí primeiro, e estava errado

«Alvo de `port-share` é escolha do administrador, e ele sabe o que é o
HTTPS dele.» Tratei o campo como sintaxe: só olhei se ele injetava diretiva
no perfil. Mas o `port-share` é um repasse de uma porta pública para dentro do
host. Quem aponta o repasse para o loopback desfaz a proteção que o loopback
dava, e a guarda A4 fica contornada sem que ninguém mexa nela.

## O que a medição disse

`phxvpn/provas/servidor-alcance/rodar.sh`, painel real em netns:

- `127.0.0.1:8484`, `[::1]:8484` e `192.168.92.1:8484` (o IP do servidor na
  porta do painel): os três foram **recusados** ao gravar, cada um com o
  motivo (`alcance.log`);
- `192.168.92.14:9443` (outra máquina): o curl pela 443 recebe a página, e a
  conexão repartida aparece no `openvpn.log` com o IP de fora.
- RED: `teste:port_share_nao_expoe_o_loopback_nem_o_painel` reprova com cada
  uma das duas guardas tirada (`red_das_guardas`, 30/30).

## A regra

Todo repasse de porta pública (port-share, proxy reverso, relé) valida o
DESTINO: não aceita loopback, link-local nem a porta de um serviço que só é
seguro por estar no loopback, e aceita só IP literal, porque um nome pode
passar a resolver para o loopback depois.

## Como está guardado hoje

- `phxvpn/src/alcance.rs`: `validar_port_share` (IP literal, fora de
  `127/8`, `::1`, `0.0.0.0`, `169.254/16`, `fe80::/10` e multicast, com o
  IPv4 mapeado desembrulhado antes) e `Alcance::validar`. Um alvo deste host
  (conferido por `bind`) não pode ser a porta do painel nem a da própria
  rede.
- Valor gravado antes da guarda cai no arranque com aviso, e a rede sobe sem
  ele.
- O buraco que fica: um serviço da LAN que também confia no «veio de
  dentro» continua alcançável pelo `port-share`, se o admin apontar para
  ele. Isso é escolha dele, e não há como a validação saber.

## Estado

- **Estado:** INFRUTÍFERO
- **Evidência:** `phxvpn/provas/servidor-alcance/alcance.log`, `phxvpn/provas/servidor-alcance/resultados.json`, `commit:b8e4b91`
- **Causa:** o alvo do `port-share` foi validado como sintaxe de perfil, não como destino de um repasse público; o loopback, que protegia o painel HTTP, virou alcançável pela 443.
- **Prevenção:** repasse de porta pública valida o destino — IP literal, sem loopback nem link-local, sem a porta do painel —, e a prova tenta gravar o repasse apontado ao painel e confere a recusa.
- **Validado em:** 24/09/2026
