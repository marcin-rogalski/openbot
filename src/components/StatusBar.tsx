import { Box, Flex, Text } from "@chakra-ui/react"
import { useBotStatus, useMetrics } from "../lib/bot"

function fmtTps(value: number | null, running: boolean): string {
  if (!running || value === null) return "—"
  return `${Math.round(value)} tok/s`
}

export function StatusBar() {
  const running = useBotStatus()
  const metrics = useMetrics()

  return (
    <Flex as="footer" className="status-bar" align="center" justify="space-between">
      <Flex align="center" gap="2">
        <Box className={`status-dot ${running ? "is-on" : "is-off"}`} />
        <Text>{running ? "Running" : "Stopped"}</Text>
      </Flex>

      <Flex align="center" gap="4">
        <Text>
          <Text as="span" color="fg.subtle">
            prefill
          </Text>{" "}
          {fmtTps(metrics.prefillTps, running)}
        </Text>
        <Text>
          <Text as="span" color="fg.subtle">
            inference
          </Text>{" "}
          {fmtTps(metrics.inferenceTps, running)}
        </Text>
      </Flex>
    </Flex>
  )
}
