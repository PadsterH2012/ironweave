use std::path::Path;
use std::process::Command;
use tracing::{info, error};

use crate::db::DbPool;
use crate::config::FilesystemConfig;
use crate::models::mount::Mount;
use crate::error::{IronweaveError, Result};

pub struct MountManager {
    db: DbPool,
    config: FilesystemConfig,
}

impl MountManager {
    pub fn new(db: DbPool, config: FilesystemConfig) -> Self {
        Self { db, config }
    }

    pub fn mount(&self, mount_id: &str) -> Result<()> {
        let conn = self.db.lock().unwrap();
        let mount = Mount::get_by_id(&conn, mount_id)?;
        drop(conn);

        std::fs::create_dir_all(&mount.local_mount_point)?;

        let result = match mount.mount_type.as_str() {
            "nfs" => self.mount_nfs(&mount),
            "smb" => self.mount_smb(&mount),
            "sshfs" => self.mount_sshfs(&mount),
            other => Err(IronweaveError::Internal(format!("unknown mount type: {}", other))),
        };

        let conn = self.db.lock().unwrap();
        match &result {
            Ok(()) => {
                info!(mount_id, "mount successful");
                Mount::update_state(&conn, mount_id, "mounted", None)?;
            }
            Err(e) => {
                let err_msg = e.to_string();
                error!(mount_id, error = %err_msg, "mount failed");
                Mount::update_state(&conn, mount_id, "error", Some(&err_msg))?;
            }
        }
        result
    }

    pub fn unmount(&self, mount_id: &str) -> Result<()> {
        let conn = self.db.lock().unwrap();
        let mount = Mount::get_by_id(&conn, mount_id)?;
        drop(conn);

        let output = Command::new("sudo")
            .args(["umount", &mount.local_mount_point])
            .output()?;

        let conn = self.db.lock().unwrap();
        if output.status.success() {
            info!(mount_id, "unmount successful");
            Mount::update_state(&conn, mount_id, "unmounted", None)?;
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            error!(mount_id, error = %stderr, "unmount failed");
            Mount::update_state(&conn, mount_id, "error", Some(&stderr))?;
            Err(IronweaveError::Internal(format!("unmount failed: {}", stderr)))
        }
    }

    pub fn check_status(&self, mount_id: &str) -> Result<String> {
        let conn = self.db.lock().unwrap();
        let mount = Mount::get_by_id(&conn, mount_id)?;

        let path = Path::new(&mount.local_mount_point);
        if path.exists() && path.is_dir() {
            let output = Command::new("mountpoint")
                .arg("-q")
                .arg(&mount.local_mount_point)
                .status();

            match output {
                Ok(status) if status.success() => {
                    Mount::update_state(&conn, mount_id, "mounted", None)?;
                    Ok("mounted".to_string())
                }
                _ => {
                    Mount::update_state(&conn, mount_id, "unmounted", None)?;
                    Ok("unmounted".to_string())
                }
            }
        } else {
            Mount::update_state(&conn, mount_id, "unmounted", None)?;
            Ok("unmounted".to_string())
        }
    }

    pub fn ensure_mounted(&self, mount_id: &str) -> Result<()> {
        let status = self.check_status(mount_id)?;
        if status != "mounted" {
            self.mount(mount_id)?;
        }
        Ok(())
    }

    fn mount_nfs(&self, mount: &Mount) -> Result<()> {
        let mut args = vec!["mount".to_string(), "-t".to_string(), "nfs".to_string()];
        if let Some(opts) = &mount.mount_options {
            args.push("-o".to_string());
            args.push(opts.clone());
        }
        args.push(mount.remote_path.clone());
        args.push(mount.local_mount_point.clone());
        self.run_sudo(&args)
    }

    fn mount_smb(&self, mount: &Mount) -> Result<()> {
        let mut opts = Vec::new();
        if let Some(username) = &mount.username {
            opts.push(format!("username={}", username));
        }
        if let Some(password) = &mount.password {
            opts.push(format!("password={}", password));
        }
        if let Some(extra) = &mount.mount_options {
            opts.push(extra.clone());
        }
        let mut args = vec!["mount".to_string(), "-t".to_string(), "cifs".to_string()];
        if !opts.is_empty() {
            args.push("-o".to_string());
            args.push(opts.join(","));
        }
        args.push(mount.remote_path.clone());
        args.push(mount.local_mount_point.clone());
        self.run_sudo(&args)
    }

