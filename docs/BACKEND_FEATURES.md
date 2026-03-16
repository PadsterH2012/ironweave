# Backend Feature Checklist

> **Living document** — update when features are added, modified, or removed.
> Use this to verify nothing has been accidentally deleted during development.

Last updated: 2026-03-16

---

## Modules (src/main.rs)

- [ ] `mod api`
- [ ] `mod auth`
- [ ] `mod config`
- [ ] `mod db`
- [ ] `mod error`
- [ ] `mod models`
- [ ] `mod orchestrator`
- [ ] `mod process`
- [ ] `mod runtime`
- [ ] `mod state`
- [ ] `mod worktree`
- [ ] `mod mount`
- [ ] `mod sync`
- [ ] `mod app_runner`

---

## API Endpoints (src/api/)

### Health
- [ ] `GET /api/health` — inline handler

### Auth (src/auth/mod.rs)
- [ ] `POST /api/auth/login` — `auth::login`
- [ ] `POST /api/auth/logout` — `auth::logout`
- [ ] Auth middleware (`auth::auth_middleware`) — conditional on config

### Projects (src/api/projects.rs)
- [ ] `GET /api/projects` — `projects::list`
- [ ] `POST /api/projects` — `projects::create`
- [ ] `GET /api/projects/{id}` — `projects::get`
- [ ] `PUT /api/projects/{id}` — `projects::update`
- [ ] `DELETE /api/projects/{id}` — `projects::delete`

### Teams (src/api/teams.rs)
- [ ] `GET /api/teams/templates` — `teams::list_templates`
- [ ] `GET /api/projects/{pid}/teams/templates` — `teams::list_project_templates`
- [ ] `POST /api/projects/{pid}/teams/from-template/{tid}` — `teams::clone_template`
- [ ] `GET /api/projects/{pid}/teams` — `teams::list`
- [ ] `POST /api/projects/{pid}/teams` — `teams::create`
- [ ] `GET /api/projects/{pid}/teams/{id}` — `teams::get`
- [ ] `DELETE /api/projects/{pid}/teams/{id}` — `teams::delete`
- [ ] `PUT /api/projects/{pid}/teams/{id}/activate` — `teams::activate`
- [ ] `PUT /api/projects/{pid}/teams/{id}/deactivate` — `teams::deactivate`
- [ ] `PUT /api/projects/{pid}/teams/{id}/config` — `teams::update_config`
- [ ] `GET /api/projects/{pid}/teams/{id}/status` — `teams::team_status`

### Team Agent Slots (src/api/teams.rs)
- [ ] `GET /api/teams/{tid}/slots` — `teams::list_slots`
- [ ] `POST /api/teams/{tid}/slots` — `teams::create_slot`
- [ ] `PUT /api/teams/{tid}/slots/{id}` — `teams::update_slot`
- [ ] `DELETE /api/teams/{tid}/slots/{id}` — `teams::delete_slot`

### Issues (src/api/issues.rs)
- [ ] `GET /api/projects/{pid}/issues` — `issues::list`
- [ ] `POST /api/projects/{pid}/issues` — `issues::create`
- [ ] `GET /api/projects/{pid}/issues/ready` — `issues::ready`
- [ ] `GET /api/projects/{pid}/issues/{id}` — `issues::get`
- [ ] `PATCH /api/projects/{pid}/issues/{id}` — `issues::update`
- [ ] `DELETE /api/projects/{pid}/issues/{id}` — `issues::delete`
- [ ] `POST /api/projects/{pid}/issues/{id}/claim` — `issues::claim`
- [ ] `GET /api/projects/{pid}/issues/{id}/children` — `issues::children`
- [ ] `POST /api/projects/{pid}/issues/{id}/unclaim` — `issues::unclaim`

### Attachments (src/api/attachments.rs)
- [ ] `GET /api/projects/{pid}/issues/{id}/attachments` — `attachments::list`
- [ ] `POST /api/projects/{pid}/issues/{id}/attachments` — `attachments::upload`
- [ ] `GET /api/attachments/{id}/download` — `attachments::download`

