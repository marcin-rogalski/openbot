import { Badge, Box, Flex, Heading, Text } from "@chakra-ui/react"
import { useEffect, useRef } from "react"
import { type ActivityEvent, type ActivityKind, useActivityFeed } from "../lib/bot"

const KIND_META: Record<ActivityKind, { label: string; palette: string }> = {
  message: { label: "message", palette: "blue" },
  model_call: { label: "model", palette: "purple" },
  tool_call: { label: "tool", palette: "orange" },
  reply: { label: "reply", palette: "green" },
  log: { label: "log", palette: "gray" },
}

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

export function ChatPane() {
  const events = useActivityFeed()
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (events.length === 0) return
    bottomRef.current?.scrollIntoView({ behavior: "smooth" })
  }, [events.length])

  return (
    <Flex direction="column" h="100%" minH="0">
      <Heading className="pane-title" size="sm">
        Chat preview
      </Heading>

      <Box className="feed" flex="1" overflowY="auto">
        {events.length === 0 ? (
          <Flex h="100%" align="center" justify="center">
            <Text color="fg.subtle">
              Nothing yet — press Start to watch the bot work.
            </Text>
          </Flex>
        ) : (
          <>
            {events.map((event) => (
              <ActivityRow key={event.id} event={event} />
            ))}
            <div ref={bottomRef} />
          </>
        )}
      </Box>
    </Flex>
  )
}
