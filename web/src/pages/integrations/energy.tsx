import { ExternalLinkIcon } from "lucide-react"

import { PageBody, PageIntro } from "@/components/page"
import { RestoreDefaults } from "@/components/restore-defaults"
import {
  NumberRow,
  RowSkeleton,
  Section,
  SecretRow,
  SettingsCard,
  SwitchRow,
  TextRow,
  useEnabled,
} from "@/components/settings"
import { useDraft } from "@/lib/draft"

export default function Energy() {
  const { ready } = useDraft()
  return (
    <PageBody>
      <RestoreDefaults page="/integrations/energy" />
      <PageIntro title="Energy">
        The power mix and renewable roles read your grid from Electricity Maps. A free token is enough.
      </PageIntro>
      {ready ? <Body /> : <RowSkeleton count={4} />}
    </PageBody>
  )
}

function Body() {
  const on = useEnabled("electricity.enabled")
  const d = useDraft()
  const zone = (d.get("electricity.zone") as string) ?? ""
  return (
    <Section
      title="Electricity Maps"
      action={
        <a
          href="https://app.electricitymaps.com"
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-1 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
        >
          Get a token <ExternalLinkIcon className="size-3" />
        </a>
      }
    >
      <SettingsCard>
        <SwitchRow id="electricity.enabled" />
        <TextRow
          id="electricity.zone"
          mono
          muted={!on}
          width="sm:w-28"
          validate={() => (on && !zone.trim() ? "Needed while Electricity Maps is on." : null)}
        />
        <SecretRow id="electricity.token" muted={!on} />
        <NumberRow id="electricity.poll_secs" min={60} unit="s" muted={!on} />
      </SettingsCard>
    </Section>
  )
}
