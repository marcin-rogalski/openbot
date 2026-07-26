import { Button, Flex, Popover, Portal, Text } from "@chakra-ui/react"
import { useState } from "react"

/** A button that asks for confirmation in an anchored popover before firing its
 * action. Used for destructive actions like deleting a bot. */
export function ConfirmButton({
  label,
  message,
  confirmLabel = "Delete",
  size = "sm",
  onConfirm,
}: {
  label: string
  message: string
  confirmLabel?: string
  size?: "xs" | "sm" | "md"
  onConfirm: () => void
}) {
  const [open, setOpen] = useState(false)
  return (
    <Popover.Root
      open={open}
      onOpenChange={(e) => setOpen(e.open)}
      positioning={{ placement: "top-start" }}
    >
      <Popover.Trigger asChild>
        <Button size={size} variant="ghost" colorPalette="red">
          {label}
        </Button>
      </Popover.Trigger>
      <Portal>
        <Popover.Positioner>
          <Popover.Content maxW="260px">
            <Popover.Arrow />
            <Popover.Body>
              <Text fontSize="sm" mb="3">
                {message}
              </Text>
              <Flex gap="2" justify="flex-end">
                <Button size="sm" variant="ghost" onClick={() => setOpen(false)}>
                  Cancel
                </Button>
                <Button
                  size="sm"
                  colorPalette="red"
                  onClick={() => {
                    setOpen(false)
                    onConfirm()
                  }}
                >
                  {confirmLabel}
                </Button>
              </Flex>
            </Popover.Body>
          </Popover.Content>
        </Popover.Positioner>
      </Portal>
    </Popover.Root>
  )
}
