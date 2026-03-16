#!/usr/bin/env python3
"""
Feature Checklist Auditor for Ironweave
Compares codebase against docs/BACKEND_FEATURES.md and docs/FRONTEND_FEATURES.md.
- Adds new items found in code but not in checklists
- Logs missing items (in checklist but gone from code) to Obsidian snag file
- Copies updated checklists to Obsidian vault
"""

import os
import re
import glob
import subprocess
from datetime import datetime
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
BACKEND_CHECKLIST = PROJECT_ROOT / "docs" / "BACKEND_FEATURES.md"
FRONTEND_CHECKLIST = PROJECT_ROOT / "docs" / "FRONTEND_FEATURES.md"
OBSIDIAN_BASE = Path("/Volumes/Breakaway/obsidian/Homelab/Projects/A1 - Main Projects/Ironweave")
OBSIDIAN_FEATURES = OBSIDIAN_BASE / "features"
OBSIDIAN_SNAGS = OBSIDIAN_BASE / "snags"
SNAG_FILE = OBSIDIAN_SNAGS / "Checklist Snags.md"


def read_file(path):
    with open(path, "r") as f:
        return f.read()


def extract_checklist_items(content, pattern):
    """Extract items matching a regex from checklist markdown."""
    return set(re.findall(pattern, content))


# ── Backend Auditors ──────────────────────────────────────────────

def get_actual_routes():
    """Parse src/main.rs for all registered API routes."""
    main_rs = read_file(PROJECT_ROOT / "src" / "main.rs")
    routes = set()
    # Match .route("/api/...", ...) patterns
    for m in re.finditer(r'\.route\("([^"]+)"', main_rs):
        routes.add(m.group(1))
    return routes


def get_checklist_routes(content):
    """Extract routes from backend checklist."""
    routes = set()
    for m in re.finditer(r'`((?:GET|POST|PUT|PATCH|DELETE)\s+(/[^`]+))`', content):
        routes.add(m.group(2))
    return routes


def get_actual_models():
    """List all model files in src/models/."""
    models_dir = PROJECT_ROOT / "src" / "models"
    return {f.stem for f in models_dir.glob("*.rs") if f.stem != "mod"}


def get_checklist_models(content):
    """Extract model filenames from the Database Models section only."""
    models = set()
    # Find the Database Models section
    section = re.search(r'## Database Models \(src/models/\)(.*?)(?:\n---|\n## )', content, re.DOTALL)
    if section:
        for m in re.finditer(r'`(\w+)\.rs`\s*—', section.group(1)):
            models.add(m.group(1))
    return models


def get_actual_api_modules():
    """List all API handler files."""
    api_dir = PROJECT_ROOT / "src" / "api"
    return {f.stem for f in api_dir.glob("*.rs") if f.stem != "mod"}


def get_actual_orchestrator_files():
    """List orchestrator files."""
    orch_dir = PROJECT_ROOT / "src" / "orchestrator"
    return {f.stem for f in orch_dir.glob("*.rs")}


def get_actual_runtime_files():
    """List runtime adapter files."""
    runtime_dir = PROJECT_ROOT / "src" / "runtime"
    return {f.stem for f in runtime_dir.glob("*.rs") if f.stem != "mod"}


# ── Frontend Auditors ─────────────────────────────────────────────

def get_actual_routes_frontend():
    """Parse App.svelte for route definitions."""
    app_svelte = read_file(PROJECT_ROOT / "frontend" / "src" / "App.svelte")
    routes = set()
    for m in re.finditer(r"'(/[^']*)':\s*(\w+)", app_svelte):
        routes.add(m.group(1))
    return routes


def get_actual_components():
    """List all component .svelte files."""
    comp_dir = PROJECT_ROOT / "frontend" / "src" / "lib" / "components"
    return {f.stem for f in comp_dir.glob("*.svelte")}


def get_actual_route_files():
    """List all route .svelte files."""
    routes_dir = PROJECT_ROOT / "frontend" / "src" / "routes"
    return {f.stem for f in routes_dir.glob("*.svelte")}


def get_checklist_components(content):
    """Extract component names from frontend checklist."""
    components = set()
    for m in re.finditer(r'###\s+(\w+)\.svelte', content):
        components.add(m.group(1))
    # Also match "ComponentName.svelte" in checklist items
    for m in re.finditer(r'`<(\w+)\s*/>`', content):
        components.add(m.group(1))
    return components


def get_actual_api_objects():
    """Parse api.ts for exported const objects."""
    api_ts = read_file(PROJECT_ROOT / "frontend" / "src" / "lib" / "api.ts")
    objects = set()
    for m in re.finditer(r'export const (\w+)\s*[=:{]', api_ts):
        objects.add(m.group(1))
    return objects


def get_actual_interfaces():
    """Parse api.ts for exported interfaces."""
    api_ts = read_file(PROJECT_ROOT / "frontend" / "src" / "lib" / "api.ts")
    interfaces = set()
    for m in re.finditer(r'export interface (\w+)', api_ts):
        interfaces.add(m.group(1))
    return interfaces


