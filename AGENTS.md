# Instruction for Coding Agent

## Prefer Clean Breaking Changes and Prohibit Fallbacks
Because this software is still pre-release, we prefer simple and explicit behavior over complex backward compatibility or fallback logic. When necessary, a clean breaking change is preferable to preserving legacy behavior through additional branches or hidden compatibility layers.

Fallback should generally be treated as an anti-pattern in error handling. Software should be designed so that expected failure modes are handled explicitly, rather than recovered through an alternative behavior after the original operation has failed. A fallback can hide defects, make failures difficult to discover, and leave the system in an unexpected state that appears to be successful.

Expected and recoverable failures should instead have an explicit recovery policy. For example, a temporary network timeout may be retried a limited number of times because transient communication failures are an expected property of remote communication. Such retries must be bounded and have a clearly defined condition for success or failure.

If the predefined recovery policy cannot resolve the problem, the application should fail explicitly rather than continue with another fallback behavior. For example, if a remote database remains unreachable after the allowed retries, the application should report the failure and allow the user to verify the network status or configuration instead of retrying indefinitely or silently switching behavior.

When recovery is impossible, the application should preserve user data as much as possible, surface the error clearly, and provide enough information for the user to understand the cause and take corrective action.