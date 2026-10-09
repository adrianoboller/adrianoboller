# Camada nova antes da marca esconde a rede velha: a guarda de uma camada passa por engano

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-server/tests/commit-pelo-soquete.rs::o_commit_contra_a_tabela_congelada_nao_sai_pela_metade`; `ba65032e`

**Validação (08/10/2026):** guarda `commit-sem-as-duas-recusas-antes-da-marca` com veredito PROVADA em `bancada/guardas/ultima-corrida.json` (corrida de 08/10/2026 20:17): o provador repôs o defeito (as duas camadas tiradas), o teste caiu, e passa com o conserto. Promovido em 08/10/2026 pelo papel H; a hipótese (b) morreu e está na §3.

**Descoberto em 08/10/2026, 16:50**, pelo papel F, medindo o pedido 720.

## 1. O que aconteceu

A guarda `commit-sem-rede-antes-da-marca` (`bancada/guardas/catalogo.py`, 426)
dava NAO PEGOU desde 07/10/2026 14:39: tirando a rede do `preparar_a_marca`
(`servidor/servico_marca_01.rs`, `quantas() == 0` -> `< usize::MAX`), o teste
`o_commit_contra_a_tabela_congelada_nao_sai_pela_metade`
(`crates/phxsql-server/tests/commit-pelo-soquete.rs`) continuava verde.

## 2. O que eu concluí primeiro, e estava errado

As duas hipóteses escritas antes de medir eram (a) outra camada passou a
recusar antes da marca, ou (b) o teste deixou de alcançar o caminho
congelado. O verde com o defeito reposto, sozinho, não distingue as duas —
e (b) é a mais perigosa, porque deixaria a garantia do 426 sem prova. O
veredito igual («EM_MIGRACAO», transação ativa, segundo COMMIT grava 2) não
dizia QUEM recusou.

## 3. O que a medição disse

Quatro rodadas numa cópia da árvore, a resposta do COMMIT impressa:

| rede 426 | pré-conferência 448 | teste | quem recusou |
|---|---|---|---|
| liga | liga | ok | a rede (frase «e o COMMIT abriria essa tabela») |
| **tira** | liga | **ok** | a pré-conferência: EM_MIGRACAO 4006 SEM a frase da rede |
| liga | **tira** | ok | a rede (a frase volta) |
| **tira** | **tira** | **FAILED** | ninguém: 1 de 2 linhas no disco, o arranque aplica a outra |

Venceu (a); (b) morreu: o teste alcança o congelado, e cai quando nenhuma das
duas camadas recusa. A pré-conferência do 448 (`pre_conferir_a_lista`) roda
depois do `preparar_a_marca` e antes da marca, abre a `mae`, recebe 4006
(classe de acesso) e devolve a lista à transação — o mesmo desfecho da rede.

## 4. A regra

**Quando uma camada nova entrar antes do mesmo ponto que uma guarda velha
protege, reprove a guarda velha com a nova também tirada** — e, para saber
quem recusou, meça a FRASE da recusa, não o veredito.

## 5. Como está guardado hoje

`commit-sem-rede-antes-da-marca` passou a `espera: "nada muda"` (REDUNDANTE,
com a nota nomeando o 448), e nasceu `commit-sem-as-duas-recusas-antes-da-marca`,
que tira as duas e espera o teste cair.

**Onde o buraco ficou:** nenhum teste prova a rede do 426 SOZINHA (rede tirada,
pré-conferência ligada é verde por desenho). O `congelada_no_alcance` olha o
componente da chave inteiro, mais largo que o que a pré-conferência abre;
não medi se existe escrita cuja passada abre uma tabela que a pré-conferência
não abre — é a hipótese que sobra para dizer se a rede ainda compra algo.

**Alcance, não lei nova:** isto é o alcance de «teste que passa por engano» (papel F): a guarda de uma camada só vale com a camada vizinha também tirada.

**Medido depois (pedido 721, 08/10/2026):** matriz de 11 escritas numa cópia
do HEAD `a0325c80`, com mãe, filha e neta encadeadas por cascata. Com a rede, ela
recusa 11 de 11. Sem a rede, a pré-conferência recusa 8 de 11 com 0 gravado,
3 de 11 confirmam inteiras e 0 de 11 quebram depois da marca. A rede é segunda
camada, e não há escrita em que ela seja a única. Nos 3 casos ela recusa a mais
(com `repetir`) uma tabela que a passada nem abre.
