import { Flex, IconButton, Menu, Portal } from "@chakra-ui/react"
import { useTheme } from "next-themes"
import { LuMonitor, LuMoon, LuSun } from "react-icons/lu"

const OPTIONS = [
  { value: "system", label: "Auto", Icon: LuMonitor },
  { value: "light", label: "Light", Icon: LuSun },
  { value: "dark", label: "Dark", Icon: LuMoon },
] as const

/** Auto / Light / Dark picker. Persists via next-themes (localStorage). */
export function ThemeToggle() {
  const { theme, setTheme } = useTheme()
  const current = OPTIONS.find((o) => o.value === theme) ?? OPTIONS[0]
  const CurrentIcon = current.Icon

  return (
    <Menu.Root
      positioning={{ placement: "bottom-end" }}
      onSelect={(d) => setTheme(d.value)}
    >
      <Menu.Trigger asChild>
        <IconButton
          aria-label={`Theme: ${current.label}`}
          size="2xs"
          variant="ghost"
          colorPalette="gray"
          borderRadius="full"
        >
          <CurrentIcon size={14} />
        </IconButton>
      </Menu.Trigger>
      <Portal>
        <Menu.Positioner>
          <Menu.Content minW="130px">
            {OPTIONS.map(({ value, label, Icon }) => (
              <Menu.Item key={value} value={value}>
                <Flex align="center" gap="2">
                  <Icon size={15} /> {label}
                </Flex>
              </Menu.Item>
            ))}
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  )
}
