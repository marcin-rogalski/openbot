import { Box, Text } from "@chakra-ui/react"
import type { ReactNode } from "react"

/** A destructive-actions section, set off by a red divider + label. */
export function DangerZone({ children }: { children: ReactNode }) {
  return (
    <Box mt="2" pt="3" borderTopWidth="1px" borderColor="red.solid">
      <Text
        fontSize="xs"
        fontWeight="semibold"
        textTransform="uppercase"
        letterSpacing="0.04em"
        color="red.fg"
        mb="2"
      >
        Danger zone
      </Text>
      {children}
    </Box>
  )
}
