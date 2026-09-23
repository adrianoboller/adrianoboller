//! O painel: o cadastro no PostgreSQL e as regras de «criar rede» e «entrar na
//! rede». Tudo o que o HTTP e a linha de comando fazem passa por aqui -- UM
//! motor so, para a regra nao divergir entre as portas.
//!
//! # Integridade
//!
//! Toda chave estrangeira do esquema e `ON DELETE RESTRICT`: nao se apaga
//! empresa com servidor, servidor com rede, rede com membro. E a regra
//! primordial da casa (nunca se mata o pai que tem filhos), imposta pelo
//! proprio PostgreSQL e nao por codigo que alguem pode esquecer de chamar.
//!
//! # O que fica no banco, e o que nao fica
//!
//! Fica: cadastro, hash das senhas (PBKDF2), certificados, e as chaves privadas
//! da AC, dos servidores e o `tls-crypt` de cada rede SELADOS pela senha mestre.
//! Nao fica: a senha mestre, e a chave privada de membro -- ela nasce no
//! «entrar», vai no perfil e some. Baixar o perfil de novo emite par novo.

use crate::cofre::{Cofre, AAD_PROVA, ITERACOES};
use crate::ovpn;
use crate::pg::{Config, Pg, Resposta, R};
use crate::pki::{self, Ac, Papel};
use phxsql_core::json::Json;
use phxsql_core::senha;
use std::fs;
use std::path::{Path, PathBuf};

