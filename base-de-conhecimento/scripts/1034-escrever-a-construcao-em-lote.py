# Escrever a construcao em lote
# 29/08 03:24

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()

anc = "    // -------------------------------------------------------------- busca\n"
assert s.count(anc)==1

novo = '''    // ---------------------------------------------------- construcao em lote

    /// Monta a arvore inteira de um indice a partir das chaves, de uma vez.
    ///
    /// # Por que existe
    ///
    /// `inserir` desce a arvore uma vez por chave. Reconstruir um indice de um
    /// milhao de linhas assim custa um milhao de descidas -- exatamente o
    /// trabalho do caminho de dentro, so que de novo. E por isso que adiar o
    /// indice numa carga e reconstruir no fim comprava 1,02x: o `reindexar`
    /// pagava o mesmo preco em outro lugar.
    ///
    /// Aqui nao ha descida nenhuma. As chaves sao ordenadas, as folhas sao
    /// enchidas em SEQUENCIA, e os niveis de cima sao montados por cima dos de
    /// baixo. Cada pagina e escrita uma vez, na ordem do arquivo.
    ///
    /// # O que ele exige
    ///
    /// **O indice tem de estar vazio.** Isto e uma construcao, e nao um remendo
    /// numa arvore existente: aproveitar as paginas de uma arvore antiga pediria
    /// devolve-las a lista de livres uma a uma, e quem chama aqui (`reindexar`)
    /// acabou de truncar o arquivo. Recusar e melhor que vazar paginas em
    /// silencio.
    pub fn construir_em_lote(&mut self, idx: usize, chaves: Vec<Vec<u8>>) -> Result<()> {
        self.construir_em_lote_com(idx, chaves, ENCHIMENTO_PADRAO)
    }

    /// O mesmo, escolhendo quanto de cada folha encher, em porcento.
    ///
    /// Existe separado porque o numero e uma troca medivel, e nao uma verdade:
    /// veja `ENCHIMENTO_PADRAO`. O medidor e `--example indice-em-lote`.
    pub fn construir_em_lote_com(
        &mut self,
        idx: usize,
        mut chaves: Vec<Vec<u8>>,
        enchimento: usize,
    ) -> Result<()> {
        let d = self.descritor(idx)?.clone();
        let ck_len = d.ck_len();
        if !(1..=100).contains(&enchimento) {
            return Err(PhxError::Esquema(format!(
                "enchimento {enchimento} invalido: use de 1 a 100 por cento"
            )));
        }
        for c in &chaves {
            if c.len() != ck_len {
                return Err(PhxError::Corrompido(format!(
                    "indice {}: chave de {} bytes, esperado {ck_len}",
                    d.nome,
                    c.len()
                )));
            }
        }

        // A arvore precisa estar vazia, e a folha que ela ja tem vira a
        // primeira do lote -- senao ela vazaria, sem entrar na lista de livres.
        let raiz_atual = self.ler_pagina(d.raiz)?;
        if pag_tipo(&raiz_atual) != TIPO_FOLHA || pag_qtd(&raiz_atual) != 0 {
            return Err(PhxError::Esquema(format!(
                "indice {}: construir em lote exige indice vazio",
                d.nome
            )));
        }

        // A codificacao preserva ordem, entao ordenar os bytes e ordenar os
        // valores -- e o rowid no fim desempata chave repetida de indice nao
        // unico, o que torna a ordem total.
        chaves.sort_unstable();
        for par in chaves.windows(2) {
            if par[0] == par[1] {
                return Err(PhxError::Corrompido(format!(
                    "indice {}: mesma chave completa duas vezes no lote",
                    d.nome
                )));
            }
            if d.unico && par[0][..d.key_len] == par[1][..d.key_len] {
                return Err(PhxError::Duplicado(format!(
                    "indice unico {}: chave repetida no lote",
                    d.nome
                )));
            }
        }

        let total = chaves.len();
        if total == 0 {
            return Ok(()); // a folha vazia que ja esta la e a arvore certa
        }

        // ------------------------------------------------------------ folhas
        let cap_folha = (self.page_size - PAG_CAB) / ck_len;
        let por_folha = (cap_folha * enchimento / 100).max(1);
        // Reparte em partes IGUAIS em vez de encher ate o teto e deixar o resto
        // na ultima: com 101 chaves e teto 100 sairiam 100 e 1, e a folha de uma
        // chave so divide na primeira insercao seguinte.
        let qtd_folhas = total.div_ceil(por_folha);
        let mut filhos: Vec<(u64, Vec<u8>)> = Vec::with_capacity(qtd_folhas);

        let mut lida = 0usize;
        let mut anterior: u64 = 0;
        for f in 0..qtd_folhas {
            let quantas = fatia(total, qtd_folhas, f);
            let pagina = if f == 0 { d.raiz } else { self.alocar_pagina()? };
            let mut p = nova_pagina(self.page_size, TIPO_FOLHA);
            pag_set_qtd(&mut p, quantas);
            pag_set_ant(&mut p, anterior);
            for j in 0..quantas {
                let a = PAG_CAB + j * ck_len;
                p[a..a + ck_len].copy_from_slice(&chaves[lida + j]);
            }
            filhos.push((pagina, chaves[lida].clone()));
            // O `prox` da anterior so se sabe agora, e por isso ela e gravada
            // com um passo de atraso -- nao ha releitura para remendar.
            if f > 0 {
                let (ant_pag, _) = filhos[f - 1];
                let mut ant = self.ler_pagina(ant_pag)?;
                pag_set_prox(&mut ant, pagina);
                self.gravar_pagina(ant_pag, &mut ant)?;
            }
            self.gravar_pagina(pagina, &mut p)?;
            anterior = pagina;
            lida += quantas;
        }

        // ---------------------------------------------------- niveis de cima
        let ent = ck_len + 8;
        let cap_interno = (self.page_size - PAG_CAB) / ent;
        let max_filhos = cap_interno + 1;

        while filhos.len() > 1 {
            let qtd_nos = filhos.len().div_ceil(max_filhos);
            let mut acima: Vec<(u64, Vec<u8>)> = Vec::with_capacity(qtd_nos);
            let mut lido = 0usize;
            for n in 0..qtd_nos {
                let quantos = fatia(filhos.len(), qtd_nos, n);
                let grupo = &filhos[lido..lido + quantos];
                let pagina = self.alocar_pagina()?;
                let mut p = nova_pagina(self.page_size, TIPO_INTERNO);
                // `escolher_filho` manda para `filho[i]` quem for MENOR que
                // `chave[i]`; entao a chave separadora e a primeira do filho
                // seguinte, que e o mesmo que a divisao promove.
                pag_set_qtd(&mut p, quantos - 1);
                for (i, (f, _)) in grupo[..quantos - 1].iter().enumerate() {
                    let a = PAG_CAB + i * ent;
                    p[a..a + ck_len].copy_from_slice(&grupo[i + 1].1);
                    interno_set_filho(&mut p, i, ck_len, *f);
                }
                pag_set_dir(&mut p, grupo[quantos - 1].0);
                self.gravar_pagina(pagina, &mut p)?;
                acima.push((pagina, grupo[0].1.clone()));
                lido += quantos;
            }
            filhos = acima;
        }

        self.indices[idx].raiz = filhos[0].0;
        self.indices[idx].qtd_chaves = total as u64;
        self.gravar_cabecalho()?;
        Ok(())
    }

    // -------------------------------------------------------------- busca
'''
s=s.replace(anc, novo)

# a funcao auxiliar `fatia` e a constante, junto das outras livres
anc2 = "fn nova_pagina(page_size: usize, tipo: u8) -> Vec<u8> {"
assert s.count(anc2)==1
aux = '''/// Quanto de cada folha a construcao em lote enche, em porcento.
///
/// 100 daria a arvore mais compacta e a varredura mais rapida -- e faria a
/// PRIMEIRA insercao em cada folha dividir, porque a tabela recem-carregada
/// continua crescendo. 70 e a folga classica; o numero esta medido em
/// `--example indice-em-lote`, e nao chutado.
const ENCHIMENTO_PADRAO: usize = 70;

/// Tamanho da fatia `i` ao repartir `total` em `partes` o mais iguais possivel.
///
/// As primeiras `total % partes` fatias levam uma a mais. Existe para nao
/// terminar com uma folha de uma chave so depois de encher todas as outras.
fn fatia(total: usize, partes: usize, i: usize) -> usize {
    total / partes + usize::from(i < total % partes)
}

fn nova_pagina(page_size: usize, tipo: u8) -> Vec<u8> {'''
s=s.replace(anc2, aux)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
