# Frontend Conventions
<!-- 仅适用于本仓库，不得跨项目复用 -->

## Meta
- project: OctoMonitor
- repo: /Users/wangyixiao/WorkSpace/OctoMonitor
- framework: React 19 + TypeScript + Vite + React Router
- styling: Tailwind CSS v4 + CSS variables + container queries
- created_by: Athena (dev agent)
- created_at: 2026-03-30T09:25:00Z

## Design Tokens

### Color Palette
| Token | 值 | 用途 |
|-------|-----|------|
| `--color-primary` | #88f7d3 | 主操作 / 在线态 |
| `--color-secondary` | #7aa2ff | 次级强调 |
| `--color-background` | #0b0f14 | 页面背景 |
| `--color-surface` | #121821 | 卡片背景 |
| `--color-surface-elevated` | #18212d | 抬升面板 |
| `--color-text-primary` | #edf3ff | 正文 |
| `--color-text-secondary` | #8ea0b8 | 辅助文字 |
| `--color-border` | #243142 | 边框 |
| `--color-error` | #ff6b7a | 错误 |
| `--color-success` | #51d89f | 成功 |
| `--color-warning` | #ffc96b | 警告 |

### Typography
| 级别 | 字体族 | 字号 | 行高 | 字重 |
|------|--------|------|------|------|
| Display | system-ui | 32px | 1.1 | 700 |
| H1-H2 | system-ui | 24px / 20px | 1.2 | 650 |
| Body | system-ui | 14px | 1.5 | 450 |
| Caption | ui-monospace, monospace | 12px | 1.4 | 500 |

### Spacing
- Base unit: 4px
- Scale: xs=4, sm=8, md=12, lg=16, xl=24, 2xl=32

### Other Tokens
- Border radius: sm=8px, md=12px, lg=18px, full=999px
- Shadow: none / subtle inner border only
- Z-index: modal=1000, drawer=900, dropdown=200, sticky=100

## Responsive Breakpoints
| 名称 | 宽度 | 说明 |
|------|------|------|
| mobile | 375px | 手机 |
| tablet | 768px | 平板 |
| desktop | 1280px | 桌面 |
| ultrawide | 1920px | 宽屏 / 条屏 |

## Component Patterns
- 命名: React 组件 PascalCase，hooks/use-store camelCase
- 文件组织: feature-based + shared ui primitives
- 状态管理: Zustand 管快照/布局/筛选；局部交互用 React hooks
- 数据获取: bootstrap via fetch, live updates via native WebSocket

## Styling Rules
- 方案: Tailwind utilities + CSS variables + `@container`
- CSS 变量: `--color-*`, `--space-*`, `--panel-*`
- 禁止: inline style、`!important`、硬编码随机颜色、混入第二套样式系统

## Asset Conventions
- 图标: Lucide React
- 图片: 尽量不用装饰性图片；必要时 SVG
- 字体: system-ui / ui-monospace，不引入外部 webfont

## Quality Checklist
- [ ] 响应式覆盖 portrait / standard / wide / strip
- [ ] 可访问性: 语义 HTML, ARIA, 键盘导航, 对比度 ≥ 4.5:1
- [ ] 交互态: hover/focus/active/disabled
- [ ] 加载态/空态/错误态
- [ ] 黑暗主题一致性