def get_checklist_interfaces(content):
    """Extract interface names from the TypeScript Interfaces section only."""
    interfaces = set()
    # Only parse the TypeScript Interfaces section
    section = re.search(r'### TypeScript Interfaces(.*?)(?:\n### |\n---|\n## |\Z)', content, re.DOTALL)
    if section:
        for m in re.finditer(r'`([A-Z]\w+)`', section.group(1)):
            interfaces.add(m.group(1))
    return interfaces


# ── Checklist Updater ─────────────────────────────────────────────

def add_to_section(content, section_header, new_items, format_fn):
    """Add new checklist items before the next section break."""
    if not new_items:
        return content

    # Find the section
    pattern = re.escape(section_header)
    match = re.search(pattern, content)
    if not match:
        return content

    # Find the next --- or ## after this section
    rest = content[match.end():]
    next_break = re.search(r'\n---|\n## ', rest)
    if next_break:
        insert_pos = match.end() + next_break.start()
    else:
        insert_pos = len(content)

    # Build new items
    new_lines = "\n".join(format_fn(item) for item in sorted(new_items))
    new_content = content[:insert_pos] + "\n" + new_lines + "\n" + content[insert_pos:]
    return new_content


# ── Snag Reporter ─────────────────────────────────────────────────

def report_snags(snags):
    """Write missing items to Obsidian snag file."""
    if not snags:
        return

    OBSIDIAN_SNAGS.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M")
    header = f"\n## Audit — {timestamp}\n\n"

    entries = []
    for category, items in snags.items():
        if items:
            entries.append(f"### {category}\n")
            for item in sorted(items):
                entries.append(f"- [ ] **MISSING**: `{item}`")
            entries.append("")

    if not entries:
        return

    new_content = header + "\n".join(entries) + "\n"

    if SNAG_FILE.exists():
        existing = read_file(SNAG_FILE)
    else:
        existing = "# Checklist Snags\n\nFeatures that were in the checklist but are no longer found in the codebase.\nInvestigate whether they were intentionally removed or accidentally deleted.\n"

    with open(SNAG_FILE, "w") as f:
        f.write(existing + new_content)

    print(f"  Snags written to {SNAG_FILE}")


# ── Main ──────────────────────────────────────────────────────────

def audit_backend():
    print("Auditing backend...")
    content = read_file(BACKEND_CHECKLIST)
    snags = {}
    changes = False

    # 1. Routes
    actual_routes = get_actual_routes()
    checklist_routes = get_checklist_routes(content)

    # Normalize: strip {param} variations for comparison
    def normalize_route(r):
        return re.sub(r'\{[^}]+\}', '{_}', r)

    actual_norm = {normalize_route(r): r for r in actual_routes}
    checklist_norm = {normalize_route(r): r for r in checklist_routes}

    new_routes = set(actual_norm.keys()) - set(checklist_norm.keys())
    missing_routes = set(checklist_norm.keys()) - set(actual_norm.keys())

    if new_routes:
        print(f"  New routes found: {len(new_routes)}")
        for nr in sorted(new_routes):
            print(f"    + {actual_norm[nr]}")
        changes = True

    if missing_routes:
        snags["API Routes"] = {checklist_norm[r] for r in missing_routes}
        print(f"  Missing routes: {len(missing_routes)}")
        for mr in sorted(missing_routes):
            print(f"    - {checklist_norm[mr]}")

    # 2. Models
    actual_models = get_actual_models()
    checklist_models = get_checklist_models(content)
    new_models = actual_models - checklist_models
    missing_models = checklist_models - actual_models

    if new_models:
        print(f"  New models found: {new_models}")
        changes = True
    if missing_models:
        snags["Database Models"] = {f"{m}.rs" for m in missing_models}
        print(f"  Missing models: {missing_models}")

    # 3. API modules
    actual_api = get_actual_api_modules()
    # Just check for completely new API module files
    known_api_sections = set()
    for m in re.finditer(r'src/api/(\w+)\.rs', content):
        known_api_sections.add(m.group(1))
    new_api = actual_api - known_api_sections
    missing_api = known_api_sections - actual_api

    if new_api:
        print(f"  New API modules: {new_api}")
        changes = True
    if missing_api:
        snags["API Modules"] = {f"{a}.rs" for a in missing_api}
        print(f"  Missing API modules: {missing_api}")

    # 4. Runtime adapters
    actual_rt = get_actual_runtime_files()
    known_rt = set()
    for m in re.finditer(r'`(\w+)\.rs`\s*—\s*\w+.*adapter', content, re.IGNORECASE):
        known_rt.add(m.group(1))
    # Also grab from the runtime section
    for m in re.finditer(r'src/runtime/\{?(\w+)', content):
        if m.group(1) not in ('mod', 'adapter'):
            known_rt.add(m.group(1))
    new_rt = actual_rt - known_rt - {'adapter', 'mod'}
    if new_rt:
        print(f"  New runtime adapters: {new_rt}")
        changes = True

    # Update checklist timestamp if changes
    if changes:
        content = re.sub(
            r'Last updated: \d{4}-\d{2}-\d{2}',
            f'Last updated: {datetime.now().strftime("%Y-%m-%d")}',
            content
        )
        with open(BACKEND_CHECKLIST, "w") as f:
            f.write(content)
        print("  Backend checklist updated.")

    return snags


