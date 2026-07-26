import { Flex } from "@chakra-ui/react"
import { useEffect, useState } from "react"
import { Approvals } from "./components/Approvals"
import { BotView } from "./components/BotView"
import { GlobalSettings } from "./components/GlobalSettings"
import { Sidebar } from "./components/Sidebar"
import { Titlebar } from "./components/Titlebar"
import {
  restartBot,
  startBot,
  stopBot,
  useActivityFeeds,
  useMetricsByBot,
  useRunningBots,
  useToolApprovals,
} from "./lib/bot"
import {
  type BotConfig,
  type GlobalConfig,
  loadBots,
  loadGlobal,
  newBot,
  saveBots,
  saveGlobal,
} from "./lib/config"

type View = "bot" | "global"

function App() {
  const [bots, setBots] = useState<BotConfig[]>([])
  const [globalCfg, setGlobalCfg] = useState<GlobalConfig | null>(null)
  const [selectedBotId, setSelectedBotId] = useState<string | null>(null)
  const [view, setView] = useState<View>("bot")
  const [verbose, setVerbose] = useState(false)

  const runningIds = useRunningBots()
  const feeds = useActivityFeeds()
  const metrics = useMetricsByBot()
  const [approvals, resolveApproval] = useToolApprovals()

  useEffect(() => {
    loadGlobal().then(setGlobalCfg)
    loadBots().then((b) => {
      setBots(b)
      setSelectedBotId((cur) => cur ?? b[0]?.id ?? null)
    })
  }, [])

  const selectedBot = bots.find((b) => b.id === selectedBotId) ?? null

  const onToggleBot = (id: string, run: boolean) => {
    void (run ? startBot(id) : stopBot(id))
  }
  const onSelectBot = (id: string) => {
    setSelectedBotId(id)
    setView("bot")
  }
  const onAddBot = async () => {
    const bot = newBot(bots.length)
    const next = [...bots, bot]
    setBots(next)
    await saveBots(next)
    setSelectedBotId(bot.id)
    setView("bot")
  }
  const onSaveBot = async (bot: BotConfig) => {
    const next = bots.map((b) => (b.id === bot.id ? bot : b))
    setBots(next)
    await saveBots(next)
    if (runningIds.has(bot.id)) await restartBot(bot.id)
  }
  const onDeleteBot = async (id: string) => {
    await stopBot(id)
    const next = bots.filter((b) => b.id !== id)
    setBots(next)
    await saveBots(next)
    setSelectedBotId(next[0]?.id ?? null)
    setView("bot")
  }
  const onSaveGlobal = async (g: GlobalConfig) => {
    setGlobalCfg(g)
    await saveGlobal(g)
    // Running bots re-read tools on restart.
    for (const id of runningIds) await restartBot(id)
  }

  return (
    <Flex className="app-shell" direction="column" h="100vh" colorPalette="brand">
      <Titlebar
        globalActive={view === "global"}
        onOpenGlobalSettings={() => setView("global")}
      />
      <Flex flex="1" minH="0" px="2" pb="2" gap="2">
        <Sidebar
          bots={bots}
          runningIds={runningIds}
          selectedBotId={selectedBotId}
          active={view === "bot"}
          onSelectBot={onSelectBot}
          onToggleBot={onToggleBot}
          onAddBot={onAddBot}
        />
        <Flex direction="column" flex="1" minW="0" className="rail-plain">
          <Approvals approvals={approvals} bots={bots} onResolve={resolveApproval} />
          <Flex as="main" className="content" direction="column" flex="1" minH="0">
            {view === "global" && globalCfg ? (
              <GlobalSettings global={globalCfg} onSave={onSaveGlobal} />
            ) : selectedBot && globalCfg ? (
              <BotView
                key={selectedBot.id}
                bot={selectedBot}
                global={globalCfg}
                events={feeds.get(selectedBot.id) ?? []}
                running={runningIds.has(selectedBot.id)}
                metrics={metrics.get(selectedBot.id)}
                verbose={verbose}
                onVerboseChange={setVerbose}
                onSaveBot={onSaveBot}
                onDelete={onDeleteBot}
              />
            ) : null}
          </Flex>
        </Flex>
      </Flex>
    </Flex>
  )
}

export default App
