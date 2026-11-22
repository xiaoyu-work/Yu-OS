| Code                  | When the kernel returns it                                                                                          |
|-----------------------|---------------------------------------------------------------------------------------------------------------------|
| `PERMISSION_DENIED`   | Capability, App policy, or owner consent refused the call. |
| `BUDGET_EXCEEDED`     | The available AI budget could not cover the request. |
| `SAFETY_VIOLATION`    | A safety check refused the request. |
| `UNKNOWN_APP`         | `--app <id>` references an app not present under `YUOS_APPS_DIR`.                                                    |
| `UNKNOWN_VERB`        | App doesn't declare this verb in its manifest, OR catalog tool name not in `yu ai tools`.                     |
| `INVALID_ARGS`        | The verb or tool rejected its arguments. |
| `KERNEL_UNAVAILABLE`  | A Yu OS service or its configured model could not complete the call. |
| `INTERNAL_ERROR`      | An internal failure without a more specific code. |

Use the stable code or its SDK error type, not the human-readable message, for
branching. `detail` and `audit_id` are optional; their presence varies by route.
An SDK transport failure may occur without a wire error code.
An error such as a timeout or missing wire response does not prove that the OS
did not accept the request. Do not retry an indeterminate mutation automatically.
