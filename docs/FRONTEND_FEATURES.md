# Frontend Feature Checklist

> **Living document** — update when features are added, modified, or removed.
> Use this to verify nothing has been accidentally deleted during development.

Last updated: 2026-03-16

---

## App Shell (frontend/src/App.svelte)

- [ ] SPA router (`svelte-spa-router`)
- [ ] Sidebar navigation (Dashboard, Projects, Mounts, Agents, Settings)
- [ ] Backend health check (`/api/health`)
- [ ] Auth check — redirect to `/login` if 401
- [ ] Logout button (conditional on auth enabled)
- [ ] `/settings` redirect to `/settings/general`
- [ ] Dark theme (Tailwind `bg-gray-950`)

---

## Routes (frontend/src/routes/)

### Dashboard (Dashboard.svelte) — `/#/`
- [ ] Dashboard stats display (projects, agents, issues, workflows)
- [ ] Current work items list
- [ ] `<KillSwitch />` component embedded
- [ ] Activity feed
- [ ] Metrics charts
- [ ] System health panel

### Projects List (Projects.svelte) — `/#/projects`
- [ ] Project tiles/cards grid
- [ ] Create project form
- [ ] Global pause state indicator
- [ ] Per-project status badges: green "Active", red "Paused", amber "Global Pause"
- [ ] Per-project Pause/Resume toggle button
- [ ] Button disabled when global override active
- [ ] Quick-trigger E2E test button per project tile (play icon with status)
- [ ] `fetchGlobalPause()` state management
- [ ] `handleTogglePause()` per project

### Project Detail (ProjectDetail.svelte) — `/#/projects/:id`
- [ ] Project info header (name, directory, context)
- [ ] Dispatch status badges (global override amber, paused red, active green)
- [ ] Dispatch toggle handler
- [ ] Tabbed interface:
  - [ ] Issues tab — `<IssueBoard />`
  - [ ] Teams tab — team management, slots, activation
  - [ ] Merge Queue tab — `<MergeQueue />`
  - [ ] Workflows tab — `<WorkflowRunner />`
  - [ ] Files tab — `<ProjectFiles />`
  - [ ] History tab — `<ProjectHistory />`
  - [ ] Settings tab — `<ProjectSettings />`
  - [ ] Loom tab — `<LoomFeed />`
  - [ ] Swarm tab — `<SwarmStatus />`
  - [ ] Costs tab — `<CostDashboard />`
  - [ ] Quality tab — `<QualitySlider />`
  - [ ] Routing tab — `<RoutingSuggestions />`
  - [ ] Coordinator tab — `<CoordinatorPanel />`
  - [ ] Details tab — `<ProjectDetailsPanel />`
  - [ ] Features tab — `<FeaturePanel />`
  - [ ] Knowledge tab — `<KnowledgePanel />`
  - [ ] Tests tab — `<TestRunPanel />`
  - [ ] App Preview — start/stop/status

### Workflow View (WorkflowView.svelte) — `/#/projects/:id/workflows/:wid`
- [ ] Workflow definition display
- [ ] Instance management (create, pause, resume, cancel)
- [ ] Stage gate approvals
- [ ] DAG visualization

### Mounts (Mounts.svelte) — `/#/mounts`
- [ ] Mount list with status indicators
- [ ] Create/edit mount form (SSHFS/NFS/SMB)
- [ ] Mount/unmount actions
- [ ] SSH test connection
- [ ] Remote directory browser
- [ ] Duplicate mount
- [ ] Proxy config selection

### Agents (Agents.svelte) — `/#/agents`
- [ ] Agent list with state indicators
- [ ] Agent spawn form
- [ ] Stop agent action
- [ ] Delete dead agents
- [ ] WebSocket terminal output (`<Terminal />`)

### Login (Login.svelte) — `/#/login`
- [ ] Username/password form
- [ ] Token-based auth (localStorage)
- [ ] Redirect on success

### Settings — General (SettingsGeneral.svelte) — `/#/settings/general`
- [ ] Settings key-value list
- [ ] Upsert/delete settings

### Settings — Proxies (SettingsProxies.svelte) — `/#/settings/proxies`
- [ ] Proxy config list
- [ ] Create/edit proxy (multi-hop SSH chains)
- [ ] Test connection
- [ ] Delete proxy

### Settings — API Keys (SettingsApiKeys.svelte) — `/#/settings/api-keys`
- [ ] API key management

---

## Components (frontend/src/lib/components/)

### KillSwitch.svelte
- [ ] Global dispatch toggle (pause/resume)
- [ ] Status display (paused/active with timestamps)
- [ ] Schedule management (create/edit/delete cron schedules)
- [ ] 15-second auto-refresh
- [ ] Timezone support for schedules

### IssueBoard.svelte
- [ ] Issue list grouped by status
- [ ] Create issue form (title, description, type, priority, role, parent, depends_on)
- [ ] Edit/update issue
- [ ] Delete issue
- [ ] Claim/unclaim by agent
- [ ] File attachment upload/download
- [ ] Scope mode and needs_intake fields

### IntakeChat.svelte
- [ ] Chat interface for issue intake/refinement

