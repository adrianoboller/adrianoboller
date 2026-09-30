-- PhxClaw v0.58 — hierarchical project control / trace index
CREATE OR REPLACE FUNCTION phx_v058_deny_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'PhxClaw v0.58 append-only relation'; END $$;

CREATE TABLE phx_project_trace_nodes (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  parent_uuid uuid,
  root_uuid uuid NOT NULL,
  depth integer NOT NULL CHECK (depth BETWEEN 0 AND 64),
  path_ids uuid[] NOT NULL,
  node_kind text NOT NULL,
  logical_key text NOT NULL,
  title text NOT NULL,
  object_type text,
  object_uuid uuid,
  source_state_sha256 char(64) NOT NULL,
  evidence_sha256 char(64),
  tags text[] NOT NULL DEFAULT '{}',
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  search_document tsvector GENERATED ALWAYS AS (
    to_tsvector('simple', coalesce(title,'') || ' ' || coalesce(logical_key,'') || ' ' || coalesce(array_to_string(tags,' '),''))
  ) STORED,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, node_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, parent_uuid)
    REFERENCES phx_project_trace_nodes(tenant_uuid, project_uuid, node_uuid)
);
CREATE INDEX phx_project_trace_nodes_parent_idx ON phx_project_trace_nodes(tenant_uuid,project_uuid,parent_uuid,created_at);
CREATE INDEX phx_project_trace_nodes_path_gin ON phx_project_trace_nodes USING gin(path_ids);
CREATE INDEX phx_project_trace_nodes_search_gin ON phx_project_trace_nodes USING gin(search_document);
CREATE INDEX phx_project_trace_nodes_meta_gin ON phx_project_trace_nodes USING gin(metadata jsonb_path_ops);
CREATE INDEX phx_project_trace_nodes_object_idx ON phx_project_trace_nodes(tenant_uuid,project_uuid,object_type,object_uuid);
CREATE INDEX phx_project_trace_nodes_state_idx ON phx_project_trace_nodes(tenant_uuid,project_uuid,source_state_sha256,node_kind);

CREATE OR REPLACE FUNCTION phx_v058_trace_node_prepare() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p phx_project_trace_nodes%ROWTYPE;
BEGIN
  IF NEW.parent_uuid IS NULL THEN
    NEW.root_uuid := NEW.node_uuid; NEW.depth := 0; NEW.path_ids := ARRAY[NEW.node_uuid];
  ELSE
    SELECT * INTO STRICT p FROM phx_project_trace_nodes
      WHERE tenant_uuid=NEW.tenant_uuid AND project_uuid=NEW.project_uuid AND node_uuid=NEW.parent_uuid;
    IF NEW.node_uuid = ANY(p.path_ids) THEN RAISE EXCEPTION 'trace cycle detected'; END IF;
    NEW.root_uuid := p.root_uuid; NEW.depth := p.depth + 1; NEW.path_ids := p.path_ids || NEW.node_uuid;
  END IF;
  IF NEW.depth > 64 THEN RAISE EXCEPTION 'trace depth > 64'; END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER phx_project_trace_nodes_prepare BEFORE INSERT ON phx_project_trace_nodes FOR EACH ROW EXECUTE FUNCTION phx_v058_trace_node_prepare();
CREATE TRIGGER phx_project_trace_nodes_immutable BEFORE UPDATE OR DELETE ON phx_project_trace_nodes FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

CREATE TABLE phx_project_trace_links (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, link_uuid uuid NOT NULL,
  from_node_uuid uuid NOT NULL, to_node_uuid uuid NOT NULL,
  link_kind text NOT NULL CHECK (link_kind IN ('supports','refutes','derived_from','caused_by','supersedes','tests','implements','uses','produced_by','decided_by','depends_on','related_to')),
  evidence_sha256 char(64), source_state_sha256 char(64) NOT NULL,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid,project_uuid,link_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,from_node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,to_node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid)
);
CREATE INDEX phx_project_trace_links_from_idx ON phx_project_trace_links(tenant_uuid,project_uuid,from_node_uuid,link_kind);
CREATE INDEX phx_project_trace_links_to_idx ON phx_project_trace_links(tenant_uuid,project_uuid,to_node_uuid,link_kind);
CREATE TRIGGER phx_project_trace_links_immutable BEFORE UPDATE OR DELETE ON phx_project_trace_links FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

CREATE TABLE phx_project_source_documents (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, source_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL, source_kind text NOT NULL,
  uri text NOT NULL, content_sha256 char(64) NOT NULL,
  repository text, commit_sha text, line_start integer, line_end integer,
  captured_by_agent_uuid uuid, source_state_sha256 char(64) NOT NULL,
  provenance jsonb NOT NULL DEFAULT '{}'::jsonb,
  search_document tsvector GENERATED ALWAYS AS (to_tsvector('simple', coalesce(uri,'') || ' ' || coalesce(repository,'') || ' ' || coalesce(commit_sha,''))) STORED,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid,project_uuid,source_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid),
  CHECK (line_start IS NULL OR line_start > 0), CHECK (line_end IS NULL OR line_end >= line_start)
);
CREATE INDEX phx_project_source_documents_hash_idx ON phx_project_source_documents(tenant_uuid,project_uuid,content_sha256);
CREATE INDEX phx_project_source_documents_search_gin ON phx_project_source_documents USING gin(search_document);
CREATE INDEX phx_project_source_documents_commit_idx ON phx_project_source_documents(tenant_uuid,project_uuid,commit_sha) WHERE commit_sha IS NOT NULL;
CREATE TRIGGER phx_project_source_documents_immutable BEFORE UPDATE OR DELETE ON phx_project_source_documents FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

