# Vault Agent Configuration
# Automatically fetches secrets and renders them to files

pid_file = "/tmp/vault-agent.pid"

vault {
  address = "http://vault:8200"
}

auto_auth {
  method "approle" {
    config = {
      role_id_file_path   = "/vault/config/role-id"
      secret_id_file_path = "/vault/config/secret-id"
      remove_secret_id_file_after_reading = false
    }
  }

  sink "file" {
    config = {
      path = "/tmp/vault-token"
    }
  }
}

cache {
  use_auto_auth_token = true
}

template {
  source      = "/vault/config/templates/gateway.env.tpl"
  destination = "/run/secrets/gateway.env"
  perms       = 0640
  command     = "pkill -HUP hadrian-gateway || true"
}

template {
  source      = "/vault/config/templates/database_url.tpl"
  destination = "/run/secrets/database_url"
  perms       = 0640
}
