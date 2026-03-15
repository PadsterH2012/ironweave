use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub project_id: String,
    pub node_type: String,
    pub path: String,
    pub name: String,
    pub language: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub complexity_score: i32,
    pub last_indexed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub project_id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub edge_type: String,
    pub weight: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateGraphNode {
    #[serde(default)]
    pub project_id: String,
    pub node_type: String,
    pub path: String,
    pub name: String,
    pub language: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGraphEdge {
    pub source_node_id: String,
    pub target_node_id: String,
    pub edge_type: String,
    pub weight: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct FileComplexity {
    pub path: String,
    pub complexity_score: i32,
    pub incoming_edges: i64,
    pub outgoing_edges: i64,
    pub total_connections: i64,
}

#[derive(Debug, Serialize)]
pub struct BlastRadius {
    pub file_path: String,
    pub directly_affected: Vec<String>,
    pub indirectly_affected: Vec<String>,
    pub total_affected: i64,
}

impl GraphNode {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            node_type: row.get("node_type")?,
            path: row.get("path")?,
            name: row.get("name")?,
            language: row.get("language")?,
            line_start: row.get("line_start")?,
            line_end: row.get("line_end")?,
            complexity_score: row.get("complexity_score")?,
            last_indexed_at: row.get("last_indexed_at")?,
        })
    }

    pub fn upsert(conn: &Connection, input: &CreateGraphNode) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO code_graph_nodes (id, project_id, node_type, path, name, language, line_start, line_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(project_id, path, name, node_type)
             DO UPDATE SET language = ?6, line_start = ?7, line_end = ?8, last_indexed_at = datetime('now')",
            params![id, input.project_id, input.node_type, input.path, input.name, input.language, input.line_start, input.line_end],
        )?;
        let node = conn.query_row(
            "SELECT * FROM code_graph_nodes WHERE project_id = ?1 AND path = ?2 AND name = ?3 AND node_type = ?4",
            params![input.project_id, input.path, input.name, input.node_type],
            Self::from_row,
        )?;
        Ok(node)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        let node = conn.query_row(
            "SELECT * FROM code_graph_nodes WHERE id = ?1",
            params![id],
            Self::from_row,
        )?;
        Ok(node)
    }

    pub fn list_by_project(conn: &Connection, project_id: &str, node_type: Option<&str>) -> Result<Vec<Self>> {
        let (sql, param_values): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match node_type {
            Some(nt) => (
                "SELECT * FROM code_graph_nodes WHERE project_id = ?1 AND node_type = ?2 ORDER BY path, name".to_string(),
                vec![Box::new(project_id.to_string()), Box::new(nt.to_string())],
            ),
            None => (
                "SELECT * FROM code_graph_nodes WHERE project_id = ?1 ORDER BY path, name".to_string(),
                vec![Box::new(project_id.to_string())],
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Compute complexity scores based on connectivity (in-degree + out-degree)
    pub fn recompute_complexity(conn: &Connection, project_id: &str) -> Result<()> {
        conn.execute(
            "UPDATE code_graph_nodes SET complexity_score = (
                SELECT COALESCE(
                    (SELECT COUNT(*) FROM code_graph_edges WHERE source_node_id = code_graph_nodes.id) +
                    (SELECT COUNT(*) FROM code_graph_edges WHERE target_node_id = code_graph_nodes.id),
                    1
                )
            ) WHERE project_id = ?1",
            params![project_id],
        )?;
        Ok(())
    }

    /// Get file-level complexity ranking
    pub fn file_complexity(conn: &Connection, project_id: &str) -> Result<Vec<FileComplexity>> {
        let mut stmt = conn.prepare(
            "SELECT n.path, n.complexity_score,
                    (SELECT COUNT(*) FROM code_graph_edges e WHERE e.target_node_id = n.id) as incoming,
                    (SELECT COUNT(*) FROM code_graph_edges e WHERE e.source_node_id = n.id) as outgoing
             FROM code_graph_nodes n
             WHERE n.project_id = ?1 AND n.node_type = 'file'
             ORDER BY n.complexity_score DESC"
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let incoming: i64 = row.get(2)?;
            let outgoing: i64 = row.get(3)?;
            Ok(FileComplexity {
                path: row.get(0)?,
                complexity_score: row.get(1)?,
                incoming_edges: incoming,
                outgoing_edges: outgoing,
                total_connections: incoming + outgoing,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Calculate blast radius — which files are affected if this file changes
    pub fn blast_radius(conn: &Connection, project_id: &str, file_path: &str) -> Result<BlastRadius> {
        // Direct dependents (files that import/reference this file)
        let mut stmt = conn.prepare(
            "SELECT DISTINCT n2.path FROM code_graph_nodes n1
             JOIN code_graph_edges e ON e.target_node_id = n1.id
             JOIN code_graph_nodes n2 ON e.source_node_id = n2.id
             WHERE n1.project_id = ?1 AND n1.path = ?2 AND n1.node_type = 'file'
               AND n2.node_type = 'file' AND n2.path != ?2"
        )?;
        let direct: Vec<String> = stmt.query_map(params![project_id, file_path], |row| {
            row.get(0)
        })?.filter_map(|r| r.ok()).collect();

        // Indirect dependents (2nd-degree)
        let mut indirect = Vec::new();
        for dep in &direct {
            let mut stmt2 = conn.prepare(
                "SELECT DISTINCT n2.path FROM code_graph_nodes n1
                 JOIN code_graph_edges e ON e.target_node_id = n1.id
                 JOIN code_graph_nodes n2 ON e.source_node_id = n2.id
                 WHERE n1.project_id = ?1 AND n1.path = ?2 AND n1.node_type = 'file'
                   AND n2.node_type = 'file' AND n2.path != ?2 AND n2.path != ?3"
            )?;
            let deps: Vec<String> = stmt2.query_map(params![project_id, dep, file_path], |row| {
                row.get(0)
            })?.filter_map(|r| r.ok()).collect();
            for d in deps {
                if !direct.contains(&d) && !indirect.contains(&d) {
                    indirect.push(d);
                }
            }
        }

        let total = (direct.len() + indirect.len()) as i64;
        Ok(BlastRadius {
            file_path: file_path.to_string(),
            directly_affected: direct,
            indirectly_affected: indirect,
            total_affected: total,
        })
    }

    /// Delete all graph data for a project (for re-indexing)
    pub fn clear_project(conn: &Connection, project_id: &str) -> Result<()> {
        conn.execute("DELETE FROM code_graph_edges WHERE project_id = ?1", params![project_id])?;
        conn.execute("DELETE FROM code_graph_nodes WHERE project_id = ?1", params![project_id])?;
        Ok(())
    }
}

impl GraphEdge {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            source_node_id: row.get("source_node_id")?,
            target_node_id: row.get("target_node_id")?,
            edge_type: row.get("edge_type")?,
            weight: row.get("weight")?,
        })
    }

    pub fn upsert(conn: &Connection, project_id: &str, input: &CreateGraphEdge) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let weight = input.weight.unwrap_or(1);
        conn.execute(
            "INSERT INTO code_graph_edges (id, project_id, source_node_id, target_node_id, edge_type, weight)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(source_node_id, target_node_id, edge_type)
             DO UPDATE SET weight = ?6",
            params![id, project_id, input.source_node_id, input.target_node_id, input.edge_type, weight],
        )?;
        let edge = conn.query_row(
            "SELECT * FROM code_graph_edges WHERE source_node_id = ?1 AND target_node_id = ?2 AND edge_type = ?3",
            params![input.source_node_id, input.target_node_id, input.edge_type],
            Self::from_row,
        )?;
        Ok(edge)
    }

    pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM code_graph_edges WHERE project_id = ?1"
        )?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'Test', '/tmp', 'homelab')",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn test_upsert_nodes_and_edges() {
        let conn = setup_db();

        let n1 = GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(),
            node_type: "file".to_string(),
            path: "src/main.rs".to_string(),
            name: "main.rs".to_string(),
            language: Some("rust".to_string()),
            line_start: None,
            line_end: None,
        }).unwrap();

        let n2 = GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(),
            node_type: "file".to_string(),
            path: "src/lib.rs".to_string(),
            name: "lib.rs".to_string(),
            language: Some("rust".to_string()),
            line_start: None,
            line_end: None,
        }).unwrap();

        let edge = GraphEdge::upsert(&conn, "p1", &CreateGraphEdge {
            source_node_id: n1.id.clone(),
            target_node_id: n2.id.clone(),
            edge_type: "imports".to_string(),
            weight: None,
        }).unwrap();

        assert_eq!(edge.edge_type, "imports");

        let nodes = GraphNode::list_by_project(&conn, "p1", Some("file")).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_complexity_scoring() {
        let conn = setup_db();

        let n1 = GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(),
            node_type: "file".to_string(),
            path: "src/main.rs".to_string(),
            name: "main.rs".to_string(),
            language: None, line_start: None, line_end: None,
        }).unwrap();
        let n2 = GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(),
            node_type: "file".to_string(),
            path: "src/lib.rs".to_string(),
            name: "lib.rs".to_string(),
            language: None, line_start: None, line_end: None,
        }).unwrap();
        let n3 = GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(),
            node_type: "file".to_string(),
            path: "src/utils.rs".to_string(),
            name: "utils.rs".to_string(),
            language: None, line_start: None, line_end: None,
        }).unwrap();

        // main imports lib and utils; lib imports utils
        GraphEdge::upsert(&conn, "p1", &CreateGraphEdge {
            source_node_id: n1.id.clone(), target_node_id: n2.id.clone(),
            edge_type: "imports".to_string(), weight: None,
        }).unwrap();
        GraphEdge::upsert(&conn, "p1", &CreateGraphEdge {
            source_node_id: n1.id.clone(), target_node_id: n3.id.clone(),
            edge_type: "imports".to_string(), weight: None,
        }).unwrap();
        GraphEdge::upsert(&conn, "p1", &CreateGraphEdge {
            source_node_id: n2.id.clone(), target_node_id: n3.id.clone(),
            edge_type: "imports".to_string(), weight: None,
        }).unwrap();

        GraphNode::recompute_complexity(&conn, "p1").unwrap();

        let complexity = GraphNode::file_complexity(&conn, "p1").unwrap();
        // utils.rs should be most complex (2 incoming)
        // main.rs has 2 outgoing
        assert_eq!(complexity.len(), 3);
    }

    #[test]
    fn test_blast_radius() {
        let conn = setup_db();

        let n1 = GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(), node_type: "file".to_string(),
            path: "src/core.rs".to_string(), name: "core.rs".to_string(),
            language: None, line_start: None, line_end: None,
        }).unwrap();
        let n2 = GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(), node_type: "file".to_string(),
            path: "src/api.rs".to_string(), name: "api.rs".to_string(),
            language: None, line_start: None, line_end: None,
        }).unwrap();
        let n3 = GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(), node_type: "file".to_string(),
            path: "src/main.rs".to_string(), name: "main.rs".to_string(),
            language: None, line_start: None, line_end: None,
        }).unwrap();

        // api imports core, main imports api
        GraphEdge::upsert(&conn, "p1", &CreateGraphEdge {
            source_node_id: n2.id.clone(), target_node_id: n1.id.clone(),
            edge_type: "imports".to_string(), weight: None,
        }).unwrap();
        GraphEdge::upsert(&conn, "p1", &CreateGraphEdge {
            source_node_id: n3.id.clone(), target_node_id: n2.id.clone(),
            edge_type: "imports".to_string(), weight: None,
        }).unwrap();

        let blast = GraphNode::blast_radius(&conn, "p1", "src/core.rs").unwrap();
        assert_eq!(blast.directly_affected.len(), 1); // api.rs
        assert_eq!(blast.indirectly_affected.len(), 1); // main.rs
        assert_eq!(blast.total_affected, 2);
    }

    #[test]
    fn test_clear_project() {
        let conn = setup_db();
        GraphNode::upsert(&conn, &CreateGraphNode {
            project_id: "p1".to_string(), node_type: "file".to_string(),
            path: "src/main.rs".to_string(), name: "main.rs".to_string(),
            language: None, line_start: None, line_end: None,
        }).unwrap();

        GraphNode::clear_project(&conn, "p1").unwrap();
        let nodes = GraphNode::list_by_project(&conn, "p1", None).unwrap();
        assert_eq!(nodes.len(), 0);
    }
}
