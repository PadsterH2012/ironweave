use rusqlite::Connection;

/// Run an ALTER TABLE migration, ignoring "duplicate column" errors (expected on re-run)
/// but logging unexpected failures.
fn migrate_alter(conn: &Connection, sql: &str) {
    if let Err(e) = conn.execute(sql, []) {
        let msg = e.to_string();
        // "duplicate column name" is expected when migration has already run
        if !msg.contains("duplicate column") {
            tracing::warn!("Migration failed ({}): {}", sql.split_whitespace().take(6).collect::<Vec<_>>().join(" "), msg);
        }
    }
}

pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS mounts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            mount_type TEXT NOT NULL CHECK(mount_type IN ('nfs', 'smb', 'sshfs')),
            remote_path TEXT NOT NULL,
            local_mount_point TEXT NOT NULL,
            username TEXT,
            password TEXT,
            ssh_key TEXT,
            mount_options TEXT,
            auto_mount INTEGER NOT NULL DEFAULT 1,
            state TEXT NOT NULL DEFAULT 'unmounted'
                CHECK(state IN ('mounted', 'unmounted', 'error')),
            last_error TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            directory TEXT NOT NULL,
            context TEXT NOT NULL CHECK(context IN ('work', 'homelab')),
            obsidian_vault_path TEXT,
            obsidian_project TEXT,
            git_remote TEXT,
            mount_id TEXT REFERENCES mounts(id),
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS teams (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            project_id TEXT NOT NULL REFERENCES projects(id),
            coordination_mode TEXT NOT NULL DEFAULT 'pipeline'
                CHECK(coordination_mode IN ('pipeline', 'swarm', 'collaborative', 'hierarchical')),
            max_agents INTEGER NOT NULL DEFAULT 5,
            token_budget INTEGER,
            cost_budget_daily REAL,
            is_template INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(name, project_id)
        );

        CREATE TABLE IF NOT EXISTS team_agent_slots (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            runtime TEXT NOT NULL CHECK(runtime IN ('claude', 'opencode', 'gemini')),
            config TEXT NOT NULL DEFAULT '{}',
            slot_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS agent_sessions (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL REFERENCES teams(id),
            slot_id TEXT NOT NULL REFERENCES team_agent_slots(id),
            workflow_instance_id TEXT,
            runtime TEXT NOT NULL,
            pid INTEGER,
            worktree_path TEXT,
            branch TEXT,
            state TEXT NOT NULL DEFAULT 'idle'
                CHECK(state IN ('idle', 'working', 'blocked', 'crashed', 'dead')),
            claimed_task_id TEXT,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            cost REAL NOT NULL DEFAULT 0.0,
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_heartbeat TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS issues (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            type TEXT NOT NULL DEFAULT 'task'
                CHECK(type IN ('bug', 'feature', 'task')),
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'open'
                CHECK(status IN ('open', 'in_progress', 'review', 'closed')),
            priority INTEGER NOT NULL DEFAULT 5,
            claimed_by TEXT REFERENCES agent_sessions(id),
            claimed_at TEXT,
            depends_on TEXT NOT NULL DEFAULT '[]',
            summary TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS workflow_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            project_id TEXT NOT NULL REFERENCES projects(id),
            team_id TEXT NOT NULL REFERENCES teams(id),
            dag TEXT NOT NULL DEFAULT '{}',
            version INTEGER NOT NULL DEFAULT 1,
            git_sha TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS workflow_instances (
            id TEXT PRIMARY KEY,
            definition_id TEXT NOT NULL REFERENCES workflow_definitions(id),
            state TEXT NOT NULL DEFAULT 'pending'
                CHECK(state IN ('pending', 'running', 'paused', 'failed', 'completed')),
            current_stage TEXT,
            checkpoint TEXT NOT NULL DEFAULT '{}',
            started_at TEXT,
            completed_at TEXT,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost REAL NOT NULL DEFAULT 0.0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS loom_entries (
            id TEXT PRIMARY KEY,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            agent_id TEXT REFERENCES agent_sessions(id),
            team_id TEXT NOT NULL REFERENCES teams(id),
            project_id TEXT NOT NULL REFERENCES projects(id),
            workflow_instance_id TEXT,
            entry_type TEXT NOT NULL
                CHECK(entry_type IN ('status', 'finding', 'warning', 'delegation', 'escalation', 'completion')),
            content TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS merge_queue_entries (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id),
            branch TEXT NOT NULL,
            worktree_path TEXT NOT NULL,
            target_branch TEXT NOT NULL DEFAULT 'main',
            status TEXT NOT NULL DEFAULT 'queued'
                CHECK(status IN ('queued', 'validating', 'merging', 'conflict', 'merged', 'failed')),
            conflict_tier INTEGER,
            queued_at TEXT NOT NULL DEFAULT (datetime('now')),
            merged_at TEXT
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'general',
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS proxy_configs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            hops TEXT NOT NULL DEFAULT '[]',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_issues_project ON issues(project_id);
        CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
        CREATE INDEX IF NOT EXISTS idx_agent_sessions_team ON agent_sessions(team_id);
        CREATE INDEX IF NOT EXISTS idx_agent_sessions_state ON agent_sessions(state);
        CREATE INDEX IF NOT EXISTS idx_loom_entries_project ON loom_entries(project_id);
        CREATE INDEX IF NOT EXISTS idx_loom_entries_team ON loom_entries(team_id);
        CREATE INDEX IF NOT EXISTS idx_merge_queue_status ON merge_queue_entries(status);
        CREATE INDEX IF NOT EXISTS idx_workflow_instances_state ON workflow_instances(state);
        CREATE INDEX IF NOT EXISTS idx_mounts_state ON mounts(state);
        CREATE INDEX IF NOT EXISTS idx_settings_category ON settings(category);
    ")?;

    // Incremental migration: add mount_id to existing projects table
    // ALTER TABLE ADD COLUMN errors if column already exists, which is fine
    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN mount_id TEXT REFERENCES mounts(id)");
    migrate_alter(conn,"ALTER TABLE mounts ADD COLUMN proxy_config_id TEXT REFERENCES proxy_configs(id)");

    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN description TEXT");
    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN sync_path TEXT");
    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN last_synced_at TEXT");
    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN sync_state TEXT NOT NULL DEFAULT 'idle'");
    migrate_alter(conn,"ALTER TABLE mounts ADD COLUMN git_remote TEXT");

    // Add workflow linkage columns to issues
    migrate_alter(conn,"ALTER TABLE issues ADD COLUMN workflow_instance_id TEXT REFERENCES workflow_instances(id)");
    migrate_alter(conn,"ALTER TABLE issues ADD COLUMN stage_id TEXT");

    // Teams & model selection
    migrate_alter(conn,"ALTER TABLE team_agent_slots ADD COLUMN model TEXT");
    migrate_alter(conn,"ALTER TABLE agent_sessions ADD COLUMN model TEXT");

    // Team dispatch
    migrate_alter(conn,"ALTER TABLE issues ADD COLUMN role TEXT");
    migrate_alter(conn,"ALTER TABLE teams ADD COLUMN auto_pickup_types TEXT DEFAULT '[\"task\",\"bug\",\"feature\"]'");
    migrate_alter(conn,"ALTER TABLE teams ADD COLUMN is_active INTEGER DEFAULT 0");

    // Intake agent columns
    migrate_alter(conn,"ALTER TABLE issues ADD COLUMN parent_id TEXT REFERENCES issues(id)");
    migrate_alter(conn,"ALTER TABLE issues ADD COLUMN needs_intake INTEGER DEFAULT 1");
    migrate_alter(conn,"ALTER TABLE issues ADD COLUMN scope_mode TEXT DEFAULT 'auto'");

    // Indexes for intake queries
    migrate_alter(conn,"CREATE INDEX IF NOT EXISTS idx_issues_parent ON issues(parent_id)");
    migrate_alter(conn,"CREATE INDEX IF NOT EXISTS idx_issues_needs_intake ON issues(needs_intake)");

    // Team coordination: is_lead flag for hierarchical mode
    migrate_alter(conn,"ALTER TABLE team_agent_slots ADD COLUMN is_lead INTEGER NOT NULL DEFAULT 0");

    // Retry tracking for issues
    migrate_alter(conn,"ALTER TABLE issues ADD COLUMN retry_count INTEGER DEFAULT 0");

    // Attachments table
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS attachments (
            id          TEXT PRIMARY KEY,
            issue_id    TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
            filename    TEXT NOT NULL,
            mime_type   TEXT NOT NULL,
            size_bytes  INTEGER NOT NULL,
            stored_path TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_attachments_issue ON attachments(issue_id);
    ")?;

    // ── Merge Queue (orchestrator-driven) ──────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS merge_queue (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            branch_name TEXT NOT NULL,
            agent_session_id TEXT,
            issue_id TEXT,
            team_id TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            conflict_files TEXT DEFAULT '[]',
            resolver_agent_id TEXT,
            error_message TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        CREATE INDEX IF NOT EXISTS idx_merge_queue_project ON merge_queue(project_id);
        CREATE INDEX IF NOT EXISTS idx_merge_queue_mq_status ON merge_queue(status);
    ")?;

    // ── Activity Log ─────────────────────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS activity_log (
            id TEXT PRIMARY KEY,
            event_type TEXT NOT NULL,
            project_id TEXT,
            team_id TEXT,
            agent_id TEXT,
            issue_id TEXT,
            workflow_instance_id TEXT,
            message TEXT NOT NULL,
            metadata TEXT DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_activity_log_created ON activity_log(created_at);
        CREATE INDEX IF NOT EXISTS idx_activity_log_project ON activity_log(project_id);
        CREATE INDEX IF NOT EXISTS idx_activity_log_type ON activity_log(event_type);
    ")?;

    // ── Project App Preview ──────────────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS project_apps (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            pid INTEGER,
            port INTEGER,
            run_command TEXT NOT NULL,
            state TEXT CHECK(state IN ('stopped', 'starting', 'running', 'error')) DEFAULT 'stopped',
            last_error TEXT,
            started_at TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );
    ")?;

    // Add app_url to projects
    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN app_url TEXT");

    // ── Prompt Template Library ─────────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS prompt_templates (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            template_type TEXT NOT NULL DEFAULT 'role' CHECK(template_type IN ('role', 'skill')),
            content TEXT NOT NULL DEFAULT '',
            project_id TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        CREATE TABLE IF NOT EXISTS prompt_template_assignments (
            id TEXT PRIMARY KEY,
            role TEXT NOT NULL,
            template_id TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (template_id) REFERENCES prompt_templates(id) ON DELETE CASCADE,
            UNIQUE(role, template_id)
        );
    ")?;

    // ── Workflow Gate Approvals ──────────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS workflow_gate_approvals (
            instance_id TEXT NOT NULL,
            stage_id TEXT NOT NULL,
            approved_at TEXT NOT NULL,
            PRIMARY KEY (instance_id, stage_id)
        );
    ")?;

    // ── Cost Tracking (v2 Phase 1.2) ─────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS cost_tracking (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            period TEXT NOT NULL CHECK(period IN ('daily', 'weekly', 'monthly')),
            period_start TEXT NOT NULL,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost_usd REAL NOT NULL DEFAULT 0.0,
            by_tier TEXT NOT NULL DEFAULT '{}',
            by_role TEXT NOT NULL DEFAULT '{}',
            by_model TEXT NOT NULL DEFAULT '{}',
            task_count INTEGER NOT NULL DEFAULT 0,
            failure_count INTEGER NOT NULL DEFAULT 0,
            escalation_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(project_id, period, period_start)
        );
        CREATE INDEX IF NOT EXISTS idx_cost_tracking_project ON cost_tracking(project_id);
        CREATE INDEX IF NOT EXISTS idx_cost_tracking_period ON cost_tracking(period, period_start);
    ")?;

    // ── Global Role Registry (v2 Phase 1.3) ──────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS roles (
            name TEXT PRIMARY KEY,
            category TEXT NOT NULL DEFAULT 'General',
            default_runtime TEXT NOT NULL DEFAULT 'claude'
                CHECK(default_runtime IN ('claude', 'opencode', 'gemini')),
            default_provider TEXT NOT NULL DEFAULT 'anthropic',
            default_model TEXT,
            default_skills TEXT NOT NULL DEFAULT '[]',
            min_model_tier INTEGER NOT NULL DEFAULT 1 CHECK(min_model_tier BETWEEN 1 AND 5),
            description TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
    ")?;

    // ── Quality Tiers (v2 Phase 2.1) ────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS quality_tiers (
            tier INTEGER PRIMARY KEY CHECK(tier BETWEEN 1 AND 5),
            label TEXT NOT NULL,
            example_models TEXT NOT NULL DEFAULT '',
            cost_range TEXT NOT NULL DEFAULT ''
        );
        INSERT OR IGNORE INTO quality_tiers (tier, label, example_models, cost_range) VALUES
            (1, 'Free/Local', 'llama-3.1-8b, gemma-3-8b, qwen-2.5-7b', 'Free'),
            (2, 'Budget', 'llama-3.1-70b, gemma-3-27b, qwen-2.5-72b', '¢'),
            (3, 'Mid', 'claude-haiku-4-5, deepseek-v3, mistral-large', '$'),
            (4, 'High', 'claude-sonnet-4-6, gemini-2.5-flash', '$$'),
            (5, 'Premium', 'claude-opus-4-6, gemini-2.5-pro', '$$$');
    ")?;

    // Add tier_floor / tier_ceiling to projects and teams
    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN tier_floor INTEGER NOT NULL DEFAULT 1");
    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN tier_ceiling INTEGER NOT NULL DEFAULT 5");
    migrate_alter(conn,"ALTER TABLE teams ADD COLUMN tier_floor INTEGER");
    migrate_alter(conn,"ALTER TABLE teams ADD COLUMN tier_ceiling INTEGER");

    // ── Model Performance Log (v2 Phase 2.2) ─────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS model_performance_log (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            role TEXT NOT NULL,
            runtime TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT 'anthropic',
            model TEXT NOT NULL,
            tier INTEGER NOT NULL DEFAULT 3,
            task_type TEXT NOT NULL DEFAULT 'task',
            task_complexity INTEGER NOT NULL DEFAULT 3,
            outcome TEXT NOT NULL CHECK(outcome IN ('success', 'failure', 'partial', 'timeout')),
            failure_reason TEXT,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            cost_usd REAL NOT NULL DEFAULT 0.0,
            duration_seconds INTEGER NOT NULL DEFAULT 0,
            retries INTEGER NOT NULL DEFAULT 0,
            escalated_from TEXT,
            complexity_score INTEGER,
            files_touched TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_perf_log_project ON model_performance_log(project_id);
        CREATE INDEX IF NOT EXISTS idx_perf_log_role_model ON model_performance_log(role, model);
        CREATE INDEX IF NOT EXISTS idx_perf_log_outcome ON model_performance_log(outcome);
    ")?;

    // ── Coordinator Memory (v2 Phase 2.3) ────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS coordinator_memory (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) UNIQUE,
            state TEXT NOT NULL DEFAULT 'dormant' CHECK(state IN ('active', 'dormant')),
            session_id TEXT,
            last_active_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
    ")?;

    // ── Phase 2.4: Coordination-mode-dependent skills ─────────────────
    // Add is_system flag and coordination_mode to prompt_templates
    migrate_alter(conn,"ALTER TABLE prompt_templates ADD COLUMN is_system INTEGER NOT NULL DEFAULT 0");
    migrate_alter(conn,"ALTER TABLE prompt_templates ADD COLUMN coordination_mode TEXT");

    // Seed system skill templates for each coordination mode
    conn.execute_batch("
        INSERT OR IGNORE INTO prompt_templates (id, name, template_type, content, is_system, coordination_mode)
        VALUES
        ('sys-skill-pipeline', 'Pipeline Coordination', 'skill',
         'You are working in a **pipeline** coordination mode. Work flows sequentially through roles in a defined order. When you complete your task, update the issue status so the next agent in the pipeline can pick it up. Do NOT work on tasks that are not assigned to your role in the pipeline sequence. Focus on doing your part well and handing off cleanly.',
         1, 'pipeline'),

        ('sys-skill-swarm', 'Swarm Coordination', 'skill',
         'You are working in a **swarm** coordination mode. All agents work independently, claiming tasks from a shared pool. Claim one task at a time, complete it fully, then claim the next. Avoid working on tasks another agent has already claimed. If you encounter a dependency on another task, skip it and move to a different one.',
         1, 'swarm'),

        ('sys-skill-collaborative', 'Collaborative Coordination', 'skill',
         'You are working in a **collaborative** coordination mode. Multiple agents may work on related aspects of the same problem simultaneously. Coordinate through issue comments and loom entries. Before making changes to shared files, check recent loom entries for conflicts. Prefer small, focused commits that are easy to merge.',
         1, 'collaborative'),

        ('sys-skill-hierarchical', 'Hierarchical Coordination', 'skill',
         'You are working in a **hierarchical** coordination mode. A lead agent (Architect or Coordinator) decomposes work and assigns sub-tasks. Follow the decomposed task assignments precisely. Report progress via loom entries so the lead can track overall status. If you encounter issues outside your assigned scope, create a new issue rather than fixing it yourself.',
         1, 'hierarchical');
    ")?;

    // ── Phase 3.1: Model Routing Overrides ─────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS model_routing_overrides (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            role TEXT NOT NULL,
            task_type TEXT NOT NULL DEFAULT 'task',
            from_model TEXT,
            to_model TEXT NOT NULL,
            to_tier INTEGER NOT NULL,
            reason TEXT NOT NULL DEFAULT '',
            confidence REAL NOT NULL DEFAULT 0.5,
            status TEXT NOT NULL DEFAULT 'suggested'
                CHECK(status IN ('suggested', 'accepted', 'rejected', 'expired')),
            evidence TEXT NOT NULL DEFAULT '{}',
            observations INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            resolved_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_routing_overrides_project ON model_routing_overrides(project_id, status);
        CREATE INDEX IF NOT EXISTS idx_routing_overrides_role ON model_routing_overrides(role, task_type);
    ")?;

    // ── Phase 3.3: Team-Level Model Overrides ──────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS team_role_overrides (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            runtime TEXT,
            provider TEXT,
            model TEXT,
            is_user_set INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(team_id, role)
        );
    ")?;

    // ══════════════════════════════════════════════════════════════════
    // Phase 4 — Intelligence
    // ══════════════════════════════════════════════════════════════════

    // ── Phase 4.1: Knowledge Graph ─────────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS code_graph_nodes (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            node_type TEXT NOT NULL CHECK(node_type IN ('file', 'function', 'class', 'module')),
            path TEXT NOT NULL,
            name TEXT NOT NULL,
            language TEXT,
            line_start INTEGER,
            line_end INTEGER,
            complexity_score INTEGER NOT NULL DEFAULT 1,
            last_indexed_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(project_id, path, name, node_type)
        );
        CREATE INDEX IF NOT EXISTS idx_graph_nodes_project ON code_graph_nodes(project_id);
        CREATE INDEX IF NOT EXISTS idx_graph_nodes_path ON code_graph_nodes(project_id, path);

        CREATE TABLE IF NOT EXISTS code_graph_edges (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            source_node_id TEXT NOT NULL REFERENCES code_graph_nodes(id) ON DELETE CASCADE,
            target_node_id TEXT NOT NULL REFERENCES code_graph_nodes(id) ON DELETE CASCADE,
            edge_type TEXT NOT NULL CHECK(edge_type IN ('imports', 'calls', 'extends', 'implements', 'contains', 'references')),
            weight INTEGER NOT NULL DEFAULT 1,
            UNIQUE(source_node_id, target_node_id, edge_type)
        );
        CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON code_graph_edges(source_node_id);
        CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON code_graph_edges(target_node_id);
    ")?;

    // ── Phase 4.2: Workflow Recording ──────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS workflow_traces (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            agent_session_id TEXT NOT NULL,
            performance_log_id TEXT,
            issue_id TEXT,
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            completed_at TEXT,
            total_steps INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'recording' CHECK(status IN ('recording', 'completed', 'failed'))
        );
        CREATE INDEX IF NOT EXISTS idx_traces_project ON workflow_traces(project_id);
        CREATE INDEX IF NOT EXISTS idx_traces_session ON workflow_traces(agent_session_id);

        CREATE TABLE IF NOT EXISTS workflow_trace_steps (
            id TEXT PRIMARY KEY,
            trace_id TEXT NOT NULL REFERENCES workflow_traces(id) ON DELETE CASCADE,
            step_number INTEGER NOT NULL,
            action TEXT NOT NULL,
            detail TEXT NOT NULL DEFAULT '',
            files_touched TEXT,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            outcome TEXT CHECK(outcome IN ('success', 'failure', 'skipped')),
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_trace_steps_trace ON workflow_trace_steps(trace_id);

        CREATE TABLE IF NOT EXISTS workflow_chokepoints (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            action TEXT NOT NULL,
            role TEXT,
            failure_count INTEGER NOT NULL DEFAULT 0,
            total_count INTEGER NOT NULL DEFAULT 0,
            avg_duration_ms INTEGER NOT NULL DEFAULT 0,
            last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(project_id, action, role)
        );
    ")?;

    // ── Phase 4.3: Context Window Management ───────────────────────────
    // Add context window limits to quality_tiers
    migrate_alter(conn,"ALTER TABLE quality_tiers ADD COLUMN max_context_tokens INTEGER NOT NULL DEFAULT 200000");
    migrate_alter(conn,"ALTER TABLE quality_tiers ADD COLUMN max_output_tokens INTEGER NOT NULL DEFAULT 8192");

    // Update with realistic limits per tier
    migrate_alter(conn,"UPDATE quality_tiers SET max_context_tokens = 8192, max_output_tokens = 2048 WHERE tier = 1");
    migrate_alter(conn,"UPDATE quality_tiers SET max_context_tokens = 32000, max_output_tokens = 4096 WHERE tier = 2");
    migrate_alter(conn,"UPDATE quality_tiers SET max_context_tokens = 200000, max_output_tokens = 8192 WHERE tier = 3");
    migrate_alter(conn,"UPDATE quality_tiers SET max_context_tokens = 200000, max_output_tokens = 16384 WHERE tier = 4");
    migrate_alter(conn,"UPDATE quality_tiers SET max_context_tokens = 200000, max_output_tokens = 32000 WHERE tier = 5");

    // ── Phase 4.4: Cross-Project Learning ──────────────────────────────
    migrate_alter(conn,"ALTER TABLE projects ADD COLUMN share_learning INTEGER NOT NULL DEFAULT 0");

    // ── Dead session state ───────────────────────────────────────────
    // Add 'dead' to agent_sessions state CHECK constraint.  Only runs once:
    // we check whether the existing CHECK already includes 'dead'.
    let needs_dead_migration: bool = {
        let mut stmt = conn.prepare(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='agent_sessions'"
        )?;
        let sql: String = stmt.query_row([], |row| row.get(0))?;
        !sql.contains("dead")
    };
    if needs_dead_migration {
        // Disable FK enforcement during table swap (other tables reference agent_sessions)
        conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        conn.execute_batch("
            DROP TABLE IF EXISTS agent_sessions_new;
            CREATE TABLE agent_sessions_new (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL REFERENCES teams(id),
                slot_id TEXT NOT NULL REFERENCES team_agent_slots(id),
                workflow_instance_id TEXT,
                runtime TEXT NOT NULL,
                pid INTEGER,
                worktree_path TEXT,
                branch TEXT,
                state TEXT NOT NULL DEFAULT 'idle'
                    CHECK(state IN ('idle', 'working', 'blocked', 'crashed', 'dead')),
                claimed_task_id TEXT,
                tokens_used INTEGER NOT NULL DEFAULT 0,
                cost REAL NOT NULL DEFAULT 0.0,
                started_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_heartbeat TEXT NOT NULL DEFAULT (datetime('now'))
            );
            INSERT INTO agent_sessions_new
                SELECT id, team_id, slot_id, workflow_instance_id, runtime, pid,
                       worktree_path, branch, state, claimed_task_id, tokens_used,
                       cost, started_at, last_heartbeat
                FROM agent_sessions;
            DROP TABLE agent_sessions;
            ALTER TABLE agent_sessions_new RENAME TO agent_sessions;
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_team ON agent_sessions(team_id);
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_state ON agent_sessions(state);
        ")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    }

    // ── Killswitch: project-level pause ─────────────────────────────────
    migrate_alter(conn, "ALTER TABLE projects ADD COLUMN is_paused INTEGER NOT NULL DEFAULT 0");
    migrate_alter(conn, "ALTER TABLE projects ADD COLUMN paused_at TEXT");
    migrate_alter(conn, "ALTER TABLE projects ADD COLUMN pause_reason TEXT");

    // ── Killswitch: dispatch schedules table ────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS dispatch_schedules (
            id TEXT PRIMARY KEY,
            scope TEXT NOT NULL CHECK(scope IN ('global', 'project')),
            project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
            cron_expression TEXT NOT NULL,
            action TEXT NOT NULL CHECK(action IN ('resume', 'pause')),
            timezone TEXT NOT NULL DEFAULT 'Europe/London',
            is_enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            description TEXT
        );
    ")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_migrations_run_cleanly() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"projects".to_string()));
        assert!(tables.contains(&"teams".to_string()));
        assert!(tables.contains(&"issues".to_string()));
        assert!(tables.contains(&"agent_sessions".to_string()));
        assert!(tables.contains(&"workflow_definitions".to_string()));
        assert!(tables.contains(&"workflow_instances".to_string()));
        assert!(tables.contains(&"loom_entries".to_string()));
        assert!(tables.contains(&"merge_queue_entries".to_string()));
    }

    #[test]
    fn test_migrations_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
    }

    #[test]
    fn test_mounts_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"mounts".to_string()));
    }

    #[test]
    fn test_projects_has_mount_id_column() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO mounts (id, name, mount_type, remote_path, local_mount_point) VALUES ('m1', 'test', 'nfs', 'server:/share', '/mnt/test')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO projects (id, name, directory, context, mount_id) VALUES ('p1', 'proj', '/tmp', 'work', 'm1')",
            [],
        ).unwrap();

        let mount_id: Option<String> = conn
            .query_row("SELECT mount_id FROM projects WHERE id = 'p1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mount_id, Some("m1".to_string()));
    }

    #[test]
    fn test_settings_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO settings (key, value, category) VALUES ('test_key', 'test_value', 'general')",
            [],
        ).unwrap();

        let val: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'test_key'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(val, "test_value");
    }

    #[test]
    fn test_proxy_configs_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO proxy_configs (id, name, hops) VALUES ('pc1', 'test-proxy', '[{\"host\":\"10.0.0.1\",\"port\":22}]')",
            [],
        ).unwrap();

        let name: String = conn
            .query_row("SELECT name FROM proxy_configs WHERE id = 'pc1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "test-proxy");
    }

    #[test]
    fn test_mounts_has_proxy_config_id() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO proxy_configs (id, name, hops) VALUES ('pc1', 'proxy', '[]')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO mounts (id, name, mount_type, remote_path, local_mount_point, proxy_config_id) VALUES ('m1', 'test', 'sshfs', 'user@host:/path', '/mnt/test', 'pc1')",
            [],
        ).unwrap();

        let pcid: Option<String> = conn
            .query_row("SELECT proxy_config_id FROM mounts WHERE id = 'm1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(pcid, Some("pc1".to_string()));
    }

    #[test]
    fn test_projects_has_sync_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO projects (id, name, directory, context, description, sync_path, last_synced_at, sync_state) VALUES ('p1', 'proj', '/tmp', 'work', 'A test project', '/sync/p1', '2026-01-01T00:00:00Z', 'idle')",
            [],
        ).unwrap();

        let desc: Option<String> = conn
            .query_row("SELECT description FROM projects WHERE id = 'p1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(desc, Some("A test project".to_string()));

        let last_synced: Option<String> = conn
            .query_row("SELECT last_synced_at FROM projects WHERE id = 'p1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(last_synced, Some("2026-01-01T00:00:00Z".to_string()));

        let sync_state: String = conn
            .query_row("SELECT sync_state FROM projects WHERE id = 'p1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sync_state, "idle");
    }

    #[test]
    fn test_issues_has_intake_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'proj', '/tmp', 'work')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO issues (id, project_id, title, parent_id, needs_intake, scope_mode) VALUES ('i1', 'p1', 'Parent', NULL, 1, 'auto')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO issues (id, project_id, title, parent_id, needs_intake, scope_mode) VALUES ('i2', 'p1', 'Child', 'i1', 0, 'auto')",
            [],
        ).unwrap();

        let parent_id: Option<String> = conn
            .query_row("SELECT parent_id FROM issues WHERE id = 'i2'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(parent_id, Some("i1".to_string()));

        let needs_intake: i64 = conn
            .query_row("SELECT needs_intake FROM issues WHERE id = 'i1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(needs_intake, 1);
    }
}
