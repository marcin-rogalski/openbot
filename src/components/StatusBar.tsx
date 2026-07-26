import { Flex, Text } from "@chakra-ui/react"
import type { MetricsData } from "../lib/bot"
import { FooterBar } from "./FooterBar"

function fmtTps(value: number | null | undefined, running: boolean): string {
  if (!running || value == null) return "—"
  return `${Math.round(value)} tok/s`
}

export function StatusBar({
  running,
  metrics,
}: {
  running: boolean
  metrics?: MetricsData
}) {
  return (
    <FooterBar className="status-bar">
      <Text>{running ? "Running" : "Stopped"}</Text>
      <Flex align="center" gap="4">
        <Text>
          <Text as="span" color="fg.subtle">
            speed
          </Text>{" "}
          {fmtTps(metrics?.inferenceTps, running)}
        </Text>
      </Flex>
    </FooterBar>
  )
}