### Agents (src/api/agents.rs)
- [ ] `GET /api/agents` — `agents::list`
- [ ] `POST /api/agents/spawn` — `agents::spawn`
- [ ] `DELETE /api/agents/dead` — `agents::delete_dead`
- [ ] `GET /api/agents/{id}` — `agents::get_agent`
- [ ] `POST /api/agents/{id}/stop` — `agents::stop`
- [ ] `GET /ws/agents/{id}` — `agents::ws_agent_output` (WebSocket)

### Workflows (src/api/workflows.rs)
- [ ] `GET /api/projects/{pid}/workflows` — `workflows::list_definitions`
- [ ] `POST /api/projects/{pid}/workflows` — `workflows::create_definition`
- [ ] `GET /api/projects/{pid}/workflows/{id}` — `workflows::get_definition`
- [ ] `GET /api/workflows/{wid}/instances` — `workflows::list_instances`
- [ ] `POST /api/workflows/{wid}/instances` — `workflows::create_instance`
- [ ] `POST /api/workflows/{wid}/instances/{iid}/pause` — `workflows::pause_instance`
- [ ] `POST /api/workflows/{wid}/instances/{iid}/resume` — `workflows::resume_instance`
- [ ] `POST /api/workflows/{wid}/instances/{iid}/cancel` — `workflows::cancel_instance`
- [ ] `POST /api/workflows/{wid}/instances/{iid}/stages/{sid}/approve` — `workflows::approve_gate`

### Dashboard (src/api/dashboard.rs)
- [ ] `GET /api/dashboard` — `dashboard::stats`
- [ ] `GET /api/dashboard/activity` — `dashboard::activity`
- [ ] `GET /api/dashboard/metrics` — `dashboard::metrics`
- [ ] `GET /api/dashboard/system` — `dashboard::system`

### Filesystem (src/api/filesystem.rs)
- [ ] `GET /api/filesystem/browse` — `filesystem::browse`

### Mounts (src/api/mounts.rs)
- [ ] `GET /api/mounts` — `mounts::list`
- [ ] `POST /api/mounts` — `mounts::create`
- [ ] `GET /api/mounts/{id}` — `mounts::get`
- [ ] `PUT /api/mounts/{id}` — `mounts::update`
- [ ] `DELETE /api/mounts/{id}` — `mounts::delete`
- [ ] `POST /api/mounts/{id}/mount` — `mounts::mount_action`
- [ ] `POST /api/mounts/{id}/unmount` — `mounts::unmount_action`
- [ ] `GET /api/mounts/{id}/status` — `mounts::status`
- [ ] `POST /api/mounts/{id}/duplicate` — `mounts::duplicate`
- [ ] `POST /api/mounts/test-ssh` — `mounts::test_ssh`
- [ ] `POST /api/mounts/browse-remote` — `mounts::browse_remote`

### Settings (src/api/settings.rs)
- [ ] `GET /api/settings` — `settings::list`
- [ ] `GET /api/settings/{key}` — `settings::get`
- [ ] `PUT /api/settings/{key}` — `settings::upsert`
- [ ] `DELETE /api/settings/{key}` — `settings::delete`

### Proxy Configs (src/api/proxy_configs.rs)
- [ ] `GET /api/proxy-configs` — `proxy_configs::list`
- [ ] `POST /api/proxy-configs` — `proxy_configs::create`
- [ ] `GET /api/proxy-configs/{id}` — `proxy_configs::get`
- [ ] `PUT /api/proxy-configs/{id}` — `proxy_configs::update`
- [ ] `DELETE /api/proxy-configs/{id}` — `proxy_configs::delete`
- [ ] `POST /api/proxy-configs/{id}/test` — `proxy_configs::test_connection`

### Project Sync (src/api/sync.rs)
- [ ] `POST /api/projects/{id}/sync` — `sync::trigger_sync`
- [ ] `GET /api/projects/{id}/sync/status` — `sync::get_status`
- [ ] `GET /api/projects/{id}/sync/history` — `sync::get_history`
- [ ] `GET /api/projects/{id}/sync/diff/{change_id}` — `sync::get_diff`
- [ ] `POST /api/projects/{id}/sync/restore` — `sync::restore`
- [ ] `GET /api/projects/{id}/files` — `sync::browse_files`
- [ ] `GET /api/projects/{id}/files/content` — `sync::read_file`

