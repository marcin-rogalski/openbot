import { Flex, Switch } from "@chakra-ui/react"
import { startBot, stopBot, useBotStatus } from "../lib/bot"

/**
 * Transparent, draggable title bar (macOS "Overlay" style). The macOS traffic
 * lights float over its left edge, so we pad past them and place the run toggle
 * right next to them.
 */
export function Titlebar() {
  const running = useBotStatus()

  return (
    <Flex
      className="titlebar"
      data-tauri-drag-region
      align="center"
      gap="3"
      flexShrink="0"
      h="28px"
      pl="80px"
    >
      <Switch.Root
        size="sm"
        checked={running}
        onCheckedChange={(e) => void (e.checked ? startBot() : stopBot())}
      >
        <Switch.HiddenInput />
        <Switch.Control>
          <Switch.Thumb />
        </Switch.Control>
        <Switch.Label>Run bot</Switch.Label>
      </Switch.Root>
    </Flex>
  )
}
