import { Box, Flex, IconButton, Switch, Text } from "@chakra-ui/react"
import type { BotConfig } from "../lib/config"

function Avatar({ bot }: { bot: BotConfig }) {
  const initial = (bot.name.trim()[0] ?? "?").toUpperCase()
  return (
    <Flex
      className="bot-avatar"
      style={{ background: bot.color }}
      align="center"
      justify="center"
    >
      {initial}
    </Flex>
  )
}

type Props = {
  bots: BotConfig[]
  runningIds: Set<string>
  selectedBotId: string | null
  active: boolean // is a bot (vs global settings) the current view
  onSelectBot: (id: string) => void
  onToggleBot: (id: string, run: boolean) => void
  onAddBot: () => void
}

export function Sidebar({
  bots,
  runningIds,
  selectedBotId,
  active,
  onSelectBot,
  onToggleBot,
  onAddBot,
}: Props) {
  return (
    <Flex as="nav" className="sidebar" direction="column" gap="2">
      <Flex align="center" justify="space-between" px="1">
        <Text className="section-label">Bots</Text>
        <IconButton
          aria-label="Add bot"
          size="2xs"
          variant="ghost"
          colorPalette="gray"
          onClick={onAddBot}
        >
          +
        </IconButton>
      </Flex>
      <Flex direction="column" className="panel" flex="1" minH="0" p="2">
        <Box className="bot-list" flex="1" overflowY="auto">
          {bots.map((bot) => {
            const running = runningIds.has(bot.id)
            const selected = active && selectedBotId === bot.id
            return (
              <Flex
                key={bot.id}
                className={`bot-row ${selected ? "is-selected" : ""}`}
                align="center"
                gap="2"
                onClick={() => onSelectBot(bot.id)}
              >
                <Avatar bot={bot} />
                <Text flex="1" minW="0" truncate fontSize="sm" fontWeight="medium">
                  {bot.name || "Untitled"}
                </Text>
                <Box onClick={(e) => e.stopPropagation()}>
                  <Switch.Root
                    size="sm"
                    checked={running}
                    onCheckedChange={(e) => onToggleBot(bot.id, e.checked)}
                  >
                    <Switch.HiddenInput />
                    <Switch.Control>
                      <Switch.Thumb />
                    </Switch.Control>
                  </Switch.Root>
                </Box>
              </Flex>
            )
          })}
          {bots.length === 0 ? (
            <Text fontSize="sm" color="fg.subtle" px="2" py="3">
              No bots yet — press + to add one.
            </Text>
          ) : null}
        </Box>
      </Flex>
    </Flex>
  )
}
