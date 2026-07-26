import { createSystem, defaultConfig, defineConfig } from "@chakra-ui/react"

// openbot design theme. Discord-inspired: a blurple brand accent over layered
// neutral surfaces, tuned for a comfortable dark-first look with a clean light
// mode. Colours are exposed as CSS vars (e.g. --chakra-colors-bg-panel), so
// both Chakra components and the app's SCSS pick them up.

const config = defineConfig({
  theme: {
    tokens: {
      colors: {
        // Brand accent (blurple) scale.
        brand: {
          50: { value: "#eef0fe" },
          100: { value: "#dfe1fc" },
          200: { value: "#c3c7f9" },
          300: { value: "#a1a7f5" },
          400: { value: "#7c84f0" },
          500: { value: "#5865f2" },
          600: { value: "#4a55d4" },
          700: { value: "#3c45b0" },
          800: { value: "#2f368c" },
          900: { value: "#262b6e" },
          950: { value: "#171a42" },
        },
        // Neutral surface scale (Discord-ish greys), light → deep.
        neutral: {
          50: { value: "#f7f7f8" },
          100: { value: "#ebedef" },
          200: { value: "#dbdee1" },
          300: { value: "#c4c9cf" },
          400: { value: "#949ba4" },
          500: { value: "#6d7178" },
          600: { value: "#4e5058" },
          700: { value: "#3a3c43" },
          800: { value: "#2b2d31" },
          900: { value: "#232428" },
          950: { value: "#1a1b1e" },
        },
      },
    },
    semanticTokens: {
      colors: {
        // Make `colorPalette="brand"` fully functional.
        brand: {
          solid: { value: "{colors.brand.500}" },
          contrast: { value: "#ffffff" },
          fg: { value: { base: "{colors.brand.700}", _dark: "{colors.brand.300}" } },
          muted: { value: { base: "{colors.brand.100}", _dark: "{colors.brand.900}" } },
          subtle: { value: { base: "{colors.brand.50}", _dark: "{colors.brand.950}" } },
          emphasized: {
            value: { base: "{colors.brand.200}", _dark: "{colors.brand.800}" },
          },
          focusRing: { value: "{colors.brand.500}" },
        },
        // Layered surfaces: canvas → subtle → panel → raised.
        bg: {
          DEFAULT: { value: { base: "#ffffff", _dark: "{colors.neutral.900}" } },
          subtle: {
            value: { base: "{colors.neutral.50}", _dark: "{colors.neutral.950}" },
          },
          muted: {
            value: { base: "{colors.neutral.100}", _dark: "{colors.neutral.800}" },
          },
          panel: { value: { base: "#ffffff", _dark: "{colors.neutral.800}" } },
          emphasized: {
            value: { base: "{colors.neutral.100}", _dark: "{colors.neutral.700}" },
          },
        },
        // Text — two levels only: primary (fg) and muted. `subtle` is aliased to
        // muted so existing usages collapse to the same secondary tone.
        fg: {
          DEFAULT: {
            value: { base: "{colors.neutral.900}", _dark: "{colors.neutral.100}" },
          },
          muted: {
            value: { base: "{colors.neutral.600}", _dark: "{colors.neutral.400}" },
          },
          subtle: {
            value: { base: "{colors.neutral.600}", _dark: "{colors.neutral.400}" },
          },
        },
        // Borders.
        border: {
          DEFAULT: {
            value: { base: "{colors.neutral.200}", _dark: "{colors.neutral.700}" },
          },
          muted: {
            value: { base: "{colors.neutral.100}", _dark: "{colors.neutral.800}" },
          },
          subtle: {
            value: { base: "{colors.neutral.100}", _dark: "{colors.neutral.700}" },
          },
          emphasized: {
            value: { base: "{colors.neutral.300}", _dark: "{colors.neutral.600}" },
          },
        },
      },
    },
    recipes: {
      badge: {
        base: {
          paddingInline: "2",
          paddingBlock: "0.5",
          borderWidth: "1px",
          borderColor: "colorPalette.solid",
        },
        variants: {
          // Vivid: a filled accent chip with one readable light text colour.
          variant: {
            subtle: { bg: "colorPalette.solid", color: "colorPalette.contrast" },
          },
        },
      },
      button: {
        base: {
          // Grayed out (not just dimmed) — a flat neutral fill, no accent colour.
          _disabled: {
            opacity: 1,
            bg: "bg.emphasized",
            color: "fg.muted",
            borderColor: "border",
            cursor: "not-allowed",
          },
        },
        variants: {
          variant: {
            // Filled buttons all use the same light text.
            solid: { color: "colorPalette.contrast" },
            // Outline shows an accent border; on hover/press it fills like solid.
            outline: {
              borderWidth: "1px",
              borderColor: "colorPalette.solid",
              color: "colorPalette.fg",
              _hover: {
                bg: "colorPalette.solid",
                color: "colorPalette.contrast",
                borderColor: "colorPalette.solid",
              },
              _active: { bg: "colorPalette.solid", color: "colorPalette.contrast" },
            },
            // Ghost fills with the accent on hover/press, like solid.
            ghost: {
              _hover: { bg: "colorPalette.solid", color: "colorPalette.contrast" },
              _active: { bg: "colorPalette.solid", color: "colorPalette.contrast" },
            },
          },
        },
      },
    },
  },
  globalCss: {
    "html, body": {
      background: "bg",
      color: "fg",
    },
  },
})

export const system = createSystem(defaultConfig, config)