    fn mount_sshfs(&self, mount: &Mount) -> Result<()> {
        // sshfs with -f runs in foreground — required when using sshpass so the
        // password pipe stays connected. We spawn() instead of output() so the
        // process keeps running after we return.
        let mut sshfs_args = vec![
            mount.remote_path.clone(),
            mount.local_mount_point.clone(),
            "-f".to_string(),
            "-o".to_string(),
            "ServerAliveInterval=15,ServerAliveCountMax=3,StrictHostKeyChecking=no,reconnect".to_string(),
        ];

        // Add proxy config: always use ProxyCommand for sshfs — ProxyJump's
        // comma-separated hop syntax gets split by fuse's option parser
        if let Some(ref proxy_id) = mount.proxy_config_id {
            let conn = self.db.lock().unwrap();
            if let Ok(pc) = crate::models::proxy_config::ProxyConfig::get_by_id(&conn, proxy_id) {
                drop(conn);
                if !pc.hops.is_empty() {
                    let proxy_cmd = Self::build_proxy_command(&pc.hops);
                    sshfs_args.push("-o".to_string());
                    sshfs_args.push(format!("ProxyCommand={}", proxy_cmd));
                }
            }
        }

        if let Some(ssh_key) = &mount.ssh_key {
            sshfs_args.push("-o".to_string());
            sshfs_args.push(format!("IdentityFile={}", ssh_key));
        }
        if let Some(opts) = &mount.mount_options {
            sshfs_args.push("-o".to_string());
            sshfs_args.push(opts.clone());
        }

        // Spawn sshfs as a detached child — it runs in foreground (-f) so sshpass
        // can feed the password, but we don't wait for it to finish.
        if let Some(password) = &mount.password {
            Command::new("sshpass")
                .arg("-p").arg(password)
                .arg("sshfs")
                .args(&sshfs_args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
        } else {
            Command::new("sshfs")
                .args(&sshfs_args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
        }

        // Wait for the mount to become active (sshfs needs a moment to connect)
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let check = Command::new("mountpoint")
                .arg("-q")
                .arg(&mount.local_mount_point)
                .status();
            if let Ok(s) = check {
                if s.success() {
                    return Ok(());
                }
            }
        }
        Err(IronweaveError::Internal(
            "sshfs spawned but mountpoint not active after 10s".to_string(),
        ))
    }

    /// Build a ProxyCommand string for hops that include password auth.
    /// Key-auth hops before the last are chained with -J.
    /// The last hop uses -W %h:%p to create a tunnel to the target.
    pub fn build_proxy_command(hops: &[crate::models::proxy_config::ProxyHop]) -> String {
        if hops.is_empty() {
            return String::new();
        }

        let last = &hops[hops.len() - 1];
        let mut parts = Vec::new();

        // Wrap with sshpass if last hop uses password
        if last.auth_type == "password" {
            if let Some(ref pwd) = last.credential {
                parts.push(format!("sshpass -p {}", pwd));
            }
        }

        parts.push("ssh -o StrictHostKeyChecking=no -W %h:%p".to_string());

        // Chain all previous hops as ProxyJump
        if hops.len() > 1 {
            let jump_chain: Vec<String> = hops[..hops.len() - 1]
                .iter()
                .map(|h| format!("{}@{}:{}", h.username, h.host, h.port))
                .collect();
            parts.push(format!("-J {}", jump_chain.join(",")));
        }

        parts.push(format!("-p {}", last.port));
        parts.push(format!("{}@{}", last.username, last.host));

        parts.join(" ")
    }

    fn run_sudo(&self, args: &[String]) -> Result<()> {
        let output = Command::new("sudo").args(args).output()?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(IronweaveError::Internal(format!("mount command failed: {}", stderr)))
        }
    }
}