### Project App Preview (src/api/project_apps.rs)
- [ ] `POST /api/projects/{id}/app/start` — `project_apps::start`
- [ ] `POST /api/projects/{id}/app/stop` — `project_apps::stop`
- [ ] `GET /api/projects/{id}/app/status` — `project_apps::status`

### Plan Import (src/api/plan_import.rs)
- [ ] `POST /api/projects/{pid}/import-plan` — `plan_import::import_plan`

### Merge Queue (src/api/merge_queue.rs)
- [ ] `GET /api/projects/{pid}/merge-queue` — `merge_queue::list_queue`
- [ ] `POST /api/projects/{pid}/merge-queue/{id}/approve` — `merge_queue::approve_merge`
- [ ] `POST /api/projects/{pid}/merge-queue/{id}/resolve` — `merge_queue::resolve`
- [ ] `GET /api/projects/{pid}/merge-queue/{id}/diff` — `merge_queue::get_diff`
- [ ] `POST /api/projects/{pid}/merge-queue/{id}/reject` — `merge_queue::reject`

### Runtimes (src/api/runtimes.rs)
- [ ] `GET /api/runtimes` — `runtimes::list`

### Loom (src/api/loom.rs)
- [ ] `GET /api/projects/{pid}/loom` — `loom::list_by_project`
- [ ] `GET /api/teams/{tid}/loom` — `loom::list_by_team`
- [ ] `GET /api/loom` — `loom::list_recent`
- [ ] `POST /api/loom` — `loom::create`

### Swarm Status (src/api/swarm.rs)
- [ ] `GET /api/projects/{pid}/swarm-status` — `swarm::get_status`

### Prompt Templates (src/api/prompt_templates.rs)
- [ ] `GET /api/prompt-templates` — `prompt_templates::list_templates`
- [ ] `POST /api/prompt-templates` — `prompt_templates::create_template`
- [ ] `GET /api/prompt-templates/{id}` — `prompt_templates::get_template`
- [ ] `PUT /api/prompt-templates/{id}` — `prompt_templates::update_template`
- [ ] `DELETE /api/prompt-templates/{id}` — `prompt_templates::delete_template`
- [ ] `POST /api/prompt-templates/assignments` — `prompt_templates::create_assignment`
- [ ] `DELETE /api/prompt-templates/assignments/{id}` — `prompt_templates::delete_assignment`
- [ ] `GET /api/prompt-templates/roles/{role}/assignments` — `prompt_templates::list_assignments`
- [ ] `GET /api/prompt-templates/roles/{role}/build` — `prompt_templates::build_prompt`

### Cost Tracking (src/api/cost_tracking.rs)
- [ ] `GET /api/projects/{pid}/costs` — `cost_tracking::list_costs`
- [ ] `GET /api/projects/{pid}/costs/summary` — `cost_tracking::get_summary`
- [ ] `GET /api/projects/{pid}/costs/daily` — `cost_tracking::get_daily_spend`
- [ ] `POST /api/projects/{pid}/costs/aggregate` — `cost_tracking::aggregate_now`

### Role Registry (src/api/roles.rs)
- [ ] `GET /api/roles` — `roles::list_roles`
- [ ] `POST /api/roles` — `roles::create_role`
- [ ] `GET /api/roles/{name}` — `roles::get_role`
- [ ] `PUT /api/roles/{name}` — `roles::update_role`
- [ ] `DELETE /api/roles/{name}` — `roles::delete_role`

### Quality Tiers (src/api/quality.rs)
- [ ] `GET /api/quality-tiers` — `quality::list_tiers`
- [ ] `GET /api/projects/{pid}/quality` — `quality::get_project_tiers`
- [ ] `PUT /api/projects/{pid}/quality` — `quality::set_project_tiers`
- [ ] `POST /api/projects/{pid}/quality/reset` — `quality::reset_project_tiers`
- [ ] `GET /api/teams/{tid}/quality` — `quality::get_team_tiers`
- [ ] `PUT /api/teams/{tid}/quality` — `quality::set_team_tiers`

