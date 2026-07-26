import { Box, Flex, Switch, Text } from "@chakra-ui/react"
import { useState } from "react"
import { BotView } from "../components/BotView"
import type { ActivityEvent } from "../lib/bot"
import {
  type BotConfig,
  DEFAULT_GLOBAL,
  type GlobalConfig,
  newBot,
  newToolInstance,
} from "../lib/config"

// A working example of the shell: real Sidebar-style rail + the real BotView,
// with the content column flat/borderless and the bots label above its panel.

function makeData() {
  const drive = { ...newToolInstance("google_drive"), name: "Case files" }
  drive.clientId = "id"
  drive.clientSecret = "secret"
  drive.folderId = "folder"
  const global: GlobalConfig = { ...DEFAULT_GLOBAL, tools: [drive] }
  const mietek: BotConfig = { ...newBot(0), name: "Mietek", enabledToolIds: [drive.id] }
  const research: BotConfig = { ...newBot(1), name: "Research" }
  return { global, bots: [mietek, research] }
}

function feed(): ActivityEvent[] {
  const now = Date.now()
  return [
    {
      botId: "x",
      id: "1",
      ts: now - 60000,
      kind: "message",
      author: "marcin",
      channel: "general",
      content: "@Mietek summarise the case files",
    },
    { botId: "x", id: "2", ts: now - 52000, kind: "model_call", content: "" },
    {
      botId: "x",
      id: "3",
      ts: now - 44000,
      kind: "tool_call",
      content: 'drive_ask {"question":"case"}',
      summary: '📚 Consulted the knowledge base for "case" — 4 results',
    },
    {
      botId: "x",
      id: "4",
      ts: now - 30000,
      kind: "reply",
      author: "Mietek",
      channel: "general",
      content: "Here's a summary across the four case files: …",
    },
  ]
}

function BotRow({
  bot,
  running,
  selected,
  onSelect,
}: {
  bot: BotConfig
  running: boolean
  selected: boolean
  onSelect: () => void
}) {
  return (
    <Flex
      className={`bot-row ${selected ? "is-selected" : ""}`}
      align="center"
      gap="2"
      onClick={onSelect}
    >
      <Flex
        className="bot-avatar"
        align="center"
        justify="center"
        style={{ background: bot.color }}
      >
        {(bot.name.trim()[0] ?? "?").toUpperCase()}
      </Flex>
      <Text flex="1" minW="0" truncate fontSize="sm" fontWeight="medium">
        {bot.name}
      </Text>
      <Box onClick={(e) => e.stopPropagation()}>
        <Switch.Root size="sm" checked={running} colorPalette="brand">
          <Switch.HiddenInput />
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
        </Switch.Root>
      </Box>
    </Flex>
  )
}

export function LayoutLive() {
  const [{ global, bots }] = useState(makeData)
  const [selectedId, setSelectedId] = useState(bots[0].id)
  const [verbose, setVerbose] = useState(false)
  const [events] = useState(feed)
  const selected = bots.find((b) => b.id === selectedId) ?? bots[0]

  return (
    <Flex
      className="app-shell"
      colorPalette="brand"
      direction="column"
      h="600px"
      maxW="900px"
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="lg"
      overflow="hidden"
    >
      <Box h="34px" flexShrink="0" />
      <Flex flex="1" minH="0" px="2" pb="2" gap="2">
        {/* Left rail: label above the bots panel */}
        <Flex direction="column" w="210px" gap="2">
          <Text className="section-label">Bots</Text>
          <Flex direction="column" className="panel" flex="1" minH="0" p="2" gap="1">
            {bots.map((bot) => (
              <BotRow
                key={bot.id}
                bot={bot}
                running={bot.id === bots[0].id}
                selected={bot.id === selectedId}
                onSelect={() => setSelectedId(bot.id)}
              />
            ))}
          </Flex>
          <Flex className="panel sidebar-foot" align="center" gap="2">
            <Text fontSize="lg">⚙</Text>
            <Text fontSize="sm" fontWeight="medium">
              General settings
            </Text>
          </Flex>
        </Flex>

        {/* Right column: flat / borderless content + footer */}
        <Box flex="1" minW="0" className="rail-plain">
          <BotView
            key={selected.id}
            bot={selected}
            global={global}
            events={events}
            running={selected.id === bots[0].id}
            metrics={{ prefillTps: null, inferenceTps: 42 }}
            verbose={verbose}
            onVerboseChange={setVerbose}
            onSaveBot={() => {}}
            onDelete={() => {}}
          />
        </Box>
      </Flex>
    </Flex>
  )
}
