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

// Internal activity hidden when folded; shown only in verbose mode. Model calls
// stay visible (as a collapsed "Thinking" block) so a loop is spottable.
export const VERBOSE_ONLY_KINDS = new Set<ActivityKind>(["log"])

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
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
          {open ? "▾" : "▸"} 🤔 Thinking
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

function ActivityRow({ event, verbose }: { event: ActivityEvent; verbose: boolean }) {
  if (event.kind === "model_call") {
    return <ThinkingRow event={event} verbose={verbose} />
  }
  const meta = KIND_META[event.kind]
  // Folded: show the friendly summary when one exists (tool calls).
  const text = !verbose && event.summary ? event.summary : event.content
  return (
    <Flex className="activity-row" gap="3" align="baseline">
      <Badge colorPalette={meta.palette} variant="subtle" flexShrink="0">
        {meta.label}
      </Badge>
      <Box minW="0" flex="1">
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

export function ActivityFeed({
  events,
  verbose,
}: {
  events: ActivityEvent[]
  verbose: boolean
}) {
  const bottomRef = useRef<HTMLDivElement>(null)
  const visible = verbose ? events : events.filter((e) => !VERBOSE_ONLY_KINDS.has(e.kind))

  useEffect(() => {
    if (visible.length === 0) return
    bottomRef.current?.scrollIntoView({ behavior: "smooth" })
  }, [visible.length])

  return (
    <Box className="feed" flex="1" overflowY="auto">
      {visible.length === 0 ? (
        <Flex h="100%" align="center" justify="center">
          <Text color="fg.subtle">Nothing yet — turn this bot on to watch it work.</Text>
        </Flex>
      ) : (
        <>
          {visible.map((event) => (
            <ActivityRow key={event.id} event={event} verbose={verbose} />
          ))}
          <div ref={bottomRef} />
        </>
      )}
    </Box>
  )
}
