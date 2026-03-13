use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{info, warn};

use crate::db::DbPool;
use crate::config::FilesystemConfig;
use crate::models::mount::Mount;
use super::manager::MountManager;

pub fn spawn_idle_monitor(
    db: DbPool,
    config: FilesystemConfig,
    mount_manager: Arc<MountManager>,
) {
    let idle_minutes = config.idle_unmount_minutes.unwrap_or(30);

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(e) = check_idle_mounts(&db, &mount_manager, idle_minutes) {
                warn!(error = %e, "idle mount check failed");
            }
        }
    });
}

fn check_idle_mounts(
    db: &DbPool,
    mount_manager: &MountManager,
    _idle_minutes: u64,
) -> crate::error::Result<()> {
    let conn = db.lock().unwrap();
    let mounts = Mount::list(&conn)?;
    drop(conn);

    for mount in mounts {
        if mount.state != "mounted" {
            continue;
        }

        let conn = db.lock().unwrap();
        // Check for active agents on projects linked by mount_id OR whose
        // directory is under this mount's local_mount_point
        let active_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_sessions a
             JOIN teams t ON a.team_id = t.id
             JOIN projects p ON t.project_id = p.id
             WHERE (p.mount_id = ?1 OR p.directory LIKE ?2)
               AND a.state IN ('working', 'idle')",
            rusqlite::params![mount.id, format!("{}%", mount.local_mount_point)],
            |row| row.get(0),
        ).unwrap_or(0);

        // Also check if any team on a project using this mount is active
        let active_teams: i64 = conn.query_row(
            "SELECT COUNT(*) FROM teams t
             JOIN projects p ON t.project_id = p.id
             WHERE (p.mount_id = ?1 OR p.directory LIKE ?2)
               AND t.is_active = 1",
            rusqlite::params![mount.id, format!("{}%", mount.local_mount_point)],
            |row| row.get(0),
        ).unwrap_or(0);
        drop(conn);

        if active_count == 0 && active_teams == 0 {
            info!(mount_id = %mount.id, mount_name = %mount.name, "unmounting idle mount");
            if let Err(e) = mount_manager.unmount(&mount.id) {
                warn!(mount_id = %mount.id, error = %e, "failed to unmount idle mount");
            }
        }
    }
    Ok(())
}
