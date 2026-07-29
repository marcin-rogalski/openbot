import { Box, Button, Flex, Heading, Menu, Stack, Text } from "@chakra-ui/react"
import { useEffect, useState } from "react"
import { connectDrive, driveStatus } from "../lib/bot"
import {
  type GlobalConfig,
  manifestFor,
  newToolInstance,
  saveGlobal,
  type ToolInstance,
  type ToolManifest,
  toolLabel,
} from "../lib/config"
import { ActionBar } from "./ActionBar"
import { CollapsibleRow } from "./CollapsibleRow"
import { DangerZone } from "./DangerZone"
import { FloatingField } from "./FloatingField"
import { Section } from "./Section"
import { Tabs } from "./Tabs"
import { ToolIcon } from "./ToolIcon"

const TABS = [
  { id: "tools", label: "Tools" },
  { id: "mcp", label: "MCP" },
]

export function GlobalSettings({
  global,
  manifests,
  onSave,
}: {
  global: GlobalConfig
  manifests: ToolManifest[]
  onSave: (g: GlobalConfig) => void
}) {
  const [tab, setTab] = useState("tools")
  const [cfg, setCfg] = useState<GlobalConfig>(global)
  const [openId, setOpenId] = useState<string | null>(null)
  const [status, setStatus] = useState("")

  const dirty = JSON.stringify(cfg) !== JSON.stringify(global)

  useEffect(() => {
    if (!openId) return
    setStatus("checking…")
    driveStatus(openId).then((c) =>
      setStatus(c ? "Connected (token cached)" : "Not connected"),
    )
  }, [openId])

  const save = () => onSave(cfg)

  const updateTool = (id: string, patch: Partial<GlobalConfig["tools"][number]>) => {
    setCfg((c) => ({
      ...c,
      tools: c.tools.map((t) => (t.id === id ? { ...t, ...patch } : t)),
    }))
  }
  const addTool = (type: string) => {
    const tool = newToolInstance(type)
    setCfg((c) => ({ ...c, tools: [...c.tools, tool] }))
    setOpenId(tool.id)
  }
  const removeTool = (id: string) => {
    setCfg((c) => ({ ...c, tools: c.tools.filter((t) => t.id !== id) }))
    if (openId === id) setOpenId(null)
  }

  const onConnect = (toolId: string) => {
    setStatus("connecting… (check your browser)")
    void saveGlobal(cfg)
      .then(() => connectDrive(toolId))
      .then((email) => setStatus(`Connected as ${email}`))
      .catch((e) => setStatus(`Error: ${e}`))
  }

  return (
    <Flex direction="column" h="100%" minH="0" gap="2">
      <Flex direction="column" className="panel" flex="1" minH="0">
        <Flex className="pane-title" align="center" justify="space-between">
          <Heading size="sm">General settings</Heading>
          <Tabs tabs={TABS} active={tab} onChange={setTab} />
        </Flex>

        <Box className="feed" flex="1" overflowY="auto">
          <Stack gap="5" maxW="560px">
            {tab === "tools" ? (
              <>
                <Flex align="center" justify="space-between">
                  <Text fontSize="sm" color="fg.muted">
                    Tool instances. Bots choose which to use.
                  </Text>
                  <Menu.Root onSelect={(d) => addTool(d.value)}>
                    <Menu.Trigger asChild>
                      <Button size="xs" variant="subtle">
                        + Add tool
                      </Button>
                    </Menu.Trigger>
                    <Menu.Positioner>
                      <Menu.Content>
                        {manifests.map((m) => (
                          <Menu.Item key={m.kind} value={m.kind}>
                            {m.icon} {m.label}
                          </Menu.Item>
                        ))}
                      </Menu.Content>
                    </Menu.Positioner>
                  </Menu.Root>
                </Flex>
                {cfg.tools.length === 0 ? (
                  <Text fontSize="sm" color="fg.subtle">
                    No tools yet — use “+ Add tool”.
                  </Text>
                ) : null}
                {cfg.tools.map((tool) => (
                  <CollapsibleRow
                    key={tool.id}
                    open={openId === tool.id}
                    onOpenChange={(o) => setOpenId(o ? tool.id : null)}
                    title={
                      <Flex align="center" gap="2" minW="0">
                        <ToolIcon type={tool.type} size={18} />
                        <Text fontSize="sm" fontWeight="medium" truncate>
                          {tool.name || "Untitled"}
                        </Text>
                        <Text as="span" color="fg.subtle" fontSize="xs">
                          ({toolLabel(tool.type)})
                        </Text>
                      </Flex>
                    }
                  >
                    <Stack gap="6">
                      <FloatingField
                        label="Name"
                        value={tool.name}
                        onChange={(e) => updateTool(tool.id, { name: e.target.value })}
                      />
                      {(() => {
                        const m = manifestFor(tool.type)
                        if (!m || (m.configFields.length === 0 && !m.oauth)) return null
                        return (
                          <Section title={m.label} caption={m.configCaption ?? undefined}>
                            {m.configFields.map((field) =>
                              field.number ? (
                                <FloatingField
                                  key={field.key}
                                  label={field.label}
                                  type="number"
                                  min={1}
                                  value={String(
                                    (tool[field.key as keyof ToolInstance] as number) ??
                                      0,
                                  )}
                                  onChange={(e) =>
                                    updateTool(tool.id, {
                                      [field.key]: Number(e.target.value) || 0,
                                    } as Partial<ToolInstance>)
                                  }
                                />
                              ) : (
                                <FloatingField
                                  key={field.key}
                                  label={field.label}
                                  type={field.secret ? "password" : undefined}
                                  value={
                                    (tool[field.key as keyof ToolInstance] as string) ??
                                    ""
                                  }
                                  onChange={(e) =>
                                    updateTool(tool.id, {
                                      [field.key]: e.target.value,
                                    } as Partial<ToolInstance>)
                                  }
                                />
                              ),
                            )}
                            {m.oauth ? (
                              <Flex align="center" gap="3">
                                <Button
                                  variant="subtle"
                                  colorPalette="brand"
                                  onClick={() => onConnect(tool.id)}
                                >
                                  Connect
                                </Button>
                                <Text fontSize="sm" color="fg.muted">
                                  {status}
                                </Text>
                              </Flex>
                            ) : null}
                          </Section>
                        )
                      })()}
                      <DangerZone>
                        <Button
                          size="sm"
                          variant="outline"
                          colorPalette="red"
                          onClick={() => removeTool(tool.id)}
                        >
                          Delete tool
                        </Button>
                      </DangerZone>
                    </Stack>
                  </CollapsibleRow>
                ))}
              </>
            ) : null}

            {tab === "mcp" ? (
              <Text fontSize="sm" color="fg.subtle">
                MCP servers will use the same instances + “+ Add” pattern as Tools, in a
                later milestone.
              </Text>
            ) : null}
          </Stack>
        </Box>
      </Flex>

      <ActionBar
        dirty={dirty}
        hint="Saving restarts running bots."
        onSave={save}
        onDiscard={() => setCfg(global)}
      />
    </Flex>
  )
}
