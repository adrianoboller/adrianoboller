# Fio sobrando vira busca, não bloco: o paralelo que não custa compressão

## 1. O que aconteceu

Pedido do dono: compactar com vários fios para entregar o `.7z` mais cedo.
Entraram duas técnicas: blocos LZMA2 independentes e a busca de casamentos
numa thread própria. O que decidiu entre elas foi a medição, e duas vezes ela
desmentiu o desenho que eu tinha escrito antes de medir.

## 2. O que eu concluí primeiro, e estava errado

**Primeiro:** que blocos bastavam, com o bloco no dobro do dicionário. Medido
em 21 MB com 4 núcleos, isso dava 8 MiB no nível 5, 5,03 s e +2,1%, **mais
lento** que o 7-Zip com 4 fios (4,67 s) e ainda maior. O 7-Zip ganhava até em
1,3 MB, onde não há bloco nenhum a cortar.

**Segundo:** depois de escrever a busca em fio, pus **sempre** dois fios por
bloco. Com blocos de sobra isso piorou: 4 blocos de 4 MiB em 4 fios levavam
3,23 s, e em 2×2 fios passaram a 4,04 s. A busca em fio rende 1,46×, e cortar
os trabalhadores pela metade custa 2×.

## 3. O que a medição disse

- A busca em fio sai **byte a byte igual** a um fio só, porque a árvore só
  muda na inserção e nunca na coleta. O primeiro teste de igualdade passou
  **sem alcançar** a regra difícil (a pergunta por posição já passada, depois
  de um pedaço cru). Ele virou um teste do contrato, pergunta a pergunta, que
  cai quando a regra é quebrada.
- Tamanho do bloco no nível 5: 1 MiB dá +10,5% e 5,5×; 4 MiB, +3,7% e 3,9×;
  8 MiB, +2,1% e 2,5×.
- Depois da política certa, com 4 fios: texto 1,6×, misto 2,8×, grande 2,6×.
  O 7-Zip com 4 fios ainda está 1,1 a 1,6× à frente.

## 4. A regra

Paralelo que muda os bytes, como o corte em blocos, só usa o fio que o
paralelo que não muda nada deixou livre. Primeiro se gasta o fio de graça;
depois o que custa compressão. E um teste de igualdade só prova alguma coisa
quando alcança o caminho difícil. Quando o dado não o alcança, o teste tem de
ser do contrato.

## 5. Como está guardado hoje

- `escritor.rs`, `em_blocos`: `busca_em_fio = nivel.arvore && partes.len() * 2 <= fios`.
- Testes: `a_busca_em_fio_da_os_mesmos_bytes_de_um_fio_so`,
  `a_fonte_em_fio_responde_como_o_buscador_ate_na_volta` (vermelho medido),
  `em_blocos_os_bytes_nao_dependem_dos_fios_e_voltam_inteiros` e
  `o_7zip_abre_o_que_foi_compactado_em_blocos`.
- Números: `bancada/phxzip/comparar-7z.json` e `docs/PHXZIP.md` §3d.
- Achado no caminho: o `DirTemp` da interoperabilidade usava só o pid, e dois
  testes com 7z apagavam a pasta um do outro. Agora o nome é por teste.
