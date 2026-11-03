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
  TextRow,
  useEnabled,
} from "@/components/settings"
import { useDraft } from "@/lib/draft"

export default function Cluster() {
  const { ready } = useDraft()
  return (
    <PageBody>
      <RestoreDefaults page="/integrations/cluster" />
      <PageIntro title="Cluster">
        How RackScreen reaches your Kubernetes cluster and the services inside it. Services are reached through
        the kube API, so nothing needs to be exposed.
      </PageIntro>
      {ready ? <Body /> : <Skeletons />}
    </PageBody>
  )
}

function Skeletons() {
  return (
    <>
      <RowSkeleton count={1} />
      <RowSkeleton count={5} />
      <RowSkeleton count={4} />
    </>
  )
}

function Body() {
  const argo = useEnabled("argocd.enabled")
  const qbit = useEnabled("qbittorrent.enabled")
  return (
    <>
      <Section title="Kubernetes">
        <SettingsCard>
          <TextRow id="k8s.kubeconfig" mono placeholder="in-cluster" width="sm:w-72" />
        </SettingsCard>
      </Section>
      <Section title="Prometheus" description="Metrics for the CPU, memory, storage, network and alert roles.">
        <SettingsCard>
          <TextRow id="prometheus.namespace" mono />
          <TextRow id="prometheus.service" mono />
          <NumberRow id="prometheus.port" min={1} max={65535} />
          <NumberRow id="prometheus.poll_secs" min={1} unit="s" />
          <TagsRow id="prometheus.ignore_alerts" placeholder="AlertName" />
        </SettingsCard>
      </Section>
      <Section title="Argo CD">
        <SettingsCard>
          <SwitchRow id="argocd.enabled" />
          <TextRow id="argocd.namespace" mono muted={!argo} />
        </SettingsCard>
      </Section>
      <Section title="qBittorrent" description="Torrent traffic on the network ring and the media card.">
        <SettingsCard>
          <SwitchRow id="qbittorrent.enabled" />
          <TextRow id="qbittorrent.namespace" mono muted={!qbit} />
          <TextRow id="qbittorrent.service" mono muted={!qbit} />
          <NumberRow id="qbittorrent.port" min={1} max={65535} muted={!qbit} />
          <TextRow id="qbittorrent.user" muted={!qbit} placeholder="none" />
          <SecretRow id="qbittorrent.pass" muted={!qbit} />
          <NumberRow id="qbittorrent.poll_secs" min={1} unit="s" muted={!qbit} />
        </SettingsCard>
      </Section>
    </>
  )
}
