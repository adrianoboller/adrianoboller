use chrono::{DateTime, Duration, Utc};
use phxclaw_types::{PermissionClaim, is_uuid_v7, new_uuid_v7};
use postgres::{Client, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u16,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1_000,
            max_delay_ms: 60_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalGate {
    pub required: bool,
    pub role: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub uuid: Uuid,
    pub name: String,
    pub capability: String,
    pub payload: Value,
    pub dependencies: Vec<Uuid>,
    pub requested_permissions: Vec<PermissionClaim>,
    pub retry: RetryPolicy,
    pub approval: Option<ApprovalGate>,
    pub idempotency_key: String,
    pub priority: u16,
}

impl TaskSpec {
    pub fn new(
        name: impl Into<String>,
        capability: impl Into<String>,
        payload: Value,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            uuid: new_uuid_v7(),
            name: name.into(),
            capability: capability.into(),
            payload,
            dependencies: Vec::new(),
            requested_permissions: Vec::new(),
            retry: RetryPolicy::default(),
            approval: None,
            idempotency_key: idempotency_key.into(),
            priority: 100,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Blocked,
    Pending,
    WaitingApproval,
    Ready,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    DeadLetter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub task_uuid: Uuid,
    pub decision: ApprovalDecision,
    pub actor: String,
    pub note: Option<String>,
    pub decided_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRun {
    pub uuid: Uuid,
    pub task_uuid: Uuid,
    pub attempt: u16,
    pub status: TaskStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRuntimeState {
    pub task_uuid: Uuid,
    pub status: TaskStatus,
    pub attempts: u16,
    pub next_eligible_at: DateTime<Utc>,
    pub approval: Option<ApprovalRecord>,
    pub active_run_uuid: Option<Uuid>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEvent {
    pub uuid: Uuid,
    pub task_uuid: Uuid,
    pub run_uuid: Option<Uuid>,
    pub event_type: String,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum TaskGraphError {
    #[error("task UUID must be UUIDv7: {0}")]
    InvalidUuid(Uuid),
    #[error("duplicate task UUID: {0}")]
    DuplicateTask(Uuid),
    #[error("duplicate idempotency key: {0}")]
    DuplicateIdempotency(String),
    #[error("task {task} depends on unknown task {dependency}")]
    MissingDependency { task: Uuid, dependency: Uuid },
    #[error("task {0} cannot depend on itself")]
    SelfDependency(Uuid),
    #[error("task graph contains a cycle")]
    Cycle,
    #[error("invalid retry policy for task {0}")]
    InvalidRetry(Uuid),
    #[error("unknown task: {0}")]
    UnknownTask(Uuid),
    #[error("unknown run: {0}")]
    UnknownRun(Uuid),
    #[error("task {task} is not running under run {run}")]
    RunMismatch { task: Uuid, run: Uuid },
    #[error("task {0} is not awaiting approval")]
    NotAwaitingApproval(Uuid),
    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] postgres::Error),
}

#[derive(Debug, Clone)]
pub struct TaskGraph {
    tasks: BTreeMap<Uuid, TaskSpec>,
    topological_order: Vec<Uuid>,
}

impl TaskGraph {
    pub fn new(tasks: Vec<TaskSpec>) -> Result<Self, TaskGraphError> {
        let mut map = BTreeMap::new();
        let mut keys = BTreeSet::new();
        for task in tasks {
            if !is_uuid_v7(&task.uuid) {
                return Err(TaskGraphError::InvalidUuid(task.uuid));
            }
            if task.retry.max_attempts == 0
                || task.retry.base_delay_ms == 0
                || task.retry.max_delay_ms < task.retry.base_delay_ms
            {
                return Err(TaskGraphError::InvalidRetry(task.uuid));
            }
            if task.idempotency_key.trim().is_empty() || !keys.insert(task.idempotency_key.clone())
            {
                return Err(TaskGraphError::DuplicateIdempotency(task.idempotency_key));
            }
            let id = task.uuid;
            if map.insert(id, task).is_some() {
                return Err(TaskGraphError::DuplicateTask(id));
            }
        }

        for task in map.values() {
            for dependency in &task.dependencies {
                if *dependency == task.uuid {
                    return Err(TaskGraphError::SelfDependency(task.uuid));
                }
                if !map.contains_key(dependency) {
                    return Err(TaskGraphError::MissingDependency {
                        task: task.uuid,
                        dependency: *dependency,
                    });
                }
            }
        }

        let order = topological_sort(&map)?;
        Ok(Self {
            tasks: map,
            topological_order: order,
        })
    }

    pub fn tasks(&self) -> impl Iterator<Item = &TaskSpec> {
        self.tasks.values()
    }

    pub fn task(&self, uuid: &Uuid) -> Option<&TaskSpec> {
        self.tasks.get(uuid)
    }

    pub fn topological_order(&self) -> &[Uuid] {
        &self.topological_order
    }
}

fn topological_sort(tasks: &BTreeMap<Uuid, TaskSpec>) -> Result<Vec<Uuid>, TaskGraphError> {
    let mut indegree = tasks
        .iter()
        .map(|(uuid, task)| (*uuid, task.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for task in tasks.values() {
        for dependency in &task.dependencies {
            dependents.entry(*dependency).or_default().push(task.uuid);
        }
    }
    for values in dependents.values_mut() {
        values.sort();
    }

    let mut queue = indegree
        .iter()
        .filter_map(|(uuid, degree)| (*degree == 0).then_some(*uuid))
        .collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(tasks.len());

    while let Some(uuid) = queue.pop_front() {
        order.push(uuid);
        if let Some(items) = dependents.get(&uuid) {
            for dependent in items {
                let degree = indegree.get_mut(dependent).expect("known dependent");
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(*dependent);
                }
            }
        }
    }

    if order.len() != tasks.len() {
        return Err(TaskGraphError::Cycle);
    }
    Ok(order)
}

#[derive(Debug)]
pub struct TaskScheduler {
    graph: TaskGraph,
    states: BTreeMap<Uuid, TaskRuntimeState>,
    runs: BTreeMap<Uuid, TaskRun>,
    events: Vec<TaskEvent>,
}

impl TaskScheduler {
    pub fn new(graph: TaskGraph) -> Self {
        let now = Utc::now();
        let states = graph
            .tasks()
            .map(|task| {
                (
                    task.uuid,
                    TaskRuntimeState {
                        task_uuid: task.uuid,
                        status: TaskStatus::Pending,
                        attempts: 0,
                        // Tarefa que nunca rodou nao tem backoff: nasce elegivel. Usar o relogio
                        // interno aqui deixava Pending quem chamasse claim_ready com um `now`
                        // tirado antes do new() -- o unico ponto que nao recebe o tempo injetado.
                        next_eligible_at: DateTime::<Utc>::MIN_UTC,
                        approval: None,
                        active_run_uuid: None,
                        last_error: None,
                    },
                )
            })
            .collect();
        let mut scheduler = Self {
            graph,
            states,
            runs: BTreeMap::new(),
            events: Vec::new(),
        };
        scheduler.refresh(now);
        scheduler
    }

    pub fn graph(&self) -> &TaskGraph {
        &self.graph
    }

    pub fn state(&self, task_uuid: &Uuid) -> Option<&TaskRuntimeState> {
        self.states.get(task_uuid)
    }

    pub fn run(&self, run_uuid: &Uuid) -> Option<&TaskRun> {
        self.runs.get(run_uuid)
    }

    pub fn events(&self) -> &[TaskEvent] {
        &self.events
    }

    pub fn approve(
        &mut self,
        task_uuid: Uuid,
        actor: impl Into<String>,
        note: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<(), TaskGraphError> {
        let task = self
            .graph
            .task(&task_uuid)
            .ok_or(TaskGraphError::UnknownTask(task_uuid))?;
        if task.approval.as_ref().is_none_or(|gate| !gate.required) {
            return Err(TaskGraphError::NotAwaitingApproval(task_uuid));
        }
        let state = self
            .states
            .get_mut(&task_uuid)
            .ok_or(TaskGraphError::UnknownTask(task_uuid))?;
        state.approval = Some(ApprovalRecord {
            task_uuid,
            decision: ApprovalDecision::Approved,
            actor: actor.into(),
            note,
            decided_at: now,
        });
        self.push_event(task_uuid, None, "approval.approved", Value::Null, now);
        self.refresh(now);
        Ok(())
    }

    pub fn reject(
        &mut self,
        task_uuid: Uuid,
        actor: impl Into<String>,
        note: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<(), TaskGraphError> {
        let task = self
            .graph
            .task(&task_uuid)
            .ok_or(TaskGraphError::UnknownTask(task_uuid))?;
        if task.approval.as_ref().is_none_or(|gate| !gate.required) {
            return Err(TaskGraphError::NotAwaitingApproval(task_uuid));
        }
        let state = self
            .states
            .get_mut(&task_uuid)
            .ok_or(TaskGraphError::UnknownTask(task_uuid))?;
        state.approval = Some(ApprovalRecord {
            task_uuid,
            decision: ApprovalDecision::Rejected,
            actor: actor.into(),
            note,
            decided_at: now,
        });
        state.status = TaskStatus::Cancelled;
        self.push_event(task_uuid, None, "approval.rejected", Value::Null, now);
        self.refresh(now);
        Ok(())
    }

    pub fn claim_ready(&mut self, limit: usize, now: DateTime<Utc>) -> Vec<TaskRun> {
        self.refresh(now);
        let mut ready = self
            .states
            .values()
            .filter(|state| state.status == TaskStatus::Ready)
            .filter_map(|state| self.graph.task(&state.task_uuid))
            .collect::<Vec<_>>();
        ready.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.uuid.cmp(&right.uuid))
        });
        let selected = ready
            .into_iter()
            .take(limit)
            .map(|task| task.uuid)
            .collect::<Vec<_>>();
        let mut claimed = Vec::with_capacity(selected.len());

        for task_uuid in selected {
            let state = self
                .states
                .get_mut(&task_uuid)
                .expect("ready task has state");
            state.attempts += 1;
            state.status = TaskStatus::Running;
            let run = TaskRun {
                uuid: new_uuid_v7(),
                task_uuid,
                attempt: state.attempts,
                status: TaskStatus::Running,
                started_at: now,
                finished_at: None,
                result: None,
                error: None,
            };
            state.active_run_uuid = Some(run.uuid);
            self.runs.insert(run.uuid, run.clone());
            self.push_event(
                task_uuid,
                Some(run.uuid),
                "task.started",
                serde_json::json!({"attempt": run.attempt}),
                now,
            );
            claimed.push(run);
        }
        claimed
    }

    pub fn succeed(
        &mut self,
        run_uuid: Uuid,
        result: Value,
        now: DateTime<Utc>,
    ) -> Result<(), TaskGraphError> {
        let task_uuid = self
            .runs
            .get(&run_uuid)
            .ok_or(TaskGraphError::UnknownRun(run_uuid))?
            .task_uuid;
        self.ensure_active_run(task_uuid, run_uuid)?;
        if let Some(run) = self.runs.get_mut(&run_uuid) {
            run.status = TaskStatus::Succeeded;
            run.finished_at = Some(now);
            run.result = Some(result.clone());
        }
        let state = self.states.get_mut(&task_uuid).expect("task state exists");
        state.status = TaskStatus::Succeeded;
        state.active_run_uuid = None;
        state.last_error = None;
        self.push_event(task_uuid, Some(run_uuid), "task.succeeded", result, now);
        self.refresh(now);
        Ok(())
    }

    pub fn fail(
        &mut self,
        run_uuid: Uuid,
        error: impl Into<String>,
        retryable: bool,
        now: DateTime<Utc>,
    ) -> Result<(), TaskGraphError> {
        let error = error.into();
        let task_uuid = self
            .runs
            .get(&run_uuid)
            .ok_or(TaskGraphError::UnknownRun(run_uuid))?
            .task_uuid;
        self.ensure_active_run(task_uuid, run_uuid)?;
        if let Some(run) = self.runs.get_mut(&run_uuid) {
            run.status = TaskStatus::Failed;
            run.finished_at = Some(now);
            run.error = Some(error.clone());
        }

        let task = self.graph.task(&task_uuid).expect("run task exists");
        let state = self.states.get_mut(&task_uuid).expect("task state exists");
        state.active_run_uuid = None;
        state.last_error = Some(error.clone());
        let can_retry = retryable && state.attempts < task.retry.max_attempts;
        if can_retry {
            let shift = state.attempts.saturating_sub(1).min(31) as u32;
            let factor = 1_u64 << shift;
            let delay_ms = task
                .retry
                .base_delay_ms
                .saturating_mul(factor)
                .min(task.retry.max_delay_ms);
            state.next_eligible_at = now + Duration::milliseconds(delay_ms as i64);
            state.status = TaskStatus::Pending;
            self.push_event(
                task_uuid,
                Some(run_uuid),
                "task.retry_scheduled",
                serde_json::json!({"delay_ms": delay_ms, "error": error}),
                now,
            );
        } else {
            state.status = if retryable {
                TaskStatus::DeadLetter
            } else {
                TaskStatus::Failed
            };
            self.push_event(
                task_uuid,
                Some(run_uuid),
                "task.failed",
                serde_json::json!({"retryable": retryable, "error": error}),
                now,
            );
        }
        self.refresh(now);
        Ok(())
    }

    pub fn refresh(&mut self, now: DateTime<Utc>) {
        let task_ids = self.graph.topological_order().to_vec();
        for task_uuid in task_ids {
            let Some(task) = self.graph.task(&task_uuid) else {
                continue;
            };
            let current = self.states.get(&task_uuid).map(|state| state.status);
            if matches!(
                current,
                Some(
                    TaskStatus::Running
                        | TaskStatus::Succeeded
                        | TaskStatus::Failed
                        | TaskStatus::Cancelled
                        | TaskStatus::DeadLetter
                )
            ) {
                continue;
            }

            let dependencies_ready = task.dependencies.iter().all(|dependency| {
                self.states
                    .get(dependency)
                    .is_some_and(|state| state.status == TaskStatus::Succeeded)
            });
            let state = self.states.get_mut(&task_uuid).expect("task state exists");
            if !dependencies_ready {
                state.status = TaskStatus::Blocked;
                continue;
            }

            let requires_approval = task.approval.as_ref().is_some_and(|gate| gate.required);
            let approved = state
                .approval
                .as_ref()
                .is_some_and(|approval| approval.decision == ApprovalDecision::Approved);
            if requires_approval && !approved {
                state.status = TaskStatus::WaitingApproval;
            } else if state.next_eligible_at > now {
                state.status = TaskStatus::Pending;
            } else {
                state.status = TaskStatus::Ready;
            }
        }
    }

    fn ensure_active_run(&self, task_uuid: Uuid, run_uuid: Uuid) -> Result<(), TaskGraphError> {
        let state = self
            .states
            .get(&task_uuid)
            .ok_or(TaskGraphError::UnknownTask(task_uuid))?;
        if state.active_run_uuid != Some(run_uuid) || state.status != TaskStatus::Running {
            return Err(TaskGraphError::RunMismatch {
                task: task_uuid,
                run: run_uuid,
            });
        }
        Ok(())
    }

    fn push_event(
        &mut self,
        task_uuid: Uuid,
        run_uuid: Option<Uuid>,
        event_type: impl Into<String>,
        payload: Value,
        occurred_at: DateTime<Utc>,
    ) {
        self.events.push(TaskEvent {
            uuid: new_uuid_v7(),
            task_uuid,
            run_uuid,
            event_type: event_type.into(),
            payload,
            occurred_at,
        });
    }
}

pub struct PostgresTaskJournal;

impl PostgresTaskJournal {
    pub fn persist_graph(client: &mut Client, graph: &TaskGraph) -> Result<(), TaskGraphError> {
        let mut transaction = client.transaction()?;
        for task in graph.tasks() {
            transaction.execute(
                "INSERT INTO phoenix_tasks \
                 (uuid, name, capability, payload, dependencies, requested_permissions, retry_policy, approval_gate, idempotency_key, priority, status, created_at, updated_at) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'pending',now(),now()) \
                 ON CONFLICT (uuid) DO UPDATE SET \
                 name=EXCLUDED.name, capability=EXCLUDED.capability, payload=EXCLUDED.payload, \
                 dependencies=EXCLUDED.dependencies, requested_permissions=EXCLUDED.requested_permissions, \
                 retry_policy=EXCLUDED.retry_policy, approval_gate=EXCLUDED.approval_gate, \
                 priority=EXCLUDED.priority, updated_at=now()",
                &[
                    &task.uuid,
                    &task.name,
                    &task.capability,
                    &task.payload,
                    &serde_json::to_value(&task.dependencies).expect("serializable dependencies"),
                    &serde_json::to_value(&task.requested_permissions).expect("serializable permissions"),
                    &serde_json::to_value(&task.retry).expect("serializable retry"),
                    &serde_json::to_value(&task.approval).expect("serializable approval"),
                    &task.idempotency_key,
                    &(task.priority as i32),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn append_events(client: &mut Client, events: &[TaskEvent]) -> Result<(), TaskGraphError> {
        let mut transaction = client.transaction()?;
        for event in events {
            transaction.execute(
                "INSERT INTO phoenix_task_events \
                 (uuid, task_uuid, run_uuid, event_type, payload, occurred_at) \
                 VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (uuid) DO NOTHING",
                &[
                    &event.uuid,
                    &event.task_uuid,
                    &event.run_uuid,
                    &event.event_type,
                    &event.payload,
                    &event.occurred_at,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// As tres tabelas da migracao 0004 existem? A fila nao cria esquema: o DDL mora nas
    /// migracoes (e no FULL_INSTALL), e uma segunda copia dele aqui seria a que envelhece
    /// calada no dia em que a migracao mudar.
    pub fn queue_schema_ready(client: &mut Client) -> Result<bool, TaskGraphError> {
        let row = client.query_one(
            "SELECT to_regclass('phoenix_tasks') IS NOT NULL \
               AND to_regclass('phoenix_task_runs') IS NOT NULL \
               AND to_regclass('phoenix_task_events') IS NOT NULL",
            &[],
        )?;
        Ok(row.get(0))
    }

    /// Poe UMA execucao na fila, ja `ready`. A chave de idempotencia decide: a mesma chave
    /// de novo nao vira segunda execucao (`None`), que e o que deixa o laco que reenfileira
    /// a cada volta (a espera vencida que ninguem pegou ainda) ser chamado sem medo.
    pub fn enqueue(client: &mut Client, task: &TaskSpec) -> Result<Option<Uuid>, TaskGraphError> {
        let row = client.query_opt(
            "INSERT INTO phoenix_tasks \
             (uuid, name, capability, payload, dependencies, requested_permissions, retry_policy, approval_gate, idempotency_key, priority, status, next_eligible_at, created_at, updated_at) \
             VALUES ($1,$2,$3,$4,'[]'::jsonb,'[]'::jsonb,$5,NULL,$6,$7,'ready',now(),now(),now()) \
             ON CONFLICT (idempotency_key) DO NOTHING RETURNING uuid",
            &[
                &task.uuid,
                &task.name,
                &task.capability,
                &task.payload,
                &serde_json::to_value(&task.retry).expect("serializable retry"),
                &task.idempotency_key,
                &(task.priority as i32),
            ],
        )?;
        Ok(row.map(|r| r.get(0)))
    }

    /// Toma a proxima execucao: `client.transaction()` + [`Self::claim_in`] + commit.
    pub fn claim(
        client: &mut Client,
        worker: &str,
        capabilities: &[String],
        lease_ms: i64,
    ) -> Result<Option<QueueClaim>, TaskGraphError> {
        let mut tx = client.transaction()?;
        let c = Self::claim_in(&mut tx, worker, capabilities, lease_ms)?;
        tx.commit()?;
        Ok(c)
    }

    /// A tomada dentro de uma transacao de quem chama (o teste segura uma aberta para
    /// provar que a segunda tomada pula a linha travada).
    ///
    /// A posse e o `next_eligible_at` da linha `running`: o instante a partir do qual outro
    /// worker pode toma-la. O batimento empurra o instante para frente; worker morto para de
    /// empurrar, a posse vence e a linha volta a ser elegivel -- sem coluna nova, porque e
    /// exatamente o que a coluna ja dizia («quando esta linha pode ser tomada de novo»). O
    /// `active_run_uuid` e a cerca: batimento e fim so valem para o run que tomou.
    ///
    /// `FOR UPDATE SKIP LOCKED` e o que impede dois workers de levarem a mesma linha: a
    /// segunda tomada nao espera a primeira, pula a linha travada e leva a proxima.
    pub fn claim_in(
        tx: &mut Transaction<'_>,
        worker: &str,
        capabilities: &[String],
        lease_ms: i64,
    ) -> Result<Option<QueueClaim>, TaskGraphError> {
        let Some(row) = tx.query_opt(
            "SELECT uuid, name, capability, payload, idempotency_key, attempts, status, active_run_uuid, \
                    COALESCE((retry_policy->>'max_attempts')::int, 3) AS max_attempts \
               FROM phoenix_tasks \
              WHERE status IN ('ready','running') \
                AND next_eligible_at <= now() \
                AND capability = ANY($1) \
              ORDER BY priority DESC, next_eligible_at, uuid \
              FOR UPDATE SKIP LOCKED \
              LIMIT 1",
            &[&capabilities],
        )?
        else {
            return Ok(None);
        };
        let task_uuid: Uuid = row.get("uuid");
        let attempts: i32 = row.get("attempts");
        let max_attempts: i32 = row.get("max_attempts");
        let status: String = row.get("status");
        let previous: Option<Uuid> = row.get("active_run_uuid");
        let reclaimed = status == "running";
        if let Some(run) = previous.filter(|_| reclaimed) {
            tx.execute(
                "UPDATE phoenix_task_runs SET status='failed', finished_at=now(), \
                 error='posse vencida: o worker parou de bater' WHERE uuid=$1 AND status='running'",
                &[&run],
            )?;
        }
        let payload: Value = row.get("payload");
        let idempotency_key: String = row.get("idempotency_key");
        if attempts >= max_attempts.max(1) {
            let error = format!(
                "a execucao foi tomada {attempts} vez(es) e a posse venceu em todas (teto {max_attempts})"
            );
            tx.execute(
                "UPDATE phoenix_tasks SET status='dead_letter', last_error=$2, updated_at=now() WHERE uuid=$1",
                &[&task_uuid, &error],
            )?;
            insert_event(
                tx,
                task_uuid,
                previous,
                "dead_letter",
                serde_json::json!({"worker": worker, "attempts": attempts}),
            )?;
            return Ok(Some(QueueClaim::DeadLetter(DeadLetter {
                task_uuid,
                idempotency_key,
                payload,
                attempts,
                error,
            })));
        }
        let run_uuid = new_uuid_v7();
        let attempt = attempts + 1;
        tx.execute(
            "INSERT INTO phoenix_task_runs (uuid, task_uuid, attempt, status, started_at) \
             VALUES ($1,$2,$3,'running',now())",
            &[&run_uuid, &task_uuid, &attempt],
        )?;
        tx.execute(
            "UPDATE phoenix_tasks SET status='running', attempts=$2, active_run_uuid=$3, \
             next_eligible_at = now() + ($4::bigint * interval '1 millisecond'), updated_at=now() \
             WHERE uuid=$1",
            &[&task_uuid, &attempt, &run_uuid, &lease_ms],
        )?;
        insert_event(
            tx,
            task_uuid,
            Some(run_uuid),
            "claimed",
            serde_json::json!({"worker": worker, "attempt": attempt, "reclaimed": reclaimed}),
        )?;
        Ok(Some(QueueClaim::Run(QueueLease {
            task_uuid,
            run_uuid,
            attempt,
            name: row.get("name"),
            capability: row.get("capability"),
            payload,
            idempotency_key,
            reclaimed,
        })))
    }

    /// O batimento: empurra a posse para `agora + lease_ms`, so se ela ainda e deste run.
    /// `Lost` = outro worker tomou (este ficou parado mais que o prazo): quem bate tem de
    /// parar, porque o resultado dele nao vai mais valer.
    pub fn renew(
        client: &mut Client,
        lease: &QueueLease,
        lease_ms: i64,
    ) -> Result<Heartbeat, TaskGraphError> {
        let n = client.execute(
            "UPDATE phoenix_tasks SET next_eligible_at = now() + ($3::bigint * interval '1 millisecond'), \
             updated_at=now() WHERE uuid=$1 AND active_run_uuid=$2 AND status='running'",
            &[&lease.task_uuid, &lease.run_uuid, &lease_ms],
        )?;
        if n == 0 {
            return Ok(Heartbeat::Lost);
        }
        let cancel: bool = client
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM phoenix_task_events WHERE task_uuid=$1 AND event_type='cancel_requested')",
                &[&lease.task_uuid],
            )?
            .get(0);
        Ok(Heartbeat::Held {
            cancel_requested: cancel,
        })
    }

    /// Fecha o run com o resultado. `false` = a posse ja nao era deste run (venceu e outro
    /// tomou): o resultado e descartado, e nada na linha muda.
    pub fn finish(
        client: &mut Client,
        lease: &QueueLease,
        outcome: &RunOutcome,
    ) -> Result<bool, TaskGraphError> {
        let status = match outcome.status {
            TaskStatus::Succeeded => "succeeded",
            TaskStatus::Cancelled => "cancelled",
            _ => "failed",
        };
        let mut tx = client.transaction()?;
        let n = tx.execute(
            "UPDATE phoenix_tasks SET status=$3, last_error=$4, updated_at=now() \
             WHERE uuid=$1 AND active_run_uuid=$2 AND status='running'",
            &[&lease.task_uuid, &lease.run_uuid, &status, &outcome.error],
        )?;
        if n == 0 {
            return Ok(false);
        }
        tx.execute(
            "UPDATE phoenix_task_runs SET status=$2, result=$3, error=$4, finished_at=now() WHERE uuid=$1",
            &[&lease.run_uuid, &status, &outcome.result, &outcome.error],
        )?;
        insert_event(
            &mut tx,
            lease.task_uuid,
            Some(lease.run_uuid),
            "finished",
            serde_json::json!({"status": status}),
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// Cancelamento pela chave: a que nao comecou sai `cancelled` aqui; a que corre ganha
    /// um evento que o batimento do worker le.
    pub fn request_cancel(
        client: &mut Client,
        idempotency_key: &str,
    ) -> Result<CancelRequest, TaskGraphError> {
        if client
            .query_opt(
                "UPDATE phoenix_tasks SET status='cancelled', updated_at=now() \
                 WHERE idempotency_key=$1 AND status='ready' RETURNING uuid",
                &[&idempotency_key],
            )?
            .is_some()
        {
            return Ok(CancelRequest::CancelledBeforeStart);
        }
        let Some(row) = client.query_opt(
            "SELECT uuid, status, active_run_uuid FROM phoenix_tasks WHERE idempotency_key=$1",
            &[&idempotency_key],
        )?
        else {
            return Ok(CancelRequest::NotFound);
        };
        let status: String = row.get("status");
        if status != "running" {
            return Ok(CancelRequest::AlreadyFinished);
        }
        let mut tx = client.transaction()?;
        insert_event(
            &mut tx,
            row.get("uuid"),
            row.get("active_run_uuid"),
            "cancel_requested",
            Value::Object(Default::default()),
        )?;
        tx.commit()?;
        Ok(CancelRequest::Requested)
    }
}

fn insert_event(
    tx: &mut Transaction<'_>,
    task_uuid: Uuid,
    run_uuid: Option<Uuid>,
    event_type: &str,
    payload: Value,
) -> Result<(), TaskGraphError> {
    tx.execute(
        "INSERT INTO phoenix_task_events (uuid, task_uuid, run_uuid, event_type, payload, occurred_at) \
         VALUES ($1,$2,$3,$4,$5,now())",
        &[&new_uuid_v7(), &task_uuid, &run_uuid, &event_type, &payload],
    )?;
    Ok(())
}

/// Uma execucao tomada da fila.
#[derive(Debug, Clone)]
pub struct QueueLease {
    pub task_uuid: Uuid,
    /// A cerca: batimento e fim so valem com este run ainda ativo na linha.
    pub run_uuid: Uuid,
    pub attempt: i32,
    pub name: String,
    pub capability: String,
    pub payload: Value,
    pub idempotency_key: String,
    /// A posse anterior venceu sem batimento: esta tomada e uma retomada.
    pub reclaimed: bool,
}

/// A linha cuja posse venceu vezes demais: vai para `dead_letter`, e quem tomou avisa.
#[derive(Debug, Clone)]
pub struct DeadLetter {
    pub task_uuid: Uuid,
    pub idempotency_key: String,
    pub payload: Value,
    pub attempts: i32,
    pub error: String,
}

#[derive(Debug, Clone)]
pub enum QueueClaim {
    Run(QueueLease),
    DeadLetter(DeadLetter),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heartbeat {
    Held { cancel_requested: bool },
    Lost,
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// `Succeeded`, `Failed` ou `Cancelled`; outro estado conta como `Failed`.
    pub status: TaskStatus,
    pub result: Value,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelRequest {
    NotFound,
    CancelledBeforeStart,
    Requested,
    AlreadyFinished,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_cycles() {
        let mut a = TaskSpec::new("a", "a.run", Value::Null, "a");
        let mut b = TaskSpec::new("b", "b.run", Value::Null, "b");
        a.dependencies.push(b.uuid);
        b.dependencies.push(a.uuid);
        assert!(matches!(
            TaskGraph::new(vec![a, b]),
            Err(TaskGraphError::Cycle)
        ));
    }

    #[test]
    fn approval_and_dependencies_are_enforced() {
        let first = TaskSpec::new("research", "research.execute", Value::Null, "research");
        let mut second = TaskSpec::new("release", "release.publish", Value::Null, "release");
        second.dependencies.push(first.uuid);
        second.approval = Some(ApprovalGate {
            required: true,
            role: "product_owner".into(),
            reason: "publication gate".into(),
        });
        let first_id = first.uuid;
        let second_id = second.uuid;
        let graph = TaskGraph::new(vec![first, second]).unwrap();
        let now = Utc::now();
        let mut scheduler = TaskScheduler::new(graph);
        assert_eq!(
            scheduler.state(&first_id).unwrap().status,
            TaskStatus::Ready
        );
        assert_eq!(
            scheduler.state(&second_id).unwrap().status,
            TaskStatus::Blocked
        );
        let run = scheduler.claim_ready(1, now)[0].clone();
        scheduler
            .succeed(run.uuid, serde_json::json!({"ok": true}), now)
            .unwrap();
        assert_eq!(
            scheduler.state(&second_id).unwrap().status,
            TaskStatus::WaitingApproval
        );
        scheduler
            .approve(second_id, "product-owner", None, now)
            .unwrap();
        assert_eq!(
            scheduler.state(&second_id).unwrap().status,
            TaskStatus::Ready
        );
    }
}
