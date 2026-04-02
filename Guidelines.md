# Kindle E-ink Design System Guidelines

本设计系统专为电子墨水屏（E-ink）优化，完全无彩色依赖，通过黑白灰色阶、几何纹理和描边样式区分数据，实现色盲友好和 Kindle 阅读器般的极简质感。

---

## 1. Design Tokens（设计令牌）

### Color Palette

#### Light Mode（浅色模式）
```css
--background: #fdfcf8;        /* 米白纸张质感 */
--foreground: #1c1c1c;        /* 深黑文本 */
--card: #ffffff;              /* 纯白卡片 */
--muted: #f3f0e8;            /* 柔和米色背景 */
--secondary: #737373;         /* 中灰色（次要元素）*/
--border: rgba(0, 0, 0, 0.15); /* 半透明黑边框 */
```

#### Dark Mode（深色模式）
```css
--background: #121212;        /* 纯黑底 */
--foreground: #e8e6e3;        /* 柔和米色文本 */
--card: #1c1c1c;             /* 深灰卡片 */
--muted: #2a2a2a;            /* 深灰背景 */
--secondary: #8c8c8c;         /* 浅灰色（次要元素）*/
--border: rgba(255, 255, 255, 0.15); /* 半透明白边框 */
```

### E-ink Chart Colors（图表专用灰阶）

5 个可区分的灰度层级，避免任何彩色：

| Token    | Light Mode | Dark Mode  | 用途               |
|----------|-----------|-----------|-------------------|
| chart-1  | #1c1c1c   | #e8e6e3   | 主数据系列（最深/最浅） |
| chart-2  | #737373   | #8c8c8c   | 次要数据系列（中灰）   |
| chart-3  | #a3a3a3   | #525252   | 第三数据系列         |
| chart-4  | #d4d4d4   | #262626   | 第四数据系列         |
| chart-5  | #262626   | #d4d4d4   | 第五数据系列         |

---

## 2. Pattern Library（SVG 纹理图案库）

### 为什么需要纹理？
在电子墨水屏上，无法依赖颜色区分数据，必须使用几何纹理和描边样式来创造视觉差异。

### Pattern 1: 斜线网格（Diagonal Lines）
**用途**: 主数据系列的填充纹理

```tsx
<pattern id="patternPrimary" patternUnits="userSpaceOnUse" width="8" height="8">
  <path 
    d="M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4" 
    stroke="var(--primary)" 
    strokeWidth="1" 
    strokeOpacity="0.3"
  />
</pattern>
```

### Pattern 2: 圆点阵列（Dot Array）
**用途**: 次要数据系列的填充纹理

```tsx
<pattern id="patternSecondary" patternUnits="userSpaceOnUse" width="6" height="6">
  <circle 
    cx="3" 
    cy="3" 
    r="1" 
    fill="var(--secondary)" 
    fillOpacity="0.4" 
  />
</pattern>
```

### Stroke Styles（描边样式）
- **实线**: `strokeWidth="2"` - 主数据系列
- **虚线**: `strokeDasharray="5 5"` - 次要数据系列

---

## 3. Component Behavior（组件交互行为）

### Hover States
- 使用极低透明度: `hover:bg-muted/5`
- 避免强烈阴影或颜色跳变
- 过渡时间: `transition-colors duration-300`

### Card Elevation
- ❌ 禁止使用 `box-shadow`
- ✅ 使用细边框: `border border-border`
- 圆角: `rounded-lg` (0.5rem)
- 内边距: 
  - 移动端: `p-4` 或 `p-6`
  - 桌面端: `p-6` 或 `p-8`

