-- Diraigent schema (flattened from migrations 001-036)

CREATE SCHEMA diraigent;

-- Functions

CREATE FUNCTION diraigent.assign_task_number() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    -- Acquire a transaction-scoped advisory lock keyed on (namespace=1, project hash).
    -- This serializes concurrent inserts for the same project while allowing
    -- inserts into different projects to proceed in parallel.
    PERFORM pg_advisory_xact_lock(1, hashtext(NEW.project_id::text));

    SELECT COALESCE(MAX(number), 0) + 1
    INTO NEW.number
    FROM diraigent.task
    WHERE project_id = NEW.project_id;

    RETURN NEW;
END;
$$;

CREATE FUNCTION diraigent.update_timestamp() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

-- Tables (dependency order: independent first, then dependent)

CREATE TABLE diraigent.tenant (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL,
    slug text NOT NULL UNIQUE,
    encryption_mode text DEFAULT 'none'::text NOT NULL,
    key_salt text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT tenant_encryption_mode_check CHECK ((encryption_mode = ANY (ARRAY['none'::text, 'login_derived'::text, 'passphrase'::text])))
);

CREATE TABLE diraigent.auth_user (
    user_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    auth_user_id text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE diraigent.package (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug text NOT NULL UNIQUE,
    name text NOT NULL,
    description text,
    is_builtin boolean DEFAULT false NOT NULL,
    allowed_task_kinds text[] DEFAULT '{}'::text[] NOT NULL,
    allowed_knowledge_categories text[] DEFAULT '{}'::text[] NOT NULL,
    allowed_observation_kinds text[] DEFAULT '{}'::text[] NOT NULL,
    allowed_event_kinds text[] DEFAULT '{}'::text[] NOT NULL,
    allowed_integration_kinds text[] DEFAULT '{}'::text[] NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT package_slug_format CHECK ((slug ~ '^[a-z0-9][a-z0-9-]*[a-z0-9]$|^[a-z0-9]$'::text))
);

CREATE TABLE diraigent.agent (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL UNIQUE,
    capabilities text[] DEFAULT '{}'::text[] NOT NULL,
    status text DEFAULT 'idle'::text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    last_seen_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    owner_id uuid,
    CONSTRAINT agent_status_check CHECK ((status = ANY (ARRAY['idle'::text, 'working'::text, 'offline'::text, 'revoked'::text])))
);

CREATE TABLE diraigent.playbook (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    title text NOT NULL,
    trigger_description text,
    steps jsonb DEFAULT '[]'::jsonb NOT NULL,
    tags text[] DEFAULT '{}'::text[] NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    initial_state text DEFAULT 'ready'::text NOT NULL,
    tenant_id uuid,
    CONSTRAINT playbook_initial_state_check CHECK ((initial_state = ANY (ARRAY['ready'::text, 'backlog'::text])))
);

CREATE TABLE diraigent.role (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL,
    description text,
    authorities text[] DEFAULT '{}'::text[] NOT NULL,
    required_capabilities text[] DEFAULT '{}'::text[] NOT NULL,
    knowledge_scope text[] DEFAULT '{}'::text[] NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT role_tenant_name_key UNIQUE (tenant_id, name)
);

CREATE TABLE diraigent.project (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL,
    slug text NOT NULL UNIQUE,
    description text,
    owner_id uuid NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    parent_id uuid,
    default_playbook_id uuid,
    repo_url text,
    repo_path text,
    default_branch text DEFAULT 'main'::text NOT NULL,
    service_name text,
    package_id uuid DEFAULT 'afa3d121-870b-4e16-84f5-77224d4b5bd4'::uuid NOT NULL,
    git_mode text DEFAULT 'standalone'::text NOT NULL,
    git_root text,
    project_root text,
    tenant_id uuid NOT NULL
);

CREATE TABLE diraigent.decision (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    title text NOT NULL,
    status text DEFAULT 'proposed'::text NOT NULL,
    context text NOT NULL,
    decision text,
    rationale text,
    alternatives jsonb DEFAULT '[]'::jsonb NOT NULL,
    consequences text,
    superseded_by uuid,
    tags text[] DEFAULT '{}'::text[] NOT NULL,
    decided_by uuid,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT decision_status_check CHECK ((status = ANY (ARRAY['proposed'::text, 'accepted'::text, 'rejected'::text, 'superseded'::text, 'deprecated'::text])))
);

CREATE TABLE diraigent.goal (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    title text NOT NULL,
    description text,
    status text DEFAULT 'active'::text NOT NULL,
    target_date timestamp with time zone,
    success_criteria jsonb DEFAULT '[]'::jsonb NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT goal_status_check CHECK ((status = ANY (ARRAY['active'::text, 'achieved'::text, 'abandoned'::text, 'paused'::text])))
);

CREATE TABLE diraigent.integration (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    name text NOT NULL,
    kind text NOT NULL,
    provider text NOT NULL,
    base_url text,
    auth_type text DEFAULT 'none'::text NOT NULL,
    credentials jsonb DEFAULT '{}'::jsonb NOT NULL,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    capabilities text[] DEFAULT '{}'::text[] NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT integration_auth_type_check CHECK ((auth_type = ANY (ARRAY['none'::text, 'token'::text, 'basic'::text, 'header'::text, 'oauth'::text]))),
    CONSTRAINT integration_project_id_provider_name_key UNIQUE (project_id, provider, name)
);

CREATE TABLE diraigent.task (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    title text NOT NULL,
    kind text DEFAULT 'feature'::text NOT NULL,
    state text DEFAULT 'backlog'::text NOT NULL,
    priority integer DEFAULT 0 NOT NULL,
    context jsonb DEFAULT '{}'::jsonb NOT NULL,
    assigned_agent_id uuid,
    claimed_at timestamp with time zone,
    required_capabilities text[] DEFAULT '{}'::text[] NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    completed_at timestamp with time zone,
    assigned_role_id uuid,
    delegated_by uuid,
    delegated_at timestamp with time zone,
    playbook_id uuid,
    playbook_step integer DEFAULT 0,
    number bigint DEFAULT 0 NOT NULL,
    decision_id uuid,
    input_tokens bigint DEFAULT 0 NOT NULL,
    output_tokens bigint DEFAULT 0 NOT NULL,
    cost_usd double precision DEFAULT 0.0 NOT NULL,
    CONSTRAINT task_state_not_empty CHECK ((length(state) > 0)),
    CONSTRAINT task_project_number_unique UNIQUE (project_id, number)
);

CREATE TABLE diraigent.knowledge (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    title text NOT NULL,
    category text DEFAULT 'general'::text NOT NULL,
    content text NOT NULL,
    tags text[] DEFAULT '{}'::text[] NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    embedding double precision[]
);

CREATE TABLE diraigent.observation (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    agent_id uuid,
    kind text DEFAULT 'insight'::text NOT NULL,
    title text NOT NULL,
    description text,
    severity text DEFAULT 'low'::text NOT NULL,
    status text DEFAULT 'open'::text NOT NULL,
    evidence jsonb DEFAULT '{}'::jsonb NOT NULL,
    resolved_task_id uuid,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT observation_severity_check CHECK ((severity = ANY (ARRAY['info'::text, 'low'::text, 'medium'::text, 'high'::text, 'critical'::text]))),
    CONSTRAINT observation_status_check CHECK ((status = ANY (ARRAY['open'::text, 'acknowledged'::text, 'acted_on'::text, 'dismissed'::text])))
);

CREATE TABLE diraigent.event (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    kind text NOT NULL,
    source text NOT NULL,
    title text NOT NULL,
    description text,
    severity text DEFAULT 'info'::text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    related_task_id uuid,
    agent_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT event_severity_check CHECK ((severity = ANY (ARRAY['info'::text, 'warning'::text, 'error'::text, 'critical'::text])))
);

CREATE TABLE diraigent.membership (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id uuid NOT NULL,
    role_id uuid NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    joined_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT membership_status_check CHECK ((status = ANY (ARRAY['active'::text, 'inactive'::text, 'suspended'::text]))),
    CONSTRAINT membership_agent_id_role_id_key UNIQUE (agent_id, role_id)
);

CREATE TABLE diraigent.agent_integration (
    agent_id uuid NOT NULL,
    integration_id uuid NOT NULL,
    permissions text[] DEFAULT '{}'::text[] NOT NULL,
    granted_at timestamp with time zone DEFAULT now() NOT NULL,
    granted_by uuid,
    role_id uuid,
    PRIMARY KEY (agent_id, integration_id)
);

CREATE TABLE diraigent.audit_log (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    actor_agent_id uuid,
    actor_user_id uuid,
    action text NOT NULL,
    entity_type text NOT NULL,
    entity_id uuid NOT NULL,
    summary text NOT NULL,
    before_state jsonb,
    after_state jsonb,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE diraigent.file_lock (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    path_glob text NOT NULL,
    locked_by uuid NOT NULL,
    locked_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE diraigent.task_changed_file (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id uuid NOT NULL,
    path text NOT NULL,
    change_type text NOT NULL,
    diff text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_changed_file_change_type_check CHECK ((change_type = ANY (ARRAY['added'::text, 'modified'::text, 'deleted'::text])))
);

CREATE TABLE diraigent.task_comment (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id uuid NOT NULL,
    agent_id uuid,
    user_id uuid,
    content text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE diraigent.task_dependency (
    task_id uuid NOT NULL,
    depends_on uuid NOT NULL,
    PRIMARY KEY (task_id, depends_on),
    CONSTRAINT task_dependency_check CHECK ((task_id <> depends_on))
);

CREATE TABLE diraigent.task_goal (
    task_id uuid NOT NULL,
    goal_id uuid NOT NULL,
    PRIMARY KEY (task_id, goal_id)
);

CREATE TABLE diraigent.task_update (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id uuid NOT NULL,
    agent_id uuid,
    user_id uuid,
    kind text DEFAULT 'progress'::text NOT NULL,
    content text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_update_kind_check CHECK ((kind = ANY (ARRAY['progress'::text, 'blocker'::text, 'question'::text, 'artifact'::text, 'review'::text, 'note'::text])))
);

CREATE TABLE diraigent.tenant_member (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid NOT NULL,
    role text DEFAULT 'member'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT tenant_member_role_check CHECK ((role = ANY (ARRAY['owner'::text, 'admin'::text, 'member'::text]))),
    CONSTRAINT tenant_member_tenant_id_user_id_key UNIQUE (tenant_id, user_id)
);

CREATE TABLE diraigent.verification (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    task_id uuid,
    agent_id uuid,
    user_id uuid,
    kind text NOT NULL,
    status text DEFAULT 'pass'::text NOT NULL,
    title text NOT NULL,
    detail text,
    evidence jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT verification_kind_check CHECK ((kind = ANY (ARRAY['test'::text, 'acceptance'::text, 'sign_off'::text]))),
    CONSTRAINT verification_status_check CHECK ((status = ANY (ARRAY['pass'::text, 'fail'::text, 'pending'::text, 'skipped'::text])))
);

CREATE TABLE diraigent.webhook (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL,
    name text NOT NULL,
    url text NOT NULL,
    secret text,
    events text[] DEFAULT '{}'::text[] NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT webhook_project_id_name_key UNIQUE (project_id, name)
);

CREATE TABLE diraigent.webhook_dead_letter (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    webhook_id uuid NOT NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    last_response_status integer,
    last_response_body text,
    attempts integer DEFAULT 0 NOT NULL,
    failed_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE diraigent.webhook_delivery (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    webhook_id uuid NOT NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    response_status integer,
    response_body text,
    delivered_at timestamp with time zone DEFAULT now() NOT NULL,
    success boolean DEFAULT false NOT NULL,
    attempt_number integer DEFAULT 1 NOT NULL
);

CREATE TABLE diraigent.wrapped_key (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid,
    key_type text NOT NULL,
    wrapped_dek text NOT NULL,
    kdf_salt text NOT NULL,
    kdf_params jsonb,
    key_version integer DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT wrapped_key_key_type_check CHECK ((key_type = ANY (ARRAY['login_derived'::text, 'passphrase'::text]))),
    CONSTRAINT wrapped_key_tenant_id_user_id_key_type_key_version_key UNIQUE (tenant_id, user_id, key_type, key_version)
);

-- Foreign keys

ALTER TABLE ONLY diraigent.agent
    ADD CONSTRAINT agent_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES diraigent.auth_user(user_id);

ALTER TABLE ONLY diraigent.agent_integration
    ADD CONSTRAINT agent_integration_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES diraigent.agent(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.agent_integration
    ADD CONSTRAINT agent_integration_integration_id_fkey FOREIGN KEY (integration_id) REFERENCES diraigent.integration(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.agent_integration
    ADD CONSTRAINT agent_integration_role_id_fkey FOREIGN KEY (role_id) REFERENCES diraigent.role(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.audit_log
    ADD CONSTRAINT audit_log_actor_agent_id_fkey FOREIGN KEY (actor_agent_id) REFERENCES diraigent.agent(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.audit_log
    ADD CONSTRAINT audit_log_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.decision
    ADD CONSTRAINT decision_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.decision
    ADD CONSTRAINT decision_superseded_by_fkey FOREIGN KEY (superseded_by) REFERENCES diraigent.decision(id);

ALTER TABLE ONLY diraigent.event
    ADD CONSTRAINT event_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES diraigent.agent(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.event
    ADD CONSTRAINT event_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.event
    ADD CONSTRAINT event_related_task_id_fkey FOREIGN KEY (related_task_id) REFERENCES diraigent.task(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.file_lock
    ADD CONSTRAINT file_lock_locked_by_fkey FOREIGN KEY (locked_by) REFERENCES diraigent.agent(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.file_lock
    ADD CONSTRAINT file_lock_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.file_lock
    ADD CONSTRAINT file_lock_task_id_fkey FOREIGN KEY (task_id) REFERENCES diraigent.task(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.goal
    ADD CONSTRAINT goal_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.integration
    ADD CONSTRAINT integration_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.knowledge
    ADD CONSTRAINT knowledge_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.membership
    ADD CONSTRAINT membership_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES diraigent.agent(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.membership
    ADD CONSTRAINT membership_role_id_fkey FOREIGN KEY (role_id) REFERENCES diraigent.role(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.membership
    ADD CONSTRAINT membership_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES diraigent.tenant(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.observation
    ADD CONSTRAINT observation_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES diraigent.agent(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.observation
    ADD CONSTRAINT observation_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.observation
    ADD CONSTRAINT observation_resolved_task_id_fkey FOREIGN KEY (resolved_task_id) REFERENCES diraigent.task(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.playbook
    ADD CONSTRAINT playbook_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES diraigent.tenant(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.project
    ADD CONSTRAINT project_default_playbook_id_fkey FOREIGN KEY (default_playbook_id) REFERENCES diraigent.playbook(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.project
    ADD CONSTRAINT project_package_id_fkey FOREIGN KEY (package_id) REFERENCES diraigent.package(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.project
    ADD CONSTRAINT project_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.project
    ADD CONSTRAINT project_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES diraigent.tenant(id);

ALTER TABLE ONLY diraigent.role
    ADD CONSTRAINT role_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES diraigent.tenant(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.task
    ADD CONSTRAINT task_assigned_agent_id_fkey FOREIGN KEY (assigned_agent_id) REFERENCES diraigent.agent(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.task
    ADD CONSTRAINT task_assigned_role_id_fkey FOREIGN KEY (assigned_role_id) REFERENCES diraigent.role(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.task
    ADD CONSTRAINT task_decision_id_fkey FOREIGN KEY (decision_id) REFERENCES diraigent.decision(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.task
    ADD CONSTRAINT task_delegated_by_fkey FOREIGN KEY (delegated_by) REFERENCES diraigent.agent(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.task
    ADD CONSTRAINT task_playbook_id_fkey FOREIGN KEY (playbook_id) REFERENCES diraigent.playbook(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.task
    ADD CONSTRAINT task_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.task_changed_file
    ADD CONSTRAINT task_changed_file_task_id_fkey FOREIGN KEY (task_id) REFERENCES diraigent.task(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.task_comment
    ADD CONSTRAINT task_comment_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES diraigent.agent(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.task_comment
    ADD CONSTRAINT task_comment_task_id_fkey FOREIGN KEY (task_id) REFERENCES diraigent.task(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.task_dependency
    ADD CONSTRAINT task_dependency_depends_on_fkey FOREIGN KEY (depends_on) REFERENCES diraigent.task(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.task_dependency
    ADD CONSTRAINT task_dependency_task_id_fkey FOREIGN KEY (task_id) REFERENCES diraigent.task(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.task_goal
    ADD CONSTRAINT task_goal_goal_id_fkey FOREIGN KEY (goal_id) REFERENCES diraigent.goal(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.task_goal
    ADD CONSTRAINT task_goal_task_id_fkey FOREIGN KEY (task_id) REFERENCES diraigent.task(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.task_update
    ADD CONSTRAINT task_update_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES diraigent.agent(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.task_update
    ADD CONSTRAINT task_update_task_id_fkey FOREIGN KEY (task_id) REFERENCES diraigent.task(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.tenant_member
    ADD CONSTRAINT tenant_member_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES diraigent.tenant(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.tenant_member
    ADD CONSTRAINT tenant_member_user_id_fkey FOREIGN KEY (user_id) REFERENCES diraigent.auth_user(user_id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.verification
    ADD CONSTRAINT verification_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES diraigent.agent(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.verification
    ADD CONSTRAINT verification_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.verification
    ADD CONSTRAINT verification_task_id_fkey FOREIGN KEY (task_id) REFERENCES diraigent.task(id) ON DELETE SET NULL;

ALTER TABLE ONLY diraigent.webhook
    ADD CONSTRAINT webhook_project_id_fkey FOREIGN KEY (project_id) REFERENCES diraigent.project(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.webhook_dead_letter
    ADD CONSTRAINT webhook_dead_letter_webhook_id_fkey FOREIGN KEY (webhook_id) REFERENCES diraigent.webhook(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.webhook_delivery
    ADD CONSTRAINT webhook_delivery_webhook_id_fkey FOREIGN KEY (webhook_id) REFERENCES diraigent.webhook(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.wrapped_key
    ADD CONSTRAINT wrapped_key_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES diraigent.tenant(id) ON DELETE CASCADE;

ALTER TABLE ONLY diraigent.wrapped_key
    ADD CONSTRAINT wrapped_key_user_id_fkey FOREIGN KEY (user_id) REFERENCES diraigent.auth_user(user_id) ON DELETE CASCADE;

-- Indexes

CREATE INDEX idx_agent_owner_id ON diraigent.agent USING btree (owner_id);
CREATE INDEX idx_agent_integration_agent ON diraigent.agent_integration USING btree (agent_id);
CREATE INDEX idx_audit_action ON diraigent.audit_log USING btree (action);
CREATE INDEX idx_audit_actor_agent ON diraigent.audit_log USING btree (actor_agent_id);
CREATE INDEX idx_audit_created ON diraigent.audit_log USING btree (project_id, created_at DESC);
CREATE INDEX idx_audit_entity ON diraigent.audit_log USING btree (entity_type, entity_id);
CREATE INDEX idx_audit_project ON diraigent.audit_log USING btree (project_id);
CREATE UNIQUE INDEX idx_auth_user_auth_user_id ON diraigent.auth_user USING btree (auth_user_id);
CREATE INDEX idx_decision_project ON diraigent.decision USING btree (project_id);
CREATE INDEX idx_decision_status ON diraigent.decision USING btree (project_id, status);
CREATE INDEX idx_event_created ON diraigent.event USING btree (project_id, created_at DESC);
CREATE INDEX idx_event_kind ON diraigent.event USING btree (project_id, kind);
CREATE INDEX idx_event_project ON diraigent.event USING btree (project_id);
CREATE INDEX idx_event_severity ON diraigent.event USING btree (severity) WHERE (severity = ANY (ARRAY['error'::text, 'critical'::text]));
CREATE INDEX idx_file_lock_project ON diraigent.file_lock USING btree (project_id);
CREATE INDEX idx_file_lock_task ON diraigent.file_lock USING btree (task_id);
CREATE INDEX idx_goal_project ON diraigent.goal USING btree (project_id);
CREATE INDEX idx_goal_status ON diraigent.goal USING btree (status);
CREATE INDEX idx_integration_kind ON diraigent.integration USING btree (project_id, kind);
CREATE INDEX idx_integration_project ON diraigent.integration USING btree (project_id);
CREATE INDEX idx_integration_provider ON diraigent.integration USING btree (project_id, provider);
CREATE INDEX idx_knowledge_category ON diraigent.knowledge USING btree (project_id, category);
CREATE INDEX idx_knowledge_project ON diraigent.knowledge USING btree (project_id);
CREATE INDEX idx_knowledge_tags ON diraigent.knowledge USING gin (tags);
CREATE INDEX idx_membership_agent ON diraigent.membership USING btree (agent_id);
CREATE INDEX idx_membership_role ON diraigent.membership USING btree (role_id);
CREATE INDEX idx_membership_tenant ON diraigent.membership USING btree (tenant_id);
CREATE INDEX idx_observation_project ON diraigent.observation USING btree (project_id);
CREATE INDEX idx_observation_severity ON diraigent.observation USING btree (project_id, severity) WHERE (status = 'open'::text);
CREATE INDEX idx_observation_status ON diraigent.observation USING btree (status);
CREATE INDEX idx_playbook_tags ON diraigent.playbook USING gin (tags);
CREATE INDEX idx_playbook_tenant ON diraigent.playbook USING btree (tenant_id);
CREATE INDEX idx_project_parent ON diraigent.project USING btree (parent_id);
CREATE INDEX idx_project_tenant ON diraigent.project USING btree (tenant_id);
CREATE INDEX idx_role_tenant ON diraigent.role USING btree (tenant_id);
CREATE INDEX idx_task_agent ON diraigent.task USING btree (assigned_agent_id);
CREATE INDEX idx_task_changed_file_task_id ON diraigent.task_changed_file USING btree (task_id);
CREATE INDEX idx_task_comment_task ON diraigent.task_comment USING btree (task_id);
CREATE INDEX idx_task_decision_id ON diraigent.task USING btree (decision_id) WHERE (decision_id IS NOT NULL);
CREATE INDEX idx_task_deps_depends ON diraigent.task_dependency USING btree (depends_on);
CREATE INDEX idx_task_goal_goal ON diraigent.task_goal USING btree (goal_id);
CREATE INDEX idx_task_playbook ON diraigent.task USING btree (playbook_id) WHERE (playbook_id IS NOT NULL);
CREATE INDEX idx_task_priority ON diraigent.task USING btree (project_id, priority DESC) WHERE (state = ANY (ARRAY['backlog'::text, 'ready'::text]));
CREATE INDEX idx_task_project ON diraigent.task USING btree (project_id);
CREATE INDEX idx_task_role ON diraigent.task USING btree (assigned_role_id);
CREATE INDEX idx_task_state ON diraigent.task USING btree (state);
CREATE INDEX idx_task_update_task ON diraigent.task_update USING btree (task_id);
CREATE INDEX idx_tenant_member_tenant ON diraigent.tenant_member USING btree (tenant_id);
CREATE INDEX idx_tenant_member_user ON diraigent.tenant_member USING btree (user_id);
CREATE INDEX idx_verification_kind ON diraigent.verification USING btree (project_id, kind);
CREATE INDEX idx_verification_project ON diraigent.verification USING btree (project_id);
CREATE INDEX idx_verification_task ON diraigent.verification USING btree (task_id);
CREATE INDEX idx_webhook_dead_letter_time ON diraigent.webhook_dead_letter USING btree (failed_at DESC);
CREATE INDEX idx_webhook_dead_letter_webhook ON diraigent.webhook_dead_letter USING btree (webhook_id);
CREATE INDEX idx_webhook_delivery_time ON diraigent.webhook_delivery USING btree (webhook_id, delivered_at DESC);
CREATE INDEX idx_webhook_delivery_webhook ON diraigent.webhook_delivery USING btree (webhook_id);
CREATE INDEX idx_webhook_project ON diraigent.webhook USING btree (project_id);
CREATE INDEX idx_wrapped_key_tenant ON diraigent.wrapped_key USING btree (tenant_id);

-- Triggers

CREATE TRIGGER task_number_trigger BEFORE INSERT ON diraigent.task FOR EACH ROW EXECUTE FUNCTION diraigent.assign_task_number();
CREATE TRIGGER trg_agent_updated BEFORE UPDATE ON diraigent.agent FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_decision_updated BEFORE UPDATE ON diraigent.decision FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_goal_updated BEFORE UPDATE ON diraigent.goal FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_integration_updated BEFORE UPDATE ON diraigent.integration FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_knowledge_updated BEFORE UPDATE ON diraigent.knowledge FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_membership_updated BEFORE UPDATE ON diraigent.membership FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_observation_updated BEFORE UPDATE ON diraigent.observation FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_package_updated BEFORE UPDATE ON diraigent.package FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_playbook_updated BEFORE UPDATE ON diraigent.playbook FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_project_updated BEFORE UPDATE ON diraigent.project FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_role_updated BEFORE UPDATE ON diraigent.role FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_task_comment_updated BEFORE UPDATE ON diraigent.task_comment FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_task_updated BEFORE UPDATE ON diraigent.task FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
CREATE TRIGGER trg_webhook_updated BEFORE UPDATE ON diraigent.webhook FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
