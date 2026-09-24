// Matriz (netns A): cria a rede, convida, liga, conversa, compartilha o USB.
import { abrir, espera, marca, pausa, clicar, digitar, legenda, D } from './comum.mjs';
import fs from 'fs';
const j = await abrir('a'); const p = j.p;
await p.waitForSelector('.vazio');
await j.cena('a1', 'Matriz: a janela começa vazia. Um clique cria a rede.');
await pausa(2200);
await clicar(p, '#b-criar'); await digitar(p, '#f-criar [name=rede]', 'Matriz');
await pausa(600); await clicar(p, '#f-criar button.inclui');
await p.waitForSelector('.rede .nome'); await pausa(1500);
await legenda(p, 'Convidar: o código sai cifrado com a senha da rede e vale uma única entrada.');
await clicar(p, '.rede .acoes button:has-text("Convidar")');
await digitar(p, '#f-convidar [name=senha]', 'senha-da-matriz');
await digitar(p, '#f-convidar [name=endereco]', '192.168.77.1:51820', 25);
await clicar(p, '#b-gerar'); await p.waitForSelector('#b-copiar:not([hidden])');
marca('codigo-b', await p.inputValue('#f-convidar [name=codigo]'));
await pausa(3000); await clicar(p, '#d-convidar [data-fechar]');
await legenda(p, 'Ligar: a senha vira a chave do túnel. Sem servidor no meio.');
await clicar(p, '.rede .acoes button:has-text("Ligar")');
await digitar(p, '#f-ligar [name=senha]', 'senha-da-matriz');
await clicar(p, '#f-ligar button.inclui');
await p.waitForSelector('.lampada.on', { timeout: 30000 }); await pausa(2000);
j.fim(); marca('a-ligada');

await espera('b-ligada');
await p.waitForSelector('.membro .ponto.on', { timeout: 60000 });
await j.cena('a2', 'A Filial entrou: aparece conectada, pelo caminho direto e cifrado.');
await pausa(2800);
await legenda(p, 'Ping pelo túnel…');
await clicar(p, '.membro button:has-text("Ping")');
await p.waitForFunction(() => /ms/.test(document.querySelector('#rodape').textContent), null, { timeout: 15000 });
await pausa(2500);
await legenda(p, '…e chat direto entre os computadores.');
await clicar(p, '.membro button:has-text("Chat")');
await digitar(p, '#f-chat [name=texto]', 'Bom dia, Filial! Tudo certo por aí?', 40);
await clicar(p, '#f-chat button.inclui'); await pausa(1800);
j.fim(); marca('a-falou');

await espera('b-respondeu');
await j.cena('a3', 'A resposta chega na hora.');
await p.waitForSelector('.bolha.dele', { timeout: 30000 }); await pausa(2800);
await clicar(p, '#d-chat [data-fechar]');
await legenda(p, 'USB pela rede: o pendrive da Matriz fica disponível só para os membros.');
await clicar(p, '.rede .acoes button:has-text("USB")');
await p.waitForSelector('#usb-locais .usb-linha'); await pausa(2200);
await clicar(p, '#usb-locais button:has-text("Compartilhar")');
await p.waitForSelector('#usb-locais button:has-text("Parar")'); await pausa(2500);
j.fim(); marca('a-compartilhou');

await espera('b-usou');
await clicar(p, '#d-usb [data-fechar]');
await j.cena('a4', 'Mais um membro: um novo convite para a Casa do diretor.');
await clicar(p, '.rede .acoes button:has-text("Convidar")');
await digitar(p, '#f-convidar [name=senha]', 'senha-da-matriz');
await digitar(p, '#f-convidar [name=endereco]', '192.168.77.1:51820', 25);
await clicar(p, '#b-gerar'); await p.waitForSelector('#b-copiar:not([hidden])');
marca('codigo-c', await p.inputValue('#f-convidar [name=codigo]'));
await pausa(2000); await clicar(p, '#d-convidar [data-fechar]');
j.fim();

await espera('c-ligada');
await p.waitForFunction(() => document.querySelectorAll('.membro .ponto.on').length >= 2, null, { timeout: 90000 });
await j.cena('a5', 'Três computadores numa rede privada. O resumo conta tudo sozinho.');
await pausa(4500);
j.fim();

// Responsividade: a MESMA janela, medida em cinco larguras (sem o cursor
// do video na foto).
await legenda(p, '');
await p.evaluate(() => document.getElementById('__cursor')?.remove());
const medidas = [];
for (const [w, h] of [[390, 844], [820, 1180], [1280, 800], [1920, 1080], [3440, 1440]]) {
  await p.setViewportSize({ width: w, height: h }); await pausa(700);
  const m = await p.evaluate(() => ({ largura: innerWidth, rolagem: document.documentElement.scrollWidth,
    colunas: getComputedStyle(document.getElementById('lista')).gridTemplateColumns.split(' ').length }));
  medidas.push(m);
  await p.screenshot({ path: `${D}/resp-${w}.png` });
}
fs.writeFileSync(`${D}/responsivo.json`, JSON.stringify(medidas));
marca('fim-a');
await j.fechar();
