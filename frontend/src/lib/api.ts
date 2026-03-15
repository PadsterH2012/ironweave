const BASE = '/api';

// ── Auth token helpers ───────────────────────────────────────────

function getToken(): string | null {
  return localStorage.getItem('ironweave_token');
}

function setToken(token: string): void {
  localStorage.setItem('ironweave_token', token);
}

function clearToken(): void {
  localStorage.removeItem('ironweave_token');
}

export function authHeaders(): Record<string, string> {
  const token = getToken();
  if (token) {
    return { 'Authorization': `Bearer ${token}` };
  }
  return {};
}

function handle401(res: Response): void {
  if (res.status === 401) {
    clearToken();
    window.location.hash = '#/login';
  }
}

// ── Type definitions ─────────────────────────────────────────────

export interface AppStatus {
  id: string | null;
  state: string;
  port: number | null;
  url: string | null;
  run_command: string | null;
  last_error: string | null;
  started_at: string | null;
}

export interface Project {
  id: string;
  name: string;
  directory: string;
  context: string;
  description: string | null;
  obsidian_vault_path: string | null;
  obsidian_project: string | null;
  git_remote: string | null;
  mount_id: string | null;
  sync_path: string | null;
  last_synced_at: string | null;
  app_url: string | null;
  sync_state: string;
  created_at: string;
}

export interface CreateProject {
  name: string;
  directory: string;
  context: string;
  obsidian_vault_path?: string;
  obsidian_project?: string;
  git_remote?: string;
  mount_id?: string;
}

export interface Team {
  id: string;
  name: string;
  project_id: string;
  coordination_mode: string;
  max_agents: number;
  token_budget: number | null;
  cost_budget_daily: number | null;
  is_template: boolean;
  auto_pickup_types: string;
  is_active: boolean;
  created_at: string;
}

export interface CreateTeam {
  name: string;
  project_id: string;
  coordination_mode?: string;
  max_agents?: number;
  token_budget?: number;
  cost_budget_daily?: number;
  is_template?: boolean;
}

export interface TeamAgentSlot {
  id: string;
  team_id: string;
  role: string;
  runtime: string;
  model: string | null;
  config: string;
  slot_order: number;
}

export interface CreateTeamAgentSlot {
  role: string;
  runtime: string;
  model?: string;
  config?: string;
  slot_order?: number;
}

export interface UpdateTeamAgentSlot {
  role?: string;
  runtime?: string;
  model?: string | null;
  slot_order?: number;
}

export interface ScalingRecommendation {
  action: string;
  count: number;
  reason: string;
}

export interface ScalingInfo {
  recommendation: ScalingRecommendation;
  pool_depth: number;
  idle_agents: number;
  active_agents: number;
  max_agents: number;
}

export interface TeamStatus {
  team_id: string;
  is_active: boolean;
  auto_pickup_types: string[];
  roles: {
    role: string;
    slot_count: number;
    running: number;
    runtime: string;
    model: string | null;
  }[];
  scaling: ScalingInfo;
}

export interface RuntimeCapabilities {
  streaming: boolean;
  tool_use: boolean;
  model_selection: boolean;
  allowed_tools_filter: boolean;
  dangerously_skip_permissions: boolean;
  non_interactive: boolean;
  supported_models: string[];
}

export interface RuntimeInfo {
  id: string;
  name: string;
  capabilities: RuntimeCapabilities;
}

export const RUNTIME_MODELS: Record<string, string[]> = {
  claude: ['claude-sonnet-4-6', 'claude-opus-4-6', 'claude-haiku-4-5-20251001'],
  opencode: [],
  gemini: ['gemini-2.5-pro', 'gemini-2.5-flash'],
};

