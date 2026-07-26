import { Box, Collapsible, Flex } from "@chakra-ui/react"
import type { ReactNode } from "react"
import { LuChevronRight } from "react-icons/lu"

/** A self-contained card whose body expands/collapses on click. Controlled when
 * `open`/`onOpenChange` are passed, else uncontrolled via `defaultOpen`. */
export function CollapsibleRow({
  title,
  meta,
  defaultOpen = false,
  open,
  onOpenChange,
  children,
}: {
  title: ReactNode
  meta?: ReactNode
  defaultOpen?: boolean
  open?: boolean
  onOpenChange?: (open: boolean) => void
  children: ReactNode
}) {
  const rootProps =
    open !== undefined
      ? { open, onOpenChange: (e: { open: boolean }) => onOpenChange?.(e.open) }
      : { defaultOpen }
  return (
    <Collapsible.Root {...rootProps} className="collapsible">
      <Collapsible.Trigger asChild>
        <Flex className="collapsible-head" align="center" gap="2">
          <LuChevronRight className="collapsible-caret" size={18} />
          <Box flex="1" minW="0">
            {title}
          </Box>
          {meta}
        </Flex>
      </Collapsible.Trigger>
      <Collapsible.Content>
        <Box className="collapsible-body">{children}</Box>
      </Collapsible.Content>
    </Collapsible.Root>
  )
}