### DagGraph.svelte
- [ ] DAG visualization for workflow stages

### MergeQueue.svelte
- [ ] Merge queue entry list
- [ ] Approve/reject/resolve actions
- [ ] Diff viewer

### MergeHealthChart.svelte
- [ ] Merge statistics visualization (clean/conflicted/escalated)

### WorkflowRunner.svelte
- [ ] Workflow definition list
- [ ] Instance creation and management
- [ ] Gate approval interface

### ProjectFiles.svelte
- [ ] File browser for synced project files
- [ ] File content viewer

### ProjectHistory.svelte
- [ ] Sync history with snapshots
- [ ] Diff viewer per change
- [ ] Restore from snapshot

### ProjectSettings.svelte
- [ ] Project configuration editor (name, directory, git remote, Obsidian, etc.)
- [ ] Mount/sync path configuration
- [ ] App URL configuration

### LoomFeed.svelte
- [ ] Narrative log of agent activity
- [ ] Filter by team/project
- [ ] Role, runtime, model display per entry

### SwarmStatus.svelte
- [ ] Active/idle/total agent counts
- [ ] Task pool depth
- [ ] Throughput metrics
- [ ] Scaling recommendation
- [ ] Per-agent detail (role, runtime, state, current issue)

### CostDashboard.svelte
- [ ] Cost summary (total tokens, USD, by role, by model)
- [ ] Daily spend chart
- [ ] Aggregate trigger

### QualitySlider.svelte
- [ ] Quality tier floor/ceiling controls
- [ ] Per-project and per-team tier settings
- [ ] Tier list with labels, example models, cost ranges

### RoutingSuggestions.svelte
- [ ] Routing override list (role, task type, model suggestions)
- [ ] Accept/reject override actions
- [ ] Detect patterns trigger
- [ ] Confidence scores and evidence display

### CoordinatorPanel.svelte
- [ ] Coordinator state display
- [ ] Wake/sleep actions
- [ ] Session ID tracking

### FeaturePanel.svelte
- [ ] Feature list with status badges and task progress bars
- [ ] Status filter tabs (All/Ideas/Designed/In Progress/Implemented/Verified/Parked)
- [ ] Add Feature form (title, description, priority)
- [ ] Import PRD modal (paste any text)
- [ ] Expandable feature cards with task lists
- [ ] Implement button on tasks (creates Ironweave issue)
- [ ] Park/Verify/Abandon action buttons
- [ ] Implementation notes editing
- [ ] 30-second auto-refresh

### ProjectDetailsPanel.svelte
- [ ] Split layout: Intent (editable) | Reality (read-only)
- [ ] Intent editor with save and version indicator
- [ ] Removal detection warning on save
- [ ] Reality viewer with Rescan button
- [ ] Gap analysis section (missing red, undocumented amber)
- [ ] Create Feature from gap items

### KnowledgePanel.svelte
- [ ] Pattern list with type badges (solution/gotcha/preference/recipe)
- [ ] Filter by type and role
- [ ] Add Pattern form (title, content, type, role, task_type, keywords)
- [ ] Extract Now button
- [ ] Pattern card with confidence bar, observations, source type
- [ ] Expandable content detail
- [ ] Delete pattern
- [ ] Shared badge indicator
- [ ] 30-second auto-refresh

### TestRunPanel.svelte
- [ ] Run test buttons (E2E / Unit / Full)
- [ ] Stop running test
- [ ] Test run history list (scrollable)
- [ ] Run detail panel (status, pass/fail/skip, duration)
- [ ] Failed test names display
- [ ] Collapsible full output viewer
- [ ] 3-second polling during active runs
- [ ] 15-second auto-refresh for run history

### TeamRoleOverrides.svelte
- [ ] Per-role runtime/model override controls
- [ ] Set/clear overrides per team

### PromptEditor.svelte
- [ ] Prompt template CRUD
- [ ] Assignment management (role-to-template mapping)
- [ ] Build/preview prompt for role

### ActivityFeed.svelte
- [ ] Activity log entries (event type, project, agent, issue, message)
- [ ] Pagination (limit/offset)

### AgentUtilChart.svelte
- [ ] Agent utilization visualization

### MetricsChart.svelte
- [ ] Daily metrics chart (events by type over time)

### SystemHealth.svelte
- [ ] CPU, memory, disk usage display
- [ ] Agent process count

### Terminal.svelte
- [ ] WebSocket-based agent output terminal
- [ ] Real-time streaming

### DirectoryBrowser.svelte
- [ ] Local filesystem directory browser
- [ ] Used in project creation and mount configuration

---

## API Client (frontend/src/lib/api.ts)

### Generic Helpers
- [ ] `get<T>()` — GET with auth headers, 401 handling
- [ ] `post<T>()` — POST with JSON body
- [ ] `patch<T>()` — PATCH with JSON body
- [ ] `put<T>()` — PUT with JSON body
- [ ] `del()` — DELETE with auth headers
- [ ] `authHeaders()` — Bearer token from localStorage
- [ ] `handle401()` — Clear token, redirect to login

