import { Flex } from "@chakra-ui/react"
import { useState } from "react"
import { Approvals } from "./components/Approvals"
import { ChatPane } from "./components/ChatPane"
import { SettingsPane } from "./components/SettingsPane"
import { Sidebar, type Tab } from "./components/Sidebar"
import { StatusBar } from "./components/StatusBar"
import { Titlebar } from "./components/Titlebar"
import { useActivityFeed, useToolApprovals } from "./lib/bot"

function App() {
  const [tab, setTab] = useState<Tab>("chat")
  const [debug, setDebug] = useState(false)
  // Kept at the App level (always mounted) so the feed/approvals and their
  // listeners survive tab switches instead of resetting when Chat remounts.
  const activity = useActivityFeed()
  const [approvals, resolveApproval] = useToolApprovals()

  return (
    <Flex className="app-shell" direction="column" h="100vh" colorPalette="blue">
      <Titlebar />
      <Flex flex="1" minH="0" px="2" pb="2" gap="2">
        <Sidebar tab={tab} onTabChange={setTab} />
        <Flex direction="column" flex="1" minW="0">
          <Approvals approvals={approvals} onResolve={resolveApproval} />
          <Flex as="main" className="content" direction="column" flex="1" minH="0">
            {tab === "chat" ? (
              <ChatPane events={activity} debug={debug} onDebugChange={setDebug} />
            ) : (
              <SettingsPane />
            )}
          </Flex>
          <StatusBar />
        </Flex>
      </Flex>
    </Flex>
  )
}

export default App
