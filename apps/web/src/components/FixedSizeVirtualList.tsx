import { useEffect, useMemo, useRef, useState, type Key, type ReactNode } from 'react'

type FixedSizeVirtualListProps<T> = {
  items: T[]
  className: string
  ariaLabel?: string
  tabIndex?: number
  itemHeight: number
  overscan?: number
  threshold?: number
  renderItem: (item: T, index: number) => ReactNode
  getKey?: (item: T, index: number) => Key
}

const DEFAULT_VIEWPORT_HEIGHT = 400

export function FixedSizeVirtualList<T>({
  items,
  className,
  ariaLabel,
  tabIndex,
  itemHeight,
  overscan = 4,
  threshold = 40,
  renderItem,
  getKey,
}: FixedSizeVirtualListProps<T>) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(DEFAULT_VIEWPORT_HEIGHT)

  useEffect(() => {
    const node = containerRef.current
    if (!node || typeof ResizeObserver === 'undefined') {
      return
    }

    const updateHeight = () => {
      setViewportHeight(node.clientHeight || DEFAULT_VIEWPORT_HEIGHT)
    }
    updateHeight()

    const observer = new ResizeObserver(() => updateHeight())
    observer.observe(node)
    return () => observer.disconnect()
  }, [])

  const shouldVirtualize = items.length > threshold
  const totalHeight = items.length * itemHeight
  const visibleCount = Math.max(1, Math.ceil(viewportHeight / itemHeight))
  const maxStartIndex = Math.max(0, items.length - visibleCount - overscan * 2)
  const startIndex = shouldVirtualize
    ? Math.min(maxStartIndex, Math.max(0, Math.floor(scrollTop / itemHeight) - overscan))
    : 0
  const endIndex = shouldVirtualize
    ? Math.min(items.length, startIndex + visibleCount + overscan * 2)
    : items.length

  const visibleItems = useMemo(
    () => items.slice(startIndex, endIndex),
    [endIndex, items, startIndex],
  )

  return (
    <div
      ref={containerRef}
      className={className}
      tabIndex={tabIndex}
      aria-label={ariaLabel}
      onScroll={shouldVirtualize ? (event) => setScrollTop(event.currentTarget.scrollTop) : undefined}
    >
      {shouldVirtualize ? (
        <div className="virtual-list-inner" style={{ height: totalHeight }}>
          {visibleItems.map((item, offset) => {
            const index = startIndex + offset
            return (
              <div
                key={getKey ? getKey(item, index) : index}
                className="virtual-list-item"
                style={{ top: index * itemHeight, minHeight: itemHeight }}
              >
                {renderItem(item, index)}
              </div>
            )
          })}
        </div>
      ) : (
        items.map((item, index) => (
          <div key={getKey ? getKey(item, index) : index}>
            {renderItem(item, index)}
          </div>
        ))
      )}
    </div>
  )
}
