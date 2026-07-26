import { Flex } from "@chakra-ui/react"
import type { ReactNode } from "react"

/** Shared bottom-bar shell — the status bar (chat) and the action bar (settings
 * tabs) both live in this slot, so they read as the same strip. */
export function FooterBar({
  children,
  className,
}: {
  children: ReactNode
  className?: string
}) {
  return (
    <Flex
      as="footer"
      className={`panel footer-bar${className ? ` ${className}` : ""}`}
      align="center"
      justify="space-between"
      gap="3"
    >
      {children}
    </Flex>
  )
}
