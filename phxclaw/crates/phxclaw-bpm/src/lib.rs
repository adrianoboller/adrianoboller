use chrono::{DateTime, Utc};
use phxclaw_types::new_uuid_v7;
use quick_xml::{Reader, events::Event};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindsetProfile {
    pub uuid: Uuid,
    pub name: String,
    pub principles: Vec<String>,
    pub priorities: Vec<String>,
    pub constraints: Vec<String>,
    pub evidence_required: bool,
    pub human_approval_for: Vec<String>,
}

impl MindsetProfile {
    pub fn phoenix_default() -> Self {
        Self {
            uuid: new_uuid_v7(),
            name: "phoenix-default".into(),
            principles: vec![
                "evidence_before_claim".into(),
                "deterministic_gates_before_llm".into(),
                "least_privilege".into(),
                "rollback_first".into(),
            ],
            priorities: vec![
                "correctness".into(),
                "security".into(),
                "traceability".into(),
                "speed".into(),
            ],
            constraints: vec![
                "no_silent_privilege_escalation".into(),
                "no_untracked_side_effects".into(),
            ],
            evidence_required: true,
            human_approval_for: vec!["release.publish".into(), "system.destructive".into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BpmNodeKind {
    Start,
    End,
    Task,
    UserTask,
    ServiceTask,
    ExclusiveGateway,
    ParallelGateway,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmNode {
    pub id: String,
    pub name: Option<String>,
    pub kind: BpmNodeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmFlow {
    pub id: String,
    pub source: String,
    pub target: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmProcess {
    pub uuid: Uuid,
    pub external_id: Option<String>,
    pub nodes: BTreeMap<String, BpmNode>,
    pub flows: Vec<BpmFlow>,
    pub imported_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum BpmError {
    #[error("BPMN XML error: {0}")]
    Xml(String),
    #[error("process contains no start node")]
    MissingStart,
    #[error("flow {flow} references unknown node {node}")]
    UnknownNode { flow: String, node: String },
    #[error("process graph contains a cycle where DAG execution was requested")]
    Cycle,
}

pub fn import_bpmn_xml(xml: &str) -> Result<BpmProcess, BpmError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut process = BpmProcess {
        uuid: new_uuid_v7(),
        external_id: None,
        nodes: BTreeMap::new(),
        flows: Vec::new(),
        imported_at: Utc::now(),
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                if local == b"process" {
                    process.external_id = attr(&e, b"id")?;
                } else if let Some(kind) = node_kind(local) {
                    if let Some(id) = attr(&e, b"id")? {
                        let name = attr(&e, b"name")?;
                        process.nodes.insert(id.clone(), BpmNode { id, name, kind });
                    }
                } else if local == b"sequenceFlow"
                    && let (Some(id), Some(source), Some(target)) = (
                        attr(&e, b"id")?,
                        attr(&e, b"sourceRef")?,
                        attr(&e, b"targetRef")?,
                    )
                {
                    process.flows.push(BpmFlow {
                        id,
                        source,
                        target,
                        name: attr(&e, b"name")?,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(BpmError::Xml(e.to_string())),
            _ => {}
        }
    }

    validate_process(&process)?;
    Ok(process)
}

fn attr(e: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Result<Option<String>, BpmError> {
    for a in e.attributes() {
        let a = a.map_err(|e| BpmError::Xml(e.to_string()))?;
        if local_name(a.key.as_ref()) == name {
            return Ok(Some(String::from_utf8_lossy(a.value.as_ref()).into_owned()));
        }
    }
    Ok(None)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|b| *b == b':').next().unwrap_or(name)
}

fn node_kind(name: &[u8]) -> Option<BpmNodeKind> {
    Some(match name {
        b"startEvent" => BpmNodeKind::Start,
        b"endEvent" => BpmNodeKind::End,
        b"task" => BpmNodeKind::Task,
        b"userTask" => BpmNodeKind::UserTask,
        b"serviceTask" => BpmNodeKind::ServiceTask,
        b"exclusiveGateway" => BpmNodeKind::ExclusiveGateway,
        b"parallelGateway" => BpmNodeKind::ParallelGateway,
        _ => return None,
    })
}

pub fn validate_process(process: &BpmProcess) -> Result<(), BpmError> {
    if !process.nodes.values().any(|n| n.kind == BpmNodeKind::Start) {
        return Err(BpmError::MissingStart);
    }
    for flow in &process.flows {
        if !process.nodes.contains_key(&flow.source) {
            return Err(BpmError::UnknownNode {
                flow: flow.id.clone(),
                node: flow.source.clone(),
            });
        }
        if !process.nodes.contains_key(&flow.target) {
            return Err(BpmError::UnknownNode {
                flow: flow.id.clone(),
                node: flow.target.clone(),
            });
        }
    }
    Ok(())
}

/// Deterministic topological order for BPM fragments that are expected to be DAGs.
/// Long-running BPMN with loops can use the token runtime instead of this helper.
pub fn dag_order(process: &BpmProcess) -> Result<Vec<String>, BpmError> {
    let mut indegree = process
        .nodes
        .keys()
        .map(|k| (k.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut edges = BTreeMap::<String, Vec<String>>::new();
    for flow in &process.flows {
        *indegree.entry(flow.target.clone()).or_default() += 1;
        edges
            .entry(flow.source.clone())
            .or_default()
            .push(flow.target.clone());
    }
    for values in edges.values_mut() {
        values.sort();
        values.dedup();
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(k, v)| (*v == 0).then_some(k.clone()))
        .collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(process.nodes.len());
    while let Some(node) = ready.pop_front() {
        order.push(node.clone());
        if let Some(next) = edges.get(&node) {
            for target in next {
                let d = indegree.get_mut(target).expect("known target");
                *d -= 1;
                if *d == 0 {
                    ready.push_back(target.clone());
                }
            }
        }
    }
    if order.len() != process.nodes.len() {
        Err(BpmError::Cycle)
    } else {
        Ok(order)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessToken {
    pub uuid: Uuid,
    pub process_uuid: Uuid,
    pub current_node: String,
    pub visited: BTreeSet<String>,
    pub updated_at: DateTime<Utc>,
}

impl ProcessToken {
    pub fn at_start(process: &BpmProcess) -> Result<Self, BpmError> {
        let start = process
            .nodes
            .values()
            .find(|n| n.kind == BpmNodeKind::Start)
            .ok_or(BpmError::MissingStart)?;
        Ok(Self {
            uuid: new_uuid_v7(),
            process_uuid: process.uuid,
            current_node: start.id.clone(),
            visited: BTreeSet::new(),
            updated_at: Utc::now(),
        })
    }

    pub fn outgoing<'a>(&self, process: &'a BpmProcess) -> Vec<&'a BpmFlow> {
        process
            .flows
            .iter()
            .filter(|f| f.source == self.current_node)
            .collect()
    }
}

// ---- v0.67 crash-safe persistent runtime -----------------------------------
use postgres::Client;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentTokenLease {
    pub token_uuid: Uuid,
    pub instance_uuid: Uuid,
    pub node_id: String,
    pub fencing_token: i64,
    pub lease_owner: String,
    pub lease_until: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplayState {
    pub instance_uuid: Uuid,
    pub last_sequence: i64,
    pub token_nodes: BTreeMap<Uuid, String>,
    pub completed_tokens: BTreeSet<Uuid>,
}

#[derive(Debug, Error)]
pub enum PersistentBpmError {
    #[error("postgres error: {0}")]
    Postgres(#[from] postgres::Error),
    #[error("no ready token available")]
    NoReadyToken,
    #[error("stale token lease / fencing token")]
    StaleLease,
    #[error("instance not found")]
    InstanceNotFound,
}

pub struct BpmStore<'a> {
    client: &'a mut Client,
}
impl<'a> BpmStore<'a> {
    pub fn new(client: &'a mut Client) -> Self {
        Self { client }
    }

    pub fn start_instance(
        &mut self,
        tenant_uuid: Uuid,
        process_uuid: Uuid,
        start_node: &str,
        variables: JsonValue,
    ) -> Result<(Uuid, Uuid), PersistentBpmError> {
        let instance_uuid = new_uuid_v7();
        let token_uuid = new_uuid_v7();
        let event_uuid = new_uuid_v7();
        let mut tx = self.client.transaction()?;
        tx.query_one(
            "SELECT set_config('phxclaw.tenant_uuid',$1,true)",
            &[&tenant_uuid.to_string()],
        )?;
        tx.execute("INSERT INTO phxclaw.bpm_instances_v2(instance_uuid,tenant_uuid,process_uuid,state,variables,started_at) VALUES($1,$2,$3,'running',$4,clock_timestamp())",&[&instance_uuid,&tenant_uuid,&process_uuid,&variables])?;
        tx.execute("INSERT INTO phxclaw.bpm_tokens_v2(token_uuid,tenant_uuid,instance_uuid,node_id,state,fencing_token,updated_at) VALUES($1,$2,$3,$4,'ready',0,clock_timestamp())",&[&token_uuid,&tenant_uuid,&instance_uuid,&start_node])?;
        tx.execute("INSERT INTO phxclaw.bpm_events_v2(event_uuid,tenant_uuid,instance_uuid,token_uuid,event_type,node_id,fencing_token,payload,occurred_at) VALUES($1,$2,$3,$4,'token_created',$5,0,'{}'::jsonb,clock_timestamp())",&[&event_uuid,&tenant_uuid,&instance_uuid,&token_uuid,&start_node])?;
        tx.commit()?;
        Ok((instance_uuid, token_uuid))
    }

    pub fn claim_ready_token(
        &mut self,
        tenant_uuid: Uuid,
        worker: &str,
        lease_seconds: i64,
    ) -> Result<PersistentTokenLease, PersistentBpmError> {
        let mut tx = self.client.transaction()?;
        tx.query_one(
            "SELECT set_config('phxclaw.tenant_uuid',$1,true)",
            &[&tenant_uuid.to_string()],
        )?;
        let row=tx.query_opt(r#"SELECT token_uuid,instance_uuid,node_id,fencing_token FROM phxclaw.bpm_tokens_v2
          WHERE tenant_uuid=$1 AND (state='ready' OR (state='leased' AND lease_until<=clock_timestamp()))
          ORDER BY updated_at,token_uuid FOR UPDATE SKIP LOCKED LIMIT 1"#,&[&tenant_uuid])?.ok_or(PersistentBpmError::NoReadyToken)?;
        let token_uuid: Uuid = row.get(0);
        let instance_uuid: Uuid = row.get(1);
        let node_id: String = row.get(2);
        let old: i64 = row.get(3);
        let next = old + 1;
        let until = Utc::now() + chrono::Duration::seconds(lease_seconds.max(1));
        tx.execute("UPDATE phxclaw.bpm_tokens_v2 SET state='leased',lease_owner=$2,lease_until=$3,fencing_token=$4,updated_at=clock_timestamp() WHERE token_uuid=$1",&[&token_uuid,&worker,&until,&next])?;
        tx.execute("INSERT INTO phxclaw.bpm_events_v2(event_uuid,tenant_uuid,instance_uuid,token_uuid,event_type,node_id,fencing_token,payload,occurred_at) VALUES($1,$2,$3,$4,'token_claimed',$5,$6,jsonb_build_object('worker',$7::text),clock_timestamp())",&[&new_uuid_v7(),&tenant_uuid,&instance_uuid,&token_uuid,&node_id,&next,&worker])?;
        tx.commit()?;
        Ok(PersistentTokenLease {
            token_uuid,
            instance_uuid,
            node_id,
            fencing_token: next,
            lease_owner: worker.into(),
            lease_until: until,
        })
    }

    pub fn advance_token(
        &mut self,
        tenant_uuid: Uuid,
        lease: &PersistentTokenLease,
        next_node: &str,
        payload: JsonValue,
    ) -> Result<(), PersistentBpmError> {
        let mut tx = self.client.transaction()?;
        tx.query_one(
            "SELECT set_config('phxclaw.tenant_uuid',$1,true)",
            &[&tenant_uuid.to_string()],
        )?;
        let changed=tx.execute(r#"UPDATE phxclaw.bpm_tokens_v2 SET node_id=$4,state='ready',lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp()
          WHERE token_uuid=$1 AND tenant_uuid=$2 AND instance_uuid=$3 AND fencing_token=$5 AND state='leased'"#,&[&lease.token_uuid,&tenant_uuid,&lease.instance_uuid,&next_node,&lease.fencing_token])?;
        if changed != 1 {
            return Err(PersistentBpmError::StaleLease);
        }
        tx.execute("INSERT INTO phxclaw.bpm_events_v2(event_uuid,tenant_uuid,instance_uuid,token_uuid,event_type,node_id,fencing_token,payload,occurred_at) VALUES($1,$2,$3,$4,'token_advanced',$5,$6,$7,clock_timestamp())",&[&new_uuid_v7(),&tenant_uuid,&lease.instance_uuid,&lease.token_uuid,&next_node,&lease.fencing_token,&payload])?;
        tx.commit()?;
        Ok(())
    }

    pub fn complete_token(
        &mut self,
        tenant_uuid: Uuid,
        lease: &PersistentTokenLease,
        payload: JsonValue,
    ) -> Result<(), PersistentBpmError> {
        let mut tx = self.client.transaction()?;
        tx.query_one(
            "SELECT set_config('phxclaw.tenant_uuid',$1,true)",
            &[&tenant_uuid.to_string()],
        )?;
        let changed=tx.execute("UPDATE phxclaw.bpm_tokens_v2 SET state='completed',lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE token_uuid=$1 AND tenant_uuid=$2 AND fencing_token=$3 AND state='leased'",&[&lease.token_uuid,&tenant_uuid,&lease.fencing_token])?;
        if changed != 1 {
            return Err(PersistentBpmError::StaleLease);
        }
        tx.execute("INSERT INTO phxclaw.bpm_events_v2(event_uuid,tenant_uuid,instance_uuid,token_uuid,event_type,node_id,fencing_token,payload,occurred_at) VALUES($1,$2,$3,$4,'token_completed',$5,$6,$7,clock_timestamp())",&[&new_uuid_v7(),&tenant_uuid,&lease.instance_uuid,&lease.token_uuid,&lease.node_id,&lease.fencing_token,&payload])?;
        let remaining:i64=tx.query_one("SELECT count(*) FROM phxclaw.bpm_tokens_v2 WHERE instance_uuid=$1 AND state<>'completed'",&[&lease.instance_uuid])?.get(0);
        if remaining == 0 {
            tx.execute("UPDATE phxclaw.bpm_instances_v2 SET state='completed',completed_at=clock_timestamp() WHERE instance_uuid=$1",&[&lease.instance_uuid])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn checkpoint(
        &mut self,
        tenant_uuid: Uuid,
        instance_uuid: Uuid,
    ) -> Result<Uuid, PersistentBpmError> {
        let state = self.replay(tenant_uuid, instance_uuid)?;
        let checkpoint_uuid = new_uuid_v7();
        let payload = serde_json::to_value(&state).unwrap_or(JsonValue::Null);
        self.client.execute("INSERT INTO phxclaw.bpm_checkpoints(checkpoint_uuid,tenant_uuid,instance_uuid,through_sequence,state_snapshot,created_at) VALUES($1,$2,$3,$4,$5,clock_timestamp())",&[&checkpoint_uuid,&tenant_uuid,&instance_uuid,&state.last_sequence,&payload])?;
        Ok(checkpoint_uuid)
    }

    pub fn replay(
        &mut self,
        tenant_uuid: Uuid,
        instance_uuid: Uuid,
    ) -> Result<ReplayState, PersistentBpmError> {
        self.client.query_one(
            "SELECT set_config('phxclaw.tenant_uuid',$1,false)",
            &[&tenant_uuid.to_string()],
        )?;
        if self
            .client
            .query_opt(
                "SELECT 1 FROM phxclaw.bpm_instances_v2 WHERE instance_uuid=$1 AND tenant_uuid=$2",
                &[&instance_uuid, &tenant_uuid],
            )?
            .is_none()
        {
            return Err(PersistentBpmError::InstanceNotFound);
        }
        let mut state = ReplayState {
            instance_uuid,
            ..Default::default()
        };
        for row in self.client.query("SELECT sequence,token_uuid,event_type,node_id FROM phxclaw.bpm_events_v2 WHERE instance_uuid=$1 ORDER BY sequence",&[&instance_uuid])?{
            let seq:i64=row.get(0);let token:Option<Uuid>=row.get(1);let kind:String=row.get(2);let node:Option<String>=row.get(3);state.last_sequence=seq;
            if let Some(t)=token{match kind.as_str(){"token_created"|"token_advanced"=>{if let Some(n)=node.clone(){state.token_nodes.insert(t,n);}state.completed_tokens.remove(&t);},"token_completed"=>{state.token_nodes.remove(&t);state.completed_tokens.insert(t);},_=>{}}}
        }
        Ok(state)
    }
}
