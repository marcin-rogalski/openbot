import { Button, Flex, Heading, Stack } from "@chakra-ui/react"

export type Tab = "chat" | "settings"

const TABS: { id: Tab; label: string }[] = [
  { id: "chat", label: "Chat" },
  { id: "settings", label: "Settings" },
]

type Props = {
  tab: Tab
  onTabChange: (tab: Tab) => void
}

export function Sidebar({ tab, onTabChange }: Props) {
  return (
    <Flex
      as="nav"
      className="sidebar"
      direction="column"
      gap="4"
      bg="bg.panel"
      borderRadius="xl"
      borderWidth="1px"
      borderColor="border"
    >
      <Heading size="sm" fontWeight="semibold">
        openbot
      </Heading>

      <Stack gap="0.5">
        {TABS.map((t) => (
          <Button
            key={t.id}
            size="sm"
            variant={tab === t.id ? "subtle" : "ghost"}
            justifyContent="flex-start"
            fontWeight="medium"
            color={tab === t.id ? "fg" : "fg.muted"}
            onClick={() => onTabChange(t.id)}
          >
            {t.label}
          </Button>
        ))}
      </Stack>
    </Flex>
  )
}
