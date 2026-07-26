import { Box, Stack, Switch, Text } from "@chakra-ui/react"
import type { Meta, StoryObj } from "@storybook/react-vite"
import { useState } from "react"
import { FloatingField } from "../components/FloatingField"
import { PolicySelect } from "../components/PolicySelect"
import { Section } from "../components/Section"

const meta: Meta = { title: "Design/Redesign" }
export default meta
type Story = StoryObj

export const Fields: Story = {
  render: () => {
    const [name, setName] = useState("qwen2.5-7b")
    return (
      <Stack gap="4" maxW="360px">
        <Text fontSize="sm" color="fg.muted">
          Label floats up on focus / when filled.
        </Text>
        <FloatingField
          label="Model name"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <FloatingField label="API key (empty)" />
        <FloatingField label="Password" type="password" defaultValue="secret" />
      </Stack>
    )
  },
}

export const Sections: Story = {
  render: () => {
    const [on, setOn] = useState(true)
    return (
      <Stack gap="6" maxW="420px">
        <Section
          title="Approvals"
          caption="Per-action permissions"
          action={<PolicySelect value="ask" onChange={() => {}} />}
        >
          <Text fontSize="sm" color="fg.muted">
            A section groups related controls under one header.
          </Text>
        </Section>
        <Section
          title="Attachments"
          action={
            <Switch.Root
              size="sm"
              checked={on}
              colorPalette="brand"
              onCheckedChange={(e) => setOn(e.checked)}
            >
              <Switch.HiddenInput />
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
            </Switch.Root>
          }
        >
          <Text fontSize="sm" color="fg.muted">
            The right slot holds a toggle, status, or action.
          </Text>
        </Section>
      </Stack>
    )
  },
}

export const ModelTab: Story = {
  render: () => (
    <Box maxW="440px">
      <Stack gap="6">
        <Section title="Endpoint" caption="OpenAI-compatible server">
          <FloatingField label="Base URL" defaultValue="http://127.0.0.1:8080/v1" />
          <FloatingField label="API key" type="password" />
        </Section>
        <Section title="Models">
          <FloatingField label="Chat model" defaultValue="qwen2.5-7b" />
          <FloatingField label="Embedding model" defaultValue="nomic-embed-text" />
        </Section>
      </Stack>
    </Box>
  ),
}
