{{ with secret "secret/data/gateway" }}
OPENROUTER_API_KEY={{ .Data.data.openrouter_api_key }}
ANTHROPIC_API_KEY={{ .Data.data.anthropic_api_key }}
OPENAI_API_KEY={{ .Data.data.openai_api_key }}
{{ end }}
