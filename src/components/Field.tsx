import { Box, Text } from "@chakra-ui/react"
import type { ReactNode } from "react"

export function Field({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: ReactNode
}) {
  return (
    <Box>
      <Text fontSize="sm" fontWeight="medium" mb="1.5">
        {label}
      </Text>
      {children}
      {hint ? (
        <Text fontSize="xs" color="fg.subtle" mt="1.5">
          {hint}
        </Text>
      ) : null}
    </Box>
  )
}