### Performance Log (src/api/performance.rs)
- [ ] `GET /api/projects/{pid}/performance` — `performance::list_logs`
- [ ] `POST /api/projects/{pid}/performance` — `performance::create_log`
- [ ] `GET /api/projects/{pid}/performance/stats` — `performance::model_stats`

### Coordinator (src/api/coordinator.rs)
- [ ] `GET /api/coordinators` — `coordinator::list_coordinators`
- [ ] `GET /api/projects/{pid}/coordinator` — `coordinator::get_coordinator`
- [ ] `POST /api/projects/{pid}/coordinator/wake` — `coordinator::wake_coordinator`
- [ ] `POST /api/projects/{pid}/coordinator/sleep` — `coordinator::sleep_coordinator`

### Routing Overrides (src/api/routing_overrides.rs)
- [ ] `GET /api/projects/{pid}/routing-overrides` — `routing_overrides::list_overrides`
- [ ] `POST /api/projects/{pid}/routing-overrides` — `routing_overrides::create_override`
- [ ] `POST /api/projects/{pid}/routing-overrides/detect` — `routing_overrides::detect_patterns`
- [ ] `POST /api/routing-overrides/{id}/accept` — `routing_overrides::accept_override`
- [ ] `POST /api/routing-overrides/{id}/reject` — `routing_overrides::reject_override`

### Team Role Overrides (src/api/team_role_overrides.rs)
- [ ] `GET /api/teams/{tid}/role-overrides` — `team_role_overrides::list_overrides`
- [ ] `POST /api/teams/{tid}/role-overrides` — `team_role_overrides::set_override`
- [ ] `DELETE /api/teams/{tid}/role-overrides/{role}` — `team_role_overrides::clear_override`

### Code Graph (src/api/code_graph.rs)
- [ ] `GET /api/projects/{pid}/graph/nodes` — `code_graph::list_nodes`
- [ ] `POST /api/projects/{pid}/graph/nodes` — `code_graph::upsert_node`
- [ ] `POST /api/projects/{pid}/graph/edges` — `code_graph::upsert_edge`
- [ ] `GET /api/projects/{pid}/graph/complexity` — `code_graph::file_complexity`
- [ ] `POST /api/projects/{pid}/graph/complexity/recompute` — `code_graph::recompute_complexity`
- [ ] `GET /api/projects/{pid}/graph/blast-radius` — `code_graph::blast_radius`
- [ ] `DELETE /api/projects/{pid}/graph/clear` — `code_graph::clear_graph`

### Workflow Traces (src/api/workflow_traces.rs)
- [ ] `GET /api/projects/{pid}/traces` — `workflow_traces::list_traces`
- [ ] `POST /api/projects/{pid}/traces` — `workflow_traces::start_trace`
- [ ] `GET /api/traces/{id}` — `workflow_traces::get_trace`
- [ ] `GET /api/traces/{id}/steps` — `workflow_traces::get_trace_steps`
- [ ] `POST /api/traces/{id}/steps` — `workflow_traces::add_step`
- [ ] `POST /api/traces/{id}/complete` — `workflow_traces::complete_trace`
- [ ] `GET /api/projects/{pid}/chokepoints` — `workflow_traces::list_chokepoints`
- [ ] `POST /api/projects/{pid}/chokepoints/detect` — `workflow_traces::detect_chokepoints`

### Knowledge Graph (src/api/knowledge.rs)
- [ ] `GET /api/projects/{pid}/knowledge` — `knowledge::list_patterns`
- [ ] `GET /api/projects/{pid}/knowledge/{id}` — `knowledge::get_pattern`
- [ ] `POST /api/projects/{pid}/knowledge` — `knowledge::create_pattern`
- [ ] `POST /api/projects/{pid}/knowledge/search` — `knowledge::search_patterns`
- [ ] `POST /api/projects/{pid}/knowledge/extract` — `knowledge::trigger_extraction`
- [ ] `PUT /api/projects/{pid}/knowledge/{id}` — `knowledge::update_pattern`
- [ ] `DELETE /api/projects/{pid}/knowledge/{id}` — `knowledge::delete_pattern`
- [ ] `GET /api/knowledge/cross-project` — `knowledge::cross_project_search`

