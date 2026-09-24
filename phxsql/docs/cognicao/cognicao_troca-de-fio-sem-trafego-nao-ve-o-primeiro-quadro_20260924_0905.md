# Prova de troca de fio sem tráfego não vê a corrida do primeiro quadro

**Descoberta:** 24/09/2026, 09:05, frente da volta ao UDP do phxvpn
(`phxvpn/src/fio.rs`, prova `phxvpn/provas/tcp/rodar.sh volta`).

## 1. O que aconteceu

A prova nova da volta ao UDP liga um ping contínuo (a cada 0,2 s) e só então
desbloqueia e rebloqueia o UDP. Na volta, tudo certo. Na **segunda** queda
para TCP — a primeira que acontece **com tráfego no túnel** — o nó A entrou
em laço: `repasse por TCP` / `repasse TCP caiu -- Connection reset by peer`,
quatro resets no registro de A daquela corrida.

## 2. O que eu concluí primeiro, e estava errado

Que era efeito da mudança nova — a carência que mantém a conexão TCP velha
aberta 2 s na volta, ou a sonda por UDP mexendo na tabela do repasse. As duas
estavam descartadas pelo próprio desenho (a SONDA não muda a ponta do nó; a
carência só vale com o fio já em UDP), e o defeito era mais velho que esta
frente.

## 3. O que a medição disse

O repasse só aceita conexão TCP cujo **primeiro quadro é um REGISTRO**
(`repasse_tcp.rs`) e derruba a que começa com dado. A conexão nasce na thread
de leitura (`receber`), o REGISTRO sai no tique (até 1 s depois), e a
`escrita` ficava publicada no meio: com tráfego, o dado da placa ganhava a
corrida. A prova antiga (`casos` do `rodar.sh`) cai para TCP **sem nenhum
ping no ar** — o primeiro ping só sai depois — e por isso nunca viu. Com o
guarda (até o REGISTRO sair pela conexão, dado se perde como no TCP ainda sem
conexão): **zero** resets na mesma prova, queda de novo em 23,0 s, e o teste
`conexao_nova_comeca_pelo_registro_mesmo_com_trafego` reprova com o guarda
removido («a conexao comecou com dado»).

## 4. A regra

Prova de troca de caminho roda **com tráfego no ar durante a troca**. Troca
feita com o túnel parado só prova a ordem que o próprio nó escolhe; a ordem
que a rede impõe (quem chega primeiro na conexão nova) só aparece quando
outra thread tem pacote para mandar no mesmo instante.

## 5. Como está guardado hoje

Pelo teste unitário (`fio.rs`) e pela prova em netns, que agora liga o ping
contínuo antes da troca e grava as perdas por fase em
`phxvpn/provas/tcp/resultados.json` (`volta.pings`).

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/tcp/resultados.json`, `teste:conexao_nova_comeca_pelo_registro_mesmo_com_trafego`
- **Validado em:** 24/09/2026
