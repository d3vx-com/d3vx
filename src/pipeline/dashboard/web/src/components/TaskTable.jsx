import { useState, useCallback } from 'react';
import StatusBadge from './StatusBadge';
import TaskDetail from './TaskDetail';

/* ------------------------------------------------------------------ */
/*  Main task table component                                         */
/* ------------------------------------------------------------------ */

export default function TaskTable({ tasks, onRefresh }) {
  const [sortKey, setSortKey] = useState('created_at');
  const [sortDir, setSortDir] = useState('desc');
  const [selectedTask, setSelectedTask] = useState(null);

  const toggleSort = useCallback(
    (key) => {
      if (sortKey === key) {
        setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
      } else {
        setSortKey(key);
        setSortDir('desc');
      }
    },
    [sortKey]
  );

  const sorted = [...tasks].sort((a, b) => {
    const av = a[sortKey] ?? '';
    const bv = b[sortKey] ?? '';
    const cmp = av < bv ? -1 : av > bv ? 1 : 0;
    return sortDir === 'asc' ? cmp : -cmp;
  });

  if (tasks.length === 0) {
    return <div className="empty">no tasks match filter</div>;
  }

  const cols = [
    { key: 'id', label: 'id', w: 90 },
    { key: 'title', label: 'title', w: 'auto' },
    { key: 'state', label: 'state', w: 100 },
    { key: 'phase', label: 'phase', w: 90 },
    { key: 'cost_usd', label: 'cost', w: 70 },
    { key: 'duration_secs', label: 'time', w: 70 },
    { key: '', label: 'actions', w: 110 },
  ];

  const thClass = (c) => {
    const base = 'th';
    if (sortKey === c.key) return `${base} sorted`;
    return base;
  };

  return (
    <>
      <table className="task-table">
        <thead>
          <tr>
            {cols.map((c) => (
              <th
                key={c.key}
                className={thClass(c)}
                style={{ width: c.w !== 'auto' ? c.w : undefined }}
                onClick={() => c.key && toggleSort(c.key)}
                title={c.key ? `Sort by ${c.key}` : ''}
              >
                {c.label}
                {sortKey === c.key && <span className="sort-arrow">{sortDir === 'asc' ? '▴' : '▾'}</span>}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.map((task) => (
            <tr
              key={task.id}
              onClick={() => setSelectedTask(task)}
              className="task-row"
            >
              <td className="mono">{task.id}</td>
              <td className="title-cell">{task.title}</td>
              <td>
                <StatusBadge state={task.state} />
              </td>
              <td className="mono dim">{task.phase || '—'}</td>
              <td className="mono cost">${task.cost_usd.toFixed(2)}</td>
              <td className="mono dim">{formatDuration(task.duration_secs)}</td>
              <td className="actions-cell">
                <ActionButtons task={task} onRefresh={onRefresh} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {selectedTask && (
        <TaskDetail
          task={selectedTask}
          onClose={() => setSelectedTask(null)}
          onRefresh={onRefresh}
        />
      )}
    </>
  );
}

/* ------------------------------------------------------------------ */
/*  Per-row action buttons                                            */
/* ------------------------------------------------------------------ */

function ActionButtons({ task, onRefresh }) {
  const isTerminal = ['SPAWNING', 'RESEARCH', 'PLAN', 'IMPLEMENT', 'REVIEW', 'TEST', 'FIX', 'INVESTIGATE'].includes(task.state);

  const handleKill = async (e) => {
    e.stopPropagation();
    if (!confirm(`Kill ${task.id}?`)) return;
    await fetch(`/api/tasks/${task.id}/kill`, { method: 'POST' });
    onRefresh();
  };

  const handleRetry = async (e) => {
    e.stopPropagation();
    await fetch(`/api/tasks/${task.id}/retry`, { method: 'POST' });
    onRefresh();
  };

  if (task.state === 'FAILED') {
    return (
      <div className="row-actions" onClick={(e) => e.stopPropagation()}>
        <button className="btn-retry" onClick={handleRetry}>retry</button>
      </div>
    );
  }

  if (isTerminal) {
    return (
      <div className="row-actions" onClick={(e) => e.stopPropagation()}>
        <button className="btn-kill" onClick={handleKill}>kill</button>
      </div>
    );
  }

  return null;
}

/* ------------------------------------------------------------------ */
/*  Helpers                                                           */
/* ------------------------------------------------------------------ */

function formatDuration(secs) {
  if (!secs || secs === 0) return '—';
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`;
}
