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
import type { SecretPath } from "@/lib/types"

export default function Media() {
  const { ready } = useDraft()
  return (
    <PageBody>
      <RestoreDefaults page="/integrations/media" />
      <PageIntro title="Media & Jellyfin">
        The media role shows library counts from Prometheus and, with an API key, the download queue of
        Radarr, Sonarr and Lidarr. Jellyfin feeds the now playing and music roles.
      </PageIntro>
      {ready ? (
        <Body />
      ) : (
        <>
          <RowSkeleton count={2} />
          <RowSkeleton count={3} />
          <RowSkeleton count={4} />
        </>
      )}
    </PageBody>
  )
}

function Arr({ app, name, on }: { app: string; name: string; on: boolean }) {
  return (
    <Section title={name}>
      <SettingsCard>
        <TextRow id={`media.${app}.service`} mono muted={!on} />
        <NumberRow id={`media.${app}.port`} min={1} max={65535} muted={!on} />
        <SecretRow id={`media.${app}.api_key` as SecretPath} muted={!on} />
      </SettingsCard>
    </Section>
  )
}

function Body() {
  const media = useEnabled("media.enabled")
  const jf = useEnabled("jellyfin.enabled")
  const d = useDraft()
  const url = ((d.get("jellyfin.url") as string) ?? "").trim()
  return (
    <>
      <Section title="Media">
        <SettingsCard>
          <SwitchRow id="media.enabled" />
          <TextRow id="media.namespace" mono muted={!media} />
        </SettingsCard>
      </Section>
      <Arr app="radarr" name="Radarr" on={media} />
      <Arr app="sonarr" name="Sonarr" on={media} />
      <Arr app="lidarr" name="Lidarr" on={media} />
      <Section title="Jellyfin" description="The server lives outside the cluster, so it needs a full address.">
        <SettingsCard>
          <SwitchRow id="jellyfin.enabled" />
          <TextRow
            id="jellyfin.url"
            mono
            muted={!jf}
            width="sm:w-72"
            placeholder="http://192.168.0.10:8096"
            validate={() =>
              url && !/^https?:\/\//.test(url)
                ? "Start with http:// or https://."
                : jf && !url
                  ? "Needed while Jellyfin is on."
                  : null
            }
          />
          <SecretRow id="jellyfin.api_key" muted={!jf} />
          <NumberRow id="jellyfin.poll_secs" min={2} unit="s" muted={!jf} />
        </SettingsCard>
      </Section>
    </>
  )
}
