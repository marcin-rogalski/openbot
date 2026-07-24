import {
  Box,
  Button,
  Flex,
  Heading,
  Input,
  Stack,
  Text,
  Textarea,
} from "@chakra-ui/react"
import { useEffect, useState } from "react"
import { restartBot } from "../lib/bot"
import { type BotConfig, DEFAULT_CONFIG, loadConfig, saveConfig } from "../lib/config"

function FormRow({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <Box>
      <Text fontSize="sm" fontWeight="medium" mb="1.5">
        {label}
      </Text>
      {children}
      {hint ? (
        <Text fontSize="xs" color="fg.subtle" mt="1.5">
          {hint}
        </Text>
      ) : null}
    </Box>
  )
}

export function SettingsPane() {
  const [config, setConfig] = useState<BotConfig>(DEFAULT_CONFIG)
  const [loaded, setLoaded] = useState(false)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    loadConfig().then((c) => {
      setConfig(c)
      setLoaded(true)
    })
  }, [])

  const update = <K extends keyof BotConfig>(key: K, value: BotConfig[K]) => {
    setConfig((prev) => ({ ...prev, [key]: value }))
    setSaved(false)
  }

  const onSave = () => {
    // Persist, then restart the bot so changes take effect (no-op if stopped).
    void saveConfig(config)
      .then(() => restartBot())
      .then(() => setSaved(true))
  }

  return (
    <Flex direction="column" h="100%" minH="0">
      <Heading className="pane-title" size="sm">
        Settings
      </Heading>

      <Box className="feed" flex="1" overflowY="auto">
        {loaded ? (
          <Stack gap="5" maxW="560px">
            <FormRow
              label="Discord bot token"
              hint="From the Discord developer portal. Enable the Message Content Intent."
            >
              <Input
                type="password"
                placeholder="MTA…"
                value={config.discordToken}
                onChange={(e) => update("discordToken", e.target.value)}
              />
            </FormRow>

            <FormRow
              label="Model base URL"
              hint="OpenAI-compatible server, e.g. http://127.0.0.1:8080/v1"
            >
              <Input
                value={config.modelBaseUrl}
                onChange={(e) => update("modelBaseUrl", e.target.value)}
              />
            </FormRow>

            <FormRow
              label="Model name"
              hint="The model id your server serves, e.g. qwen2.5-7b."
            >
              <Input
                value={config.modelName}
                onChange={(e) => update("modelName", e.target.value)}
              />
            </FormRow>

            <FormRow
              label="API key"
              hint="Leave empty for local servers that don't require one."
            >
              <Input
                type="password"
                value={config.apiKey}
                onChange={(e) => update("apiKey", e.target.value)}
              />
            </FormRow>

            <FormRow label="System prompt">
              <Textarea
                rows={4}
                value={config.systemPrompt}
                onChange={(e) => update("systemPrompt", e.target.value)}
              />
            </FormRow>

            <Flex gap="4">
              <FormRow label="Follow-up window (messages)">
                <Input
                  type="number"
                  min={0}
                  value={String(config.followupWindowMessages)}
                  onChange={(e) =>
                    update("followupWindowMessages", Number(e.target.value) || 0)
                  }
                />
              </FormRow>
              <FormRow label="Follow-up window (seconds)">
                <Input
                  type="number"
                  min={0}
                  value={String(config.followupWindowSecs)}
                  onChange={(e) =>
                    update("followupWindowSecs", Number(e.target.value) || 0)
                  }
                />
              </FormRow>
            </Flex>

            <Flex align="center" gap="3" pt="1">
              <Button colorPalette="blue" onClick={onSave}>
                Save
              </Button>
              <Text fontSize="sm" color={saved ? "green.500" : "fg.subtle"}>
                {saved
                  ? "Saved — the bot restarts automatically if running."
                  : "Saving restarts the bot if it's running."}
              </Text>
            </Flex>
          </Stack>
        ) : null}
      </Box>
    </Flex>
  )
}
