import { useEffect, useMemo, useState, type CSSProperties } from "react";
import {
  Activity,
  Bot,
  Check,
  CheckCircle2,
  ChevronDown,
  Circle,
  Clock3,
  Cpu,
  Database,
  FileDiff,
  GitBranch,
  GitFork,
  Layers3,
  LayoutDashboard,
  LockKeyhole,
  MessageSquareText,
  Network,
  PanelLeftClose,
  Play,
  Plus,
  Puzzle,
  Search,
  Settings2,
  ShieldCheck,
  Sparkles,
  Square,
  Waypoints,
  Wrench,
  Zap,
} from "lucide-react";
import { getHostInfo, type HostInfo } from "./bridge.js";

type RunState = "running" | "queued" | "blocked" | "done";

interface TaskView {
  id: string;
  title: string;
  role: string;
  state: RunState;
  model: string;
  effort: string;
  branch: string;
  duration?: string;
}

interface ProjectView {
  id: string;
  name: string;
  path: string;
  accent: string;
  branch: string;
  changed: number;
  active: number;
  queued: number;
  tasks: TaskView[];
}

const projects: ProjectView[] = [
  {
    id: "vea",
    name: "Vea Core",
    path: "~/Documents/vea-ai",
    accent: "#8b7cff",
    branch: "main",
    changed: 42,
    active: 2,
    queued: 3,
    tasks: [
      {
        id: "VEA-14",
        title: "Implement deterministic task scheduler",
        role: "implementation",
        state: "running",
        model: "Codex",
        effort: "High",
        branch: "vea/vea-14-scheduler",
        duration: "08:42",
      },
      {
        id: "VEA-15",
        title: "Review worktree lease invariants",
        role: "review",
        state: "running",
        model: "Claude",
        effort: "Medium",
        branch: "vea/vea-15-review",
        duration: "03:18",
      },
      {
        id: "VEA-16",
        title: "Add quota reservation ledger",
        role: "implementation",
        state: "queued",
        model: "Gemini",
        effort: "Medium",
        branch: "awaiting lease",
      },
      {
        id: "VEA-17",
        title: "Validate adapter event contract",
        role: "validation",
        state: "blocked",
        model: "Auto",
        effort: "Low",
        branch: "depends on VEA-14",
      },
      {
        id: "VEA-13",
        title: "Define normalized effort vocabulary",
        role: "planning",
        state: "done",
        model: "Claude",
        effort: "High",
        branch: "merged locally",
        duration: "12:06",
      },
    ],
  },
  {
    id: "storefront",
    name: "Storefront",
    path: "~/Code/storefront",
    accent: "#4fd1a1",
    branch: "develop",
    changed: 8,
    active: 1,
    queued: 1,
    tasks: [
      {
        id: "WEB-82",
        title: "Fix checkout state race",
        role: "implementation",
        state: "running",
        model: "Claude",
        effort: "High",
        branch: "vea/web-82-checkout",
        duration: "05:11",
      },
      {
        id: "WEB-83",
        title: "Add regression coverage",
        role: "validation",
        state: "queued",
        model: "Codex",
        effort: "Medium",
        branch: "awaiting dependency",
      },
    ],
  },
  {
    id: "ios",
    name: "Orbit iOS",
    path: "~/Code/orbit-ios",
    accent: "#f2b45f",
    branch: "main",
    changed: 0,
    active: 0,
    queued: 2,
    tasks: [
      {
        id: "IOS-31",
        title: "Plan offline sync migration",
        role: "planning",
        state: "queued",
        model: "Claude",
        effort: "Max",
        branch: "ready",
      },
      {
        id: "IOS-32",
        title: "Audit migration rollback",
        role: "review",
        state: "blocked",
        model: "Auto",
        effort: "High",
        branch: "depends on IOS-31",
      },
    ],
  },
];

const statusLabel: Record<RunState, string> = {
  running: "Running",
  queued: "Queued",
  blocked: "Blocked",
  done: "Complete",
};

