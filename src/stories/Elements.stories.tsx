import {
  Badge,
  Button,
  Flex,
  Heading,
  Input,
  Stack,
  Switch,
  Text,
} from "@chakra-ui/react"
import type { Meta, StoryObj } from "@storybook/react-vite"
import type { ReactNode } from "react"
import { ConfirmButton } from "../components/ConfirmButton"
import { Field } from "../components/Field"

const meta: Meta = { title: "Design/Elements" }
export default meta
type Story = StoryObj

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Flex align="center" gap="4" wrap="wrap">
      <Text w="120px" fontSize="sm" color="fg.muted" flexShrink="0">
        {label}
      </Text>
      <Flex align="center" gap="3" wrap="wrap">
        {children}
      </Flex>
    </Flex>
  )
}

export const Buttons: Story = {
  render: () => (
    <Stack gap="5" maxW="640px">
      <Heading size="md">Buttons</Heading>
      <Row label="Variants">
        <Button colorPalette="brand">Solid</Button>
        <Button colorPalette="brand" variant="outline">
          Outline
        </Button>
        <Button colorPalette="brand" variant="ghost">
          Ghost
        </Button>
        <Button colorPalette="brand" disabled>
          Disabled
        </Button>
      </Row>
      <Row label="Destructive">
        <Button colorPalette="red">Delete</Button>
        <Button colorPalette="red" variant="outline">
          Delete
        </Button>
        <ConfirmButton
          label="Delete bot"
          message="Delete this bot? This can't be undone."
          size="md"
          onConfirm={() => {}}
        />
      </Row>
      <Row label="Sizes">
        <Button colorPalette="brand" size="xs">
          xs
        </Button>
        <Button colorPalette="brand" size="sm">
          sm
        </Button>
        <Button colorPalette="brand" size="md">
          md
        </Button>
      </Row>
      <Row label="sm — align">
        <Input size="sm" placeholder="Input" w="160px" />
        <Button size="sm" colorPalette="brand">
          Button
        </Button>
      </Row>
      <Row label="md — align">
        <Input size="md" placeholder="Input" w="160px" />
        <Button size="md" colorPalette="brand">
          Button
        </Button>
      </Row>
    </Stack>
  ),
}

export const Badges: Story = {
  render: () => (
    <Stack gap="4" maxW="640px">
      <Heading size="md">Badges</Heading>
      <Flex gap="3" wrap="wrap">
        {["brand", "gray", "green", "orange", "purple", "red"].map((palette) => (
          <Badge key={palette} colorPalette={palette} variant="subtle">
            {palette}
          </Badge>
        ))}
      </Flex>
    </Stack>
  ),
}

export const Inputs: Story = {
  render: () => (
    <Stack gap="5" maxW="440px">
      <Heading size="md">Inputs</Heading>
      <Field label="Name" hint="A short, human-friendly name.">
        <Input placeholder="New bot" />
      </Field>
      <Field label="API key">
        <Input type="password" defaultValue="secret-value" />
      </Field>
      <Switch.Root colorPalette="brand" defaultChecked>
        <Switch.HiddenInput />
        <Switch.Control>
          <Switch.Thumb />
        </Switch.Control>
        <Switch.Label>Enable memory</Switch.Label>
      </Switch.Root>
    </Stack>
  ),
}
