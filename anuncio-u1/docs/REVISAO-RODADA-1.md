# Revisão da rodada 1 — o que os revisores mediram

Gerado do resultado do workflow `wf_ffcac9f6-44e`. Cada item traz gravidade, beat, o defeito e a correção proposta. Quem corrige lê o item inteiro; a proposta é ponto de partida, não ordem — se medir outra causa, corrige a causa medida e anota.


## Lente: storyboard — nota 7/10

**O que o revisor viu:** Abri os sete quadros-chave (540x960), os cinco extras (q001, q236, q350, q545, q590), tres quadros da sequencia parcial (q0009, q0019, q0029 em 360x640), previa_caixa_detalhe.png e previa_u1_tela_boot.png, e li mod_coreografia.py inteiro mais as funcoes animar_* dos modulos que ele chama. NAO ha video de previa (saida/previa_seq tem so 15 quadros, 1..29); rodei uma sonda numerica por quadro (posicao da camera, alvo, lente, z/x da tampa e hide_render, z do corpo da caixa e do U1) em 46 quadros e lancei dois lotes de render extra (q90,100,140,150,262,268,330,340,352,359 e q362,390,420,452,470,505,510,530,549,552) que ainda nao tinham terminado quando a resposta foi exigida - o julgamento de movimento abaixo vem da sonda e dos quadros existentes, nao desses renders. Beat 1: q001 e chao vazio (caixa a z=-1,07, some inteira; so no q009 a tampa comeca a aparecer - ~0,3 s de quadro vazio no abre); q0009/q0019/q0029/beat_1 mostram a caixa branca subindo e girando com a logo PLANA e impressa no topo da tampa (previa_caixa_detalhe confirma: e decalque, sem relevo); 2 voltas em 2,5 s desacelerando = media-rapida, ok. Beat 2: tampa sobe inclinada e sai para +X (q90 z=1,26, q100 x=1,49 ja fora do meio-campo horizontal de ~0,75 m; escondida no q103 - fora do quadro, sem pop visivel), espuma explode q95-151, beat_2 (q120) mostra dezenas de flocos e o topo do U1 dentro da caixa aberta; o U1 sobe a z=0,95 (q120-142), a CAIXA AFUNDA PELO CHAO (q131-158, z=-1,07) e o U1 pousa no chao na origem - o storyboard diz 'o U1 sai da caixa', nao que a caixa some. Beat 3: orbita frente->traseira em 50 quadros; beat_3 mostra a traseira (coluna com USB, botao vermelho, tomada IEC, acrilico), q236 mostra o plugue encaixado com o cabo caindo ao chao; botao afunda q246-268 e LED acende por chave de emissao (nao vi quadro do LED aceso). Beat 4: orbita de volta pela frente; beat_4 (q315) corta o U1 na borda esquerda com a tela apagada; boot no q322-345 (0,8 s) e UI no q346; q350 mostra a UI acesa ja legivel, mas a camera so chega ao fim do dolly no q359, que e o ULTIMO quadro antes do corte para a foto A - o close na tela nao segura. Beat 5: beat_5 (q405, 50 mm) e um close do canto da porta com puxador embaixo a direita, sem corpo inteiro, reflexo rose no vidro - fiel; foto A (cabecotes, 60 mm) e foto C (mesa vista de cima, camera a z=1,01 dentro da pegada do U1) nao foram olhadas. Beat 6: beat_6 (q480) plano geral, U1 flutuando sobre a caixa que voltou, espuma no chao, tampa reaparece no q497 a x=1,6 (fora do meio-campo de ~0,87 m) e fecha ate q510 - fiel; cabo some e tela apaga no corte (invisivel de frente). Beat 7: camera sobe ao eixo da logo (q530 em (0,-0,06,2,61) olhando reto para baixo, centrada), mergulha ate 12 cm da tampa (q549), q545 mostra a logo enchendo o quadro; a 'travessia' e um CORTE seco no q550 para camera limpa; beat_7 (q555) e a cartela entrando em fade e q590 mostra exatamente 'EnginePrint / qualidade excepcional / 13 unidades restantes / compre em engineprint.com.br' - com a linha 3 em cobre e o EnginePrint em bold, nao branco fino como a paleta. Fundo: preto com faixa rose-branca no horizonte em todos os beats largos, e preto->rose na cartela. Todos os sete beats existem; os desvios sao de grau, nao de ausencia.


### [AJUSTE] beat 4

O storyboard pede 'foca na tela; boot -> interface'. A chave final do dolly esta em q_fim-1 = 359 (CONSTANT) e o corte para a foto A e no q360: o enquadramento com a tela a ~69% do quadro existe por UM quadro. O boot (q322-345) e a entrada da UI (q346) acontecem com a camera ainda a 1,0-1,2 m da tela (sonda: q322 cam a (-0,35,-1,26,0,71), q345 a (0,31,-1,01,0,62)), onde a tela de 0,104 m ocupa pouco do quadro em 9:16 - o boot 'bem rapido' vai passar sem que se leia 'Snapmaker' e a barra.

**Como corrigir:** Em ROTEIRO[4], encerrar a orbita+dolly em ~0,55 e segurar o close de q(0,55) ate q_fim-1 (chave repetida, CONSTANT no ultimo quadro); mover 'boot' para ~0,60 e 'ui' para ~0,85 de modo que o fade do boot comece ja com a camera parada no close. Conferir com QUADROS=330,345,352,359 que a tela enche o quadro nos tres ultimos.


### [AJUSTE] beat 2

O storyboard diz 'o U1 aparece e sai da caixa'. Aqui o U1 sobe 0,95 m e a CAIXA afunda pelo chao (corpo a z=-1,07 no q158) enquanto o U1 flutua, depois o U1 pousa onde a caixa estava. A caixa desaparecendo pelo piso nao esta no texto do cliente; o motivo (orbita do beat 3 sem a caixa no caminho) esta bem documentado no cabecalho, mas e uma decisao de direcao que o cliente nao viu, e no beat 6 ela volta pelo chao do mesmo jeito.

**Como corrigir:** Apresentar ao Adriano como escolha explicita (com o quadro q150 renderizado mostrando a caixa metade afundada) antes de fechar; alternativa fiel ao texto: o U1 desliza para -Y e pousa na frente da caixa, e a caixa fica atras dele fora do raio da orbita (raio 1,25-1,5 m ja e menor que a distancia da tomada ate a caixa se ela recuar 1,0 m em +Y no proprio beat 2).


### [AJUSTE] beat 7

'Aproxima ate ATRAVESSAR a logo' virou um corte seco: a camera para a 12 cm da tampa (q549, z=0,94 para tampa a ~0,82) e no q550 ja esta a 4 m do outro lado, de costas, com a cartela em fade. Nao ha quadro em que a camera esta dentro/na superficie da logo, nem escurecimento progressivo - a passagem se le como corte, nao como travessia.

**Como corrigir:** Levar a chave 'baixo' ate normal*(-0,02) (dentro da tampa, com clip_start 0,01) em 3-4 quadros e chavear um veu preto (o mesmo plano do flash com emissao 0 e alfa 0->1) nos 2 quadros da entrada, para o preto do corte nascer da propria logo; a cartela entra do preto como hoje.


### [AJUSTE] beat 7

Texto da cartela e o exato, mas o estilo diverge da paleta da especificacao ('branco #FFFFFF, peso fino, tracking largo'): a linha 3 '13 unidades restantes' esta em cobre #C8641F e 'EnginePrint' em FreeSans Bold pesado (q590).

**Como corrigir:** Na coreografia, passar em p['cartela'] cor_destaque branca (ou linha_destaque=0) e fonte_forte apontando para uma fonte semibold/regular, deixando o cobre so na logo; ou registrar no relatorio ao cliente que o destaque em cobre e escolha e pedir aprovacao.


### [AJUSTE] beat 4

