#!/bin/bash
set -e

# Wait for primary to be ready
until pg_isready -h "$PRIMARY_HOST" -p 5432 -U "${POSTGRES_USER:-gateway}"; do
    echo "Waiting for primary to be ready..."
    sleep 2
done

# Check if data directory is empty or needs initialization
if [ -z "$(ls -A "$PGDATA" 2>/dev/null)" ]; then
    echo "Initializing replica from primary..."

    # Create base backup from primary
    PGPASSWORD="$REPLICATION_PASSWORD" pg_basebackup \
        -h "$PRIMARY_HOST" \
        -p 5432 \
        -U "${REPLICATION_USER:-repl_user}" \
        -D "$PGDATA" \
        -Fp -Xs -P -R

    # Set correct permissions
    chmod 700 "$PGDATA"

    echo "Replica initialization complete"
fi

# Start PostgreSQL with any additional arguments passed to the container
exec docker-entrypoint.sh "$@"