export const PREDEFINED_ROLES: string[] = [
  'Architect',
  'Senior Coder',
  'Code Reviewer',
  'DB Senior Engineer',
  'UI/UX Senior Coder',
  'Senior UX/UI Designer',
  'Brand Designer',
  'Senior Tester',
  'Security Engineer',
  'DevOps Engineer',
  'Infrastructure Engineer',
  'Researcher',
  'Documentor',
  'Marketing Manager',
  'News Letter Writer',
  'Office Monkey',
];

export interface Issue {
  id: string;
  project_id: string;
  type: string;
  title: string;
  description: string;
  status: string;
  priority: number;
  claimed_by: string | null;
  claimed_at: string | null;
  depends_on: string;
  summary: string | null;
  workflow_instance_id: string | null;
  stage_id: string | null;
  role: string | null;
  parent_id: string | null;
  needs_intake: number;
  scope_mode: string;
  created_at: string;
  updated_at: string;
}

export interface CreateIssue {
  project_id: string;
  issue_type?: string;
  title: string;
  description?: string;
  priority?: number;
  depends_on?: string[];
  workflow_instance_id?: string;
  stage_id?: string;
  role?: string;
  parent_id?: string;
  needs_intake?: number;
  scope_mode?: string;
}

export interface UpdateIssue {
  status?: string;
  title?: string;
  description?: string;
  summary?: string;
  priority?: number;
  role?: string;
  needs_intake?: number;
  scope_mode?: string;
}

export interface Attachment {
  id: string;
  issue_id: string;
  filename: string;
  mime_type: string;
  size_bytes: number;
  created_at: string;
}

export interface AgentInfo {
  id: string;
  runtime: string;
  state: string;
  role?: string;
  claimed_issue?: string;
  last_heartbeat?: string;
}

export interface SpawnAgentRequest {
  runtime: string;
  working_directory: string;
  prompt: string;
  env?: Record<string, string>;
}

export interface WorkflowDefinition {
  id: string;
  name: string;
  project_id: string;
  team_id: string;
  dag: string;
  version: number;
  git_sha: string | null;
  created_at: string;
}

export interface CreateWorkflowDef {
  name: string;
  project_id: string;
  team_id: string;
  dag?: string;
  version?: number;
  git_sha?: string;
}

export interface WorkflowInstance {
  id: string;
  definition_id: string;
  state: string;
  current_stage: string | null;
  checkpoint: string;
  started_at: string | null;
  completed_at: string | null;
  total_tokens: number;
  total_cost: number;
  created_at: string;
}

export interface CreateInstance {
  definition_id: string;
  current_stage?: string;
}

export interface DashboardStats {
  project_count: number;
  active_agents: number;
  open_issues: number;
  running_workflows: number;
}

export interface ActivityLogEntry {
  id: string;
  event_type: string;
  project_id: string | null;
  team_id: string | null;
  agent_id: string | null;
  issue_id: string | null;
  workflow_instance_id: string | null;
  message: string;
  metadata: string;
  created_at: string;
}

export interface DailyMetric {
  day: string;
  event_type: string;
  count: number;
}

export interface MetricsResponse {
  daily: DailyMetric[];
  merge_stats: { total: number; clean: number; conflicted: number; escalated: number };
  avg_resolution_hours: number;
}

export interface SystemHealth {
  cpu_usage_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  disk_used_gb: number;
  disk_total_gb: number;
  agent_process_count: number;
}

export interface BrowseEntry {
  name: string;
  type: 'directory' | 'file';
}

export interface BrowseResponse {
  path: string;
  parent: string | null;
  entries: BrowseEntry[];
}

export interface MountConfig {
  id: string;
  name: string;
  mount_type: 'nfs' | 'smb' | 'sshfs';
  remote_path: string;
  local_mount_point: string;
  username: string | null;
  password: string | null;
  ssh_key: string | null;
  mount_options: string | null;
  auto_mount: boolean;
  proxy_config_id: string | null;
  git_remote: string | null;
  state: 'mounted' | 'unmounted' | 'error';
  last_error: string | null;
  created_at: string;
}