Meio da orbita de volta (beat_4, q315): o U1 esta cortado na borda esquerda e a tela apagada e pequena; e o quadro que representa o beat 'Tela' e nao mostra tela nenhuma. O alvo em q(0,30) e (0,0,0,42) com raio 1,9 e lente 35, e em q_orb o alvo ja e a tela deslocada - a transicao passa pelo canto do corpo.

**Como corrigir:** Chave intermediaria em q(0,30) com alvo no centro do corpo a 0,45 m de altura e raio 2,1 (ou fx=0,5 via _enquadrar), para o produto ficar inteiro no meio da orbita; renderizar q300 e q315 para conferir.


### [NOTA] beat 1

Os primeiros ~8 quadros (0,27 s) sao chao vazio: a caixa comeca a z=-1,07 e so a tampa aponta no q009. Num Reels o primeiro quadro e o que segura o dedo.

**Como corrigir:** Comecar com a tampa ja rente ao chao (profundidade = topo_tampa_z, nao +0,25) ou subir a curva s_z nos 3 primeiros quadros; conferir q001 e q005.


### [NOTA] beat 3

Nao vi quadro com o LED/botao aceso (a chave e q257-268) nem o plugue em voo legivel (o relato admite que e preto sobre chao preto). 'Botao liga - sem maos' so se prova olhando q262-268.

**Como corrigir:** Renderizar q262 e q268 (lote lancado nesta revisao em scratchpad/rev/) e conferir que a janela do botao e os LEDs da camara mudam de apagados para acesos; se o plugue nao se ler em voo, luz de preenchimento so entre q185-240.


### [NOTA] beat 5

So a foto B (beat_5) foi olhada. A foto C poe a camera a z=1,01 dentro da pegada do U1 (cam (-0,12,-0,15,1,01), alvo na mesa a z=0,19) olhando para baixo pelo topo aberto: precisa provar que ha 'produto no canto inferior direito' e nao so o interior da camara, e que o aro do topo nao corta o quadro.

**Como corrigir:** Renderizar q362, q390 e q420 (lote B desta revisao) e conferir os tres closes contra 'canto inferior direito, sem corpo inteiro'.


### [NOTA] beat 6

Cabo some e tela apaga por chave no corte q450; a sonda mostra a camera em (0,52,-2,95,1,30) olhando a frente, entao o cabo (atras, +Y) nao aparece - mas se o modelo real for maior ou a camera mudar, o cabo sumindo vira pop.

**Como corrigir:** Manter; registrar no cabecalho que a invisibilidade depende da camera do beat 6 estar em -Y.


## Lente: visual — nota 6/10

**O que o revisor viu:** Abri os sete beat_N.png, os extras (q001, 236, 350, 545, 590), 6 quadros da previa_seq (1..29) e renderizei por conta propria 27 quadros a 360x640/8 amostras com o codigo ATUAL (/tmp/claude-0/-home-user-adrianoboller/857c9466-78dd-51a3-8b8e-9f13227d5fd4/scratchpad/rev_visual/A e /B: q1,55,75,95,105,140,160,165,185,205,240,270,290,340,359 e q361,380,391,421,445,451,500,507,530,548,551,565), mais uma sonda de velocidade da camera quadro a quadro (m/quadro, no log A.log). Custo medido: 16-41 s/quadro, nao 6. O que a imagem mostra: (1) q001 NAO esta limpo - ha um ponto branco no horizonte (centro-direita) e uma faixa marrom mosqueada de ~8% da altura entre o rose e o chao preto (a fusao 6->30 m do chao infinito le como sujeira, nao como gradiente); quadro_001.png do relato ainda tem a barra branca inteira. (2) Beat 1: a caixa emerge atravessando o chao sem corte (ok como magica), gira e assenta; em q75 fica de frente morta (retangulo chato, sem 3/4). (3) Beat 2: em q140 o U1 branco flutua sobre a faixa rose clara - branco sobre rose, sem recorte de borda; e o momento-heroi e e o de menor contraste do filme. (4) q165: ao entrar o beat 3 o specular do rim salta de 0 para 0,5 por chave CONSTANT e aparece de um quadro para o outro uma cunha branca no chao a direita do U1 (ausente em q160) - um pop de luz no meio de um plano continuo. (5) q185: meio da orbita e um retangulo branco liso com um circulo (lateral do substituto) enchendo o quadro; com o rim atras vira um slab sem forma. (6) Beat 3: plugue voa preto sobre preto (q205 nao se le), encaixa em q236/240 e le bem; mas o 'liga' (q246-268) nao acontece visualmente - botao afunda 2 mm, LED/UI nao mudam nada legivel, e a camera fica parada (0,001 m/quadro) por 25 quadros. (7) Sonda: az 105 (q215) -> 100 (q270) -> 180: a orbita RECUA 5 graus e a camera para de vez em q269 (0,0005 m/quadro) por ser minimo local com handle auto-clamped - um solavanco no meio de um plano continuo. (8) Beat 4: a UI entra em q346 e a camera ACELERA de 0,023 para 0,062 m/quadro ate o corte (handle do q359 puxado pela chave da foto em q360): a tela nunca fica parada, e a 0,26 m com shutter 0,5 o borrao e de 1-3 cm sobre uma tela de 10 cm. (9) q359 e BRANCO: o flash tem chave alfa 0 em q359 e 1 em q360, e com o obturador START o quadro 359 expoe 359->359,5, ou seja, meio flash ANTES do corte; cada foto sao 2-3 quadros brancos estourados (emissao 16). (10) Foto A (q361/380): barra branca estourada atravessando o quadro (fita de LED) e um borrao branco vertical no centro-baixo (reflexo do rim a 550 W num cabecote + bloom); foto C (q421/445): faixa branca estourada no pe do quadro e cabecote desfocado invadindo a esquerda; a mesa fica centrada, nao ancorada embaixo a direita. Foto B (q391) e a melhor: vidro com reflexo rose, puxador, tela cortada - mas 60% do quadro e painel branco plano sem gradiente de luz. (11) Beat 6 (q451, 507): U1/caixa ocupam ~28% da altura, centrados; dois tercos do 9:16 sao preto vazio e o rose vira uma tira - o vertical nao e aproveitado. Tampa entra borrada em q500 e assenta em q507 - le. (12) Beat 7: q530 top-down na logo e elegante; sonda mostra a camera subir a 0,19 m/quadro, quase parar em q532 (0,016) e mergulhar em QUAD EASE_IN - stop-and-go no alto; q548 e um borrao cinza/laranja irreconhecivel (DoF f/2,8 + motion blur); corte em q550 cai num quadro VAZIO preto/rose (q551-553) com o horizonte invertido (rose embaixo, quando o filme inteiro teve rose em cima), e a logo so aparece em fade a partir de q555. (13) Cartela (q565/590): legivel no celular, hierarquia ok (marca > qualidade > laranja > url), mas FreeSans Bold pesada, tracking normal, e a linha 4 assenta sobre a transicao escuro->rose. Materiais em close convencem (chanfros, PEI texturizado, acrilico com reflexo rose, plugue com anel); ruido branco no acrilico traseiro a 8-16 amostras e pendencia conhecida. Gradiente sem banding visivel a 8 bits.


### [BLOQUEIA] beat 5

O flash comeca um quadro ANTES do corte: chaves alfa em q_a-1 (0), q_a (1), q_a+1 (0) com motion_blur_position START fazem o quadro q_a-1 expor de q_a-1 a q_a-0,5 com alfa subindo a 0,5 - q359 (ultimo quadro do close da tela) saiu branco (rev_visual/A/q0359.png). Alem disso cada foto sao 2-3 quadros brancos puros (emissao 16 x alfa 1), tres vezes em 3 s - agressivo no celular e o oposto do 'flash de foto' curto.

