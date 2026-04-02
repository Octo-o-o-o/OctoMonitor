import type { ReactNode } from 'react'
import { handoffData, type SessionCardData, type SessionState, type SourceColumnData } from './mockData'

const stateLabels: Record<SessionState, string> = {
  active: 'Active',
  waitingApproval: 'Waiting',
  completed: 'Done',
  error: 'Error',
}

function Section({
  eyebrow,
  title,
  description,
  children,
}: {
  eyebrow: string
  title: string
  description?: string
  children: ReactNode
}) {
  return (
    <section className="handoff-section">
      <div className="section-heading">
        <span className="section-eyebrow">{eyebrow}</span>
        <h2>{title}</h2>
        {description ? <p>{description}</p> : null}
      </div>
      {children}
    </section>
  )
}

function SessionCard({ session }: { session: SessionCardData }) {
  return (
    <article className={`session-card state-${session.state}`}>
      <div className="session-card-top">
        <span className={`status-pill status-${session.state}`}>{stateLabels[session.state]}</span>
        <span className="session-meta">{session.duration}</span>
        <span className="session-meta">{session.updated}</span>
      </div>
      <h4>{session.title}</h4>
      <p>{session.detail}</p>
      <div className="session-card-bottom">
        <span className="tag-chip">{session.tag}</span>
        {session.origin ? <span className="soft-chip">{session.origin}</span> : null}
        {session.model ? <span className="soft-chip">{session.model}</span> : null}
      </div>
      <div className="session-card-foot">
        <span>{session.messageCount} inputs</span>
        <span>{session.tokens} tokens</span>
      </div>
    </article>
  )
}

function SourceColumn({ source }: { source: SourceColumnData }) {
  return (
    <article className={`source-column source-${source.id}`}>
      <div className="source-column-head">
        <div>
          <div className="source-title-row">
            <h3>{source.name}</h3>
            <span className={`source-dot ${source.status}`}>{source.status}</span>
          </div>
          <p>{source.auth}</p>
        </div>
        {source.quotaLabel ? <span className="quota-chip">{source.quotaLabel}</span> : null}
      </div>

      <div className="source-session-list">
        {source.sessions.map((session) => (
          <SessionCard key={session.id} session={session} />
        ))}
      </div>

      {source.scheduled?.length ? (
        <div className="schedule-panel">
          <div className="schedule-title">Scheduled agents</div>
          {source.scheduled.map((item) => (
            <div key={item.name} className="schedule-row">
              <div>
                <strong>{item.name}</strong>
                <span>{item.schedule}</span>
              </div>
              {item.agent ? <span className="soft-chip">{item.agent}</span> : null}
            </div>
          ))}
        </div>
      ) : null}
    </article>
  )
}

function ProgressBars({
  title,
  total,
  cost,
  items,
}: {
  title: string
  total: string
  cost: string
  items: ReadonlyArray<{ label: string; value: number }>
}) {
  return (
    <article className="usage-card">
      <div className="usage-card-head">
        <div>
          <h3>{title}</h3>
          <p>{total} tokens</p>
        </div>
        <span className="soft-chip">{cost}</span>
      </div>
      <div className="usage-bars">
        {items.map((item) => (
          <div key={item.label} className="usage-bar-row">
            <div className="usage-bar-labels">
              <span>{item.label}</span>
              <span>{item.value}%</span>
            </div>
            <div className="usage-bar-track">
              <span className="usage-bar-fill" style={{ width: `${item.value}%` }} />
            </div>
          </div>
        ))}
      </div>
    </article>
  )
}

function KeyValueCard({
  title,
  items,
}: {
  title: string
  items: ReadonlyArray<{ label: string; value: string }>
}) {
  return (
    <article className="detail-card">
      <div className="detail-card-head">
        <h3>{title}</h3>
      </div>
      <div className="key-value-list">
        {items.map((item) => (
          <div key={item.label} className="key-value-row">
            <span>{item.label}</span>
            <strong>{item.value}</strong>
          </div>
        ))}
      </div>
    </article>
  )
}

