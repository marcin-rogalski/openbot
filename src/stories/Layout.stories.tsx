import type { Meta, StoryObj } from "@storybook/react-vite"
import { LayoutLive } from "./LayoutLive"
import { LayoutSchema } from "./LayoutSchema"

const meta: Meta = { title: "Design/Layout" }
export default meta
type Story = StoryObj

export const Schema: Story = {
  render: () => <LayoutSchema />,
}

export const Live: Story = {
  render: () => <LayoutLive />,
}
