# Resolve the three semantic conflicts
# 29/08 17:22

import re, pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()

# ---- 1: o laco que puxa. O corte do cluster continua primeiro (o laco do
# cluster puxa do master corrente); a condicao vira a do papel, que cobre os
# papeis novos (read replica, spare, multi) e nao so `replica`.
c1_head = """        if self.cluster.is_some() {
            // Com cluster, quem puxa e o laco do proprio cluster, do master
            // CORRENTE -- uma lista fixa de origens apontaria para o master
            // de ontem, e dois lacos aplicando na mesma tabela brigariam.
            if !self.config.replicacao.origens.is_empty() {
                eprintln!(
                    "AVISO: replicacao.origens e IGNORADA num servidor com o \\
                     bloco cluster -- a origem e o master corrente, descoberto \\
                     pelo pulso"
                );
            }
            return;
        }
        if self.config.replicacao.papel != crate::config::Papel::Replica {
"""
c1_ramo = """        let papel = self.config.replicacao.papel;
        if !papel.puxa_de_origem() {
"""
alvo1 = "<<<<<<< HEAD\n" + c1_head + "=======\n" + c1_ramo + ">>>>>>> worktree-agent-aeba5ba7fe4b19f92\n"
assert alvo1 in t, "conflito 1 nao casou"
t = t.replace(alvo1, c1_head.replace(
    "        if self.config.replicacao.papel != crate::config::Papel::Replica {\n",
    "        let papel = self.config.replicacao.papel;\n        if !papel.puxa_de_origem() {\n"), 1)

# ---- 2: os dois portoes, na ordem certa. O 2a (papel) recusa spare e read
# replica; o 2b decide a escrita.
i = t.index("<<<<<<< HEAD\n        // Portao 2b -- a escrita.")
j = t.index(">>>>>>> worktree-agent-aeba5ba7fe4b19f92\n", i) + len(">>>>>>> worktree-agent-aeba5ba7fe4b19f92\n")
novo2 = '''        // Portao 2a -- o PAPEL do servidor. Antes do somente-leitura, para a
        // recusa dizer o que importa: nao e "voce nao pode", e "este servidor
        // nao atende isso -- o primario e ali". Um portao so, aqui, e nao
        // espalhado pelas operacoes: a que alguem esquecesse viraria a porta
        // dos fundos.
        match self.papel_atual() {
            Papel::Spare if !OPS_NO_SPARE.contains(&op) => {
                return Err(PhxError::SpareEmEspera(format!(
                    "este servidor e um spare de contingencia e nao atende \\
                     cliente (nem leitura); o primario e {}. Para assumir o \\
                     trabalho: {{\\"op\\":\\"spare_promover\\"}}",
                    self.primario()
                )));
            }
            // O mesmo erro do redirecionamento de cluster, e de proposito: para
            // o cliente, "escreveu no no errado, va para aquele" e UM evento so.
            // O `REDIRECIONA host:porta` na frente e o pedaco que ele recorta.
            Papel::ReadReplica if OPS_ESCRITA.contains(&op) => {
                return Err(PhxError::Redireciona(format!(
                    "REDIRECIONA {} -- este servidor e uma replica de leitura; \\
                     escreva no primario",
                    self.primario()
                )));
            }
            _ => {}
        }

        // Portao 2b -- a escrita. Com cluster, quem decide e o papel VIVO dele:
        // a replica redireciona para o master e um master sem maioria visivel
        // recusa, para conter o split-brain. SEM cluster, vale o
        // `somente_leitura` VIVO, que a promocao de um spare abre sem
        // reiniciar. A ordem importa: um no de cluster carrega
        // `somente_leitura: true` como replica, e conferir isso ANTES do
        // cluster faria a promocao nao promover nada.
        //
        // A recusa do somente-leitura sai pela tabela de mensagens (texto que
        // gente le, entao acompanha o idioma); a do cluster ja vem pronta.
        if OPS_ESCRITA.contains(&op) {
            if let Some(estado) = &self.cluster {
                if let Some(recusa) = estado.recusa_de_escrita() {
                    return Err(recusa);
                }
            } else if self.somente_leitura_vivo.load(Ordering::Relaxed) {
                return Err(PhxError::Autorizacao(self.msg("erro.somente_leitura", &[])));
            }
'''
t = t[:i] + novo2 + t[j:]

# ---- 3: o papel que a tela ve. Com cluster, o do cluster; sem, o papel vivo
# do processo (que a promocao do spare muda).
c3 = re.search(r"<<<<<<< HEAD\n(\s+// Num cluster o papel.*?)=======\n(.*?)>>>>>>> worktree-agent-aeba5ba7fe4b19f92\n", t, re.S)
assert c3, "conflito 3 nao casou"
t = t[:c3.start()] + '''                // O papel VIVO, das duas fontes que podem muda-lo: o cluster
                // (eleicao e promocao automatica) e a promocao manual do
                // spare. Responder o papel do config.json seria mentir depois
                // de qualquer uma das duas.
                (
                    "papel",
                    Json::texto_de(match &self.cluster {
                        Some(e) => e.papel().nome(),
                        None => self.papel_atual().nome(),
                    }),
                ),
                (
                    "id_servidor",
                    Json::texto_de(&self.config.replicacao.id_servidor),
''' + t[c3.end():]
p.write_text(t)
print("marcas restantes:", t.count("<<<<<<<"), t.count(">>>>>>>"))
