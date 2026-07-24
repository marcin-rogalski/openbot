import { Box, Button, Flex, Text } from "@chakra-ui/react"
import type { ApprovalDecision, ToolApproval } from "../lib/bot"

function ApprovalCard({
  approval,
  onResolve,
}: {
  approval: ToolApproval
  onResolve: (id: string, decision: ApprovalDecision) => void
}) {
  const decide = (decision: ApprovalDecision) => onResolve(approval.id, decision)
  return (
    <Box className="approval-card">
      <Text fontSize="sm" mb="1">
        Run tool{" "}
        <Text as="span" fontWeight="semibold">
          {approval.tool}
        </Text>
        ?
      </Text>
      <Box className="approval-args">{JSON.stringify(approval.args)}</Box>
      <Flex gap="2" mt="3" wrap="wrap">
        <Button size="xs" colorPalette="blue" onClick={() => decide("approve")}>
          Approve
        </Button>
        <Button size="xs" variant="subtle" onClick={() => decide("deny")}>
          Deny
        </Button>
        <Button size="xs" variant="ghost" onClick={() => decide("always_allow")}>
          Always allow
        </Button>
        <Button size="xs" variant="ghost" onClick={() => decide("always_deny")}>
          Always deny
        </Button>
      </Flex>
    </Box>
  )
}

/// App-level approval bar: renders over whichever tab is active so a pending
/// tool approval is never missed. Empty → renders nothing.
export function Approvals({
  approvals,
  onResolve,
}: {
  approvals: ToolApproval[]
  onResolve: (id: string, decision: ApprovalDecision) => void
}) {
  if (approvals.length === 0) return null
  return (
    <Box className="approvals">
      {approvals.map((approval) => (
        <ApprovalCard key={approval.id} approval={approval} onResolve={onResolve} />
      ))}
    </Box>
  )
}
