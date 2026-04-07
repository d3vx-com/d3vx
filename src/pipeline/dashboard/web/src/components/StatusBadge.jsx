/* ------------------------------------------------------------------ */
/*  Minimal status badge — monospace text with subtle color           */
/* ------------------------------------------------------------------ */

const STATE_MAP = {
  QUEUED:       { label: 'QUEUED',     color: 'var(--amber)',    bg: 'var(--amber-dim)' },
  SPAWNING:     { label: 'SPAWNING',   color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  PREPARE:      { label: 'PREPARE',    color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  RESEARCH:     { label: 'RESEARCH',   color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  PLAN:         { label: 'PLAN',       color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  IMPLEMENT:    { label: 'RUNNING',    color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  VALIDATE:     { label: 'VALIDATE',   color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  REVIEW:       { label: 'REVIEW',     color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  TEST:         { label: 'TEST',       color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  FIX:          { label: 'FIXING',     color: 'var(--accent)',   bg: 'var(--accent-dim)' },
  INVESTIGATE:  { label: 'INVESTIGATING', color: 'var(--accent)', bg: 'var(--accent-dim)' },
  DONE:         { label: 'DONE',       color: 'var(--green)',    bg: 'var(--green-dim)' },
  FAILED:       { label: 'FAILED',     color: 'var(--red)',      bg: 'var(--red-dim)' },
};

export default function StatusBadge({ state }) {
  const info = STATE_MAP[state] || { label: state, color: 'var(--text-muted)', bg: 'var(--border)' };

  return (
    <span
      className="status-badge"
      style={{ color: info.color, background: info.bg }}
    >
      {info.label}
    </span>
  );
}
