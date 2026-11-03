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
  TagsRow,
  useEnabled,
} from "@/components/settings"
import { useDraft } from "@/lib/draft"

const REPO = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/

export default function Github() {
  const { ready } = useDraft()
  return (
    <PageBody>
      <RestoreDefaults page="/integrations/github" />
      <PageIntro title="GitHub">
        Your contribution graph on the contributions role and recent Actions runs on the CI role.
      </PageIntro>
      {ready ? <Body /> : <RowSkeleton count={4} />}
    </PageBody>
  )
}

function Body() {
  const on = useEnabled("github.enabled")
  return (
    <Section
      title="GitHub"
      action={
        <a
          href="https://github.com/settings/personal-access-tokens/new"
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-1 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
        >
          New token <ExternalLinkIcon className="size-3" />
        </a>
      }
    >
      <SettingsCard>
        <SwitchRow id="github.enabled" />
        <SecretRow id="github.token" muted={!on} />
        <NumberRow id="github.poll_secs" min={60} unit="s" muted={!on} />
        <TagsRow
          id="github.ci_repos"
          placeholder="owner/repo"
          muted={!on}
          validate={(v) => (REPO.test(v) ? null : "Use owner/repo, for example SilkePilon/RackScreen.")}
        />
      </SettingsCard>
    </Section>
  )
}
