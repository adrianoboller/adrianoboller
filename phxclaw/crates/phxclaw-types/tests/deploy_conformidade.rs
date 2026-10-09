//! Conformidade de `deploy/`: Dockerfile, compose, manifestos k8s e o ignore que o Docker LE.
//!
//! Roda no `cargo test -p phxclaw-types` e NAO reescreve a deteccao de credencial: chama o
//! motor unico (`phxclaw_types::segredo`), o mesmo do broker, do `config.json` e da entrada
//! de fluxo. Se a lista de prefixos ou de nomes crescer la, este teste cresce junto.
//!
//! O leitor de YAML abaixo e MINIMO e de proposito (so a std, como o resto da base): cobre
//! mapas, listas com `- `, escalares, listas em linha `[a, b]`, blocos `|` e `---`. Nao e um
//! parser de YAML geral, e o primeiro teste (`o_leitor_le_o_que_os_manifestos_usam`) trava o
//! que ele promete ler, para um arquivo novo nao ser aprovado por ele ter lido NADA.
//!
//! Cada conferencia tem o seu mutante: o teste `mutantes_*` parte do arquivo real, aplica
//! UM defeito e exige que a conferencia o nomeie. Teste que so le o arquivo bom passaria
//! mesmo se a conferencia nunca recusasse nada.

use phxclaw_types::segredo::{nome_de_segredo, texto_tem_credencial};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------------------
// leitor minimo de YAML
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Entrada {
    caminho: String,
    /// O nome cru da chave (`master.key` fica inteiro); vazio num escalar de lista.
    chave: String,
    valor: String,
    linha: usize,
}

struct No {
    indent: usize,
    item: bool,
    seg: String,
    cont: usize,
}

fn sem_comentario(l: &str) -> String {
    let (mut simples, mut duplas) = (false, false);
    let mut saida = String::new();
    let mut anterior = ' ';
    for c in l.chars() {
        match c {
            '\'' if !duplas => simples = !simples,
            '"' if !simples => duplas = !duplas,
            '#' if !simples && !duplas && anterior.is_whitespace() => break,
            _ => {}
        }
        saida.push(c);
        anterior = c;
    }
    saida.trim_end().to_string()
}

fn sem_aspas(v: &str) -> String {
    let v = v.trim();
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            return v[1..v.len() - 1].to_string();
        }
    }
    v.to_string()
}

/// `chave: valor` -> (chave, valor). `:` dentro de aspas ou de `[ ]`/`{ }` nao divide, e
/// `:` colado (`127.0.0.1:8787`, `/tmp:size=64m`) tambem nao: so `: ` ou `:` no fim.
fn dividir_chave(s: &str) -> Option<(String, String)> {
    if s.starts_with(['"', '\'', '[', '{']) {
        return None;
    }
    let (mut simples, mut duplas, mut fundo) = (false, false, 0i32);
    let cs: Vec<char> = s.chars().collect();
    for (i, c) in cs.iter().enumerate() {
        match c {
            '\'' if !duplas => simples = !simples,
            '"' if !simples => duplas = !duplas,
            '[' | '{' if !simples && !duplas => fundo += 1,
            ']' | '}' if !simples && !duplas => fundo -= 1,
            ':' if !simples && !duplas && fundo == 0 && (i + 1 == cs.len() || cs[i + 1] == ' ') => {
                let k: String = cs[..i].iter().collect();
                let v: String = cs[i + 1..].iter().collect();
                return Some((sem_aspas(&k), v.trim().to_string()));
            }
            _ => {}
        }
    }
    None
}

fn juntar(pilha: &[No], chave: Option<&str>) -> String {
    let mut s = String::new();
    for n in pilha {
        if !n.item && !s.is_empty() {
            s.push('.');
        }
        s.push_str(&n.seg);
    }
    if let Some(k) = chave {
        if !s.is_empty() {
            s.push('.');
        }
        s.push_str(k);
    }
    s
}

/// Os documentos (separados por `---`) como listas planas de entradas.
fn ler_yaml(texto: &str) -> Vec<Vec<Entrada>> {
    let mut docs: Vec<Vec<Entrada>> = vec![Vec::new()];
    let mut pilha: Vec<No> = Vec::new();
    let mut raiz_cont = 0usize;
    let mut bloco: Option<(usize, usize)> = None;
    for (n, bruta) in texto.lines().enumerate() {
        let linha = n + 1;
        if bruta.trim_end() == "---" {
            docs.push(Vec::new());
            pilha.clear();
            raiz_cont = 0;
            bloco = None;
            continue;
        }
        let l = sem_comentario(bruta);
        let indent = l.len() - l.trim_start().len();
        if let Some((ind_bloco, idx)) = bloco {
            if l.trim().is_empty() || indent > ind_bloco {
                let doc = docs.last_mut().unwrap();
                doc[idx].valor.push_str(l.trim());
                doc[idx].valor.push('\n');
                continue;
            }
            bloco = None;
        }
        if l.trim().is_empty() {
            continue;
        }
        let mut resto = l.trim_start().to_string();
        let mut ind = indent;
        if resto == "-" || resto.starts_with("- ") {
            while let Some(t) = pilha.last() {
                if t.indent > ind || (t.indent == ind && t.item) {
                    pilha.pop();
                } else {
                    break;
                }
            }
            let idx = match pilha.last_mut() {
                Some(p) => {
                    p.cont += 1;
                    p.cont - 1
                }
                None => {
                    raiz_cont += 1;
                    raiz_cont - 1
                }
            };
            pilha.push(No {
                indent: ind,
                item: true,
                seg: format!("[{idx}]"),
                cont: 0,
            });
            let apos = resto[1..].to_string();
            let brancos = apos.len() - apos.trim_start().len();
            resto = apos.trim_start().to_string();
            ind += 1 + brancos;
            if resto.is_empty() {
                continue;
            }
        }
        let doc = docs.last_mut().unwrap();
        match dividir_chave(&resto) {
            Some((k, v)) => {
                while let Some(t) = pilha.last() {
                    if t.indent >= ind && !(t.item && t.indent < ind) {
                        pilha.pop();
                    } else {
                        break;
                    }
                }
                if v.is_empty() {
                    pilha.push(No {
                        indent: ind,
                        item: false,
                        seg: k,
                        cont: 0,
                    });
                } else {
                    let caminho = juntar(&pilha, Some(&k));
                    let eh_bloco = v.starts_with(['|', '>']);
                    doc.push(Entrada {
                        caminho,
                        chave: k,
                        valor: if eh_bloco {
                            String::new()
                        } else {
                            sem_aspas(&v)
                        },
                        linha,
                    });
                    if eh_bloco {
                        bloco = Some((ind, doc.len() - 1));
                    }
                }
            }
            None => doc.push(Entrada {
                caminho: juntar(&pilha, None),
                chave: String::new(),
                valor: sem_aspas(&resto),
                linha,
            }),
        }
    }
    docs.retain(|d| !d.is_empty());
    docs
}

