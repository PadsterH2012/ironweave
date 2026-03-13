use std::path::Path;

pub struct DetectedApp {
    pub command: String,
    pub args: Vec<String>,
    pub port_via_env: bool,
}

fn python_cmd() -> String {
    // Prefer python3, fall back to python
    if std::process::Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        "python3".into()
    } else {
        "python".into()
    }
}

pub fn detect_app(project_dir: &Path) -> Option<DetectedApp> {
    let python = python_cmd();

    // Flask app — use `python3 -m flask run` which respects FLASK_RUN_PORT/FLASK_RUN_HOST
    if project_dir.join("app.py").exists() && file_contains(project_dir, "app.py", "Flask") {
        return Some(DetectedApp {
            command: python.clone(),
            args: vec!["-m".into(), "flask".into(), "run".into()],
            port_via_env: true,
        });
    }

    if project_dir.join("main.py").exists() && file_contains(project_dir, "main.py", "Flask") {
        return Some(DetectedApp {
            command: python.clone(),
            args: vec!["-m".into(), "flask".into(), "run".into()],
            port_via_env: true,
        });
    }

    // Django
    if project_dir.join("manage.py").exists() {
        return Some(DetectedApp {
            command: "python".into(),
            args: vec!["manage.py".into(), "runserver".into()],
            port_via_env: false,
        });
    }

    // Node.js
    if project_dir.join("package.json").exists() {
        if file_contains(project_dir, "package.json", "\"start\"") {
            return Some(DetectedApp {
                command: "npm".into(),
                args: vec!["start".into()],
                port_via_env: true,
            });
        }
    }

    // Rust
    if project_dir.join("Cargo.toml").exists() {
        return Some(DetectedApp {
            command: "cargo".into(),
            args: vec!["run".into()],
            port_via_env: true,
        });
    }

    // Go
    if project_dir.join("go.mod").exists() {
        return Some(DetectedApp {
            command: "go".into(),
            args: vec!["run".into(), ".".into()],
            port_via_env: true,
        });
    }

    // Static site
    if project_dir.join("index.html").exists() {
        return Some(DetectedApp {
            command: "python".into(),
            args: vec!["-m".into(), "http.server".into()],
            port_via_env: false,
        });
    }

    None
}

fn file_contains(dir: &Path, filename: &str, needle: &str) -> bool {
    std::fs::read_to_string(dir.join(filename))
        .map(|content| content.contains(needle))
        .unwrap_or(false)
}
