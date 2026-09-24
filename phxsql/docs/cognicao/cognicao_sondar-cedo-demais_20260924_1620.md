# Sondar cedo demais inventa defeito — e o Wine derruba serviços

## O que aconteceu

Em uma tarde, três sondagens com prazo curto deram três diagnósticos falsos:

1. **Painel com credencial do systemd, 6 s:** registro vazio → "a credencial
   não funciona".
2. **Serviço do repasse no Wine, 4 s:** porta sem escutar → "o `println!`
   sem console derruba o serviço".
3. **Painel como serviço no Wine, 40 s e 150 s:** registro vazio → "o painel
   trava dentro do serviço".

## O que eu concluí primeiro, e estava errado

- **O 1 e o 2 eram só pressa.** O painel de depuração leva cerca de 7 s no
  aquecimento (PBKDF2 sem otimização); o Wine leva alguns segundos para
  subir. Repetidos com espera por **condição** ("responde / escuta"), e não
  por relógio, os dois passaram — sem mudar uma linha de código.
- **O 3 não era pressa, e também não era o painel.** Era o Wine: quando o
  último processo de usuário sai (aqui, o próprio `phxvpn servico
  instalar`), ele derruba os processos de sistema, e o `services.exe` e os
  serviços caem juntos. O `sc query` seguinte sobe outro gerenciador, que
  religa o serviço automático e mostra `RUNNING` — de um processo que vai
  morrer de novo. O repasse escapava porque sobe em 1 s; o painel, com 8 s de
  aquecimento, nunca chegava a escrever.

## O que a medição disse

- Painel com a credencial: responde em 7 s. Repasse: escuta em 1 s.
- `ps` sem nenhum processo do Wine vivo, enquanto o `sc query` mostrava
  `RUNNING`.
- Com um `cmd.exe /c ping -n 190` segurando o Wine: painel-serviço no ar em
  8 s, respondendo `/api/estado`, e a conexão vindo só da credencial DPAPI.

## A regra

Esperar por condição, nunca por um número de segundos escolhido de cabeça:
repetir até a condição valer, com um teto generoso, e dizer em quanto tempo
valeu. E, sob o Wine, manter um processo de usuário vivo enquanto se
prova um serviço.

## Como está guardado hoje

- As provas desta rodada esperam por condição (`for i in $(seq N)` até
  `grep`, `curl` ou `ss` responderem) e imprimem o tempo.
- O `PHXVPN.md` diz como o Wine derruba os serviços.
- **E o alcance de uma lei que já existia:** na mesma tarde, um
  `pkill -f "phxteste-painel"` matou o próprio shell (código 144). É a
  cognição de 23/09, repetida. Ela existe só como documento, e nada impede o
  comando. O buraco é esse: a lição está escrita, mas não está guardada por
  nenhum mecanismo.

## Estado

- **Estado:** INFRUTÍFERO
- **Evidência:** `commit:3adbdb9`
- **Causa:** sondas com prazo fixo escolhido de cabeça (6 s, 4 s) mediram antes de o serviço subir e inventaram defeito; sob o Wine, o serviço ainda morre quando o último processo de usuário sai.
- **Prevenção:** esperar por condição, com teto generoso, e imprimir em quanto tempo valeu; sob o Wine, manter um processo de usuário vivo durante a prova.
- **Validado em:** 24/09/2026
