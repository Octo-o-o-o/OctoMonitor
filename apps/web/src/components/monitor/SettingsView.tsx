import { AppearanceSection } from './settings/AppearanceSection'
import { MonitorSection } from './settings/MonitorSection'
import { FilterSection } from './settings/FilterSection'
import { RemoteAccessSection } from './settings/RemoteAccessSection'
import { SystemSection } from './settings/SystemSection'
import { SetupSection } from './settings/SetupSection'
import { DesktopDisplaySection } from './settings/DesktopDisplaySection'

export function SettingsView() {
  return (
    <div className="settings-page">
      <AppearanceSection />
      <DesktopDisplaySection />
      <RemoteAccessSection />
      <MonitorSection />
      <FilterSection />
      <SystemSection />
      <SetupSection />
    </div>
  )
}