export function FigmaMakeExport() {
  return (
    <main className="handoff-shell">
      <header className="hero-card">
        <div className="hero-copy">
          <span className="hero-eyebrow">{handoffData.header.eyebrow}</span>
          <h1>{handoffData.header.title}</h1>
          <p>{handoffData.header.description}</p>
        </div>
        <div className="hero-side">
          <div className="generated-at">Generated {handoffData.generatedAt}</div>
          <div className="hero-chip-row">
            {handoffData.header.chips.map((chip) => (
              <span key={chip} className="hero-chip">
                {chip}
              </span>
            ))}
          </div>
        </div>
      </header>

      <section className="attention-strip">
        {handoffData.attention.map((item) => (
          <article key={item} className="attention-card">
            {item}
          </article>
        ))}
      </section>

      <section className="summary-grid">
        {handoffData.summary.map((item) => (
          <article key={item.label} className={`summary-card tone-${item.tone}`}>
            <span>{item.label}</span>
            <strong>{item.value}</strong>
          </article>
        ))}
      </section>

      <Section
        eyebrow="Monitor"
        title="Operational source board"
        description="This is the part of OctoMonitor that users keep open all day. Each column is a source, each card is a tracked run, and waiting sessions should be visually prominent."
      >
        <div className="monitor-grid">
          {handoffData.sources.map((source) => (
            <SourceColumn key={source.id} source={source} />
          ))}
        </div>
      </Section>

      <Section
        eyebrow="Inspect"
        title="Selected run drawer content"
        description="The real product opens this in a drawer. For handoff purposes it is flattened into a full-width section so Figma Make can see the hierarchy, metadata, transcript, and action-oriented summary."
      >
        <div className="inspect-grid">
          <article className="inspect-hero">
            <div className="inspect-hero-top">
              <span className="soft-chip">{handoffData.inspect.tool}</span>
              <span className="status-pill status-active">{handoffData.inspect.state}</span>
            </div>
            <h3>{handoffData.inspect.runName}</h3>
            <p>{handoffData.inspect.summary}</p>
          </article>

          <KeyValueCard title="Run metadata" items={handoffData.inspect.metadata} />

          <article className="detail-card">
            <div className="detail-card-head">
              <h3>Design goals</h3>
            </div>
            <ul className="plain-list">
              {handoffData.inspect.checklist.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </article>

          <article className="detail-card transcript-card">
            <div className="detail-card-head">
              <h3>Transcript slice</h3>
            </div>
            <div className="transcript-list">
              {handoffData.inspect.transcript.map((item) => (
                <div key={`${item.speaker}-${item.text}`} className="transcript-item">
                  <span>{item.speaker}</span>
                  <p>{item.text}</p>
                </div>
              ))}
            </div>
          </article>
        </div>
      </Section>

      <Section
        eyebrow="Usage"
        title="Token, cost, and throughput views"
        description="The analytics surface is split into summary KPIs, by-source usage distribution, and a lightweight activity timeline."
      >
        <div className="usage-totals-grid">
          {handoffData.usage.totals.map((item) => (
            <article key={item.label} className="detail-card compact-card">
              <span className="mini-label">{item.label}</span>
              <strong className="big-number">{item.value}</strong>
            </article>
          ))}
        </div>

        <div className="usage-grid">
          {handoffData.usage.bySource.map((group) => (
            <ProgressBars
              key={group.source}
              title={group.source}
              total={group.total}
              cost={group.cost}
              items={group.items}
            />
          ))}

          <article className="detail-card timeline-card">
            <div className="detail-card-head">
              <h3>7 day token timeline</h3>
              <span className="soft-chip">Mocked aggregate</span>
            </div>
            <div className="timeline-bars">
              {handoffData.usage.timeline.map((point) => (
                <div key={point.day} className="timeline-row">
                  <span>{point.day}</span>
                  <div className="timeline-track">
                    <span
                      className="timeline-fill"
                      style={{ width: `${(Number.parseInt(point.tokens, 10) / 330) * 100}%` }}
                    />
                  </div>
                  <strong>{point.tokens}</strong>
                </div>
              ))}
            </div>
          </article>
        </div>
      </Section>

      <Section
        eyebrow="Settings"
        title="Controls, identities, and read-only system facts"
        description="Settings mix editable UI preferences, monitor controls, safe filtering rules, and read-only system configuration. The design should separate those responsibilities clearly."
      >
        <div className="settings-grid">
          <KeyValueCard title="Appearance" items={handoffData.settings.appearance} />
          <KeyValueCard title="Monitor controls" items={handoffData.settings.monitor} />

          <article className="detail-card">
            <div className="detail-card-head">
              <h3>Filter rules</h3>
            </div>
            <div className="filter-list">
              {handoffData.settings.filters.map((filter) => (
                <div key={filter.source} className="filter-item">
                  <div className="filter-item-head">
                    <strong>{filter.source}</strong>
                    <span className="soft-chip">{filter.mode}</span>
                  </div>
                  <div className="tag-row">
                    {filter.patterns.length ? (
                      filter.patterns.map((pattern) => (
                        <span key={pattern} className="tag-chip">
                          {pattern}
                        </span>
                      ))
                    ) : (
                      <span className="muted-copy">No active patterns</span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </article>

          <article className="detail-card">
            <div className="detail-card-head">
              <h3>Identities</h3>
            </div>
            <div className="identity-list">
              {handoffData.settings.identities.map((item) => (
                <div key={item.tool} className="identity-row">
                  <div>
                    <strong>{item.tool}</strong>
                    <p>{item.identity}</p>
                  </div>
                  <div className="identity-meta">
                    <span>{item.auth}</span>
                    <span className="soft-chip">{item.status}</span>
                  </div>
                </div>
              ))}
            </div>
          </article>

          <KeyValueCard title="Server config" items={handoffData.settings.server} />

          <article className="detail-card">
            <div className="detail-card-head">
              <h3>Installer + doctor</h3>
            </div>
            <div className="installer-list">
              {handoffData.settings.installer.map((item) => (
                <div key={item.tool} className="installer-row">
                  <div>
                    <strong>{item.tool}</strong>
                    <p>{item.detail}</p>
                  </div>
                  <span className={`status-pill ${item.status === 'Missing' ? 'status-error' : 'status-completed'}`}>
                    {item.status}
                  </span>
                </div>
              ))}
            </div>
          </article>
        </div>
      </Section>

      <Section
        eyebrow="States"
        title="Important edge cases that should stay visible in the redesign"
        description="These notes are included because screenshots often miss them, but the final product needs dedicated affordances for these operational states."
      >
        <div className="state-gallery">
          {handoffData.stateGallery.map((item) => (
            <article key={item.title} className="detail-card">
              <div className="detail-card-head">
                <h3>{item.title}</h3>
              </div>
              <p>{item.text}</p>
            </article>
          ))}
        </div>
      </Section>
    </main>
  )
}
