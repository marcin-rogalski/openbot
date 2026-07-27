import { Badge, Box, Flex, Text } from "@chakra-ui/react"
import { useEffect, useRef, useState } from "react"
import type { ActivityEvent, ActivityKind } from "../lib/bot"

const KIND_META: Record<ActivityKind, { label: string; palette: string }> = {
  message: { label: "message", palette: "blue" },
  model_call: { label: "model", palette: "purple" },
  tool_call: { label: "tool", palette: "orange" },
  reply: { label: "reply", palette: "green" },
  log: { label: "log", palette: "gray" },
}

// Hidden when folded; shown only in verbose. Thinking (model_call) is internal —
// in non-verbose the status bar shows a "Thinking…" indicator instead.
export const VERBOSE_ONLY_KINDS = new Set<ActivityKind>(["log", "model_call"])

// Content longer than this (or multi-line) collapses to one line by default.
const LONG_TEXT = 140

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

/** Drop a leading icon/emoji so folded tool summaries read as plain text. */
function stripIcon(s: string): string {
  return s.replace(/^[^\p{L}\p{N}]+/u, "")
}

function Timestamp({ ts, verbose }: { ts: number; verbose: boolean }) {
  return (
    <Text
      className={`activity-time ${verbose ? "is-visible" : ""}`}
      fontSize="xs"
      color="fg.subtle"
    >
      {formatTime(ts)}
    </Text>
  )
}

function ThinkingRow({ event, verbose }: { event: ActivityEvent; verbose: boolean }) {
  const [open, setOpen] = useState(false)
  return (
    <Flex className="activity-row" gap="3" align="baseline">
      <Box minW="0" flex="1">
        <button
          type="button"
          className="thinking-toggle"
          onClick={() => setOpen((o) => !o)}
        >
          {open ? "▾" : "▸"} Thinking
          {!open ? (
            <Text as="span" color="fg.subtle" ml="1">
              {event.content ? `· ${event.content.length} chars` : "…"}
            </Text>
          ) : null}
        </button>
        {open ? <Box className="thinking-body">{event.content || "…"}</Box> : null}
      </Box>
      <Timestamp ts={event.ts} verbose={verbose} />
    </Flex>
  )
}

/** A run of consecutive tool calls with the same summary, folded into one row. */
function GroupedToolRow({
  summary,
  events,
}: {
  summary: string
  events: ActivityEvent[]
}) {
  const [open, setOpen] = useState(false)
  return (
    <Flex className="activity-row" gap="3" align="baseline">
      <Badge colorPalette="orange" variant="subtle" flexShrink="0">
        tool
      </Badge>
      <Box minW="0" flex="1">
        <button
          type="button"
          className="thinking-toggle"
          onClick={() => setOpen((o) => !o)}
        >
          {open ? "▾" : "▸"} {stripIcon(summary)}
          <Text as="span" color="fg.subtle" ml="1">
            ×{events.length}
          </Text>
        </button>
        {open ? (
          <Box className="thinking-body">
            {events.map((e) => (
              <div key={e.id}>
                {formatTime(e.ts)} · {e.content}
              </div>
            ))}
          </Box>
        ) : null}
      </Box>
      <Timestamp ts={events[events.length - 1].ts} verbose={false} />
    </Flex>
  )
}

function ActivityRow({ event, verbose }: { event: ActivityEvent; verbose: boolean }) {
  const [open, setOpen] = useState(false)
  if (event.kind === "model_call") {
    return <ThinkingRow event={event} verbose={verbose} />
  }
  const meta = KIND_META[event.kind]
  const text =
    !verbose && event.kind === "tool_call" && event.summary
      ? stripIcon(event.summary)
      : event.content
  const long = text.length > LONG_TEXT || text.includes("\n")

  return (
    <Flex className="activity-row" gap="3" align="baseline">
      <Badge colorPalette={meta.palette} variant="subtle" flexShrink="0">
        {meta.label}
      </Badge>
      <Box
        minW="0"
        flex="1"
        className={long ? (open ? "msg-wrap" : "msg-clamp") : undefined}
        cursor={long ? "pointer" : undefined}
        onClick={long ? () => setOpen((o) => !o) : undefined}
      >
        {event.channel ? (
          <Text as="span" color="fg.subtle" mr="2">
            #{event.channel}
          </Text>
        ) : null}
        {event.author ? (
          <Text as="span" fontWeight="semibold" mr="2">
            {event.author}
          </Text>
        ) : null}
        <Text as="span" color="fg">
          {text}
        </Text>
      </Box>
      <Timestamp ts={event.ts} verbose={verbose} />
    </Flex>
  )
}

type Item =
  | { kind: "event"; event: ActivityEvent }
  | { kind: "group"; summary: string; events: ActivityEvent[] }

/** Fold consecutive same-summary tool calls into groups (non-verbose only). */
function toItems(events: ActivityEvent[], verbose: boolean): Item[] {
  if (verbose) return events.map((event) => ({ kind: "event", event }))
  const items: Item[] = []
  for (const event of events) {
    const last = items[items.length - 1]
    if (event.kind === "tool_call" && event.summary) {
      if (last?.kind === "group" && last.summary === event.summary) {
        last.events.push(event)
      } else {
        items.push({ kind: "group", summary: event.summary, events: [event] })
      }
    } else {
      items.push({ kind: "event", event })
    }
  }
  return items
}

export function ActivityFeed({
  events,
  verbose,
}: {
  events: ActivityEvent[]
  verbose: boolean
}) {
  const bottomRef = useRef<HTMLDivElement>(null)
  const visible = verbose ? events : events.filter((e) => !VERBOSE_ONLY_KINDS.has(e.kind))
  const items = toItems(visible, verbose)

  useEffect(() => {
    if (visible.length === 0) return
    bottomRef.current?.scrollIntoView({ behavior: "smooth" })
  }, [visible.length])

  return (
    <Box className="feed chat-feed" flex="1" overflowY="auto">
      {items.length === 0 ? (
        <Flex h="100%" align="center" justify="center">
          <Text color="fg.subtle">Nothing yet — turn this bot on to watch it work.</Text>
        </Flex>
      ) : (
        <>
          {items.map((item) => {
            if (item.kind === "event") {
              return (
                <ActivityRow key={item.event.id} event={item.event} verbose={verbose} />
              )
            }
            if (item.events.length === 1) {
              return (
                <ActivityRow
                  key={item.events[0].id}
                  event={item.events[0]}
                  verbose={verbose}
                />
              )
            }
            return (
              <GroupedToolRow
                key={`group-${item.events[0].id}`}
                summary={item.summary}
                events={item.events}
              />
            )
          })}
          <div ref={bottomRef} />
        </>
      )}
    </Box>
  )
}
