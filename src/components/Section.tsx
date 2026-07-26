import { Box, Flex, Stack, Text } from "@chakra-ui/react"
import type { ReactNode } from "react"

/** A titled group of controls: a header (title + optional caption + right-side
 * action slot) over a consistently-spaced body. The building block for tidy
 * settings tabs. */
export function Section({
  title,
  caption,
  action,
  children,
}: {
  title: ReactNode
  caption?: ReactNode
  action?: ReactNode
  children: ReactNode
}) {
  return (
    <Box>
      <Flex align="center" justify="space-between" gap="3" mb="3" minH="6">
        <Box minW="0">
          <Text fontSize="sm" fontWeight="semibold">
            {title}
          </Text>
          {caption ? (
            <Text fontSize="xs" color="fg.muted">
              {caption}
            </Text>
          ) : null}
        </Box>
        {action}
      </Flex>
      <Stack gap="3">{children}</Stack>
    </Box>
  )
}
