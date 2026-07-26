import { Button, Flex, Text } from "@chakra-ui/react"
import type { ReactNode } from "react"
import { FooterBar } from "./FooterBar"

/** Bottom bar for settings tabs — always present (so the footer aligns with the
 * sidebar). Save/Discard appear on the right when there are unsaved changes;
 * `left` holds always-available actions (e.g. Delete). */
export function ActionBar({
  dirty,
  hint,
  onSave,
  onDiscard,
  left,
}: {
  dirty: boolean
  hint?: string
  onSave: () => void
  onDiscard: () => void
  left?: ReactNode
}) {
  return (
    <FooterBar>
      <Flex align="center" gap="3" minW="0">
        {left}
        {dirty && hint ? (
          <Text fontSize="sm" color="fg.subtle" truncate>
            {hint}
          </Text>
        ) : null}
      </Flex>
      {dirty ? (
        <Flex gap="2" flexShrink="0">
          <Button size="sm" variant="ghost" onClick={onDiscard}>
            Discard
          </Button>
          <Button size="sm" colorPalette="brand" onClick={onSave}>
            Save
          </Button>
        </Flex>
      ) : null}
    </FooterBar>
  )
}
