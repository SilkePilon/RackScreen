import * as React from "react"
import { LocateFixedIcon, MapPinIcon, TriangleAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { PageBody, PageIntro } from "@/components/page"
import { RestoreDefaults } from "@/components/restore-defaults"
import { NumberRow, Row, RowSkeleton, Section, SettingsCard, SwitchRow, useEnabled } from "@/components/settings"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useDraft } from "@/lib/draft"

export default function Sky() {
  const { ready } = useDraft()
  return (
    <PageBody>
      <RestoreDefaults page="/integrations/sky" />
      <PageIntro title="Sky">
        Weather, wind, air quality, rain, sun, moon and ISS passes. They all need to know where the rack lives;
        none of them needs a key.
      </PageIntro>
      {ready ? (
        <Body />
      ) : (
        <>
          <RowSkeleton count={3} />
          <RowSkeleton count={2} />
        </>
      )}
    </PageBody>
  )
}

function round(n: number) {
  return Math.round(n * 10_000) / 10_000
}

function Body() {
  const d = useDraft()
  const lat = d.get("location.lat") as number | null
  const lon = d.get("location.lon") as number | null
  const [locating, setLocating] = React.useState(false)
  const weather = useEnabled("weather.enabled")
  const rain = useEnabled("rain.enabled")
  const iss = useEnabled("iss.enabled")
  const missing = (weather || rain || iss) && (lat === null || lon === null)
  const half = (lat === null) !== (lon === null)

  const locate = () => {
    if (!("geolocation" in navigator) || !window.isSecureContext) {
      toast.error("Your browser only shares its location over https or on localhost", {
        description: "Type the coordinates instead; any map app shows them.",
      })
      return
    }
    setLocating(true)
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        d.update((c) => ({
          ...c,
          location: { lat: round(pos.coords.latitude), lon: round(pos.coords.longitude) },
        }))
        setLocating(false)
        toast.success("Location filled in", { description: "Rounded to about 10 m. Save to apply." })
      },
      (err) => {
        setLocating(false)
        toast.error("No location", { description: err.message })
      },
      { enableHighAccuracy: false, timeout: 15_000 },
    )
  }

  return (
    <>
      {missing && (
        <Alert className="border-warning/30 bg-warning/[0.06]">
          <TriangleAlertIcon className="text-warning!" />
          <AlertTitle>Set a location first</AlertTitle>
          <AlertDescription>Weather, rain and ISS need both a latitude and a longitude.</AlertDescription>
        </Alert>
      )}
      <Section
        title="Location"
        description="Decimal degrees. Only used on the Pi to ask for your local sky."
        action={
          <Button variant="outline" size="sm" onClick={locate} disabled={locating}>
            {locating ? <Spinner data-icon="inline-start" /> : <LocateFixedIcon data-icon="inline-start" />}
            Use my location
          </Button>
        }
      >
        <SettingsCard>
          <NumberRow id="location.lat" min={-90} max={90} step={0.01} integer={false} nullable placeholder="52.09" />
          <NumberRow id="location.lon" min={-180} max={180} step={0.01} integer={false} nullable placeholder="5.12" />
          <Row
            title="On the map"
            description={
              half ? (
                <span className="text-destructive">Fill in both, or clear both.</span>
              ) : lat !== null && lon !== null ? (
                `${lat.toFixed(4)}, ${lon.toFixed(4)}`
              ) : (
                "No location set."
              )
            }
          >
            <Button variant="ghost" size="sm" asChild disabled={lat === null || lon === null}>
              <a
                href={
                  lat !== null && lon !== null
                    ? `https://www.openstreetmap.org/?mlat=${lat}&mlon=${lon}#map=12/${lat}/${lon}`
                    : undefined
                }
                target="_blank"
                rel="noreferrer"
                aria-disabled={lat === null || lon === null}
              >
                <MapPinIcon data-icon="inline-start" />
                Open map
              </a>
            </Button>
          </Row>
        </SettingsCard>
      </Section>
      <Section title="Weather" description="Open-Meteo current conditions and air quality.">
        <SettingsCard>
          <SwitchRow id="weather.enabled" />
          <NumberRow id="weather.poll_secs" min={60} unit="s" muted={!weather} />
        </SettingsCard>
      </Section>
      <Section title="Rain" description="Buienradar two-hour nowcast, Netherlands and Belgium only.">
        <SettingsCard>
          <SwitchRow id="rain.enabled" />
          <NumberRow id="rain.poll_secs" min={60} unit="s" muted={!rain} />
        </SettingsCard>
      </Section>
      <Section title="ISS" description="Passes are computed on the Pi from the Celestrak orbit data.">
        <SettingsCard>
          <SwitchRow id="iss.enabled" />
          <NumberRow id="iss.min_elevation" min={0} max={90} unit="°" integer={false} muted={!iss} />
        </SettingsCard>
      </Section>
    </>
  )
}
