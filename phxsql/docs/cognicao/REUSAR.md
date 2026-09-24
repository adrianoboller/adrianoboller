<!-- GERADO por classificar.py a partir das cognições. Não se edita: mude a seção «## Estado» da cognição e rode o gerador. -->

# Reusar — sucessos comprovados

288 cognições: **5 frutíferas**, **4 infrutíferas**, **279 pendentes** (sem evidência validada — não entram aqui).

Só entra o que tem evidência que o `classificar.py` conferiu. Antes de desenhar, procure aqui o que já se provou.

## [Túnel que pinga não é túnel que se recupera](cognicao_tunel-que-pinga-nao-e-tunel-que-se-recupera_20260924_0010.md)

Protocolo com estado se prova também **reiniciando um dos lados no meio**. O caminho feliz com os dois nascendo juntos não exercita a perda de estado.

- Evidência: teste `par_surdo_dispara_aperto_novo` (phxvpn/src/p2p.rs)
- Validado em: 24/09/2026

## [A gerência do OpenVPN: `kill CN` não avisa o cliente; `client-kill` avisa](cognicao_gerencia-openvpn-kill-nao-avisa-o-cliente_20260924_0910.md)

Para derrubar um cliente do OpenVPN pela gerência, use `client-kill` com o CID do `status 2`, nunca `kill CN` — e prove a queda **no cliente**, não pela resposta `SUCCESS` do servidor.

- Evidência: `phxvpn/provas/mfa/resultados.json`; `phxvpn/src/credencial.rs`; `phxvpn/tests/postgres_real.rs`
- Validado em: 24/09/2026

## [Trocar o fio não reenvia o que já saiu pelo fio velho](cognicao_fio-novo-nao-refaz-o-aperto-pendente_20260924_0412.md)

Quando o caminho muda, refaça **tudo** o que está pendente no caminho velho, não só o que o próprio caminho controla: estado de aperto, fila e temporizador de reenvio moram fora do fio e não sabem que ele trocou.

- Evidência: `phxvpn/provas/tcp/resultados.json`; commit `8b35d60`
- Validado em: 24/09/2026

## [Broadcast na LAN: o limitado não sai sem rota, e o `ifa_broadaddr` mente sem `brd`](cognicao_broadcast-na-lan-sem-rota-e-sem-brd_20260924_0415.md)

Broadcast dirigido se **calcula** (endereço `|` `!máscara`), não se lê do `ifa_broadaddr`; o limitado vai junto, nunca sozinho. E quando duas mensagens de um lote dependem uma da outra, a ordem de envio é parte do protocolo — ponha a que dá permissão antes da que usa a permissão.

- Evidência: `phxvpn/provas/rol-descoberta/resultados.json`; commit `aba3610`
- Validado em: 24/09/2026

## [Arquivo que outro processo relê se troca inteiro, nunca se reescreve](cognicao_arquivo-relido-por-outro-processo_20260924_1340.md)

Arquivo que outro processo relê (config, CRL, lista de revogados) se grava num temporário e se troca por renomear. No Windows, tenta de novo por um prazo curto e, não conseguindo, dá erro — nunca volta a escrever por cima.

- Evidência: teste `gravar_troca_o_arquivo_inteiro_sem_meio` (phxvpn/src/painel.rs); `phxvpn/prova-openvpn.sh`
- Validado em: 24/09/2026

