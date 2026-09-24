<!-- GERADO por classificar.py a partir das cognições. Não se edita: mude a seção «## Estado» da cognição e rode o gerador. -->

# Reusar — sucessos comprovados

292 cognições: **9 frutíferas**, **4 infrutíferas**, **279 pendentes** (sem evidência validada — não entram aqui).

Só entra o que tem evidência que o `classificar.py` conferiu. Antes de desenhar, procure aqui o que já se provou.

## [Túnel que pinga não é túnel que se recupera](cognicao_tunel-que-pinga-nao-e-tunel-que-se-recupera_20260924_0010.md)

Protocolo com estado se prova também **reiniciando um dos lados no meio**. O caminho feliz com os dois nascendo juntos não exercita a perda de estado.

- Evidência: teste `par_surdo_dispara_aperto_novo` (phxvpn/src/p2p.rs)
- Validado em: 24/09/2026

## [Prova de troca de fio sem tráfego não vê a corrida do primeiro quadro](cognicao_troca-de-fio-sem-trafego-nao-ve-o-primeiro-quadro_20260924_0905.md)

Prova de troca de caminho roda **com tráfego no ar durante a troca**. Troca feita com o túnel parado só prova a ordem que o próprio nó escolhe; a ordem que a rede impõe (quem chega primeiro na conexão nova) só aparece quando outra thread tem pacote para mandar no mesmo instante.

- Evidência: `phxvpn/provas/tcp/resultados.json`; teste `conexao_nova_comeca_pelo_registro_mesmo_com_trafego` (phxvpn/src/fio.rs)
- Validado em: 24/09/2026

## [A lista de pares já perfura NAT cone — o farol só é indispensável no simétrico](cognicao_lista-de-pares-ja-perfura-nat-cone_20260924_0845.md)

Numa malha com um membro alcançável, conte a lista de pares como perfurador: controle negativo de NAT se faz com NAT **simétrico**, não com cone — com cone, o direto nasce da sincronia dos INICIOs, com ou sem mediador.

- Evidência: `phxvpn/provas/farol/resultados.json`; `phxvpn/provas/farol/rodar.sh`
- Validado em: 24/09/2026

## [Ligar o `ip_forward` para a LAN da empresa abre a rede A para a rede B](cognicao_ip-forward-abre-o-isolamento-entre-redes-vpn_20260924_1100.md)

Quem acende o encaminhamento do host acende JUNTO uma guarda que só deixa passar o par (origem da rede N, LAN da rede N) e descarta o resto do espaço da VPN — antes de escrever o `1` no `ip_forward`, e apagando os dois juntos.

- Evidência: `phxvpn/provas/rotas/resultados.json`; `phxvpn/src/rotas.rs`; `phxvpn/provas/rotas/rodar.sh`
- Validado em: 24/09/2026

## [A gerência do OpenVPN: `kill CN` não avisa o cliente; `client-kill` avisa](cognicao_gerencia-openvpn-kill-nao-avisa-o-cliente_20260924_0910.md)

Para derrubar um cliente do OpenVPN pela gerência, use `client-kill` com o CID do `status 2`, nunca `kill CN` — e prove a queda **no cliente**, não pela resposta `SUCCESS` do servidor.

- Evidência: `phxvpn/provas/mfa/resultados.json`; `phxvpn/src/credencial.rs`; `phxvpn/tests/postgres_real.rs`
- Validado em: 24/09/2026

## [Trocar o fio não reenvia o que já saiu pelo fio velho](cognicao_fio-novo-nao-refaz-o-aperto-pendente_20260924_0412.md)

Quando o caminho muda, refaça **tudo** o que está pendente no caminho velho, não só o que o próprio caminho controla: estado de aperto, fila e temporizador de reenvio moram fora do fio e não sabem que ele trocou.

- Evidência: `phxvpn/provas/tcp/resultados.json`; commit `8b35d60`
- Validado em: 24/09/2026

## [Cognição: difusão numa placa TUN — o Linux entrega, o TAP-Windows6 não origina](cognicao_difusao-na-placa-tun-linux-e-windows_20260924_1040.md)

Antes de mudar a placa para «consertar» a entrega, escreva o pacote no descritor e conte o que chega ao soquete — o kernel sabe mais do que o `IFF_*` sugere. E no Windows, o modo TUN do TAP é unicast para quem ORIGINA: difusão originada no Windows pede o TAP em modo Ethernet.

- Evidência: `phxvpn/provas/broadcast/resultados.json`; `phxvpn/src/difusao.rs`; `phxvpn/provas/broadcast/rodar.sh`
- Validado em: 24/09/2026

## [Broadcast na LAN: o limitado não sai sem rota, e o `ifa_broadaddr` mente sem `brd`](cognicao_broadcast-na-lan-sem-rota-e-sem-brd_20260924_0415.md)

Broadcast dirigido se **calcula** (endereço `|` `!máscara`), não se lê do `ifa_broadaddr`; o limitado vai junto, nunca sozinho. E quando duas mensagens de um lote dependem uma da outra, a ordem de envio é parte do protocolo — ponha a que dá permissão antes da que usa a permissão.

- Evidência: `phxvpn/provas/rol-descoberta/resultados.json`; commit `aba3610`
- Validado em: 24/09/2026

## [Arquivo que outro processo relê se troca inteiro, nunca se reescreve](cognicao_arquivo-relido-por-outro-processo_20260924_1340.md)

Arquivo que outro processo relê (config, CRL, lista de revogados) se grava num temporário e se troca por renomear. No Windows, tenta de novo por um prazo curto e, não conseguindo, dá erro — nunca volta a escrever por cima.

- Evidência: teste `gravar_troca_o_arquivo_inteiro_sem_meio` (phxvpn/src/painel.rs); `phxvpn/prova-openvpn.sh`
- Validado em: 24/09/2026

