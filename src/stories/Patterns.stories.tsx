import { Badge, Box, Flex, Heading, Stack, Text } from "@chakra-ui/react"
import type { Meta, StoryObj } from "@storybook/react-vite"
import { CollapsibleRow } from "../components/CollapsibleRow"

const meta: Meta = { title: "Design/Patterns" }
export default meta
type Story = StoryObj

export const Layout: Story = {
  render: () => (
    <Stack gap="8" maxW="560px">
      <Box>
        <Heading size="md" mb="3">
          List rows
        </Heading>
        <Stack gap="2">
          {["Google Drive", "Web Search", "Case notes"].map((name) => (
            <Flex key={name} className="list-row" align="center" gap="2">
              <Text flex="1" minW="0" truncate fontSize="sm" fontWeight="medium">
                {name}
              </Text>
              <Badge colorPalette="gray" variant="subtle">
                enabled
              </Badge>
            </Flex>
          ))}
        </Stack>
      </Box>

      <Box>
        <Heading size="md" mb="3">
          Bot rows
        </Heading>
        <Box className="sidebar" p="2" bg="bg.panel" borderRadius="lg" w="240px">
          {[
            { name: "Mietek", color: "#5865f2", on: true },
            { name: "Research", color: "#22a06b", on: false },
          ].map((b) => (
            <Flex key={b.name} className="bot-row" align="center" gap="2">
              <Flex
                className="bot-avatar"
                align="center"
                justify="center"
                style={{ background: b.color }}
              >
                {b.name[0]}
              </Flex>
              <Text flex="1" fontSize="sm" fontWeight="medium">
                {b.name}
              </Text>
              <Box className={`status-dot ${b.on ? "is-on" : "is-off"}`} />
            </Flex>
          ))}
        </Box>
      </Box>

      <Box>
        <Heading size="md" mb="3">
          Collapsible list
        </Heading>
        <CollapsibleRow
          title="Approvals"
          meta={
            <Text fontSize="xs" color="fg.muted">
              5 actions
            </Text>
          }
        >
          <Stack gap="1.5">
            {["Search", "Read", "Create", "Delete", "Reindex"].map((op) => (
              <Flex key={op} align="center" justify="space-between" gap="3">
                <Text fontSize="sm">{op}</Text>
                <select className="policy-select" defaultValue="allow">
                  <option value="allow">Allow</option>
                  <option value="ask">Ask</option>
                  <option value="deny">Deny</option>
                </select>
              </Flex>
            ))}
          </Stack>
        </CollapsibleRow>
      </Box>

      <Box>
        <Heading size="md" mb="3">
          Footer / status bar
        </Heading>
        <Flex
          as="footer"
          className="footer-bar status-bar"
          align="center"
          justify="space-between"
          borderRadius="lg"
        >
          <Flex align="center" gap="2">
            <Box className="status-dot is-on" />
            <Text>Running</Text>
          </Flex>
          <Text>
            <Text as="span" color="fg.subtle">
              speed
            </Text>{" "}
            42 tok/s
          </Text>
        </Flex>
      </Box>
    </Stack>
  ),
}