**Como corrigir:** Em animar_flash chamar com chaves (q_a, q_a+1, q_a+2) em vez de (q_a-1, q_a, q_a+1) - ou passar largura=1 e deslocar o pico para q_a+1 - de modo que nenhum quadro anterior ao corte expoe o veu. Reduzir o pico: forca=0.5 (emissao 8) e alfa 0 -> 1 -> 0.35 -> 0 em 4 quadros (decaimento), que le como flash sem branco solido.


### [BLOQUEIA] beat 3

Pop de luz em q165: _chave_rim_especular grava 0,5 com interpolacao CONSTANT no primeiro quadro do beat 3, e a cunha branca do rim no chao aparece de um quadro para o outro (ausente em q160, presente em q165, rev_visual/A/q0160.png vs q0165.png) enquanto o plano e continuo e o produto ainda nao a esconde.

**Como corrigir:** Subir o specular do rim so quando a camera ja passou de az ~0 (q_ini + 0.25*(q_orb-q_ini) ~ q178) e com rampa Bezier de 12 quadros (0 -> 0,5), nao CONSTANT; ou manter 0 ate q_orb (q215), onde o U1 cobre o reflexo. Guardar a lista de quadros CONSTANT so para os cortes reais (beat 5 e q550).


### [AJUSTE] beat 3

A camera para de vez e recua: azimute 105 (q215) -> 100 (q270) -> 180 (q297) faz q270 minimo local e o handle auto-clamped zera a velocidade (sonda: 0,0005 m/quadro em q269, 0,0146 em q272), com 5 graus de marcha a re antes de retomar. Junto com o 'liga' invisivel, sao ~25 quadros de imagem parada no meio de um plano que devia fluir.

**Como corrigir:** Fazer o azimute monotono: q_orb az 100 e q_fim az 112 (ou q_orb 105 e q_fim 115), e raio 1.5 -> 1.30 -> 1.9 sem inverter tao forte; alternativa: no q_fim do beat 3 usar handle_left/right_type 'AUTO' (nao clamped) so nesse par de chaves, que mantem velocidade residual de ~0,01 m/quadro. Meta: nenhum quadro fora de corte com velocidade < 0,005 m/quadro.


### [AJUSTE] beat 4

A tela nunca fica parada: a UI corta em q346 e a camera acelera de 0,023 (q344) para 0,062 m/quadro (q359) porque a chave CONSTANT em q_fim-1 tem o handle esquerdo puxado pela chave da foto em q360. A 0,26 m da tela com shutter 0,5, isso e 1 a 3 cm de borrao sobre uma tela de 10 cm - a UI (o ponto do beat) fica legivel ~0,3 s e em movimento (rev_visual/A/q0340.png ainda longe, q0359 ja no flash).

**Como corrigir:** Terminar o dolly em q(0.78) (~q340) e repetir a MESMA chave (az_fim, r_fim, z_fim, alvo tela) em q_fim-1 com interp CONSTANT: duas chaves iguais consecutivas zeram o handle e seguram a tela parada 19 quadros (0,63 s). Se quiser vida, drift de 0,004 m/quadro entre elas. Antecipar 'ui' para 0.72 para a UI ter ~0,9 s de tela.


### [AJUSTE] beat 5

Estouros nas fotos A e C: na A (q361/380) uma barra branca solida atravessa o quadro (fita de LED emissiva com bloom) e ha um borrao branco vertical no centro-baixo (reflexo do rim a 550 W num cabecote); na C (q421/445) uma faixa branca cega o pe do quadro e um cabecote desfocado invade a esquerda. Nada disso e 'iluminacao cinematica'; e clip. A mesa da foto C fica centrada, nao no canto inferior direito como o storyboard pede.

**Como corrigir:** Foto A: e_rim 550 -> 250 e checar o objeto do borrao vertical (render com rim escondido; se sumir, e ele - baixar rim.especular para 0,2 nas fotos). LED: forca de emissao da fita <= 3 nas fotos, ou excluir do bloom (limiar 2,5 -> 4). Foto C: camera de s['mesa'] + (0.10, -0.22, 0.80) com fx=0.70/fy=0.78 no _enquadrar para ancorar a mesa embaixo a direita e tirar o cabecote desfocado da esquerda; e_rim 800 -> 350.


### [AJUSTE] beat 7

O corte da travessia cai num quadro vazio: q548-549 e um borrao cinza/laranja irreconhecivel (DoF f/2,8 a 0,15 m + motion blur), q550-553 e preto com rose embaixo sem nada, e a logo da cartela so ganha corpo por volta de q560. O match-cut logo -> logo nao existe na imagem, e o horizonte da cartela fica INVERTIDO (rose embaixo) em relacao aos 17 s anteriores (rose em cima) - o corte vira o mundo de cabeca para baixo.

**Como corrigir:** (a) No mergulho, chavear aperture_fstop 2.8 -> 8 entre q_topo e q_t-1 e usar interp BEZIER EASE_IN em vez de QUAD para chegar mais devagar nos ultimos 6 quadros (0,06 m/quadro), para a logo ler nitida ate o quadro do corte. (b) Comecar a cartela com a logo ja em alfa 1 e centrada no quadro em q_t (mesmo tamanho aparente da logo no ultimo quadro do mergulho), e so entao subir para a posicao final enquanto o texto entra - match-cut real. (c) Camera da cartela olhando 24 graus para BAIXO com a faixa rose do gradiente no terco de cima (ou virar a cartela e a camera 180 graus no eixo optico) para o rose continuar em cima; se o bloco cair sobre o brilho, baixar 'forca' do World para 0,5 so nesse corte.


### [AJUSTE] beat 6

O 9:16 nao e aproveitado nos planos largos: em q451 e q507 o U1/caixa ocupam ~28% da altura, centrados, com dois tercos de preto vazio embaixo e o rose reduzido a uma tira; o mesmo acontece em q75 (beat 1, caixa a 40% e de frente morta). O olho de anuncio pede o produto grande, sentado no terco inferior, e o brilho do horizonte atras dele.

**Como corrigir:** Beat 6: raio 3.0/2.8 -> 2.2/2.1, z 1.3/2.3 -> 1.0/1.7, alvo z 0.55/0.70 -> 0.45/0.60 (o topo da subida do U1 a 0,95+0,73 m ainda cabe com 35 mm vertical de 54 graus a 2,2 m). Beat 1: acabar a orbita da caixa em -80 graus (nao -90) para um 3/4 leve em q75, e raio 2.6 -> 2.3 na chave final.


### [AJUSTE] beat 2

No momento-heroi (q140-160) o U1 branco flutua sobre a faixa rose clara: branco sobre rose-branco, sem recorte de aresta (rim com specular 0 e rig a 15 graus). E o quadro de menor contraste do filme justamente na revelacao do produto.

**Como corrigir:** Subir o alvo e a camera menos (z_alto + 0.30 -> z_alto + 0.05 e camera z 1.6 -> 1.25) para o U1 recortar contra a transicao escura, ou deixar o rim com specular 0,3 e rig a 30 graus so entre q(0.50) e q(0.90) do beat 2 (o chao esta coberto pelo U1 e pela caixa nesse trecho, entao a cunha nao aparece - conferir com um render de q140).


### [AJUSTE] beat 1

Primeiro quadro do anuncio (rev_visual/A/q0001.png, codigo atual) ainda tem um ponto branco no horizonte (centro-direita) e uma faixa marrom mosqueada de ~8% da altura entre o rose e o preto (fusao 6 -> 30 m do chao infinito lida em angulo rasante como textura suja). O quadro_001.png do relato mostra a barra branca inteira e nao bate com 'chao vazio limpo'.

**Como corrigir:** Ponto branco: render de q1 escondendo top e key um por vez (o rim ja esta em 0) e zerar o specular da luz culpada nos beats 1-2 como se fez com o rim. Faixa mosqueada: fusao_chao (6, 30) -> (3, 12) e rugosidade_chao 0.45 -> 0.6 no trecho fundido, ou desligar o bump do chao acima de 3 m (o brilho do horizonte deve ser liso). Regenerar quadro_001.png e conferir.


