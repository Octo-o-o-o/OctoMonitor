import { useI18n } from '../../lib/i18n'

interface Props {
  onClose: () => void
}

export function WorkflowGuide({ onClose }: Props) {
  const { locale, t } = useI18n()

  return (
    <div className="wf-guide-overlay" onClick={onClose}>
      <div className="wf-guide-panel" onClick={(e) => e.stopPropagation()}>
        <div className="wf-guide-header">
          <h2 className="wf-guide-title">{t('wf.helpTitle')}</h2>
          <button className="wf-guide-close" onClick={onClose}>&times;</button>
        </div>
        <div className="wf-guide-body">
          {locale === 'zh' ? <GuideZh /> : <GuideEn />}
        </div>
      </div>
    </div>
  )
}

function GuideZh() {
  return (
    <>
      <section className="wf-guide-section">
        <h3>创建工作流</h3>
        <p>在工作流页面点击 <kbd>+ 新建工作流</kbd>，填写名称、描述、工作目录，并添加步骤。</p>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row">
            <span className="wf-guide-label">名称</span>
            <span>工作流名称，例如"方案设计与审查"</span>
          </div>
          <div className="wf-guide-row">
            <span className="wf-guide-label">描述</span>
            <span>可选，简要说明此工作流用途</span>
          </div>
          <div className="wf-guide-row">
            <span className="wf-guide-label">工作目录</span>
            <span>项目路径（下拉选择 Monitor 中已有的项目）。决定 <code>{'{{file:...}}'}</code> 从哪里读取文件</span>
          </div>
        </div>
      </section>

      <section className="wf-guide-section">
        <h3>步骤参数</h3>

        <h4>Tool（工具）</h4>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row"><span className="wf-guide-label">Claude</span><span>Claude Code 会话</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Codex</span><span>Codex 会话</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">OpenClaw</span><span>OpenClaw 会话</span></div>
        </div>

        <h4>Kind（类型）</h4>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row">
            <span className="wf-guide-label">观察 Observe</span>
            <span>你在工具自有界面中操作，OctoMonitor 等待你手动关联对应 run。适合交互式操作</span>
          </div>
          <div className="wf-guide-row">
            <span className="wf-guide-label">启动 Launch</span>
            <span>由 OctoMonitor 通过 CLI 非交互模式自动发起。需填写 Prompt 模板。适合结构化 review</span>
          </div>
        </div>

        <h4>Completion（完成条件）</h4>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row"><span className="wf-guide-label">手动关联</span><span>关联一个 monitor run 后点"完成"。<strong>最常用，适合 observe</strong></span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">手动完成</span><span>无需关联 run，直接标记完成。适合人工审查文档等</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">启动器退出</span><span>CLI 子进程正常退出即自动完成。<strong>适合 launch</strong></span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Hook 事件</span><span>收到匹配的 ingest hook 事件时完成</span></div>
        </div>

        <h4>需审批</h4>
        <p>勾选后，launch step 在辅助/自动模式下执行前会暂停，等你查看 Prompt 预览并点击"审批并启动"。</p>
      </section>

      <section className="wf-guide-section">
        <h3>Prompt 模板变量</h3>
        <p>仅 <strong>Launch</strong> 类型步骤可用。在 Prompt 模板中使用 <code>{'{{变量名}}'}</code> 语法：</p>
        <div className="wf-guide-field-table wf-guide-mono-table">
          <div className="wf-guide-row"><code>{'{{workflow.name}}'}</code><span>工作流名称</span></div>
          <div className="wf-guide-row"><code>{'{{step.label}}'}</code><span>当前步骤名称</span></div>
          <div className="wf-guide-row"><code>{'{{working_dir}}'}</code><span>工作目录路径</span></div>
          <div className="wf-guide-row"><code>{'{{previous.summary}}'}</code><span>上一步关联 run 的最后提问</span></div>
          <div className="wf-guide-row"><code>{'{{previous.artifacts}}'}</code><span>上一步产物文件列表</span></div>
          <div className="wf-guide-row"><code>{'{{linked_run.last_question}}'}</code><span>当前步骤关联 run 的最后提问</span></div>
          <div className="wf-guide-row"><code>{'{{linked_run.summary}}'}</code><span>当前步骤关联 run 的首次提问</span></div>
          <div className="wf-guide-row"><code>{'{{file:相对路径}}'}</code><span>从工作目录读取文件内容注入（最大 8,000 字符）</span></div>
        </div>
        <div className="wf-guide-note">
          总 Prompt 上限 32,000 字符。文件不存在时替换为 <code>[file not found: path]</code>。
        </div>
      </section>

      <section className="wf-guide-section">
        <h3>三种执行模式</h3>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row"><span className="wf-guide-label">跟踪</span><span>纯手动。所有步骤需要你自己关联 run 并标记完成</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">辅助</span><span>Launch step 前序完成后自动进入"待审批"→ 你确认后启动 CLI</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">自动</span><span>Launch step 在前序完成后自动执行（需审批的仍会暂停）</span></div>
        </div>
        <div className="wf-guide-note">运行中可随时切换模式。Observe 步骤在任何模式下都需要手动操作。</div>
      </section>

      <section className="wf-guide-section">
        <h3>典型流程示例</h3>
        <div className="wf-guide-example">
          <div className="wf-guide-ex-step"><span className="wf-guide-ex-num">1</span><span className="wf-guide-ex-tool cc">CC</span>Claude 写方案 → 手动关联 → 完成</div>
          <div className="wf-guide-ex-step"><span className="wf-guide-ex-num">2</span><span className="wf-guide-ex-tool cx">CX</span>Codex Review → 手动关联 → 完成</div>
          <div className="wf-guide-ex-step"><span className="wf-guide-ex-num">3</span><span className="wf-guide-ex-tool cc">CC</span>Claude 修改 → 手动关联 → 完成</div>
          <div className="wf-guide-ex-step"><span className="wf-guide-ex-num">4</span><span className="wf-guide-ex-tool cx">CX</span>Codex 再审 → <em>辅助模式下自动启动 CLI</em></div>
          <div className="wf-guide-ex-step"><span className="wf-guide-ex-num">5</span><span className="wf-guide-ex-tool cc">CC</span>Claude 实施 → 手动关联 → 完成</div>
          <div className="wf-guide-ex-step"><span className="wf-guide-ex-num">6</span><span className="wf-guide-ex-tool cx">CX</span>Codex 实施 Review → <em>辅助模式下自动启动 CLI</em></div>
        </div>
        <div className="wf-guide-note">建议先用"跟踪"模式熟悉流程，再切换到"辅助"或"自动"。</div>
      </section>
    </>
  )
}

