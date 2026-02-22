#!/bin/bash
set -e

# Create replication user
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    CREATE USER ${REPLICATION_USER:-repl_user} WITH REPLICATION ENCRYPTED PASSWORD '${REPLICATION_PASSWORD:-replication}';
EOSQL

# Configure pg_hba.conf for replication - insert at beginning for priority
# Use md5 for compatibility (scram-sha-256 requires the password to be set with it)
{
    echo "host replication ${REPLICATION_USER:-repl_user} 0.0.0.0/0 md5"
    echo "host all ${REPLICATION_USER:-repl_user} 0.0.0.0/0 md5"
    cat "$PGDATA/pg_hba.conf"
} > "$PGDATA/pg_hba.conf.new"
mv "$PGDATA/pg_hba.conf.new" "$PGDATA/pg_hba.conf"

# Reload postgres configuration to pick up pg_hba.conf changes
pg_ctl reload -D "$PGDATA"
