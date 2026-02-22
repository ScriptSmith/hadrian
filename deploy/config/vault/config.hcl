# Vault Server Configuration

# Storage backend
storage "file" {
  path = "/vault/data"
}

# Listener configuration
listener "tcp" {
  address     = "0.0.0.0:8200"
  tls_disable = true  # Enable TLS in production with proper certificates
}

# API address
api_addr = "http://0.0.0.0:8200"

# Cluster settings (for HA setup)
cluster_addr = "http://0.0.0.0:8201"

# UI
ui = true

# Telemetry
telemetry {
  prometheus_retention_time = "30s"
  disable_hostname          = true
}

# Disable memory locking (enable in production with proper capabilities)
disable_mlock = true

# Logging
log_level = "info"
