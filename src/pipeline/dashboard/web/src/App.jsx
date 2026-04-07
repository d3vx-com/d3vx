import { useEffect, useState, useCallback, createContext, useContext } from 'react';
import CostBar from './components/CostBar';
import TaskTable from './components/TaskTable';

/* ------------------------------------------------------------------ */
/*  SSE Event Context                                                 */
/* ------------------------------------------------------------------ */

export const EventContext = createContext(null);

function EventProvider({ children }) {
  const [events, setEvents] = useState([]);

  useEffect(() => {
    const es = new EventSource('/api/events');
    es.onmessage = (e) => {
      try {
        setEvents((prev) => [...prev.slice(-128), JSON.parse(e.data)]);
      } catch {
        // ignore bad events
      }
    };
    es.onerror = () => es.reconnect();
    return () => es.close();
  }, []);

  return (
    <EventContext.Provider value={events}>
      {children}
    </EventContext.Provider>
  );
}

/* ------------------------------------------------------------------ */
/*  Stats hook                                                        */
/* ------------------------------------------------------------------ */

function useStats() {
  const [stats, setStats] = useState(null);

  useEffect(() => {
    const fetchStats = () =>
      fetch('/api/stats')
        .then((r) => r.json())
        .then((d) => setStats(d.data))
        .catch(() => {});

    fetchStats();
    const interval = setInterval(fetchStats, 10000);
    return () => clearInterval(interval);
  }, []);

  return stats;
}

/* ------------------------------------------------------------------ */
/*  SSE handler for optimistic polling                                */
/* ------------------------------------------------------------------ */

function useSSETrigger(refetch) {
  const events = useContext(EventContext);
  const prevLen = useRef(0);

  useEffect(() => {
    if (events.length > prevLen) {
      prevLen.current = events.length;
      refetch();
    }
  }, [events.length, refetch]);
}

/* ------------------------------------------------------------------ */
/*  App                                                               */
/* ------------------------------------------------------------------ */

import { useRef } from 'react';

export default function App() {
  const [tasks, setTasks] = useState([]);
  const [filter, setFilter] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');
  const [loading, setLoading] = useState(true);

  const refetch = useCallback(() => {
    fetch('/api/tasks')
      .then((r) => r.json())
      .then((d) => {
        setTasks(d.data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  // Initial load
  useEffect(() => { refetch(); }, [refetch]);

  // Refresh on SSE events
  const events = useContext(EventContext);
  const prevCount = useRef(0);
  useEffect(() => {
    if (events.length > prevCount.current) {
      prevCount.current = events.length;
      refetch();
    }
  }, [events.length, refetch]);

  /* ---- filtering ---- */
  const filtered = tasks.filter((t) => {
    if (statusFilter !== 'all') {
      if (t.state !== statusFilter.toUpperCase()) return false;
    }
    if (filter) {
      const q = filter.toLowerCase();
      return (
        t.id.toLowerCase().includes(q) ||
        t.title.toLowerCase().includes(q) ||
        (t.phase && t.phase.toLowerCase().includes(q))
      );
    }
    return true;
  });

  const stats = useStats();

  return (
    <EventProvider>
      <div id="app">
        <header>
          <div className="header-left">
            <h1>d3vx</h1>
            {stats && (
              <div className="pill-group">
                <Pill label="active" value={stats.active_tasks} color="var(--accent)" />
                <Pill label="queued" value={stats.queued_tasks} color="var(--amber)" />
                <Pill label="done" value={stats.done_tasks} color="var(--green)" />
                <Pill label="failed" value={stats.failed_tasks} color="var(--red)" />
              </div>
            )}
          </div>
          <CostBar />
        </header>

        <div className="toolbar">
          <input
            type="text"
            placeholder="Filter tasks…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="search"
          />
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="status-select"
          >
            <option value="all">all states</option>
            <option value="queued">queued</option>
            <option value="spawning">spawning</option>
            <option value="research">research</option>
            <option value="plan">plan</option>
            <option value="implement">implement</option>
            <option value="review">review</option>
            <option value="done">done</option>
            <option value="failed">failed</option>
          </select>
          <span className="count">{filtered.length} task{filtered.length !== 1 && 's'}</span>
        </div>

        {loading ? (
          <div className="loading">loading…</div>
        ) : (
          <TaskTable tasks={filtered} onRefresh={refetch} />
        )}
      </div>
    </EventProvider>
  );
}

/* ------------------------------------------------------------------ */
/*  Pill                                                              */
/* ------------------------------------------------------------------ */

function Pill({ label, value, color }) {
  if (value == null || value === 0) return null;
  return (
    <span
      className="pill"
      style={{ '--pill-color': color }}
    >
      <span className="pill-dot" style={{ background: color }} />
      {label} {value}
    </span>
  );
}
