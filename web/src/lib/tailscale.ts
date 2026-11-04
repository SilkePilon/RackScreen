import type { TailscaleDevice, TailscaleState } from "@/lib/types"

/** The name the admin console shows: the first label of the MagicDNS name. */
export function deviceName(d: TailscaleDevice) {
  return d.dns_name ? d.dns_name.split(".")[0] : d.hostname
}

/** The Pi's MagicDNS name when Tailscale is up, else null. */
export function tailscaleAddress(t: TailscaleState | undefined) {
  return t?.backend_state === "Running" && t.self?.dns_name ? t.self.dns_name : null
}
