import { Box } from "@chakra-ui/react"
import type { InputHTMLAttributes } from "react"

/** An input whose label lives inside it and floats up when focused or filled —
 * keeps dense forms tidy (one control per row, no separate label line). */
export function FloatingField({
  label,
  ...props
}: { label: string } & InputHTMLAttributes<HTMLInputElement>) {
  return (
    <Box className="floating-field">
      {/* placeholder=" " so :placeholder-shown drives the float state */}
      <input className="floating-input" placeholder=" " {...props} />
      <span className="floating-label">{label}</span>
    </Box>
  )
}
