import { Flex, IconButton } from "@chakra-ui/react"
import { LuSettings } from "react-icons/lu"

/**
 * Transparent, draggable title bar (macOS "Overlay" style). The traffic lights
 * float over its left edge; the General settings button sits at the right.
 */
export function Titlebar({
  onOpenGlobalSettings,
  globalActive,
}: {
  onOpenGlobalSettings: () => void
  globalActive: boolean
}) {
  return (
    <Flex
      className="titlebar"
      data-tauri-drag-region
      flexShrink="0"
      h="36px"
      align="center"
      justify="flex-end"
      pr="1.5"
    >
      <IconButton
        aria-label="General settings"
        size="2xs"
        variant={globalActive ? "subtle" : "ghost"}
        colorPalette="gray"
        borderRadius="full"
        onClick={onOpenGlobalSettings}
      >
        <LuSettings size={14} />
      </IconButton>
    </Flex>
  )
}
