import { Box, ChakraProvider } from "@chakra-ui/react"
import type { Preview } from "@storybook/react-vite"
import { useEffect } from "react"
import "../src/styles/app.scss"
import { system } from "../src/theme"

const preview: Preview = {
  globalTypes: {
    theme: {
      description: "Color mode",
      defaultValue: "dark",
      toolbar: {
        title: "Theme",
        icon: "circlehollow",
        items: [
          { value: "light", title: "Light" },
          { value: "dark", title: "Dark" },
        ],
        dynamicTitle: true,
      },
    },
  },
  decorators: [
    (Story, context) => {
      const theme = context.globals.theme ?? "dark"
      // biome-ignore lint/correctness/useHookAtTopLevel: decorator body is a component
      useEffect(() => {
        const root = document.documentElement
        root.classList.remove("light", "dark")
        root.classList.add(theme)
        root.style.colorScheme = theme
      }, [theme])
      return (
        <ChakraProvider value={system}>
          <Box colorPalette="brand" bg="bg" color="fg" minH="100vh" p="6">
            <Story />
          </Box>
        </ChakraProvider>
      )
    },
  ],
  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
  },
}

export default preview