export interface CreateMountConfig {
  name: string;
  mount_type: 'nfs' | 'smb' | 'sshfs';
  remote_path: string;
  local_mount_point: string;
  username?: string;
  password?: string;
  ssh_key?: string;
  mount_options?: string;
  auto_mount?: boolean;
  proxy_config_id?: string;
  git_remote?: string;
}

export interface Setting {
  key: string;
  value: string;
  category: string;
  updated_at: string;
}

export interface UpsertSetting {
  value: string;
  category?: string;
}

export interface ProxyHop {
  host: string;
  port: number;
  username: string;
  auth_type: 'key' | 'password';
  credential: string | null;
}

export interface ProxyConfigResponse {
  id: string;
  name: string;
  hops: ProxyHop[];
  is_active: boolean;
  created_at: string;
}

export interface CreateProxyConfig {
  name: string;
  hops: ProxyHop[];
}

export interface UpdateProxyConfig {
  name?: string;
  hops?: ProxyHop[];
  is_active?: boolean;
}

export interface TestConnectionResult {
  success: boolean;
  hops_tested?: number;
  failed_hop?: number;
  message?: string;
  error?: string;
}

export interface UpdateProject {
  name?: string;
  directory?: string;
  context?: string;
  description?: string;
  obsidian_vault_path?: string;
  obsidian_project?: string;
  git_remote?: string;
  mount_id?: string;
  app_url?: string;
}

export interface SyncStatus {
  sync_state: string;
  last_synced_at: string | null;
  sync_path: string | null;
  source: string;
}

export interface SyncSnapshot {
  change_id: string;
  description: string;
  timestamp: string;
}

// ── Generic fetch helpers ────────────────────────────────────────

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { ...authHeaders() },
  });
  if (!res.ok) {
    handle401(res);
    throw new Error(`GET ${path} failed: ${res.status} ${res.statusText}`);
  }
  return res.json();
}

async function post<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...authHeaders() },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    handle401(res);
    throw new Error(`POST ${path} failed: ${res.status} ${res.statusText}`);
  }
  const text = await res.text();
  return text ? JSON.parse(text) : (undefined as unknown as T);
}

async function patch<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json', ...authHeaders() },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    handle401(res);
    throw new Error(`PATCH ${path} failed: ${res.status} ${res.statusText}`);
  }
  const text = await res.text();
  return text ? JSON.parse(text) : (undefined as unknown as T);
}

async function put<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json', ...authHeaders() },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    handle401(res);
    throw new Error(`PUT ${path} failed: ${res.status} ${res.statusText}`);
  }
  const text = await res.text();
  return text ? JSON.parse(text) : (undefined as unknown as T);
}

async function del(path: string): Promise<void> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'DELETE',
    headers: { ...authHeaders() },
  });
  if (!res.ok) {
    handle401(res);
    throw new Error(`DELETE ${path} failed: ${res.status} ${res.statusText}`);
  }
}

// ── Auth API ─────────────────────────────────────────────────────

