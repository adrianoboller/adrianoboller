# Trocar para buffer plano com permutacao
# 29/08 03:25

import io,re
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()

ini = s.index("    pub fn construir_em_lote(&mut self, idx: usize, chaves: Vec<Vec<u8>>) -> Result<()> {")
fim = s.index("    // -------------------------------------------------------------- busca\n", ini)
novo = '''    pub fn construir_em_lote(&mut self, idx: usize, chaves: Vec<u8>) -> Result<()> {
        self.construir_em_lote_com(idx, chaves, ENCHIMENTO_PADRAO)
    }

    /// O mesmo, escolhendo quanto de cada folha encher, em porcento.
    ///
    /// Existe separado porque o numero e uma troca medivel, e nao uma verdade:
    /// veja `ENCHIMENTO_PADRAO`. O medidor e `--example indice-em-lote`.
    pub fn construir_em_lote_com(
        &mut self,
        idx: usize,
        chaves: Vec<u8>,
        enchimento: usize,
    ) -> Result<()> {
        let d = self.descritor(idx)?.clone();
        let ck_len = d.ck_len();
        if !(1..=100).contains(&enchimento) {
            return Err(PhxError::Esquema(format!(
                "enchimento {enchimento} invalido: use de 1 a 100 por cento"
            )));
        }
        if chaves.len() % ck_len != 0 {
            return Err(PhxError::Corrompido(format!(
                "indice {}: lote de {} bytes nao e multiplo da chave de {ck_len}",
                d.nome,
                chaves.len()
            )));
        }
        let total = chaves.len() / ck_len;
        if total > u32::MAX as usize {
            return Err(PhxError::Esquema(format!(
                "indice {}: {total} chaves passam do teto de {} por lote",
                d.nome,
                u32::MAX
            )));
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
        if total == 0 {
            return Ok(()); // a folha vazia que ja esta la e a arvore certa
        }

        let em = |i: u32| {
            let a = i as usize * ck_len;
            &chaves[a..a + ck_len]
        };

        // Ordena uma PERMUTACAO, e nao as chaves: mover 4 bytes por troca em vez
        // da chave inteira, e sem uma alocacao por chave. Num indice de dez
        // milhoes isso e a diferenca entre ~200 MiB e mais de meio giga.
        //
        // A codificacao preserva ordem, entao ordenar os bytes e ordenar os
        // valores -- e o rowid no fim desempata chave repetida de indice nao
        // unico, o que torna a ordem total.
        let mut ordem: Vec<u32> = (0..total as u32).collect();
        ordem.sort_unstable_by(|a, b| em(*a).cmp(em(*b)));

        for par in ordem.windows(2) {
            let (x, y) = (em(par[0]), em(par[1]));
            if x == y {
                return Err(PhxError::Corrompido(format!(
                    "indice {}: mesma chave completa duas vezes no lote",
                    d.nome
                )));
            }
            if d.unico && x[..d.key_len] == y[..d.key_len] {
                return Err(PhxError::Duplicado(format!(
                    "indice unico {}: chave repetida no lote",
                    d.nome
                )));
            }
        }

        // ------------------------------------------------------------ folhas
        let cap_folha = (self.page_size - PAG_CAB) / ck_len;
        let por_folha = (cap_folha * enchimento / 100).max(1);
        // Reparte em partes IGUAIS em vez de encher ate o teto e deixar o resto
        // na ultima: com 101 chaves e teto 100 sairiam 100 e 1, e a folha de uma
        // chave so divide na primeira insercao seguinte.
        let qtd_folhas = total.div_ceil(por_folha);

        // As paginas das folhas sao reservadas ANTES de escrever qualquer uma:
        // assim cada folha ja nasce sabendo o numero da seguinte, e nenhuma
        // precisa ser relida para ganhar o `prox`.
        let mut paginas = Vec::with_capacity(qtd_folhas);
        paginas.push(d.raiz);
        for _ in 1..qtd_folhas {
            paginas.push(self.alocar_pagina()?);
        }

        let mut filhos: Vec<(u64, Vec<u8>)> = Vec::with_capacity(qtd_folhas);
        let mut lida = 0usize;
        for f in 0..qtd_folhas {
            let quantas = fatia(total, qtd_folhas, f);
            let mut p = nova_pagina(self.page_size, TIPO_FOLHA);
            pag_set_qtd(&mut p, quantas);
            pag_set_ant(&mut p, if f == 0 { 0 } else { paginas[f - 1] });
            pag_set_prox(&mut p, if f + 1 < qtd_folhas { paginas[f + 1] } else { 0 });
            for j in 0..quantas {
                let a = PAG_CAB + j * ck_len;
                p[a..a + ck_len].copy_from_slice(em(ordem[lida + j]));
            }
            filhos.push((paginas[f], em(ordem[lida]).to_vec()));
            self.gravar_pagina(paginas[f], &mut p)?;
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

'''
s = s[:ini] + novo + s[fim:]
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
