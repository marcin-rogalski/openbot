import { Badge, Box, Flex, Heading, Switch, Text } from "@chakra-ui/react"
import { useEffect, useRef } from "react"
import type { ActivityEvent, ActivityKind } from "../lib/bot"

const KIND_META: Record<ActivityKind, { label: string; palette: string }> = {
  message: { label: "message", palette: "blue" },
  model_call: { label: "model", palette: "purple" },
  tool_call: { label: "tool", palette: "orange" },
  reply: { label: "reply", palette: "green" },
  log: { label: "log", palette: "gray" },
}

// Internal activity hidden in normal mode; shown only with Debug on. Extend
// this set as more behind-the-scenes detail gets added.
const DEBUG_ONLY_KINDS = new Set<ActivityKind>(["model_call", "log"])

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function ActivityRow({ event }: { event: ActivityEvent }) {
  const meta = KIND_META[event.kind]
  return (
    <Flex className="activity-row" gap="3" align="baseline">
      <Text className="activity-time" fontSize="xs" color="fg.subtle">
        {formatTime(event.ts)}
      </Text>
      <Badge colorPalette={meta.palette} variant="subtle" flexShrink="0">
        {meta.label}
      </Badge>
      <Box minW="0">
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
          {event.content}
        </Text>
      </Box>
    </Flex>
  )
}

type Props = {
  events: ActivityEvent[]
  debug: boolean
  onDebugChange: (debug: boolean) => void
}

export function ChatPane({ events, debug, onDebugChange }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null)
  const visible = debug ? events : events.filter((e) => !DEBUG_ONLY_KINDS.has(e.kind))

  useEffect(() => {
    if (visible.length === 0) return
    bottomRef.current?.scrollIntoView({ behavior: "smooth" })
  }, [visible.length])

  return (
    <Flex direction="column" h="100%" minH="0">
      <Flex className="pane-title" align="center" justify="space-between">
        <Heading size="sm">Chat preview</Heading>
        <Switch.Root
          size="sm"
          checked={debug}
          onCheckedChange={(e) => onDebugChange(e.checked)}
        >
          <Switch.HiddenInput />
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
          <Switch.Label>Debug</Switch.Label>
        </Switch.Root>
      </Flex>

      <Box className="feed" flex="1" overflowY="auto">
        {visible.length === 0 ? (
          <Flex h="100%" align="center" justify="center">
            <Text color="fg.subtle">
              Nothing yet — press Start to watch the bot work.
            </Text>
          </Flex>
        ) : (
          <>
            {visible.map((event) => (
              <ActivityRow key={event.id} event={event} />
            ))}
            <div ref={bottomRef} />
          </>
        )}
      </Box>
    </Flex>
  )
}
