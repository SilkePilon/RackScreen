import { QueryClient, useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { api, ApiError, errorMessage } from "@/lib/api"
import type { NightMode } from "@/lib/types"

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 10_000,
      refetchOnWindowFocus: false,
      retry: (count, err) => {
        if (err instanceof ApiError && (err.status === 401 || err.status === 403 || err.status === 404))
          return false
        return count < 2
      },
    },
  },
})

export const keys = {
  auth: ["auth"] as const,
  info: ["info"] as const,
  status: ["status"] as const,
  roles: ["roles"] as const,
  shows: ["shows"] as const,
  config: ["config"] as const,
  yaml: ["config", "yaml"] as const,
  spotify: ["spotify"] as const,
  update: ["update"] as const,
  system: ["system"] as const,
  gifMemory: ["gifs", "memory"] as const,
  tailscale: ["tailscale"] as const,
  tailscaleSetup: ["tailscale", "setup"] as const,
}

export const useAuth = () => useQuery({ queryKey: keys.auth, queryFn: api.auth, retry: false })
export const useInfo = () => useQuery({ queryKey: keys.info, queryFn: api.info, staleTime: 60_000 })
export const useStatus = () =>
  useQuery({
    queryKey: keys.status,
    queryFn: async () => ({ ...(await api.status()), received: performance.now() }),
    refetchInterval: 1500,
    refetchIntervalInBackground: false,
    retry: false,
    staleTime: 0,
  })
// CPU usage of the last ~2 minutes, kept across visits to the System page.
const CPU_HISTORY = 25
const cpuHistory: { at: number; pct: number }[] = []
export const getCpuHistory = () => cpuHistory.slice()
async function fetchSystem() {
  const s = await api.system()
  if (s.cpu.usage_pct !== null) {
    cpuHistory.push({ at: Date.now(), pct: s.cpu.usage_pct })
    while (cpuHistory.length > CPU_HISTORY || (cpuHistory.length && Date.now() - cpuHistory[0].at > 130_000))
      cpuHistory.shift()
  }
  return s
}

export const useSystem = () =>
  useQuery({
    queryKey: keys.system,
    queryFn: fetchSystem,
    refetchInterval: 5000,
    refetchIntervalInBackground: false,
    staleTime: 0,
  })
export const useGifMemory = () =>
  useQuery({
    queryKey: keys.gifMemory,
    queryFn: api.gifMemory,
    refetchInterval: 3000,
    refetchIntervalInBackground: false,
    staleTime: 0,
  })
export const useRoles = () => useQuery({ queryKey: keys.roles, queryFn: api.roles, staleTime: 30_000 })
export const useShows = () => useQuery({ queryKey: keys.shows, queryFn: api.shows, staleTime: Infinity })
export const useConfig = () => useQuery({ queryKey: keys.config, queryFn: api.config, staleTime: Infinity })
export const useSpotify = () =>
  useQuery({ queryKey: keys.spotify, queryFn: api.spotify, refetchInterval: 5000 })
export const useUpdate = () =>
  useQuery({
    queryKey: keys.update,
    queryFn: api.update,
    staleTime: 60_000,
    refetchInterval: (q) => (q.state.data?.job.state === "running" ? 1000 : 5 * 60_000),
  })

/** Tailscale; `every` ms polling (false: none, for pages that only peek). */
export const useTailscale = (every: number | false = false) =>
  useQuery({
    queryKey: keys.tailscale,
    queryFn: api.tailscale,
    refetchInterval: every,
    refetchIntervalInBackground: false,
    staleTime: every === false ? 60_000 : 0,
  })
export const useTailscaleSetup = (running: boolean) =>
  useQuery({
    queryKey: keys.tailscaleSetup,
    queryFn: api.tailscaleSetupJob,
    refetchInterval: running ? 1000 : false,
    staleTime: 0,
  })

/** A fire-and-forget control action with a toast on success or failure. */
export function useControl<A>(fn: (arg: A) => Promise<unknown>, success?: (arg: A) => string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: fn,
    onSuccess: (_d, arg) => {
      if (success) toast.success(success(arg))
      void qc.invalidateQueries({ queryKey: keys.status })
    },
    onError: (e) => toast.error(errorMessage(e)),
  })
}

export const nightLabel: Record<NightMode, string> = {
  auto: "Night mode follows the schedule",
  on: "Night mode on",
  off: "Night mode off",
}