### API Objects
- [ ] `auth` — login, logout, isAuthenticated, getToken
- [ ] `projects` — list, get, create, update, delete
- [ ] `projectApps` — start, stop, status
- [ ] `teams` — list, get, create, delete, templates, projectTemplates, cloneTemplate, activate, deactivate, updateConfig, status
- [ ] `teams.slots` — list, create, update, delete
- [ ] `issues` — list, get, create, update, claim, unclaim, ready, delete, updateStatus
- [ ] `issues.attachments` — list, upload, downloadUrl
- [ ] `agents` — list, get, spawn, stop
- [ ] `workflows.definitions` — list, get, create
- [ ] `workflows.instances` — list, create, approveGate, pause, resume, cancel
- [ ] `dashboard` — stats, activity, metrics, system
- [ ] `filesystem` — browse
- [ ] `mounts` — list, get, create, update, delete, duplicate, mount, unmount, status, testSsh, browseRemote
- [ ] `settings` — list, get, upsert, delete
- [ ] `proxyConfigs` — list, get, create, update, delete, test
- [ ] `runtimes` — list
- [ ] `mergeQueue` — list, approve, resolve, diff, reject
- [ ] `loom` — recent, byProject, byTeam
- [ ] `swarm` — status
- [ ] `promptTemplates` — list, get, create, update, delete, listAssignments, createAssignment, deleteAssignment, buildPrompt
- [ ] `sync` — trigger, status, history, diff, restore, browseFiles, readFile
- [ ] `qualityTiers` — list, getProject, setProject, resetProject, getTeam, setTeam
- [ ] `costTracking` — summary, daily, aggregate
- [ ] `coordinator` — get, wake, sleep, list
- [ ] `routingOverrides` — list, detect, accept, reject
- [ ] `teamRoleOverrides` — list, set, clear
- [ ] `performanceLog` — list, stats
- [ ] `dispatch` — status, pause, resume, projectStatus, projectPause, projectResume
- [ ] `dispatch.schedules` — list, create, update, delete
- [ ] `testRunner` — trigger, list, get, latest, stop
- [ ] `features` — list, get, create, update, delete, park, verify, import, summary
- [ ] `featureTasks` — list, create, update, delete, implement
- [ ] `projectDocuments` — get, update, history, scan, gaps
- [ ] `knowledge` — list, get, create, search, crossProject, update, delete, extract

### TypeScript Interfaces
- [ ] `AppStatus`
- [ ] `Project` (includes `is_paused`, `paused_at`, `pause_reason`)
- [ ] `CreateProject`, `UpdateProject`
- [ ] `Team`, `CreateTeam`
- [ ] `TeamAgentSlot`, `CreateTeamAgentSlot`, `UpdateTeamAgentSlot`
- [ ] `ScalingRecommendation`, `ScalingInfo`, `TeamStatus`
- [ ] `RuntimeCapabilities`, `RuntimeInfo`
- [ ] `Issue`, `CreateIssue`, `UpdateIssue`
- [ ] `Attachment`
- [ ] `AgentInfo`, `SpawnAgentRequest`
- [ ] `WorkflowDefinition`, `CreateWorkflowDef`
- [ ] `WorkflowInstance`, `CreateInstance`
- [ ] `CurrentWorkItem`, `DashboardStats`
- [ ] `ActivityLogEntry`
- [ ] `DailyMetric`, `MetricsResponse`
- [ ] `SystemHealth`
- [ ] `BrowseEntry`, `BrowseResponse`
- [ ] `MountConfig`, `CreateMountConfig`
- [ ] `Setting`, `UpsertSetting`
- [ ] `ProxyHop`, `ProxyConfigResponse`, `CreateProxyConfig`, `UpdateProxyConfig`
- [ ] `TestConnectionResult`
- [ ] `SyncStatus`, `SyncSnapshot`
- [ ] `DispatchStatus`, `ProjectDispatchStatus`
- [ ] `DispatchSchedule`, `CreateDispatchSchedule`
- [ ] `SshTestRequest`, `RemoteBrowseRequest`, `RemoteBrowseResponse`
- [ ] `MergeQueueEntry`, `MergeQueueDiff`
- [ ] `LoomEntry`
- [ ] `SwarmAgent`, `SwarmStatus`
- [ ] `PromptTemplate`, `PromptTemplateAssignment`
- [ ] `QualityTier`, `TierRange`
- [ ] `CostSummary`, `DailySpend`
- [ ] `CoordinatorMemory`
- [ ] `RoutingOverride`
- [ ] `TeamRoleOverride`
- [ ] `PerformanceLogEntry`, `ModelStats`
- [ ] `TestRun`
- [ ] `Feature`, `CreateFeature`, `FeatureWithTasks`, `FeatureTask`, `FeatureSummary`
- [ ] `ProjectDocument`, `GapAnalysis`
- [ ] `KnowledgePattern`, `CreateKnowledgePattern`, `KnowledgeSearchResult`

### Constants
- [ ] `RUNTIME_MODELS` — model lists per runtime (claude, opencode, gemini)
- [ ] `PREDEFINED_ROLES` — 16 predefined agent roles
