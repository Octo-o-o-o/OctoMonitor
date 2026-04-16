import type { I18nKey } from './i18n'
import type {
  AgentDisplayFormat,
  FilterMode,
  FontSize,
  UiDensity,
} from './preferences'
import type { RunState } from './types'

export const stateLabelKeys: Record<RunState, I18nKey> = {
  active: 'state.active',
  waitingApproval: 'state.waitingApproval',
  idle: 'state.idle',
  completed: 'state.completed',
  error: 'state.error',
  stale: 'state.stale',
  gatewayOffline: 'state.gatewayOffline',
  limitExceeded: 'state.limitExceeded',
  contextExceeded: 'state.contextExceeded',
  cancelled: 'state.cancelled',
}

export const fontSizeLabelKeys: Record<FontSize, I18nKey> = {
  xsmall: 'settings.fontSize.xsmall',
  small: 'settings.fontSize.small',
  default: 'settings.fontSize.default',
  large: 'settings.fontSize.large',
  xlarge: 'settings.fontSize.xlarge',
}

export const uiDensityLabelKeys: Record<UiDensity, I18nKey> = {
  compact: 'settings.uiDensity.compact',
  comfortable: 'settings.uiDensity.comfortable',
  spacious: 'settings.uiDensity.spacious',
}

export const filterModeLabelKeys: Record<FilterMode, I18nKey> = {
  off: 'settings.filterMode.off',
  include: 'settings.filterMode.include',
  exclude: 'settings.filterMode.exclude',
}

export const agentDisplayLabelKeys: Record<AgentDisplayFormat, I18nKey> = {
  id: 'settings.agentDisplay.id',
  name: 'settings.agentDisplay.name',
  'id:name': 'settings.agentDisplay.id:name',
}