const ESQUEMA: &str = "
CREATE TABLE IF NOT EXISTS phx_empresa (
    id              serial PRIMARY KEY,
    nome            text NOT NULL,
    finalidade      text NOT NULL DEFAULT '',
    responsavel     text NOT NULL,
    email           text NOT NULL,
    telefone        text NOT NULL DEFAULT '',
    certificado_pem text,
    prova_mestre    text NOT NULL,
    ac_cert_pem     text NOT NULL,
    ac_chave_selada text NOT NULL,
    criada_em       timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phx_servidor (
    id           serial PRIMARY KEY,
    empresa_id   int NOT NULL REFERENCES phx_empresa ON DELETE RESTRICT,
    nome         text NOT NULL UNIQUE,
    ip           text NOT NULL,
    dns          text NOT NULL DEFAULT '',
    cert_pem     text NOT NULL,
    chave_selada text NOT NULL,
    criado_em    timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phx_usuario (
    id         serial PRIMARY KEY,
    empresa_id int NOT NULL REFERENCES phx_empresa ON DELETE RESTRICT,
    login      text NOT NULL UNIQUE,
    email      text NOT NULL DEFAULT '',
    senha_hash text NOT NULL,
    admin      boolean NOT NULL DEFAULT false,
    ativo      boolean NOT NULL DEFAULT true,
    criado_em  timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phx_rede (
    id               serial PRIMARY KEY,
    servidor_id      int NOT NULL REFERENCES phx_servidor ON DELETE RESTRICT,
    dono_id          int NOT NULL REFERENCES phx_usuario ON DELETE RESTRICT,
    nome             text NOT NULL UNIQUE,
    finalidade       text NOT NULL DEFAULT '',
    senha_hash       text NOT NULL,
    octeto           smallint NOT NULL UNIQUE CHECK (octeto BETWEEN 1 AND 254),
    porta            int NOT NULL UNIQUE CHECK (porta BETWEEN 1 AND 65535),
    tls_crypt_selada text NOT NULL,
    criada_em        timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phx_membro (
    rede_id    int NOT NULL REFERENCES phx_rede ON DELETE RESTRICT,
    usuario_id int NOT NULL REFERENCES phx_usuario ON DELETE RESTRICT,
    host       smallint NOT NULL CHECK (host BETWEEN 2 AND 254),
    cn         text NOT NULL UNIQUE,
    cert_serie text NOT NULL,
    entrou_em  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (rede_id, usuario_id),
    UNIQUE (rede_id, host)
);
";

/// Porta da primeira rede; a rede de octeto N escuta em `PORTA_BASE + N`.
pub const PORTA_BASE: u16 = 1194;

/// Os campos da instalacao, na ordem da tela.
#[derive(Default, Clone)]
pub struct Instalacao {
    pub empresa: String,
    pub finalidade: String,
    pub responsavel: String,
    pub email: String,
    pub telefone: String,
    pub admin_usuario: String,
    pub admin_senha: String,
    pub senha_mestre: String,
    pub servidor_nome: String,
    pub servidor_ip: String,
    pub servidor_dns: String,
    /// Certificado digital da empresa (PEM), opcional. Guardado como
    /// identificacao; a VPN usa a AC propria -- ver `docs/PHXVPN.md`.
    pub certificado_pem: String,
}

#[derive(Clone, Debug)]
pub struct Usuario {
    pub id: i64,
    pub login: String,
    pub admin: bool,
}

pub struct Painel {
    pg: Pg,
    cofre: Option<Cofre>,
    dados: PathBuf,
    /// Custo do PBKDF2 (senha mestre e senhas de login). Os testes baixam.
    pub iteracoes: u32,
}

impl Painel {
    pub fn abrir(cfg: &Config, dados: &Path) -> R<Painel> {
        let mut pg = Pg::conectar(cfg)?;
        pg.lote(ESQUEMA)?;
        Ok(Painel {
            pg,
            cofre: None,
            dados: dados.to_path_buf(),
            iteracoes: ITERACOES,
        })
    }

    pub fn instalado(&mut self) -> R<bool> {
        let r = self
            .pg
            .executar("SELECT count(*) AS n FROM phx_empresa", &[])?;
        Ok(r.valor(0, "n") != Some("0"))
    }

    pub fn destrancado(&self) -> bool {
        self.cofre.is_some()
    }

    fn cofre(&self) -> R<&Cofre> {
        self.cofre
            .as_ref()
            .ok_or_else(|| "cofre trancado: o administrador precisa informar a senha mestre".into())
    }

    /// Instala: empresa, AC, primeiro servidor e o usuario administrador, numa
    /// transacao so -- instalacao pela metade deixaria o painel sem admin.
    pub fn instalar(&mut self, i: &Instalacao) -> R<()> {
        validar_instalacao(i)?;
        if self.instalado()? {
            return Err("o phxvpn ja esta instalado".into());
        }
        let cofre = Cofre::novo(&i.senha_mestre, self.iteracoes);
        let ac = pki::emitir(Papel::Ac, &i.empresa, &cn_ac(&i.empresa), 10, None)?;
        let prova = cofre.selar(b"ok", AAD_PROVA);
        let ac_selada = cofre.selar(&ac.privada, "ac");
        let ac_pem = pki::cert_pem(&ac.der);
        let certificado =
            (!i.certificado_pem.trim().is_empty()).then_some(i.certificado_pem.as_str());

        self.pg.lote("BEGIN")?;
        let resultado = (|| -> R<()> {
            let r = self.pg.executar(
                "INSERT INTO phx_empresa (nome, finalidade, responsavel, email, telefone, \
                 certificado_pem, prova_mestre, ac_cert_pem, ac_chave_selada) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id",
                &[
                    Some(&i.empresa),
                    Some(&i.finalidade),
                    Some(&i.responsavel),
                    Some(&i.email),
                    Some(&i.telefone),
                    certificado,
                    Some(&prova),
                    Some(&ac_pem),
                    Some(&ac_selada),
                ],
            )?;
            let empresa = r.valor(0, "id").ok_or("empresa sem id")?.to_string();
            let hash = senha::cifrar_com(&i.admin_senha, self.iteracoes);
            self.pg.executar(
                "INSERT INTO phx_usuario (empresa_id, login, email, senha_hash, admin) \
                 VALUES ($1::int, $2, $3, $4, true)",
                &[
                    Some(&empresa),
                    Some(&i.admin_usuario),
                    Some(&i.email),
                    Some(&hash),
                ],
            )?;
            self.cofre = Some(cofre.clone());
            self.inserir_servidor(&empresa, &i.servidor_nome, &i.servidor_ip, &i.servidor_dns)?;
            Ok(())
        })();
        match resultado {
            Ok(()) => self.pg.lote("COMMIT"),
            Err(e) => {
                self.cofre = None;
                let _ = self.pg.lote("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Prova a senha mestre contra o selo de prova e guarda a chave em memoria.
    pub fn destrancar(&mut self, senha_mestre: &str) -> R<()> {
        let r = self
            .pg
            .executar("SELECT prova_mestre FROM phx_empresa LIMIT 1", &[])?;
        let prova = r
            .valor(0, "prova_mestre")
            .ok_or("o phxvpn ainda nao foi instalado")?;
        self.cofre = Some(Cofre::destrancar(senha_mestre, prova)?);
        Ok(())
    }

    pub fn login(&mut self, login: &str, senha_clara: &str) -> R<Usuario> {
        let r = self.pg.executar(
            "SELECT id, login, senha_hash, admin FROM phx_usuario WHERE login = $1 AND ativo",
            &[Some(login)],
        )?;
        // Mesma resposta para usuario inexistente e senha errada: nao se
        // confirma a um estranho quais logins existem.
        let negado = || "usuario ou senha nao conferem".to_string();
        let hash = r.valor(0, "senha_hash").ok_or_else(negado)?;
        if !senha::conferir(senha_clara, hash) {
            return Err(negado());
        }
        usuario_da_linha(&r, 0)
    }

    pub fn criar_usuario(
        &mut self,
        login: &str,
        senha_clara: &str,
        email: &str,
        admin: bool,
    ) -> R<()> {
        validar_nome("login", login)?;
        validar_senha("senha", senha_clara, 8)?;
        let hash = senha::cifrar_com(senha_clara, self.iteracoes);
        self.pg
            .executar(
                "INSERT INTO phx_usuario (empresa_id, login, email, senha_hash, admin) \
                 SELECT id, $1, $2, $3, $4::boolean FROM phx_empresa LIMIT 1",
                &[
                    Some(login),
                    Some(email),
                    Some(&hash),
                    Some(if admin { "true" } else { "false" }),
                ],
            )
            .map_err(|e| traduzir_unico(e, "ja existe usuario com esse login"))?;
        Ok(())
    }

    pub fn usuarios(&mut self) -> R<Json> {
        let r = self.pg.executar(
            "SELECT id, login, email, admin, ativo FROM phx_usuario ORDER BY login",
            &[],
        )?;
        Ok(para_json(&r, &["admin", "ativo"], &["id"]))
    }

    pub fn servidores(&mut self) -> R<Json> {
        let r = self.pg.executar(
            "SELECT s.id, s.nome, s.ip, s.dns, (SELECT count(*) FROM phx_rede r WHERE r.servidor_id = s.id) AS redes \
             FROM phx_servidor s ORDER BY s.id",
            &[],
        )?;
        Ok(para_json(&r, &[], &["id", "redes"]))
    }

    pub fn criar_servidor(&mut self, nome: &str, ip: &str, dns: &str) -> R<()> {
        let r = self
            .pg
            .executar("SELECT id FROM phx_empresa LIMIT 1", &[])?;
        let empresa = r
            .valor(0, "id")
            .ok_or("o phxvpn ainda nao foi instalado")?
            .to_string();
        self.inserir_servidor(&empresa, nome, ip, dns)
    }

    fn inserir_servidor(&mut self, empresa: &str, nome: &str, ip: &str, dns: &str) -> R<()> {
        validar_nome("nome do servidor", nome)?;
        validar_ip(ip)?;
        validar_dns(dns)?;
        let (org, ac_privada) = self.ac()?;
        let emissora = Ac {
            organizacao: &org,
            cn: &cn_ac(&org),
            privada: &ac_privada,
        };
        let e = pki::emitir(Papel::Servidor, &org, nome, 5, Some(&emissora))?;
        let selada = self.cofre()?.selar(&e.privada, &format!("servidor:{nome}"));
        self.pg
            .executar(
                "INSERT INTO phx_servidor (empresa_id, nome, ip, dns, cert_pem, chave_selada) \
                 VALUES ($1::int, $2, $3, $4, $5, $6)",
                &[
                    Some(empresa),
                    Some(nome),
                    Some(ip),
                    Some(dns),
                    Some(&pki::cert_pem(&e.der)),
                    Some(&selada),
                ],
            )
            .map_err(|e| traduzir_unico(e, "ja existe servidor com esse nome"))?;
        Ok(())
    }

    /// Nome da organizacao e a chave privada da AC, aberta do cofre.
    fn ac(&mut self) -> R<(String, [u8; 32])> {
        let r = self
            .pg
            .executar("SELECT nome, ac_chave_selada FROM phx_empresa LIMIT 1", &[])?;
        let org = r
            .valor(0, "nome")
            .ok_or("o phxvpn ainda nao foi instalado")?
            .to_string();
        let selada = r.valor(0, "ac_chave_selada").ok_or("AC ausente")?;
        let privada = self.cofre()?.abrir_com(selada, "ac")?;
        Ok((org, privada.try_into().map_err(|_| "chave da AC torta")?))
    }

    /// Cria a rede no servidor pedido (ou no primeiro). O criador entra nela
    /// junto, como no Radmin: quem cria ja esta dentro.
    pub fn criar_rede(
        &mut self,
        dono: &Usuario,
        nome: &str,
        senha_rede: &str,
        finalidade: &str,
        servidor_id: Option<i64>,
    ) -> R<String> {
        validar_nome("nome da rede", nome)?;
        validar_senha("senha da rede", senha_rede, 6)?;
        let cofre = self.cofre()?.clone();
        let srv = match servidor_id {
            Some(id) => id.to_string(),
            None => {
                let r = self
                    .pg
                    .executar("SELECT min(id) AS id FROM phx_servidor", &[])?;
                r.valor(0, "id")
                    .ok_or("nenhum servidor cadastrado")?
                    .to_string()
            }
        };
        let livre = self.pg.executar(
            "SELECT g AS octeto FROM generate_series(1, 254) g \
             WHERE g NOT IN (SELECT octeto FROM phx_rede) ORDER BY g LIMIT 1",
            &[],
        )?;
        let octeto: u8 = livre
            .valor(0, "octeto")
            .ok_or("as 254 sub-redes estao ocupadas")?
            .parse()
            .map_err(|_| "octeto invalido")?;
        let porta = (PORTA_BASE + octeto as u16).to_string();
        let hash = senha::cifrar_com(senha_rede, self.iteracoes);
        let tc = pki::chave_tls_crypt();
        // O aad amarra o selo ao NOME da rede: nome e unico e nao muda.
        let tc_selada = cofre.selar(tc.as_bytes(), &format!("rede:{nome}"));
        self.pg
            .executar(
                "INSERT INTO phx_rede (servidor_id, dono_id, nome, finalidade, senha_hash, octeto, porta, tls_crypt_selada) \
                 VALUES ($1::int, $2::int, $3, $4, $5, $6::smallint, $7::int, $8)",
                &[
                    Some(&srv),
                    Some(&dono.id.to_string()),
                    Some(nome),
                    Some(finalidade),
                    Some(&hash),
                    Some(&octeto.to_string()),
                    Some(&porta),
                    Some(&tc_selada),
                ],
            )
            .map_err(|e| traduzir_unico(e, "ja existe rede com esse nome"))?;
        self.entrar_na_rede(dono, nome, senha_rede)
    }

    /// Entra (ou reentra) na rede: confere a senha dela, reserva o IP fixo,
    /// emite certificado novo e devolve o perfil `.ovpn`.
    pub fn entrar_na_rede(&mut self, usuario: &Usuario, nome: &str, senha_rede: &str) -> R<String> {
        let cofre = self.cofre()?.clone();
        let r = self.pg.executar(
            "SELECT r.id, r.nome, r.senha_hash, r.octeto, r.porta, r.tls_crypt_selada, \
                    s.nome AS srv_nome, s.ip, s.dns \
             FROM phx_rede r JOIN phx_servidor s ON s.id = r.servidor_id WHERE r.nome = $1",
            &[Some(nome)],
        )?;
        let negado = || "rede ou senha da rede nao conferem".to_string();
        let hash = r.valor(0, "senha_hash").ok_or_else(negado)?;
        if !senha::conferir(senha_rede, hash) {
            return Err(negado());
        }
        let campo = |c: &str| r.valor(0, c).unwrap_or_default().to_string();
        let rede_id = campo("id");
        let octeto: u8 = campo("octeto").parse().map_err(|_| "octeto invalido")?;
        let porta: u16 = campo("porta").parse().map_err(|_| "porta invalida")?;
        let tc = String::from_utf8(
            cofre.abrir_com(&campo("tls_crypt_selada"), &format!("rede:{nome}"))?,
        )
        .map_err(|_| "chave tls-crypt torta")?;

        let (org, ac_privada) = self.ac()?;
        let cn = format!("{}.{}", usuario.login, rede_id);
        let emissora = Ac {
            organizacao: &org,
            cn: &cn_ac(&org),
            privada: &ac_privada,
        };
        let e = pki::emitir(Papel::Membro, &org, &cn, 2, Some(&emissora))?;

        // Reentrada mantem o IP; entrada nova pega o menor livre.
        let ja = self.pg.executar(
            "SELECT host FROM phx_membro WHERE rede_id = $1::int AND usuario_id = $2::int",
            &[Some(&rede_id), Some(&usuario.id.to_string())],
        )?;
        let host: u8 = match ja.valor(0, "host") {
            Some(h) => {
                self.pg.executar(
                    "UPDATE phx_membro SET cert_serie = $3 WHERE rede_id = $1::int AND usuario_id = $2::int",
                    &[Some(&rede_id), Some(&usuario.id.to_string()), Some(&e.serie_hex)],
                )?;
                h.parse().map_err(|_| "host invalido")?
            }
            None => {
                let livre = self.pg.executar(
                    "SELECT g AS host FROM generate_series(2, 254) g \
                     WHERE g NOT IN (SELECT host FROM phx_membro WHERE rede_id = $1::int) ORDER BY g LIMIT 1",
                    &[Some(&rede_id)],
                )?;
                let h = livre
                    .valor(0, "host")
                    .ok_or("a rede esta cheia (253 membros)")?
                    .to_string();
                self.pg.executar(
                    "INSERT INTO phx_membro (rede_id, usuario_id, host, cn, cert_serie) \
                     VALUES ($1::int, $2::int, $3::smallint, $4, $5)",
                    &[
                        Some(&rede_id),
                        Some(&usuario.id.to_string()),
                        Some(&h),
                        Some(&cn),
                        Some(&e.serie_hex),
                    ],
                )?;
                h.parse().map_err(|_| "host invalido")?
            }
        };

        let ac_pem = self.ac_pem()?;
        let (srv_nome, ip, dns) = (campo("srv_nome"), campo("ip"), campo("dns"));
        let endereco = if dns.is_empty() { ip } else { dns };
        let rede = ovpn::Rede {
            nome,
            porta,
            octeto,
        };
        self.materializar_rede(&rede_id)?;
        fs::write(
            self.dir_rede(&rede_id).join("ccd").join(&cn),
            ovpn::ccd_membro(octeto, host),
        )
        .map_err(|e| format!("gravar ccd: {e}"))?;
        Ok(ovpn::perfil_membro(&ovpn::Perfil {
            rede: &rede,
            servidor: &ovpn::Servidor {
                nome: &srv_nome,
                endereco: &endereco,
            },
            ca_pem: &ac_pem,
            cert_pem: &pki::cert_pem(&e.der),
            chave_pem: &pki::chave_pem(&e.privada),
            tls_crypt: &tc,
        }))
    }

    /// Sai da rede: apaga o vinculo e o arquivo `ccd/`, e com `ccd-exclusive`
    /// o OpenVPN deixa de aceitar o certificado nesta rede.
    pub fn sair_da_rede(&mut self, usuario: &Usuario, rede_id: i64) -> R<()> {
        let r = self.pg.executar(
            "DELETE FROM phx_membro WHERE rede_id = $1::int AND usuario_id = $2::int RETURNING cn",
            &[Some(&rede_id.to_string()), Some(&usuario.id.to_string())],
        )?;
        let cn = r
            .valor(0, "cn")
            .ok_or("voce nao e membro desta rede")?
            .to_string();
        let _ = fs::remove_file(self.dir_rede(&rede_id.to_string()).join("ccd").join(cn));
        Ok(())
    }

    /// Redes visiveis ao usuario: as dele (membro) com o IP, e as demais so com
    /// nome e contagem -- o admin ve todas; o usuario comum ve as que integra.
    pub fn redes(&mut self, u: &Usuario) -> R<Json> {
        let r = self.pg.executar(
            "SELECT r.id, r.nome, r.finalidade, r.porta, r.octeto, s.nome AS servidor, d.login AS dono, \
                    (SELECT count(*) FROM phx_membro m WHERE m.rede_id = r.id) AS membros, \
                    (SELECT m.host FROM phx_membro m WHERE m.rede_id = r.id AND m.usuario_id = $1::int) AS meu_host \
             FROM phx_rede r JOIN phx_servidor s ON s.id = r.servidor_id JOIN phx_usuario d ON d.id = r.dono_id \
             WHERE $2::boolean OR EXISTS (SELECT 1 FROM phx_membro m WHERE m.rede_id = r.id AND m.usuario_id = $1::int) \
             ORDER BY r.nome",
            &[Some(&u.id.to_string()), Some(if u.admin { "true" } else { "false" })],
        )?;
        let mut lista = Vec::new();
        for i in 0..r.linhas.len() {
            let v = |c: &str| r.valor(i, c).unwrap_or_default().to_string();
            let octeto: u8 = v("octeto").parse().unwrap_or(0);
            let meu_ip = r
                .valor(i, "meu_host")
                .and_then(|h| h.parse::<u8>().ok())
                .map(|h| Json::texto_de(ovpn::ip_membro(octeto, h)))
                .unwrap_or(Json::Nulo);
            lista.push(Json::objeto(vec![
                ("id", Json::de_i64(v("id").parse().unwrap_or(0))),
                ("nome", Json::texto_de(v("nome"))),
                ("finalidade", Json::texto_de(v("finalidade"))),
                ("servidor", Json::texto_de(v("servidor"))),
                ("dono", Json::texto_de(v("dono"))),
                ("porta", Json::de_i64(v("porta").parse().unwrap_or(0))),
                (
                    "subrede",
                    Json::texto_de(format!("{}/24", ovpn::subrede(octeto))),
                ),
                ("membros", Json::de_i64(v("membros").parse().unwrap_or(0))),
                ("meu_ip", meu_ip),
            ]));
        }
        Ok(Json::Lista(lista))
    }

    /// Membros de uma rede, com IP e se estao conectados agora (lido do
    /// `status.log` do OpenVPN daquela rede, quando existe).
    pub fn membros(&mut self, u: &Usuario, rede_id: i64) -> R<Json> {
        let id = rede_id.to_string();
        let pode = self.pg.executar(
            "SELECT 1 FROM phx_membro WHERE rede_id = $1::int AND usuario_id = $2::int",
            &[Some(&id), Some(&u.id.to_string())],
        )?;
        if !u.admin && pode.linhas.is_empty() {
            return Err("so membro da rede ve os outros membros".into());
        }
        let r = self.pg.executar(
            "SELECT u.login, m.host, m.cn, r.octeto, m.entrou_em::text AS entrou_em \
             FROM phx_membro m JOIN phx_usuario u ON u.id = m.usuario_id JOIN phx_rede r ON r.id = m.rede_id \
             WHERE m.rede_id = $1::int ORDER BY m.host",
            &[Some(&id)],
        )?;
        let online = conectados(&self.dir_rede(&id).join("status.log"));
        let mut lista = Vec::new();
        for i in 0..r.linhas.len() {
            let v = |c: &str| r.valor(i, c).unwrap_or_default().to_string();
            let octeto: u8 = v("octeto").parse().unwrap_or(0);
            let host: u8 = v("host").parse().unwrap_or(0);
            lista.push(Json::objeto(vec![
                ("login", Json::texto_de(v("login"))),
                ("ip", Json::texto_de(ovpn::ip_membro(octeto, host))),
                ("online", Json::de_bool(online.contains(&v("cn")))),
                ("entrou_em", Json::texto_de(v("entrou_em"))),
            ]));
        }
        Ok(Json::Lista(lista))
    }

    pub fn empresa(&mut self) -> R<Json> {
        let r = self.pg.executar(
            "SELECT nome, finalidade, responsavel, email, telefone, \
                    (certificado_pem IS NOT NULL) AS tem_certificado FROM phx_empresa LIMIT 1",
            &[],
        )?;
        let v = para_json(&r, &["tem_certificado"], &[]);
        Ok(match v {
            Json::Lista(mut l) if !l.is_empty() => l.remove(0),
            _ => Json::Nulo,
        })
    }

    fn ac_pem(&mut self) -> R<String> {
        let r = self
            .pg
            .executar("SELECT ac_cert_pem FROM phx_empresa LIMIT 1", &[])?;
        Ok(r.valor(0, "ac_cert_pem").ok_or("AC ausente")?.to_string())
    }

    fn dir_rede(&self, id: &str) -> PathBuf {
        self.dados.join("redes").join(id)
    }

    /// Escreve em disco o que o OpenVPN daquela rede le: conf, AC, certificado
    /// e chave do servidor, `tls-crypt` e o diretorio `ccd/`. Chave em claro no
    /// disco e o preco do OpenVPN ler sem perguntar senha; por isso 0600.
    pub fn materializar_rede(&mut self, rede_id: &str) -> R<PathBuf> {
        let cofre = self.cofre()?.clone();
        let r = self.pg.executar(
            "SELECT r.nome, r.porta, r.octeto, r.tls_crypt_selada, s.nome AS srv, s.cert_pem, s.chave_selada \
             FROM phx_rede r JOIN phx_servidor s ON s.id = r.servidor_id WHERE r.id = $1::int",
            &[Some(rede_id)],
        )?;
        let v = |c: &str| r.valor(0, c).unwrap_or_default().to_string();
        let nome = v("nome");
        if nome.is_empty() {
            return Err(format!("rede {rede_id} nao existe"));
        }
        let dir = self.dir_rede(rede_id);
        fs::create_dir_all(dir.join("ccd")).map_err(|e| format!("criar {}: {e}", dir.display()))?;
        let chave: [u8; 32] = cofre
            .abrir_com(&v("chave_selada"), &format!("servidor:{}", v("srv")))?
            .try_into()
            .map_err(|_| "chave do servidor torta")?;
        let tc = cofre.abrir_com(&v("tls_crypt_selada"), &format!("rede:{nome}"))?;
        let conf = ovpn::conf_servidor(
            &ovpn::Rede {
                nome: &nome,
                porta: v("porta").parse().map_err(|_| "porta invalida")?,
                octeto: v("octeto").parse().map_err(|_| "octeto invalido")?,
            },
            &dir.display().to_string(),
        );
        let ac_pem = self.ac_pem()?;
        gravar(&dir.join("servidor.conf"), conf.as_bytes(), false)?;
        gravar(&dir.join("ca.crt"), ac_pem.as_bytes(), false)?;
        gravar(&dir.join("servidor.crt"), v("cert_pem").as_bytes(), false)?;
        gravar(
            &dir.join("servidor.key"),
            pki::chave_pem(&chave).as_bytes(),
            true,
        )?;
        gravar(&dir.join("tls-crypt.key"), &tc, true)?;
        Ok(dir)
    }

    /// Materializa todas as redes (arranque do painel) e devolve os diretorios.
    pub fn materializar_todas(&mut self) -> R<Vec<(String, PathBuf)>> {
        let r = self
            .pg
            .executar("SELECT id, nome FROM phx_rede ORDER BY id", &[])?;
        let mut saida = Vec::new();
        for i in 0..r.linhas.len() {
            let id = r.valor(i, "id").unwrap_or_default().to_string();
            let nome = r.valor(i, "nome").unwrap_or_default().to_string();
            saida.push((nome, self.materializar_rede(&id)?));
        }
        // O ccd de cada membro sai do banco: e ele que decide quem conecta.
        let m = self.pg.executar(
            "SELECT m.rede_id, m.cn, m.host, r.octeto FROM phx_membro m JOIN phx_rede r ON r.id = m.rede_id",
            &[],
        )?;
        for i in 0..m.linhas.len() {
            let v = |c: &str| m.valor(i, c).unwrap_or_default().to_string();
            let caminho = self.dir_rede(&v("rede_id")).join("ccd").join(v("cn"));
            let octeto: u8 = v("octeto").parse().unwrap_or(0);
            let host: u8 = v("host").parse().unwrap_or(0);
            gravar(&caminho, ovpn::ccd_membro(octeto, host).as_bytes(), false)?;
        }
        Ok(saida)
    }
}

fn gravar(caminho: &Path, dados: &[u8], secreto: bool) -> R<()> {
    fs::write(caminho, dados).map_err(|e| format!("gravar {}: {e}", caminho.display()))?;
    #[cfg(unix)]
    if secreto {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(caminho, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("permissao de {}: {e}", caminho.display()))?;
    }
    #[cfg(not(unix))]
    let _ = secreto;
    Ok(())
}

fn cn_ac(org: &str) -> String {
    format!("phxvpn AC - {org}")
}

/// CNs conectados agora, lidos do `status.log` versao 2 do OpenVPN
/// (`CLIENT_LIST,<cn>,...`). Arquivo ausente = ninguem conectado.
fn conectados(status: &Path) -> Vec<String> {
    fs::read_to_string(status)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.strip_prefix("CLIENT_LIST,"))
        .filter_map(|l| l.split(',').next())
        .map(str::to_string)
        .collect()
}

fn usuario_da_linha(r: &Resposta, i: usize) -> R<Usuario> {
    Ok(Usuario {
        id: r
            .valor(i, "id")
            .and_then(|v| v.parse().ok())
            .ok_or("usuario sem id")?,
        login: r.valor(i, "login").unwrap_or_default().to_string(),
        admin: r.valor(i, "admin") == Some("t"),
    })
}

fn para_json(r: &Resposta, booleanos: &[&str], inteiros: &[&str]) -> Json {
    Json::Lista(
        r.linhas
            .iter()
            .map(|l| {
                Json::Objeto(
                    r.colunas
                        .iter()
                        .zip(l)
                        .map(|(c, v)| {
                            let j = match v {
                                None => Json::Nulo,
                                Some(t) if booleanos.contains(&c.as_str()) => {
                                    Json::de_bool(t == "t")
                                }
                                Some(t) if inteiros.contains(&c.as_str()) => {
                                    Json::de_i64(t.parse().unwrap_or(0))
                                }
                                Some(t) => Json::texto_de(t.clone()),
                            };
                            (c.clone(), j)
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

/// A violacao de unicidade (SQLSTATE 23505) vira frase de gente; o resto passa.
fn traduzir_unico(e: String, frase: &str) -> String {
    if e.contains(" 23505:") {
        frase.to_string()
    } else {
        e
    }
}

// ------------------------------------------------------------- validacao ----

fn validar_nome(campo: &str, v: &str) -> R<()> {
    let ok = (2..=40).contains(&v.chars().count())
        && v.chars().all(|c| c.is_alphanumeric() || " _.-".contains(c))
        && !v.starts_with(' ')
        && !v.ends_with(' ');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{campo}: de 2 a 40 letras, numeros, espaco, _ . ou -"
        ))
    }
}

fn validar_senha(campo: &str, v: &str, minimo: usize) -> R<()> {
    if v.chars().count() < minimo {
        return Err(format!("{campo}: no minimo {minimo} caracteres"));
    }
    Ok(())
}

fn validar_ip(v: &str) -> R<()> {
    v.parse::<std::net::IpAddr>()
        .map(|_| ())
        .map_err(|_| format!("IP do servidor invalido: {v}"))
}

fn validar_dns(v: &str) -> R<()> {
    let ok = v.is_empty()
        || (v.len() <= 253
            && v.split('.').all(|p| {
                !p.is_empty()
                    && p.len() <= 63
                    && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                    && !p.starts_with('-')
                    && !p.ends_with('-')
            }));
    if ok {
        Ok(())
    } else {
        Err(format!("DNS do servidor invalido: {v}"))
    }
}

pub fn validar_instalacao(i: &Instalacao) -> R<()> {
    let obrigatorio = [
        ("nome da empresa", &i.empresa),
        ("responsavel", &i.responsavel),
        ("e-mail", &i.email),
    ];
    for (campo, v) in obrigatorio {
        if v.trim().is_empty() {
            return Err(format!("{campo} e obrigatorio"));
        }
    }
    if !i.email.contains('@') {
        return Err("e-mail invalido".into());
    }
    validar_nome("usuario admin", &i.admin_usuario)?;
    validar_senha("senha admin", &i.admin_senha, 8)?;
    validar_senha("senha mestre", &i.senha_mestre, 12)?;
    if i.senha_mestre == i.admin_senha {
        // Iguais, quem descobre a de login destranca o cofre: a separacao das
        // duas e o motivo de existir senha mestre.
        return Err("a senha mestre tem de ser diferente da senha admin".into());
    }
    validar_nome("nome do servidor", &i.servidor_nome)?;
    validar_ip(&i.servidor_ip)?;
    validar_dns(&i.servidor_dns)?;
    if !i.certificado_pem.trim().is_empty() {
        // Analisa em vez de procurar a faixa: texto com a faixa e lixo dentro
        // passaria por um `contains` e ficaria gravado como certificado.
        let valido = i.certificado_pem.contains("-----BEGIN CERTIFICATE-----")
            && pki::de_pem(&i.certificado_pem)
                .ok()
                .and_then(|der| {
                    phxsql_core::asn1::esperar(&der, phxsql_core::asn1::TAG_SEQUENCE)
                        .ok()
                        .map(|(_, resto)| resto.is_empty())
                })
                .unwrap_or(false);
        if !valido {
            return Err(
                "certificado digital: cole o certificado em PEM (-----BEGIN CERTIFICATE-----)"
                    .into(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn base() -> Instalacao {
        Instalacao {
            empresa: "Empresa".into(),
            responsavel: "Fulano".into(),
            email: "f@e.com".into(),
            admin_usuario: "admin".into(),
            admin_senha: "senha-admin".into(),
            senha_mestre: "senha-mestre-longa".into(),
            servidor_nome: "vpn1".into(),
            servidor_ip: "203.0.113.10".into(),
            ..Default::default()
        }
    }

    #[test]
    fn instalacao_valida_passa() {
        validar_instalacao(&base()).unwrap();
    }

    #[test]
    fn certificado_torto_e_recusado_e_o_bom_passa() {
        let mut i = base();
        i.certificado_pem =
            "-----BEGIN CERTIFICATE-----\nlixo!!\n-----END CERTIFICATE-----\n".into();
        assert!(validar_instalacao(&i).is_err());
        let ac = pki::emitir(Papel::Ac, "E", "AC", 1, None).unwrap();
        i.certificado_pem = pki::cert_pem(&ac.der);
        validar_instalacao(&i).unwrap();
    }

    #[test]
    fn senha_mestre_igual_a_admin_e_recusada() {
        let mut i = base();
        i.senha_mestre = "mesma-senha-12".into();
        i.admin_senha = "mesma-senha-12".into();
        assert!(validar_instalacao(&i).is_err());
    }

    #[test]
    fn ip_e_dns_tortos_sao_recusados() {
        let mut i = base();
        i.servidor_ip = "300.1.1.1".into();
        assert!(validar_instalacao(&i).is_err());
        let mut i = base();
        i.servidor_dns = "vpn..empresa".into();
        assert!(validar_instalacao(&i).is_err());
    }

    #[test]
    fn nome_com_barra_nao_vira_caminho() {
        // O nome da rede nao vai para caminho de arquivo (o diretorio e o id),
        // mas o login vai no CN: barra e aspas ficam fora.
        assert!(validar_nome("login", "../x").is_err());
        assert!(validar_nome("login", "joao.silva").is_ok());
    }

    #[test]
    fn status_do_openvpn_da_os_conectados() {
        let d = std::env::temp_dir().join(format!("phxvpn-status-{}", std::process::id()));
        fs::write(
            &d,
            "TITLE,OpenVPN\nHEADER,CLIENT_LIST,...\nCLIENT_LIST,ana.1,198.51.100.2:5000,10.77.1.2\nEND\n",
        )
        .unwrap();
        assert_eq!(conectados(&d), vec!["ana.1".to_string()]);
        let _ = fs::remove_file(d);
    }
}
