export function MonitorSkeleton() {
  return (
    <div className="monitor-view">
      <div className="source-columns source-columns-desktop" style={{ gridTemplateColumns: '1fr 1fr 1fr' }}>
        {[0, 1, 2].map((i) => (
          <div key={i} className="source-column">
            <div className="skeleton-header" />
            {[0, 1, 2].map((j) => (
              <div key={j} className="skeleton-card" />
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}

export function UsageSkeleton() {
  return (
    <div className="usage-view">
      <div className="skeleton-header" style={{ width: '40%' }} />
      <div className="skeleton-bar-chart">
        {[0, 1, 2].map((i) => (
          <div key={i} className="skeleton-bar" />
        ))}
      </div>
      <div className="skeleton-table">
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="skeleton-row" />
        ))}
      </div>
    </div>
  )
}

export function CommitsSkeleton() {
  return (
    <div className="commits-view">
      <div className="skeleton-header" style={{ width: '35%' }} />
      {[0, 1].map((i) => (
        <div key={i} className="skeleton-table" style={{ marginTop: 12 }}>
          {[0, 1, 2].map((j) => (
            <div key={j} className="skeleton-row" />
          ))}
        </div>
      ))}
    </div>
  )
}
