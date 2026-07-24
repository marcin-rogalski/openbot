import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useEffect, useState } from "react"

// --- Event/command contract (mirrors src-tauri/src/bot.rs) ------------------

export const STATUS_EVENT = "bot://status"
export const ACTIVITY_EVENT = "bot://activity"
export const METRICS_EVENT = "bot://metrics"
export const TOOL_APPROVAL_EVENT = "bot://tool-approval"
export const TOOL_APPROVAL_RESOLVED_EVENT = "bot://tool-approval-resolved"

export type BotStatus = { running: boolean }

export type Metrics = {
  prefillTps: number | null
  inferenceTps: number | null
}

export type ActivityKind = "message" | "model_call" | "tool_call" | "reply" | "log"

export type ActivityEvent = {
  id: string
  ts: number
  kind: ActivityKind
  author?: string
  channel?: string
  content: string
}

// --- Command wrappers -------------------------------------------------------

export const startBot = (): Promise<void> => invoke("start_bot")
export const stopBot = (): Promise<void> => invoke("stop_bot")
export const getBotStatus = (): Promise<BotStatus> => invoke("get_bot_status")
/** Restart the bot to apply new settings (no-op if it isn't running). */
export const restartBot = (): Promise<void> => invoke("restart_bot")

/** Run the Google OAuth flow; resolves to the connected account email. */
export const connectDrive = (): Promise<string> => invoke("connect_drive")
/** Whether Drive already has a cached token (no network). */
export const driveStatus = (): Promise<boolean> => invoke("drive_status")

export type ApprovalDecision = "approve" | "deny" | "always_allow" | "always_deny"
export type ToolApproval = { id: string; tool: string; args: unknown }

export const resolveToolApproval = (
  id: string,
  decision: ApprovalDecision,
): Promise<void> => invoke("resolve_tool_approval", { id, decision })

// --- Hooks ------------------------------------------------------------------

/**
 * Tracks the bot's running state: seeds from `get_bot_status` on mount, then
 * follows every `bot://status` event. The backend is the source of truth — we
 * never set this optimistically.
 */
export function useBotStatus(): boolean {
  const [running, setRunning] = useState(false)

  useEffect(() => {
    let active = true
    getBotStatus().then((s) => {
      if (active) setRunning(s.running)
    })
    const unlisten = listen<BotStatus>(STATUS_EVENT, (e) => {
      setRunning(e.payload.running)
    })
    return () => {
      active = false
      unlisten.then((off) => off())
    }
  }, [])

  return running
}

/** Latest throughput numbers from `bot://metrics` (nulls until first sample). */
export function useMetrics(): Metrics {
  const [metrics, setMetrics] = useState<Metrics>({
    prefillTps: null,
    inferenceTps: null,
  })

  useEffect(() => {
    const unlisten = listen<Metrics>(METRICS_EVENT, (e) => {
      setMetrics(e.payload)
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
    // Drop a card when the backend resolves it (including on timeout).
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

/** Accumulates `bot://activity` events into a feed for the chat preview. */
export function useActivityFeed(): ActivityEvent[] {
  const [events, setEvents] = useState<ActivityEvent[]>([])

  useEffect(() => {
    const unlisten = listen<ActivityEvent>(ACTIVITY_EVENT, (e) => {
      setEvents((prev) => [...prev, e.payload])
    })
    return () => {
      unlisten.then((off) => off())
    }
  }, [])

  return events
}