fn get<'a>(doc: &'a [Entrada], caminho: &str) -> Option<&'a str> {
    doc.iter()
        .find(|e| e.caminho == caminho)
        .map(|e| e.valor.as_str())
}

/// Os prefixos `base[n]` distintos, em ordem (`containers[0]`, `containers[1]`...).
fn itens(doc: &[Entrada], base: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for e in doc {
        if let Some(r) = e.caminho.strip_prefix(base)
            && r.starts_with('[')
            && let Some(f) = r.find(']')
        {
            let p = format!("{base}{}", &r[..=f]);
            if !v.contains(&p) {
                v.push(p);
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------
// conferencias comuns a todo YAML
// ---------------------------------------------------------------------------------------

fn vazio(v: &str) -> bool {
    matches!(v.trim(), "" | "{}" | "[]" | "null" | "~")
}

/// Segredo no YAML: forma de credencial (motor unico), nome de segredo com valor literal,
/// `NOME=valor` em lista de ambiente e o par `name:`/`value:` do k8s. O VALOR nunca entra
/// na mensagem: o relatorio de uma conferencia de segredo nao pode ser ele o vazamento.
fn segredos_no_yaml(arq: &str, doc: &[Entrada], v: &mut Vec<String>) {
    for e in doc {
        if texto_tem_credencial(&e.valor) {
            v.push(format!(
                "{arq}:{}: valor com forma de credencial em `{}`",
                e.linha, e.caminho
            ));
            continue;
        }
        // `secretKeyRef.key`, `items[n].key` e afins NOMEIAM o segredo, nao o contem.
        let referencia = ["secretKeyRef", "configMapKeyRef", "items["]
            .iter()
            .any(|p| e.caminho.contains(p));
        if !e.chave.is_empty() && !vazio(&e.valor) && !referencia && nome_de_segredo(&e.chave) {
            v.push(format!(
                "{arq}:{}: `{}` tem nome de segredo e valor literal (use arquivo/secret)",
                e.linha, e.chave
            ));
        }
        if e.chave.is_empty()
            && let Some((k, val)) = e.valor.split_once('=')
            && !val.trim().is_empty()
            && nome_de_segredo(k)
        {
            v.push(format!(
                "{arq}:{}: `{k}=...` com valor literal no ambiente",
                e.linha
            ));
        }
        if let Some(pai) = e.caminho.strip_suffix(".name")
            && e.chave == "name"
            && nome_de_segredo(&e.valor)
            && let Some(val) = get(doc, &format!("{pai}.value"))
            && !vazio(val)
        {
            v.push(format!(
                "{arq}:{}: variavel `{}` com `value` literal (use secretKeyRef)",
                e.linha, e.valor
            ));
        }
    }
}

// ---------------------------------------------------------------------------------------
// Dockerfile
// ---------------------------------------------------------------------------------------

/// As instrucoes com a continuacao `\` colada, sem comentario: (numero da linha, texto).
fn instrucoes(texto: &str) -> Vec<(usize, String)> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    let mut inicio = 0usize;
    for (n, l) in texto.lines().enumerate() {
        let t = l.trim();
        if t.starts_with('#') {
            continue;
        }
        if atual.is_empty() {
            if t.is_empty() {
                continue;
            }
            inicio = n + 1;
        }
        if let Some(sem) = t.strip_suffix('\\') {
            atual.push_str(sem);
            atual.push(' ');
        } else {
            atual.push_str(t);
            saida.push((inicio, std::mem::take(&mut atual)));
        }
    }
    saida
}

fn usuario_root(u: &str) -> bool {
    let nome = u.split(':').next().unwrap_or(u).trim();
    nome.eq_ignore_ascii_case("root") || nome == "0"
}

/// `ENV A=b C=d`, `ENV A b` e `ARG A=b`: os pares (chave, valor).
fn pares_env(args: &str) -> Vec<(String, String)> {
    if args.contains('=') {
        args.split_whitespace()
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.to_string(), sem_aspas(v)))
            .collect()
    } else {
        let mut it = args.splitn(2, char::is_whitespace);
        match (it.next(), it.next()) {
            (Some(k), Some(v)) => vec![(k.to_string(), sem_aspas(v))],
            (Some(k), None) => vec![(k.to_string(), String::new())],
            _ => vec![],
        }
    }
}

fn conferir_dockerfile(texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    for (n, l) in texto.lines().enumerate() {
        if texto_tem_credencial(l) {
            v.push(format!("Dockerfile:{}: forma de credencial", n + 1));
        }
    }
    let ins = instrucoes(texto);
    let froms: Vec<usize> = ins
        .iter()
        .enumerate()
        .filter(|(_, (_, t))| t.to_ascii_uppercase().starts_with("FROM "))
        .map(|(i, _)| i)
        .collect();
    if froms.len() < 2 {
        v.push("Dockerfile: nao e multi-estagio (precisa de pelo menos 2 FROM)".into());
    }
    let Some(&ultimo) = froms.last() else {
        v.push("Dockerfile: sem FROM".into());
        return v;
    };
    let final_ = &ins[ultimo..];
    let de = |nome: &str| -> Vec<&(usize, String)> {
        final_
            .iter()
            .filter(|(_, t)| {
                t.split_whitespace()
                    .next()
                    .is_some_and(|p| p.eq_ignore_ascii_case(nome))
            })
            .collect()
    };
    match de("USER").last() {
        None => v.push("Dockerfile: a imagem final nao tem USER (rodaria como root)".into()),
        Some((n, t)) => {
            let u = t.split_whitespace().nth(1).unwrap_or("");
            if usuario_root(u) {
                v.push(format!("Dockerfile:{n}: a imagem final roda como root"));
            }
        }
    }
    match de("HEALTHCHECK").last() {
        None => v.push("Dockerfile: a imagem final nao tem HEALTHCHECK".into()),
        Some((n, t)) if t.to_ascii_uppercase().contains("NONE") && !t.contains("CMD") => {
            v.push(format!("Dockerfile:{n}: HEALTHCHECK NONE"));
        }
        _ => {}
    }
    if de("EXPOSE").is_empty() {
        v.push("Dockerfile: a imagem final nao declara a porta da API (EXPOSE)".into());
    }
    if de("VOLUME").is_empty() {
        v.push("Dockerfile: a imagem final nao declara o volume de dados (VOLUME)".into());
    }
    for (n, t) in ins.iter() {
        let cabeca = t
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        if cabeca == "ENV" || cabeca == "ARG" {
            let args = t.split_once(char::is_whitespace).map_or("", |(_, r)| r);
            for (k, val) in pares_env(args) {
                if !val.is_empty() && nome_de_segredo(&k) {
                    v.push(format!(
                        "Dockerfile:{n}: {cabeca} `{k}` com nome de segredo e valor literal"
                    ));
                }
            }
        }
    }
    // O estagio final so copia o binario: `COPY . .` ali levaria o contexto inteiro (e o que
    // o .dockerignore deixasse passar) para a imagem que roda.
    for (n, t) in final_ {
        let cabeca = t
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        if cabeca == "COPY" || cabeca == "ADD" {
            let fontes: Vec<&str> = t
                .split_whitespace()
                .skip(1)
                .filter(|p| !p.starts_with("--"))
                .collect();
            let fontes = &fontes[..fontes.len().saturating_sub(1)];
            let de_estagio = t.contains("--from=");
            for f in fontes {
                let suspeito = [
                    ".env",
                    ".key",
                    ".pem",
                    "master",
                    "api.token",
                    "secret",
                    "segredo",
                ]
                .iter()
                .any(|p| f.to_ascii_lowercase().contains(p));
                if (!de_estagio && (*f == "." || *f == "./")) || suspeito {
                    v.push(format!(
                        "Dockerfile:{n}: {cabeca} de `{f}` na imagem final (contexto inteiro ou arquivo de segredo)"
                    ));
                }
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------
// .dockerignore
// ---------------------------------------------------------------------------------------

fn conferir_dockerignore(texto: &str) -> Vec<String> {
    let linhas: Vec<&str> = texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let mut v = Vec::new();
    // (o que tem de estar excluido, as grafias aceitas)
    let exigidos: [(&str, &[&str]); 7] = [
        ("target/", &["target/", "**/target/", "target"]),
        (".git/", &[".git/", ".git", "**/.git"]),
        (".env", &[".env", ".env.*", "*.env", "**/.env"]),
        ("chaves (*.key/*.pem)", &["*.key", "**/*.key", "*.pem"]),
        (
            "cofre/master.key",
            &["**/master.key", "master.key", "**/segredos/", "segredos/"],
        ),
        ("dados (var/)", &["var/", "/var", "var"]),
        (
            "segredos do compose (deploy/secrets/)",
            &["deploy/secrets/", "deploy/secrets", "**/secrets/"],
        ),
    ];
    for (nome, aceitas) in exigidos {
        if !aceitas.iter().any(|a| linhas.contains(a)) {
            v.push(format!("dockerignore: nao exclui {nome}"));
        }
    }
    v
}

/// QUAL arquivo o Docker le. Com `docker build -f deploy/Dockerfile .` (contexto = raiz) o
/// BuildKit procura `deploy/Dockerfile.dockerignore` (o nome do Dockerfile + `.dockerignore`,
/// ao lado dele) e, so na falta, o `.dockerignore` da RAIZ do contexto. Qualquer outro nome
/// -- `deploy/.dockerignore` era o nosso -- nao e lido por ninguem e passa por protecao.
/// `presentes`: caminhos relativos a raiz do repositorio que existem de verdade.
fn ignore_que_o_docker_le(dockerfile: &str, presentes: &[&str]) -> Option<String> {
    let ao_lado = format!("{dockerfile}.dockerignore");
    [ao_lado.as_str(), ".dockerignore"]
        .into_iter()
        .find(|c| presentes.contains(c))
        .map(str::to_string)
}

/// Confere o NOME do ignore (e nao so o conteudo) e a COPY que o Dockerfile faz do contexto.
/// - nenhum arquivo que o Docker leia -> violacao (o conteudo conferido seria de enfeite);
/// - arquivo-isca (`deploy/.dockerignore`) presente -> violacao: parece protecao e nao e;
/// - `COPY . .` (ou `./`, `*`) em qualquer estagio -> so passa se o ignore LIDO excluir
///   segredo, dado, .git e target/; sem ele, recusa.
fn conferir_contexto(
    dockerfile: &str,
    presentes: &[&str],
    ignore_lido: Option<&str>,
) -> Vec<String> {
    let mut v = Vec::new();
    if ignore_que_o_docker_le("deploy/Dockerfile", presentes).is_none() {
        v.push(
            "dockerignore: nenhum arquivo que o Docker leia (esperado deploy/Dockerfile.dockerignore \
             ou .dockerignore na raiz do contexto)"
                .to_string(),
        );
    }
    if presentes.contains(&"deploy/.dockerignore") {
        v.push(
            "dockerignore: deploy/.dockerignore existe e o Docker NAO o le (renomeie para \
             deploy/Dockerfile.dockerignore)"
                .to_string(),
        );
    }
    let mut copia_larga = false;
    for (n, t) in instrucoes(dockerfile) {
        let mut p = t.split_whitespace();
        let cabeca = p.next().unwrap_or("").to_ascii_uppercase();
        if (cabeca != "COPY" && cabeca != "ADD") || t.contains("--from=") {
            continue;
        }
        let args: Vec<&str> = p.filter(|a| !a.starts_with("--")).collect();
        let fontes = &args[..args.len().saturating_sub(1)];
        for f in fontes {
            let bruto = f.trim_start_matches("./");
            if matches!(bruto, "" | "." | "*" | "**") {
                copia_larga = true;
                continue;
            }
            // pasta de segredo/dado/historico copiada pelo nome: nunca, ignore ou nao
            let primeira = bruto.split('/').next().unwrap_or("");
            if matches!(
                primeira,
                "deploy" | "var" | ".git" | "target" | "secrets" | ".phxclaw"
            ) {
                v.push(format!(
                    "Dockerfile:{n}: {cabeca} de `{f}` leva segredo/dado/historico para a camada"
                ));
            }
        }
    }
    if copia_larga {
        match ignore_lido {
            None => v.push(
                "Dockerfile: COPY do contexto inteiro (`COPY . .`) sem ignore lido pelo Docker: \
                 deploy/secrets, var/, .git e target/ entram na camada"
                    .to_string(),
            ),
            Some(txt) => {
                for m in conferir_dockerignore(txt) {
                    v.push(format!(
                        "Dockerfile: COPY do contexto inteiro com ignore falho -- {m}"
                    ));
                }
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------
// manifestos k8s
// ---------------------------------------------------------------------------------------

fn conferir_k8s(arq: &str, texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    for doc in ler_yaml(texto) {
        segredos_no_yaml(arq, &doc, &mut v);
        let kind = get(&doc, "kind").unwrap_or("").to_string();
        match kind.as_str() {
            "Secret" => {
                for e in &doc {
                    let dentro =
                        e.caminho.starts_with("data.") || e.caminho.starts_with("stringData.");
                    if dentro && !vazio(&e.valor) {
                        v.push(format!(
                            "{arq}:{}: o Secret de exemplo tem valor em `{}`",
                            e.linha, e.caminho
                        ));
                    }
                }
            }
            "Service" => {
                if let Some(t) = get(&doc, "spec.type")
                    && matches!(t, "NodePort" | "LoadBalancer")
                {
                    v.push(format!(
                        "{arq}: Service {t} expoe a API fora do cluster (decisao do operador, nao do exemplo)"
                    ));
                }
            }
            "Deployment" => conferir_deployment(arq, &doc, &mut v),
            "NetworkPolicy" => conferir_networkpolicy(arq, &doc, &mut v),
            _ => {}
        }
    }
    v
}

/// A politica tem de de fato RESTRINGIR o ingresso: `ingress: [{}]`, `from` ausente ou
/// `ipBlock 0.0.0.0/0` seriam uma politica que existe e libera tudo -- pior que nenhuma,
/// porque parece isolamento.
fn conferir_networkpolicy(arq: &str, doc: &[Entrada], v: &mut Vec<String>) {
    if get(doc, "spec.podSelector.matchLabels.app.kubernetes.io/name").is_none() {
        v.push(format!(
            "{arq}: NetworkPolicy sem podSelector do agente (valeria para o namespace todo)"
        ));
    }
    if !doc
        .iter()
        .any(|e| e.caminho.starts_with("spec.policyTypes[") && e.valor == "Ingress")
    {
        v.push(format!("{arq}: NetworkPolicy sem policyTypes: [Ingress]"));
    }
    let regras = itens(doc, "spec.ingress");
    if regras.is_empty() {
        v.push(format!("{arq}: NetworkPolicy sem regra de ingresso"));
    }
    for r in &regras {
        let origem = doc.iter().any(|e| {
            e.caminho.starts_with(&format!("{r}.from["))
                && (e.caminho.contains(".namespaceSelector.")
                    || e.caminho.contains(".podSelector."))
        });
        if !origem {
            v.push(format!(
                "{arq}: ingresso sem `from` com namespaceSelector/podSelector (aberto a todos)"
            ));
        }
        if !doc
            .iter()
            .any(|e| e.caminho == format!("{r}.ports[0].port") && e.valor == "8787")
        {
            v.push(format!("{arq}: ingresso sem restringir a porta 8787"));
        }
    }
    if doc
        .iter()
        .any(|e| e.chave == "cidr" && (e.valor == "0.0.0.0/0" || e.valor == "::/0"))
    {
        v.push(format!(
            "{arq}: ipBlock 0.0.0.0/0 no ingresso (aberto a todos)"
        ));
    }
}

fn conferir_deployment(arq: &str, doc: &[Entrada], v: &mut Vec<String>) {
    let pod = "spec.template.spec";
    let pod_nao_root = get(doc, &format!("{pod}.securityContext.runAsNonRoot")) == Some("true");
    let pod_usuario = get(doc, &format!("{pod}.securityContext.runAsUser"));
    for ruim in ["hostNetwork", "hostPID", "hostIPC"] {
        if get(doc, &format!("{pod}.{ruim}")) == Some("true") {
            v.push(format!("{arq}: {ruim}: true"));
        }
    }
    let containers = itens(doc, &format!("{pod}.containers"));
    if containers.is_empty() {
        v.push(format!("{arq}: Deployment sem containers"));
    }
    let init = itens(doc, &format!("{pod}.initContainers"));
    for (c, eh_init) in containers
        .iter()
        .map(|c| (c, false))
        .chain(init.iter().map(|c| (c, true)))
    {
        let nome = get(doc, &format!("{c}.name")).unwrap_or(c);
        let sc = |campo: &str| get(doc, &format!("{c}.securityContext.{campo}"));
        let nao_root = sc("runAsNonRoot").map_or(pod_nao_root, |x| x == "true");
        if !nao_root {
            v.push(format!("{arq}: container `{nome}` sem runAsNonRoot: true"));
        }
        let usuario = sc("runAsUser").or(pod_usuario);
        if usuario == Some("0") {
            v.push(format!("{arq}: container `{nome}` com runAsUser: 0"));
        }
        if sc("readOnlyRootFilesystem") != Some("true") {
            v.push(format!(
                "{arq}: container `{nome}` sem readOnlyRootFilesystem: true"
            ));
        }
        if sc("allowPrivilegeEscalation") != Some("false") {
            v.push(format!(
                "{arq}: container `{nome}` sem allowPrivilegeEscalation: false"
            ));
        }
        if sc("privileged") == Some("true") {
            v.push(format!("{arq}: container `{nome}` privileged: true"));
        }
        let derruba_tudo = doc.iter().any(|e| {
            e.caminho
                .starts_with(&format!("{c}.securityContext.capabilities.drop"))
                && e.valor.contains("ALL")
        });
        if !derruba_tudo {
            v.push(format!(
                "{arq}: container `{nome}` sem capabilities.drop: [ALL]"
            ));
        }
        if let Some(img) = get(doc, &format!("{c}.image"))
            && (img.ends_with(":latest") || !img.contains(':'))
        {
            v.push(format!(
                "{arq}: container `{nome}` com imagem sem versao fixa (`{img}`)"
            ));
        }
        if !eh_init {
            for sonda in ["readinessProbe", "livenessProbe"] {
                if get(doc, &format!("{c}.{sonda}.httpGet.path")).is_none()
                    && get(doc, &format!("{c}.{sonda}.tcpSocket.port")).is_none()
                    && get(doc, &format!("{c}.{sonda}.exec.command")).is_none()
                {
                    v.push(format!("{arq}: container `{nome}` sem {sonda}"));
                }
            }
        }
    }
}

fn conferir_kustomization(dir: &Path, texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    let docs = ler_yaml(texto);
    let Some(doc) = docs.first() else {
        return vec!["kustomization.yaml: vazio".into()];
    };
    if get(doc, "kind") != Some("Kustomization") {
        v.push("kustomization.yaml: nao e Kustomization".into());
    }
    let listados: Vec<&str> = doc
        .iter()
        .filter(|e| e.caminho.starts_with("resources["))
        .map(|e| e.valor.as_str())
        .collect();
    for obrigatorio in [
        "namespace.yaml",
        "configmap.yaml",
        "pvc.yaml",
        "deployment.yaml",
        "service.yaml",
        "networkpolicy.yaml",
    ] {
        if !listados.contains(&obrigatorio) {
            v.push(format!("kustomization.yaml: nao lista {obrigatorio}"));
        }
    }
    for r in listados {
        if !dir.join(r).is_file() {
            v.push(format!("kustomization.yaml: lista `{r}`, que nao existe"));
        }
    }
    v
}

// ---------------------------------------------------------------------------------------
// compose
// ---------------------------------------------------------------------------------------

fn conferir_compose(texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    let docs = ler_yaml(texto);
    let Some(doc) = docs.first() else {
        return vec!["docker-compose.yml: vazio".into()];
    };
    segredos_no_yaml("docker-compose.yml", doc, &mut v);
    let app = "services.phxclaw";
    match get(doc, &format!("{app}.user")) {
        None => v.push("docker-compose.yml: phxclaw sem `user` (nao-root explicito)".into()),
        Some(u) if usuario_root(u) => v.push("docker-compose.yml: phxclaw roda como root".into()),
        _ => {}
    }
    if get(doc, &format!("{app}.read_only")) != Some("true") {
        v.push("docker-compose.yml: phxclaw sem read_only: true".into());
    }
    if !doc
        .iter()
        .any(|e| e.caminho.starts_with(&format!("{app}.cap_drop")) && e.valor == "ALL")
    {
        v.push("docker-compose.yml: phxclaw sem cap_drop: ALL".into());
    }
    for e in doc
        .iter()
        .filter(|e| e.caminho.starts_with(&format!("{app}.ports[")))
    {
        if !(e.valor.starts_with("127.0.0.1:") || e.valor.starts_with("[::1]:")) {
            v.push(format!(
                "docker-compose.yml:{}: porta publicada fora do loopback (`{}`)",
                e.linha, e.valor
            ));
        }
    }
    if !doc
        .iter()
        .any(|e| e.caminho.starts_with(&format!("{app}.secrets[")) && e.valor.contains("api_token"))
    {
        v.push("docker-compose.yml: phxclaw nao recebe o token por `secrets:`".into());
    }
    match get(doc, "services.postgres.profiles") {
        Some(p) if p.contains("postgres") => {}
        _ if doc.iter().any(|e| {
            e.caminho.starts_with("services.postgres.profiles[") && e.valor == "postgres"
        }) => {}
        _ => v.push(
            "docker-compose.yml: o PostgreSQL nao e opcional (sem profiles: [postgres])".into(),
        ),
    }
    if get(doc, "services.postgres.environment.POSTGRES_PASSWORD_FILE").is_none() {
        v.push("docker-compose.yml: PostgreSQL sem POSTGRES_PASSWORD_FILE".into());
    }
    for e in doc.iter().filter(|e| e.caminho.starts_with("secrets.")) {
        if e.chave == "content" {
            v.push(format!(
                "docker-compose.yml:{}: secret com `content:` literal (use `file:`)",
                e.linha
            ));
        }
    }
    v
}

// ---------------------------------------------------------------------------------------
// os arquivos reais
// ---------------------------------------------------------------------------------------

fn deploy() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy")
}

fn ler(rel: &str) -> String {
    let p = deploy().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Os candidatos a ignore que existem em disco (caminhos relativos a raiz do repositorio) e o
/// texto do que o Docker de fato le, se algum.
fn ignore_real() -> (Vec<String>, Option<String>) {
    let raiz = deploy().join("..");
    let presentes: Vec<String> = [
        "deploy/Dockerfile.dockerignore",
        "deploy/.dockerignore",
        ".dockerignore",
    ]
    .iter()
    .filter(|c| raiz.join(c).is_file())
    .map(|c| c.to_string())
    .collect();
    let refs: Vec<&str> = presentes.iter().map(String::as_str).collect();
    let texto = ignore_que_o_docker_le("deploy/Dockerfile", &refs)
        .map(|c| std::fs::read_to_string(raiz.join(&c)).unwrap());
    (presentes, texto)
}

const K8S: [&str; 7] = [
    "namespace.yaml",
    "configmap.yaml",
    "secret-exemplo.yaml",
    "pvc.yaml",
    "deployment.yaml",
    "service.yaml",
    "networkpolicy.yaml",
];

fn todas_as_violacoes() -> Vec<String> {
    let mut v = conferir_dockerfile(&ler("Dockerfile"));
    let (presentes, ignore) = ignore_real();
    let presentes: Vec<&str> = presentes.iter().map(String::as_str).collect();
    v.extend(conferir_contexto(
        &ler("Dockerfile"),
        &presentes,
        ignore.as_deref(),
    ));
    if let Some(txt) = &ignore {
        v.extend(conferir_dockerignore(txt));
    }
    v.extend(conferir_compose(&ler("docker-compose.yml")));
    for a in K8S {
        v.extend(conferir_k8s(a, &ler(&format!("k8s/{a}"))));
    }
    v.extend(conferir_kustomization(
        &deploy().join("k8s"),
        &ler("k8s/kustomization.yaml"),
    ));
    v
}

/// Troca UMA ocorrencia (ou todas) e exige que ela exista: mutante que nao muda nada
/// passaria por «a conferencia nao reclamou».
fn mutar(texto: &str, de: &str, para: &str) -> String {
    assert!(
        texto.contains(de),
        "o arquivo real nao contem `{de}`: o mutante nao mutaria nada"
    );
    texto.replace(de, para)
}

fn token_falso() -> String {
    format!("ghp_{}", "a1B2c3D4e5".repeat(3))
}

fn exige(v: &[String], trecho: &str) {
    assert!(
        v.iter().any(|m| m.contains(trecho)),
        "esperava uma violacao com `{trecho}`; veio: {v:#?}"
    );
}

// ---- o arquivo real ----

#[test]
fn deploy_esta_conforme() {
    let v = todas_as_violacoes();
    assert!(v.is_empty(), "deploy/ nao conforme:\n{}", v.join("\n"));
}

#[test]
fn existem_todos_os_artefatos_e_os_tipos_de_recurso() {
    for a in [
        "Dockerfile",
        "Dockerfile.dockerignore",
        "docker-compose.yml",
        "k8s/kustomization.yaml",
    ] {
        assert!(deploy().join(a).is_file(), "falta deploy/{a}");
    }
    let mut kinds = Vec::new();
    for a in K8S {
        for d in ler_yaml(&ler(&format!("k8s/{a}"))) {
            kinds.push(get(&d, "kind").unwrap_or("?").to_string());
        }
    }
    for k in [
        "Namespace",
        "ConfigMap",
        "Secret",
        "PersistentVolumeClaim",
        "Deployment",
        "Service",
        "NetworkPolicy",
    ] {
        assert!(
            kinds.iter().any(|x| x == k),
            "falta o recurso {k} em deploy/k8s"
        );
    }
}

#[test]
fn o_leitor_le_o_que_os_manifestos_usam() {
    let docs = ler_yaml(&ler("k8s/deployment.yaml"));
    assert_eq!(docs.len(), 1);
    let d = &docs[0];
    assert_eq!(get(d, "kind"), Some("Deployment"));
    let c = "spec.template.spec.containers[0]";
    assert_eq!(get(d, &format!("{c}.name")), Some("phxclaw"));
    assert_eq!(
        get(d, &format!("{c}.securityContext.readOnlyRootFilesystem")),
        Some("true")
    );
    assert_eq!(
        get(d, &format!("{c}.securityContext.capabilities.drop")),
        Some("[\"ALL\"]")
    );
    assert_eq!(
        get(d, &format!("{c}.readinessProbe.httpGet.path")),
        Some("/health")
    );
    assert_eq!(
        get(d, &format!("{c}.volumeMounts[2].subPath")),
        Some("api-token")
    );
    assert_eq!(
        get(d, "spec.template.spec.initContainers[0].name"),
        Some("prepara-dados")
    );
    assert_eq!(
        get(d, "spec.template.spec.securityContext.runAsNonRoot"),
        Some("true")
    );

    let comp = ler_yaml(&ler("docker-compose.yml"));
    let c = &comp[0];
    assert_eq!(get(c, "services.phxclaw.user"), Some("10001:10001"));
    assert_eq!(
        get(c, "services.phxclaw.ports[0]"),
        Some("127.0.0.1:8787:8787")
    );
    assert_eq!(
        get(c, "services.phxclaw.secrets[1].target"),
        Some("/data/segredos/master.key")
    );
    assert_eq!(
        get(c, "services.postgres.environment.POSTGRES_PASSWORD_FILE"),
        Some("/run/secrets/pg_senha")
    );
    assert_eq!(
        get(c, "secrets.master_key.file"),
        Some("./secrets/master_key")
    );

    let multi = ler_yaml("a: 1\n---\nb:\n- x\n- y: 2\n  z: 3\nc: 4\n");
    assert_eq!(multi.len(), 2);
    assert_eq!(get(&multi[1], "b[0]"), Some("x"));
    assert_eq!(get(&multi[1], "b[1].z"), Some("3"));
    assert_eq!(get(&multi[1], "c"), Some("4"));
}

// ---- o irmao: a conferencia nao pode recusar tudo ----

#[test]
fn mudanca_inofensiva_continua_conforme() {
    let cm = mutar(&ler("k8s/configmap.yaml"), "\"10\"", "\"20\"");
    assert!(conferir_k8s("configmap.yaml", &cm).is_empty());
    let df = format!(
        "{}\nLABEL versao=\"0.70.0\"\nENV RUST_LOG=info\n",
        ler("Dockerfile")
    );
    assert!(
        conferir_dockerfile(&df).is_empty(),
        "{:?}",
        conferir_dockerfile(&df)
    );
}

// ---- mutantes: Dockerfile ----

#[test]
fn mutantes_dockerfile() {
    let base = ler("Dockerfile");
    exige(
        &conferir_dockerfile(&mutar(&base, "USER 10001:10001", "")),
        "nao tem USER",
    );
    exige(
        &conferir_dockerfile(&mutar(&base, "USER 10001:10001", "USER root")),
        "como root",
    );
    exige(
        &conferir_dockerfile(&mutar(&base, "USER 10001:10001", "USER 0")),
        "como root",
    );
    let com_token = format!("{base}\nENV GITHUB_TOKEN={}\n", token_falso());
    let v = conferir_dockerfile(&com_token);
    exige(&v, "forma de credencial");
    exige(&v, "nome de segredo e valor literal");
    let arg = format!("{base}\nARG REGISTRY_PASSWORD=hunter2\n");
    exige(
        &conferir_dockerfile(&arg),
        "nome de segredo e valor literal",
    );
    let sem_hc = base
        .lines()
        .filter(|l| !l.contains("HEALTHCHECK") && !l.starts_with("  CMD curl"))
        .collect::<Vec<_>>()
        .join("\n");
    exige(&conferir_dockerfile(&sem_hc), "HEALTHCHECK");
    exige(
        &conferir_dockerfile(&mutar(
            &base,
            "COPY --from=build /src/target/release/phxclaw /usr/local/bin/phxclaw",
            "COPY . /app",
        )),
        "contexto inteiro",
    );
    exige(
        &conferir_dockerfile(&format!(
            "{base}\nCOPY master.key /data/segredos/master.key\n"
        )),
        "arquivo de segredo",
    );
    let um_estagio = base.replace("FROM debian:bookworm-slim AS final", "RUN true");
    exige(&conferir_dockerfile(&um_estagio), "multi-estagio");
}

#[test]
fn mutantes_dockerignore() {
    let base = ler("Dockerfile.dockerignore");
    exige(
        &conferir_dockerignore(&mutar(&base, "target/\n**/target/", "")),
        "target/",
    );
    // Duas grafias aceitas (a chave e o diretorio do cofre): o mutante tira as duas.
    let sem_cofre = mutar(&mutar(&base, "**/master.key", ""), "**/segredos/", "");
    exige(&conferir_dockerignore(&sem_cofre), "master.key");
    exige(
        &conferir_dockerignore(&mutar(&base, "\n.git/\n", "\n")),
        ".git/",
    );
    let sem_secrets = mutar(&mutar(&base, "**/secrets/\n", ""), "deploy/secrets/\n", "");
    exige(&conferir_dockerignore(&sem_secrets), "deploy/secrets");
}

const AMBOS: [&str; 2] = ["deploy/Dockerfile.dockerignore", "deploy/.dockerignore"];

#[test]
fn o_ignore_conferido_e_o_que_o_docker_le() {
    // o real: o arquivo ao lado do Dockerfile existe e a isca nao
    let (presentes, lido) = ignore_real();
    assert!(
        presentes
            .iter()
            .any(|p| p == "deploy/Dockerfile.dockerignore"),
        "{presentes:?}"
    );
    assert!(lido.is_some());
    assert_eq!(
        ignore_que_o_docker_le("deploy/Dockerfile", &["deploy/.dockerignore"]),
        None,
        "deploy/.dockerignore nao e lido pelo Docker"
    );
    // a raiz do contexto tambem vale, e o ao-lado vence quando os dois existem
    assert_eq!(
        ignore_que_o_docker_le("deploy/Dockerfile", &[".dockerignore"]).as_deref(),
        Some(".dockerignore")
    );
    assert_eq!(
        ignore_que_o_docker_le("deploy/Dockerfile", &[".dockerignore", AMBOS[0]]).as_deref(),
        Some(AMBOS[0])
    );
}

#[test]
fn mutantes_contexto_do_build() {
    let base = ler("Dockerfile");
    let bom = ["deploy/Dockerfile.dockerignore"];
    let ignore_bom = ler("Dockerfile.dockerignore");
    // o real passa, e o irmao: COPY largo COM ignore lido e completo tambem passa
    assert!(conferir_contexto(&base, &bom, Some(&ignore_bom)).is_empty());
    let largo = mutar(&base, "COPY crates/ crates/", "COPY . .");
    assert!(
        conferir_contexto(&largo, &bom, Some(&ignore_bom)).is_empty(),
        "{:?}",
        conferir_contexto(&largo, &bom, Some(&ignore_bom))
    );
    // RED 1: repor `COPY . .` sem ignore que o Docker leia -> cai
    exige(
        &conferir_contexto(&largo, &[], None),
        "COPY do contexto inteiro",
    );
    // RED 2: o ignore com o nome que o Docker NAO le (so a isca existe) -> cai, mesmo com o
    // conteudo perfeito dentro dela
    exige(
        &conferir_contexto(&base, &["deploy/.dockerignore"], None),
        "nenhum arquivo que o Docker leia",
    );
    exige(
        &conferir_contexto(&base, &["deploy/.dockerignore"], None),
        "NAO o le",
    );
    exige(
        &conferir_contexto(&largo, &["deploy/.dockerignore"], None),
        "sem ignore lido",
    );
    // RED 3: COPY largo com ignore lido, mas sem excluir o segredo -> cai
    let sem_secrets = mutar(
        &mutar(&ignore_bom, "**/secrets/\n", ""),
        "deploy/secrets/\n",
        "",
    );
    exige(
        &conferir_contexto(&largo, &bom, Some(&sem_secrets)),
        "deploy/secrets",
    );
    // RED 4: copiar a pasta de segredos pelo nome
    exige(
        &conferir_contexto(
            &format!("{base}\nCOPY deploy/secrets/ /s/\n"),
            &bom,
            Some(&ignore_bom),
        ),
        "leva segredo",
    );
}

// ---- mutantes: k8s ----

#[test]
fn mutantes_deployment() {
    let base = ler("k8s/deployment.yaml");
    let c = |t: &str| conferir_k8s("deployment.yaml", t);
    exige(
        &c(&mutar(&base, "runAsNonRoot: true", "runAsNonRoot: false")),
        "runAsNonRoot",
    );
    exige(
        &c(&mutar(&base, "runAsUser: 10001", "runAsUser: 0")),
        "runAsUser: 0",
    );
    exige(
        &c(&mutar(
            &base,
            "readOnlyRootFilesystem: true",
            "readOnlyRootFilesystem: false",
        )),
        "readOnlyRootFilesystem",
    );
    exige(
        &c(&mutar(
            &base,
            "allowPrivilegeEscalation: false",
            "allowPrivilegeEscalation: true",
        )),
        "allowPrivilegeEscalation",
    );
    exige(
        &c(&mutar(&base, "drop: [\"ALL\"]", "drop: []")),
        "capabilities.drop",
    );
    exige(
        &c(&mutar(&base, "livenessProbe:", "outraCoisa:")),
        "livenessProbe",
    );
    exige(
        &c(&mutar(&base, "readinessProbe:", "outraCoisa2:")),
        "readinessProbe",
    );
    exige(
        &c(&mutar(
            &base,
            "image: phxclaw:0.70.0",
            "image: phxclaw:latest",
        )),
        "sem versao fixa",
    );
    let com_env = mutar(
        &base,
        "          envFrom:",
        &format!(
            "          env:\n            - name: GITHUB_TOKEN\n              value: {}\n          envFrom:",
            token_falso()
        ),
    );
    let v = c(&com_env);
    exige(&v, "forma de credencial");
    exige(&v, "com `value` literal");
    let env_nome = mutar(
        &base,
        "          envFrom:",
        "          env:\n            - name: PHXCLAW_API_TOKEN\n              value: abc\n          envFrom:",
    );
    exige(&c(&env_nome), "com `value` literal");
}

#[test]
fn mutantes_demais_recursos() {
    let sec = ler("k8s/secret-exemplo.yaml");
    let c = |t: &str| conferir_k8s("secret-exemplo.yaml", t);
    exige(
        &c(&mutar(
            &sec,
            "api-token: \"\"",
            "api-token: \"valor-de-verdade-123\"",
        )),
        "Secret de exemplo tem valor",
    );
    exige(
        &c(&mutar(
            &sec,
            "master.key: \"\"",
            &format!("master.key: \"{}\"", token_falso()),
        )),
        "forma de credencial",
    );
    let svc = ler("k8s/service.yaml");
    exige(
        &conferir_k8s(
            "service.yaml",
            &mutar(&svc, "type: ClusterIP", "type: LoadBalancer"),
        ),
        "expoe a API",
    );
    let np = ler("k8s/networkpolicy.yaml");
    let n = |t: &str| conferir_k8s("networkpolicy.yaml", t);
    assert!(n(&np).is_empty(), "{:?}", n(&np));
    // ingresso aberto a todos: o `from` some, ou vira um bloco de CIDR escancarado
    exige(
        &n(&mutar(
            &np,
            "    - from:\n        - namespaceSelector:\n            matchLabels:\n              kubernetes.io/metadata.name: ingress-nginx\n      ports:",
            "    - ports:",
        )),
        "aberto a todos",
    );
    exige(
        &n(&mutar(
            &np,
            "namespaceSelector:\n            matchLabels:\n              kubernetes.io/metadata.name: ingress-nginx",
            "ipBlock:\n            cidr: 0.0.0.0/0",
        )),
        "0.0.0.0/0",
    );
    exige(&n(&mutar(&np, "port: 8787", "port: 9999")), "porta 8787");
    exige(&n(&mutar(&np, "    - Ingress\n", "")), "policyTypes");
    let cm = ler("k8s/configmap.yaml");
    exige(
        &conferir_k8s(
            "configmap.yaml",
            &format!("{cm}  PHXCLAW_API_TOKEN: \"abcdefghijklmnopqrstuvwxyz\"\n"),
        ),
        "nome de segredo e valor literal",
    );
    exige(
        &conferir_k8s(
            "configmap.yaml",
            &format!("{cm}  PHXCLAW_PG_URL: \"postgresql://u:senha@db:5432/x\"\n"),
        ),
        "forma de credencial",
    );
}

#[test]
fn mutantes_kustomization() {
    let base = ler("k8s/kustomization.yaml");
    let dir = deploy().join("k8s");
    exige(
        &conferir_kustomization(&dir, &mutar(&base, "  - pvc.yaml\n", "")),
        "pvc.yaml",
    );
    exige(
        &conferir_kustomization(&dir, &mutar(&base, "  - service.yaml", "  - fantasma.yaml")),
        "nao existe",
    );
    exige(
        &conferir_kustomization(&dir, &mutar(&base, "  - networkpolicy.yaml\n", "")),
        "networkpolicy.yaml",
    );
}

// ---- mutantes: compose ----

#[test]
fn mutantes_compose() {
    let base = ler("docker-compose.yml");
    let c = |t: &str| conferir_compose(t);
    exige(
        &c(&mutar(&base, "    user: \"10001:10001\"\n", "")),
        "sem `user`",
    );
    exige(
        &c(&mutar(&base, "user: \"10001:10001\"", "user: root")),
        "como root",
    );
    exige(
        &c(&mutar(&base, "read_only: true", "read_only: false")),
        "read_only",
    );
    exige(
        &c(&mutar(
            &base,
            "      PHXCLAW_HOME: /data\n",
            "      PHXCLAW_HOME: /data\n      PHXCLAW_API_TOKEN: abcdefghijklmnopqrstuvwxyz\n",
        )),
        "nome de segredo e valor literal",
    );
    exige(
        &c(&mutar(
            &base,
            "      POSTGRES_PASSWORD_FILE: /run/secrets/pg_senha\n",
            "      POSTGRES_PASSWORD: trocar\n",
        )),
        "POSTGRES_PASSWORD",
    );
    exige(
        &c(&mutar(&base, "\"127.0.0.1:8787:8787\"", "\"8787:8787\"")),
        "fora do loopback",
    );
    exige(
        &c(&mutar(&base, "profiles: [\"postgres\"]", "")),
        "nao e opcional",
    );
    exige(
        &c(&mutar(
            &base,
            "    file: ./secrets/api_token",
            "    content: segredo-no-compose",
        )),
        "content",
    );
    exige(
        &c(&mutar(
            &base,
            "      POSTGRES_USER: phxclaw\n",
            &format!(
                "      POSTGRES_USER: phxclaw\n      GH: {}\n",
                token_falso()
            ),
        )),
        "forma de credencial",
    );
}
