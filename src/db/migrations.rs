use rusqlite::Connection;

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
                CHECK(state IN ('idle', 'working', 'blocked', 'crashed')),
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
    let _ = conn.execute("ALTER TABLE projects ADD COLUMN mount_id TEXT REFERENCES mounts(id)", []);
    let _ = conn.execute("ALTER TABLE mounts ADD COLUMN proxy_config_id TEXT REFERENCES proxy_configs(id)", []);

    let _ = conn.execute("ALTER TABLE projects ADD COLUMN description TEXT", []);
    let _ = conn.execute("ALTER TABLE projects ADD COLUMN sync_path TEXT", []);
    let _ = conn.execute("ALTER TABLE projects ADD COLUMN last_synced_at TEXT", []);
    let _ = conn.execute("ALTER TABLE projects ADD COLUMN sync_state TEXT NOT NULL DEFAULT 'idle'", []);
    let _ = conn.execute("ALTER TABLE mounts ADD COLUMN git_remote TEXT", []);

    // Add workflow linkage columns to issues
    let _ = conn.execute("ALTER TABLE issues ADD COLUMN workflow_instance_id TEXT REFERENCES workflow_instances(id)", []);
    let _ = conn.execute("ALTER TABLE issues ADD COLUMN stage_id TEXT", []);

    // Teams & model selection
    let _ = conn.execute("ALTER TABLE team_agent_slots ADD COLUMN model TEXT", []);

    // Team dispatch
    let _ = conn.execute("ALTER TABLE issues ADD COLUMN role TEXT", []);
    let _ = conn.execute("ALTER TABLE teams ADD COLUMN auto_pickup_types TEXT DEFAULT '[\"task\",\"bug\",\"feature\"]'", []);
    let _ = conn.execute("ALTER TABLE teams ADD COLUMN is_active INTEGER DEFAULT 0", []);

    // Intake agent columns
    let _ = conn.execute("ALTER TABLE issues ADD COLUMN parent_id TEXT REFERENCES issues(id)", []);
    let _ = conn.execute("ALTER TABLE issues ADD COLUMN needs_intake INTEGER DEFAULT 1", []);
    let _ = conn.execute("ALTER TABLE issues ADD COLUMN scope_mode TEXT DEFAULT 'auto'", []);

    // Indexes for intake queries
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_issues_parent ON issues(parent_id)", []);
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_issues_needs_intake ON issues(needs_intake)", []);

    // Team coordination: is_lead flag for hierarchical mode
    let _ = conn.execute("ALTER TABLE team_agent_slots ADD COLUMN is_lead INTEGER NOT NULL DEFAULT 0", []);

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
    let _ = conn.execute("ALTER TABLE projects ADD COLUMN app_url TEXT", []);

    // ── Workflow Gate Approvals ──────────────────────────────────────
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS workflow_gate_approvals (
            instance_id TEXT NOT NULL,
            stage_id TEXT NOT NULL,
            approved_at TEXT NOT NULL,
            PRIMARY KEY (instance_id, stage_id)
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
