//! Tarefas agendadas: um objetivo que vira tarefa nova a cada disparo -- ou um FLUXO
//! (`fluxo`, caminho do JSON) que roda pelo `fluxos::rodar_com` a cada disparo, sem
//! modelo quando so tem passos de ferramenta. O `objetivo` com prefixo `fluxo: ARQ`
//! continua valendo (e o caminho antigo); o campo e o explicito.
//!
//! A proxima execucao sai do `next_fire` do rustclaw-native (mesmo motor de cron da base).
//! A agenda persiste em JSON; quem a roda chama `due(agora)` periodicamente.

use chrono::{DateTime, Utc};
pub use phxclaw_rustclaw_native::ScheduleSpec;
use phxclaw_rustclaw_native::{next_fire, validate_schedule};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub objective: String,
    /// Caminho do fluxo a rodar no disparo, em vez de criar tarefa com o objetivo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluxo: Option<String>,
    pub spec: ScheduleSpec,
    pub next_run: DateTime<Utc>,
    pub enabled: bool,
    #[serde(default)]
    pub last_task: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Agenda {
    path: PathBuf,
    pub items: Vec<Schedule>,
}

impl Agenda {
    pub fn open(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        let items = match std::fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b).map_err(std::io::Error::other)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(e),
        };
        Ok(Self { path, items })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn add(
        &mut self,
        name: &str,
        objective: &str,
        spec: ScheduleSpec,
        now: DateTime<Utc>,
    ) -> Result<Schedule, String> {
        self.add_com(name, objective, None, spec, now)
    }

    /// Agenda um FLUXO: o objetivo gravado e so o rotulo (`fluxo: ARQ`), e o disparo roda
    /// o arquivo. O fluxo e lido aqui, para o arquivo invalido parar na mao de quem
    /// agenda e nao no disparo das 3h.
    pub fn add_fluxo(
        &mut self,
        name: &str,
        caminho: &str,
        spec: ScheduleSpec,
        now: DateTime<Utc>,
    ) -> Result<Schedule, String> {
        let texto = std::fs::read_to_string(caminho).map_err(|e| format!("{caminho}: {e}"))?;
        let f = crate::fluxos::ler(&texto)?;
        self.add_com(
            name,
            &format!("{}{}", crate::api::PREFIXO_FLUXO, f.nome),
            Some(caminho.to_string()),
            spec,
            now,
        )
    }

    fn add_com(
        &mut self,
        name: &str,
        objective: &str,
        fluxo: Option<String>,
        spec: ScheduleSpec,
        now: DateTime<Utc>,
    ) -> Result<Schedule, String> {
        validate_schedule(&spec).map_err(|e| e.to_string())?;
        let next_run = next_fire(&spec, now).ok_or("a expressao nunca dispara")?;
        let s = Schedule {
            id: phxclaw_types::new_uuid_v7().to_string(),
            name: name.into(),
            objective: objective.into(),
            fluxo,
            spec,
            next_run,
            enabled: true,
            last_task: None,
        };
        self.items.push(s.clone());
        self.save().map_err(|e| e.to_string())?;
        Ok(s)
    }

    /// `add_fluxo` com o relogio de agora, para a CLI.
    pub fn adicionar_fluxo_agora(
        &mut self,
        name: &str,
        caminho: &str,
        spec: ScheduleSpec,
    ) -> Result<Schedule, String> {
        self.add_fluxo(name, caminho, spec, Utc::now())
    }

    /// `add` com o relogio de agora: a porta da CLI, que nao carrega o chrono.
    pub fn adicionar_agora(
        &mut self,
        name: &str,
        objective: &str,
        spec: ScheduleSpec,
    ) -> Result<Schedule, String> {
        self.add(name, objective, spec, Utc::now())
    }

    /// Agendamentos vencidos em `now`; cada um ja avanca para a proxima execucao. Disparo
    /// atrasado (processo parado) dispara UMA vez, nao uma por janela perdida.
    pub fn due(&mut self, now: DateTime<Utc>) -> Vec<Schedule> {
        let mut vencidos = vec![];
        for s in self
            .items
            .iter_mut()
            .filter(|s| s.enabled && s.next_run <= now)
        {
            vencidos.push(s.clone());
            match next_fire(&s.spec, now) {
                Some(n) => s.next_run = n,
                None => s.enabled = false,
            }
        }
        if !vencidos.is_empty() {
            let _ = self.save();
        }
        vencidos
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(p) = self.path.parent() {
            std::fs::create_dir_all(p)?;
        }
        // A troca atomica da base: o temporario de nome fixo que havia aqui fazia dois
        // processos (CLI e servidor) escreverem no mesmo `.tmp`.
        phxclaw_types::arquivo::gravar_atomico(&self.path, &serde_json::to_vec_pretty(&self.items)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispara_uma_vez_mesmo_atrasado_e_persiste() {
        let p = std::env::temp_dir().join(format!("phx-agenda-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let t0: DateTime<Utc> = "2026-09-30T08:00:00Z".parse().unwrap();
        let mut a = Agenda::open(&p).unwrap();
        a.add(
            "bom dia",
            "resuma as noticias",
            ScheduleSpec::CronExpression("0 9 * * *".into()),
            t0,
        )
        .unwrap();
        assert!(a.due(t0).is_empty());
        // processo parado 3 dias: dispara uma vez so, e a proxima e amanha 09:00
        let tarde: DateTime<Utc> = "2026-10-03T10:30:00Z".parse().unwrap();
        assert_eq!(a.due(tarde).len(), 1);
        assert!(a.due(tarde).is_empty());
        let relida = Agenda::open(&p).unwrap();
        assert_eq!(
            relida.items[0].next_run,
            "2026-10-04T09:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn recusa_frequencia_abaixo_de_um_minuto() {
        let p = std::env::temp_dir().join(format!("phx-agenda2-{}.json", std::process::id()));
        let mut a = Agenda::open(&p).unwrap();
        assert!(
            a.add("x", "y", ScheduleSpec::EverySeconds(5), Utc::now())
                .is_err()
        );
        let _ = std::fs::remove_file(p);
    }
}
