# Model providers

Ultron uses a language model for three roles. Each can use a different provider.

| Role | Used for | Wants |
| --- | --- | --- |
| Router | rewriting an unclear request into a skill command | fast and cheap |
| Desktop agent | multi-step tasks on the screen | strong at computer use |
| Team | research and longer jobs split across specialists | strong reasoning |

## Providers

| Provider | Setup |
| --- | --- |
| **Claude Code CLI** (default) | Install Claude Code; Ultron runs `claude -p`. Uses your Claude plan. |
| **Codex CLI** | Install Codex; Ultron runs `codex exec`. |
| **Local** | Run Ollama, LM Studio, llama.cpp server or vLLM, and give Ultron its OpenAI-compatible URL. Nothing leaves your machine. |
| **API key** | Anthropic, OpenAI, Google Gemini, OpenRouter, or any OpenAI-compatible endpoint. Keys are kept in the OS keychain, never in config files. |

The contract is the same for every provider: the model returns a skill command or a one-sentence
answer, and the router validates it before anything runs.

Status: planned for wave 2 (see the [roadmap](../ROADMAP.md)).
