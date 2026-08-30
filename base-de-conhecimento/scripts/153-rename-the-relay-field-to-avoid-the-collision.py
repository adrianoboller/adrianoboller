# Rename the relay field to avoid the collision
# 27/08 20:50

import re
# config: destinos -> servidores
p='crates/phxsql-server/src/config.rs'
s=open(p).read()
s=s.replace('''    /// Servidores PhxSql que esta interface pode alcancar, como "host:porta".
    ///
    /// VAZIO = so este servidor. E o padrao, e e o padrao certo: uma interface
    /// que fala com qualquer endereco e um proxy aberto de saida, e quem
    /// invadir a porta da web ganha a rede inteira junto.
    pub destinos: Vec<String>,''','''    /// Servidores PhxSql que esta interface pode alcancar, como "host:porta".
    ///
    /// VAZIO = so este servidor. E o padrao, e e o padrao certo: uma interface
    /// que fala com qualquer endereco e um proxy aberto de saida, e quem
    /// invadir a porta da web ganha a rede inteira junto.
    pub servidores: Vec<String>,''')
s=s.replace('            destinos: Vec::new(),','            servidores: Vec::new(),')
s=s.replace('                destinos: w.textos("destinos"),','                servidores: w.textos("servidores"),')
s=s.replace('''    /// Ha algum destino configurado? Sem isso a interface so fala consigo.
    pub fn destinos_permitidos_algum(&self) -> bool {
        !self.destinos.is_empty()
    }

    pub fn destino_permitido(&self, destino: &str) -> bool {
        let d = destino.trim();
        !d.is_empty() && self.destinos.iter().any(|p| p.trim() == d)
    }''','''    /// Ha algum servidor configurado? Sem isso a interface so fala consigo.
    pub fn alcanca_outro_servidor(&self) -> bool {
        !self.servidores.is_empty()
    }

    pub fn servidor_permitido(&self, alvo: &str) -> bool {
        let d = alvo.trim();
        !d.is_empty() && self.servidores.iter().any(|p| p.trim() == d)
    }''')
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('''        // Abrir conexao para outro PhxSql, se o login pediu um destino.
        let destino = Json::analisar(&pedido.corpo)
            .ok()
            .map(|j| j.texto_ou("destino", "").trim().to_string())
            .unwrap_or_default();''','''        // Abrir conexao para outro PhxSql, se o login pediu um servidor.
        //
        // O campo se chama "servidor" e nao "destino" porque "destino" ja e o
        // diretorio do backup -- e a colisao de nome mandava todo pedido de
        // backup para o relay. Achado ligando a peca, nao lendo o codigo.
        let servidor_remoto = Json::analisar(&pedido.corpo)
            .ok()
            .map(|j| j.texto_ou("servidor", "").trim().to_string())
            .unwrap_or_default();''')
s=s.replace('let (op, autenticado, resultado) = match (&ja_remota, destino.is_empty()) {',
            'let (op, autenticado, resultado) = match (&ja_remota, servidor_remoto.is_empty()) {')
s=s.replace('''            // Login novo pedindo destino: abre, encaminha, e guarda se entrou.
            (None, false) => {
                let r = self.abrir_remoto(&destino, &pedido.corpo, ip);''','''            // Login novo pedindo servidor: abre, encaminha, e guarda se entrou.
            (None, false) => {
                let r = self.abrir_remoto(&servidor_remoto, &pedido.corpo, ip);''')
s=s.replace('let remota = ja_remota.is_some() || !destino.is_empty();',
            'let remota = ja_remota.is_some() || !servidor_remoto.is_empty();')
s=s.replace('''        if !self.config.web.destinos_permitidos_algum() {
            return Err((
                op,
                PhxError::Autorizacao(
                    "esta interface nao fala com outro servidor: preencha web.destinos no config.json".into(),
                ),
            ));
        }
        if !self.config.web.destino_permitido(destino) {
            // Endereco fora da lista e sondagem de rede, nao engano: alguem
            // esta procurando o que mais existe do outro lado.
            self.violacao_grave(ip, &op, "destino fora de web.destinos");
            return Err((
                op,
                PhxError::Autorizacao(format!(
                    "{destino} nao esta em web.destinos; o IP foi bloqueado"
                )),
            ));
        }''','''        if !self.config.web.alcanca_outro_servidor() {
            return Err((
                op,
                PhxError::Autorizacao(
                    "esta interface nao fala com outro servidor: preencha web.servidores no config.json".into(),
                ),
            ));
        }
        if !self.config.web.servidor_permitido(destino) {
            // Endereco fora da lista e sondagem de rede, nao engano: alguem
            // esta procurando o que mais existe do outro lado.
            self.violacao_grave(ip, &op, "servidor fora de web.servidores");
            return Err((
                op,
                PhxError::Autorizacao(format!(
                    "{destino} nao esta em web.servidores; o IP foi bloqueado"
                )),
            ));
        }''')
s=s.replace('''                        (
                            "destinos",
                            Json::Lista(
                                self.config.web.destinos.iter().map(Json::texto_de).collect(),
                            ),
                        ),''','''                        (
                            "servidores",
                            Json::Lista(
                                self.config.web.servidores.iter().map(Json::texto_de).collect(),
                            ),
                        ),''')
open(p,'w').write(s)
print('renomeado')
