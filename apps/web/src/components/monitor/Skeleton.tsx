import { NARROW_LAYOUT_QUERY, useMediaQuery } from '../../lib/responsive'

function repeat(count: number, className: string) {
  return Array.from({ length: count }, (_, i) => (
    <div key={i} className={className} />
  ))
}

export function MonitorSkeleton() {
  const isNarrowLayout = useMediaQuery(NARROW_LAYOUT_QUERY)

  return (
    <div className="monitor-view">
      <div className="monitor-board-panel">
        <div className={`task-feed-layout${isNarrowLayout ? '' : ' has-rail'}`}>
          <div className="task-feed-board">
            <div className="skeleton-header" />
            {repeat(6, 'skeleton-row')}
          </div>
          {!isNarrowLayout && (
            <div className="monitor-rail">
              <div className="skeleton-header" />
              {repeat(2, 'skeleton-card')}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export function UsageSkeleton() {
  return (
    <div className="usage-view">
      <div className="skeleton-header skeleton-w-40" />
      <div className="skeleton-bar-chart">{repeat(3, 'skeleton-bar')}</div>
      <div className="skeleton-table">{repeat(4, 'skeleton-row')}</div>
    </div>
  )
}

export function CommitsSkeleton() {
  return (
    <div className="commits-view">
      <div className="skeleton-header skeleton-w-35" />
      {Array.from({ length: 2 }, (_, i) => (
        <div key={i} className="skeleton-table">{repeat(3, 'skeleton-row')}</div>
      ))}
    </div>
  )
}

export function HeatmapSkeleton() {
  return (
    <div className="heatmap-view">
      <div className="skeleton-header skeleton-w-38" />
      <div className="skeleton-bar-chart">{repeat(4, 'skeleton-bar')}</div>
      <div className="skeleton-table">{repeat(5, 'skeleton-row')}</div>
    </div>
  )
}
