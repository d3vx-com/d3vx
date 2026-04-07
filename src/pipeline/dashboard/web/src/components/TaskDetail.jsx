import { useEffect, useState } from 'react';

/* ------------------------------------------------------------------ */
/*  Slide-in detail panel for a selected task                         */
/* ------------------------------------------------------------------ */

export default function TaskDetail({ task, onClose, onRefresh }) {
  const [detail, setDetail] = useState(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch(`/api/tasks/${task.id}`)
      .then((r) => r.json())
      .then((d) => {
        setDetail(d.data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [task.id]);

  const handleKill = async () => {
    await fetch(`/api/tasks/${task.id}/kill`, { method: 'POST' });
    onRefresh();
    onClose();
  };

  const handleRetry = async () => {
    await fetch(`/api/tasks/${task.id}/retry`, { method: 'POST' });
    onRefresh();
    onClose();
  };

  return (
    <div className="overlay" onClick={onClose}>
      <div className="detail-panel" onClick={(e) => e.stopPropagation()}>
        <header className="detail-header">
          <div>
            <span className="mono detail-id">{task.id}</span>
            <h2 className="detail-title">{task.title}</h2>
          </div>
          <button className="btn-close" onClick={onClose}>×</button>
        </header>

        {loading ? (
          <div className="detail-loading">loading…</div>
        ) : detail ? (
          <div className="detail-body">
            <dl className="detail-meta">
              <div>
                <dt>state</dt>
                <dd className="mono">{task.state}</dd>
              </div>
              <div>
                <dt>phase</dt>
                <dd className="mono">{detail.phase || '—'}</dd>
              </div>
              {detail.branch && (
                <div>
                  <dt>branch</dt>
                  <dd className="mono">{detail.branch}</dd>
                </div>
              )}
              <div>
                <dt>cost</dt>
                <dd className="mono">${detail.cost_usd.toFixed(4)}</dd>
              </div>
              <div>
                <dt>duration</dt>
                <dd className="mono">{formatDuration(detail.duration_secs)}</dd>
              </div>
              <div>
                <dt>messages</dt>
                <dd className="mono">{detail.message_count}</dd>
              </div>
            </dl>

            {task.error && <div className="error-block">{task.error}</div>}

            <div className="detail-actions">
              {task.state === 'FAILED' ? (
                <button className="btn btn-retry" onClick={handleRetry}>Retry</button>
              ) : (
                <button className="btn btn-kill" onClick={handleKill}>Kill Session</button>
              )}
            </div>
          </div>
        ) : (
          <div className="detail-loading">no details available</div>
        )}

        {/* Terminal placeholder — will integrate with actual terminal later */}
        <div className="terminal-placeholder">
          <code>terminal view coming soon</code>
        </div>
      </div>
    </div>
  );
}

function formatDuration(secs) {
  if (!secs || secs === 0) return '—';
  const s = Math.round(secs);
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
}