export const auth = {
  login: async (username: string, password: string): Promise<string> => {
    const res = await fetch(`${BASE}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });
    if (!res.ok) {
      throw new Error('Invalid username or password');
    }
    const data = await res.json();
    setToken(data.token);
    return data.token;
  },
  logout: async (): Promise<void> => {
    try {
      await fetch(`${BASE}/auth/logout`, {
        method: 'POST',
        headers: { ...authHeaders() },
      });
    } finally {
      clearToken();
      window.location.hash = '#/login';
    }
  },
  isAuthenticated: (): boolean => {
    return getToken() !== null;
  },
  getToken,
};

// ── Resource APIs ────────────────────────────────────────────────

export const projects = {
  list: () => get<Project[]>('/projects'),
  get: (id: string) => get<Project>(`/projects/${id}`),
  create: (data: CreateProject) => post<Project>('/projects', data),
  update: (id: string, data: UpdateProject) => put<Project>(`/projects/${id}`, data),
  delete: (id: string) => del(`/projects/${id}`),
};

export const projectApps = {
  start: (projectId: string) => post<AppStatus>(`/projects/${projectId}/app/start`, {}),
  stop: (projectId: string) => post<void>(`/projects/${projectId}/app/stop`, {}),
  status: (projectId: string) => get<AppStatus>(`/projects/${projectId}/app/status`),
};

export const teams = {
  list: (projectId: string) => get<Team[]>(`/projects/${projectId}/teams`),
  get: (projectId: string, id: string) => get<Team>(`/projects/${projectId}/teams/${id}`),
  create: (projectId: string, data: CreateTeam) => post<Team>(`/projects/${projectId}/teams`, data),
  delete: (projectId: string, id: string) => del(`/projects/${projectId}/teams/${id}`),
  templates: () => get<Team[]>('/teams/templates'),
  projectTemplates: (projectId: string) => get<Team[]>(`/projects/${projectId}/teams/templates`),
  cloneTemplate: (projectId: string, templateId: string) => post<Team>(`/projects/${projectId}/teams/from-template/${templateId}`, {}),
  activate: (projectId: string, id: string) => put<Team>(`/projects/${projectId}/teams/${id}/activate`, {}),
  deactivate: (projectId: string, id: string) => put<Team>(`/projects/${projectId}/teams/${id}/deactivate`, {}),
  updateConfig: (projectId: string, id: string, types: string[]) => put<Team>(`/projects/${projectId}/teams/${id}/config`, { types }),
  status: (projectId: string, id: string) => get<TeamStatus>(`/projects/${projectId}/teams/${id}/status`),
  slots: {
    list: (teamId: string) => get<TeamAgentSlot[]>(`/teams/${teamId}/slots`),
    create: (teamId: string, data: CreateTeamAgentSlot) => post<TeamAgentSlot>(`/teams/${teamId}/slots`, data),
    update: (teamId: string, id: string, data: UpdateTeamAgentSlot) => put<TeamAgentSlot>(`/teams/${teamId}/slots/${id}`, data),
    delete: (teamId: string, id: string) => del(`/teams/${teamId}/slots/${id}`),
  },
};

export const issues = {
  list: (projectId: string) => get<Issue[]>(`/projects/${projectId}/issues`),
  get: (projectId: string, id: string) => get<Issue>(`/projects/${projectId}/issues/${id}`),
  create: (projectId: string, data: CreateIssue) => post<Issue>(`/projects/${projectId}/issues`, data),
  update: (projectId: string, id: string, data: UpdateIssue) => patch<Issue>(`/projects/${projectId}/issues/${id}`, data),
  claim: (projectId: string, id: string, agentId: string) => post<void>(`/projects/${projectId}/issues/${id}/claim`, { agent_session_id: agentId }),
  unclaim: (projectId: string, id: string) => post<void>(`/projects/${projectId}/issues/${id}/unclaim`, {}),
  ready: (projectId: string) => get<Issue[]>(`/projects/${projectId}/issues/ready`),
  delete: (projectId: string, id: string) => del(`/projects/${projectId}/issues/${id}`),
  updateStatus: (projectId: string, id: string, status: string) => patch<Issue>(`/projects/${projectId}/issues/${id}`, { status }),
  attachments: {
    list: (projectId: string, issueId: string) =>
      get<Attachment[]>(`/projects/${projectId}/issues/${issueId}/attachments`),
    upload: async (projectId: string, issueId: string, file: File): Promise<Attachment> => {
      const form = new FormData();
      form.append('file', file);
      const res = await fetch(`${BASE}/projects/${projectId}/issues/${issueId}/attachments`, {
        method: 'POST',
        headers: { ...authHeaders() },
        body: form,
      });
      if (!res.ok) {
        handle401(res);
        throw new Error(`Upload failed: ${res.status}`);
      }
      return res.json();
    },
    downloadUrl: (attachmentId: string) => `${BASE}/attachments/${attachmentId}/download`,
  },
};

export const agents = {
  list: () => get<AgentInfo[]>('/agents'),
  get: (id: string) => get<AgentInfo>(`/agents/${id}`),
  spawn: (data: SpawnAgentRequest) => post<AgentInfo>('/agents/spawn', data),
  stop: (id: string) => post<void>(`/agents/${id}/stop`, {}),
};

export const workflows = {
  definitions: {
    list: (projectId: string) => get<WorkflowDefinition[]>(`/projects/${projectId}/workflows`),
    get: (projectId: string, id: string) => get<WorkflowDefinition>(`/projects/${projectId}/workflows/${id}`),
    create: (projectId: string, data: CreateWorkflowDef) => post<WorkflowDefinition>(`/projects/${projectId}/workflows`, data),
  },
  instances: {
    list: (workflowId: string) => get<WorkflowInstance[]>(`/workflows/${workflowId}/instances`),
    create: (workflowId: string, data: CreateInstance) => post<WorkflowInstance>(`/workflows/${workflowId}/instances`, data),
    approveGate: (workflowId: string, instanceId: string, stageId: string) =>
      post(`/workflows/${workflowId}/instances/${instanceId}/stages/${stageId}/approve`, {}),
    pause: (workflowId: string, instanceId: string) =>
      post<WorkflowInstance>(`/workflows/${workflowId}/instances/${instanceId}/pause`, {}),
    resume: (workflowId: string, instanceId: string) =>
      post<WorkflowInstance>(`/workflows/${workflowId}/instances/${instanceId}/resume`, {}),
    cancel: (workflowId: string, instanceId: string) =>
      post<WorkflowInstance>(`/workflows/${workflowId}/instances/${instanceId}/cancel`, {}),
  },
};

export const dashboard = {
  stats: () => get<DashboardStats>('/dashboard'),
  activity: (limit = 50, offset = 0) => get<ActivityLogEntry[]>(`/dashboard/activity?limit=${limit}&offset=${offset}`),
  metrics: (days = 7) => get<MetricsResponse>(`/dashboard/metrics?days=${days}`),
  system: () => get<SystemHealth>(`/dashboard/system`),
};

export const filesystem = {
  browse: (path: string, includeFiles = false) =>
    get<BrowseResponse>(`/filesystem/browse?path=${encodeURIComponent(path)}&include_files=${includeFiles}`),
};

export interface SshTestRequest {
  host: string;
  port?: number;
  username: string;
  password?: string;
  ssh_key?: string;
  proxy_config_id?: string;
}

export interface RemoteBrowseRequest extends SshTestRequest {
  path?: string;
}

export interface RemoteBrowseResponse {
  path: string;
  entries: Array<{ name: string; type: 'directory' | 'file' }>;
  git_remote: string | null;
  error?: string;
}

export const mounts = {
  list: () => get<MountConfig[]>('/mounts'),
  get: (id: string) => get<MountConfig>(`/mounts/${id}`),
  create: (data: CreateMountConfig) => post<MountConfig>('/mounts', data),
  update: (id: string, data: CreateMountConfig) => put<MountConfig>(`/mounts/${id}`, data),
  delete: (id: string) => del(`/mounts/${id}`),
  duplicate: (id: string) => post<MountConfig>(`/mounts/${id}/duplicate`, {}),
  mount: (id: string) => post<MountConfig>(`/mounts/${id}/mount`, {}),
  unmount: (id: string) => post<MountConfig>(`/mounts/${id}/unmount`, {}),
  status: (id: string) => get<{ status: string }>(`/mounts/${id}/status`),
  testSsh: (data: SshTestRequest) => post<{ success: boolean; message?: string; error?: string }>('/mounts/test-ssh', data),
  browseRemote: (data: RemoteBrowseRequest) => post<RemoteBrowseResponse>('/mounts/browse-remote', data),
};

export const settings = {
  list: () => get<Setting[]>('/settings'),
  get: (key: string) => get<Setting>(`/settings/${key}`),
  upsert: (key: string, data: UpsertSetting) => put<Setting>(`/settings/${key}`, data),
  delete: (key: string) => del(`/settings/${key}`),
};

export const proxyConfigs = {
  list: () => get<ProxyConfigResponse[]>('/proxy-configs'),
  get: (id: string) => get<ProxyConfigResponse>(`/proxy-configs/${id}`),
  create: (data: CreateProxyConfig) => post<ProxyConfigResponse>('/proxy-configs', data),
  update: (id: string, data: UpdateProxyConfig) => put<ProxyConfigResponse>(`/proxy-configs/${id}`, data),
  delete: (id: string) => del(`/proxy-configs/${id}`),
  test: (id: string) => post<TestConnectionResult>(`/proxy-configs/${id}/test`, {}),
};

export interface MergeQueueEntry {
  id: string;
  project_id: string;
  branch_name: string;
  agent_session_id: string | null;
  issue_id: string | null;
  team_id: string | null;
  status: string; // pending, merging, conflicted, resolving, resolved, merged, failed
  conflict_files: string; // JSON array
  resolver_agent_id: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export const runtimes = {
  list: () => get<RuntimeInfo[]>('/runtimes'),
};

export interface MergeQueueDiff {
  branch: string;
  target: string;
  diff: string;
  conflict_files: string[];
}

export const mergeQueue = {
  list: (projectId: string) => get<MergeQueueEntry[]>(`/projects/${projectId}/merge-queue`),
  approve: (projectId: string, id: string) => post<MergeQueueEntry>(`/projects/${projectId}/merge-queue/${id}/approve`, {}),
  resolve: (projectId: string, id: string) => post<MergeQueueEntry>(`/projects/${projectId}/merge-queue/${id}/resolve`, {}),
  diff: (projectId: string, id: string) => get<MergeQueueDiff>(`/projects/${projectId}/merge-queue/${id}/diff`),
  reject: (projectId: string, id: string) => post<MergeQueueEntry>(`/projects/${projectId}/merge-queue/${id}/reject`, {}),
};

export interface LoomEntry {
  id: string;
  timestamp: string;
  agent_id: string | null;
  team_id: string;
  project_id: string;
  workflow_instance_id: string | null;
  entry_type: string;
  content: string;
  role?: string;
  runtime?: string;
  model?: string;
}

export const loom = {
  recent: (limit = 50) => get<LoomEntry[]>(`/loom?limit=${limit}`),
  byProject: (projectId: string, limit = 50) => get<LoomEntry[]>(`/projects/${projectId}/loom?limit=${limit}`),
  byTeam: (teamId: string, limit = 50) => get<LoomEntry[]>(`/teams/${teamId}/loom?limit=${limit}`),
};

// ── Swarm status ─────────────────────────────────────────────────

export interface SwarmAgent {
  session_id: string;
  role: string;
  runtime: string;
  state: string;
  issue_id: string | null;
  issue_title: string | null;
}

export interface SwarmStatus {
  coordination_mode: string;
  active_agents: number;
  idle_agents: number;
  total_agents: number;
  task_pool_depth: number;
  throughput_issues_per_hour: number;
  scaling_recommendation: string;
  agents: SwarmAgent[];
}

export const swarm = {
  status: (projectId: string) => get<SwarmStatus>(`/projects/${projectId}/swarm-status`),
};

// ── Prompt Templates ─────────────────────────────────────────────

export interface PromptTemplate {
  id: string;
  name: string;
  template_type: 'role' | 'skill';
  content: string;
  project_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface PromptTemplateAssignment {
  id: string;
  role: string;
  template_id: string;
  priority: number;
  created_at: string;
  template_name?: string;
  template_type?: string;
}

export const promptTemplates = {
  list: (projectId?: string) =>
    get<PromptTemplate[]>(`/prompt-templates${projectId ? `?project_id=${projectId}` : ''}`),
  get: (id: string) => get<PromptTemplate>(`/prompt-templates/${id}`),
  create: (data: { name: string; template_type?: string; content: string; project_id?: string }) =>
    post<PromptTemplate>('/prompt-templates', data),
  update: (id: string, data: { name?: string; content?: string }) =>
    put<PromptTemplate>(`/prompt-templates/${id}`, data),
  delete: (id: string) => del(`/prompt-templates/${id}`),
  // Assignments
  listAssignments: (role: string) =>
    get<PromptTemplateAssignment[]>(`/prompt-templates/roles/${encodeURIComponent(role)}/assignments`),
  createAssignment: (data: { role: string; template_id: string; priority?: number }) =>
    post<PromptTemplateAssignment>('/prompt-templates/assignments', data),
  deleteAssignment: (id: string) => del(`/prompt-templates/assignments/${id}`),
  // Build full prompt for a role
  buildPrompt: (role: string, projectId?: string) =>
    get<{ role: string; prompt: string }>(`/prompt-templates/roles/${encodeURIComponent(role)}/build${projectId ? `?project_id=${projectId}` : ''}`),
};

export const sync = {
  trigger: (projectId: string) => post<SyncStatus>(`/projects/${projectId}/sync`, {}),
  status: (projectId: string) => get<SyncStatus>(`/projects/${projectId}/sync/status`),
  history: (projectId: string) => get<SyncSnapshot[]>(`/projects/${projectId}/sync/history`),
  diff: (projectId: string, changeId: string) => get<string>(`/projects/${projectId}/sync/diff/${changeId}`),
  restore: (projectId: string, changeId: string) => post<void>(`/projects/${projectId}/sync/restore`, { change_id: changeId }),
  browseFiles: (projectId: string, path?: string) =>
    get<BrowseEntry[]>(`/projects/${projectId}/files${path ? `?path=${encodeURIComponent(path)}` : ''}`),
  readFile: (projectId: string, path: string) =>
    get<string>(`/projects/${projectId}/files/content?path=${encodeURIComponent(path)}`),
};

// ── v2: Quality Tiers ─────────────────────────────────────────────

export interface QualityTier {
  tier: number;
  label: string;
  example_models: string;
  cost_range: string;
  max_context_tokens: number;
  max_output_tokens: number;
}

export interface TierRange {
  tier_floor: number;
  tier_ceiling: number;
}

export const qualityTiers = {
  list: () => get<QualityTier[]>('/quality-tiers'),
  getProject: (projectId: string) => get<TierRange>(`/projects/${projectId}/quality`),
  setProject: (projectId: string, data: { tier_floor?: number; tier_ceiling?: number }) =>
    put<TierRange>(`/projects/${projectId}/quality`, data),
  resetProject: (projectId: string) => post<TierRange>(`/projects/${projectId}/quality/reset`, {}),
  getTeam: (teamId: string) => get<TierRange>(`/teams/${teamId}/quality`),
  setTeam: (teamId: string, data: { tier_floor?: number; tier_ceiling?: number }) =>
    put<TierRange>(`/teams/${teamId}/quality`, data),
};

// ── v2: Cost Tracking ─────────────────────────────────────────────

export interface CostSummary {
  total_tokens: number;
  total_cost_usd: number;
  task_count: number;
  failure_count: number;
  by_role: Record<string, number>;
  by_model: Record<string, number>;
}

export interface DailySpend {
  date: string;
  cost_usd: number;
  tokens: number;
}

export const costTracking = {
  summary: (projectId: string, days?: number) =>
    get<CostSummary>(`/projects/${projectId}/costs/summary${days ? `?days=${days}` : ''}`),
  daily: (projectId: string, days?: number) =>
    get<DailySpend[]>(`/projects/${projectId}/costs/daily${days ? `?days=${days}` : ''}`),
  aggregate: (projectId: string) => post<void>(`/projects/${projectId}/costs/aggregate`, {}),
};

// ── v2: Coordinator ───────────────────────────────────────────────

export interface CoordinatorMemory {
  id: string;
  project_id: string;
  state: string;
  session_id: string | null;
  last_active_at: string | null;
  created_at: string;
}

export const coordinator = {
  get: (projectId: string) => get<CoordinatorMemory>(`/projects/${projectId}/coordinator`),
  wake: (projectId: string, sessionId: string) =>
    post<CoordinatorMemory>(`/projects/${projectId}/coordinator/wake`, { session_id: sessionId }),
  sleep: (projectId: string) =>
    post<CoordinatorMemory>(`/projects/${projectId}/coordinator/sleep`, {}),
  list: () => get<CoordinatorMemory[]>('/coordinators'),
};

// ── v2: Routing Overrides ─────────────────────────────────────────

export interface RoutingOverride {
  id: string;
  project_id: string;
  role: string;
  task_type: string;
  from_model: string | null;
  to_model: string;
  to_tier: number;
  reason: string;
  confidence: number;
  status: string;
  evidence: string;
  observations: number;
  created_at: string;
  resolved_at: string | null;
}

export const routingOverrides = {
  list: (projectId: string) => get<RoutingOverride[]>(`/projects/${projectId}/routing-overrides`),
  detect: (projectId: string) => post<RoutingOverride[]>(`/projects/${projectId}/routing-overrides/detect`, {}),
  accept: (id: string) => post<RoutingOverride>(`/routing-overrides/${id}/accept`, {}),
  reject: (id: string) => post<RoutingOverride>(`/routing-overrides/${id}/reject`, {}),
};

// ── v2: Team Role Overrides ───────────────────────────────────────

export interface TeamRoleOverride {
  id: string;
  team_id: string;
  role: string;
  runtime: string | null;
  provider: string | null;
  model: string | null;
  is_user_set: boolean;
  created_at: string;
  updated_at: string;
}

export const teamRoleOverrides = {
  list: (teamId: string) => get<TeamRoleOverride[]>(`/teams/${teamId}/role-overrides`),
  set: (teamId: string, data: { role: string; runtime?: string; provider?: string; model?: string }) =>
    post<TeamRoleOverride>(`/teams/${teamId}/role-overrides`, data),
  clear: (teamId: string, role: string) => del(`/teams/${teamId}/role-overrides/${encodeURIComponent(role)}`),
};

// ── v2: Performance Log ───────────────────────────────────────────

export interface PerformanceLogEntry {
  id: string;
  project_id: string;
  role: string;
  runtime: string;
  provider: string;
  model: string;
  tier: number;
  task_type: string;
  task_complexity: number;
  outcome: string;
  failure_reason: string | null;
  tokens_used: number;
  cost_usd: number;
  duration_seconds: number;
  retries: number;
  escalated_from: string | null;
  complexity_score: number | null;
  files_touched: string | null;
  created_at: string;
}

export interface ModelStats {
  model: string;
  role: string;
  total: number;
  successes: number;
  failures: number;
  success_rate: number;
  avg_cost: number;
  avg_duration: number;
}

export const performanceLog = {
  list: (projectId: string, params?: { role?: string; model?: string; outcome?: string; days?: number; limit?: number }) => {
    const q = new URLSearchParams();
    if (params?.role) q.set('role', params.role);
    if (params?.model) q.set('model', params.model);
    if (params?.outcome) q.set('outcome', params.outcome);
    if (params?.days) q.set('days', String(params.days));
    if (params?.limit) q.set('limit', String(params.limit));
    const qs = q.toString();
    return get<PerformanceLogEntry[]>(`/projects/${projectId}/performance${qs ? `?${qs}` : ''}`);
  },
  stats: (projectId: string, days?: number) =>
    get<ModelStats[]>(`/projects/${projectId}/performance/stats${days ? `?days=${days}` : ''}`),
};
