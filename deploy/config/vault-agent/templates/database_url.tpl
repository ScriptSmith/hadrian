{{ with secret "secret/data/gateway/database" }}
postgres://{{ .Data.data.username }}:{{ .Data.data.password }}@postgres:5432/gateway
{{ end }}