def audit_frontend():
    print("Auditing frontend...")
    content = read_file(FRONTEND_CHECKLIST)
    snags = {}
    changes = False

    # 1. Route files — only check the Routes section
    actual_routes = get_actual_route_files()
    routes_section = re.search(r'## Routes \(frontend/src/routes/\)(.*?)(?:\n---|\n## )', content, re.DOTALL)
    known_routes = set()
    if routes_section:
        for m in re.finditer(r'(\w+)\.svelte', routes_section.group(1)):
            known_routes.add(m.group(1))
    # Exclude non-route files that live in routes/ but are layout wrappers
    route_skiplist = {'Settings'}  # redirect wrapper, not a real route page
    new_routes = actual_routes - known_routes - route_skiplist
    missing_routes = known_routes - actual_routes

    if new_routes:
        print(f"  New route files: {new_routes}")
        changes = True
    if missing_routes:
        snags["Route Files"] = {f"{r}.svelte" for r in missing_routes}
        print(f"  Missing route files: {missing_routes}")

    # 2. Components
    actual_comps = get_actual_components()
    known_comps = set()
    for m in re.finditer(r'###\s+(\w+)\.svelte', content):
        known_comps.add(m.group(1))
    new_comps = actual_comps - known_comps
    missing_comps = known_comps - actual_comps

    if new_comps:
        print(f"  New components: {new_comps}")
        changes = True
    if missing_comps:
        snags["Components"] = {f"{c}.svelte" for c in missing_comps}
        print(f"  Missing components: {missing_comps}")

    # 3. API objects
    actual_api = get_actual_api_objects()
    known_api = set()
    for m in re.finditer(r'`(\w+)`\s*—\s*\w+', content):
        known_api.add(m.group(1))
    # Broader: check API Objects section
    api_section = re.search(r'### API Objects(.*?)###', content, re.DOTALL)
    if api_section:
        for m in re.finditer(r'`(\w+)`', api_section.group(1)):
            known_api.add(m.group(1))

    # Filter to actual export names (exclude helpers and constants)
    skip = {'get', 'post', 'put', 'patch', 'del', 'authHeaders', 'RUNTIME_MODELS', 'PREDEFINED_ROLES', 'BASE'}
    actual_api_filtered = actual_api - skip
    # Also add nested object parents that appear as "parent.child" in checklist
    for m in re.finditer(r'`(\w+)\.(?:definitions|instances|slots|attachments|schedules)`', content):
        known_api.add(m.group(1))
    new_api = actual_api_filtered - known_api
    missing_api_objs = known_api & actual_api_filtered  # only flag if was in known but gone
    gone_api = known_api - actual_api_filtered - skip

    if new_api:
        print(f"  New API objects: {new_api}")
        changes = True
    if gone_api:
        snags["API Client Objects"] = gone_api
        print(f"  Missing API objects: {gone_api}")

    # 4. Interfaces (use the section-aware parser)
    actual_ifaces = get_actual_interfaces()
    checklist_ifaces = get_checklist_interfaces(content)

    new_ifaces = actual_ifaces - checklist_ifaces
    missing_ifaces = checklist_ifaces - actual_ifaces

    if new_ifaces:
        print(f"  New interfaces: {new_ifaces}")
        changes = True
    if missing_ifaces:
        snags["TypeScript Interfaces"] = missing_ifaces
        print(f"  Missing interfaces: {missing_ifaces}")

    # Update timestamp
    if changes:
        content = re.sub(
            r'Last updated: \d{4}-\d{2}-\d{2}',
            f'Last updated: {datetime.now().strftime("%Y-%m-%d")}',
            content
        )
        with open(FRONTEND_CHECKLIST, "w") as f:
            f.write(content)
        print("  Frontend checklist updated.")

    return snags


def sync_to_obsidian():
    """Copy updated checklists to Obsidian vault."""
    if not OBSIDIAN_BASE.exists():
        print("  Obsidian vault not accessible (drive not mounted?)")
        return False

    OBSIDIAN_FEATURES.mkdir(parents=True, exist_ok=True)

    import shutil
    shutil.copy2(BACKEND_CHECKLIST, OBSIDIAN_FEATURES / "Backend Features Checklist.md")
    shutil.copy2(FRONTEND_CHECKLIST, OBSIDIAN_FEATURES / "Frontend Features Checklist.md")
    print("  Synced checklists to Obsidian.")
    return True


def main():
    print(f"=== Ironweave Feature Audit — {datetime.now().strftime('%Y-%m-%d %H:%M')} ===\n")

    all_snags = {}

    backend_snags = audit_backend()
    all_snags.update(backend_snags)

    print()

    frontend_snags = audit_frontend()
    all_snags.update(frontend_snags)

    print()

    # Report snags to Obsidian
    if any(v for v in all_snags.values()):
        report_snags(all_snags)
    else:
        print("  No missing features detected.")

    # Sync checklists to Obsidian
    sync_to_obsidian()

    print("\nDone.")


if __name__ == "__main__":
    main()
