# `mlockall` com limite finito quebra o processo — e `VmLck` não é o que está preso

## O que aconteceu

Item `mlock` do phxvpn (`phxvpn/src/memoria.rs`): tirar do swap as chaves do
painel, do nó P2P e do repasse, e ligar o `mlock` do OpenVPN do modo servidor.
Este contêiner roda como root **com** `CAP_IPC_LOCK` e **sem**
`CAP_SYS_RESOURCE`, com `RLIMIT_MEMLOCK` 8 MiB (macio e duro) — o mesmo
limite que o systemd novo dá a um serviço.

## O que eu concluí primeiro, e estava errado

1. «Root pode travar, então é só ligar.» Root sem `CAP_SYS_RESOURCE` não sobe
   o limite duro, e o `--mlock` do OpenVPN tenta subir para 100 MiB e sai com
   **FATAL** quando não consegue: ligar a diretiva sempre derrubaria a rede
   justamente no serviço instalado (painel `User=root`, sem essa capacidade).
2. «`VmLck` diz quanta memória ficou presa.» Com `MCL_ONFAULT` ele conta o
   **reservado** (a pilha inteira de cada thread), não o que foi tocado: o
   painel com 120 conexões mostra 2.291.256 KiB em `VmLck` com 6.288 KiB de
   `VmRSS`.

## O que a medição disse

- `openvpn --mlock` aqui: `setrlimit() failed: Operation not permitted …
  Exiting due to fatal error` (`provas/operacao/resultados.json`, seção
  `openvpn.ambiente`).
- `VmRSS` do painel com 120 conexões: sem mlock 6.260 KiB; `mlockall` sem
  `MCL_ONFAULT` **263.240 KiB** (42×); com `MCL_ONFAULT` **6.288 KiB**
  (seção `mlock`).
- `nobody` com 8 MiB e sem capacidades: o código novo não trava (VmLck 0),
  o processo segue vivo e diz no log.

## A regra

Trave a memória do processo só quando o limite não alcança (capacidade ou
limite infinito), com `MCL_ONFAULT`; prove que travou pelo `VmLck` e quanto
custou pelo `VmRSS` — nunca o custo pelo `VmLck`.

## Como está guardado hoje

`memoria::decidir` e `memoria::openvpn_aguenta_mlock` (testes
`com_limite_finito_nao_trava` e `mlock_do_openvpn_so_quando_ele_sobe`), a
prova no processo filho (`filho_trava_e_o_proc_confirma`, RED: sem a chamada,
VmLck 0), `LimitMEMLOCK=infinity` na unidade do systemd
(`repasse_sem_root_e_painel_com_credencial_cifrada`) e
`provas/operacao/mlock.sh`. O buraco: o `mlock` do OpenVPN **ligado** não foi
medido — subir o limite duro pede a capacidade que este ambiente não tem.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/operacao/resultados.json`, `phxvpn/provas/operacao/mlock.sh`, `teste:com_limite_finito_nao_trava`, `teste:filho_trava_e_o_proc_confirma`
- **Validado em:** 24/09/2026
