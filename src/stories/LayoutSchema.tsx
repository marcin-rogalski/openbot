import { Box, Flex, type FlexProps, Text } from "@chakra-ui/react"

function Region({ label, ...props }: { label: string } & FlexProps) {
  return (
    <Flex align="center" justify="center" fontSize="xs" color="fg.muted" {...props}>
      {label}
    </Flex>
  )
}

/** A schematic of the app shell. Left rail = panels (bots list, with its label
 * above it, + the settings button). Right column = flat, borderless content and
 * footer that sit directly on the window. A reference, not the real UI. */
export function LayoutSchema() {
  return (
    <Box
      h="520px"
      maxW="840px"
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="lg"
      overflow="hidden"
    >
      <Region
        label="Titlebar · window drag region"
        h="38px"
        bg="bg.panel"
        borderBottomWidth="1px"
        borderColor="border"
      />
      <Flex h="calc(100% - 38px)" p="2" gap="2">
        {/* Left rail: panels */}
        <Flex direction="column" w="210px" gap="2">
          <Text className="section-label">Bots</Text>
          <Region label="Bots list" flex="1" className="panel" color="fg.muted" />
          <Region label="General settings ⚙" h="52px" className="panel" />
        </Flex>

        {/* Right column: flat, borderless */}
        <Flex direction="column" flex="1" minW="0" gap="2">
          <Flex
            direction="column"
            flex="1"
            borderWidth="1px"
            borderStyle="dashed"
            borderColor="border"
            borderRadius="md"
          >
            <Region
              label="Tabs · pane title"
              h="48px"
              borderBottomWidth="1px"
              borderColor="border"
            />
            <Region label="Content · chat feed / settings pane" flex="1" />
          </Flex>
          <Region
            label="Footer · status (chat) / actions (settings)"
            h="52px"
            borderWidth="1px"
            borderStyle="dashed"
            borderColor="border"
            borderRadius="md"
          />
        </Flex>
      </Flex>
    </Box>
  )
}
