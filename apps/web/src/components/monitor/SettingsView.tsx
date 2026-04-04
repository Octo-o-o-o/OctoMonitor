import { AppearanceSection } from './settings/AppearanceSection'
import { InsightsSection } from './settings/InsightsSection'
import { MonitorSection } from './settings/MonitorSection'
import { FilterSection } from './settings/FilterSection'
import { RemoteAccessSection } from './settings/RemoteAccessSection'
import { SystemSection } from './settings/SystemSection'
import { SetupSection } from './settings/SetupSection'

export function SettingsView() {
  return (
    <div className="settings-page">
      <AppearanceSection />
      <RemoteAccessSection />
      <InsightsSection />
      <MonitorSection />
      <FilterSection />
      <SystemSection />
      <SetupSection />
    </div>
  )
}