### Cross-Project Learning (src/api/cross_project.rs)
- [ ] `GET /api/cross-project/suggestions` — `cross_project::global_suggestions`
- [ ] `GET /api/cross-project/opted-in` — `cross_project::list_opted_in`
- [ ] `POST /api/projects/{pid}/share-learning` — `cross_project::toggle_sharing`

### Test Runner (src/api/tests.rs)
- [ ] `POST /api/projects/{pid}/tests/run` — `tests::trigger_run`
- [ ] `GET /api/projects/{pid}/tests/runs` — `tests::list_runs`
- [ ] `GET /api/projects/{pid}/tests/runs/{id}` — `tests::get_run`
- [ ] `GET /api/projects/{pid}/tests/latest` — `tests::latest_run`
- [ ] `POST /api/projects/{pid}/tests/runs/{id}/stop` — `tests::stop_run`

### Dispatch Killswitch (src/api/dispatch.rs)
- [ ] `POST /api/dispatch/pause` — `dispatch::global_pause`
- [ ] `POST /api/dispatch/resume` — `dispatch::global_resume`
- [ ] `GET /api/dispatch/status` — `dispatch::global_status`
- [ ] `POST /api/projects/{pid}/dispatch/pause` — `dispatch::project_pause`
- [ ] `POST /api/projects/{pid}/dispatch/resume` — `dispatch::project_resume`
- [ ] `GET /api/projects/{pid}/dispatch/status` — `dispatch::project_status`
- [ ] `GET /api/dispatch/schedules` — `dispatch::list_schedules`
- [ ] `POST /api/dispatch/schedules` — `dispatch::create_schedule`
- [ ] `GET /api/dispatch/schedules/{id}` — `dispatch::get_schedule`
- [ ] `PUT /api/dispatch/schedules/{id}` — `dispatch::update_schedule`
- [ ] `DELETE /api/dispatch/schedules/{id}` — `dispatch::delete_schedule`

---

## Database Models (src/models/)

- [ ] `activity_log.rs` — ActivityLog (event tracking)
- [ ] `agent.rs` — Agent (runtime agent sessions)
- [ ] `attachment.rs` — Attachment (file attachments on issues)
- [ ] `code_graph.rs` — CodeGraphNode, CodeGraphEdge (dependency graph)
- [ ] `coordinator.rs` — Coordinator (per-project coordination state)
- [ ] `cost_tracking.rs` — CostRecord, CostAggregate (token/cost tracking)
- [ ] `dispatch_schedule.rs` — DispatchSchedule (cron-based killswitch schedules)
- [ ] `issue.rs` — Issue, CreateIssue, UpdateIssue (task/issue management)
- [ ] `knowledge_pattern.rs` — KnowledgePattern, hybrid search, keyword extraction
- [ ] `loom.rs` — LoomEntry (agent activity narrative log)
- [ ] `merge_queue.rs` — MergeQueueEntry (branch merge management)
- [ ] `merge_queue_entry.rs` — MergeQueueEntry (alternate/extended)
- [ ] `mount.rs` — MountConfig (remote filesystem mounts)
- [ ] `performance_log.rs` — PerformanceLog (model performance tracking)
- [ ] `project.rs` — Project (with pause fields: `is_paused`, `paused_at`, `pause_reason`)
- [ ] `project_app.rs` — ProjectApp (app preview state)
- [ ] `prompt_template.rs` — PromptTemplate, PromptTemplateAssignment
- [ ] `test_run.rs` — TestRun, CreateTestRun (Playwright test execution results)
- [ ] `proxy_config.rs` — ProxyConfig, ProxyHop (SSH proxy chains)
- [ ] `quality.rs` — QualityTier, TierRange (model quality tiers)
- [ ] `role.rs` — Role (global role registry with seed defaults)
- [ ] `routing_override.rs` — RoutingOverride (model routing suggestions)
- [ ] `setting.rs` — Setting (key-value config with categories)
- [ ] `team.rs` — Team, TeamAgentSlot (team composition)
- [ ] `team_role_override.rs` — TeamRoleOverride (per-team model overrides)
- [ ] `workflow.rs` — WorkflowDefinition, WorkflowInstance
- [ ] `workflow_trace.rs` — WorkflowTrace, TraceStep, Chokepoint

