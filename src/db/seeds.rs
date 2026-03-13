use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn seed_team_templates(conn: &Connection) -> Result<(), rusqlite::Error> {
    // Ensure the __global__ project exists for template ownership
    conn.execute(
        "INSERT OR IGNORE INTO projects (id, name, directory, context) VALUES ('__global__', '__global__', '', 'work')",
        [],
    )?;

    let templates: Vec<(&str, &str, Vec<(&str, &str, Option<&str>)>)> = vec![
        ("Dev Team", "pipeline", vec![
            ("Architect", "claude", Some("claude-opus-4-6")),
            ("Coder", "claude", Some("claude-sonnet-4-6")),
            ("Reviewer", "claude", Some("claude-sonnet-4-6")),
        ]),
        ("Fix Team", "pipeline", vec![
            ("Investigator", "claude", Some("claude-sonnet-4-6")),
            ("Fixer", "claude", Some("claude-sonnet-4-6")),
            ("Tester", "claude", Some("claude-haiku-4-5-20251001")),
        ]),
        ("Research Team", "collaborative", vec![
            ("Researcher", "claude", Some("claude-opus-4-6")),
            ("Writer", "claude", Some("claude-sonnet-4-6")),
        ]),
        ("Docs Team", "pipeline", vec![
            ("Analyst", "claude", Some("claude-opus-4-6")),
            ("Documenter", "claude", Some("claude-sonnet-4-6")),
            ("Gap Reviewer", "claude", Some("claude-sonnet-4-6")),
        ]),
        ("Mixed Fleet", "swarm", vec![
            ("Claude Agent", "claude", Some("claude-sonnet-4-6")),
            ("OpenCode Agent", "opencode", None),
            ("Gemini Agent", "gemini", None),
        ]),
        ("Budget Squad", "swarm", vec![
            ("Worker 1", "claude", Some("claude-haiku-4-5-20251001")),
            ("Worker 2", "claude", Some("claude-haiku-4-5-20251001")),
            ("Worker 3", "claude", Some("claude-haiku-4-5-20251001")),
        ]),
    ];

    for (name, mode, slots) in templates {
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM teams WHERE name = ?1 AND project_id = '__global__' AND is_template = 1",
            params![name],
            |row| row.get(0),
        ).unwrap_or(false);

        if exists {
            continue;
        }

        let team_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO teams (id, name, project_id, coordination_mode, max_agents, is_template)
             VALUES (?1, ?2, '__global__', ?3, ?4, 1)",
            params![team_id, name, mode, slots.len() as i64],
        )?;

        for (order, (role, runtime, model)) in slots.into_iter().enumerate() {
            let slot_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO team_agent_slots (id, team_id, role, runtime, model, slot_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![slot_id, team_id, role, runtime, model, order as i64],
            )?;
        }

        tracing::info!("Seeded team template: {}", name);
    }

    Ok(())
}
