import { useEffect, useState } from 'react';

/* ------------------------------------------------------------------ */
/*  Compact cost + budget bar in the header                           */
/* ------------------------------------------------------------------ */

export default function CostBar() {
  const [costs, setCosts] = useState(null);

  useEffect(() => {
    fetch('/api/costs')
      .then((r) => r.json())
      .then((d) => setCosts(d.data))
      .catch(() => {});
  }, []);

  return (
    <div className="cost-bar">
      {costs ? (
        <>
          <span className="cost-total">
            ${costs[1]?.spent_today_usd.toFixed(2) ?? '0.00'}
          </span>
          <div className="cost-progress">
            <div
              className="cost-fill"
              style={{
                width: `${Math.min(100, ((costs[1]?.spent_today_usd ?? 0) / (costs[1]?.daily_budget_usd ?? 1)) * 100)}%`,
              }}
            />
          </div>
          <span className="cost-limit">/ ${costs[1]?.daily_budget_usd.toFixed(0)}</span>
          {costs[0]?.length > 0 && (
            <span className="cost-models">
              {costs[0].map((m) => (
                <span key={m.model} className="cost-model" title={`${m.model}: $${m.cost_usd.toFixed(2)}`}>
                  {shortModel(m.model)}
                </span>
              ))}
            </span>
          )}
        </>
      ) : (
        <span className="cost-total dim">cost: —</span>
      )}
    </div>
  );
}

function shortModel(name) {
  if (!name) return '';
  return name.split('/').pop().substring(0, 10);
}