### [AJUSTE] beat 3

O 'liga' nao se ve: o botao afunda 2 mm (invisivel a 1,3 m), o LED e um ponto laranja de 2 px, a tela esta do outro lado e nada mais muda de q246 a q268 (rev_visual/A/q0270.png igual ao q0240 salvo o cabo). Em anuncio, ligar e um evento de luz.

**Como corrigir:** Ao chavear o botao, acender o interior: fitas de LED do U1 (u1.led.N) com Emission Strength 0 -> 4 em 6 quadros a partir de q(0.90), visiveis pelo painel acrilico traseiro; e um leve push-in da camera (raio 1.25 -> 1.15 entre q(0.77) e q_fim) para o plano nao ficar parado. Se o modelo real nao tiver LEDs, pelo menos uma area light interna 20 W ligada por chave.


### [NOTA] beat 7

Tipografia da cartela nao e 'Apple': EnginePrint em FreeSans Bold pesada, tracking praticamente normal (0,05 em), e a linha 4 assenta sobre a transicao escuro -> rose em q590 (branco sobre rosa claro, contraste cai). Legivel no celular, hierarquia funciona.

**Como corrigir:** Peso: usar a fonte fina (FreeSans regular) tambem para a marca, com tamanho maior (x1.3) em vez de bold; tracking final 0.05 -> 0.12 em nas linhas 2-4 e 0.08 na marca. Linha 4: cartela_subida 0.13 -> 0.18 ou reduzir a forca do World no corte para o horizonte cair abaixo de 80% da altura.


### [NOTA] beat 3

Meio da orbita (q185): lateral do substituto e um slab branco liso com um circulo enchendo 70% do quadro, sem aresta nem gradiente - com o rim exatamente atras, a face fica plana. No modelo real a lateral tem outro desenho, mas a luz plana continua.

**Como corrigir:** Rig de luz durante a orbita com offset de +60 graus em vez de +90 (mod_ambiente.animar_rig 15 -> 195 vira 15 -> 165), para a key varrer a lateral e criar gradiente na face; e raio da orbita 1.5 -> 1.7 em q_orb para a lateral nao encher o quadro.


### [NOTA] beat 5

Fotos quase estaticas: drift de 0,0014 m/quadro (4 cm/s) e imperceptivel entre q361 e q380; as tres fotos leem como stills de 1 s.

**Como corrigir:** Push-in lento em cada foto: deslocamento do drift na direcao do sujeito de 0,06 m em 30 quadros (0,002 m/quadro) mais lente 50 -> 52 mm chaveada (LINEAR) - vida sem perder a ideia de 'foto'.


## Lente: tecnica: compatibilidade e robustez do arquivo unico (anuncio_u1.py), caminho do modelo real, API 4.2+/5.0, motor, imports, custo de render — nota 6/10

**O que o revisor viu:** Li a ESPECIFICACAO inteira, mod_coreografia.py, montar.py, teste_coreografia.py e os trechos sensiveis a versao dos outros modulos. Regenerei o arquivo unico com o montar.py num caminho meu e ele e byte a byte igual ao scripts/anuncio_u1.py (em sincronia). Rodei o arquivo unico EU MESMO no Blender 4.2.5 daqui, via exec do texto sem __file__ e sem o sys.path do projeto (como a aba Scripting), na cena de fabrica: removeu Cube/Light/Camera, construiu em 5,9 s, coreografou 600 quadros, conferiu colisoes (0/0/0), gravou /root/anuncio_u1.blend (9,8 MB); rodei DE NOVO na mesma cena: 272 objetos, 8 colecoes, 5 imagens, 253 malhas nas duas rodadas, nenhum objeto '.001' fora da espuma - so os 3 materiais da caixa vazam. Sonda no .blend gravado: engine BLENDER_EEVEE_NEXT, 1080x1920, 30 fps, 64 amostras, AgX + 'AgX - Medium High Contrast', use_raytracing True, motion blur ligado com shutter 0,5 e render.motion_blur_position = START (a propriedade existe em render no 4.2.5, nao em eevee), shadow 4/8, camera da cena = camera.principal com DoF no Empty camera.foco f/2.8, compositor RLayers -> Glare(BLOOM) -> Composite, mundo ambiente.mundo, 15 objetos com hide_render animado; imagens NAO empacotadas (apontam para a pasta temporaria), fontes empacotadas. Caminho do modelo real: testei com um bloco girado como OBJETO (rotacao cancelada, tela/pontos por heuristica funcionam na primeira rodada) e como COLECAO com Tela/Botao/Led nomeados (tela, botao e LED animados; objeto extra do cliente sobrevive) - mas a sequencia substituto -> real funde os dois sob a mesma raiz e a rodada repetida com nome de objeto dobra a rotacao (detalhado no bloqueio). Abri os sete beats: beat_1 caixa branca com logo centrada sobre chao escuro e faixa rose, sem barra branca; beat_2 caixa aberta, dezenas de flocos com motion blur, aro preto do U1 dentro; beat_3 traseira com coluna branca, botao vermelho, tomada IEC, janela acrilica com mesa dourada e cabecotes, espuma no chao, chuvisco branco no acrilico e no vidro (16 amostras); beat_4 U1 em 3/4 com wordmark, tela apagada, porta de vidro escura, cortado na borda esquerda; beat_5 canto da porta refletindo o rose, puxador embaixo a direita, UI acesa cortada no topo; beat_6 plano geral alto, U1 borrado flutuando sobre a caixa, espuma espalhada; beat_7 logo e 'EnginePrint' quase apagados entrando em fade, brilho rose so no terco de baixo. Como nao ha video (previa_seq tem 7 quadros do beat 1), rendi 8 quadros extras num script meu: 359 quase branco com a UI fantasma (flash vazando um quadro para tras, causa medida nas fcurves: alfa LINEAR 0->1 entre 359 e 360 sob obturador START), 360 branco puro, 361 close dos cabecotes com faixa de luz clara, 449 close da mesa PEI com hastes, 450 corte limpo para o plano geral com espuma no chao, 451 igual, 505 tampa inclinada pousando com espuma dentro da caixa, 545 logo enchendo o quadro borrada. Nada foi editado nem commitado; tudo que escrevi esta no scratchpad (rev/).


### [BLOQUEIA] beat 0

O caminho do MODELO REAL nao e idempotente nem sobrevive ao fluxo que o proprio cabecalho convida ('ajuste os parametros e Run Script'). Provado rodando o arquivo unico tres vezes na mesma cena (scratchpad/rev/real_duas.py): (1) rodada com U1_NOME='' constroi o substituto; (2) rodada com U1_NOME='MeuU1' REAPROVEITA o Empty 'u1.raiz' do substituto com os 174 filhos dele ainda parenteados - o envelope medido virou 0,884 x 0,958 m (substituto + bloco somados), disparou 'nao cabe na caixa', o modelo foi centralizado contra a caixa combinada (x = 0,317) e os dois renderizam sobrepostos; (3) rodada de novo com o mesmo nome e U1_ROTACAO_Z = -30 leu obj.matrix_world JA COZIDO como 'original' e aplicou a rotacao outra vez: rot_z passou de 0,0 para -30,0 graus - a frente deixa de apontar para -Y, o plugue entra na face errada e a camera do beat 4 olha o lado. Com o nome de COLECAO a segunda rodada so funciona por acidente (os filhos ja parenteados na raiz caem fora de 'fontes' porque a raiz foi linkada na colecao dele).

**Como corrigir:** Em _u1_real: (a) se o modelo nao estiver na colecao 'u1', chamar mod_u1.limpar_colecao('u1') antes de reaproveitar/criar a raiz, ou criar a raiz sempre nova depois de desparentear o que houver nela; (b) guardar a matriz ORIGINAL de cada objeto do cliente numa propriedade customizada na primeira rodada (obj['anuncio.matriz_original'] = matriz achatada) e, nas seguintes, partir dela em vez de matrix_world - assim rz e a centralizacao sao aplicadas uma vez so; (c) na coleta de 'fontes' para colecao, excluir a raiz de 'todos' para os filhos dela continuarem entrando. Provar com o mesmo teste: tres rodadas, rot_z e envelope iguais nas tres.


