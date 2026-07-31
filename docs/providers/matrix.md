# Provider release matrix

No provider adapter may be enabled in a release until this matrix is completed and reviewed for that adapter.

| Adapter              | Surface                   | Authentication                            | Subscription/API use               | Protocol/version | Data/retention disclosure | Terms reviewed | Status           |
| -------------------- | ------------------------- | ----------------------------------------- | ---------------------------------- | ---------------- | ------------------------- | -------------- | ---------------- |
| Mock                 | In-process fixture        | None                                      | None                               | Vea contract v1  | Local synthetic data      | N/A            | Development only |
| Anthropic API        | Official API              | OS-keychain API key                       | API billing                        | Not selected     | Pending                   | Pending        | Planned          |
| OpenAI API           | Official API              | OS-keychain API key/OAuth where supported | API billing                        | Not selected     | Pending                   | Pending        | Planned          |
| Claude native agent  | Official SDK/CLI contract | CLI-owned session                         | Existing account, subject to terms | Not selected     | Pending                   | Pending        | Research         |
| Codex native agent   | Official SDK/app-server   | CLI-owned session                         | Existing account, subject to terms | Not selected     | Pending                   | Pending        | Research         |
| Gemini native agent  | Documented structured CLI | CLI-owned session                         | Existing account, subject to terms | Not selected     | Pending                   | Pending        | Research         |
| Copilot native agent | Official SDK/CLI          | CLI-owned session                         | Existing account, subject to terms | Not selected     | Pending                   | Pending        | Research         |

Vea never copies browser cookies or CLI credential databases, invokes undocumented endpoints, or attempts to evade rate/subscription limits.
