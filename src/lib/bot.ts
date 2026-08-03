import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useEffect, useState } from "react"

// --- Event/command contract (mirrors src-tauri/src/bot.rs) ------------------

export const STATUS_EVENT = "bot://status"
export const ACTIVITY_EVENT = "bot://activity"
export const STREAM_EVENT = "bot://stream"
export const BUSY_EVENT = "bot://busy"
export const METRICS_EVENT = "bot://metrics"
export const TOOL_APPROVAL_EVENT = "bot://tool-approval"
export const TOOL_APPROVAL_RESOLVED_EVENT = "bot://tool-approval-resolved"

export type ActivityKind = "message" | "model_call" | "tool_call" | "reply" | "log"

export type ActivityEvent = {
  botId: string
  id: string
  ts: number
  kind: ActivityKind
  author?: string
  channel?: string
  content: string
  /** Friendly one-liner shown when the feed is folded (non-verbose). */
  summary?: string
}

export type MetricsData = { prefillTps: number | null; inferenceTps: number | null }

export type ApprovalDecision = "approve" | "deny" | "always_allow" | "always_deny"
export type ToolApproval = { id: string; botId: string; tool: string; args: unknown }

// --- Lifecycle commands (per bot) -------------------------------------------

export const startBot = (botId: string): Promise<void> => invoke("start_bot", { botId })
export const stopBot = (botId: string): Promise<void> => invoke("stop_bot", { botId })
export const restartBot = (botId: string): Promise<void> =>
  invoke("restart_bot", { botId })
export const getRunningBots = (): Promise<string[]> => invoke("get_running_bots")

// --- Google Drive (global) --------------------------------------------------

export const connectDrive = (toolId: string): Promise<string> =>
  invoke("connect_drive", { toolId })
export const driveStatus = (toolId: string): Promise<boolean> =>
  invoke("drive_status", { toolId })

// --- Memory (per bot) -------------------------------------------------------

export type Memory = { id: string; kind: "note" | "rule"; text: string; created: number }

export const getMemories = (storeId: string): Promise<Memory[]> =>
  invoke("get_memories", { storeId })
export const deleteMemory = (storeId: string, id: string): Promise<void> =>
  invoke("delete_memory", { storeId, id })
export const clearMemories = (storeId: string): Promise<void> =>
  invoke("clear_memories", { storeId })

// --- Tool approvals ---------------------------------------------------------

export const resolveToolApproval = (
  id: string,
  decision: ApprovalDecision,
): Promise<void> => invoke("resolve_tool_approval", { id, decision })

// --- Hooks (all app-level; scoped by botId) ---------------------------------

/** Set of currently-running bot ids; seeds from the backend, follows status. */
export function useRunningBots(): Set<string> {
  const [running, setRunning] = useState<Set<string>>(new Set())

  useEffect(() => {
    getRunningBots().then((ids) => setRunning(new Set(ids)))
    const unlisten = listen<{ botId: string; running: boolean }>(STATUS_EVENT, (e) => {
      setRunning((prev) => {
        const next = new Set(prev)
        if (e.payload.running) next.add(e.payload.botId)
        else next.delete(e.payload.botId)
        return next
      })
    })
    return () => {
      unlisten.then((off) => off())
    }
  }, [])

  return running
}

/** A bot's live activity: `label` (left side, e.g. "🔎 Searching the web…") plus
 * an optional `detail` (right side, e.g. tool progress "42%"). */
export type BusyState = { label: string; detail: string | null }

/** Per-bot activity from `bot://busy`; a bot is absent from the map when idle. */
export function useBusyState(): Map<string, BusyState> {
  const [state, setState] = useState<Map<string, BusyState>>(new Map())

  useEffect(() => {
    const unlisten = listen<{
      botId: string
      label: string | null
      detail: string | null
    }>(BUSY_EVENT, (e) => {
      setState((prev) => {
        const next = new Map(prev)
        if (e.payload.label)
          next.set(e.payload.botId, { label: e.payload.label, detail: e.payload.detail })
        else next.delete(e.payload.botId)
        return next
      })
    })
    return () => {
      unlisten.then((off) => off())
    }
  }, [])

  return state
}

/** Per-bot activity feeds, accumulated from `bot://activity`; live model-call
 * entries are updated in place from `bot://stream`. */
export function useActivityFeeds(): Map<string, ActivityEvent[]> {
  const [feeds, setFeeds] = useState<Map<string, ActivityEvent[]>>(new Map())

  useEffect(() => {
    const unlisten = listen<ActivityEvent>(ACTIVITY_EVENT, (e) => {
      setFeeds((prev) => {
        const next = new Map(prev)
        next.set(e.payload.botId, [...(next.get(e.payload.botId) ?? []), e.payload])
        return next
      })
    })
    const unlistenStream = listen<{ botId: string; id: string; content: string }>(
      STREAM_EVENT,
      (e) => {
        setFeeds((prev) => {
          const list = prev.get(e.payload.botId)
          if (!list) return prev
          const next = new Map(prev)
          next.set(
            e.payload.botId,
            list.map((ev) =>
              ev.id === e.payload.id ? { ...ev, content: e.payload.content } : ev,
            ),
          )
          return next
        })
      },
    )
    return () => {
      unlisten.then((off) => off())
      unlistenStream.then((off) => off())
    }
  }, [])

  return feeds
}

/** Latest throughput per bot from `bot://metrics`. */
export function useMetricsByBot(): Map<string, MetricsData> {
  const [metrics, setMetrics] = useState<Map<string, MetricsData>>(new Map())

  useEffect(() => {
    const unlisten = listen<{ botId: string } & MetricsData>(METRICS_EVENT, (e) => {
      setMetrics((prev) => {
        const next = new Map(prev)
        next.set(e.payload.botId, {
          prefillTps: e.payload.prefillTps,
          inferenceTps: e.payload.inferenceTps,
        })
        return next
      })
    })
    return () => {
      unlisten.then((off) => off())
    }
  }, [])

  return metrics
}

/** Pending tool-approval requests + a resolver that removes them optimistically. */
export function useToolApprovals(): [
  ToolApproval[],
  (id: string, decision: ApprovalDecision) => void,
] {
  const [pending, setPending] = useState<ToolApproval[]>([])

  useEffect(() => {
    const unlistenReq = listen<ToolApproval>(TOOL_APPROVAL_EVENT, (e) => {
      setPending((prev) => [...prev, e.payload])
    })
    const unlistenDone = listen<{ id: string }>(TOOL_APPROVAL_RESOLVED_EVENT, (e) => {
      setPending((prev) => prev.filter((a) => a.id !== e.payload.id))
    })
    return () => {
      unlistenReq.then((off) => off())
      unlistenDone.then((off) => off())
    }
  }, [])

  const resolve = (id: string, decision: ApprovalDecision) => {
    setPending((prev) => prev.filter((a) => a.id !== id))
    void resolveToolApproval(id, decision)
  }

  return [pending, resolve]
}
