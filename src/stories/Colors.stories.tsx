import { Box, Flex, Heading, SimpleGrid, Stack, Text } from "@chakra-ui/react"
import type { Meta, StoryObj } from "@storybook/react-vite"

const meta: Meta = { title: "Design/Colors" }
export default meta
type Story = StoryObj

const SHADES = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950]

function Scale({ name }: { name: string }) {
  return (
    <Box>
      <Text fontSize="sm" fontWeight="medium" mb="2">
        {name}
      </Text>
      <Flex borderRadius="md" overflow="hidden" borderWidth="1px" borderColor="border">
        {SHADES.map((s) => (
          <Box key={s} flex="1" bg={`${name}.${s}`} h="44px" />
        ))}
      </Flex>
      <Flex mt="1">
        {SHADES.map((s) => (
          <Text key={s} flex="1" fontSize="10px" color="fg.subtle" textAlign="center">
            {s}
          </Text>
        ))}
      </Flex>
    </Box>
  )
}

function Token({ token }: { token: string }) {
  return (
    <Flex align="center" gap="3">
      <Box
        w="44px"
        h="44px"
        borderRadius="md"
        bg={token}
        borderWidth="1px"
        borderColor="border"
        flexShrink="0"
      />
      <Text fontSize="sm" fontFamily="mono">
        {token}
      </Text>
    </Flex>
  )
}

export const Palette: Story = {
  render: () => (
    <Stack gap="8" maxW="760px">
      <Box>
        <Heading size="md" mb="4">
          Color scales
        </Heading>
        <Stack gap="5">
          <Scale name="brand" />
          <Scale name="neutral" />
        </Stack>
      </Box>

      <Box>
        <Heading size="md" mb="4">
          Surfaces
        </Heading>
        <SimpleGrid columns={{ base: 1, sm: 2 }} gap="3">
          {["bg", "bg.subtle", "bg.muted", "bg.panel", "bg.emphasized"].map((t) => (
            <Token key={t} token={t} />
          ))}
        </SimpleGrid>
      </Box>

      <Box>
        <Heading size="md" mb="4">
          Text &amp; border
        </Heading>
        <SimpleGrid columns={{ base: 1, sm: 2 }} gap="3">
          {["fg", "fg.muted", "fg.subtle", "border", "border.emphasized"].map((t) => (
            <Token key={t} token={t} />
          ))}
        </SimpleGrid>
      </Box>

      <Box>
        <Heading size="md" mb="4">
          Brand palette tokens
        </Heading>
        <SimpleGrid columns={{ base: 1, sm: 2 }} gap="3">
          {[
            "brand.solid",
            "brand.fg",
            "brand.muted",
            "brand.subtle",
            "brand.emphasized",
          ].map((t) => (
            <Token key={t} token={t} />
          ))}
        </SimpleGrid>
      </Box>
    </Stack>
  ),
}
