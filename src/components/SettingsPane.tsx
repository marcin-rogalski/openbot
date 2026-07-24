import { Box, Flex, Heading, Text } from "@chakra-ui/react"

const SECTIONS: { title: string; hint: string }[] = [
  {
    title: "Bot",
    hint: "Discord token, model server URL, and behaviour. Coming next.",
  },
  {
    title: "MCP Servers",
    hint: "Add and manage MCP servers (stdio / HTTP). Coming next.",
  },
]

export function SettingsPane() {
  return (
    <Flex direction="column" h="100%" minH="0">
      <Heading className="pane-title" size="sm">
        Settings
      </Heading>

      <Box className="feed" flex="1" overflowY="auto">
        <Flex direction="column" gap="4">
          {SECTIONS.map((section) => (
            <Box key={section.title} className="settings-card">
              <Heading size="xs" mb="1">
                {section.title}
              </Heading>
              <Text fontSize="sm" color="fg.muted">
                {section.hint}
              </Text>
            </Box>
          ))}
        </Flex>
      </Box>
    </Flex>
  )
}
