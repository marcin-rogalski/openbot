import { Flex } from "@chakra-ui/react"
import { useState } from "react"
import { ChatPane } from "./components/ChatPane"
import { SettingsPane } from "./components/SettingsPane"
import { Sidebar, type Tab } from "./components/Sidebar"
import { StatusBar } from "./components/StatusBar"
import { Titlebar } from "./components/Titlebar"

function App() {
  const [tab, setTab] = useState<Tab>("chat")

  return (
    <Flex className="app-shell" direction="column" h="100vh" colorPalette="blue">
      <Titlebar />
      <Flex flex="1" minH="0" px="2" pb="2" gap="2">
        <Sidebar tab={tab} onTabChange={setTab} />
        <Flex direction="column" flex="1" minW="0">
          <Flex as="main" className="content" direction="column" flex="1" minH="0">
            {tab === "chat" ? <ChatPane /> : <SettingsPane />}
          </Flex>
          <StatusBar />
        </Flex>
      </Flex>
    </Flex>
  )
}

export default App
