import { Badge, Box, Button, Flex, Stack, Switch, Text, Textarea } from "@chakra-ui/react"
import { useEffect, useState } from "react"
import {
  type ActivityEvent,
  clearMemories,
  deleteMemory,
  getMemories,
  type Memory,
  type MetricsData,
} from "../lib/bot"
import {
  BOT_COLORS,
  type BotConfig,
  defaultPolicy,
  type GlobalConfig,
  type ToolManifest,
  type ToolPolicy,
  toolLabel,
} from "../lib/config"
import { ActionBar } from "./ActionBar"
import { ActivityFeed } from "./ActivityFeed"
import { CollapsibleRow } from "./CollapsibleRow"
import { ConfirmButton } from "./ConfirmButton"
import { FloatingField } from "./FloatingField"
import { PolicySelect } from "./PolicySelect"
import { Section } from "./Section"
import { StatusBar } from "./StatusBar"
import { Tabs } from "./Tabs"
import { ToolIcon } from "./ToolIcon"

const TABS = [
  { id: "chat", label: "Chat" },
  { id: "general", label: "General" },
  { id: "model", label: "Model" },
  { id: "behavior", label: "Behavior" },
  { id: "tools", label: "Tools" },
  { id: "memory", label: "Memory" },
]

export function BotView({
  bot,
  global,
  manifests,
  events,
  running,
  busy,
  detail,
  metrics,
  verbose,
  onVerboseChange,
  onSaveBot,
  onDelete,
}: {
  bot: BotConfig
  global: GlobalConfig
  manifests: ToolManifest[]
  events: ActivityEvent[]
  running: boolean
  busy?: string | null
  detail?: string | null
  metrics?: MetricsData
  verbose: boolean
  onVerboseChange: (verbose: boolean) => void
  onSaveBot: (bot: BotConfig) => void
  onDelete: (id: string) => void
}) {
  const [tab, setTab] = useState("chat")
  const [cfg, setCfg] = useState<BotConfig>(bot)
  const [memories, setMemories] = useState<Memory[]>([])

  const dirty = JSON.stringify(cfg) !== JSON.stringify(bot)

  const refreshMemories = () => getMemories(bot.id).then(setMemories)
  useEffect(() => {
    if (tab === "memory") void getMemories(bot.id).then(setMemories)
  }, [tab, bot.id])

  const update = <K extends keyof BotConfig>(key: K, value: BotConfig[K]) => {
    setCfg((c) => ({ ...c, [key]: value }))
  }
  const setModel = (patch: Partial<BotConfig["model"]>) => {
    setCfg((c) => ({ ...c, model: { ...c.model, ...patch } }))
  }
  const toggleTool = (id: string, on: boolean) => {
    setCfg((c) => ({
      ...c,
      enabledToolIds: on
        ? [...c.enabledToolIds, id]
        : c.enabledToolIds.filter((t) => t !== id),
    }))
  }
  const setPolicy = (key: string, policy: ToolPolicy) => {
    setCfg((c) => ({ ...c, toolPolicies: { ...c.toolPolicies, [key]: policy } }))
  }
  const save = () => onSaveBot(cfg)

  return (
    <Flex direction="column" h="100%" minH="0" gap="2">
      <Flex direction="column" className="panel" flex="1" minH="0">
        <Flex className="pane-title" align="center" justify="space-between" gap="3">
          <Tabs tabs={TABS} active={tab} onChange={setTab} />
          {tab === "chat" ? (
            <Switch.Root
              size="sm"
              checked={verbose}
              onCheckedChange={(e) => onVerboseChange(e.checked)}
            >
              <Switch.HiddenInput />
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
              <Switch.Label>Verbose</Switch.Label>
            </Switch.Root>
          ) : null}
        </Flex>

        {tab === "chat" ? (
          <ActivityFeed events={events} verbose={verbose} />
        ) : (
          <Box className="feed" flex="1" overflowY="auto">
            <Stack gap="5" maxW="560px">
              {tab === "general" ? (
                <>
                  <Section title="Identity">
                    <FloatingField
                      label="Name"
                      value={cfg.name}
                      onChange={(e) => update("name", e.target.value)}
                    />
                    <Flex gap="2">
                      {BOT_COLORS.map((color) => (
                        <button
                          key={color}
                          type="button"
                          aria-label={color}
                          className={`swatch ${cfg.color === color ? "is-active" : ""}`}
                          style={{ background: color }}
                          onClick={() => update("color", color)}
                        />
                      ))}
                    </Flex>
                  </Section>
                  <Section
                    title="Discord"
                    caption="From the developer portal — enable the Message Content Intent."
                  >
                    <FloatingField
                      label="Bot token"
                      type="password"
                      value={cfg.discordToken}
                      onChange={(e) => update("discordToken", e.target.value)}
                    />
                  </Section>
                </>
              ) : null}

              {tab === "model" ? (
                <>
                  <Section title="Endpoint" caption="OpenAI-compatible server">
                    <FloatingField
                      label="Base URL"
                      value={cfg.model.baseUrl}
                      onChange={(e) => setModel({ baseUrl: e.target.value })}
                    />
                    <FloatingField
                      label="API key (blank for local)"
                      type="password"
                      value={cfg.model.apiKey}
                      onChange={(e) => setModel({ apiKey: e.target.value })}
                    />
                  </Section>
                  <Section title="Models">
                    <FloatingField
                      label="Chat model"
                      value={cfg.model.modelName}
                      onChange={(e) => setModel({ modelName: e.target.value })}
                    />
                    <FloatingField
                      label="Embedding model"
                      value={cfg.model.embeddingModel}
                      onChange={(e) => setModel({ embeddingModel: e.target.value })}
                    />
                    <FloatingField
                      label="Transcription model"
                      value={cfg.model.transcriptionModel}
                      onChange={(e) => setModel({ transcriptionModel: e.target.value })}
                    />
                    <Text fontSize="xs" color="fg.muted">
                      Embeddings (/embeddings) power the Drive knowledge index;
                      transcription (/audio/transcriptions) turns audio into text. Both
                      use the same base URL.
                    </Text>
                  </Section>
                </>
              ) : null}

              {tab === "behavior" ? (
                <>
                  <Section title="System prompt">
                    <Textarea
                      rows={5}
                      value={cfg.systemPrompt}
                      onChange={(e) => update("systemPrompt", e.target.value)}
                    />
                  </Section>
                  <Section
                    title="Follow-up window"
                    caption="How long the bot keeps replying without an @-mention."
                  >
                    <Flex gap="3">
                      <FloatingField
                        label="Messages"
                        type="number"
                        min={0}
                        value={String(cfg.followupWindowMessages)}
                        onChange={(e) =>
                          update("followupWindowMessages", Number(e.target.value) || 0)
                        }
                      />
                      <FloatingField
                        label="Seconds"
                        type="number"
                        min={0}
                        value={String(cfg.followupWindowSecs)}
                        onChange={(e) =>
                          update("followupWindowSecs", Number(e.target.value) || 0)
                        }
                      />
                    </Flex>
                  </Section>
                  <Section
                    title="Attachments"
                    action={
                      <Switch.Root
                        size="sm"
                        checked={cfg.attachmentsEnabled}
                        colorPalette="brand"
                        onCheckedChange={(e) => update("attachmentsEnabled", e.checked)}
                      >
                        <Switch.HiddenInput />
                        <Switch.Control>
                          <Switch.Thumb />
                        </Switch.Control>
                      </Switch.Root>
                    }
                  >
                    <Text fontSize="sm" color="fg.muted">
                      Text files you attach are read inline so the bot can act on them
                      this turn; files are also offered to enabled tools (e.g. Google
                      Drive archives relevant ones, guided by memory rules).
                    </Text>
                  </Section>
                  <Section
                    title="Audio transcription"
                    action={
                      <Switch.Root
                        size="sm"
                        checked={cfg.transcriptionEnabled}
                        colorPalette="brand"
                        onCheckedChange={(e) => update("transcriptionEnabled", e.checked)}
                      >
                        <Switch.HiddenInput />
                        <Switch.Control>
                          <Switch.Thumb />
                        </Switch.Control>
                      </Switch.Root>
                    }
                  >
                    <Text fontSize="sm" color="fg.muted">
                      Audio you post is transcribed (via the transcription model); the bot
                      replies with a transcript and a summary <code>.md</code>, and — when
                      a Google Drive tool is enabled — saves and indexes them into the
                      knowledge base.
                    </Text>
                  </Section>
                </>
              ) : null}

              {tab === "tools" ? (
                <>
                  <Text fontSize="sm" color="fg.muted">
                    Tools this bot may use, and per-action approval. Manage the tool list
                    in General settings → Tools. Default: reads run automatically, writes
                    ask for approval.
                  </Text>
                  {global.tools.length === 0 ? (
                    <Text fontSize="sm" color="fg.subtle">
                      No tools configured yet.
                    </Text>
                  ) : null}
                  {global.tools.map((tool) => {
                    const enabled = cfg.enabledToolIds.includes(tool.id)
                    const ops = manifests.find((m) => m.kind === tool.type)?.ops ?? []
                    return (
                      <Flex key={tool.id} align="start" gap="3">
                        <Flex align="center" minH="40px" flexShrink="0">
                          <Switch.Root
                            size="sm"
                            checked={enabled}
                            colorPalette="brand"
                            onCheckedChange={(e) => toggleTool(tool.id, e.checked)}
                          >
                            <Switch.HiddenInput />
                            <Switch.Control>
                              <Switch.Thumb />
                            </Switch.Control>
                          </Switch.Root>
                        </Flex>
                        <Box flex="1" minW="0">
                          <CollapsibleRow
                            title={
                              <Flex align="center" gap="2" minW="0">
                                <ToolIcon type={tool.type} />
                                <Text fontSize="sm" fontWeight="medium" truncate>
                                  {tool.name}
                                </Text>
                                <Text as="span" color="fg.subtle" fontSize="xs">
                                  ({toolLabel(tool.type)})
                                </Text>
                              </Flex>
                            }
                          >
                            {enabled ? (
                              <Section title="Approvals">
                                <Stack gap="1.5">
                                  {ops.map((o) => {
                                    const key = `${tool.id}/${o.op}`
                                    const value =
                                      cfg.toolPolicies[key] ?? defaultPolicy(o.write)
                                    return (
                                      <Flex
                                        key={o.op}
                                        align="center"
                                        justify="space-between"
                                        gap="3"
                                      >
                                        <Text fontSize="sm">{o.label}</Text>
                                        <PolicySelect
                                          value={value}
                                          onChange={(p) => setPolicy(key, p)}
                                        />
                                      </Flex>
                                    )
                                  })}
                                </Stack>
                              </Section>
                            ) : (
                              <Text fontSize="sm" color="fg.muted">
                                Enable this tool (toggle on the left) to configure it.
                              </Text>
                            )}
                          </CollapsibleRow>
                        </Box>
                      </Flex>
                    )
                  })}
                </>
              ) : null}

              {tab === "memory" ? (
                <>
                  <Section
                    title="Memory"
                    caption="The bot saves memories/rules via a tool; they're injected into its prompt and consolidated when over budget."
                    action={
                      <Switch.Root
                        size="sm"
                        checked={cfg.memoryEnabled}
                        colorPalette="brand"
                        onCheckedChange={(e) => update("memoryEnabled", e.checked)}
                      >
                        <Switch.HiddenInput />
                        <Switch.Control>
                          <Switch.Thumb />
                        </Switch.Control>
                      </Switch.Root>
                    }
                  >
                    <Flex gap="3">
                      <FloatingField
                        label="Max notes"
                        type="number"
                        min={1}
                        value={String(cfg.memoryMaxNotes)}
                        onChange={(e) =>
                          update("memoryMaxNotes", Number(e.target.value) || 1)
                        }
                      />
                      <FloatingField
                        label="Char budget"
                        type="number"
                        min={100}
                        value={String(cfg.memoryCharBudget)}
                        onChange={(e) =>
                          update("memoryCharBudget", Number(e.target.value) || 100)
                        }
                      />
                    </Flex>
                  </Section>
                  <Section
                    title={`Saved memories (${memories.length})`}
                    action={
                      memories.length > 0 ? (
                        <Button
                          size="xs"
                          variant="ghost"
                          colorPalette="red"
                          onClick={() => clearMemories(bot.id).then(refreshMemories)}
                        >
                          Clear all
                        </Button>
                      ) : undefined
                    }
                  >
                    {memories.length === 0 ? (
                      <Text fontSize="sm" color="fg.subtle">
                        Nothing remembered yet.
                      </Text>
                    ) : (
                      memories.map((mem) => (
                        <Flex key={mem.id} className="list-row" align="center" gap="2">
                          <Badge
                            size="sm"
                            colorPalette={mem.kind === "rule" ? "purple" : "gray"}
                            flexShrink="0"
                          >
                            {mem.kind}
                          </Badge>
                          <Text flex="1" minW="0" fontSize="sm">
                            {mem.text}
                          </Text>
                          <Button
                            size="2xs"
                            variant="ghost"
                            onClick={() =>
                              deleteMemory(bot.id, mem.id).then(refreshMemories)
                            }
                          >
                            🗑
                          </Button>
                        </Flex>
                      ))
                    )}
                  </Section>
                </>
              ) : null}
            </Stack>
          </Box>
        )}
      </Flex>

      {tab === "chat" ? (
        <StatusBar running={running} busy={busy} detail={detail} metrics={metrics} />
      ) : (
        <ActionBar
          dirty={dirty}
          hint="Saving restarts the bot if running."
          onSave={save}
          onDiscard={() => setCfg(bot)}
          left={
            <ConfirmButton
              label="Delete bot"
              message={`Delete "${cfg.name || "this bot"}"? This can't be undone.`}
              onConfirm={() => onDelete(cfg.id)}
            />
          }
        />
      )}
    </Flex>
  )
}