function GuideEn() {
  return (
    <>
      <section className="wf-guide-section">
        <h3>Create a Workflow</h3>
        <p>Click <kbd>+ New Workflow</kbd>, fill in name, description, working directory, and add steps.</p>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row"><span className="wf-guide-label">Name</span><span>Workflow name, e.g. "Plan Design & Review"</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Description</span><span>Optional brief description</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Working Dir</span><span>Project path (dropdown shows known projects). Determines where <code>{'{{file:...}}'}</code> reads from</span></div>
        </div>
      </section>

      <section className="wf-guide-section">
        <h3>Step Parameters</h3>
        <h4>Tool</h4>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row"><span className="wf-guide-label">Claude</span><span>Claude Code sessions</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Codex</span><span>Codex sessions</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">OpenClaw</span><span>OpenClaw sessions</span></div>
        </div>

        <h4>Kind</h4>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row"><span className="wf-guide-label">Observe</span><span>You work in the tool's own UI. OctoMonitor waits for you to link the run manually</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Launch</span><span>OctoMonitor starts a CLI process. Requires a prompt template. Good for automated reviews</span></div>
        </div>

        <h4>Completion</h4>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row"><span className="wf-guide-label">Manual Link</span><span>Link a monitor run then click Complete. <strong>Most common for observe</strong></span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Manual Complete</span><span>No run needed, just mark as done</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Launcher Exit</span><span>Auto-complete when CLI exits successfully. <strong>Best for launch</strong></span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Hook Event</span><span>Complete when a matching ingest hook event arrives</span></div>
        </div>

        <h4>Approval</h4>
        <p>When checked, launch steps pause before execution in Assisted/Auto mode so you can review the prompt preview.</p>
      </section>

      <section className="wf-guide-section">
        <h3>Prompt Template Variables</h3>
        <p>Available for <strong>Launch</strong> steps only. Use <code>{'{{variable}}'}</code> syntax:</p>
        <div className="wf-guide-field-table wf-guide-mono-table">
          <div className="wf-guide-row"><code>{'{{workflow.name}}'}</code><span>Workflow name</span></div>
          <div className="wf-guide-row"><code>{'{{step.label}}'}</code><span>Current step label</span></div>
          <div className="wf-guide-row"><code>{'{{working_dir}}'}</code><span>Working directory path</span></div>
          <div className="wf-guide-row"><code>{'{{previous.summary}}'}</code><span>Previous step's linked run last question</span></div>
          <div className="wf-guide-row"><code>{'{{previous.artifacts}}'}</code><span>Previous step's artifact file paths</span></div>
          <div className="wf-guide-row"><code>{'{{linked_run.last_question}}'}</code><span>Current step's linked run last question</span></div>
          <div className="wf-guide-row"><code>{'{{linked_run.summary}}'}</code><span>Current step's linked run first question</span></div>
          <div className="wf-guide-row"><code>{'{{file:path}}'}</code><span>Inject file content from working dir (max 8,000 chars)</span></div>
        </div>
        <div className="wf-guide-note">Total prompt capped at 32,000 chars. Missing files become <code>[file not found: path]</code>.</div>
      </section>

      <section className="wf-guide-section">
        <h3>Execution Modes</h3>
        <div className="wf-guide-field-table">
          <div className="wf-guide-row"><span className="wf-guide-label">Tracking</span><span>Fully manual. Link runs and complete steps yourself</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Assisted</span><span>Launch steps wait for approval, then auto-start CLI</span></div>
          <div className="wf-guide-row"><span className="wf-guide-label">Auto</span><span>Launch steps auto-start (unless approval_required is set)</span></div>
        </div>
        <div className="wf-guide-note">Modes can be switched at any time during a run. Observe steps always require manual action.</div>
      </section>
    </>
  )
}