---

## Database Layer (src/db/)

- [ ] `mod.rs` — `init_db()`, connection pool setup
- [ ] `migrations.rs` — All schema migrations (run_migrations)
- [ ] `seeds.rs` — `seed_team_templates()` (default team templates)

### Key Seeded Defaults (in main.rs)
- [ ] `browse_roots` setting from config
- [ ] `mount_base` setting from config
- [ ] `idle_unmount_minutes` setting from config
- [ ] `master_key` setting from security config
- [ ] `global_dispatch_paused` = "false" (killswitch category)
- [ ] Team template seeds (`seed_team_templates`)
- [ ] Role registry seeds (`Role::seed_defaults`)

---

## Orchestrator (src/orchestrator/)

- [ ] `runner.rs` — OrchestratorRunner, OrchestratorHandle
  - [ ] 30-second sweep loop
  - [ ] `sweep()` — main orchestration cycle
  - [ ] `sweep_teams()` — per-project team work dispatch
  - [ ] `evaluate_schedules()` — cron-based killswitch schedule evaluation
  - [ ] Global pause check (reads `global_dispatch_paused` setting)
  - [ ] Per-project pause check (`project.is_paused`)
  - [ ] `restore_running_instances()` — restart on boot
  - [ ] Build server integration (`.with_build_server()`)
- [ ] `engine.rs` — Orchestration engine logic
- [ ] `context.rs` — Orchestration context
- [ ] `state_machine.rs` — Agent state machine
- [ ] `plan_parser.rs` — Plan file parsing
- [ ] `swarm.rs` — Swarm coordination
- [ ] `mod.rs` — Module exports

---

## Runtime Adapters (src/runtime/)

- [ ] `adapter.rs` — RuntimeAdapter trait definition
- [ ] `claude.rs` — Claude Code CLI adapter
- [ ] `opencode.rs` — OpenCode (Ollama/OpenRouter) adapter
- [ ] `gemini.rs` — Gemini CLI adapter
- [ ] `mod.rs` — RuntimeRegistry (adapter discovery)

---

## Infrastructure

### Process Management (src/process/)
- [ ] `manager.rs` — ProcessManager (spawn, stop, track agent processes)

### Worktree Management (src/worktree/)
- [ ] `manager.rs` — WorktreeManager (git worktree lifecycle)
- [ ] `merge_queue.rs` — Merge queue operations
- [ ] `mod.rs` — Module exports

### Mount Management (src/mount/)
- [ ] `manager.rs` — MountManager (SSHFS/NFS/SMB mount/unmount)
- [ ] `crypto.rs` — Credential encryption
- [ ] `idle_monitor.rs` — Auto-unmount idle mounts
- [ ] `mod.rs` — Module exports

### Sync Management (src/sync/)
- [ ] `manager.rs` — SyncManager (project file sync)
- [ ] `mod.rs` — Module exports

### App Runner (src/app_runner/)
- [ ] `runner.rs` — AppRunner (project app preview start/stop)

### State (src/state.rs)
- [ ] `AppState` struct (db, process_manager, runtime_registry, auth_config, mount_manager, filesystem_config, sync_manager, orchestrator, data_dir, app_runner)

### Config (src/config.rs)
- [ ] Config loading from `ironweave.toml`
- [ ] TLS support (cert_path, key_path)
- [ ] Filesystem config (browse_roots, mount_base, idle_unmount_minutes)
- [ ] Security config (master_key)
- [ ] Auth config
- [ ] Build server config

### Frontend Serving
- [ ] `RustEmbed` for `frontend/dist/`
- [ ] SPA fallback to `index.html`
- [ ] MIME type detection
