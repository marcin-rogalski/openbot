import { Button, Menu, Portal } from "@chakra-ui/react"
import { LuChevronDown } from "react-icons/lu"
import type { ToolPolicy } from "../lib/config"

const OPTIONS: { value: ToolPolicy; label: string; palette: string }[] = [
  { value: "allow", label: "Allow", palette: "green" },
  { value: "ask", label: "Ask", palette: "orange" },
  { value: "deny", label: "Deny", palette: "red" },
]

/** A small allow/ask/deny picker: a plain gray outline button that opens a
 * color-aligned dropdown. */
export function PolicySelect({
  value,
  onChange,
}: {
  value: ToolPolicy
  onChange: (policy: ToolPolicy) => void
}) {
  const current = OPTIONS.find((o) => o.value === value) ?? OPTIONS[0]
  return (
    <Menu.Root
      positioning={{ placement: "bottom-end" }}
      onSelect={(d) => onChange(d.value as ToolPolicy)}
    >
      <Menu.Trigger asChild>
        <Button
          size="xs"
          variant="outline"
          colorPalette="gray"
          borderColor="border"
          minW="76px"
          justifyContent="space-between"
        >
          {current.label}
          <LuChevronDown size={13} />
        </Button>
      </Menu.Trigger>
      <Portal>
        <Menu.Positioner>
          <Menu.Content minW="120px">
            {OPTIONS.map((o) => (
              <Menu.Item key={o.value} value={o.value} colorPalette={o.palette}>
                {o.label}
              </Menu.Item>
            ))}
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  )
}