### [AJUSTE] beat 0

As tres imagens (logo, tela_boot, tela_ui) NAO sao embutidas no .blend gravado: em /root/anuncio_u1.blend elas apontam para '//../tmp/anuncio_u1_assets/*.png' (packed_file = False, medido com sonda_api.py). No Windows isso e %TEMP%\anuncio_u1_assets; limpeza de temporarios, reinicio ou abrir o .blend em outra maquina/render farm deixa logo da caixa e telas rosa. As fontes sao empacotadas (FreeSans packed = True); as imagens, nao - mod_caixa so faz img.pack() na logo provisoria procedural.

**Como corrigir:** Depois de bpy.data.images.load(...) em mod_caixa (linha ~372), mod_u1 (~471) e mod_cartela (~306), chamar img.pack() (sao ~140 kB no total); ou empacotar tudo no main() antes do save: for img in bpy.data.images: if img.filepath and not img.packed_file: img.pack().


### [AJUSTE] beat 5

O flash VAZA para o ultimo quadro do plano anterior. O quadro 359 (fim do dolly na tela, o payoff da UI) saiu quase branco com a UI mal visivel (render em scratchpad/rev/quadros/q_359.png), embora a chave 'alfa' em 359 seja 0,0. Causa medida: animar_flash grava alfa 0 -> 1 -> 0 em 359/360/361 com interpolacao LINEAR, e o motion blur com obturador 0,5 em START avalia o material dentro de 359,0..359,5, onde alfa ja vale 0,25 x forca 16 = radiancia 4 (branco no AgX). O mesmo acontece em 389 e 419. A chave CONSTANT da camera evita o borrao entre planos, mas a rampa do material nao.

**Como corrigir:** Em animar_flash, deixar a fcurve do alfa CONSTANT (0 ate 359, 1 em 360, 0 em 361) em vez de LINEAR, ou gravar a chave 0 em quadro-1 e a de 1,0 em quadro com interpolacao CONSTANT so nessas duas; conferir rerenderizando 359/360/361: 359 tem de mostrar a UI limpa.


### [AJUSTE] beat 0

Blender 5.0: configurar_render de mod_ambiente escreve cena.use_nodes = True dentro de _bloom (protegido por try/except) MAS o proprio handler do except e o ramo else escrevem cena.use_nodes = False sem protecao. No 5.0 o compositor passou para Scene.compositing_node_group e Scene.use_nodes/Scene.node_tree sairam da API; a excecao nasce no handler e sobe ate main(), que aborta na ultima etapa (render configurado pela metade, .blend nao gravado). O resto da API sensivel esta coberto: BLENDER_EEVEE_NEXT -> BLENDER_EEVEE (TypeError), surface_render_method/blend_method, use_auto_smooth/set_sharp_from_angle, propriedades do Glare em 4.5+, look AgX, motion_blur_position (existe em render no 4.2.5, medido: START aplicado).

**Como corrigir:** Trocar os dois cena.use_nodes = False por _ajustar(cena, 'use_nodes', False) e, em _bloom, obter a arvore com getattr(cena, 'node_tree', None) ou, se existir compositing_node_group, criar um bpy.data.node_groups.new(type='CompositorNodeTree') e atribui-lo; sem conseguir, seguir sem bloom (o print ja existe). Nao da para provar aqui (so ha 4.2.5), entao o try/except tem de cobrir o fallback tambem.


### [AJUSTE] beat 0

Video de previa nao entregue (saida/previa_seq tem 7 quadros, todos do beat 1 com a caixa sob o chao); o movimento entre beats, a suavidade da orbita, os tres cortes e o fechar da tampa nao foram vistos por quem entregou. Rendi 8 quadros extras (359/360/361, 449/450/451, 505, 545 a 360x640/8 amostras): o corte 449 -> 450 e limpo (close da mesa, depois plano geral sem mistura), 505 mostra a tampa inclinada pousando com a espuma dentro, 545 e a logo enchendo o quadro - so o 359 esta errado (item do flash).

**Como corrigir:** Concluir lotes.sh + MODO=video e olhar; enquanto isso, ao menos renderizar os quadros q-1/q/q+1 de cada corte (359, 389, 419, 449, 543) e as transicoes 75, 165, 270, 450, 510 antes de declarar a coreografia pronta.


### [NOTA] beat 4

animar_tela com material do cliente chaveia o PRIMEIRO no com forca de emissao que encontrar, na ordem de nt.nodes. Provado (scratchpad/rev/real_completo.py): material com Emission ligado ao Output e um Principled BSDF sobrando desligado -> a chave foi em nodes['Principled BSDF'].inputs[27] e a tela nunca acende. O mesmo padrao esta em animar_botao para o LED.

**Como corrigir:** Partir do no Material Output: seguir o link de Surface e chavear a forca do no que esta de fato conectado (Emission.Strength ou Principled.Emission Strength); so cair na varredura se nao houver link.


### [NOTA] beat 0

Idempotencia do substituto esta boa (272 objetos, 8 colecoes, 5 imagens nas duas rodadas, nenhum '.001' fora da espuma), mas cada rodada vaza 3 materiais da caixa (33 -> 36: caixa.espuma.001, caixa.papel.001, caixa.papel_tampa.001; os antigos ficam orfaos e os objetos passam a usar os nomes .001) porque _material_papel usa materials.new sem reaproveitar; a colecao 'Collection' de fabrica fica vazia na cena. Alem disso, se a colecao do modelo do cliente se chamar 'u1' e U1_NOME ficar vazio, limpar_colecao('u1') APAGA o modelo dele.

**Como corrigir:** Na caixa, materials.get(nome) e remover/reconstruir os nos como o mod_u1 faz; no main, avisar (ou recusar) se existir colecao 'u1' que nao seja a do substituto quando U1_NOME == ''.


### [NOTA] beat 0

Imports: o arquivo unico usa 'import sys' e 'import types' (stdlib, sempre presentes no Blender) alem da lista da especificacao (bpy/bmesh/math/mathutils/random/numpy/base64/os/tempfile) - inofensivo, mas fora da regra; 'from mathutils import noise' e numpy so em funcoes. Cena do cliente: objetos dele fora de ANUNCIO (chao, luzes, mundo) continuam renderizando sem aviso - o mundo e substituido por ambiente.mundo, mas luzes e malhas dele entram no quadro. Custo estimado (nao medido, sem GPU aqui): 43.158 poligonos avaliados, 5 imagens, 64 amostras com raytracing + DoF + motion blur 1 passo + sombras 4/8 -> na RTX 4050 algo entre 2 e 5 s por quadro em 1080x1920, 20 a 50 min para os 600 quadros; a VRAM de 6 GB sobra. Por software aqui foram ~16 s/quadro a 360x640/8 amostras, coerente com o relato.

**Como corrigir:** Trocar sys/types por o que ja existe (types.ModuleType pode virar type(sys)(nome) - ou aceitar e documentar a excecao na especificacao). No main, listar objetos visiveis para render fora de ANUNCIO e do modelo do cliente e avisar. Medir o tempo real na 4050 com 3 quadros (1, 218, 405) antes de prometer duracao.


## Pendências declaradas pela própria coreografia

