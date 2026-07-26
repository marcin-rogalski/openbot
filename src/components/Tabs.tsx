import { Button, Flex } from "@chakra-ui/react"

export type TabDef = { id: string; label: string }

/** Gentle tab bar shown at the top of a settings pane. */
export function Tabs({
  tabs,
  active,
  onChange,
}: {
  tabs: TabDef[]
  active: string
  onChange: (id: string) => void
}) {
  return (
    <Flex className="tabbar" gap="1" align="center">
      {tabs.map((t) => (
        <Button
          key={t.id}
          size="xs"
          variant={active === t.id ? "subtle" : "ghost"}
          colorPalette="gray"
          fontWeight="medium"
          className={active === t.id ? "tab-active" : undefined}
          onClick={() => onChange(t.id)}
        >
          {t.label}
        </Button>
      ))}
    </Flex>
  )
}