function StateIcon({ state }: { state: RunState }) {
  if (state === "running") return <Activity size={14} aria-hidden="true" />;
  if (state === "queued") return <Clock3 size={14} aria-hidden="true" />;
  if (state === "blocked") return <LockKeyhole size={14} aria-hidden="true" />;
  return <CheckCircle2 size={14} aria-hidden="true" />;
}

export function App() {
  const [selectedProjectId, setSelectedProjectId] = useState("vea");
  const [selectedTaskId, setSelectedTaskId] = useState("VEA-14");
  const [view, setView] = useState<"graph" | "board">("graph");
  const [hostInfo, setHostInfo] = useState<HostInfo | null>(null);

  useEffect(() => {
    void getHostInfo().then(setHostInfo);
  }, []);

  const project = useMemo(
    () => projects.find((entry) => entry.id === selectedProjectId) ?? projects[0]!,
    [selectedProjectId],
  );
  const task = project.tasks.find((entry) => entry.id === selectedTaskId) ?? project.tasks[0]!;
  const hostActive = hostInfo?.securityBoundary === "rust-host";

  function selectProject(projectId: string): void {
    const nextProject = projects.find((entry) => entry.id === projectId);
    if (!nextProject) return;
    setSelectedProjectId(projectId);
    setSelectedTaskId(nextProject.tasks[0]?.id ?? "");
  }

  return (
    <div className="app-shell">
      <header className="titlebar">
        <div className="brand-mark" aria-label="Vea home">
          <div className="brand-glyph">
            <Waypoints size={18} />
          </div>
          <span>Vea</span>
          <span className="pre-release">ALPHA</span>
        </div>
        <div className="global-search" role="search">
          <Search size={15} aria-hidden="true" />
          <input
            aria-label="Search projects and tasks"
            placeholder="Search projects, tasks, runs…"
          />
          <kbd>⌘ K</kbd>
        </div>
        <div className="title-actions">
          <div
            className={`host-status ${hostInfo && !hostActive ? "mock" : ""}`}
            title={hostInfo?.securityBoundary ?? "Connecting to host"}
          >
            <span className="status-dot" />
            {!hostInfo
              ? "Connecting"
              : hostActive
                ? `Protocol v${hostInfo.protocolVersion}`
                : "Browser demo"}
          </div>
          <button className="icon-button" aria-label="Open settings">
            <Settings2 size={17} />
          </button>
        </div>
      </header>

      <aside className="project-rail" aria-label="Projects">
        <div className="rail-heading">
          <span>Projects</span>
          <button className="icon-button small" aria-label="Add project">
            <Plus size={15} />
          </button>
        </div>
        <nav className="project-list">
          {projects.map((entry) => (
            <button
              key={entry.id}
              className={`project-item ${entry.id === project.id ? "selected" : ""}`}
              onClick={() => selectProject(entry.id)}
              aria-current={entry.id === project.id ? "page" : undefined}
            >
              <span
                className="project-monogram"
                style={{ "--accent": entry.accent } as CSSProperties}
              >
                {entry.name.slice(0, 1)}
              </span>
              <span className="project-copy">
                <strong>{entry.name}</strong>
                <small>
                  <GitBranch size={11} /> {entry.branch}
                </small>
              </span>
              {entry.active > 0 && <span className="run-count">{entry.active}</span>}
            </button>
          ))}
        </nav>
        <div className="rail-section">
          <span className="rail-section-label">Control plane</span>
          <button>
            <LayoutDashboard size={15} /> Overview
          </button>
          <button>
            <Cpu size={15} /> Providers <span className="healthy-pill">4</span>
          </button>
          <button>
            <Puzzle size={15} /> Extensions
          </button>
          <button>
            <Database size={15} /> Audit log
          </button>
        </div>
        <div className={`rail-footer ${hostInfo && !hostActive ? "mock" : ""}`}>
          <ShieldCheck size={16} />
          <div>
            <strong>{hostActive ? "Local only" : "Browser mock"}</strong>
            <small>{hostActive ? "Rust policy host" : "No privileged host"}</small>
          </div>
        </div>
      </aside>

      <main className="workspace">
        <section className="project-header">
          <div>
            <div className="eyebrow">
              <span className="trust-dot" /> Trusted project
            </div>
            <h1>{project.name}</h1>
            <p>{project.path}</p>
          </div>
          <div className="project-metrics" aria-label="Project run metrics">
            <div>
              <strong>{project.active}</strong>
              <span>active</span>
            </div>
            <div>
              <strong>{project.queued}</strong>
              <span>queued</span>
            </div>
            <div>
              <strong>{project.changed}</strong>
              <span>changes</span>
            </div>
          </div>
          <button className="primary-button">
            <Plus size={15} /> New task
          </button>
        </section>

        <section className="task-panel" aria-label="Task graph">
          <div className="panel-toolbar">
            <div className="segmented" role="group" aria-label="Task view">
              <button className={view === "graph" ? "active" : ""} onClick={() => setView("graph")}>
                <GitFork size={14} /> Graph
              </button>
              <button className={view === "board" ? "active" : ""} onClick={() => setView("board")}>
                <Layers3 size={14} /> Board
              </button>
            </div>
            <div className="toolbar-copy">
              <strong>Release foundation</strong>
              <span>5 tasks · revision 3</span>
            </div>
            <button className="filter-button">
              All states <ChevronDown size={13} />
            </button>
            <button className="icon-button">
              <PanelLeftClose size={16} />
            </button>
          </div>

          <div className="task-table" role="list">
            <div className="task-table-head" aria-hidden="true">
              <span>Task</span>
              <span>Route</span>
              <span>Worktree</span>
              <span>Status</span>
            </div>
            {project.tasks.map((entry, index) => (
              <button
                className={`task-row ${entry.id === task.id ? "selected" : ""}`}
                key={entry.id}
                onClick={() => setSelectedTaskId(entry.id)}
                role="listitem"
              >
                <span className="task-identity">
                  <span className="graph-line">
                    <Circle size={10} fill={index === 0 ? "currentColor" : "transparent"} />
                  </span>
                  <span>
                    <small>
                      {entry.id} · {entry.role}
                    </small>
                    <strong>{entry.title}</strong>
                  </span>
                </span>
                <span className="route-cell">
                  <Bot size={14} />
                  <span>
                    <strong>{entry.model}</strong>
                    <small>{entry.effort} effort</small>
                  </span>
                </span>
                <span className="branch-cell">
                  <GitBranch size={13} /> {entry.branch}
                </span>
                <span className={`state-pill ${entry.state}`}>
                  <StateIcon state={entry.state} /> {entry.duration ?? statusLabel[entry.state]}
                </span>
              </button>
            ))}
          </div>
        </section>

        <section className="session-panel" aria-label={`Session for ${task.title}`}>
          <div className="session-header">
            <div className="agent-avatar">
              <Sparkles size={17} />
            </div>
            <div>
              <strong>{task.title}</strong>
              <span>
                {task.id} · {task.model} · {task.effort} effort
              </span>
            </div>
            <div className="session-actions">
              <button className="quiet-button">
                <Square size={12} fill="currentColor" /> Stop
              </button>
              <button className="icon-button">
                <FileDiff size={16} />
              </button>
            </div>
          </div>
          <div className="transcript">
            <div className="message user-message">
              <div className="message-label">Task brief</div>
              <p>
                Implement deterministic cross-project scheduling. Preserve fair access to provider
                capacity and block overlapping write scopes.
              </p>
              <div className="scope-chips">
                <span>packages/scheduler/**</span>
                <span>high effort</span>
                <span>max 30 turns</span>
              </div>
            </div>
            <div className="message agent-message">
              <div className="message-label">
                <Bot size={13} /> Agent
              </div>
              <p>
                I mapped the scheduling constraints and found three invariants to encode before
                selecting work:
              </p>
              <ol>
                <li>Dependencies and trust must fail closed before route reservation.</li>
                <li>Active and newly selected write scopes must be checked together.</li>
                <li>Provider and account concurrency are separate limits.</li>
              </ol>
              <div className="tool-card">
                <div>
                  <Wrench size={14} />
                  <strong>Inspect files</strong>
                  <span className="tool-state">
                    <Check size={12} /> complete
                  </span>
                </div>
                <code>packages/scheduler/src/index.ts · packages/domain/src/index.ts</code>
              </div>
              <div className="thinking-row">
                <span className="pulse" /> Evaluating fairness property tests…
              </div>
            </div>
          </div>
          <div className="composer">
            <button className="icon-button">
              <Plus size={17} />
            </button>
            <textarea
              aria-label="Send follow-up instruction"
              placeholder="Steer the agent or add context…"
              rows={1}
            />
            <button className="route-button">
              <Zap size={13} /> Auto · High <ChevronDown size={12} />
            </button>
            <button className="send-button" aria-label="Send message">
              <Play size={15} fill="currentColor" />
            </button>
          </div>
        </section>
      </main>

      <aside className="inspector" aria-label="Run inspector">
        <div className="inspector-tabs">
          <button className="active">Run</button>
          <button>
            Diff <span>12</span>
          </button>
          <button>Audit</button>
        </div>
        <section className="inspector-section">
          <div className="section-heading">
            <span>Route decision</span>
            <span className="decision-id">#7F2A</span>
          </div>
          <div className="chosen-route">
            <div className="provider-logo">O</div>
            <div>
              <strong>Codex · coding-pro</strong>
              <span>API account · High effort</span>
            </div>
            <CheckCircle2 size={16} />
          </div>
          <div className="score-row">
            <span>Capability fit</span>
            <div>
              <i style={{ width: "100%" }} />
            </div>
            <strong>1.00</strong>
          </div>
          <div className="score-row">
            <span>Budget headroom</span>
            <div>
              <i style={{ width: "76%" }} />
            </div>
            <strong>.76</strong>
          </div>
          <div className="score-row">
            <span>Reliability</span>
            <div>
              <i style={{ width: "92%" }} />
            </div>
            <strong>.92</strong>
          </div>
          <button className="explain-button">
            <Network size={13} /> Why this route?
          </button>
        </section>
        <section className="inspector-section">
          <div className="section-heading">
            <span>Usage</span>
            <span>live</span>
          </div>
          <div className="usage-grid">
            <div>
              <strong>18.4k</strong>
              <span>input tokens</span>
            </div>
            <div>
              <strong>3.2k</strong>
              <span>output tokens</span>
            </div>
            <div>
              <strong>$0.42</strong>
              <span>estimated</span>
            </div>
            <div>
              <strong>8:42</strong>
              <span>elapsed</span>
            </div>
          </div>
          <div className="quota-note">
            <Activity size={14} />
            <span>
              <strong>API budget: 84% available</strong>Subscription quota is never inferred from
              token cost.
            </span>
          </div>
        </section>
        <section className="inspector-section">
          <div className="section-heading">
            <span>Isolation</span>
            <ShieldCheck size={14} />
          </div>
          <dl className="detail-list">
            <div>
              <dt>Worktree</dt>
              <dd>.vea/worktrees/vea-14</dd>
            </div>
            <div>
              <dt>Base</dt>
              <dd>main@2bc41ae</dd>
            </div>
            <div>
              <dt>Write scope</dt>
              <dd>packages/scheduler/**</dd>
            </div>
            <div>
              <dt>Policy</dt>
              <dd>{hostActive ? "local-safe-v1" : "not connected"}</dd>
            </div>
          </dl>
          <div className={`security-note ${hostInfo && !hostActive ? "mock" : ""}`}>
            <ShieldCheck size={15} />
            <p>
              <strong>{hostActive ? "Host policy active" : "Demo state only"}</strong>
              {hostActive
                ? "Worktree isolation prevents collisions; privileged actions still require Rust-host approval."
                : "The browser preview cannot access projects, credentials, tools, or host policy."}
            </p>
          </div>
        </section>
        <section className="inspector-section integrations">
          <div className="section-heading">
            <span>Context</span>
            <span>3 active</span>
          </div>
          <div>
            <MessageSquareText size={14} />
            <span>Skill</span>
            <strong>typescript-scheduler</strong>
          </div>
          <div>
            <Network size={14} />
            <span>MCP</span>
            <strong>disabled</strong>
          </div>
          <div>
            <Puzzle size={14} />
            <span>Plugin</span>
            <strong>none</strong>
          </div>
        </section>
      </aside>
    </div>
  );
}