CREATE TABLE phx_project_trace_events (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL, event_type text NOT NULL, correlation_uuid uuid, causation_uuid uuid,
  source_state_sha256 char(64) NOT NULL, evidence_sha256 char(64) NOT NULL,
  payload jsonb NOT NULL DEFAULT '{}'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid,project_uuid,event_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid)
);
CREATE INDEX phx_project_trace_events_node_idx ON phx_project_trace_events(tenant_uuid,project_uuid,node_uuid,created_at);
CREATE INDEX phx_project_trace_events_corr_idx ON phx_project_trace_events(tenant_uuid,project_uuid,correlation_uuid) WHERE correlation_uuid IS NOT NULL;
CREATE TRIGGER phx_project_trace_events_immutable BEFORE UPDATE OR DELETE ON phx_project_trace_events FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

-- Generic fast search/index binding for existing domain objects (v0.43/v0.44/v0.45/v0.55/v0.57 etc.).
CREATE TABLE phx_project_trace_bindings (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, binding_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL, object_table text NOT NULL, object_uuid uuid NOT NULL,
  object_sha256 char(64), source_state_sha256 char(64) NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid,project_uuid,binding_uuid),
  UNIQUE (tenant_uuid,project_uuid,object_table,object_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid)
);
CREATE INDEX phx_project_trace_bindings_obj_idx ON phx_project_trace_bindings(tenant_uuid,project_uuid,object_uuid,object_table);
CREATE TRIGGER phx_project_trace_bindings_immutable BEFORE UPDATE OR DELETE ON phx_project_trace_bindings FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

-- RLS
ALTER TABLE phx_project_trace_nodes ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_trace_nodes FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_project_trace_links ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_trace_links FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_project_source_documents ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_source_documents FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_project_trace_events ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_trace_events FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_project_trace_bindings ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_trace_bindings FORCE ROW LEVEL SECURITY;
CREATE POLICY phx_project_trace_nodes_tenant ON phx_project_trace_nodes USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_project_trace_links_tenant ON phx_project_trace_links USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_project_source_documents_tenant ON phx_project_source_documents USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_project_trace_events_tenant ON phx_project_trace_events USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_project_trace_bindings_tenant ON phx_project_trace_bindings USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);

-- Hierarchy queries
CREATE OR REPLACE FUNCTION phx_project_trace_descendants(p_node uuid, p_max_depth integer DEFAULT 64)
RETURNS SETOF phx_project_trace_nodes LANGUAGE sql STABLE AS $$
  WITH anchor AS (
    SELECT tenant_uuid,project_uuid,depth FROM phx_project_trace_nodes
    WHERE tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid AND node_uuid=p_node
  )
  SELECT n FROM phx_project_trace_nodes n JOIN anchor a
    ON a.tenant_uuid=n.tenant_uuid AND a.project_uuid=n.project_uuid
  WHERE n.path_ids @> ARRAY[p_node]::uuid[] AND n.depth <= a.depth + LEAST(GREATEST(p_max_depth,0),64)
  ORDER BY n.depth,n.created_at;
$$;
CREATE OR REPLACE FUNCTION phx_project_trace_ancestors(p_node uuid)
RETURNS SETOF phx_project_trace_nodes LANGUAGE sql STABLE AS $$
  SELECT a FROM phx_project_trace_nodes n
  JOIN phx_project_trace_nodes a ON a.tenant_uuid=n.tenant_uuid AND a.project_uuid=n.project_uuid AND a.node_uuid = ANY(n.path_ids)
  WHERE n.tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid AND n.node_uuid=p_node
  ORDER BY a.depth;
$$;

CREATE OR REPLACE VIEW phx_project_decision_trace_v AS
SELECT n.tenant_uuid,n.project_uuid,n.node_uuid AS decision_node_uuid,n.title,n.source_state_sha256,
       count(DISTINCT s.source_uuid) AS source_count,
       count(DISTINCT l.link_uuid) FILTER (WHERE l.link_kind='supports') AS supports_count,
       count(DISTINCT l.link_uuid) FILTER (WHERE l.link_kind='refutes') AS refutes_count
FROM phx_project_trace_nodes n
LEFT JOIN phx_project_source_documents s ON s.tenant_uuid=n.tenant_uuid AND s.project_uuid=n.project_uuid AND s.node_uuid=n.node_uuid
LEFT JOIN phx_project_trace_links l ON l.tenant_uuid=n.tenant_uuid AND l.project_uuid=n.project_uuid AND (l.from_node_uuid=n.node_uuid OR l.to_node_uuid=n.node_uuid)
WHERE n.node_kind='decision'
GROUP BY n.tenant_uuid,n.project_uuid,n.node_uuid,n.title,n.source_state_sha256;


CREATE OR REPLACE FUNCTION phx_project_trace_search(p_query text, p_limit integer DEFAULT 50)
RETURNS TABLE(node_uuid uuid,node_kind text,title text,rank real,source_state_sha256 char(64))
LANGUAGE sql STABLE AS $$
  SELECT n.node_uuid,n.node_kind,n.title,
         ts_rank_cd(n.search_document, websearch_to_tsquery('simple', p_query)) AS rank,
         n.source_state_sha256
  FROM phx_project_trace_nodes n
  WHERE n.tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid
    AND n.search_document @@ websearch_to_tsquery('simple', p_query)
  ORDER BY rank DESC,n.created_at DESC
  LIMIT LEAST(GREATEST(p_limit,1),200);
$$;