- VIDEO DE PREVIA NAO ENTREGUE: a cadeia esta pronta (lotes.sh renderiza 1..599 de 2 em 2 a 360x640/8 amostras em processos < 10 min; MODO=video do teste_coreografia.py junta saida/previa_seq/*.png num MP4 a 15 fps pelo VSE/ffmpeg do Blender), mas cada quadro custa ~16 s por software (nao os 6 s previstos: cena completa com booleans, DoF e raytracing), ou seja ~100 min para 300 quadros; o loop foi reiniciado tres vezes por correcoes de luz e estava no primeiro lote quando o relatorio foi exigido. Para concluir: bash <scratchpad>/lotes.sh e depois MODO=video bash scripts/previa.sh scripts/teste_coreografia.py -> saida/previa_20s.mp4.
- So os quadros-chave e os extras foram OLHADOS; a suavidade da camera entre beats, os 3 flashes/cortes do beat 5, a tampa voltando (q497-510), o clique do plugue em sequencia e a travessia no q550 so se provam no video.
- O plugue em voo (beat 3) e preto sobre chao preto e le mal ate encostar na coluna branca; se incomodar, uma luz de preenchimento so para o beat 3 ou origem do voo mais alta (param 'origem' de animar_conexao) - a decidir olhando o video.
- Beat 4: o quadro de meio de orbita (q315) corta o U1 na borda esquerda; e transicional, mas pode merecer alvo mais centrado no corpo ate q_orb.
- A luz muda no corte do beat 6 (rig de luz e specular_factor do rim voltam ao padrao) - e um corte, mas se aparecer 'pulo' de luz no video, chavear com 2-3 quadros de rampa.
- Ruido a 16 amostras no acrilico traseiro e no vidro (pontos brancos no beat 3): pendencia do mod_u1, 64+ amostras no final.
- A camada de cima de espuma afunda 5-9 mm no topo do U1 no repouso (pendencia do mod_caixa, z_min = e + uz + 0.7*raio); invisivel a 2,5 m, apareceria num close do beat 2.
- Espuma fica espalhada no chao durante os beats 3-5 (consequencia do storyboard); se o Adriano quiser chao limpo nos closes, esconder os flocos por chave entre q165 e q450.
- Cabo e escondido no corte do beat 6 (o modulo cabo nao acompanha o U1 subindo); o U1 volta para a caixa sem cabo e com a tela apagada por chave direta no no 'ligada'.
- Cartela: 'EnginePrint' em FreeSans Bold aqui (pesado); no Windows do cliente a lista prefere Segoe UI Semibold. Linha 4 fica a ~68% da altura; cartela_subida (0,13) e o ajuste.
- Preset de 15 s (DURACAO_S = 15, fator 0,75) so foi conferido numericamente (tudo escala por fracao de beat, inclusive as folgas anti-colisao); nao renderizado.
- Modelo real: heuristica de pontos e por fracao do envelope; se o modelo tiver a tela em outro canto, passar U1_TELA. Com colecao chamada 'u1', a raiz e linkada nela mesma.
- scripts/__pycache__/ nasce a cada import pelo Blender; convem ir para o .gitignore antes do commit unico (nao editei arquivo fora do meu namespace).
- Parametros do ambiente alterados por param (nao por edicao do modulo): rim.especular 0,5 + specular_factor do rim chaveado a 0 nos beats 1-2 e 6-7 (medido: reflexo do painel de 350 W em Fresnel rasante, ~100x acima do branco, so o zero apaga). motion_blur_position = START e shadow_ray_count/step_count 4/8 setados em configurar_render da coreografia.
- Nada foi commitado nem enviado (regra do modulo).

## Ressalvas de módulo (ajustes que os revisores de módulo deixaram)


### caixa

- **Logo pastel: cinza da engrenagem rende (136,137,141) contra (56,58,62) na fonte; laranja (232,166,128) contra (201,101,32). A metade escura da marca vira cinza-medio. Medido: com Sheen Weight 0 na tampa o cinza cai para 112 e o laranja para (234,156,106) - o Sheen branco (tint padrao branca) sobre tinta escura e uma causa concreta, nao so o AgX.** → Em _material_papel: (1) ligar a mascara da logo a um Math MULTIPLY_ADD no 'Sheen Weight' (papel 0.35 -> tinta ~0.05) e no 'Specular IOR Level' (papel 0.35 -> tinta ~0.2), do mesmo jeito que ja faz com Roughness; (2) setar bsdf.inputs['Sheen Tint'] = cor do papel (dentro do mesmo try/except do Sheen) para o sheen nao branquear; (3) trocar o HueSaturation por um MixRGB MULTIPLY leve ou baixar 'Value' para ~0.85 na tinta, porque saturacao 1.3 nao devolve o escuro do cinza. Reconferir medindo os pixels do render contra a fonte.
- **Variante 'escura' (#141416) nunca foi renderizada pelo construtor; rendi: o papel sai cinza medio (#6C6B6C) e a metade escura da logo fica mais clara que a caixa - a marca perde metade e a caixa nao le como preto premium.** → Para cor == 'escura': Sheen Weight <= 0.1 com Sheen Tint = base color, Specular IOR Level ~0.25, Roughness 0.7. Para a logo sobre papel escuro, expor param 'logo_escura' (arquivo alternativo, versao clara da marca) ou, na falta dele, clarear a tinta pela mascara (MixRGB SCREEN com cinza claro) - decisao de marca, mas o modulo precisa de um caminho. Acrescentar um quinto quadro no teste com a variante escura.
- **Quatro flocos (caixa.espuma.030, .047, .044, .058 - todos do vao lateral +Y, repouso y=0,27) saem ATRAVESSANDO a parede do corpo entre z 0,59 e 0,79 em vez de sair pela boca (medido quadro a quadro). No teste a parede +Y esta escondida da camera; em qualquer orbita do beat 2 aparece floco brotando do lado da caixa.** → Em _trajetoria_espuma, zerar o deslocamento horizontal enquanto z < topo do corpo + raio: calcular w so a partir do instante em que z ultrapassa objs['exterior_corpo'][2] + raio (interpolando u a partir dali), ou testar por quadro e reter xy = ini.xy ate a saida pela boca. Reproduzir a medicao (transicao dentro->fora da pegada interna com z abaixo do topo) como assert no teste.
- **Idempotencia parcial: objetos e colecoes nao duplicam (67/66/2 nas duas rodadas), mas cada construir_caixa cria materiais novos (caixa.papel.001, caixa.papel_tampa.001, caixa.espuma.001) e deixa 65 actions orfas. No Blender do cliente, rodar o script varias vezes na aba Scripting acumula lixo no .blend.** → Em limpar_colecao, alem das malhas, remover obj.animation_data.action quando users == 0 antes de apagar o objeto; e ao final remover bpy.data.materials cujo nome comece com 'caixa.' e users == 0 (ou reaproveitar por nome com bpy.data.materials.get em _material_papel/_material_espuma). Acrescentar ao teste uma segunda chamada de construir_caixa com contagem de materiais e actions.
- **_caminho_asset depende de __file__: na aba Scripting do Blender do cliente com texto nao salvo, __file__ nao aponta para o disco, o PNG nao e achado e a logo vira o quadrado provisorio em silencio (so um print). O entregavel final e exatamente esse caso.** → Em _caminho_asset, tentar em ordem: caminho absoluto em params; variavel de modulo RAIZ_ASSETS que o arquivo final/coreografia preenche; os.path.dirname(bpy.data.filepath)/assets; e so entao __file__. Se cair no provisorio, alem do print, gravar objs['logo_provisoria'] = True para a coreografia poder avisar na tela.
- **Flocos da camada de cima afundam no topo do U1: z_min = e + uz + raio*0.7 poe a base do floco de 4,5 a 9 mm abaixo do topo do U1 (0,738). Se o beat 2 mostrar o U1 emergindo com espuma ainda em cima, os flocos aparecem enterrados no painel superior.** → Usar z_min = e + uz + raio (nao 0.7*raio); a camada 0,738-0,808 tem 7 cm e comporta os flocos de ate 6 cm. Nos vaos laterais, idem: x = sinal*(ux/2 + max(raio, vao/2)) para o floco nao encostar no U1.

### u1

- **O cortador do vao da porta (u1.cortador.porta.vao) e filho de u1.raiz, nao de u1.porta. Com porta_aberta_graus diferente de 0 a moldura gira e o furo fica parado no lugar: o boolean corta a porta no lugar errado e o vidro (que e filho da porta) sai da moldura. Confirmado na sonda headless: com 60 graus a porta avaliada vira uma malha de 102 vertices deslocada para -Y sem o vao acompanhando. Com o padrao 0 nada aparece, por isso nao bloqueia.** → Criar o cortador com pai=None e, como se faz com o vidro e o puxador (linhas 731-738), fazer cortador.parent = porta e cortador.location = Vector((porta_l/2, 0, 0)) depois de mover a origem para a dobradica - assim ele gira junto. Acrescentar no teste uma construcao com porta_aberta_graus=45 e conferir que o vidro continua dentro da moldura (bbox do vidro contido no bbox da porta).
- **No close da UI o preto da interface rende como cinza-ardosia azulado: o Principled soma o reflexo especular (Specular 0,5, roughness 0,03) sobre a emissao, e a tela ligada continua espelhando a luz. Num anuncio estilo Apple o preto da tela precisa ser preto.** → Em _mat_tela, ligar 'Specular IOR Level' a um Math que o reduz quando 'ligada' sobe (ex.: 0,5 * (1 - 0,7*ligada)) e/ou subir o multiplicador de emissao de 4 para 6-8 so no ramo ligado; conferir no previa_u1_tela.png que o fundo da UI sai abaixo de ~0,05 no PNG.
- **Na frente 3/4 o aro preto do topo le como faixa clara e o topo inteiro como bandeja branca: com o aro apenas 2 mm acima do casco e roughness 0,55 a luz rasante clareia o preto, e nada da mecanica aparece pelo vao. O U1 real tem a moldura preta do topo bem visivel e os cabecotes a mostra.** → Deixar o aro mais alto (8-10 mm acima do casco) e mais escuro/menos especular (base #0C0D10, Specular 0,3), e subir os cabecotes/viga uns 2-3 cm para a trava laranja aparecer acima do labio numa 3/4 alta. Conferir olhando previa_u1_frente.png: a moldura tem de ler preta.

### ambiente

- **A metade escura do gradiente e MARROM-LARANJA, nao rose nem preto. A cor do world e saturar(#F4E6E4, 6,0) x 1,8 = (2,5, 1,18, 1,0) em linear - uma luz vermelho-alaranjada; no pico o AgX a empalidece para o rose medido, mas em tudo que e mais escuro (a transicao no 3q, linhas 240-330: #8A5E55, #462B26; o chao inteiro do 3q, #1C100D; a face inferior do cubo no meu experimento, #644139) a saturacao sobrevive e vira marrom. No 3q isso le-se como um horizonte de entardecer, e o beat 3 (orbita) e o beat 5 (fotos) vao mostrar exatamente esse angulo.** → Em _mundo, trocar o ShaderNodeMix (preto -> rose saturado) por um ColorRamp de tres paradas: 0,0 = #050507; ~0,5 = o rose com saturacao baixa (saturar(cor, ~1,5)) escurecido para ~40% da luminancia; 1,0 = o rose saturado atual. Assim a compensacao de saturacao que o AgX exige fica so no extremo claro, e a descida passa por um cinza-rose neutro em vez de por um laranja escuro. Expor as tres paradas em PARAMS_PADRAO e medir a cor da linha 270-330 do 3q no teste (criterio: saturacao (max-min)/max < 0,35 nessa faixa).
- **Na camera de cima o reflexo do rim no chao e uma cunha branca grande (L ate 212/255) no canto superior direito, com listras diagonais tenues dentro dela - compete com o produto e parece um farol no chao. O beat 1 (caixa sobe, camera alta) e os planos de cima do beat 5 vao mostrar isso.** → Nao ha light linking no EEVEE, entao a solucao e por geometria/params: (a) baixar o padrao de rim.especular para ~0,35 e recuperar o recorte da aresta subindo a energia do rim (o difuso nao entra na cunha; o teste ja mede a aresta do cubo); ou (b) adicionar a PARAMS_PADRAO um 'rim_alto' alternativo (z ~2,2, mais longe) que a coreografia troca via animar/params nos planos de cima. Documentar no cabecalho qual das duas e a padrao e conferir a camera de cima no teste com um criterio (fracao de pixels L > 0,6 no chao acima dos objetos < 2%). Rodar tambem a 64 amostras para ver se as listras diagonais dentro da cunha somem (se nao somem, sao do spread de 40 graus ou do denoise do raytracing e merecem um teste proprio).
- **O look do fundo depende fortemente do pitch da camera: na frente (8,8 graus para baixo) o topo do quadro e preto; no 3q (17,7 graus) o topo e rose claro sem nenhum preto na imagem. A curva chega ao preto a 14,6 graus de elevacao, e a coreografia ainda nao decidiu os pitches. Se um beat quiser topo preto com a camera inclinada, o unico caminho hoje e trocar 'curva' por params - a pendencia esta anotada, mas nao ha valor pronto.** → Acrescentar em PARAMS_PADRAO um segundo preset de curva ja medido (ex.: 'curva_fechada': ((0,1),(0.09,1),(0.24,0),(1,0)), preto a ~9 graus) e renderizar o 3q com ele no teste, para a coreografia ter dois fundos provados em vez de um numero solto. Alternativa mais estrutural, se o Adriano quiser fundo igual em todo plano: como o world ja separa camera de probe pelo Is Camera Ray, o ramo da CAMERA pode usar a coordenada Window (gradiente em espaco de tela, sempre preto em cima e rose embaixo) e o ramo do probe seguir por elevacao; o custo e o chao infinito nao fundir mais no ceu - so vale se a fusao for testada de novo.

### cabo

- **posicionar_repouso NAO funciona depois de animar_conexao: ela so escreve location/rotation e os pontos da curva sem chave, e as fcurves (extrapolacao constante) sobrescrevem tudo na proxima avaliacao de quadro. Medi: chamei posicionar_repouso(objs, (0.5,0.9,0.4), d) apos a animacao, location ficou (0.5,0.9,0.4) e apos frame_set(120) voltou para (0.245,0.25,0.125). A propria pendencia do construtor manda a coreografia usar essa funcao no beat 6 - ela seria usada e falharia em silencio.** → Dar a posicionar_repouso um parametro quadro=None: com quadro, chamar _aplicar_pose(..., quadro=quadro) (grava chave, como animar_conexao faz); sem quadro e havendo animation_data, limpar as fcurves (plugue.animation_data_clear() e curva.data.animation_data_clear()) antes de posar, e documentar isso na docstring. Para o beat 6 (U1 descendo com o cabo ligado) o certo e uma funcao que grave uma chave por quadro seguindo o ponto da tomada movel - por exemplo animar_seguir(objs, q_ini, q_fim, funcao_ponto_direcao) que chama _aplicar_pose com quadro=f.
- **Assinatura fora do padrao da especificacao: a API pede animar_<acao>(objs, quadro_ini, quadro_fim, **kw); aqui e animar_conexao(objs, ponto_tomada, direcao_entrada, q_ini, q_fim, ...). A coreografia que seguir a espec (animar_conexao(objs, 165, 220, ponto_tomada=..., ...)) recebe TypeError ou, pior, passa quadros na posicao de ponto.** → Guardar ponto_tomada e direcao_entrada no dict devolvido por construir_cabo (objs['ponto_tomada'], objs['direcao_entrada']) e mudar para animar_conexao(objs, q_ini, q_fim, easing='EASE_IN_OUT', ponto_tomada=None, direcao_entrada=None, ...), usando os do dict quando None. Ajustar a chamada em teste_cabo.py.
- **O 'clique' le como rebote para FORA, nao como encaixe: o smoothstep zera a velocidade no fim (medi 1,32 / 0,80 / 0,27 mm por quadro nos ultimos 3 quadros antes do toque - o plugue rasteja e para), e so DEPOIS de assentado ele sai 3 mm (q96) e volta (q100). Um plugue que ja encaixou nao salta para fora; o pedido era desacelerar, parar a 3 mm e dar o empurrao final. O comentario em _ease tambem esta errado: o smoothstep cubico tem derivada zero em u=1, para tao completamente quanto o quintico.** → Na fase de voo, mirar em assento + normal*recuo (para a 3 mm) mantendo velocidade util no ultimo quadro (por exemplo terminar a parametrizacao em s=_ease(u)*0.97 ou usar EASE_OUT quadratico, que chega com ~2 mm/quadro); na fase do clique, ir de +recuo ate assento com ease-in curto (2-3 quadros) e ficar parado nos restantes, em vez do meio-seno que sai e volta. Corrigir o comentario de _ease. Confirmar rendendo 12 quadros do final em sequencia (q_fim-10..q_fim) e olhando.
- **Vazamento de Actions na idempotencia: limpar_colecao remove objetos, malhas e curva, mas nao as Actions; apos 3 construcoes+animacoes ha 6 actions (cabo.plugueAction, .001, .002, cabo.curvaAction, .001, .002). Nao duplica objeto, mas acumula dado orfao a cada rodada na cena do cliente.** → Em limpar_colecao, antes de remover cada objeto, guardar obj.animation_data.action e obj.data.animation_data.action (se existirem) e, depois de remover objeto e dado, chamar bpy.data.actions.remove(acao) quando acao.users == 0.
- **Antes de q_ini o plugue fica DEITADO NO CHAO a 0,9 m atras do U1 por extrapolacao constante das fcurves (medi q-50 = (0.545, 1.10, 0.02)), e o cabo cruza o chao dali para tras. Nos beats 1-2 do anuncio (quadros 1-165) isso aparece em qualquer camera que veja o chao atras da caixa. O modulo nao oferece nada para esconder o cabo ate a hora do voo.** → Em animar_conexao, gravar hide_render/hide_viewport = True em q_ini-1 e False em q_ini nos objetos da colecao (parametro esconder_antes=True), ou expor animar_visibilidade(objs, quadro, visivel). Anotar na docstring que antes de q_ini o plugue esta em 'origem'.
- **Com o U1 substituto real, penetracao=0 deixa o bico (7,5 mm) inteiro FORA do aro do C14: a cara do bico so encosta no plano do aro (face_coluna+0,0035) e o bolso do mod_u1 (16,5 mm de fundo, 24x18 mm) fica vazio. Num C13 encaixado o corpo assenta contra o aro e o bico some dentro. O construtor sugere 0,006, mas o valor natural e o comprimento do bico.** → Devolver objs['comprimento_bico'] = -BICO[4] (0,0075) e documentar: com o mod_u1 usar penetracao=objs['comprimento_bico'] para o corpo assentar no aro. Provar com um render de close no teste usando o mod_u1 (o teste ja pode importar mod_u1 e construir so a traseira, ou aceitar QUAIS=u1).

### cartela

- **O texto 'branco' renderiza cinza claro: medido 0,82 sRGB no maximo (~#D1D1D1) no quadro assentado. Emission com forca 1,0 sob AgX 'Medium High Contrast' nao chega ao branco; num anuncio Apple o branco tem de ser branco, e o relato 'nao estoura no AgX' virou 'nem chega'.** → Subir 'forca_texto' de 1,0 para ~1,8-2,0 (e 'forca_destaque' proporcionalmente, ~2,2) e MEDIR o pixel do render ate o branco assentado ficar >= 0,95 sRGB. Fica abaixo do limiar_bloom 2,5 do mod_ambiente, entao nao floresce. Deixar o teste imprimir o maximo do canal na faixa da linha 1, para o numero nao voltar a ser chute.
- **A linha 4 (a chamada com o site - a linha que vende) assenta em ~76% da altura do quadro (y ~730 de 960), dentro da zona que o Instagram Reels e o TikTok cobrem com legenda, nome do perfil e icones (os ~30-35% de baixo e a coluna da direita). O bloco esta centrado geometricamente, mas o centro util de um Reel fica acima do centro do quadro.** → Adicionar um param 'centro_vertical' (fracao do quadro, padrao ~0,44) ou um deslocamento em Y local em posicionar_cartela (a 2 m, 0,10 do quadro = 0,21 m), para o bloco subir ate a linha 4 ficar acima de ~65% da altura; como a logo ja esta em 22% do topo, compensar reduzindo 'largura_logo' (0,30 -> 0,24) e/ou a entrelinha 3 (1,80 -> 1,55). Renderizar e conferir que a logo nao entra nos 14% de cima.
- **'EnginePrint' em FreeSans Bold sai pesado e 'anos 90'; a hierarquia Apple vem do tamanho e de um semibold no maximo, nunca de um bold cheio. No Linux a lista cai direto em FreeSansBold; a Segoe UI Semibold do Windows nao foi vista.** → Em FONTES_FORTES, depois das Segoe (seguisb.ttf), listar FreeSans.ttf / LiberationSans-Regular.ttf (Regular, nao Bold) como fallback: o tamanho 1,0 x 0,62 ja carrega a hierarquia. Se quiser diferenca de peso sem fonte, usar 'extrusao' 0 e 'offset' do TextCurve (curva.offset ~ +0,004 m) na linha 1 - engorda um fio, nao um bold. Renderizar e olhar.
- **A camera do mod_ambiente tem DoF ligado (f/2.8) com foco no 'camera.alvo' na caixa. Ao posicionar a cartela a 2 m na frente da camera no beat 7, ela sai fora do plano de foco e vira um borrao - nada no modulo nem no teste (que nao usa DoF) prova a cartela nitida com a camera de verdade.** → O modulo nao pode tocar a camera, entao: documentar na docstring de posicionar_cartela que a coreografia precisa mover o camera.alvo para a raiz (ou keyar dof.focus_distance = distancia / desligar dof) no beat 7; e no teste_cartela ligar dof com focus_object = raiz para provar que fica nitido.
- **posicionar_cartela fixa a raiz no mundo no instante da chamada; a raiz nao e keyframed nem parenteada. No beat 7 a camera continua aproximando e atravessando a logo, entao a cartela fixa vai crescer no quadro e ser atravessada tambem.** → Acrescentar param 'parentear=True' em posicionar_cartela: raiz.parent = camera; raiz.matrix_parent_inverse = Matrix.Identity(4); raiz.matrix_basis = Matrix.Translation((0, 0, -distancia)). So a raiz e alterada (a camera continua apenas lida), e a cartela acompanha qualquer movimento da camera sem chave nenhuma.

### tela_ui

- **Tom dos cartoes lido a olho ficou errado: a captura oficial (首页_勾选设置_英文.png, baixada e amostrada em varios pontos) tem cartoes em #2B2E32 (43,46,50), levemente azulados; o HTML usa #1C1C1E (28,28,30), cerca de 40% mais escuro. Quem conhece a maquina percebe a UI mais 'chapada' que a real, e sob AgX o contraste cartao/fundo some ainda mais.** → Trocar background dos .cartao de #1c1c1e para #2b2e32 (e o fill dos circulos do icone de reguas, que copia a cor do cartao); no cabecalho, mover 'tons de cinza' de APROXIMADO para CONFIRMADO com o valor medido. Regenerar os dois PNGs com o comando do cabecalho.
- **No render do modulo u1 (saida/previa_u1_tela.png) a UI sai lavada pelo AgX: cartoes cinza medio, vermelho salmao, azul do Start lavanda. O PNG esta certo; a emissao do material u1.tela esta forte demais e o AgX dessatura o que passa de 1.0.** → E do modulo u1, nao deste: baixar o Emission Strength da tela ligada para perto de 1.0 (nao clarear o PNG) e, se ainda faltar preto, usar look 'AgX - Medium High Contrast' na cena. Registrar como pendencia do u1.