### Progress Bars（进度条）
- 高度: `h-1.5` (极细线条，类似 Kindle 阅读进度)
- 轨道背景: `bg-muted/30`
- 填充色:
  - Primary: `bg-primary` (#1c1c1c)
  - Secondary: `bg-secondary` (#737373)
- 过渡: `transition-all duration-500 ease-out`

---

## 4. Typography System（字体系统）

### Font Weights
| 场景        | Weight | CSS 变量                      |
|-----------|--------|------------------------------|
| 正文/输入框  | 400    | `var(--font-weight-normal)`   |
| 标题/按钮/标签 | 500    | `var(--font-weight-medium)`   |

**重要**: 禁止使用 Bold 700+，避免墨水过重导致模糊

### Font Sizes
- 基础字号: `16px` (Kindle 标准可读性)
- h1: `text-2xl` + Medium weight
- h2: `text-xl` + Medium weight
- h3: `text-lg` + Medium weight
- body/label/button: `text-base`

### Special Typography
- **数字**: 使用 `tabular-nums` (等宽数字，便于对齐)
- **代码/路径**: 使用 `font-mono`
- **大写标题**: `uppercase tracking-wider` (加宽字间距提升可读性)
- **截断长文本**: `truncate` (单行截断) 或 `line-clamp-2` (多行截断)

---

## 5. Spacing & Layout Rhythm（间距韵律）

### Vertical Rhythm（垂直间距）
| 场景             | 移动端        | 桌面端         |
|-----------------|-------------|--------------|
| Card 间距       | `space-y-6` | `space-y-8`  |
| Section 间距    | `mb-12`     | `mb-16`      |
| 大型 Section 间距 | `mt-16`     | `mt-24`      |
| 元素内部小间距    | `space-y-2` | `space-y-2`  |

### Horizontal Padding（水平内边距）
- 全局容器: `px-4 md:px-6 lg:px-8`
- 最大宽度: `max-w-[1600px]`
- Grid 列间距: `gap-6 md:gap-8`

### Responsive Breakpoints（响应式断点）
- **sm**: `sm:grid-cols-3` (统计卡片)
- **lg**: `lg:grid-cols-2` (源卡片在中等屏幕)
- **xl**: `xl:grid-cols-3` (源卡片在大屏幕)

---

## 6. Component Variants（组件变体）

### Data Visualization Variants
所有数据可视化组件（Chart, ProgressBar, SourceCard）必须支持以下变体：

| Variant     | 颜色            | 纹理      | 描边样式  | 用途        |
|-------------|----------------|---------|---------|-----------|
| `primary`   | `var(--primary)` | 斜线网格 | 实线 (2px) | 主数据系列  |
| `secondary` | `var(--secondary)` | 圆点阵列 | 虚线 (5 5) | 次要数据系列 |

### Usage Example
```tsx
<SourceCard {...data} variant="primary" />
<ProgressBar {...props} variant="secondary" />
```

---

## 7. Accessibility & Contrast（无障碍和对比度）

### Color Blindness Friendly（色盲友好）
- ❌ **禁止使用**: 红绿色组合
- ✅ **仅使用**: 黑白灰色阶
- ✅ **区分方式**: 纹理、描边样式、间距、字体大小

### Contrast Ratios（对比度）
- 文本对背景: **至少 4.5:1** (WCAG AA 标准)
- Light Mode: `#1c1c1c` on `#fdfcf8` ✅
- Dark Mode: `#e8e6e3` on `#121212` ✅

### Pattern Opacity（纹理透明度）
- SVG 纹理: `strokeOpacity="0.3"` 或 `fillOpacity="0.4"`
- Hover 背景: `bg-muted/5` (5%)
- Progress 轨道: `bg-muted/30` (30%)

---

## 8. Chart Configuration Template（图表配置模板）

### Recharts CartesianGrid
```tsx
<CartesianGrid 
  strokeDasharray="3 3" 
  stroke="var(--border)" 
  vertical={false}  // 仅显示水平网格线
/>
```

### Axis Style（坐标轴样式）
```tsx
<XAxis 
  stroke="var(--muted-foreground)" 
  fontSize={12} 
  tickLine={false}   // 隐藏刻度线
  axisLine={false}   // 隐藏轴线
  dy={10}           // 向下偏移
/>

<YAxis 
  stroke="var(--muted-foreground)" 
  fontSize={12} 
  tickLine={false} 
  axisLine={false} 
  dx={-10}          // 向左偏移
  tickFormatter={(value) => `${value}M`}  // 数值格式化
/>
```

### Tooltip Style（提示框样式）
```tsx
<Tooltip 
  contentStyle={{ 
    backgroundColor: 'var(--popover)', 
    borderColor: 'var(--border)',
    color: 'var(--popover-foreground)',
    borderRadius: '8px',
    boxShadow: '0 4px 12px rgba(0, 0, 0, 0.05)'
  }} 
  itemStyle={{ color: 'var(--foreground)', fontWeight: 500 }}
  cursor={{ 
    stroke: 'var(--border)', 
    strokeWidth: 1, 
    strokeDasharray: '4 4' 
  }}
/>
```

### Area Chart with Patterns（带纹理的面积图）
```tsx
<AreaChart data={data}>
  <defs>
    <pattern id="patternA" patternUnits="userSpaceOnUse" width="8" height="8">
      <path d="M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4" 
            stroke="var(--primary)" 
            strokeWidth="1" 
            strokeOpacity="0.3"/>
    </pattern>
    <pattern id="patternB" patternUnits="userSpaceOnUse" width="6" height="6">
      <circle cx="3" cy="3" r="1" 
              fill="var(--secondary)" 
              fillOpacity="0.4" />
    </pattern>
  </defs>
  
  <Area 
    type="monotone" 
    dataKey="seriesA" 
    stroke="var(--primary)" 
    strokeWidth={2}
    fill="url(#patternA)" 
    fillOpacity={1}
  />
  
  <Area 
    type="monotone" 
    dataKey="seriesB" 
    stroke="var(--secondary)" 
    strokeWidth={2}
    strokeDasharray="5 5"
    fill="url(#patternB)" 
    fillOpacity={1}
  />
</AreaChart>
```

---

## 9. Do's and Don'ts（最佳实践）

### ✅ Do（推荐做法）
- 使用 CSS 变量引用颜色: `var(--primary)`, `var(--border)`
- 使用 SVG pattern 区分数据系列
- 使用 `tabular-nums` 对齐数字
- 使用 `uppercase tracking-wider` 提升标签可读性
- 使用细边框 (`border-border`) 而非阴影
- 移动端优先，使用响应式断点
- 保持充足留白 (generous spacing)

### ❌ Don't（避免做法）
- 不使用任何彩色 (蓝色、琥珀色、红色、绿色等)
- 不使用 `box-shadow` 或强烈阴影
- 不使用 `font-weight: 700` 以上的粗体
- 不使用渐变 (`gradient`)
- 不使用彩色图标库 (仅使用 lucide-react 的黑白图标)
- 不使用高饱和度的 hover 效果

---

## 10. File Structure（文件结构约定）

```
/src/styles/
├── theme.css          # 设计令牌定义 (CSS 变量)
├── fonts.css          # 字体导入
└── tailwind.css       # Tailwind 配置

/src/app/components/
├── navigation-tabs.tsx
├── status-bar.tsx
├── stat-card.tsx
├── source-card.tsx
├── usage-chart.tsx
├── usage-progress-bar.tsx
└── theme-toggle.tsx

/guidelines/
└── Guidelines.md      # 本设计规范文档
```

---

## 11. Theme Toggle（主题切换）

### Implementation
- 使用 `next-themes` 的 `ThemeProvider`
- 支持 `light` 和 `dark` 两种模式
- 禁用系统主题: `enableSystem={false}`
- 默认主题: `defaultTheme="dark"`

### CSS Class Toggle
```tsx
<ThemeProvider attribute="class" defaultTheme="dark" enableSystem={false}>
  {/* 通过 .dark 类切换主题 */}
</ThemeProvider>
```

---

## 总结

这套设计系统的核心理念是：**通过几何形状、间距、字重和纹理来传达信息层次，完全摒弃颜色依赖**。这不仅适配电子墨水屏，还天然具备色盲友好特性，实现了视觉设计的普适性（Universal Design）。
