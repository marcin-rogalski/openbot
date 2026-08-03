import { Button, Flex } from "@chakra-ui/react"

export type TabDef = { id: string; label: string }

/** Gentle tab bar shown in a settings pane header. `dense` shrinks it to fit
 * inline on the title line. */
export function Tabs({
  tabs,
  active,
  onChange,
  dense = false,
}: {
  tabs: TabDef[]
  active: string
  onChange: (id: string) => void
  dense?: boolean
}) {
  return (
    <Flex className="tabbar" gap="1" align="center">
      {tabs.map((t) => (
        <Button
          key={t.id}
          size={dense ? "2xs" : "xs"}
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
